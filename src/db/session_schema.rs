use std::time::Duration;

use mongodb::{
    bson::{oid::ObjectId, DateTime},
    IndexModel,
};
use serde::{Deserialize, Serialize};

use crate::db::mongo::{build_ttl_index, build_unique_index};

pub const SESSION_TTL: Duration = Duration::from_secs(30 * 24 * 60 * 60); // 30 days

#[derive(Debug, Serialize, Deserialize)]
pub struct Session {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub token: String,
    pub user_id: ObjectId,
    pub created_at: DateTime,
}

impl Session {
    pub fn new(token: String, user_id: ObjectId) -> Self {
        Self {
            id: None,
            token,
            user_id,
            created_at: DateTime::now(),
        }
    }

    pub fn indexes() -> Vec<IndexModel> {
        vec![
            build_unique_index("token", "unique_session_token"),
            build_ttl_index("created_at", "session_ttl", SESSION_TTL),
        ]
    }
}
