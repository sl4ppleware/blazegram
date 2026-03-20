//! File-backed session storage — `MemorySession` + periodic JSON persistence.
//!
//! All reads are instant RAM lookups. Writes go to memory first, then
//! flushed to disk atomically (tmp + rename). No SQLite, no C deps.
//!
//! ```ignore
//! let session = FileSession::open("bot.session").await;
//! // ... pass to SenderPool::new(Arc::new(session), api_id)
//! ```

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use std::future::Future;
use std::pin::Pin;

use grammers_session::{
    Session, SessionData,
    storages::MemorySession,
    types::{DcOption, PeerId, PeerInfo, UpdateState, UpdatesState},
};

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Serializable snapshot of session data.
#[derive(serde::Serialize, serde::Deserialize)]
struct SessionSnapshot {
    home_dc: i32,
    dc_options: std::collections::HashMap<i32, DcOption>,
    updates_state: UpdatesState,
    // peers omitted — re-cached from incoming updates
}

impl SessionSnapshot {
    fn into_session_data(self) -> SessionData {
        SessionData {
            home_dc: self.home_dc,
            dc_options: self.dc_options,
            updates_state: self.updates_state,
            ..SessionData::default()
        }
    }
}

/// A session that keeps everything in RAM and persists to a JSON file.
///
/// Reads are zero-cost memory lookups (delegated to `MemorySession`).
/// Writes set a dirty flag; call [`FileSession::flush`] or use
/// [`FileSession::start_flush_task`] to persist periodically.
pub struct FileSession {
    inner: MemorySession,
    path: PathBuf,
    dirty: AtomicBool,
    /// Mutex only for serialization during flush — reads don't touch it.
    flush_lock: tokio::sync::Mutex<()>,
}

impl FileSession {
    /// Open or create a session file. If the file exists, loads it.
    pub async fn open(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref().to_path_buf();
        let inner = if path.exists() {
            match tokio::fs::read(&path).await {
                Ok(bytes) => match serde_json::from_slice::<SessionSnapshot>(&bytes) {
                    Ok(snap) => {
                        tracing::info!(path = %path.display(), "session loaded");
                        MemorySession::from(snap.into_session_data())
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "corrupt session file, starting fresh");
                        MemorySession::default()
                    }
                },
                Err(e) => {
                    tracing::warn!(error = %e, "cannot read session file, starting fresh");
                    MemorySession::default()
                }
            }
        } else {
            MemorySession::default()
        };

        Self {
            inner,
            path,
            dirty: AtomicBool::new(false),
            flush_lock: tokio::sync::Mutex::new(()),
        }
    }

    /// Persist current state to disk (atomic write).
    pub async fn flush(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !self.dirty.swap(false, Ordering::AcqRel) {
            return Ok(()); // nothing changed
        }

        let _guard = self.flush_lock.lock().await;
        // Extract SessionData from the inner MemorySession.
        // MemorySession stores SessionData in a Mutex, we access it via
        // the Session trait methods.
        let snap = self.export_snapshot().await;

        let bytes = serde_json::to_vec_pretty(&snap)?;
        let tmp = self.path.with_extension("session.tmp");
        tokio::fs::write(&tmp, &bytes).await?;
        tokio::fs::rename(&tmp, &self.path).await?;
        tracing::debug!(path = %self.path.display(), "session flushed");
        Ok(())
    }

    /// Spawn a background task that flushes every `interval`.
    pub fn start_flush_task(
        self: &std::sync::Arc<Self>,
        interval: std::time::Duration,
    ) -> tokio::task::JoinHandle<()> {
        let session = std::sync::Arc::clone(self);
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(interval);
            loop {
                tick.tick().await;
                if let Err(e) = session.flush().await {
                    tracing::error!(error = %e, "session flush failed");
                }
            }
        })
    }

    /// Export session data for serialization.
    async fn export_snapshot(&self) -> SessionSnapshot {
        let home_dc = self.inner.home_dc_id();
        let mut dc_options = std::collections::HashMap::new();
        // DCs 1–5 are always known; also check 6–10 for test DCs
        for dc_id in 1..=10 {
            if let Some(opt) = self.inner.dc_option(dc_id) {
                dc_options.insert(dc_id, opt);
            }
        }
        let updates_state = self.inner.updates_state().await;
        SessionSnapshot {
            home_dc,
            dc_options,
            updates_state,
        }
    }

    fn mark_dirty(&self) {
        self.dirty.store(true, Ordering::Release);
    }
}

