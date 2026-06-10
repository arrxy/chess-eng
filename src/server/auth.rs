use std::collections::HashMap;

use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use futures_util::TryStreamExt;
use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use mongodb::bson::{doc, DateTime};
use mongodb::options::ReturnDocument;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::db::game_schema::GameStatus;
use crate::db::mongo::Db;
use crate::db::session_schema::Session;
use crate::db::user_schema::User;
use super::{color_str, piece_type_str, AppState, SessionUser};

const GOOGLE_JWKS_URL: &str = "https://www.googleapis.com/oauth2/v3/certs";
const SESSION_COOKIE: &str = "session";
const SESSION_MAX_AGE_SECS: u64 = 30 * 24 * 60 * 60; // keep in sync with session TTL index

// ── Google ID token verification ────────────────────────────────────────────

#[derive(Deserialize, Clone)]
struct Jwk {
    kid: String,
    n: String,
    e: String,
}

#[derive(Deserialize)]
struct JwkSet {
    keys: Vec<Jwk>,
}

#[derive(Deserialize)]
pub struct GoogleClaims {
    pub sub: String,
    pub email: Option<String>,
    pub name: Option<String>,
    pub picture: Option<String>,
}

pub struct GoogleVerifier {
    client_id: String,
    http: reqwest::Client,
    keys: RwLock<HashMap<String, Jwk>>,
}

impl GoogleVerifier {
    pub fn new(client_id: String) -> Self {
        Self {
            client_id,
            http: reqwest::Client::new(),
            keys: RwLock::new(HashMap::new()),
        }
    }

    pub fn client_id(&self) -> &str {
        &self.client_id
    }

    async fn key_for(&self, kid: &str) -> Option<Jwk> {
        if let Some(k) = self.keys.read().await.get(kid) {
            return Some(k.clone());
        }
        // Unknown kid: Google rotates its signing keys, refetch the set.
        let set: JwkSet = self
            .http
            .get(GOOGLE_JWKS_URL)
            .send()
            .await
            .ok()?
            .json()
            .await
            .ok()?;
        let mut keys = self.keys.write().await;
        keys.clear();
        for k in set.keys {
            keys.insert(k.kid.clone(), k);
        }
        keys.get(kid).cloned()
    }

    pub async fn verify(&self, token: &str) -> Result<GoogleClaims, String> {
        let header =
            jsonwebtoken::decode_header(token).map_err(|e| format!("bad token header: {e}"))?;
        let kid = header.kid.ok_or("token has no key id")?;
        let jwk = self
            .key_for(&kid)
            .await
            .ok_or("no matching Google signing key")?;
        let key = DecodingKey::from_rsa_components(&jwk.n, &jwk.e)
            .map_err(|e| format!("bad signing key: {e}"))?;
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_audience(&[&self.client_id]);
        validation.set_issuer(&["https://accounts.google.com", "accounts.google.com"]);
        let data = jsonwebtoken::decode::<GoogleClaims>(token, &key, &validation)
            .map_err(|e| format!("invalid token: {e}"))?;
        Ok(data.claims)
    }
}

// ── Session helpers ──────────────────────────────────────────────────────────

fn session_token(headers: &HeaderMap) -> Option<String> {
    let cookies = headers.get(header::COOKIE)?.to_str().ok()?;
    cookies.split(';').find_map(|c| {
        let (k, v) = c.trim().split_once('=')?;
        (k == SESSION_COOKIE).then(|| v.to_string())
    })
}

pub async fn user_from_headers(db: &Db, headers: &HeaderMap) -> Option<SessionUser> {
    let token = session_token(headers)?;
    let session = db.sessions.find_one(doc! { "token": &token }).await.ok()??;
    let user = db
        .users
        .find_one(doc! { "_id": session.user_id })
        .await
        .ok()??;
    Some(SessionUser {
        id: user.id?,
        name: user.name.or(user.email).unwrap_or_else(|| "Player".to_string()),
        picture: user.picture,
    })
}

fn user_json(user: &SessionUser) -> Value {
    json!({ "name": user.name, "picture": user.picture })
}

fn error_response(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({ "error": message }))).into_response()
}

// ── HTTP handlers ────────────────────────────────────────────────────────────

pub async fn auth_config(State(state): State<AppState>) -> Json<Value> {
    Json(json!({
        "google_client_id": state.google.as_ref().map(|g| g.client_id()),
    }))
}

#[derive(Deserialize)]
pub struct GoogleLogin {
    credential: String,
}

