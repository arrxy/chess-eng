mod board;
mod db;
mod pieces;
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

    let google = match std::env::var("GOOGLE_CLIENT_ID") {
        Ok(client_id) => Some(auth::GoogleVerifier::new(client_id)),
        Err(_) => {
            eprintln!("warning: GOOGLE_CLIENT_ID unset — Google login disabled");
            None
        }
    };
    routes::router::route(db, google).await;
}
