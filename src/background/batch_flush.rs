use std::collections::HashMap;
use std::sync::Arc;
use mongodb::bson::oid::ObjectId;
use std::str::FromStr;

use crate::redis_state::{self, shards::Shards, stream};
use crate::repository::game_repository::GameRepository;

pub async fn run(shards: Arc<Shards>, server_id: String, repo: GameRepository) {
    let coord = shards.coord();
    loop {
        let entries = match stream::xreadgroup(
            coord,
            &server_id,
            stream::BATCH_SIZE,
            stream::FLUSH_INTERVAL_MS,
        )
        .await
        {
            Ok(e) => e,
            Err(e) => {
                eprintln!("batch_flush: xreadgroup error: {e}");
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                continue;
            }
        };

        if entries.is_empty() {
            continue;
        }

        // Group moves by (mongo_id, game_id).
        let mut batches: HashMap<(String, String), Vec<crate::db::game_schema::Move>> =
            HashMap::new();
        let mut ack_ids: Vec<String> = Vec::with_capacity(entries.len());

        for entry in entries {
            ack_ids.push(entry.id);
            batches
                .entry((entry.mongo_id, entry.game_id))
                .or_default()
                .push(entry.move_record);
        }

        // BulkWrite all moves.
        let bulk: Vec<(ObjectId, Vec<crate::db::game_schema::Move>)> = batches
            .iter()
            .filter_map(|((mongo_id, _), moves)| {
                ObjectId::from_str(mongo_id).ok().map(|id| (id, moves.clone()))
            })
            .collect();

        if !bulk.is_empty() {
            if let Err(e) = repo.bulk_push_moves(bulk).await {
                eprintln!("batch_flush: bulk_push_moves error: {e}");
                // Do not ACK — entries stay pending and will be retried.
                continue;
            }
        }

        // ACK then delete the flushed entries so the stream doesn't grow forever.
        if let Err(e) = stream::xack(coord, &ack_ids).await {
            eprintln!("batch_flush: xack error: {e}");
        }
        if let Err(e) = stream::xdel(coord, &ack_ids).await {
            eprintln!("batch_flush: xdel error: {e}");
        }

        // Check if any flushed game has final_status set — finalize those.
        for ((mongo_id_str, game_id), _) in &batches {
            let mongo_id = match ObjectId::from_str(mongo_id_str) {
                Ok(id) => id,
                Err(_) => continue,
            };
            let gpool = shards.pool(game_id);
            if let Ok(Some(mut rs)) = redis_state::load_state(gpool, game_id).await {
                if let Some(status) = rs.final_status {
                    if let Err(e) = repo.finalize_game(mongo_id, status).await {
                        eprintln!("batch_flush: finalize_game error: {e}");
                    } else {
                        rs.final_status = None;
                        let _ = redis_state::save_state(
                            gpool,
                            game_id,
                            &rs,
                            redis_state::TTL_PERSISTED,
                        )
                        .await;
                    }
                }
            }
        }
    }
}
