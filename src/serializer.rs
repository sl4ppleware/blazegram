//! Chat Serializer — per-chat lock guaranteeing sequential update processing.

use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::state::StateStore;
use crate::types::*;

pub struct ChatSerializer {
    locks: DashMap<ChatId, Arc<Mutex<()>>>,
    pub(crate) store: Arc<dyn StateStore>,
}

impl ChatSerializer {
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

        let mut state = self
            .store
            .load(chat_id)
            .await
            .unwrap_or_else(|| ChatState::new(chat_id, user.clone()));

        // Update user info on every interaction
        state.user = user.clone();

        let state = f(state).await;
        self.store.save(&state).await;
    }

    /// GC: remove mutexes for idle chats.
    /// Only evicts if strong_count == 1 (no clones exist outside the DashMap).
    pub fn gc(&self) {
        self.locks.retain(|_, mutex| Arc::strong_count(mutex) > 1);
    }
}
