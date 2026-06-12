use crate::db::mongo::Db;
use crate::redis_state::pool::RedisPool;
use crate::repository::game_repository::GameRepository;
use crate::server::{AppState, auth};
use crate::service::frontend_service;
use crate::service::game_service;
use axum::{
    Router,
    routing::{get, post},
};
use std::env;

pub async fn route(
    db: Db,
    google: Option<auth::GoogleVerifier>,
    redis: RedisPool,
    redis_url: String,
    server_id: String,
    game_repository: GameRepository,
) {
    let _ = dotenvy::dotenv();

    let state = AppState::new(db, google, redis, redis_url, server_id, game_repository);
    let app = Router::new()
        .route("/", get(frontend_service::serve_html))
        .route("/app.js", get(frontend_service::serve_js))
        .route("/app.css", get(frontend_service::serve_css))
        .route("/ws", get(game_service::ws_upgrade))
        .route("/stats", get(game_service::stats))
        .route("/auth/config", get(auth::auth_config))
        .route("/auth/google", post(auth::auth_google))
        .route("/auth/me", get(auth::auth_me))
        .route("/auth/logout", post(auth::auth_logout))
        .route("/api/games", get(game_service::my_games))
        .with_state(state);

    let port = env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .unwrap();
    println!("Chess server running at http://localhost:{}", port);
    axum::serve(listener, app).await.unwrap();
}
