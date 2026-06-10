use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use axum::response::{IntoResponse, Response};
use futures_util::TryStreamExt;
use mongodb::{
    bson::{doc, oid::ObjectId, DateTime, Document},
    Collection,
};
use serde_json::{json, Value};
use crate::db::game_schema::GameStatus;
use crate::db::mongo::Db;
use crate::server::{color_str, piece_type_str, AppState};
use crate::server::auth::user_from_headers;

const USER_COLLECTION: &str = "users";

pub async fn my_games(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(user) = user_from_headers(&state.db, &headers).await else {
        return crate::server::auth::error_response(StatusCode::UNAUTHORIZED, "not signed in");
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
            return crate::server::auth::error_response(StatusCode::INTERNAL_SERVER_ERROR, "database error");
        }
    };
    let games: Vec<_> = match cursor.try_collect::<Vec<_>>().await {
        Ok(g) => g,
        Err(e) => {
            eprintln!("game history query failed: {e}");
            return crate::server::auth::error_response(StatusCode::INTERNAL_SERVER_ERROR, "database error");
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
