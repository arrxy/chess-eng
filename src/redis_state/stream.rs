use bb8_redis::redis::{cmd, Value as RedisValue};
use serde_json;

use crate::db::game_schema::Move as MoveRecord;
use super::pool::RedisPool;

pub const STREAM_KEY: &str = "moves_stream";
pub const CONSUMER_GROUP: &str = "workers";
pub const BATCH_SIZE: usize = 500;
pub const FLUSH_INTERVAL_MS: u64 = 500;

#[derive(Debug)]
pub struct StreamEntry {
    pub id: String,
    pub game_id: String,
    pub mongo_id: String,
    pub move_record: MoveRecord,
}

/// Ensure the consumer group exists (idempotent; call once at startup).
pub async fn ensure_consumer_group(pool: &RedisPool) -> anyhow::Result<()> {
    let mut conn = pool.get().await?;
    let result: Result<(), _> = cmd("XGROUP")
        .arg("CREATE")
        .arg(STREAM_KEY)
        .arg(CONSUMER_GROUP)
        .arg("$")
        .arg("MKSTREAM")
        .query_async(&mut *conn)
        .await;
    match result {
        Ok(_) => Ok(()),
        Err(e) if e.to_string().contains("BUSYGROUP") => Ok(()),
        Err(e) => Err(e.into()),
    }
}

pub async fn xadd_move(
    pool: &RedisPool,
    game_id: &str,
    mongo_id: &str,
    move_record: &MoveRecord,
) -> anyhow::Result<()> {
    let move_json = serde_json::to_string(move_record)?;
    let mut conn = pool.get().await?;
    let _: String = cmd("XADD")
        .arg(STREAM_KEY)
        .arg("*")
        .arg("game_id")
        .arg(game_id)
        .arg("mongo_id")
        .arg(mongo_id)
        .arg("move_json")
        .arg(move_json)
        .query_async(&mut *conn)
        .await?;
    Ok(())
}

pub async fn xreadgroup(
    pool: &RedisPool,
    server_id: &str,
    count: usize,
    block_ms: u64,
) -> anyhow::Result<Vec<StreamEntry>> {
    let mut conn = pool.get().await?;
    let raw: RedisValue = cmd("XREADGROUP")
        .arg("GROUP")
        .arg(CONSUMER_GROUP)
        .arg(server_id)
        .arg("COUNT")
        .arg(count)
        .arg("BLOCK")
        .arg(block_ms)
        .arg("STREAMS")
        .arg(STREAM_KEY)
        .arg(">")
        .query_async(&mut *conn)
        .await
        .unwrap_or(RedisValue::Nil);
    parse_xreadgroup_response(raw)
}

pub async fn xack(pool: &RedisPool, ids: &[String]) -> anyhow::Result<()> {
    if ids.is_empty() {
        return Ok(());
    }
    let mut conn = pool.get().await?;
    let mut c = cmd("XACK");
    c.arg(STREAM_KEY).arg(CONSUMER_GROUP);
    for id in ids {
        c.arg(id);
    }
    let _: i64 = c.query_async(&mut *conn).await?;
    Ok(())
}

/// Claim all entries idle for at least `min_idle_ms` (from any consumer).
pub async fn xautoclaim(
    pool: &RedisPool,
    claimant_id: &str,
    _dead_consumer_id: &str,
    min_idle_ms: u64,
) -> anyhow::Result<Vec<StreamEntry>> {
    let mut conn = pool.get().await?;
    let raw: RedisValue = cmd("XAUTOCLAIM")
        .arg(STREAM_KEY)
        .arg(CONSUMER_GROUP)
        .arg(claimant_id)
        .arg(min_idle_ms)
        .arg("0-0")
        .query_async(&mut *conn)
        .await
        .unwrap_or(RedisValue::Nil);
    parse_xautoclaim_response(raw)
}

// ---------------------------------------------------------------------------
// Parsing helpers — walk the raw redis::Value tree
// ---------------------------------------------------------------------------

fn redis_str(v: &RedisValue) -> Option<String> {
    match v {
        RedisValue::BulkString(b) => String::from_utf8(b.clone()).ok(),
        RedisValue::SimpleString(s) => Some(s.clone()),
        _ => None,
    }
}

fn parse_fields(fields: &[RedisValue]) -> Option<StreamEntry> {
    // fields is a flat [key, val, key, val, ...] list
    let id = None::<String>; // id comes from the enclosing array
    let _ = id;
    let mut game_id = None;
    let mut mongo_id = None;
    let mut move_json_str = None;
    let mut i = 0;
    while i + 1 < fields.len() {
        let key = redis_str(&fields[i])?;
        let val = redis_str(&fields[i + 1]);
        match key.as_str() {
            "game_id"   => game_id    = val,
            "mongo_id"  => mongo_id   = val,
            "move_json" => move_json_str = val,
            _ => {}
        }
        i += 2;
    }
    let move_record: MoveRecord = serde_json::from_str(&move_json_str?).ok()?;
    Some(StreamEntry {
        id: String::new(), // filled in by caller
        game_id: game_id?,
        mongo_id: mongo_id?,
        move_record,
    })
}

fn parse_entry_pair(pair: &RedisValue) -> Option<StreamEntry> {
    // pair = [id, [field, val, ...]]
    let arr = match pair {
        RedisValue::Array(a) => a,
        _ => return None,
    };
    if arr.len() < 2 { return None; }
    let id = redis_str(&arr[0])?;
    let fields = match &arr[1] {
        RedisValue::Array(f) => f,
        _ => return None,
    };
    let mut entry = parse_fields(fields)?;
    entry.id = id;
    Some(entry)
}

fn parse_xreadgroup_response(raw: RedisValue) -> anyhow::Result<Vec<StreamEntry>> {
    // XREADGROUP returns: [[stream_name, [[id, fields], ...]]]
    let mut result = Vec::new();
    let streams = match raw {
        RedisValue::Array(a) => a,
        _ => return Ok(result),
    };
    for stream in streams {
        let stream_arr = match stream {
            RedisValue::Array(a) => a,
            _ => continue,
        };
        if stream_arr.len() < 2 { continue; }
        let entries = match &stream_arr[1] {
            RedisValue::Array(a) => a,
            _ => continue,
        };
        for entry in entries {
            if let Some(e) = parse_entry_pair(entry) {
                result.push(e);
            }
        }
    }
    Ok(result)
}

fn parse_xautoclaim_response(raw: RedisValue) -> anyhow::Result<Vec<StreamEntry>> {
    // XAUTOCLAIM returns: [next-id, [[id, fields], ...], [deleted-ids]]
    let mut result = Vec::new();
    let outer = match raw {
        RedisValue::Array(a) => a,
        _ => return Ok(result),
    };
    if outer.len() < 2 { return Ok(result); }
    let entries = match &outer[1] {
        RedisValue::Array(a) => a,
        _ => return Ok(result),
    };
    for entry in entries {
        if let Some(e) = parse_entry_pair(entry) {
            result.push(e);
        }
    }
    Ok(result)
}
