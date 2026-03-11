//! Middleware system.

use async_trait::async_trait;
use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::error::HandlerResult;
use crate::types::*;

/// Middleware trait — runs before/after every update.
#[async_trait]
pub trait Middleware: Send + Sync + 'static {
    /// Called before handler. Return false to skip the update.
    async fn before(&self, chat_id: ChatId, user: &UserInfo, update: &IncomingUpdate) -> bool {
        let _ = (chat_id, user, update);
        true
    }
    /// Called after handler.
    async fn after(&self, chat_id: ChatId, user: &UserInfo, update: &IncomingUpdate, result: &HandlerResult) {
        let _ = (chat_id, user, update, result);
    }
}

// ─── Built-in: Logging ───

pub struct LoggingMiddleware;

#[async_trait]
impl Middleware for LoggingMiddleware {
    async fn before(&self, chat_id: ChatId, user: &UserInfo, update: &IncomingUpdate) -> bool {
        tracing::info!(
            chat_id = chat_id.0,
            user_id = user.id.0,
            update_type = update.type_name(),
            "incoming update"
        );
        true
    }
    async fn after(&self, chat_id: ChatId, _user: &UserInfo, update: &IncomingUpdate, result: &HandlerResult) {
        match result {
            Ok(()) => tracing::debug!(chat_id = chat_id.0, update_type = update.type_name(), "ok"),
            Err(e) => tracing::error!(chat_id = chat_id.0, error = %e, "handler error"),
        }
    }
}

// ─── Built-in: Auth ───

pub struct AuthMiddleware {
    allowed_ids: HashSet<u64>,
}

impl AuthMiddleware {
    pub fn new(ids: impl IntoIterator<Item = u64>) -> Self {
        Self { allowed_ids: ids.into_iter().collect() }
    }
}

#[async_trait]
impl Middleware for AuthMiddleware {
    async fn before(&self, _chat_id: ChatId, user: &UserInfo, _update: &IncomingUpdate) -> bool {
        if self.allowed_ids.contains(&user.id.0) {
            true
        } else {
            tracing::warn!(user_id = user.id.0, "unauthorized access blocked");
            false
        }
    }
}

// ─── Built-in: Throttle ───

pub struct ThrottleMiddleware {
    max_per_second: u64,
    counter: dashmap::DashMap<ChatId, (std::time::Instant, u64)>,
}

impl ThrottleMiddleware {
    pub fn new(max_per_second: u64) -> Self {
        Self {
            max_per_second,
            counter: dashmap::DashMap::new(),
        }
    }
}

#[async_trait]
impl Middleware for ThrottleMiddleware {
    async fn before(&self, chat_id: ChatId, _user: &UserInfo, _update: &IncomingUpdate) -> bool {
        let now = std::time::Instant::now();
        let mut entry = self.counter.entry(chat_id).or_insert((now, 0));
        if now.duration_since(entry.0).as_secs() >= 1 {
            *entry = (now, 1);
            true
        } else {
            entry.1 += 1;
            if entry.1 > self.max_per_second {
                tracing::warn!(chat_id = chat_id.0, "throttled");
                false
            } else {
                true
            }
        }
    }

    async fn after(&self, _chat_id: ChatId, _user: &UserInfo, _update: &IncomingUpdate, _result: &HandlerResult) {
        // Periodic cleanup: remove entries older than 60s to prevent unbounded growth.
        // Only run cleanup ~1% of the time to avoid overhead.
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if n % 100 == 0 {
            let now = std::time::Instant::now();
            self.counter.retain(|_, (ts, _)| now.duration_since(*ts).as_secs() < 60);
        }
    }
}

// ─── Built-in: Analytics ───

/// Tracks total updates, unique users, screen views.
pub struct AnalyticsMiddleware {
    pub total_updates: AtomicU64,
    pub total_messages: AtomicU64,
    pub total_callbacks: AtomicU64,
    pub unique_users: dashmap::DashMap<UserId, ()>,
}

impl AnalyticsMiddleware {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            total_updates: AtomicU64::new(0),
            total_messages: AtomicU64::new(0),
            total_callbacks: AtomicU64::new(0),
            unique_users: dashmap::DashMap::new(),
        })
    }

    pub fn stats(&self) -> (u64, u64, u64, usize) {
        (
            self.total_updates.load(Ordering::Relaxed),
            self.total_messages.load(Ordering::Relaxed),
            self.total_callbacks.load(Ordering::Relaxed),
            self.unique_users.len(),
        )
    }
}

impl Default for AnalyticsMiddleware {
    fn default() -> Self {
        Self {
            total_updates: AtomicU64::new(0),
            total_messages: AtomicU64::new(0),
            total_callbacks: AtomicU64::new(0),
            unique_users: dashmap::DashMap::new(),
        }
    }
}

#[async_trait]
impl Middleware for Arc<AnalyticsMiddleware> {
    async fn before(&self, _chat_id: ChatId, user: &UserInfo, update: &IncomingUpdate) -> bool {
        self.total_updates.fetch_add(1, Ordering::Relaxed);
        self.unique_users.insert(user.id, ());
        match update {
            IncomingUpdate::Message { .. } | IncomingUpdate::Photo { .. } | IncomingUpdate::Document { .. } => {
                self.total_messages.fetch_add(1, Ordering::Relaxed);
            }
            IncomingUpdate::CallbackQuery { .. } => {
                self.total_callbacks.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }
        true
    }
}
