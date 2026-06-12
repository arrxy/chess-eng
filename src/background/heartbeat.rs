use std::collections::HashMap;
use std::str::FromStr;
use mongodb::bson::oid::ObjectId;

use bb8_redis::redis::{AsyncCommands, cmd};

use crate::redis_state::{self, pool::RedisPool, stream};
use crate::repository::game_repository::GameRepository;

const HEARTBEAT_INTERVAL_SECS: u64 = 5;
const HEARTBEAT_TTL_SECS: u64 = 15;
const PEER_CHECK_EVERY_N_TICKS: u32 = 2; // check peers every 10s (2 × 5s)
const CLAIM_MIN_IDLE_MS: u64 = 15_000;

pub async fn run(pool: RedisPool, server_id: String, repo: GameRepository) {
    let mut tick: u32 = 0;
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(HEARTBEAT_INTERVAL_SECS)).await;
        tick += 1;

        // Refresh our own heartbeat.
        if let Ok(mut conn) = pool.get().await {
            let _: Result<(), _> = cmd("SETEX")
                .arg(format!("server:{}:heartbeat", server_id))
                .arg(HEARTBEAT_TTL_SECS)
                .arg("1")
                .query_async(&mut *conn)
                .await;
            let _: Result<i64, _> = conn.sadd("known_servers", &server_id).await;
        }

        if tick % PEER_CHECK_EVERY_N_TICKS != 0 {
            continue;
        }

        // Scan known peers.
        let peers: Vec<String> = {
            let Ok(mut conn) = pool.get().await else { continue };
            conn.smembers::<_, Vec<String>>("known_servers")
                .await
                .unwrap_or_default()
        };

        for peer_id in peers {
            if peer_id == server_id {
                continue;
            }

            let alive: bool = {
                let Ok(mut conn) = pool.get().await else { continue };
                let v: Option<String> = conn
                    .get(format!("server:{peer_id}:heartbeat"))
                    .await
                    .unwrap_or(None);
                v.is_some()
            };

            if alive {
                continue;
            }

            eprintln!("heartbeat: detected dead peer {peer_id} — claiming pending entries");

            // Claim all entries that were pending for this dead consumer.
            let entries =
                match stream::xautoclaim(&pool, &server_id, &peer_id, CLAIM_MIN_IDLE_MS).await {
                    Ok(e) => e,
                    Err(e) => {
                        eprintln!("heartbeat: xautoclaim error for {peer_id}: {e}");
                        continue;
                    }
                };

            if !entries.is_empty() {
                let mut batches: HashMap<(String, String), Vec<crate::db::game_schema::Move>> =
                    HashMap::new();
                let mut ack_ids = Vec::new();

                for entry in entries {
                    ack_ids.push(entry.id);
                    batches
                        .entry((entry.mongo_id, entry.game_id))
                        .or_default()
                        .push(entry.move_record);
                }

                let bulk: Vec<(ObjectId, Vec<_>)> = batches
                    .iter()
                    .filter_map(|((mid, _), moves)| {
                        ObjectId::from_str(mid).ok().map(|id| (id, moves.clone()))
                    })
                    .collect();

                if !bulk.is_empty() {
                    if let Err(e) = repo.bulk_push_moves(bulk).await {
                        eprintln!("heartbeat: emergency bulk_push_moves error: {e}");
                        continue;
                    }
                }
                let _ = stream::xack(&pool, &ack_ids).await;

                // Finalize or mark games as disconnected.
                for ((mongo_id_str, game_id), _) in &batches {
                    let mongo_id = match ObjectId::from_str(mongo_id_str) {
                        Ok(id) => id,
                        Err(_) => continue,
                    };
                    recover_game(&pool, &repo, game_id, mongo_id, &server_id).await;
                }
            }

            // Recover games that were on the dead server (may not have pending stream entries
            // if the server died before sending any moves through the queue).
            let game_ids: Vec<String> = {
                let Ok(mut conn) = pool.get().await else { continue };
                conn.smembers::<_, Vec<String>>(format!("server:{peer_id}:games"))
                    .await
                    .unwrap_or_default()
            };

            for game_id in &game_ids {
                if let Ok(Some(rs)) = redis_state::load_state(&pool, game_id).await {
                    let mongo_id = match rs
                        .mongo_game_id
                        .as_deref()
                        .and_then(|h| ObjectId::from_str(h).ok())
                    {
                        Some(id) => id,
                        None => continue,
                    };
                    recover_game(&pool, &repo, game_id, mongo_id, &server_id).await;
                }
            }

            // Clean up dead peer's tracking keys.
            if let Ok(mut conn) = pool.get().await {
                let _: Result<i64, _> = conn.srem("known_servers", &peer_id).await;
                let _: Result<i64, _> = conn.del(format!("server:{peer_id}:games")).await;
            }
        }
    }
}

async fn recover_game(
    pool: &RedisPool,
    repo: &GameRepository,
    game_id: &str,
    mongo_id: ObjectId,
    server_id: &str,
) {
    let rs = match redis_state::load_state(pool, game_id).await {
        Ok(Some(s)) => s,
        _ => return,
    };

    if let Some(status) = rs.final_status {
        // Game ended but wasn't finalized — do it now.
        if let Err(e) = repo.finalize_game(mongo_id, status).await {
            eprintln!("heartbeat: finalize_game error for {game_id}: {e}");
            return;
        }
        let mut rs = rs;
        rs.final_status = None;
        let _ = redis_state::save_state(pool, game_id, &rs, redis_state::TTL_PERSISTED).await;
    } else if rs.started {
        // Game was in progress — mark as disconnected and notify the surviving player.
        let _ = redis_state::save_state(pool, game_id, &rs, redis_state::TTL_DISCONNECTED).await;
        publish_disconnect(pool, game_id, server_id).await;
    }
}

async fn publish_disconnect(pool: &RedisPool, game_id: &str, _server_id: &str) {
    let msg = serde_json::json!({"type": "opponent_disconnected"}).to_string();
    let channel = redis_state::pubsub_channel(game_id);
    if let Ok(mut conn) = pool.get().await {
        let _: Result<i64, _> = conn.publish(channel, msg).await;
    }
}
