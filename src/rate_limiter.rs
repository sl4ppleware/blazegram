//! Adaptive, multi-layer rate limiter for Telegram bots.
//!
//! Three layers:
//! 1. **Global** — 30 write requests/sec across all chats (configurable)
//! 2. **Per-chat** — ~1 req/sec for private chats (burst up to 3), 20 req/min for groups
//! 3. **Burst control** — allows brief spikes, then compensates
//!
//! Adaptive behavior:
//! - Tracks actual FLOOD_WAIT (429) responses and tightens limits dynamically
//! - After a cool-down period with no floods, gradually relaxes back to defaults
//! - Methods like `answer_callback_query` bypass the main limiter entirely
//! - Auto-retries on 429 with exponential backoff

use async_trait::async_trait;
use dashmap::DashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, Semaphore};
use tokio::time::sleep;

use crate::bot_api::{BotApi, SendOptions};
use crate::error::ApiError;
use crate::keyboard::InlineKeyboard;
use crate::types::*;

// ─── Constants ───

const DEFAULT_GLOBAL_RPS: u32 = 30;
const DEFAULT_PER_CHAT_BURST: u32 = 3;
const DEFAULT_PER_CHAT_INTERVAL_MS: u64 = 1000; // 1 req/sec for private chats
const GROUP_MIN_INTERVAL_MS: u64 = 3000; // 20/min ≈ 1 per 3sec
const MAX_RETRIES: u32 = 8;
const FLOOD_TIGHTEN_FACTOR: f64 = 0.6; // Reduce allowed rate to 60% after a flood
const FLOOD_RELAX_INTERVAL: Duration = Duration::from_secs(30); // Relax every 30s with no floods
const FLOOD_RELAX_STEP: f64 = 0.1; // Relax by 10% each step
const GLOBAL_WINDOW: Duration = Duration::from_secs(1);

// ─── Metrics ───

/// Rate limiter metrics, readable at any time.
#[derive(Debug)]
pub struct RateLimiterMetrics {
    /// Total API calls attempted (includes retries).
    pub total_calls: AtomicU64,
    /// Calls that were delayed by the rate limiter (not 429, just pre-emptive waits).
    pub throttled_calls: AtomicU64,
    /// Number of actual 429 responses received from Telegram.
    pub flood_waits: AtomicU64,
    /// Requests in the current 1-second window.
    pub current_window_count: AtomicU32,
    /// Configured max RPS (may be dynamically adjusted).
    pub effective_rps: AtomicU32,
}

impl RateLimiterMetrics {
    fn new(rps: u32) -> Self {
        Self {
            total_calls: AtomicU64::new(0),
            throttled_calls: AtomicU64::new(0),
            flood_waits: AtomicU64::new(0),
            current_window_count: AtomicU32::new(0),
            effective_rps: AtomicU32::new(rps),
        }
    }

    /// Current utilization as a percentage (0–100+).
    pub fn utilization_pct(&self) -> f64 {
        let current = self.current_window_count.load(Ordering::Relaxed) as f64;
        let max = self.effective_rps.load(Ordering::Relaxed) as f64;
        if max == 0.0 {
            return 0.0;
        }
        (current / max) * 100.0
    }
}

// ─── Per-Chat Bucket ───

/// Token-bucket state for a single chat.
struct ChatBucket {
    /// Available tokens (can go up to burst limit).
    tokens: f64,
    /// Last time tokens were replenished.
    last_refill: Instant,
    /// Max burst tokens.
    burst: u32,
    /// Milliseconds between token refills (1 token per interval).
    interval_ms: u64,
    /// Whether this is a group chat (negative chat_id).
    is_group: bool,
}

impl ChatBucket {
    fn new_private() -> Self {
        Self {
            tokens: DEFAULT_PER_CHAT_BURST as f64,
            last_refill: Instant::now(),
            burst: DEFAULT_PER_CHAT_BURST,
            interval_ms: DEFAULT_PER_CHAT_INTERVAL_MS,
            is_group: false,
        }
    }

    fn new_group() -> Self {
        Self {
            tokens: DEFAULT_PER_CHAT_BURST as f64,
            last_refill: Instant::now(),
            burst: DEFAULT_PER_CHAT_BURST,
            interval_ms: GROUP_MIN_INTERVAL_MS,
            is_group: true,
        }
    }

