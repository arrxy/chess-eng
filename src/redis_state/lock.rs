use super::{lock_key, pool::RedisPool};
use bb8_redis::redis::cmd;
use uuid::Uuid;

pub struct RedisLock {
    pool: RedisPool,
    key: String,
    token: String,
}

impl RedisLock {
    async fn try_acquire(
        pool: &RedisPool,
        game_id: &str,
        server_id: &str,
    ) -> anyhow::Result<Option<Self>> {
        let key = lock_key(game_id);
        let token = format!("{server_id}:{}", Uuid::new_v4());
        let mut conn = pool.get().await?;
        let acquired: bool = cmd("SET")
            .arg(&key)
            .arg(&token)
            .arg("NX")
            .arg("PX")
            .arg(3000u64)
            .query_async(&mut *conn)
            .await?;
        if acquired {
            Ok(Some(Self {
                pool: pool.clone(),
                key,
                token,
            }))
        } else {
            Ok(None)
        }
    }

    /// Spin up to ~500 ms with 50/150/300 ms back-off.
    pub async fn acquire(
        pool: &RedisPool,
        game_id: &str,
        server_id: &str,
    ) -> anyhow::Result<Self> {
        let delays = [50u64, 150, 300];
        for delay in delays {
            if let Some(lock) = Self::try_acquire(pool, game_id, server_id).await? {
                return Ok(lock);
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
        }
        Self::try_acquire(pool, game_id, server_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("failed to acquire lock for game {game_id}"))
    }

    /// Atomic release via Lua EVAL: only delete if the token matches.
    pub async fn release(self) {
        let lua = "if redis.call('get', KEYS[1]) == ARGV[1] then \
                       return redis.call('del', KEYS[1]) \
                   else \
                       return 0 \
                   end";
        if let Ok(mut conn) = self.pool.get().await {
            let _: Result<i64, _> = cmd("EVAL")
                .arg(lua)
                .arg(1)
                .arg(&self.key)
                .arg(&self.token)
                .query_async(&mut *conn)
                .await;
        }
    }
}
