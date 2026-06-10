use crate::db::game_schema::Game;
use futures_util::TryStreamExt;
use mongodb::bson::oid::ObjectId;
use mongodb::{Collection, bson::doc};

#[derive(Clone)]
pub struct GameRepository {
    games: Collection<Game>,
}

impl GameRepository {
    pub fn new(games: Collection<Game>) -> Self {
        Self { games }
    }

    pub(crate) async fn get_games_by_user_id(
        &self,
        user_id: ObjectId,
        page: u64,
        limit: u64,
    ) -> mongodb::error::Result<Vec<Game>> {
        let page = page.max(1);
        let limit = limit.clamp(1, 100);
        let cursor = self
            .games
            .find(doc! {
                "$or": [
                    { "white_user_id": user_id },
                    { "black_user_id": user_id },
                ]
            })
            .sort(doc! { "created_at": -1 })
            .skip((page - 1) * limit)
            .limit(limit as i64)
            .await?;
        let sessions = cursor.try_collect().await?;
        Ok(sessions)
    }
}
