use super::pool::{RedisPool, init_pool};

/// A fixed set of independent Valkey/Redis nodes. Game state is sharded across
/// them by a deterministic hash of the game id, so every server routes a given
/// game — its state, lock, AND pub/sub channel — to the same node. Global
/// coordination data (the move stream, known_servers, heartbeats) lives on the
/// coordination node (shard 0).
pub struct Shards {
    pools: Vec<RedisPool>,
    urls: Vec<String>,
}

/// FNV-1a — deterministic across processes/restarts (unlike std's randomized
/// hasher), which is required so all servers agree on a game's shard.
fn shard_index(game_id: &str, n: usize) -> usize {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in game_id.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    (h % n as u64) as usize
}

impl Shards {
    pub async fn new(urls: Vec<String>) -> anyhow::Result<Self> {
        anyhow::ensure!(!urls.is_empty(), "at least one redis url is required");
        let mut pools = Vec::with_capacity(urls.len());
        for u in &urls {
            pools.push(init_pool(u).await?);
        }
        Ok(Self { pools, urls })
    }

    pub fn len(&self) -> usize {
        self.pools.len()
    }

    fn idx(&self, game_id: &str) -> usize {
        shard_index(game_id, self.pools.len())
    }

    /// Pool for a game's keys (state, lock, pub/sub).
    pub fn pool(&self, game_id: &str) -> &RedisPool {
        &self.pools[self.idx(game_id)]
    }

    /// URL for a game's shard, used to open the pub/sub connection.
    pub fn url(&self, game_id: &str) -> &str {
        &self.urls[self.idx(game_id)]
    }

    /// Coordination node — move stream, known_servers, heartbeats, server-games.
    pub fn coord(&self) -> &RedisPool {
        &self.pools[0]
    }
}
