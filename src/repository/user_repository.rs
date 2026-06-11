use crate::db::user_schema::User;
use mongodb::bson::oid::ObjectId;
use mongodb::options::ReturnDocument;
use mongodb::{
    Collection,
    bson::{DateTime, doc},
};

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

    pub async fn upsert_google_user(
        &self,
        google_id: &str,
        email: Option<String>,
        name: Option<String>,
        picture: Option<String>,
    ) -> mongodb::error::Result<Option<User>> {
        let now = DateTime::now();
        let mut set = doc! {
            "google_id": google_id,
            "updated_at": now,
        };

        if let Some(email) = email {
            set.insert("email", email);
        }

        if let Some(name) = name {
            set.insert("name", name);
        }

        if let Some(picture) = picture {
            set.insert("picture", picture);
        }

        self.users
            .find_one_and_update(
                doc! { "google_id": google_id },
                doc! {
                    "$set": set,
                    "$setOnInsert": {
                        "created_at": now,
                    }
                },
            )
            .upsert(true)
            .return_document(ReturnDocument::After)
            .await
    }
}
