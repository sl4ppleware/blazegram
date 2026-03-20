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
use redb::{Database, ReadableTable, ReadableTableMetadata, TableDefinition};
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
        table.len().unwrap_or(0) as usize
    }

    /// Is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[async_trait]
impl StateStore for RedbStore {
    async fn load(&self, chat_id: ChatId) -> Result<Option<ChatState>, String> {
        let db = self.db.clone();
        let id = chat_id.0;
        tokio::task::spawn_blocking(move || {
            let txn = db.begin_read().map_err(|e| format!("redb read txn: {e}"))?;
            let table = txn
                .open_table(STATE_TABLE)
                .map_err(|e| format!("redb open table: {e}"))?;
            match table.get(id).map_err(|e| format!("redb get: {e}"))? {
                Some(guard) => match serde_json::from_slice(guard.value()) {
                    Ok(state) => Ok(Some(state)),
                    Err(e) => {
                        tracing::warn!(chat_id = id, error = %e, "corrupt state in redb — treating as fresh");
                        Ok(None)
                    }
                },
                None => Ok(None),
            }
        })
        .await
        .map_err(|e| format!("redb spawn_blocking: {e}"))?
    }

    async fn save(&self, state: &ChatState) -> Result<(), String> {
        let db = self.db.clone();
        let bytes = serde_json::to_vec(state).map_err(|e| format!("serialize: {e}"))?;
        let chat_id = state.chat_id.0;
        tokio::task::spawn_blocking(move || {
            let txn = db
                .begin_write()
                .map_err(|e| format!("redb write txn: {e}"))?;
            {
                let mut table = txn
                    .open_table(STATE_TABLE)
                    .map_err(|e| format!("redb open table: {e}"))?;
                table
                    .insert(chat_id, bytes.as_slice())
                    .map_err(|e| format!("redb insert: {e}"))?;
            }
            txn.commit().map_err(|e| format!("redb commit: {e}"))?;
            Ok(())
        })
        .await
        .map_err(|e| format!("redb spawn_blocking: {e}"))?
    }

    async fn delete(&self, chat_id: ChatId) -> Result<(), String> {
        let db = self.db.clone();
        let id = chat_id.0;
        tokio::task::spawn_blocking(move || {
            let txn = db
                .begin_write()
                .map_err(|e| format!("redb write txn: {e}"))?;
            {
                let mut table = txn
                    .open_table(STATE_TABLE)
                    .map_err(|e| format!("redb open table: {e}"))?;
                table.remove(id).map_err(|e| format!("redb remove: {e}"))?;
            }
            txn.commit().map_err(|e| format!("redb commit: {e}"))?;
            Ok(())
        })
        .await
        .map_err(|e| format!("redb spawn_blocking: {e}"))?
    }

    async fn all_chat_ids(&self) -> Result<Vec<ChatId>, String> {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            let txn = db.begin_read().map_err(|e| format!("redb read txn: {e}"))?;
            let table = txn
                .open_table(STATE_TABLE)
                .map_err(|e| format!("redb open table: {e}"))?;
            let ids: Vec<ChatId> = table
                .iter()
                .map_err(|e| format!("redb iter: {e}"))?
                .map(|r| {
                    r.map(|(k, _)| ChatId(k.value()))
                        .map_err(|e| format!("redb row: {e}"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(ids)
        })
        .await
        .map_err(|e| format!("redb spawn_blocking: {e}"))?
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
        assert!(store.load(chat_id).await.unwrap().is_none());

        let mut state = ChatState::new(chat_id, test_user());
        state.data.insert("key".into(), serde_json::json!("value"));
        store.save(&state).await.unwrap();

        let loaded = store.load(chat_id).await.unwrap().unwrap();
        assert_eq!(loaded.chat_id, chat_id);
        assert_eq!(loaded.data["key"], "value");

        store.delete(chat_id).await.unwrap();
        assert!(store.load(chat_id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn all_chat_ids_works() {
        let dir = tempfile::tempdir().unwrap();
        let store = RedbStore::open(dir.path().join("ids.redb")).unwrap();

        for id in [1i64, 2, 3] {
            store
                .save(&ChatState::new(ChatId(id), test_user()))
                .await
                .unwrap();
        }
        let mut ids: Vec<i64> = store
            .all_chat_ids()
            .await
            .unwrap()
            .iter()
            .map(|c| c.0)
            .collect();
        ids.sort();
        assert_eq!(ids, vec![1, 2, 3]);
    }
}
