use mongodb::{
    bson::{oid::ObjectId, DateTime, doc},
    IndexModel,
};
use mongo::{build_unique_index};
use serde::{Deserialize, Serialize};
use crate::db::mongo;

#[derive(Debug, Serialize, Deserialize)]
pub struct User {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub google_id: String,

    pub name: Option<String>,
    pub picture: Option<String>,
    pub email: Option<String>,

    pub username: Option<String>,

    pub created_at: DateTime,
    pub updated_at: DateTime,
}

impl User {
    pub fn new(
        google_id: String,
        name: Option<String>,
        picture: Option<String>,
        email: Option<String>,
        username: Option<String>,
    ) -> Self {
        let now = DateTime::now();

        Self {
            id: None,
            google_id,
            name,
            picture,
            email,
            username,
            created_at: now,
            updated_at: now,
        }
    }
    
    pub fn indexes() -> Vec<IndexModel> {
        vec![
            build_unique_index("google_id", "google_id_index"),
            build_unique_index("email", "unique_email"),
            build_unique_index("username", "unique_username"),
        ]
    }
}