    /// Refill tokens based on elapsed time, returns current token count.
    fn refill(&mut self) -> f64 {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill);
        let new_tokens = elapsed.as_millis() as f64 / self.interval_ms as f64;
        if new_tokens > 0.0 {
            self.tokens = (self.tokens + new_tokens).min(self.burst as f64);
            self.last_refill = now;
        }
        self.tokens
    }

    /// Try to consume a token. Returns Ok(()) or the Duration to wait.
    fn try_consume(&mut self) -> Result<(), Duration> {
        self.refill();
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            Ok(())
        } else {
            // How long until we have 1 token?
            let deficit = 1.0 - self.tokens;
            let wait_ms = (deficit * self.interval_ms as f64).ceil() as u64;
            Err(Duration::from_millis(wait_ms))
        }
    }

    /// Tighten this bucket after a flood wait.
    fn tighten(&mut self) {
        let new_interval = (self.interval_ms as f64 / FLOOD_TIGHTEN_FACTOR) as u64;
        // Cap: don't go slower than 10 seconds per request.
        self.interval_ms = new_interval.min(10_000);
        self.tokens = 0.0; // Drain tokens on flood.
    }

    /// Relax this bucket back toward defaults.
    fn relax(&mut self) {
        let default_interval = if self.is_group {
            GROUP_MIN_INTERVAL_MS
        } else {
            DEFAULT_PER_CHAT_INTERVAL_MS
        };
        if self.interval_ms > default_interval {
            let new_interval = (self.interval_ms as f64 * (1.0 - FLOOD_RELAX_STEP)) as u64;
            self.interval_ms = new_interval.max(default_interval);
        }
    }
}

// ─── Global Sliding Window ───

/// Global rate limiter using a sliding-window counter with a semaphore for backpressure.
struct GlobalLimiter {
    /// Semaphore with permits = max RPS. Permits are released when the window slides.
    semaphore: Arc<Semaphore>,
    /// Timestamps of recent requests within the current window.
    timestamps: Mutex<Vec<Instant>>,
    /// Base (configured) max RPS.
    base_rps: u32,
    /// Current effective max RPS (may be reduced after floods).
    effective_rps: Arc<AtomicU32>,
}

impl GlobalLimiter {
    fn new(rps: u32, effective_rps: Arc<AtomicU32>) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(rps as usize)),
            timestamps: Mutex::new(Vec::with_capacity(rps as usize * 2)),
            base_rps: rps,
            effective_rps,
        }
    }

    /// Wait until a global slot is available, then record the request.
    async fn acquire(&self) {
        loop {
            // Clean up expired timestamps and release their permits.
            {
                let mut ts = self.timestamps.lock().await;
                let cutoff = Instant::now() - GLOBAL_WINDOW;
                let before = ts.len();
                ts.retain(|t| *t > cutoff);
                let released = before - ts.len();
                if released > 0 {
                    // Don't exceed the semaphore's max permits.
                    let max = self.effective_rps.load(Ordering::Relaxed) as usize;
                    let available = self.semaphore.available_permits();
                    let to_add = released.min(max.saturating_sub(available));
                    if to_add > 0 {
                        self.semaphore.add_permits(to_add);
                    }
                }

                let current_count = ts.len();
                let limit = self.effective_rps.load(Ordering::Relaxed) as usize;

                if current_count < limit {
                    ts.push(Instant::now());
                    return;
                }
            }

            // Window is full — wait a bit and retry.
            sleep(Duration::from_millis(10)).await;
        }
    }

    /// Record a flood event: tighten the global limit.
    fn on_flood(&self) {
        let current = self.effective_rps.load(Ordering::Relaxed);
        let tightened = ((current as f64) * FLOOD_TIGHTEN_FACTOR) as u32;
        let new_rps = tightened.max(5); // Never go below 5 RPS.
        self.effective_rps.store(new_rps, Ordering::Relaxed);
    }

    /// Gradually relax the global limit back toward the base.
    fn relax(&self) {
        let current = self.effective_rps.load(Ordering::Relaxed);
        if current < self.base_rps {
            let relaxed = ((current as f64) * (1.0 + FLOOD_RELAX_STEP)) as u32;
            let new_rps = relaxed.min(self.base_rps);
            self.effective_rps.store(new_rps, Ordering::Relaxed);
        }
    }

    /// Current count in the sliding window.
    async fn current_count(&self) -> usize {
        let ts = self.timestamps.lock().await;
        let cutoff = Instant::now() - GLOBAL_WINDOW;
        ts.iter().filter(|t| **t > cutoff).count()
    }
}

// ─── RateLimitedBotApi ───

/// A [`BotApi`] wrapper that enforces Telegram rate limits via a token bucket.
pub struct RateLimitedBotApi<B: BotApi> {
    inner: B,
    global: GlobalLimiter,
    chat_buckets: DashMap<i64, ChatBucket>,
    metrics: Arc<RateLimiterMetrics>,
    last_flood: Mutex<Option<Instant>>,
}

impl<B: BotApi> RateLimitedBotApi<B> {
    /// Wrap a BotApi with adaptive rate limiting.
    /// `rps` = max global requests per second (30 for public API, higher for local).
    pub fn new(inner: B, rps: u32) -> Self {
        let metrics = Arc::new(RateLimiterMetrics::new(rps));
        let effective_rps = Arc::new(AtomicU32::new(rps));
        metrics.effective_rps.store(rps, Ordering::Relaxed);

        Self {
            inner,
            global: GlobalLimiter::new(rps, effective_rps),
            chat_buckets: DashMap::new(),
            metrics: metrics.clone(),
            last_flood: Mutex::new(None),
        }
    }

