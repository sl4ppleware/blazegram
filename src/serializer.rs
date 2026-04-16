//! Chat Serializer — per-chat lock guaranteeing sequential update processing.

use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::state::StateStore;
use crate::types::*;

/// Per-chat mutex + state load/save ensuring sequential update processing.
pub struct ChatSerializer {
    locks: DashMap<ChatId, Arc<Mutex<()>>>,
    pub(crate) store: Arc<dyn StateStore>,
}

impl ChatSerializer {
    /// Create a new serializer backed by the given state store.
    pub fn new(store: Arc<dyn StateStore>) -> Self {
        Self {
            locks: DashMap::new(),
            store,
        }
    }

    /// Execute `f` with exclusive access to a chat's state.
    /// `f` receives owned ChatState and must return the modified ChatState.
    pub async fn serialize<F, Fut>(&self, chat_id: ChatId, user: &UserInfo, f: F)
    where
        F: FnOnce(ChatState) -> Fut,
        Fut: std::future::Future<Output = ChatState>,
    {
        let mutex = self
            .locks
            .entry(chat_id)
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone();

        let _guard = mutex.lock().await;

        let mut state = match self.store.load(chat_id).await {
            Ok(Some(s)) => s,
            Ok(None) => ChatState::new(chat_id, user.clone()),
            Err(e) => {
                tracing::error!(chat_id = chat_id.0, error = %e, "state load failed, using fresh");
                ChatState::new(chat_id, user.clone())
            }
        };

        // Update user info on every interaction
        state.user = user.clone();

        let state = f(state).await;
        if let Err(e) = self.store.save(&state).await {
            tracing::error!(chat_id = chat_id.0, error = %e, "state save failed");
        }
    }

    /// GC: remove mutexes for idle chats.
    /// Only evicts if strong_count == 1 (no clones exist outside the DashMap).
    /// Two-phase: collect keys first, then remove — avoids holding DashMap shard
    /// locks during retain (which would deadlock with concurrent serialize() calls).
    pub fn gc(&self) {
        let stale: Vec<ChatId> = self
            .locks
            .iter()
            .filter(|entry| Arc::strong_count(entry.value()) <= 1)
            .map(|entry| *entry.key())
            .collect();
        for key in stale {
            // Re-check under removal — another task may have cloned between collect and remove
            self.locks
                .remove_if(&key, |_, mutex| Arc::strong_count(mutex) <= 1);
        }
    }
}
