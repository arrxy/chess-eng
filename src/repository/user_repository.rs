use crate::db::user_schema::User;
use mongodb::bson::oid::ObjectId;
use mongodb::{Collection, bson::doc};

#[derive(Clone)]
pub struct UserRepository {
    users: Collection<User>,
}

impl UserRepository {
    pub fn new(users: Collection<User>) -> Self {
        Self { users }
    }

    pub(crate) async fn find_by_email(&self, email: &str) -> mongodb::error::Result<Option<User>> {
        self.users.find_one(doc! { "email": email }).await
    }

    pub(crate) async fn find_by_user_id(
        &self,
        id: &ObjectId,
    ) -> mongodb::error::Result<Option<User>> {
        self.users.find_one(doc! { "_id": id }).await
    }
}
