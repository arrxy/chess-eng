use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};

use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use mongodb::bson::{doc};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use tokio::sync::RwLock;
use uuid::Uuid;

pub(crate) use super::{AppState, SessionUser};

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

pub async fn user_from_headers(state: &AppState, headers: &HeaderMap) -> Option<SessionUser> {
    let token = session_token(headers)?;
    let session = state
        .session_repository
        .find_session_by_token(&token)
        .await
        .ok()??;
    let user = state
        .user_repository
        .find_by_user_id(&session.user_id)
        .await
        .ok()??;
    Some(SessionUser {
        id: user.id?,
        name: user
            .name
            .or(user.email)
            .unwrap_or_else(|| "Player".to_string()),
        picture: user.picture,
    })
}

fn user_json(user: &SessionUser) -> Value {
    json!({ "name": user.name, "picture": user.picture })
}

pub(crate) fn error_response(status: StatusCode, message: &str) -> Response {
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

pub async fn auth_google(State(state): State<AppState>, Json(body): Json<GoogleLogin>) -> Response {
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

    let user = match state.user_repository.upsert_google_user(
        &claims.sub,
        claims.email.clone(),
        claims.name.clone(),
        claims.picture.clone(),
    ).await {
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
        .session_repository
        .create_session(&token, user_id)
        .await {
        eprintln!("session insert failed: {e}");
        return error_response(StatusCode::INTERNAL_SERVER_ERROR, "database error");
    }

    let session_user = SessionUser {
        id: user_id,
        name: user
            .name
            .or(user.email)
            .unwrap_or_else(|| "Player".to_string()),
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
    let user = user_from_headers(&state, &headers).await;
    Json(json!({ "user": user.as_ref().map(user_json) }))
}

pub async fn auth_logout(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Some(token) = session_token(&headers) {
        let _ = state.session_repository.delete_session_by_token(&token).await;
    }
    let cookie = format!("{SESSION_COOKIE}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0");
    ([(header::SET_COOKIE, cookie)], Json(json!({ "ok": true }))).into_response()
}
