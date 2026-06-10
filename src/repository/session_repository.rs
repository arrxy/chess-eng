use crate::db::session_schema::Session;
use mongodb::{Collection, bson::doc};

#[derive(Clone)]
pub struct SessionRepository {
    sessions: Collection<Session>,
}

impl SessionRepository {
    pub fn new(sessions: Collection<Session>) -> Self {
        Self { sessions }
    }

    pub(crate) async fn find_session_by_token(
        &self,
        token: &str,
    ) -> mongodb::error::Result<Option<Session>> {
        self.sessions.find_one(doc! { "token": token }).await
    }
}
