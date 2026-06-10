mod board;
mod pieces;
mod server;
mod db;

use axum::{
    extract::{ws::WebSocketUpgrade, State},
    http::HeaderMap,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use db::mongo::Db;
use server::{auth, AppState};
use serde_json::json;

#[tokio::main]
async fn main() {
    let _ = dotenvy::dotenv();

    let db = Db::init().await.expect("failed to set up MongoDB client");
    if let Err(e) = db.ensure_indexes().await {
        eprintln!("warning: could not create MongoDB indexes (is mongod running?): {e}");
    }

    let google = match std::env::var("GOOGLE_CLIENT_ID") {
        Ok(client_id) => Some(auth::GoogleVerifier::new(client_id)),
        Err(_) => {
            eprintln!("warning: GOOGLE_CLIENT_ID unset — Google login disabled");
            None
        }
    };

    let state = AppState::new(db, google);

    let app = Router::new()
        .route("/", get(serve_html))
        .route("/ws", get(ws_upgrade))
        .route("/stats", get(stats))
        .route("/auth/config", get(auth::auth_config))
        .route("/auth/google", post(auth::auth_google))
        .route("/auth/me", get(auth::auth_me))
        .route("/auth/logout", post(auth::auth_logout))
        .route("/api/games", get(auth::my_games))
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
    headers: HeaderMap,
) -> impl IntoResponse {
    // The browser sends the session cookie with the upgrade request, so the
    // connection knows who is playing before any message arrives.
    let user = auth::user_from_headers(&state.db, &headers).await;
    ws.on_upgrade(move |socket| server::ws::handle_socket(socket, state, user))
}
