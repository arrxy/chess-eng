mod background;
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
use repository::game_repository::GameRepository;

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

    // Register this server and create the stream consumer group (idempotent).
    {
        use bb8_redis::redis::AsyncCommands;
        let mut conn = redis.get().await.expect("redis get conn");
        let _: Result<i64, _> = conn.sadd("known_servers", &server_id).await;
    }
    redis_state::stream::ensure_consumer_group(&redis)
        .await
        .expect("failed to create stream consumer group");

    let google = match std::env::var("GOOGLE_CLIENT_ID") {
        Ok(client_id) => Some(auth::GoogleVerifier::new(client_id)),
        Err(_) => {
            eprintln!("warning: GOOGLE_CLIENT_ID unset — Google login disabled");
            None
        }
    };

    let game_repository = GameRepository::new(db.games.clone());

    // Spawn background tasks.
    tokio::spawn(background::heartbeat::run(
        redis.clone(),
        server_id.clone(),
        game_repository.clone(),
    ));
    tokio::spawn(background::batch_flush::run(
        redis.clone(),
        server_id.clone(),
        game_repository.clone(),
    ));

    routes::router::route(db, google, redis, redis_url, server_id, game_repository).await;
}
