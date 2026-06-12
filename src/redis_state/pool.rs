use bb8_redis::bb8::Pool;
use bb8_redis::RedisConnectionManager;

pub type RedisPool = Pool<RedisConnectionManager>;

pub async fn init_pool(redis_url: &str) -> anyhow::Result<RedisPool> {
    let manager = RedisConnectionManager::new(redis_url)?;
    let pool = Pool::builder().max_size(16).build(manager).await?;
    Ok(pool)
}
