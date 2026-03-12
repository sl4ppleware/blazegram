//! Persistent state store backed by [`redb`](https://docs.rs/redb) — a pure-Rust,
//! single-file, ACID embedded database.
//!
//! Zero C dependencies. No SQLite linking conflicts.
//!
//! ```ignore
//! App::builder("TOKEN")
//!     .redb_store("bot.redb")
//!     .run().await;
//! ```

use async_trait::async_trait;
use redb::{Database, ReadableTable, TableDefinition};
use std::path::Path;
use std::sync::Arc;

use crate::state::StateStore;
use crate::types::*;

/// Table: chat_id (i64) → JSON-serialized ChatState.
/// JSON is used because ChatState contains `serde_json::Value` fields.
const STATE_TABLE: TableDefinition<i64, &[u8]> = TableDefinition::new("chat_state");

/// Pure-Rust persistent state store. Thread-safe, ACID, zero external deps.
pub struct RedbStore {
    db: Arc<Database>,
}

impl RedbStore {
    /// Open or create a database at `path`.
    #[allow(clippy::result_large_err)]
    pub fn open(path: impl AsRef<Path>) -> Result<Self, redb::Error> {
        let db = Database::create(path)?;
        // Ensure table exists.
        let txn = db.begin_write()?;
        txn.open_table(STATE_TABLE)?;
        txn.commit()?;
        Ok(Self { db: Arc::new(db) })
    }

    /// Number of stored chat states.
    pub fn len(&self) -> usize {
        let Ok(txn) = self.db.begin_read() else {
            return 0;
        };
        let Ok(table) = txn.open_table(STATE_TABLE) else {
            return 0;
        };
        table.iter().into_iter().flatten().count()
    }

    /// Is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[async_trait]
impl StateStore for RedbStore {
    async fn load(&self, chat_id: ChatId) -> Option<ChatState> {
        let db = self.db.clone();
        let id = chat_id.0;
        tokio::task::spawn_blocking(move || {
            let txn = db.begin_read().ok()?;
            let table = txn.open_table(STATE_TABLE).ok()?;
            let guard = table.get(id).ok()??;
            match serde_json::from_slice(guard.value()) {
                Ok(state) => Some(state),
                Err(e) => {
                    tracing::warn!(chat_id = id, error = %e, "corrupt state in redb — treating as fresh");
                    None
                }
            }
        })
        .await
        .ok()
        .flatten()
    }

    async fn save(&self, state: &ChatState) {
        let db = self.db.clone();
        let bytes = match serde_json::to_vec(state) {
            Ok(b) => b,
            Err(e) => {
                tracing::error!(error = %e, "failed to serialize state");
                return;
            }
        };
        let chat_id = state.chat_id.0;
        if let Err(e) = tokio::task::spawn_blocking(move || {
            let txn = db.begin_write()?;
            {
                let mut table = txn.open_table(STATE_TABLE)?;
                table.insert(chat_id, bytes.as_slice())?;
            }
            txn.commit()?;
            Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
        })
        .await
        .unwrap_or_else(|e| Err(e.into()))
        {
            tracing::error!(chat_id, error = %e, "failed to save state to redb");
        }
    }

    async fn delete(&self, chat_id: ChatId) {
        let db = self.db.clone();
        let id = chat_id.0;
        if let Err(e) = tokio::task::spawn_blocking(move || {
            let txn = db.begin_write()?;
            {
                let mut table = txn.open_table(STATE_TABLE)?;
                table.remove(id)?;
            }
            txn.commit()?;
            Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
        })
        .await
        .unwrap_or_else(|e| Err(e.into()))
        {
            tracing::error!(chat_id = id, error = %e, "failed to delete state from redb");
        }
    }

    async fn all_chat_ids(&self) -> Vec<ChatId> {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            let txn = match db.begin_read() {
                Ok(t) => t,
                Err(_) => return Vec::new(),
            };
            let table = match txn.open_table(STATE_TABLE) {
                Ok(t) => t,
                Err(_) => return Vec::new(),
            };
            table
                .iter()
                .into_iter()
                .flatten()
                .filter_map(|r| r.ok())
                .map(|(k, _)| ChatId(k.value()))
                .collect()
        })
        .await
        .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_user() -> UserInfo {
        UserInfo {
            id: UserId(1),
            first_name: "Test".into(),
            last_name: None,
            username: None,
            language_code: None,
        }
    }

    #[tokio::test]
    async fn roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.redb");
        let store = RedbStore::open(&path).unwrap();

        let chat_id = ChatId(42);
        assert!(store.load(chat_id).await.is_none());

        let mut state = ChatState::new(chat_id, test_user());
        state.data.insert("key".into(), serde_json::json!("value"));
        store.save(&state).await;

        let loaded = store.load(chat_id).await.unwrap();
        assert_eq!(loaded.chat_id, chat_id);
        assert_eq!(loaded.data["key"], "value");

        store.delete(chat_id).await;
        assert!(store.load(chat_id).await.is_none());
    }

    #[tokio::test]
    async fn all_chat_ids_works() {
        let dir = tempfile::tempdir().unwrap();
        let store = RedbStore::open(dir.path().join("ids.redb")).unwrap();

        for id in [1i64, 2, 3] {
            store.save(&ChatState::new(ChatId(id), test_user())).await;
        }
        let mut ids: Vec<i64> = store.all_chat_ids().await.iter().map(|c| c.0).collect();
        ids.sort();
        assert_eq!(ids, vec![1, 2, 3]);
    }
}
