mod board;
mod pieces;
mod server;
mod db;

use axum::{
    extract::{ws::WebSocketUpgrade, State},
    response::IntoResponse,
    routing::get,
    Router,
};
use server::AppState;
use serde_json::json;

#[tokio::main]
async fn main() {
    let state = AppState::new();

    let app = Router::new()
        .route("/", get(serve_html))
        .route("/ws", get(ws_upgrade))
        .route("/stats", get(stats))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Chess server running at http://localhost:3000");
    axum::serve(listener, app).await.unwrap();
}

async fn serve_html() -> impl IntoResponse {
    axum::response::Html(include_str!("../static/index.html"))
}

async fn stats(State(state): State<AppState>) -> impl IntoResponse {
    let count = state.games.lock().unwrap().len();
    axum::Json(json!({ "games": count }))
}

async fn ws_upgrade(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| server::ws::handle_socket(socket, state))
}
