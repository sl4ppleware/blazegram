//! Redis-backed state store.
//!
//! Enable with `features = ["redis"]` in Cargo.toml.
//!
//! Uses `deadpool-redis` for connection pooling and `serde_json` for serialization.
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
/// Each chat's state is stored as a JSON blob with a configurable TTL.
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
    async fn load(&self, chat_id: ChatId) -> Result<Option<ChatState>, String> {
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| format!("redis pool: {e}"))?;
        let bytes: Option<Vec<u8>> = conn
            .get(self.key(chat_id))
            .await
            .map_err(|e| format!("redis get: {e}"))?;
        match bytes {
            Some(b) => match serde_json::from_slice(&b) {
                Ok(state) => Ok(Some(state)),
                Err(e) => {
                    tracing::warn!(chat_id = chat_id.0, error = %e, "corrupt state in redis — treating as fresh");
                    Ok(None)
                }
            },
            None => Ok(None),
        }
    }

    async fn save(&self, state: &ChatState) -> Result<(), String> {
        let bytes = serde_json::to_vec(state).map_err(|e| format!("serialize: {e}"))?;
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| format!("redis pool: {e}"))?;
        conn.set_ex::<_, _, ()>(self.key(state.chat_id), bytes, self.ttl.as_secs())
            .await
            .map_err(|e| format!("redis set_ex: {e}"))?;
        Ok(())
    }

    async fn delete(&self, chat_id: ChatId) -> Result<(), String> {
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| format!("redis pool: {e}"))?;
        conn.del::<_, ()>(self.key(chat_id))
            .await
            .map_err(|e| format!("redis del: {e}"))?;
        Ok(())
    }

    async fn all_chat_ids(&self) -> Result<Vec<ChatId>, String> {
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| format!("redis pool: {e}"))?;
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
                .map_err(|e| format!("redis scan: {e}"))?;
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
        Ok(chat_ids)
    }
}
