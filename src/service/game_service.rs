use crate::db::game_schema::{Game, GameStatus, Move};
use crate::server;
use crate::server::auth::user_from_headers;
use crate::server::{AppState, auth};
use crate::server::{color_str, piece_type_str};
use axum::Json;
use axum::extract::{Query, State, WebSocketUpgrade};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use mongodb::bson::oid::ObjectId;
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Debug, Deserialize)]
pub struct GamesQuery {
    pub page: Option<u64>,
    pub limit: Option<u64>,
}

pub async fn my_games(
    State(state): State<AppState>,
    Query(query): Query<GamesQuery>,
    headers: HeaderMap,
) -> Response {
    let Some(user) = user_from_headers(&state, &headers).await else {
        return auth::error_response(StatusCode::UNAUTHORIZED, "not signed in");
    };
    let games = match state
        .game_repository
        .get_games_by_user_id(user.id, query.page.unwrap_or(1), query.limit.unwrap_or(20))
        .await
    {
        Ok(games) => games,
        Err(_) => {
            return auth::error_response(StatusCode::INTERNAL_SERVER_ERROR, "database error");
        }
    };

    let response = games_response(&games, user.id);
    Json(response).into_response()
}

pub(crate) async fn stats(State(state): State<AppState>) -> impl IntoResponse {
    let count = state.games.lock().unwrap().len();
    Json(json!({ "games": count }))
}

pub(crate) async fn ws_upgrade(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    // The browser sends the session cookie with the upgrade request, so the
    // connection knows who is playing before any message arrives.
    let user = user_from_headers(&state, &headers).await;
    ws.on_upgrade(move |socket| server::ws::handle_socket(socket, state, user))
}

fn games_response(games: &[Game], user_id: ObjectId) -> Value {
    let games: Vec<Value> = games
        .iter()
        .map(|game| game_to_json(game, user_id))
        .collect();
    json!({ "games": games })
}

fn game_to_json(game: &Game, user_id: ObjectId) -> Value {
    json!({
        "id": game.id.map(|id| id.to_hex()),
        "white_name": game.white_name,
        "black_name": game.black_name,
        "your_color": your_color(game, user_id),
        "result": game_result_str(game.status),
        "moves": moves_to_json(&game.moves),
        "created_at": game.created_at.timestamp_millis(),
    })
}

fn your_color(game: &Game, user_id: ObjectId) -> &'static str {
    if game.white_user_id == Some(user_id) {
        "white"
    } else {
        "black"
    }
}

fn game_result_str(status: GameStatus) -> &'static str {
    match status {
        GameStatus::InProgress => "in_progress",
        GameStatus::WhiteWon => "white_won",
        GameStatus::BlackWon => "black_won",
        GameStatus::Draw => "draw",
        GameStatus::Abandoned => "abandoned",
    }
}

fn moves_to_json(moves: &[Move]) -> Vec<Value> {
    moves.iter().map(move_to_json).collect()
}

fn move_to_json(m: &Move) -> Value {
    json!({
        "color": color_str(m.color),
        "piece": piece_type_str(m.piece),
        "from": {
            "x": m.from_x,
            "y": m.from_y,
        },
        "to": {
            "x": m.to_x,
            "y": m.to_y,
        },
        "captured": m.captured.map(piece_type_str),
        "promotion": m.promotion.map(piece_type_str),
    })
}
