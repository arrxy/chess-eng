pub mod hydrate;
pub mod lock;
pub mod pool;
pub mod pubsub;
pub mod stream;

use crate::db::game_schema::{GameStatus, Move as MoveRecord};
use crate::pieces::pieces::{Color, PieceType};
use bb8_redis::redis::AsyncCommands;
use pool::RedisPool;
use serde::{Deserialize, Serialize};

pub const TTL_WAITING: u64 = 600;
pub const TTL_ACTIVE: u64 = 7_200;
pub const TTL_DISCONNECTED: u64 = 1_800;
pub const TTL_PERSISTED: u64 = 300;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedPiece {
    pub piece_type: PieceType,
    pub color: Color,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisGameState {
    /// 8x8 row-major board, None = empty square.
    pub board: Vec<Vec<Option<SerializedPiece>>>,
    pub turn: Color,
    pub castling_white_kingside: bool,
    pub castling_white_queenside: bool,
    pub castling_black_kingside: bool,
    pub castling_black_queenside: bool,
    /// None if no en-passant target this turn.
    pub en_passant_target: Option<(u8, u8)>,
    pub moves: Vec<MoveRecord>,
    pub captured_by_white: Vec<PieceType>,
    pub captured_by_black: Vec<PieceType>,
    pub white_user_id: Option<String>,
    pub white_user_name: Option<String>,
    pub black_user_id: Option<String>,
    pub black_user_name: Option<String>,
    pub started: bool,
    pub persisted: bool,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    /// MongoDB ObjectId hex — set when both players have joined.
    pub mongo_game_id: Option<String>,
    /// Set when the game ends but before the Mongo doc is finalized.
    /// Lets the peer monitor finalize the game if this server dies first.
    pub final_status: Option<GameStatus>,
}

fn game_key(game_id: &str) -> String {
    format!("game:{game_id}")
}

pub fn pubsub_channel(game_id: &str) -> String {
    format!("game:{game_id}:events")
}

pub fn lock_key(game_id: &str) -> String {
    format!("game:{game_id}:lock")
}

pub async fn load_state(
    pool: &RedisPool,
    game_id: &str,
) -> anyhow::Result<Option<RedisGameState>> {
    let mut conn = pool.get().await?;
    let raw: Option<String> = conn.hget(game_key(game_id), "state").await?;
    match raw {
        None => Ok(None),
        Some(s) => Ok(Some(serde_json::from_str(&s)?)),
    }
}

pub async fn save_state(
    pool: &RedisPool,
    game_id: &str,
    state: &RedisGameState,
    ttl_secs: u64,
) -> anyhow::Result<()> {
    let json = serde_json::to_string(state)?;
    let key = game_key(game_id);
    let mut conn = pool.get().await?;
    let _: usize = conn.hset(&key, "state", json).await?;
    let _: bool = conn.expire(&key, ttl_secs as i64).await?;
    Ok(())
}
