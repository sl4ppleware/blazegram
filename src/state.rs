//! State storage trait and in-memory implementation with versioned snapshots.

use std::sync::Arc;

use async_trait::async_trait;
use dashmap::DashMap;

use crate::types::*;

/// Snapshot format magic bytes + version for forward-compatible persistence.
const SNAPSHOT_MAGIC: &[u8; 4] = b"BG\x01\x00"; // "BG" + version 1.0

/// Backend for persisting per-chat [`ChatState`].
///
/// Built-in implementations: [`InMemoryStore`], [`RedbStore`](crate::redb_store::RedbStore),
/// and (with the `redis` feature) `RedisStore`.
#[async_trait]
pub trait StateStore: Send + Sync + 'static {
    /// Load state for a chat, or `None` if unseen.
    async fn load(&self, chat_id: ChatId) -> Option<ChatState>;
    /// Persist the current state.
    async fn save(&self, state: &ChatState);
    /// Delete all state for a chat.
    async fn delete(&self, chat_id: ChatId);
    /// Return all chat IDs that have stored state (used by broadcast).
    async fn all_chat_ids(&self) -> Vec<ChatId>;
}

// ─── In-Memory Store ───

/// In-memory state store backed by a [`DashMap`].
///
/// State is lost on restart unless combined with `.snapshot()`.
pub struct InMemoryStore {
    states: DashMap<ChatId, ChatState>,
}

impl InMemoryStore {
    /// Open or create a redb database at the given path.
    pub fn new() -> Self {
        Self {
            states: DashMap::new(),
        }
    }

    /// Len.
    pub fn len(&self) -> usize {
        self.states.len()
    }

    /// Is empty.
    pub fn is_empty(&self) -> bool {
        self.states.is_empty()
    }
}

impl Default for InMemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StateStore for InMemoryStore {
    async fn load(&self, chat_id: ChatId) -> Option<ChatState> {
        self.states.get(&chat_id).map(|r| r.value().clone())
    }

    async fn save(&self, state: &ChatState) {
        self.states.insert(state.chat_id, state.clone());
    }

    async fn delete(&self, chat_id: ChatId) {
        self.states.remove(&chat_id);
    }

    async fn all_chat_ids(&self) -> Vec<ChatId> {
        self.states.iter().map(|r| *r.key()).collect()
    }
}

// ─── Versioned Snapshot / Restore ───

impl InMemoryStore {
    /// Snapshot all state to a file using postcard with a version header.
    ///
    /// Format: `[MAGIC 4B][postcard-encoded Vec<ChatState>]`
    ///
    /// Uses atomic write (tmp + rename) to prevent corruption on crash.
    pub async fn snapshot(
        &self,
        path: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let states: Vec<ChatState> = self.states.iter().map(|r| r.value().clone()).collect();
        let payload = postcard::to_allocvec(&states)?;

        let mut buf = Vec::with_capacity(SNAPSHOT_MAGIC.len() + payload.len());
        buf.extend_from_slice(SNAPSHOT_MAGIC);
        buf.extend_from_slice(&payload);

        let tmp = format!("{path}.tmp");
        tokio::fs::write(&tmp, buf).await?;
        tokio::fs::rename(&tmp, path).await?;
        Ok(())
    }

    /// Restore state from a versioned snapshot file.
    ///
    /// Returns the number of restored chat states, or 0 if no file found.
    pub async fn restore(
        &self,
        path: &str,
    ) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        let bytes = match tokio::fs::read(path).await {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
            Err(e) => return Err(e.into()),
        };

        let payload = if bytes.starts_with(SNAPSHOT_MAGIC) {
            // Versioned format
            &bytes[SNAPSHOT_MAGIC.len()..]
        } else {
            // Legacy bincode format — try to deserialize directly for migration
            tracing::warn!("snapshot has no version header — attempting legacy bincode migration");
            return self.restore_legacy_bincode(&bytes);
        };

        let states: Vec<ChatState> = postcard::from_bytes(payload)?;
        let count = states.len();
        for state in states {
            self.states.insert(state.chat_id, state);
        }
        Ok(count)
    }

    /// Attempt to restore from old bincode v1 format (migration path).
    fn restore_legacy_bincode(
        &self,
        bytes: &[u8],
    ) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        // Try serde_json as final fallback
        match serde_json::from_slice::<Vec<ChatState>>(bytes) {
            Ok(states) => {
                let count = states.len();
                for state in states {
                    self.states.insert(state.chat_id, state);
                }
                tracing::info!(count, "migrated from legacy JSON snapshot");
                Ok(count)
            }
            Err(_) => Err("unrecognized snapshot format — delete the file and restart".into()),
        }
    }

    /// Start a background task that snapshots every `interval`.
    pub fn start_snapshot_task(
        self: &Arc<Self>,
        path: String,
        interval: std::time::Duration,
    ) -> tokio::task::JoinHandle<()> {
        let store = Arc::clone(self);
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(interval);
            loop {
                tick.tick().await;
                if let Err(e) = store.snapshot(&path).await {
                    tracing::error!(error = %e, "snapshot failed");
                }
            }
        })
    }
}
