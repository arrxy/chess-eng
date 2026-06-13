use crate::db::game_schema::{Game, GameStatus, Move};
use futures_util::TryStreamExt;
use mongodb::bson::{DateTime, doc, oid::ObjectId, to_bson};
use mongodb::{Collection, bson};

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
                "status": { "$ne": "InProgress" },
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

    /// Insert a new in-progress game document; returns its assigned ObjectId.
    pub async fn create_in_progress(&self, doc: Game) -> mongodb::error::Result<ObjectId> {
        let result = self.games.insert_one(doc).await?;
        Ok(result.inserted_id.as_object_id().unwrap())
    }

    /// Append move batches to multiple game documents concurrently.
    /// Also bumps `updated_at` so the inactivity sweeper can tell live games
    /// from idle ones.
    pub async fn bulk_push_moves(
        &self,
        batches: Vec<(ObjectId, Vec<Move>)>,
    ) -> mongodb::error::Result<()> {
        if batches.is_empty() {
            return Ok(());
        }
        let mut futs = Vec::with_capacity(batches.len());
        for (id, moves) in batches {
            let moves_bson = to_bson(&moves).unwrap_or(bson::Bson::Array(vec![]));
            let col = self.games.clone();
            futs.push(async move {
                col.update_one(
                    doc! { "_id": id },
                    doc! {
                        "$push": { "moves": { "$each": moves_bson } },
                        "$set": { "updated_at": DateTime::now() },
                    },
                )
                .await
            });
        }
        futures_util::future::try_join_all(futs).await?;
        Ok(())
    }

    /// Mark a game as recently active without changing moves/status.
    /// Called on disconnect so the rejoin window is measured from the
    /// last event, not just the last move.
    pub async fn touch_game(&self, id: ObjectId) -> mongodb::error::Result<()> {
        self.games
            .update_one(
                doc! { "_id": id },
                doc! { "$set": { "updated_at": DateTime::now() } },
            )
            .await?;
        Ok(())
    }

    /// In-progress games the user is part of — used to offer rejoin.
    pub async fn get_active_games_by_user(
        &self,
        user_id: ObjectId,
    ) -> mongodb::error::Result<Vec<Game>> {
        let cursor = self
            .games
            .find(doc! {
                "status": "InProgress",
                "$or": [
                    { "white_user_id": user_id },
                    { "black_user_id": user_id },
                ]
            })
            .sort(doc! { "updated_at": -1 })
            .await?;
        cursor.try_collect().await
    }

    /// In-progress games whose last activity is older than `cutoff`.
    /// The sweeper finalizes these as Abandoned.
    pub async fn find_stale_in_progress(
        &self,
        cutoff: DateTime,
    ) -> mongodb::error::Result<Vec<Game>> {
        let cursor = self
            .games
            .find(doc! {
                "status": "InProgress",
                "updated_at": { "$lt": cutoff },
            })
            .await?;
        cursor.try_collect().await
    }

    /// Set the final status and updated_at on a finished game.
    pub async fn finalize_game(
        &self,
        id: ObjectId,
        status: GameStatus,
    ) -> mongodb::error::Result<()> {
        let status_bson = to_bson(&status).unwrap_or(bson::Bson::Null);
        self.games
            .update_one(
                doc! { "_id": id },
                doc! { "$set": { "status": status_bson, "updated_at": DateTime::now() } },
            )
            .await?;
        Ok(())
    }
}
