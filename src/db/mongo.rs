use std::string::String;
use mongodb::{Client, Database, IndexModel};
use std::env;
use mongodb::bson::doc;
use mongodb::options::IndexOptions;

pub async fn connect_db () -> mongodb::error::Result<Database> {
    let _ = dotenvy::dotenv();
    let uri = env::var(String::from("MONGODB_URI")).expect("MONGODB_URI must be set");
    let db_name = env::var(String::from("MONGODB_DB")).expect("MONGODB_DB must be set");
    let client = Client::with_uri_str(uri).await?;
    Ok(client.database(db_name.as_str()))
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

pub(crate) fn build_index(key: &str, index_name: &str) -> IndexModel {
    IndexModel::builder().keys(doc! { key: 1 })
        .options(
            IndexOptions::builder()
                .name(index_name.to_string())
                .build(),
        )
        .build()
}