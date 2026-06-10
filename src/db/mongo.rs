use std::time::Duration;

use mongodb::{Client, Collection, Database, IndexModel};
use std::env;
use mongodb::bson::doc;
use mongodb::options::{ClientOptions, IndexOptions};

use crate::db::game_schema::Game;
use crate::db::session_schema::Session;
use crate::db::user_schema::User;

pub async fn connect_db() -> mongodb::error::Result<Database> {
    let _ = dotenvy::dotenv();
    let uri = env::var("MONGODB_URI").expect("MONGODB_URI must be set");
    let db_name = env::var("MONGODB_DB").expect("MONGODB_DB must be set");
    let mut options = ClientOptions::parse(uri).await?;
    options.server_selection_timeout = Some(Duration::from_secs(5));
    let client = Client::with_options(options)?;
    Ok(client.database(db_name.as_str()))
}

/// Typed handles to the collections the app uses.
pub struct Db {
    pub users: Collection<User>,
    pub games: Collection<Game>,
    pub sessions: Collection<Session>,
}

impl Db {
    pub async fn init() -> mongodb::error::Result<Self> {
        let db = connect_db().await?;
        Ok(Self {
            users: db.collection("users"),
            games: db.collection("games"),
            sessions: db.collection("sessions"),
        })
    }

    pub async fn ensure_indexes(&self) -> mongodb::error::Result<()> {
        self.users.create_indexes(User::indexes()).await?;
        self.games.create_indexes(Game::indexes()).await?;
        self.sessions.create_indexes(Session::indexes()).await?;
        Ok(())
    }
}

pub(crate) fn build_unique_index(key: &str, index_name: &str) -> IndexModel {
    IndexModel::builder().keys(doc! { key: 1 })
        .options(
            IndexOptions::builder()
                .unique(true)
                .name(index_name.to_string())
                .build(),
        )
        .build()
}

/// Unique only across documents that actually have the field — lets many
/// documents omit an optional field (e.g. username) without colliding.
pub(crate) fn build_sparse_unique_index(key: &str, index_name: &str) -> IndexModel {
    IndexModel::builder().keys(doc! { key: 1 })
        .options(
            IndexOptions::builder()
                .unique(true)
                .sparse(true)
                .name(index_name.to_string())
                .build(),
        )
        .build()
}

pub(crate) fn build_index(key: &str, index_name: &str) -> IndexModel {
    IndexModel::builder().keys(doc! { key: 1 })
        .options(
            IndexOptions::builder()
                .name(index_name.to_string())
                .build(),
        )
        .build()
}

/// Expires documents `ttl` after the value of `key` (a Date field).
pub(crate) fn build_ttl_index(key: &str, index_name: &str, ttl: Duration) -> IndexModel {
    IndexModel::builder().keys(doc! { key: 1 })
        .options(
            IndexOptions::builder()
                .expire_after(ttl)
                .name(index_name.to_string())
                .build(),
        )
        .build()
}
