//! Redis-backed state store.
//!
//! Enable with `features = ["redis"]` in Cargo.toml.
//!
//! Uses `deadpool-redis` for connection pooling and `bincode` for serialization.
//!
//! ```toml
//! [dependencies]
//! blazegram = { path = ".", features = ["redis"] }
//! ```

use std::time::Duration;

use async_trait::async_trait;
use deadpool_redis::{Config, Pool, Runtime};
use redis::AsyncCommands;

use crate::state::StateStore;
use crate::types::*;

/// Redis-backed [`StateStore`].
///
/// Each chat's state is stored as a bincode blob with a configurable TTL.
pub struct RedisStore {
    pool: Pool,
    prefix: String,
    ttl: Duration,
}

impl RedisStore {
    /// Connect to Redis.  `url` is e.g. `"redis://127.0.0.1/"`.
    pub fn new(url: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let cfg = Config::from_url(url);
        let pool = cfg.create_pool(Some(Runtime::Tokio1))?;
        Ok(Self {
            pool,
            prefix: "bg:state".to_string(),
            ttl: Duration::from_secs(86400 * 7), // 7 days
        })
    }

    /// Override the key prefix (default: `bg:state`).
    pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = prefix.into();
        self
    }

    /// Override the TTL (default: 7 days).
    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.ttl = ttl;
        self
    }

    fn key(&self, chat_id: ChatId) -> String {
        format!("{}:{}", self.prefix, chat_id.0)
    }
}

#[async_trait]
impl StateStore for RedisStore {
    async fn load(&self, chat_id: ChatId) -> Option<ChatState> {
        let mut conn = self.pool.get().await.ok()?;
        let bytes: Option<Vec<u8>> = conn.get(self.key(chat_id)).await.ok()?;
        bytes.and_then(|b| bincode::deserialize(&b).ok())
    }

    async fn save(&self, state: &ChatState) {
        if let Ok(mut conn) = self.pool.get().await {
            if let Ok(bytes) = bincode::serialize(state) {
                let _: Result<(), _> = conn
                    .set_ex(self.key(state.chat_id), bytes, self.ttl.as_secs())
                    .await;
            }
        }
    }

    async fn delete(&self, chat_id: ChatId) {
        if let Ok(mut conn) = self.pool.get().await {
            let _: Result<(), _> = conn.del(self.key(chat_id)).await;
        }
    }

    async fn all_chat_ids(&self) -> Vec<ChatId> {
        if let Ok(mut conn) = self.pool.get().await {
            // Use SCAN instead of KEYS to avoid blocking Redis on large datasets
            let pattern = format!("{}:*", self.prefix);
            let mut chat_ids = Vec::new();
            let mut cursor: u64 = 0;
            loop {
                let (next_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
                    .arg(cursor)
                    .arg("MATCH")
                    .arg(&pattern)
                    .arg("COUNT")
                    .arg(100)
                    .query_async(&mut *conn)
                    .await
                    .unwrap_or((0, Vec::new()));
                for key in &keys {
                    if let Some(id_str) = key.rsplit(':').next() {
                        if let Ok(id) = id_str.parse::<i64>() {
                            chat_ids.push(ChatId(id));
                        }
                    }
                }
                cursor = next_cursor;
                if cursor == 0 {
                    break;
                }
            }
            chat_ids
        } else {
            Vec::new()
        }
    }
}
