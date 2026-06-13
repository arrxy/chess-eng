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
use redis_state::shards::Shards;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    let _ = dotenvy::dotenv();

    let db = Db::init().await.expect("failed to set up MongoDB client");
    if let Err(e) = db.ensure_indexes().await {
        eprintln!("warning: could not create MongoDB indexes (is mongod running?): {e}");
    }

    // REDIS_URLS = comma-separated shard nodes; falls back to a single REDIS_URL.
    let urls: Vec<String> = match std::env::var("REDIS_URLS") {
        Ok(s) => s
            .split(',')
            .map(|u| u.trim().to_string())
            .filter(|u| !u.is_empty())
            .collect(),
        Err(_) => vec![
            std::env::var("REDIS_URL")
                .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string()),
        ],
    };
    let server_id = std::env::var("SERVER_ID")
        .unwrap_or_else(|_| uuid::Uuid::new_v4().to_string());
    let shards = Arc::new(
        Shards::new(urls)
            .await
            .expect("failed to connect to Redis shards"),
    );
    println!("connected to {} redis shard(s)", shards.len());

    // Register this server and create the stream consumer group (idempotent),
    // both on the coordination node.
    {
        use bb8_redis::redis::AsyncCommands;
        let mut conn = shards.coord().get().await.expect("redis get conn");
        let _: Result<i64, _> = conn.sadd("known_servers", &server_id).await;
    }
    redis_state::stream::ensure_consumer_group(shards.coord())
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
        shards.clone(),
        server_id.clone(),
        game_repository.clone(),
    ));
    tokio::spawn(background::batch_flush::run(
        shards.clone(),
        server_id.clone(),
        game_repository.clone(),
    ));
    tokio::spawn(background::sweeper::run(
        shards.clone(),
        game_repository.clone(),
    ));

    routes::router::route(db, google, shards, server_id, game_repository).await;
}