impl Session for FileSession {
    fn home_dc_id(&self) -> i32 {
        self.inner.home_dc_id()
    }

    fn set_home_dc_id(&self, dc_id: i32) -> BoxFuture<'_, ()> {
        Box::pin(async move {
            self.inner.set_home_dc_id(dc_id).await;
            self.mark_dirty();
        })
    }

    fn dc_option(&self, dc_id: i32) -> Option<DcOption> {
        self.inner.dc_option(dc_id)
    }

    fn set_dc_option(&self, dc_option: &DcOption) -> BoxFuture<'_, ()> {
        let dc_option = dc_option.clone();
        Box::pin(async move {
            self.inner.set_dc_option(&dc_option).await;
            self.mark_dirty();
        })
    }

    fn peer(&self, peer: PeerId) -> BoxFuture<'_, Option<PeerInfo>> {
        self.inner.peer(peer)
    }

    fn cache_peer(&self, peer: &PeerInfo) -> BoxFuture<'_, ()> {
        let peer = peer.clone();
        Box::pin(async move {
            self.inner.cache_peer(&peer).await;
            // Don't mark dirty for peer cache — too frequent, and peers
            // are re-populated from incoming updates on restart anyway.
        })
    }

    fn updates_state(&self) -> BoxFuture<'_, UpdatesState> {
        self.inner.updates_state()
    }

    fn set_update_state(&self, update: UpdateState) -> BoxFuture<'_, ()> {
        Box::pin(async move {
            self.inner.set_update_state(update).await;
            self.mark_dirty();
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use grammers_session::Session;

    #[tokio::test]
    async fn roundtrip_save_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.session");

        // Create a session, set home DC, and flush
        {
            let session = FileSession::open(&path).await;
            session.set_home_dc_id(2).await;
            session.mark_dirty();
            session.flush().await.expect("flush failed");
        }

        // Reopen and verify state persisted
        {
            let session = FileSession::open(&path).await;
            assert_eq!(
                session.home_dc_id(),
                2,
                "home DC should persist across save/load"
            );
        }
    }

    #[tokio::test]
    async fn flush_skips_when_clean() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("clean.session");

        let session = FileSession::open(&path).await;
        // No changes made — flush should be a no-op
        session.flush().await.expect("flush should succeed");
        // File should not exist since nothing was dirty
        assert!(
            !path.exists(),
            "no file should be written when session is clean"
        );
    }

    #[tokio::test]
    async fn corrupt_file_starts_fresh() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("corrupt.session");

        // Write garbage
        tokio::fs::write(&path, b"not valid json").await.unwrap();

        // Should load without panic, falling back to defaults
        let session = FileSession::open(&path).await;
        // Default MemorySession home_dc_id is 2 (Telegram default DC)
        let default_dc = grammers_session::storages::MemorySession::default().home_dc_id();
        assert_eq!(
            session.home_dc_id(),
            default_dc,
            "corrupt file should yield default DC"
        );
    }

    #[tokio::test]
    async fn set_home_dc_persists_across_sessions() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("dc.session");

        // Set DC 3, flush, reopen, verify
        {
            let session = FileSession::open(&path).await;
            session.set_home_dc_id(3).await;
            session.flush().await.expect("flush failed");
        }
        {
            let session = FileSession::open(&path).await;
            assert_eq!(session.home_dc_id(), 3);

            // Change to DC 5
            session.set_home_dc_id(5).await;
            session.flush().await.expect("flush failed");
        }
        {
            let session = FileSession::open(&path).await;
            assert_eq!(session.home_dc_id(), 5);
        }
    }

    #[tokio::test]
    async fn atomic_write_no_partial() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("atomic.session");
        let tmp_path = path.with_extension("session.tmp");

        let session = FileSession::open(&path).await;
        session.set_home_dc_id(4).await;
        session.flush().await.expect("flush failed");

        // After flush, tmp file should not exist (renamed to final path)
        assert!(
            !tmp_path.exists(),
            "tmp file should be cleaned up after atomic rename"
        );
        assert!(path.exists(), "final session file should exist");
    }
}
