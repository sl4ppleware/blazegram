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

use dashmap::DashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
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
pub struct RateLimiterMetrics {
    /// Total API calls attempted (includes retries).
    pub total_calls: AtomicU64,
    /// Calls that were delayed by the rate limiter (not 429, just pre-emptive waits).
    pub throttled_calls: AtomicU64,
    /// Number of actual 429 responses received from Telegram.
    pub flood_waits: AtomicU64,
    /// Requests in the current 1-second window.
    pub current_window_count: AtomicU32,
    /// Current effective max RPS. Shared with GlobalLimiter — always in sync.
    effective_rps: Arc<AtomicU32>,
}

impl std::fmt::Debug for RateLimiterMetrics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RateLimiterMetrics")
            .field("total_calls", &self.total_calls.load(Ordering::Relaxed))
            .field("throttled_calls", &self.throttled_calls.load(Ordering::Relaxed))
            .field("flood_waits", &self.flood_waits.load(Ordering::Relaxed))
            .field("current_window_count", &self.current_window_count.load(Ordering::Relaxed))
            .field("effective_rps", &self.effective_rps.load(Ordering::Relaxed))
            .finish()
    }
}

impl RateLimiterMetrics {
    fn new(effective_rps: Arc<AtomicU32>) -> Self {
        Self {
            total_calls: AtomicU64::new(0),
            throttled_calls: AtomicU64::new(0),
            flood_waits: AtomicU64::new(0),
            current_window_count: AtomicU32::new(0),
            effective_rps,
        }
    }

