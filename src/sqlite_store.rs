//! SQLite-backed persistent state store.
//!
//! Uses `spawn_blocking` to avoid blocking the tokio runtime during DB I/O.

use async_trait::async_trait;
use rusqlite::Connection;
use std::sync::{Arc, Mutex};

use crate::state::StateStore;
use crate::types::*;

pub struct SqliteStore {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteStore {
    /// Open (or create) a SQLite database at the given path.
    pub fn open(path: &str) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(path)?;
        // WAL mode + busy timeout BEFORE any writes for better concurrency
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "busy_timeout", 5000)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS chat_state (
                chat_id INTEGER PRIMARY KEY,
                data TEXT NOT NULL,
                updated_at INTEGER DEFAULT (strftime('%s','now'))
            );",
        )?;
        Ok(Self { conn: Arc::new(Mutex::new(conn)) })
    }

    /// In-memory SQLite (for testing).
    pub fn in_memory() -> Result<Self, rusqlite::Error> {
        Self::open(":memory:")
    }
}

#[async_trait]
impl StateStore for SqliteStore {
    async fn load(&self, chat_id: ChatId) -> Option<ChatState> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let mut stmt = conn
                .prepare_cached("SELECT data FROM chat_state WHERE chat_id = ?1")
                .ok()?;
            let json: String = stmt.query_row([chat_id.0], |row| row.get(0)).ok()?;
            serde_json::from_str(&json).ok()
        })
        .await
        .ok()
        .flatten()
    }

    async fn save(&self, state: &ChatState) {
        let conn = Arc::clone(&self.conn);
        let json = serde_json::to_string(state).unwrap();
        let chat_id = state.chat_id.0;
        let _ = tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            conn.execute(
                "INSERT OR REPLACE INTO chat_state (chat_id, data, updated_at) VALUES (?1, ?2, strftime('%s','now'))",
                rusqlite::params![chat_id, json],
            )
        })
        .await;
    }

    async fn delete(&self, chat_id: ChatId) {
        let conn = Arc::clone(&self.conn);
        let _ = tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            conn.execute("DELETE FROM chat_state WHERE chat_id = ?1", [chat_id.0])
        })
        .await;
    }

    async fn all_chat_ids(&self) -> Vec<ChatId> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let mut stmt = conn
                .prepare_cached("SELECT chat_id FROM chat_state")
                .unwrap();
            stmt.query_map([], |row| row.get::<_, i64>(0))
                .unwrap()
                .filter_map(|r| r.ok())
                .map(ChatId)
                .collect()
        })
        .await
        .unwrap_or_default()
    }
}
