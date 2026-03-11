use std::sync::Arc;

use async_trait::async_trait;
use dashmap::DashMap;
use crate::types::*;

#[async_trait]
pub trait StateStore: Send + Sync + 'static {
    async fn load(&self, chat_id: ChatId) -> Option<ChatState>;
    async fn save(&self, state: &ChatState);
    async fn delete(&self, chat_id: ChatId);
    async fn all_chat_ids(&self) -> Vec<ChatId>;
}

// ─── In-Memory Store ───

pub struct InMemoryStore {
    states: DashMap<ChatId, ChatState>,
}

impl InMemoryStore {
    pub fn new() -> Self {
        Self {
            states: DashMap::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.states.len()
    }

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

// ─── Snapshot / Restore ───

impl InMemoryStore {
    /// Snapshot all state to a file using bincode.
    pub async fn snapshot(&self, path: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let states: Vec<ChatState> = self.states.iter().map(|r| r.value().clone()).collect();
        let bytes = bincode::serialize(&states)?;
        // Atomic write: tmp file + rename to prevent corruption on crash
        let tmp = format!("{}.tmp", path);
        tokio::fs::write(&tmp, bytes).await?;
        tokio::fs::rename(&tmp, path).await?;
        Ok(())
    }

    /// Restore state from a snapshot file.
    pub async fn restore(&self, path: &str) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        let bytes = match tokio::fs::read(path).await {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
            Err(e) => return Err(e.into()),
        };
        let states: Vec<ChatState> = bincode::deserialize(&bytes)?;
        let count = states.len();
        for state in states {
            self.states.insert(state.chat_id, state);
        }
        Ok(count)
    }

    /// Start a background task that snapshots every `interval`.
    pub fn start_snapshot_task(self: &Arc<Self>, path: String, interval: std::time::Duration) -> tokio::task::JoinHandle<()> {
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