    /// Read the current effective RPS.
    pub fn effective_rps(&self) -> u32 {
        self.effective_rps.load(Ordering::Relaxed)
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

/// Global rate limiter using a sliding-window counter with Notify for wake-up.
struct GlobalLimiter {
    /// Timestamps of recent requests within the current window.
    timestamps: Mutex<Vec<Instant>>,
    /// Wakes waiters when a slot opens (timestamp expires).
    notify: tokio::sync::Notify,
    /// Base (configured) max RPS.
    base_rps: u32,
    /// Current effective max RPS (may be reduced after floods). Shared with metrics.
    effective_rps: Arc<AtomicU32>,
}

impl GlobalLimiter {
    fn new(rps: u32, effective_rps: Arc<AtomicU32>) -> Self {
        Self {
            timestamps: Mutex::new(Vec::with_capacity(rps as usize * 2)),
            notify: tokio::sync::Notify::new(),
            base_rps: rps,
            effective_rps,
        }
    }

    /// Wait until a global slot is available, then record the request.
    async fn acquire(&self) {
        loop {
            {
                let mut ts = self.timestamps.lock().await;
                let cutoff = Instant::now() - GLOBAL_WINDOW;
                ts.retain(|t| *t > cutoff);

                let limit = self.effective_rps.load(Ordering::Relaxed) as usize;
                if ts.len() < limit {
                    ts.push(Instant::now());
                    return;
                }

                // Calculate how long until the oldest timestamp expires.
                if let Some(oldest) = ts.first() {
                    let wait = GLOBAL_WINDOW.saturating_sub(oldest.elapsed());
                    if !wait.is_zero() {
                        // Drop lock before sleeping.
                        drop(ts);
                        // Wait for either the timeout or a notify (flood relax).
                        tokio::select! {
                            _ = sleep(wait) => {},
                            _ = self.notify.notified() => {},
                        }
                        continue;
                    }
                }
            }
            // Fallback: shouldn't reach here, but yield to avoid spin.
            sleep(Duration::from_millis(1)).await;
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
            self.notify.notify_waiters();
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
        let effective_rps = Arc::new(AtomicU32::new(rps));
        let metrics = Arc::new(RateLimiterMetrics::new(Arc::clone(&effective_rps)));

        Self {
            inner,
            global: GlobalLimiter::new(rps, Arc::clone(&effective_rps)),
            chat_buckets: DashMap::new(),
            metrics,
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

    /// Evict per-chat buckets idle for more than 10 minutes.
    /// Call periodically (e.g. from the GC task) to prevent unbounded growth.
    pub fn gc_idle_buckets(&self) {
        let cutoff = Duration::from_secs(600);
        let before = self.chat_buckets.len();
        self.chat_buckets.retain(|_, bucket| bucket.last_refill.elapsed() < cutoff);
        let evicted = before - self.chat_buckets.len();
        if evicted > 0 {
            tracing::debug!(evicted, remaining = self.chat_buckets.len(), "rate limiter: gc idle chat buckets");
        }
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

        // Tighten global (effective_rps is shared — metrics see it immediately).
        self.global.on_flood();

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
                self.global.relax(); // effective_rps is shared — metrics see it immediately
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

impl_rate_limited_botapi! {
    rate_limited: [
        fn send_message(chat_id: ChatId, content: MessageContent, opts: SendOptions) -> SentMessage;
        fn edit_message_text(chat_id: ChatId, message_id: MessageId, text: String, parse_mode: ParseMode, keyboard: Option<InlineKeyboard>, link_preview: bool) -> ();
        fn edit_message_caption(chat_id: ChatId, message_id: MessageId, caption: Option<String>, parse_mode: ParseMode, keyboard: Option<InlineKeyboard>) -> ();
        fn edit_message_media(chat_id: ChatId, message_id: MessageId, content: MessageContent, keyboard: Option<InlineKeyboard>) -> ();
        fn edit_message_keyboard(chat_id: ChatId, message_id: MessageId, keyboard: Option<InlineKeyboard>) -> ();
        fn delete_messages(chat_id: ChatId, message_ids: Vec<MessageId>) -> ();
        fn send_chat_action(chat_id: ChatId, action: ChatAction) -> ();
        fn forward_message(chat_id: ChatId, from_chat_id: ChatId, message_id: MessageId) -> SentMessage;
        fn copy_message(chat_id: ChatId, from_chat_id: ChatId, message_id: MessageId) -> MessageId;
        fn send_media_group(chat_id: ChatId, media: Vec<MediaGroupItem>) -> Vec<SentMessage>;
        fn send_poll(chat_id: ChatId, poll: SendPoll) -> SentMessage;
        fn stop_poll(chat_id: ChatId, message_id: MessageId) -> ();
        fn send_dice(chat_id: ChatId, emoji: DiceEmoji) -> SentMessage;
        fn send_contact(chat_id: ChatId, contact: Contact) -> SentMessage;
        fn send_venue(chat_id: ChatId, venue: Venue) -> SentMessage;
        fn send_invoice(chat_id: ChatId, invoice: Invoice) -> SentMessage;
        fn ban_chat_member(chat_id: ChatId, user_id: UserId) -> ();
        fn unban_chat_member(chat_id: ChatId, user_id: UserId) -> ();
        fn restrict_chat_member(chat_id: ChatId, user_id: UserId, permissions: ChatPermissions) -> ();
        fn promote_chat_member(chat_id: ChatId, user_id: UserId, permissions: ChatPermissions) -> ();
        fn leave_chat(chat_id: ChatId) -> ();
        fn pin_chat_message(chat_id: ChatId, message_id: MessageId, silent: bool) -> ();
        fn unpin_chat_message(chat_id: ChatId, message_id: MessageId) -> ();
        fn unpin_all_chat_messages(chat_id: ChatId) -> ();
        fn set_message_reaction(chat_id: ChatId, message_id: MessageId, emoji: &str) -> ();
        fn set_chat_permissions(chat_id: ChatId, permissions: ChatPermissions) -> ();
        fn revoke_chat_invite_link(chat_id: ChatId, invite_link: &str) -> ChatInviteLink;
        fn approve_chat_join_request(chat_id: ChatId, user_id: UserId) -> ();
        fn decline_chat_join_request(chat_id: ChatId, user_id: UserId) -> ();
        fn set_chat_title(chat_id: ChatId, title: &str) -> ();
        fn set_chat_description(chat_id: ChatId, description: Option<&str>) -> ();
        fn set_chat_photo(chat_id: ChatId, photo: FileSource) -> ();
        fn delete_chat_photo(chat_id: ChatId) -> ();
        fn set_chat_administrator_custom_title(chat_id: ChatId, user_id: UserId, custom_title: &str) -> ();
        fn forward_messages(chat_id: ChatId, from_chat_id: ChatId, message_ids: Vec<MessageId>) -> Vec<MessageId>;
        fn copy_messages(chat_id: ChatId, from_chat_id: ChatId, message_ids: Vec<MessageId>) -> Vec<MessageId>;
        fn send_sticker(chat_id: ChatId, sticker: FileSource) -> SentMessage;
        fn send_location(chat_id: ChatId, latitude: f64, longitude: f64) -> SentMessage;
        fn send_paid_media(chat_id: ChatId, star_count: i64, media: Vec<PaidMediaInput>, caption: Option<String>, parse_mode: ParseMode, opts: SendOptions) -> SentMessage;
        fn send_live_location(chat_id: ChatId, latitude: f64, longitude: f64, live_period: i32, opts: SendOptions) -> SentMessage;
        fn edit_message_live_location(chat_id: ChatId, message_id: MessageId, latitude: f64, longitude: f64) -> ();
        fn stop_message_live_location(chat_id: ChatId, message_id: MessageId) -> ();
        fn send_checklist(chat_id: ChatId, title: String, items: Vec<ChecklistItem>, opts: SendOptions) -> SentMessage;
        fn edit_message_checklist(chat_id: ChatId, message_id: MessageId, title: String, items: Vec<ChecklistItem>) -> ();
        fn send_message_draft(chat_id: ChatId, text: String, parse_mode: ParseMode) -> ();
        fn create_forum_topic(chat_id: ChatId, title: &str, icon_color: Option<i32>, icon_custom_emoji_id: Option<i64>) -> ForumTopic;
        fn edit_forum_topic(chat_id: ChatId, topic_id: i32, title: Option<&str>, icon_custom_emoji_id: Option<i64>, closed: Option<bool>, hidden: Option<bool>) -> ();
        fn delete_forum_topic(chat_id: ChatId, topic_id: i32) -> ();
        fn unpin_all_forum_topic_messages(chat_id: ChatId, topic_id: i32) -> ();
        fn send_game(chat_id: ChatId, game_short_name: &str, opts: SendOptions) -> SentMessage;
        fn set_chat_sticker_set(chat_id: ChatId, sticker_set_name: &str) -> ();
        fn delete_chat_sticker_set(chat_id: ChatId) -> ();
        fn approve_suggested_post(chat_id: ChatId, message_id: MessageId) -> ();
        fn decline_suggested_post(chat_id: ChatId, message_id: MessageId) -> ();
    ]
    bypass: [
        fn answer_callback_query(id: String, text: Option<String>, show_alert: bool) -> ();
    ]
    passthrough: [
        fn answer_inline_query(query_id: String, results: Vec<InlineQueryResult>, next_offset: Option<String>, cache_time: Option<i32>, is_personal: bool) -> ();
        fn download_file(file_id: &str) -> DownloadedFile;
        fn answer_pre_checkout_query(id: String, ok: bool, error_message: Option<String>) -> ();
        fn get_chat_member(chat_id: ChatId, user_id: UserId) -> ChatMember;
        fn get_chat_member_count(chat_id: ChatId) -> i32;
        fn get_chat(chat_id: ChatId) -> ChatInfo;
        fn set_my_commands(commands: Vec<BotCommand>) -> ();
        fn delete_my_commands() -> ();
        fn get_me() -> BotInfo;
        fn create_chat_invite_link(chat_id: ChatId, name: Option<&str>, expire_date: Option<i64>, member_limit: Option<i32>) -> String;
        fn export_chat_invite_link(chat_id: ChatId) -> String;
        fn get_chat_administrators(chat_id: ChatId) -> Vec<ChatMember>;
        fn get_user_profile_photos(user_id: UserId, offset: Option<i32>, limit: Option<i32>) -> UserProfilePhotos;
        fn get_my_commands() -> Vec<BotCommand>;
        fn set_my_description(description: Option<&str>, language_code: Option<&str>) -> ();
        fn get_my_description(language_code: Option<&str>) -> BotDescription;
        fn set_my_short_description(short_description: Option<&str>, language_code: Option<&str>) -> ();
        fn get_my_short_description(language_code: Option<&str>) -> BotShortDescription;
        fn set_my_name(name: Option<&str>, language_code: Option<&str>) -> ();
        fn get_my_name(language_code: Option<&str>) -> BotName;
        fn set_chat_menu_button(chat_id: Option<ChatId>, menu_button: MenuButton) -> ();
        fn get_chat_menu_button(chat_id: Option<ChatId>) -> MenuButton;
        fn answer_shipping_query(shipping_query_id: String, ok: bool, shipping_options: Option<Vec<ShippingOption>>, error_message: Option<String>) -> ();
        fn create_invoice_link(invoice: Invoice) -> String;
        fn get_star_transactions(offset: Option<&str>, limit: Option<i32>) -> StarTransactions;
        fn refund_star_payment(user_id: UserId, charge_id: &str) -> ();
        fn log_out() -> ();
        fn close() -> ();
        fn set_user_emoji_status(user_id: UserId, emoji_status_custom_emoji_id: Option<String>, emoji_status_expiration_date: Option<i64>) -> ();
        fn get_user_profile_audios(user_id: UserId, offset: Option<i32>, limit: Option<i32>) -> UserProfileAudios;
        fn ban_chat_sender_chat(chat_id: ChatId, sender_chat_id: ChatId) -> ();
        fn unban_chat_sender_chat(chat_id: ChatId, sender_chat_id: ChatId) -> ();
        fn set_chat_member_tag(chat_id: ChatId, user_id: UserId, tag: Option<String>) -> ();
        fn edit_chat_invite_link(chat_id: ChatId, invite_link: &str, name: Option<&str>, expire_date: Option<i64>, member_limit: Option<i32>) -> ChatInviteLink;
        fn create_chat_subscription_invite_link(chat_id: ChatId, name: Option<&str>, subscription_period: i32, subscription_price: i64) -> ChatInviteLink;
        fn edit_chat_subscription_invite_link(chat_id: ChatId, invite_link: &str, name: Option<&str>) -> ChatInviteLink;
        fn get_user_chat_boosts(chat_id: ChatId, user_id: UserId) -> UserChatBoosts;
        fn set_my_default_administrator_rights(rights: Option<ChatPermissions>, for_channels: Option<bool>) -> ();
        fn get_my_default_administrator_rights(for_channels: Option<bool>) -> ChatPermissions;
        fn set_my_profile_photo(photo: FileSource, is_public: Option<bool>) -> ();
        fn remove_my_profile_photo(file_id: Option<String>) -> ();
        fn verify_user(user_id: UserId, custom_description: Option<String>) -> ();
        fn verify_chat(chat_id: ChatId, custom_description: Option<String>) -> ();
        fn remove_user_verification(user_id: UserId) -> ();
        fn remove_chat_verification(chat_id: ChatId) -> ();
        fn get_business_connection(business_connection_id: &str) -> BusinessConnection;
        fn read_business_message(business_connection_id: &str, chat_id: ChatId, message_id: MessageId) -> ();
        fn delete_business_messages(business_connection_id: &str, message_ids: Vec<MessageId>) -> ();
        fn set_business_account_name(business_connection_id: &str, first_name: &str, last_name: Option<&str>) -> ();
        fn set_business_account_username(business_connection_id: &str, username: Option<&str>) -> ();
        fn set_business_account_bio(business_connection_id: &str, bio: Option<&str>) -> ();
        fn set_business_account_profile_photo(business_connection_id: &str, photo: FileSource, is_public: Option<bool>) -> ();
        fn remove_business_account_profile_photo(business_connection_id: &str, is_public: Option<bool>) -> ();
        fn set_business_account_gift_settings(business_connection_id: &str, show_gift_button: bool, accepted_gift_types: AcceptedGiftTypes) -> ();
        fn get_business_account_star_balance(business_connection_id: &str) -> StarBalance;
        fn transfer_business_account_stars(business_connection_id: &str, star_count: i64) -> ();
        fn get_business_account_gifts(business_connection_id: &str, exclude_unsaved: Option<bool>, exclude_saved: Option<bool>, exclude_unlimited: Option<bool>, exclude_limited: Option<bool>, exclude_unique: Option<bool>, sort_by_price: Option<bool>, offset: Option<&str>, limit: Option<i32>) -> OwnedGifts;
        fn get_available_gifts() -> Vec<Gift>;
        fn send_gift(user_id: UserId, gift_id: String, text: Option<String>, text_parse_mode: Option<ParseMode>) -> ();
        fn gift_premium_subscription(user_id: UserId, month_count: i32, star_count: i64, text: Option<String>, text_parse_mode: Option<ParseMode>) -> ();
        fn get_user_gifts(user_id: UserId, offset: Option<&str>, limit: Option<i32>) -> OwnedGifts;
        fn get_chat_gifts(chat_id: ChatId, offset: Option<&str>, limit: Option<i32>) -> OwnedGifts;
        fn convert_gift_to_stars(business_connection_id: Option<&str>, owned_gift_id: &str) -> ();
        fn upgrade_gift(business_connection_id: Option<&str>, owned_gift_id: &str, keep_original_details: Option<bool>, star_count: Option<i64>) -> ();
        fn transfer_gift(business_connection_id: Option<&str>, owned_gift_id: &str, new_owner_chat_id: ChatId, star_count: Option<i64>) -> ();
        fn post_story(chat_id: ChatId, content: StoryContent, active_period: i32, caption: Option<String>, parse_mode: Option<ParseMode>) -> Story;
        fn edit_story(chat_id: ChatId, story_id: i32, content: Option<StoryContent>, caption: Option<String>, parse_mode: Option<ParseMode>) -> Story;
        fn delete_story(chat_id: ChatId, story_id: i32) -> ();
        fn get_my_star_balance() -> StarBalance;
        fn edit_user_star_subscription(user_id: UserId, telegram_payment_charge_id: &str, is_canceled: bool) -> ();
        fn get_managed_bot_token(bot_id: UserId) -> String;
        fn replace_managed_bot_token(bot_id: UserId) -> String;
        fn save_prepared_keyboard_button(user_id: UserId, button: PreparedKeyboardButtonData) -> PreparedKeyboardButton;
        fn get_sticker_set(name: &str) -> StickerSet;
        fn get_custom_emoji_stickers(custom_emoji_ids: Vec<String>) -> Vec<StickerInfo>;
        fn upload_sticker_file(user_id: UserId, sticker: FileSource, sticker_format: StickerFormat) -> TelegramFile;
        fn create_new_sticker_set(user_id: UserId, name: String, title: String, stickers: Vec<InputSticker>, sticker_type: Option<StickerType>) -> ();
        fn add_sticker_to_set(user_id: UserId, name: &str, sticker: InputSticker) -> ();
        fn set_sticker_position_in_set(sticker: &str, position: i32) -> ();
        fn delete_sticker_from_set(sticker: &str) -> ();
        fn replace_sticker_in_set(user_id: UserId, name: &str, old_sticker: &str, sticker: InputSticker) -> ();
        fn set_sticker_emoji_list(sticker: &str, emoji_list: Vec<String>) -> ();
        fn set_sticker_keywords(sticker: &str, keywords: Vec<String>) -> ();
        fn set_sticker_mask_position(sticker: &str, mask_position: Option<MaskPosition>) -> ();
        fn set_sticker_set_title(name: &str, title: &str) -> ();
        fn set_sticker_set_thumbnail(name: &str, user_id: UserId, thumbnail: Option<FileSource>, format: StickerFormat) -> ();
        fn set_custom_emoji_sticker_set_thumbnail(name: &str, custom_emoji_id: Option<String>) -> ();
        fn delete_sticker_set(name: &str) -> ();
        fn get_forum_topic_icon_stickers() -> Vec<StickerInfo>;
        fn set_game_score(user_id: UserId, score: i64, chat_id: ChatId, message_id: MessageId, force: bool, disable_edit_message: bool) -> ();
        fn get_game_high_scores(user_id: UserId, chat_id: ChatId, message_id: MessageId) -> Vec<GameHighScore>;
        fn answer_web_app_query(web_app_query_id: &str, result: InlineQueryResult) -> SentWebAppMessage;
        fn save_prepared_inline_message(user_id: UserId, result: InlineQueryResult, allow_user_chats: Option<bool>, allow_bot_chats: Option<bool>, allow_group_chats: Option<bool>, allow_channel_chats: Option<bool>) -> PreparedInlineMessage;
        fn set_passport_data_errors(user_id: UserId, errors: Vec<PassportElementError>) -> ();
    ]
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

    #[tokio::test]
    async fn test_gc_idle_buckets() {
        let limiter = RateLimitedBotApi::new(MockBotApi::new(), 30);

        // Create some buckets.
        {
            let _ = limiter.get_chat_bucket(ChatId(1));
            let _ = limiter.get_chat_bucket(ChatId(2));
            let _ = limiter.get_chat_bucket(ChatId(3));
        }
        assert_eq!(limiter.chat_buckets.len(), 3);

        // GC with fresh buckets should keep all (last_refill is recent).
        limiter.gc_idle_buckets();
        assert_eq!(limiter.chat_buckets.len(), 3);

        // Manually age one bucket.
        limiter.chat_buckets.get_mut(&2).unwrap().last_refill =
            Instant::now() - Duration::from_secs(700);
        limiter.gc_idle_buckets();
        assert_eq!(limiter.chat_buckets.len(), 2);
        assert!(!limiter.chat_buckets.contains_key(&2));
    }
}
