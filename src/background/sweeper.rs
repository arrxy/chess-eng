use mongodb::bson::DateTime;

use crate::db::game_schema::GameStatus;
use crate::redis_state::{self, INACTIVITY_SECS, pool::RedisPool};
use crate::repository::game_repository::GameRepository;

const SWEEP_INTERVAL_SECS: u64 = 300; // every 5 minutes

/// Periodically finalizes in-progress games that have seen no activity for
/// `INACTIVITY_SECS` as Abandoned, and clears their Redis state so they can't
/// be rejoined.
pub async fn run(pool: RedisPool, repo: GameRepository) {
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(SWEEP_INTERVAL_SECS)).await;

        let cutoff = DateTime::from_millis(
            DateTime::now().timestamp_millis() - INACTIVITY_SECS * 1000,
        );

        let stale = match repo.find_stale_in_progress(cutoff).await {
            Ok(games) => games,
            Err(e) => {
                eprintln!("sweeper: find_stale_in_progress error: {e}");
                continue;
            }
        };

        for game in stale {
            let Some(id) = game.id else { continue };
            if let Err(e) = repo.finalize_game(id, GameStatus::Abandoned).await {
                eprintln!("sweeper: finalize_game error: {e}");
                continue;
            }
            if let Some(gid) = &game.game_id {
                let _ = redis_state::delete_state(&pool, gid).await;
            }
        }
    }
}
