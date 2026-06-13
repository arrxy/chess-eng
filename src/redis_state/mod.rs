pub mod hydrate;
pub mod lock;
pub mod pool;
pub mod pubsub;
pub mod shards;
pub mod stream;

use crate::db::game_schema::{GameStatus, Move as MoveRecord};
use crate::pieces::pieces::{Color, PieceType};
use bb8_redis::redis::AsyncCommands;
use pool::RedisPool;
use serde::{Deserialize, Serialize};

pub const TTL_WAITING: u64 = 600;
pub const TTL_DISCONNECTED: u64 = 1_800;
pub const TTL_PERSISTED: u64 = 300;
/// Rejoin window: a game with no activity for this long is abandoned.
/// Also the live-game TTL, refreshed on every move.
pub const TTL_INACTIVITY: u64 = 3_600;
/// Same value as a Duration of seconds, for the Mongo sweeper cutoff.
pub const INACTIVITY_SECS: i64 = 3_600;

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
    // Bump the version so any in-flight optimistic move sees a conflict and
    // retries against this newer state.
    let _: i64 = conn.hincr(&key, "version", 1).await?;
    let _: bool = conn.expire(&key, ttl_secs as i64).await?;
    Ok(())
}

/// Read both the state and its version in a single round-trip. Used by the
/// optimistic move path so it can compare-and-set.
pub async fn load_versioned(
    pool: &RedisPool,
    game_id: &str,
) -> anyhow::Result<Option<(RedisGameState, u64)>> {
    let key = game_key(game_id);
    let mut conn = pool.get().await?;
    let (state_json, ver): (Option<String>, Option<String>) =
        bb8_redis::redis::cmd("HMGET")
            .arg(&key)
            .arg("state")
            .arg("version")
            .query_async(&mut *conn)
            .await?;
    match state_json {
        None => Ok(None),
        Some(s) => {
            let state = serde_json::from_str(&s)?;
            let version = ver.and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
            Ok(Some((state, version)))
        }
    }
}

pub enum CasResult {
    Updated,
    Conflict,
    Missing,
}

/// Atomically write the state only if the stored version still matches
/// `expected_version`. Replaces the per-move distributed lock: one Lua call
/// does the version check + write + TTL refresh, so a move costs HMGET + EVAL
/// instead of SET-NX + HGET + HSET + EXPIRE + EVAL-release.
pub async fn cas_save(
    pool: &RedisPool,
    game_id: &str,
    state: &RedisGameState,
    expected_version: u64,
    ttl_secs: u64,
) -> anyhow::Result<CasResult> {
    const LUA: &str = "local v = redis.call('HGET', KEYS[1], 'version') \
        if v == false then return -1 end \
        if v ~= ARGV[1] then return 0 end \
        redis.call('HSET', KEYS[1], 'state', ARGV[2]) \
        redis.call('HINCRBY', KEYS[1], 'version', 1) \
        redis.call('PEXPIRE', KEYS[1], ARGV[3]) \
        return 1";
    let json = serde_json::to_string(state)?;
    let key = game_key(game_id);
    let mut conn = pool.get().await?;
    let r: i64 = bb8_redis::redis::cmd("EVAL")
        .arg(LUA)
        .arg(1)
        .arg(&key)
        .arg(expected_version.to_string())
        .arg(json)
        .arg((ttl_secs * 1000) as i64)
        .query_async(&mut *conn)
        .await?;
    Ok(match r {
        1 => CasResult::Updated,
        0 => CasResult::Conflict,
        _ => CasResult::Missing,
    })
}

/// Remove a game's state entirely (used when the inactivity sweeper abandons it).
pub async fn delete_state(pool: &RedisPool, game_id: &str) -> anyhow::Result<()> {
    let mut conn = pool.get().await?;
    let _: usize = conn.del(game_key(game_id)).await?;
    Ok(())
}
