use crate::db::session_schema::Session;
use mongodb::{Collection, bson::doc};
use mongodb::bson::oid::ObjectId;

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
    pub async fn create_session(
        &self,
        token: &str,
        user_id: ObjectId,
    ) -> mongodb::error::Result<()> {
        self.sessions
            .insert_one(Session::new(token.to_string(), user_id))
            .await?;
        Ok(())
    }

    pub async fn delete_session_by_token(
        &self,
        token: &str,
    ) -> mongodb::error::Result<()> {
        self.sessions
            .delete_one(doc! { "token": token })
            .await?;
        Ok(())
    }
}