pub async fn auth_google(
    State(state): State<AppState>,
    Json(body): Json<GoogleLogin>,
) -> Response {
    let Some(google) = &state.google else {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "Google login is not configured (GOOGLE_CLIENT_ID unset)",
        );
    };

    let claims = match google.verify(&body.credential).await {
        Ok(c) => c,
        Err(e) => return error_response(StatusCode::UNAUTHORIZED, &e),
    };

    let now = DateTime::now();
    let mut set = doc! { "updated_at": now };
    if let Some(v) = &claims.name {
        set.insert("name", v);
    }
    if let Some(v) = &claims.picture {
        set.insert("picture", v);
    }
    if let Some(v) = &claims.email {
        set.insert("email", v);
    }

    let user: Option<User> = match state
        .db
        .users
        .find_one_and_update(
            doc! { "google_id": &claims.sub },
            doc! { "$set": set, "$setOnInsert": { "created_at": now } },
        )
        .upsert(true)
        .return_document(ReturnDocument::After)
        .await
    {
        Ok(u) => u,
        Err(e) => {
            eprintln!("user upsert failed: {e}");
            return error_response(StatusCode::INTERNAL_SERVER_ERROR, "database error");
        }
    };
    let Some(user) = user else {
        return error_response(StatusCode::INTERNAL_SERVER_ERROR, "database error");
    };
    let Some(user_id) = user.id else {
        return error_response(StatusCode::INTERNAL_SERVER_ERROR, "database error");
    };

    let token = Uuid::new_v4().to_string();
    if let Err(e) = state
        .db
        .sessions
        .insert_one(Session::new(token.clone(), user_id))
        .await
    {
        eprintln!("session insert failed: {e}");
        return error_response(StatusCode::INTERNAL_SERVER_ERROR, "database error");
    }

    let session_user = SessionUser {
        id: user_id,
        name: user.name.or(user.email).unwrap_or_else(|| "Player".to_string()),
        picture: user.picture,
    };
    let cookie = format!(
        "{SESSION_COOKIE}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={SESSION_MAX_AGE_SECS}"
    );
    (
        [(header::SET_COOKIE, cookie)],
        Json(json!({ "user": user_json(&session_user) })),
    )
        .into_response()
}

pub async fn auth_me(State(state): State<AppState>, headers: HeaderMap) -> Json<Value> {
    let user = user_from_headers(&state.db, &headers).await;
    Json(json!({ "user": user.as_ref().map(user_json) }))
}

pub async fn auth_logout(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Some(token) = session_token(&headers) {
        let _ = state.db.sessions.delete_one(doc! { "token": token }).await;
    }
    let cookie = format!("{SESSION_COOKIE}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0");
    ([(header::SET_COOKIE, cookie)], Json(json!({ "ok": true }))).into_response()
}

// ── Game history ─────────────────────────────────────────────────────────────

pub async fn my_games(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(user) = user_from_headers(&state.db, &headers).await else {
        return error_response(StatusCode::UNAUTHORIZED, "not signed in");
    };

    let filter = doc! {
        "$or": [
            { "white_user_id": user.id },
            { "black_user_id": user.id },
        ]
    };
    let cursor = match state
        .db
        .games
        .find(filter)
        .sort(doc! { "created_at": -1 })
        .limit(50)
        .await
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("game history query failed: {e}");
            return error_response(StatusCode::INTERNAL_SERVER_ERROR, "database error");
        }
    };
    let games: Vec<_> = match cursor.try_collect::<Vec<_>>().await {
        Ok(g) => g,
        Err(e) => {
            eprintln!("game history query failed: {e}");
            return error_response(StatusCode::INTERNAL_SERVER_ERROR, "database error");
        }
    };

    let games: Vec<Value> = games
        .iter()
        .map(|g| {
            let your_color = if g.white_user_id == Some(user.id) {
                "white"
            } else {
                "black"
            };
            let result = match g.status {
                GameStatus::WhiteWon => "white_won",
                GameStatus::BlackWon => "black_won",
                GameStatus::Draw => "draw",
                GameStatus::Abandoned => "abandoned",
            };
            let moves: Vec<Value> = g
                .moves
                .iter()
                .map(|m| {
                    json!({
                        "color": color_str(m.color),
                        "piece": piece_type_str(m.piece),
                        "from": { "x": m.from_x, "y": m.from_y },
                        "to": { "x": m.to_x, "y": m.to_y },
                        "captured": m.captured.map(piece_type_str),
                    })
                })
                .collect();
            json!({
                "id": g.id.map(|id| id.to_hex()),
                "white_name": g.white_name,
                "black_name": g.black_name,
                "your_color": your_color,
                "result": result,
                "moves": moves,
                "created_at": g.created_at.timestamp_millis(),
            })
        })
        .collect();

    Json(json!({ "games": games })).into_response()
}
