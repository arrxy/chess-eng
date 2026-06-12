mod board;
mod db;
mod pieces;
mod redis_state;
mod repository;
mod routes;
mod server;
mod service;

use db::mongo::Db;
use server::auth;

#[tokio::main]
async fn main() {
    let _ = dotenvy::dotenv();

    let db = Db::init().await.expect("failed to set up MongoDB client");
    if let Err(e) = db.ensure_indexes().await {
        eprintln!("warning: could not create MongoDB indexes (is mongod running?): {e}");
    }

    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    let server_id = std::env::var("SERVER_ID")
        .unwrap_or_else(|_| uuid::Uuid::new_v4().to_string());
    let redis = redis_state::pool::init_pool(&redis_url)
        .await
        .expect("failed to connect to Redis");

    let google = match std::env::var("GOOGLE_CLIENT_ID") {
        Ok(client_id) => Some(auth::GoogleVerifier::new(client_id)),
        Err(_) => {
            eprintln!("warning: GOOGLE_CLIENT_ID unset — Google login disabled");
            None
        }
    };

    routes::router::route(db, google, redis, redis_url, server_id).await;
}