    /// For Telegram public API (30 rps).
    pub fn public(inner: B) -> Self {
        Self::new(inner, DEFAULT_GLOBAL_RPS)
    }

    /// For local Bot API server (high throughput).
    pub fn local(inner: B) -> Self {
        Self::new(inner, 500)
    }

    /// Access metrics.
    pub fn metrics(&self) -> &Arc<RateLimiterMetrics> {
        &self.metrics
    }

    /// Get or create a per-chat bucket.
    fn get_chat_bucket(
        &self,
        chat_id: ChatId,
    ) -> dashmap::mapref::one::RefMut<'_, i64, ChatBucket> {
        self.chat_buckets.entry(chat_id.0).or_insert_with(|| {
            if chat_id.0 < 0 {
                ChatBucket::new_group()
            } else {
                ChatBucket::new_private()
            }
        })
    }

    /// Wait for both global and per-chat rate limits.
    async fn wait_for_slot(&self, chat_id: ChatId) {
        let mut throttled = false;

        // Per-chat rate limit.
        loop {
            let result = self.get_chat_bucket(chat_id).try_consume();
            match result {
                Ok(()) => break,
                Err(wait) => {
                    if !throttled {
                        throttled = true;
                        self.metrics.throttled_calls.fetch_add(1, Ordering::Relaxed);
                    }
                    sleep(wait).await;
                }
            }
        }

        // Global rate limit.
        self.global.acquire().await;

        // Update window count metric.
        let count = self.global.current_count().await;
        self.metrics
            .current_window_count
            .store(count as u32, Ordering::Relaxed);
    }

    /// Handle a 429 response: record, tighten limits, sleep.
    async fn handle_flood(&self, retry_after: u32, chat_id: Option<ChatId>) {
        self.metrics.flood_waits.fetch_add(1, Ordering::Relaxed);

        tracing::warn!(
            retry_after,
            chat_id = ?chat_id.map(|c| c.0),
            "FLOOD_WAIT from Telegram, tightening limits"
        );

        // Tighten global.
        self.global.on_flood();
        self.metrics.effective_rps.store(
            self.global.effective_rps.load(Ordering::Relaxed),
            Ordering::Relaxed,
        );

        // Tighten per-chat if applicable.
        if let Some(cid) = chat_id {
            self.get_chat_bucket(cid).tighten();
        }

        // Record flood time.
        *self.last_flood.lock().await = Some(Instant::now());
    }

    /// Periodically called to relax limits if no recent floods.
    async fn maybe_relax(&self) {
        let last = *self.last_flood.lock().await;
        if let Some(last_time) = last {
            if last_time.elapsed() > FLOOD_RELAX_INTERVAL {
                self.global.relax();
                self.metrics.effective_rps.store(
                    self.global.effective_rps.load(Ordering::Relaxed),
                    Ordering::Relaxed,
                );
                // Relax all chat buckets.
                for mut entry in self.chat_buckets.iter_mut() {
                    entry.value_mut().relax();
                }
            }
        }
    }

    /// Execute a rate-limited call with auto-retry on 429.
    async fn rate_limited_call<F, Fut, T>(
        &self,
        chat_id: Option<ChatId>,
        f: F,
    ) -> Result<T, ApiError>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<T, ApiError>>,
    {
        self.maybe_relax().await;

        let mut attempt = 0u32;
        loop {
            // Wait for rate limit slot (only for chat-bound requests).
            if let Some(cid) = chat_id {
                self.wait_for_slot(cid).await;
            } else {
                // Non-chat global requests still respect global limit.
                self.global.acquire().await;
            }

            self.metrics.total_calls.fetch_add(1, Ordering::Relaxed);

            match f().await {
                Ok(v) => return Ok(v),
                Err(ApiError::TooManyRequests { retry_after }) => {
                    attempt += 1;
                    self.handle_flood(retry_after, chat_id).await;

                    if attempt > MAX_RETRIES {
                        tracing::error!(
                            attempt,
                            retry_after,
                            "max retries exceeded for rate-limited call"
                        );
                        return Err(ApiError::TooManyRequests { retry_after });
                    }

                    // Exponential backoff: retry_after + jitter.
                    let base_wait = retry_after as u64;
                    let backoff = base_wait + (1u64 << attempt.min(5));
                    tracing::warn!(
                        attempt,
                        retry_after,
                        backoff_secs = backoff,
                        "rate limited, backing off"
                    );
                    sleep(Duration::from_secs(backoff)).await;
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Execute a call that bypasses the main rate limiter (e.g. answer_callback_query).
    /// Still retries on 429, but does not consume global/per-chat tokens.
    async fn bypass_call<F, Fut, T>(&self, f: F) -> Result<T, ApiError>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<T, ApiError>>,
    {
        self.metrics.total_calls.fetch_add(1, Ordering::Relaxed);

        let mut attempt = 0u32;
        loop {
            match f().await {
                Ok(v) => return Ok(v),
                Err(ApiError::TooManyRequests { retry_after }) => {
                    attempt += 1;
                    self.metrics.flood_waits.fetch_add(1, Ordering::Relaxed);

                    if attempt > MAX_RETRIES {
                        return Err(ApiError::TooManyRequests { retry_after });
                    }

                    let backoff = retry_after as u64 + (1u64 << attempt.min(4));
                    tracing::warn!(
                        attempt,
                        retry_after,
                        backoff_secs = backoff,
                        "bypass call rate limited, backing off"
                    );
                    sleep(Duration::from_secs(backoff)).await;
                }
                Err(e) => return Err(e),
            }
        }
    }
}

#[async_trait]
impl<B: BotApi> BotApi for RateLimitedBotApi<B> {
    async fn send_message(
        &self,
        chat_id: ChatId,
        content: MessageContent,
        opts: SendOptions,
    ) -> Result<SentMessage, ApiError> {
        self.rate_limited_call(Some(chat_id), || {
            self.inner
                .send_message(chat_id, content.clone(), opts.clone())
        })
        .await
    }

    async fn edit_message_text(
        &self,
        chat_id: ChatId,
        message_id: MessageId,
        text: String,
        parse_mode: ParseMode,
        keyboard: Option<InlineKeyboard>,
        link_preview: bool,
    ) -> Result<(), ApiError> {
        self.rate_limited_call(Some(chat_id), || {
            self.inner.edit_message_text(
                chat_id,
                message_id,
                text.clone(),
                parse_mode,
                keyboard.clone(),
                link_preview,
            )
        })
        .await
    }

    async fn edit_message_caption(
        &self,
        chat_id: ChatId,
        message_id: MessageId,
        caption: Option<String>,
        parse_mode: ParseMode,
        keyboard: Option<InlineKeyboard>,
    ) -> Result<(), ApiError> {
        self.rate_limited_call(Some(chat_id), || {
            self.inner.edit_message_caption(
                chat_id,
                message_id,
                caption.clone(),
                parse_mode,
                keyboard.clone(),
            )
        })
        .await
    }

    async fn edit_message_media(
        &self,
        chat_id: ChatId,
        message_id: MessageId,
        content: MessageContent,
        keyboard: Option<InlineKeyboard>,
    ) -> Result<(), ApiError> {
        self.rate_limited_call(Some(chat_id), || {
            self.inner
                .edit_message_media(chat_id, message_id, content.clone(), keyboard.clone())
        })
        .await
    }

    async fn edit_message_keyboard(
        &self,
        chat_id: ChatId,
        message_id: MessageId,
        keyboard: Option<InlineKeyboard>,
    ) -> Result<(), ApiError> {
        self.rate_limited_call(Some(chat_id), || {
            self.inner
                .edit_message_keyboard(chat_id, message_id, keyboard.clone())
        })
        .await
    }

    async fn delete_messages(
        &self,
        chat_id: ChatId,
        message_ids: Vec<MessageId>,
    ) -> Result<(), ApiError> {
        self.rate_limited_call(Some(chat_id), || {
            self.inner.delete_messages(chat_id, message_ids.clone())
        })
        .await
    }

    /// answer_callback_query bypasses the main rate limiter — it's not counted
    /// in the 30 RPS write limit and has much higher throughput.
    async fn answer_callback_query(
        &self,
        id: String,
        text: Option<String>,
        show_alert: bool,
    ) -> Result<(), ApiError> {
        self.bypass_call(|| {
            self.inner
                .answer_callback_query(id.clone(), text.clone(), show_alert)
        })
        .await
    }

    async fn send_chat_action(&self, chat_id: ChatId, action: ChatAction) -> Result<(), ApiError> {
        self.rate_limited_call(Some(chat_id), || {
            self.inner.send_chat_action(chat_id, action)
        })
        .await
    }

    async fn answer_inline_query(
        &self,
        query_id: String,
        results: Vec<InlineQueryResult>,
        next_offset: Option<String>,
        cache_time: Option<i32>,
        is_personal: bool,
    ) -> Result<(), ApiError> {
        self.inner
            .answer_inline_query(query_id, results, next_offset, cache_time, is_personal)
            .await
    }

    // ── Forward / Copy (rate-limited) ──

    async fn forward_message(
        &self,
        chat_id: ChatId,
        from_chat_id: ChatId,
        message_id: MessageId,
    ) -> Result<SentMessage, ApiError> {
        self.rate_limited_call(Some(chat_id), || {
            self.inner
                .forward_message(chat_id, from_chat_id, message_id)
        })
        .await
    }

    async fn copy_message(
        &self,
        chat_id: ChatId,
        from_chat_id: ChatId,
        message_id: MessageId,
    ) -> Result<MessageId, ApiError> {
        self.rate_limited_call(Some(chat_id), || {
            self.inner.copy_message(chat_id, from_chat_id, message_id)
        })
        .await
    }

    // ── Media (rate-limited) ──

    async fn send_media_group(
        &self,
        chat_id: ChatId,
        media: Vec<MediaGroupItem>,
    ) -> Result<Vec<SentMessage>, ApiError> {
        self.rate_limited_call(Some(chat_id), || {
            self.inner.send_media_group(chat_id, media.clone())
        })
        .await
    }

    async fn download_file(&self, file_id: &str) -> Result<DownloadedFile, ApiError> {
        // Downloads don't count against send rate limits
        self.inner.download_file(file_id).await
    }

    // ── Fun (rate-limited) ──

    async fn send_poll(&self, chat_id: ChatId, poll: SendPoll) -> Result<SentMessage, ApiError> {
        self.rate_limited_call(Some(chat_id), || {
            self.inner.send_poll(chat_id, poll.clone())
        })
        .await
    }

    async fn stop_poll(&self, chat_id: ChatId, message_id: MessageId) -> Result<(), ApiError> {
        self.rate_limited_call(Some(chat_id), || self.inner.stop_poll(chat_id, message_id))
            .await
    }

    async fn send_dice(&self, chat_id: ChatId, emoji: DiceEmoji) -> Result<SentMessage, ApiError> {
        self.rate_limited_call(Some(chat_id), || self.inner.send_dice(chat_id, emoji))
            .await
    }

    async fn send_contact(
        &self,
        chat_id: ChatId,
        contact: Contact,
    ) -> Result<SentMessage, ApiError> {
        self.rate_limited_call(Some(chat_id), || {
            self.inner.send_contact(chat_id, contact.clone())
        })
        .await
    }

    async fn send_venue(&self, chat_id: ChatId, venue: Venue) -> Result<SentMessage, ApiError> {
        self.rate_limited_call(Some(chat_id), || {
            self.inner.send_venue(chat_id, venue.clone())
        })
        .await
    }

    // ── Payments (bypass rate limiter) ──

    async fn send_invoice(
        &self,
        chat_id: ChatId,
        invoice: Invoice,
    ) -> Result<SentMessage, ApiError> {
        self.rate_limited_call(Some(chat_id), || {
            self.inner.send_invoice(chat_id, invoice.clone())
        })
        .await
    }

    async fn answer_pre_checkout_query(
        &self,
        id: String,
        ok: bool,
        error_message: Option<String>,
    ) -> Result<(), ApiError> {
        // Must respond immediately, bypass rate limiter
        self.inner
            .answer_pre_checkout_query(id, ok, error_message)
            .await
    }

    // ── Admin (rate-limited) ──

    async fn ban_chat_member(&self, chat_id: ChatId, user_id: UserId) -> Result<(), ApiError> {
        self.rate_limited_call(Some(chat_id), || {
            self.inner.ban_chat_member(chat_id, user_id)
        })
        .await
    }

    async fn unban_chat_member(&self, chat_id: ChatId, user_id: UserId) -> Result<(), ApiError> {
        self.rate_limited_call(Some(chat_id), || {
            self.inner.unban_chat_member(chat_id, user_id)
        })
        .await
    }

    async fn restrict_chat_member(
        &self,
        chat_id: ChatId,
        user_id: UserId,
        permissions: ChatPermissions,
    ) -> Result<(), ApiError> {
        self.rate_limited_call(Some(chat_id), || {
            self.inner
                .restrict_chat_member(chat_id, user_id, permissions.clone())
        })
        .await
    }

    async fn promote_chat_member(
        &self,
        chat_id: ChatId,
        user_id: UserId,
        permissions: ChatPermissions,
    ) -> Result<(), ApiError> {
        self.rate_limited_call(Some(chat_id), || {
            self.inner
                .promote_chat_member(chat_id, user_id, permissions.clone())
        })
        .await
    }

    async fn get_chat_member(
        &self,
        chat_id: ChatId,
        user_id: UserId,
    ) -> Result<ChatMember, ApiError> {
        // Read-only, no rate limit
        self.inner.get_chat_member(chat_id, user_id).await
    }

    async fn get_chat_member_count(&self, chat_id: ChatId) -> Result<i32, ApiError> {
        self.inner.get_chat_member_count(chat_id).await
    }

    async fn get_chat(&self, chat_id: ChatId) -> Result<ChatInfo, ApiError> {
        self.inner.get_chat(chat_id).await
    }

    async fn leave_chat(&self, chat_id: ChatId) -> Result<(), ApiError> {
        self.rate_limited_call(Some(chat_id), || self.inner.leave_chat(chat_id))
            .await
    }

    // ── Bot settings (bypass) ──

    async fn set_my_commands(&self, commands: Vec<BotCommand>) -> Result<(), ApiError> {
        self.inner.set_my_commands(commands).await
    }

    async fn delete_my_commands(&self) -> Result<(), ApiError> {
        self.inner.delete_my_commands().await
    }

    async fn get_me(&self) -> Result<BotInfo, ApiError> {
        self.inner.get_me().await
    }

    // ── Pinning (rate-limited) ──

    async fn pin_chat_message(
        &self,
        chat_id: ChatId,
        message_id: MessageId,
        silent: bool,
    ) -> Result<(), ApiError> {
        self.rate_limited_call(Some(chat_id), || {
            self.inner.pin_chat_message(chat_id, message_id, silent)
        })
        .await
    }

    async fn unpin_chat_message(
        &self,
        chat_id: ChatId,
        message_id: MessageId,
    ) -> Result<(), ApiError> {
        self.rate_limited_call(Some(chat_id), || {
            self.inner.unpin_chat_message(chat_id, message_id)
        })
        .await
    }

    async fn unpin_all_chat_messages(&self, chat_id: ChatId) -> Result<(), ApiError> {
        self.rate_limited_call(Some(chat_id), || {
            self.inner.unpin_all_chat_messages(chat_id)
        })
        .await
    }

    // ── Reactions (rate-limited) ──

    async fn set_message_reaction(
        &self,
        chat_id: ChatId,
        message_id: MessageId,
        emoji: &str,
    ) -> Result<(), ApiError> {
        self.rate_limited_call(Some(chat_id), || {
            self.inner.set_message_reaction(chat_id, message_id, emoji)
        })
        .await
    }

    // ── Invite links ──

    async fn create_chat_invite_link(
        &self,
        chat_id: ChatId,
        name: Option<&str>,
        expire_date: Option<i64>,
        member_limit: Option<i32>,
    ) -> Result<String, ApiError> {
        self.inner
            .create_chat_invite_link(chat_id, name, expire_date, member_limit)
            .await
    }

    async fn export_chat_invite_link(&self, chat_id: ChatId) -> Result<String, ApiError> {
        self.inner.export_chat_invite_link(chat_id).await
    }

    // ── Chat management (rate-limited writes, passthrough reads) ──

    async fn set_chat_permissions(
        &self,
        chat_id: ChatId,
        permissions: ChatPermissions,
    ) -> Result<(), ApiError> {
        self.rate_limited_call(Some(chat_id), || {
            self.inner.set_chat_permissions(chat_id, permissions.clone())
        })
        .await
    }

    async fn revoke_chat_invite_link(
        &self,
        chat_id: ChatId,
        invite_link: &str,
    ) -> Result<ChatInviteLink, ApiError> {
        self.rate_limited_call(Some(chat_id), || {
            self.inner.revoke_chat_invite_link(chat_id, invite_link)
        })
        .await
    }

    async fn approve_chat_join_request(
        &self,
        chat_id: ChatId,
        user_id: UserId,
    ) -> Result<(), ApiError> {
        self.rate_limited_call(Some(chat_id), || {
            self.inner.approve_chat_join_request(chat_id, user_id)
        })
        .await
    }

    async fn decline_chat_join_request(
        &self,
        chat_id: ChatId,
        user_id: UserId,
    ) -> Result<(), ApiError> {
        self.rate_limited_call(Some(chat_id), || {
            self.inner.decline_chat_join_request(chat_id, user_id)
        })
        .await
    }

    async fn set_chat_title(&self, chat_id: ChatId, title: &str) -> Result<(), ApiError> {
        self.rate_limited_call(Some(chat_id), || {
            self.inner.set_chat_title(chat_id, title)
        })
        .await
    }

    async fn set_chat_description(
        &self,
        chat_id: ChatId,
        description: Option<&str>,
    ) -> Result<(), ApiError> {
        self.rate_limited_call(Some(chat_id), || {
            self.inner.set_chat_description(chat_id, description)
        })
        .await
    }

    async fn set_chat_photo(&self, chat_id: ChatId, photo: FileSource) -> Result<(), ApiError> {
        self.rate_limited_call(Some(chat_id), || {
            self.inner.set_chat_photo(chat_id, photo.clone())
        })
        .await
    }

    async fn delete_chat_photo(&self, chat_id: ChatId) -> Result<(), ApiError> {
        self.rate_limited_call(Some(chat_id), || {
            self.inner.delete_chat_photo(chat_id)
        })
        .await
    }

    async fn get_chat_administrators(&self, chat_id: ChatId) -> Result<Vec<ChatMember>, ApiError> {
        // Read-only
        self.inner.get_chat_administrators(chat_id).await
    }

    async fn set_chat_administrator_custom_title(
        &self,
        chat_id: ChatId,
        user_id: UserId,
        custom_title: &str,
    ) -> Result<(), ApiError> {
        self.rate_limited_call(Some(chat_id), || {
            self.inner
                .set_chat_administrator_custom_title(chat_id, user_id, custom_title)
        })
        .await
    }

    async fn get_user_profile_photos(
        &self,
        user_id: UserId,
        offset: Option<i32>,
        limit: Option<i32>,
    ) -> Result<UserProfilePhotos, ApiError> {
        // Read-only
        self.inner.get_user_profile_photos(user_id, offset, limit).await
    }

    // ── Bot settings (bypass — low frequency, no chat context) ──

    async fn get_my_commands(&self) -> Result<Vec<BotCommand>, ApiError> {
        self.inner.get_my_commands().await
    }

    async fn set_my_description(
        &self,
        description: Option<&str>,
        language_code: Option<&str>,
    ) -> Result<(), ApiError> {
        self.inner.set_my_description(description, language_code).await
    }

    async fn get_my_description(
        &self,
        language_code: Option<&str>,
    ) -> Result<BotDescription, ApiError> {
        self.inner.get_my_description(language_code).await
    }

    async fn set_my_short_description(
        &self,
        short_description: Option<&str>,
        language_code: Option<&str>,
    ) -> Result<(), ApiError> {
        self.inner
            .set_my_short_description(short_description, language_code)
            .await
    }

    async fn get_my_short_description(
        &self,
        language_code: Option<&str>,
    ) -> Result<BotShortDescription, ApiError> {
        self.inner.get_my_short_description(language_code).await
    }

    async fn set_my_name(
        &self,
        name: Option<&str>,
        language_code: Option<&str>,
    ) -> Result<(), ApiError> {
        self.inner.set_my_name(name, language_code).await
    }

    async fn get_my_name(&self, language_code: Option<&str>) -> Result<BotName, ApiError> {
        self.inner.get_my_name(language_code).await
    }

    async fn set_chat_menu_button(
        &self,
        chat_id: Option<ChatId>,
        menu_button: MenuButton,
    ) -> Result<(), ApiError> {
        self.inner.set_chat_menu_button(chat_id, menu_button).await
    }

    async fn get_chat_menu_button(&self, chat_id: Option<ChatId>) -> Result<MenuButton, ApiError> {
        self.inner.get_chat_menu_button(chat_id).await
    }

    // ── Payments (bypass — time-sensitive) ──

    async fn answer_shipping_query(
        &self,
        shipping_query_id: String,
        ok: bool,
        shipping_options: Option<Vec<ShippingOption>>,
        error_message: Option<String>,
    ) -> Result<(), ApiError> {
        self.inner
            .answer_shipping_query(shipping_query_id, ok, shipping_options, error_message)
            .await
    }

    async fn create_invoice_link(&self, invoice: Invoice) -> Result<String, ApiError> {
        self.inner.create_invoice_link(invoice).await
    }

    // ── Batch operations (rate-limited) ──

    async fn forward_messages(
        &self,
        chat_id: ChatId,
        from_chat_id: ChatId,
        message_ids: Vec<MessageId>,
    ) -> Result<Vec<MessageId>, ApiError> {
        self.rate_limited_call(Some(chat_id), || {
            self.inner
                .forward_messages(chat_id, from_chat_id, message_ids.clone())
        })
        .await
    }

    async fn copy_messages(
        &self,
        chat_id: ChatId,
        from_chat_id: ChatId,
        message_ids: Vec<MessageId>,
    ) -> Result<Vec<MessageId>, ApiError> {
        self.rate_limited_call(Some(chat_id), || {
            self.inner
                .copy_messages(chat_id, from_chat_id, message_ids.clone())
        })
        .await
    }

    // ── Sticker & Location (rate-limited) ──

    async fn send_sticker(
        &self,
        chat_id: ChatId,
        sticker: FileSource,
    ) -> Result<SentMessage, ApiError> {
        self.rate_limited_call(Some(chat_id), || {
            self.inner.send_sticker(chat_id, sticker.clone())
        })
        .await
    }

    async fn send_location(
        &self,
        chat_id: ChatId,
        latitude: f64,
        longitude: f64,
    ) -> Result<SentMessage, ApiError> {
        self.rate_limited_call(Some(chat_id), || {
            self.inner.send_location(chat_id, latitude, longitude)
        })
        .await
    }

    // ── Forum topics (rate-limited) ──

    async fn create_forum_topic(
        &self,
        chat_id: ChatId,
        title: &str,
        icon_color: Option<i32>,
        icon_custom_emoji_id: Option<i64>,
    ) -> Result<ForumTopic, ApiError> {
        self.rate_limited_call(Some(chat_id), || {
            self.inner
                .create_forum_topic(chat_id, title, icon_color, icon_custom_emoji_id)
        })
        .await
    }

    async fn edit_forum_topic(
        &self,
        chat_id: ChatId,
        topic_id: i32,
        title: Option<&str>,
        icon_custom_emoji_id: Option<i64>,
        closed: Option<bool>,
        hidden: Option<bool>,
    ) -> Result<(), ApiError> {
        self.rate_limited_call(Some(chat_id), || {
            self.inner.edit_forum_topic(
                chat_id,
                topic_id,
                title,
                icon_custom_emoji_id,
                closed,
                hidden,
            )
        })
        .await
    }

    async fn delete_forum_topic(&self, chat_id: ChatId, topic_id: i32) -> Result<(), ApiError> {
        self.rate_limited_call(Some(chat_id), || {
            self.inner.delete_forum_topic(chat_id, topic_id)
        })
        .await
    }

    async fn unpin_all_forum_topic_messages(
        &self,
        chat_id: ChatId,
        topic_id: i32,
    ) -> Result<(), ApiError> {
        self.rate_limited_call(Some(chat_id), || {
            self.inner.unpin_all_forum_topic_messages(chat_id, topic_id)
        })
        .await
    }

    // ── Stars (bypass — no chat context) ──

    async fn get_star_transactions(
        &self,
        offset: Option<&str>,
        limit: Option<i32>,
    ) -> Result<StarTransactions, ApiError> {
        self.inner.get_star_transactions(offset, limit).await
    }

    async fn refund_star_payment(&self, user_id: UserId, charge_id: &str) -> Result<(), ApiError> {
        self.inner.refund_star_payment(user_id, charge_id).await
    }
}

// ─── Tests ───

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bot_api::SendOptions;
    use crate::mock::MockBotApi;
    use std::sync::atomic::AtomicU32;
    use std::time::Instant;

    #[tokio::test]
    async fn test_bypass_for_callback() {
        let mock = MockBotApi::new();
        let limiter = RateLimitedBotApi::new(mock, 2); // very low RPS

        // answer_callback_query should not be throttled by global limit.
        let start = Instant::now();
        for i in 0..10 {
            let _ = limiter
                .answer_callback_query(format!("cb_{i}"), None, false)
                .await;
        }
        let elapsed = start.elapsed();
        // Should be near-instant since bypass doesn't wait for global slots.
        assert!(
            elapsed < Duration::from_secs(1),
            "bypass calls took too long: {elapsed:?}"
        );
    }

    #[tokio::test]
    async fn test_metrics_increment() {
        let mock = MockBotApi::new();
        let limiter = RateLimitedBotApi::new(mock, 30);

        let _ = limiter
            .send_message(
                ChatId(123),
                MessageContent::Text {
                    text: "hello".into(),
                    parse_mode: ParseMode::Html,
                    keyboard: None,
                    link_preview: LinkPreview::Disabled,
                },
                SendOptions::default(),
            )
            .await;

        assert!(limiter.metrics().total_calls.load(Ordering::Relaxed) >= 1);
    }

    #[tokio::test]
    async fn test_chat_bucket_private_vs_group() {
        let limiter = RateLimitedBotApi::new(MockBotApi::new(), 30);

        // Private chat (positive ID).
        {
            let bucket = limiter.get_chat_bucket(ChatId(12345));
            assert!(!bucket.is_group);
            assert_eq!(bucket.interval_ms, DEFAULT_PER_CHAT_INTERVAL_MS);
        }

        // Group chat (negative ID).
        {
            let bucket = limiter.get_chat_bucket(ChatId(-100123));
            assert!(bucket.is_group);
            assert_eq!(bucket.interval_ms, GROUP_MIN_INTERVAL_MS);
        }
    }

    #[tokio::test]
    async fn test_global_limiter_basic() {
        let effective = Arc::new(AtomicU32::new(5));
        let gl = GlobalLimiter::new(5, effective);

        // Should be able to acquire 5 slots quickly.
        let start = Instant::now();
        for _ in 0..5 {
            gl.acquire().await;
        }
        assert!(start.elapsed() < Duration::from_millis(200));
    }

    #[tokio::test]
    async fn test_flood_tightens_and_relaxes() {
        let effective = Arc::new(AtomicU32::new(30));
        let gl = GlobalLimiter::new(30, effective.clone());

        gl.on_flood();
        let after_flood = effective.load(Ordering::Relaxed);
        assert!(after_flood < 30, "should tighten: got {after_flood}");

        gl.relax();
        let after_relax = effective.load(Ordering::Relaxed);
        assert!(
            after_relax >= after_flood,
            "should relax: {after_relax} >= {after_flood}"
        );
    }

    #[tokio::test]
    async fn test_chat_bucket_tighten_and_relax() {
        let mut bucket = ChatBucket::new_private();
        let orig_interval = bucket.interval_ms;

        bucket.tighten();
        assert!(bucket.interval_ms > orig_interval);
        assert_eq!(bucket.tokens, 0.0);

        // Relax many times to get back.
        for _ in 0..50 {
            bucket.relax();
        }
        assert_eq!(bucket.interval_ms, orig_interval);
    }
}
