use crate::db::mongo::build_index;
use crate::pieces::pieces::{Color, PieceType};
use mongodb::{
    IndexModel,
    bson::{DateTime, oid::ObjectId},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Move {
    pub color: Color,
    pub piece: PieceType,

    // from square
    pub from_x: usize,
    pub from_y: usize,

    // to square
    pub to_x: usize,
    pub to_y: usize,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub captured: Option<PieceType>,

    // set when a pawn reached the last rank and became this piece
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub promotion: Option<PieceType>,

    pub created_at: DateTime,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Game {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,

    // The in-memory / Redis game id (8-char), used by clients to rejoin.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub game_id: Option<String>,

    // None = that side was played anonymously
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub white_user_id: Option<ObjectId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub black_user_id: Option<ObjectId>,

    // Display names captured at game time so history doesn't need a join
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub white_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub black_name: Option<String>,

    pub moves: Vec<Move>,

    pub status: GameStatus,

    pub created_at: DateTime,
    pub updated_at: DateTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameStatus {
    InProgress,
    WhiteWon,
    BlackWon,
    Draw,
    Abandoned,
}

impl Game {
    pub fn indexes() -> Vec<IndexModel> {
        vec![
            build_index("white_user_id", "idx_white_user_id"),
            build_index("black_user_id", "idx_black_user_id"),
            build_index("status", "idx_game_status"),
        ]
    }
}
