use mongodb::{
    bson::{doc, oid::ObjectId, DateTime},
    IndexModel,
};
use serde::{Deserialize, Serialize};
use crate::pieces::pieces::{Color, PieceType};
use crate::db::mongo::{build_index};

#[derive(Debug, Serialize, Deserialize)]
pub struct Move {
    pub color: Color,
    pub piece: PieceType,

    // from square
    pub from_x: usize,
    pub from_y: usize,

    // to square
    pub to_x: usize,
    pub to_y: usize,

    pub created_at: DateTime,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Game {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,

    pub white_user_id: ObjectId,
    pub black_user_id: ObjectId,

    pub moves: Vec<Move>,

    pub status: GameStatus,

    pub created_at: DateTime,
    pub updated_at: DateTime,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum GameStatus {
    Waiting,
    Active,
    WhiteWon,
    BlackWon,
    Draw,
}

impl Game {
    pub fn new(
        white_user_id: ObjectId,
        black_user_id: ObjectId,
    ) -> Self {
        let now = DateTime::now();

        Self {
            id: None,
            white_user_id,
            black_user_id,
            moves: Vec::new(),
            status: GameStatus::Active,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn indexes() -> Vec<IndexModel> {
        vec![
            build_index("white_user_id", "idx_white_user_id"),
            build_index("black_user_id", "idx_black_user_id"),
            build_index("status", "idx_game_status"),
        ]
    }
}