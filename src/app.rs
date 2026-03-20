//! App — the main entry point for building a Blazegram bot.
//!
//! Uses grammers (pure Rust MTProto) for direct connection to Telegram DC.

use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;

use crate::file_session::FileSession;
use grammers_client::{Client, client::UpdatesConfiguration};
use grammers_mtsender::SenderPool;

use crate::bot_api::BotApi;
use crate::conversation::Conversation;
use crate::ctx::Ctx;
use crate::error::{HandlerError, HandlerResult};
use crate::form::{Form, FormData};
use crate::grammers_adapter::{DEFAULT_API_HASH, DEFAULT_API_ID, GrammersAdapter};
use crate::i18n::{self, I18n};
use crate::metrics::metrics;
use crate::middleware::Middleware;
use crate::router::Router;
use crate::serializer::ChatSerializer;
use crate::state::{InMemoryStore, StateStore};
use crate::types::*;
use crate::update_parser::convert_update;

/// Entry point for building a Blazegram bot. Use [`App::builder`] to start.
pub struct App;

/// Fluent builder for configuring and launching an [`App`].
pub struct AppBuilder {
    token: String,
    api_id: i32,
    api_hash: String,
    session_file: String,
    router: Router,
    store: Option<Arc<dyn StateStore>>,
    middlewares: Vec<Arc<dyn Middleware>>,
    forms: HashMap<String, Form>,
    conversations: HashMap<String, Conversation>,
    rate_limit_rps: Option<u32>,
    on_error: Option<Arc<ErrorHandler>>,
    snapshot_path: Option<String>,
    snapshot_interval: std::time::Duration,
    max_state_keys: usize,
}

type ErrorHandler = dyn Fn(ChatId, HandlerError) + Send + Sync;

impl App {
    /// Create a new bot application builder with the given token.
    pub fn builder(token: impl Into<String>) -> AppBuilder {
        AppBuilder {
            token: token.into(),
            api_id: DEFAULT_API_ID,
            api_hash: DEFAULT_API_HASH.to_string(),
            session_file: "bot.session".to_string(),
            router: Router::new(),
            store: None,
            middlewares: Vec::new(),
            forms: HashMap::new(),
            conversations: HashMap::new(),
            rate_limit_rps: None,
            on_error: None,
            snapshot_path: None,
            snapshot_interval: std::time::Duration::from_secs(300),
            max_state_keys: 1000,
        }
    }
}

impl AppBuilder {
    /// Override Telegram API credentials (default: TDesktop).
    pub fn api_credentials(mut self, api_id: i32, api_hash: impl Into<String>) -> Self {
        self.api_id = api_id;
        self.api_hash = api_hash.into();
        self
    }

    /// Session file path for MTProto auth keys (default: "bot.session").
    pub fn session_file(mut self, path: impl Into<String>) -> Self {
        self.session_file = path.into();
        self
    }

    /// Set the state persistence backend (default: in-memory).
    pub fn store(mut self, store: impl StateStore + 'static) -> Self {
        self.store = Some(Arc::new(store));
        self
    }

    /// Register a middleware that runs before every handler.
    pub fn middleware(mut self, m: impl Middleware + 'static) -> Self {
        self.middlewares.push(Arc::new(m));
        self
    }

    /// Register a handler for a `/command`.
    pub fn command(
        mut self,
        name: &str,
        handler: impl Fn(&mut Ctx) -> std::pin::Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.router.command(name, handler);
        self
    }

    /// Register a handler for a callback query prefix (e.g. `"pick"` matches `"pick:a"`).
    pub fn callback(
        mut self,
        prefix: &str,
        handler: impl Fn(&mut Ctx) -> std::pin::Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.router.callback(prefix, handler);
        self
    }

    /// Register a text input handler for a specific screen.
    pub fn on_input(
        mut self,
        screen_id: &str,
        handler: impl Fn(
            &mut Ctx,
            String,
        ) -> std::pin::Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.router.on_input(screen_id, handler);
        self
    }

    /// Register a media input handler for a specific screen.
    pub fn on_media_input(
        mut self,
        screen_id: &str,
        handler: impl Fn(
            &mut Ctx,
            ReceivedMedia,
        ) -> std::pin::Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.router.on_media_input(screen_id, handler);
        self
    }

    /// Catch-all handler for any text message not matched by screen-specific input handlers.
    pub fn on_any_text(
        mut self,
        handler: impl Fn(
            &mut Ctx,
            String,
        ) -> std::pin::Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.router.on_any_text(handler);
        self
    }

    /// Handler for unrecognized commands / messages that match no other route.
    pub fn on_unrecognized(
        mut self,
        handler: impl Fn(&mut Ctx) -> std::pin::Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.router.on_unrecognized(handler);
        self
    }

    /// Register a handler for inline queries. The handler receives `(ctx, query, offset)`.
    pub fn on_inline(
        mut self,
        handler: impl Fn(
            &mut Ctx,
            String,
            String,
        ) -> std::pin::Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.router.on_inline(handler);
        self
    }

    /// Register a handler for when a user picks one of the inline results.
    pub fn on_chosen_inline(
        mut self,
        handler: impl Fn(&mut Ctx) -> std::pin::Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.router.on_chosen_inline(handler);
        self
    }

    /// Register a handler for edited messages.
    pub fn on_message_edited(
        mut self,
        handler: impl Fn(
            &mut Ctx,
            String,
        ) -> std::pin::Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.router.on_message_edited(handler);
        self
    }

    /// Handler for pre-checkout queries (payment flow).
    /// The handler should call `ctx.approve_checkout()` or `ctx.decline_checkout(reason)` to approve/decline.
    pub fn on_pre_checkout(
        mut self,
        handler: impl Fn(&mut Ctx) -> std::pin::Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.router.on_pre_checkout(handler);
        self
    }

    /// Handler for successful payments.
    pub fn on_successful_payment(
        mut self,
        handler: impl Fn(&mut Ctx) -> std::pin::Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.router.on_successful_payment(handler);
        self
    }

    /// Handler for new members joining the chat.
    pub fn on_member_joined(
        mut self,
        handler: impl Fn(&mut Ctx) -> std::pin::Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.router.on_member_joined(handler);
        self
    }

    /// Handler for members leaving the chat.
    pub fn on_member_left(
        mut self,
        handler: impl Fn(&mut Ctx) -> std::pin::Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.router.on_member_left(handler);
        self
    }

    /// Handler for [Web App](https://core.telegram.org/bots/webapps) data.
    pub fn on_web_app_data(
        mut self,
        handler: impl Fn(
            &mut Ctx,
            String,
        ) -> std::pin::Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.router.on_web_app_data(handler);
        self
    }

    /// Register a multi-step [`Form`].
    pub fn form(mut self, form: Form) -> Self {
        self.forms.insert(form.id.clone(), form);
        self
    }

    /// Register a [`RouterGroup`](crate::router::RouterGroup) with its own middleware stack.
    pub fn group(mut self, group: crate::router::RouterGroup) -> Self {
        self.router.group(group);
        self
    }

    /// Register a branching [`Conversation`].
    pub fn conversation(mut self, conv: Conversation) -> Self {
        self.conversations.insert(conv.id.clone(), conv);
        self
    }

    /// Set the maximum Telegram API requests per second (wraps BotApi in a rate limiter).
    pub fn rate_limit(mut self, rps: u32) -> Self {
        self.rate_limit_rps = Some(rps);
        self
    }

    /// Control whether unrecognized messages are silently deleted (default: `true`).
    ///
    /// When `true`, messages that match no command, callback, or input handler
    /// are automatically deleted to keep the chat clean. Set to `false` to
    /// leave user messages untouched.
    pub fn delete_unrecognized(mut self, yes: bool) -> Self {
        self.router.delete_unrecognized = yes;
        self
    }

    /// Maximum number of keys in per-chat state data (default: 1000).
    ///
    /// When the limit is reached, `ctx.set()` logs a warning and the oldest
    /// key is evicted. Prevents unbounded memory growth from accidental
    /// state accumulation.
    pub fn max_state_keys(mut self, max: usize) -> Self {
        self.max_state_keys = max;
        self
    }

    /// Set a custom [`I18n`] instance for translations.
    pub fn i18n(self, i: I18n) -> Self {
        i18n::set_i18n(i);
        self
    }

    /// Locales.
    pub fn locales(self, dir: &str, default_lang: &str) -> Self {
        let i = I18n::load(dir, default_lang).unwrap_or_else(|e| {
            panic!("AppBuilder::locales(): failed to load locales from {dir:?}: {e}")
        });
        i18n::set_i18n(i);
        self
    }

    /// Use Redis as the state backend. Requires the `redis` feature.
    #[cfg(feature = "redis")]
    pub fn redis_store(self, url: &str) -> Self {
        let store = crate::redis_store::RedisStore::new(url).unwrap_or_else(|e| {
            panic!("AppBuilder::redis_store(): failed to connect to Redis at {url:?}: {e}")
        });
        self.store(store)
    }

    /// Use redb (pure Rust, ACID) as the persistent state backend.
    #[cfg(feature = "redb")]
    pub fn redb_store(self, path: &str) -> Self {
        let store = crate::redb_store::RedbStore::open(path).unwrap_or_else(|e| {
            panic!("AppBuilder::redb_store(): failed to open redb store at {path:?}: {e}")
        });
        self.store(store)
    }

    /// Register a global error handler, called when any handler returns an error.
    pub fn on_error(
        mut self,
        handler: impl Fn(ChatId, HandlerError) + Send + Sync + 'static,
    ) -> Self {
        self.on_error = Some(Arc::new(handler));
        self
    }

    /// Enable periodic state snapshots to disk (InMemoryStore only).
    pub fn snapshot(mut self, path: impl Into<String>) -> Self {
        self.snapshot_path = Some(path.into());
        self
    }

    /// Set snapshot interval (default: 5 minutes).
    pub fn snapshot_interval(mut self, interval: std::time::Duration) -> Self {
        self.snapshot_interval = interval;
        self
    }

    /// Build and run the bot. Phases: connect → event_loop → shutdown.
    pub async fn run(self) {
        // ━━━ Phase 1: Build state & connect ━━━
        let snapshot_store: Option<Arc<InMemoryStore>>;
        let store: Arc<dyn StateStore> = if let Some(custom) = self.store {
            snapshot_store = None;
            custom
        } else {
            let mem = Arc::new(InMemoryStore::new());
            if let Some(ref snap_path) = self.snapshot_path {
                match mem.restore(snap_path).await {
                    Ok(0) => tracing::info!("No snapshot found, starting fresh"),
                    Ok(n) => tracing::info!(count = n, "Restored state from snapshot"),
                    Err(e) => tracing::error!(error = %e, "Failed to restore snapshot"),
                }
                snapshot_store = Some(Arc::clone(&mem));
            } else {
                snapshot_store = None;
            }
            mem
        };

        let serializer = Arc::new(ChatSerializer::new(store));
        let router = Arc::new(self.router);
        let middlewares = Arc::new(self.middlewares);
        let forms = Arc::new(self.forms);
        let conversations = Arc::new(self.conversations);
        let on_error = self.on_error;

        tracing::info!("Blazegram: connecting via MTProto...");

        // ── Create grammers session & client (pure Rust, no SQLite) ──
        let session = Arc::new(FileSession::open(&self.session_file).await);

        let SenderPool {
            runner,
            updates,
            handle,
        } = SenderPool::new(Arc::clone(&session) as _, self.api_id);
        let client = Client::new(handle.clone());

        // Spawn the sender pool runner
        let pool_task = tokio::spawn(runner.run());

        // ── Bot sign-in ──
        let is_authorized = match client.is_authorized().await {
            Ok(v) => v,
            Err(e) => {
                tracing::error!(error = %e, "Authorization check failed, aborting");
                return;
            }
        };
        if !is_authorized {
            tracing::info!("Signing in as bot...");
            if let Err(e) = client.bot_sign_in(&self.token, &self.api_hash).await {
                tracing::error!(error = %e, "Bot sign-in failed, aborting");
                return;
            }
            tracing::info!("Signed in successfully.");
        } else {
            tracing::info!("Already authorized (session restored).");
        }

        // ── Build adapter ──
        let adapter = GrammersAdapter::new(client.clone());

        // Restore peer cache from disk if snapshot is enabled
        if let Some(ref snap_path) = self.snapshot_path {
            let peers_path = format!("{}.peers", snap_path);
            if let Ok(bytes) = tokio::fs::read(&peers_path).await {
                if let Ok(peers) = postcard::from_bytes::<Vec<(i64, i64, i64)>>(&bytes) {
                    let count = peers.len();
                    adapter.import_peers(&peers);
                    tracing::info!(count, "Restored peer cache from disk");
                }
            }
        }
        let bot_api: Arc<dyn BotApi> = if let Some(rps) = self.rate_limit_rps {
            Arc::new(crate::rate_limiter::RateLimitedBotApi::new(
                adapter.clone(),
                rps,
            ))
        } else {
            Arc::new(adapter.clone())
        };

        // ── Session flush task (persist auth keys + update state every 30s) ──
        session.start_flush_task(std::time::Duration::from_secs(30));

        // ── GC task ──
        let gc_ser = serializer.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(600));
            loop {
                interval.tick().await;
                gc_ser.gc();
            }
        });

        // ── Snapshot task ──
        if let (Some(mem_store), Some(snap_path)) = (&snapshot_store, &self.snapshot_path) {
            mem_store.start_snapshot_task(snap_path.clone(), self.snapshot_interval);
            tracing::info!(
                interval_secs = self.snapshot_interval.as_secs(),
                "Snapshot task started"
            );
        }

        // ── Scheduler ──
        let (sched_cb_tx, mut sched_cb_rx) = tokio::sync::mpsc::unbounded_channel();
        let scheduler = crate::scheduler::spawn_scheduler(bot_api.clone(), sched_cb_tx);

        // ── Build shared runtime ──
        let runtime = Runtime {
            bot_api: bot_api.clone(),
            router: router.clone(),
            serializer: serializer.clone(),
            middlewares: middlewares.clone(),
            forms: forms.clone(),
            conversations: conversations.clone(),
            grammers_client: client.clone(),
            peer_cache: adapter.peer_cache(),
            on_error: on_error.clone(),
            max_state_keys: self.max_state_keys,
            scheduler,
        };

        // ━━━ Phase 2: Event loop ━━━
        tracing::info!("Blazegram bot running. Waiting for updates...");
        let mut update_stream = client
            .stream_updates(
                updates,
                UpdatesConfiguration {
                    catch_up: true,
                    ..Default::default()
                },
            )
            .await;

        let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to register SIGTERM");

        loop {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    tracing::info!("Ctrl+C received, shutting down...");
                    break;
                }
                _ = sigterm.recv() => {
                    tracing::info!("SIGTERM received, shutting down...");
                    break;
                }
                Some((chat_id, kind)) = sched_cb_rx.recv() => {
                    if let crate::scheduler::ScheduledKind::Callback(data) = kind {
                        // Load existing user info from state to avoid overwriting
                        // with fake scheduler UserInfo.
                        let user = match serializer.store.load(chat_id).await {
                            Ok(Some(s)) => s.user,
                            _ => UserInfo {
                                id: UserId(0),
                                first_name: "scheduler".to_string(),
                                last_name: None,
                                username: None,
                                language_code: None,
                            },
                        };
                        let rt = runtime.clone();
                        tokio::spawn(async move {
                            let incoming = IncomingUpdate {
                                chat_id,
                                user,
                                message_id: None,
                                kind: UpdateKind::CallbackQuery {
                                    id: "__scheduled".to_string(),
                                    data: Some(data),
                                    inline_message_id: None,
                                },
                            };
                            process_update(incoming, rt).await;
                        });
                    }
                }
                result = update_stream.next() => {
                    let update = match result {
                        Ok(update) => update,
                        Err(e) => {
                            tracing::error!(error = %e, "update stream error");
                            continue;
                        }
                    };

                    // Convert grammers Update → IncomingUpdate + cache peer
                    if let Some((incoming, peer_ref)) = convert_update(&update).await {
                        adapter.cache_peer(peer_ref);

                        // ── Inline queries & chosen results: fast path (no chat state, no serializer) ──
                        if matches!(&incoming.kind, UpdateKind::InlineQuery { .. } | UpdateKind::ChosenInlineResult { .. }) {
                            let bot = bot_api.clone();
                            let router = router.clone();
                            tokio::spawn(async move {
                                handle_inline_fast(incoming, bot, router).await;
                            });
                            continue;
                        }

                        let rt = runtime.clone();
                        tokio::spawn(async move {
                            process_update(incoming, rt).await;
                        });
                    }
                }
            }
        }

        // ━━━ Phase 3: Graceful shutdown ━━━
        tracing::info!("Syncing update state...");
        update_stream.sync_update_state().await;

        // Flush session (auth keys, update state) to disk
        if let Err(e) = session.flush().await {
            tracing::error!(error = %e, "failed to flush session");
        } else {
            tracing::info!("Session flushed to disk");
        }

        // Final snapshot before exit
        if let (Some(mem_store), Some(snap_path)) = (&snapshot_store, &self.snapshot_path) {
            tracing::info!("Saving final snapshot...");
            if let Err(e) = mem_store.snapshot(snap_path).await {
                tracing::error!(error = %e, "Failed to save final snapshot");
            } else {
                tracing::info!(chats = mem_store.len(), "Snapshot saved");
            }
            // Persist peer cache alongside snapshot
            let peers_path = format!("{}.peers", snap_path);
            let peers = adapter.export_peers();
            if let Ok(bytes) = postcard::to_allocvec(&peers) {
                let tmp = format!("{}.tmp", peers_path);
                let _ = tokio::fs::write(&tmp, bytes).await;
                let _ = tokio::fs::rename(&tmp, &peers_path).await;
                tracing::info!(count = peers.len(), "Peer cache saved");
            }
        }

        handle.quit();
        let _ = pool_task.await;
        tracing::info!("Blazegram bot stopped.");
    }
}

// ── Inline query: fast path (no chat state, no serializer) ──

async fn handle_inline_fast(
    incoming: IncomingUpdate,
    bot_api: Arc<dyn BotApi>,
    router: Arc<Router>,
) {
    let user = incoming.user().clone();
    let chat_id = ChatId(user.id.0 as i64);
    let dummy_state = ChatState::new(chat_id, user);
    let mut ctx = Ctx::new(dummy_state, bot_api.clone(), None);

    match &incoming.kind {
        UpdateKind::InlineQuery { query, offset, id } => {
            ctx.inline_query_id = Some(id.clone());
            tracing::debug!(query_id = %id, query = %query, "dispatching inline query to handler");
            match router
                .dispatch_inline(&mut ctx, query.clone(), offset.clone())
                .await
            {
                Ok(()) => tracing::debug!("inline query handler completed OK"),
                Err(e) => tracing::error!(error = %e, "inline query handler error"),
            }
        }
        UpdateKind::ChosenInlineResult {
            result_id,
            inline_message_id,
            ..
        } => {
            ctx.chosen_inline_result_id = Some(result_id.clone());
            if let Some(imid) = inline_message_id {
                ctx.mode = CtxMode::Inline {
                    inline_message_id: imid.clone(),
                };
            }
            if let Err(e) = router.route(&mut ctx, &incoming).await {
                tracing::error!(error = %e, "chosen inline result handler error");
            }
        }
        _ => {}
    }
}

// ── Shared runtime context (replaces 7+ Arc arguments) ──

#[derive(Clone)]
struct Runtime {
    bot_api: Arc<dyn BotApi>,
    router: Arc<Router>,
    serializer: Arc<ChatSerializer>,
    middlewares: Arc<Vec<Arc<dyn Middleware>>>,
    forms: Arc<HashMap<String, Form>>,
    conversations: Arc<HashMap<String, Conversation>>,
    grammers_client: grammers_client::Client,
    peer_cache: Arc<dashmap::DashMap<i64, grammers_session::types::PeerRef>>,
    on_error: Option<Arc<ErrorHandler>>,
    max_state_keys: usize,
    scheduler: crate::scheduler::SchedulerHandle,
}

// ── Process update ──

#[tracing::instrument(skip_all, fields(chat_id = %incoming.chat_id().0, user_id = %incoming.user().id.0))]
async fn process_update(incoming: IncomingUpdate, rt: Runtime) {
    metrics().inc_updates();
    let _timer = metrics().timer("update");

    let chat_id = incoming.chat_id();
    let user = incoming.user().clone();

    // (inline queries use user_id as pseudo chat_id)

    for mw in rt.middlewares.iter() {
        if !mw.before(chat_id, &user, &incoming).await {
            return;
        }
    }

    rt.serializer
        .serialize(chat_id, &user, |state| {
            let rt = rt.clone();
            let incoming = incoming.clone();

            async move {
                let callback_data = match &incoming.kind {
                    UpdateKind::CallbackQuery { data, .. } => data.clone(),
                    _ => None,
                };

                let mut ctx = Ctx::new(state, rt.bot_api.clone(), callback_data);
                ctx.grammers_client = Some(rt.grammers_client.clone());
                ctx.peer_cache = Some(rt.peer_cache.clone());
                ctx.max_state_keys = rt.max_state_keys;
                ctx.scheduler = Some(rt.scheduler.clone());

                // Determine CtxMode
                let cid = incoming.chat_id;
                if let UpdateKind::CallbackQuery {
                    inline_message_id: Some(ref imid),
                    ..
                } = incoming.kind
                {
                    ctx.mode = CtxMode::Inline {
                        inline_message_id: imid.clone(),
                    };
                    tracing::debug!(imid = %imid, "inline callback detected");
                } else if cid.0 < 0 {
                    let trigger = match &incoming.kind {
                        UpdateKind::CallbackQuery { .. } => incoming.message_id,
                        _ => None,
                    };
                    ctx.mode = CtxMode::Group {
                        trigger_message_id: trigger,
                    };
                }

                if let UpdateKind::CallbackQuery { id, .. } = &incoming.kind {
                    ctx.state.pending_callback_id = Some(id.clone());
                }
                ctx.deep_link = incoming.deep_link().map(String::from);
                ctx.incoming_message_id = incoming.message_id;

                // Set context fields from incoming update
                match &incoming.kind {
                    UpdateKind::Message { text, .. } => {
                        ctx.message_text = text.clone();
                    }
                    UpdateKind::InlineQuery { id, .. } => {
                        ctx.inline_query_id = Some(id.clone());
                    }
                    UpdateKind::ChosenInlineResult {
                        result_id,
                        inline_message_id,
                        ..
                    } => {
                        ctx.chosen_inline_result_id = Some(result_id.clone());
                        if let Some(imid) = inline_message_id {
                            ctx.mode = CtxMode::Inline {
                                inline_message_id: imid.clone(),
                            };
                        }
                    }
                    UpdateKind::PreCheckoutQuery {
                        id,
                        currency,
                        total_amount,
                        payload,
                    } => {
                        ctx.payment = crate::ctx::PaymentContext {
                            query_id: Some(id.clone()),
                            payload: Some(payload.clone()),
                            currency: Some(currency.clone()),
                            total_amount: Some(*total_amount),
                        };
                    }
                    UpdateKind::SuccessfulPayment {
                        currency,
                        total_amount,
                        payload,
                    } => {
                        ctx.payment = crate::ctx::PaymentContext {
                            query_id: None,
                            payload: Some(payload.clone()),
                            currency: Some(currency.clone()),
                            total_amount: Some(*total_amount),
                        };
                    }
                    _ => {}
                }

                // Built-in: dismiss button
                if let UpdateKind::CallbackQuery {
                    data: Some(ref d), ..
                } = incoming.kind
                {
                    if d == "__dismiss" {
                        if let Some(mid) = incoming.message_id {
                            let _ = rt
                                .bot_api
                                .delete_messages(incoming.chat_id, vec![mid])
                                .await;
                            ctx.state
                                .active_bot_messages
                                .retain(|t| t.message_id != mid);
                        }
                        if let Some(cb_id) = ctx.state.pending_callback_id.take() {
                            let _ = rt.bot_api.answer_callback_query(cb_id, None, false).await;
                        }
                        return ctx.state;
                    }
                }

                let result = {
                    let handler_fut = handle_form_or_route(
                        &rt.forms,
                        &rt.conversations,
                        &rt.router,
                        &mut ctx,
                        &incoming,
                    );
                    match tokio::time::timeout(std::time::Duration::from_secs(120), handler_fut)
                        .await
                    {
                        Ok(r) => r,
                        Err(_) => {
                            tracing::error!(chat_id = chat_id.0, "handler timed out (120s)");
                            Err(HandlerError::Timeout(std::time::Duration::from_secs(120)))
                        }
                    }
                };

                if let Some(cb_id) = ctx.state.pending_callback_id.take() {
                    let _ = rt.bot_api.answer_callback_query(cb_id, None, false).await;
                }

                for mw in rt.middlewares.iter() {
                    mw.after(chat_id, &ctx.state.user, &incoming, &result).await;
                }

                if let Err(ref e) = result {
                    metrics().inc_errors();
                    tracing::error!(chat_id = chat_id.0, error = %e, "handler error");
                }
                if let Err(e) = result {
                    if let Some(ref on_err) = rt.on_error {
                        on_err(chat_id, e);
                    }
                }

                // Seal reply — next handler call's reply() will send a new message
                ctx.state.reply_sealed = true;

                ctx.state
            }
        })
        .await;
}

async fn handle_form_or_route(
    forms: &HashMap<String, Form>,
    conversations: &HashMap<String, Conversation>,
    router: &Router,
    ctx: &mut Ctx,
    update: &IncomingUpdate,
) -> HandlerResult {
    // Check conversation first
    let conv_id: Option<String> = ctx.get("__conv_id");
    if let Some(conv_id) = conv_id {
        if let Some(conv) = conversations.get(&conv_id) {
            // If the user sent a /command, the conversation clears itself
            // and returns Ok(()) — fall through to the router below.
            let was_command = matches!(&update.kind,
                UpdateKind::Message { text: Some(t) } if t.starts_with('/'));
            let result = run_conversation_step(conv, ctx, update).await;
            if !was_command {
                return result;
            }
            // /command cancelled the conversation — fall through to router
        } else {
            ctx.remove("__conv_id");
        }
    }

    // Then form
    let form_id: Option<String> = ctx.get("__form_id");
    if let Some(form_id) = form_id {
        if let Some(form) = forms.get(&form_id) {
            // Same logic: /command cancels form, fall through to router.
            let was_command = matches!(&update.kind,
                UpdateKind::Message { text: Some(t) } if t.starts_with('/'));
            let result = run_form_step(form, ctx, update).await;
            if !was_command {
                return result;
            }
        } else {
            ctx.remove("__form_id");
        }
    }
    router.route(ctx, update).await
}

async fn run_form_step(form: &Form, ctx: &mut Ctx, update: &IncomingUpdate) -> HandlerResult {
    let step_idx: usize = ctx.get("__form_step").unwrap_or(0);
    let mut form_data: FormData = ctx.get("__form_data").unwrap_or_default();

    if let Some(mid) = update.message_id {
        match &update.kind {
            UpdateKind::Message { .. } | UpdateKind::Photo { .. } => {
                ctx.state.pending_user_messages.push(mid);
            }
            _ => {}
        }
    }

    match &update.kind {
        UpdateKind::CallbackQuery {
            data: Some(data),
            id,
            ..
        } => {
            ctx.state.pending_callback_id = Some(id.clone());
            ctx.callback_data = Some(data.clone());

            if data == "__form_cancel" {
                ctx.remove("__form_id");
                ctx.remove("__form_step");
                ctx.remove("__form_data");
                if let Some(ref on_cancel) = form.on_cancel {
                    return on_cancel(ctx).await;
                }
                return Ok(());
            }

            if data.starts_with("__form_confirm:") || data.starts_with("__form_choice:") {
                let value = data.split(':').nth(1).unwrap_or("").to_string();
                if step_idx < form.steps.len() {
                    let step = &form.steps[step_idx];
                    match step.parser.validate(&value, ctx.lang()) {
                        Ok(val) => {
                            form_data.insert(step.field.clone(), val);
                            ctx.set("__form_data", &form_data);
                            return advance_form_step(form, ctx, step_idx + 1, form_data).await;
                        }
                        Err(err) => {
                            let _ = ctx.toast(format!("❌ {}", err)).await;
                            return Ok(());
                        }
                    }
                }
            }
        }

        UpdateKind::Message { text: Some(text) } => {
            if text.starts_with('/') {
                ctx.remove("__form_id");
                ctx.remove("__form_step");
                ctx.remove("__form_data");
                return Ok(());
            }
            if step_idx < form.steps.len() {
                let step = &form.steps[step_idx];
                match step.parser.validate(text, ctx.lang()) {
                    Ok(val) => {
                        form_data.insert(step.field.clone(), val);
                        ctx.set("__form_data", &form_data);
                        return advance_form_step(form, ctx, step_idx + 1, form_data).await;
                    }
                    Err(err) => {
                        if let Some(mid) = update.message_id {
                            let _ = ctx.delete_now(mid).await;
                            ctx.state.pending_user_messages.retain(|id| *id != mid);
                        }
                        let _ = ctx
                            .notify_temp(format!("❌ {}", err), std::time::Duration::from_secs(3))
                            .await;
                        return Ok(());
                    }
                }
            }
        }

        UpdateKind::Photo { file_id, .. } => {
            if step_idx < form.steps.len() {
                let step = &form.steps[step_idx];
                if matches!(step.parser, crate::form::FieldParser::Photo) {
                    form_data.insert(
                        step.field.clone(),
                        serde_json::Value::String(file_id.clone()),
                    );
                    ctx.set("__form_data", &form_data);
                    return advance_form_step(form, ctx, step_idx + 1, form_data).await;
                }
            }
        }

        _ => {}
    }
    Ok(())
}

async fn advance_form_step(
    form: &Form,
    ctx: &mut Ctx,
    next_step: usize,
    form_data: FormData,
) -> HandlerResult {
    if next_step >= form.steps.len() {
        ctx.remove("__form_id");
        ctx.remove("__form_step");
        ctx.remove("__form_data");
        // Call the completion handler with collected form data.
        return (form.on_complete)(ctx, form_data).await;
    }
    ctx.set("__form_step", &next_step);
    let lang = ctx.lang().to_string();
    let screen = (form.steps[next_step].screen_fn)(&form_data, &lang);
    ctx.navigate(screen).await
}

async fn run_conversation_step(
    conv: &Conversation,
    ctx: &mut Ctx,
    update: &IncomingUpdate,
) -> HandlerResult {
    use crate::conversation::ConversationData;

    let step_idx: usize = ctx.get("__conv_step").unwrap_or(0);
    let mut conv_data: ConversationData = ctx.get("__conv_data").unwrap_or_default();

    if let Some(mid) = update.message_id {
        match &update.kind {
            UpdateKind::Message { .. } | UpdateKind::Photo { .. } => {
                ctx.state.pending_user_messages.push(mid);
            }
            _ => {}
        }
    }

    match &update.kind {
        UpdateKind::CallbackQuery {
            data: Some(data),
            id,
            ..
        } => {
            ctx.state.pending_callback_id = Some(id.clone());
            ctx.callback_data = Some(data.clone());

            if data == "__conv_cancel" {
                ctx.remove("__conv_id");
                ctx.remove("__conv_step");
                ctx.remove("__conv_data");
                if let Some(ref on_cancel) = conv.on_cancel {
                    return on_cancel(ctx).await;
                }
                return Ok(());
            }

            // Treat callback data as input for current step
            if step_idx < conv.steps.len() {
                let step = &conv.steps[step_idx];
                if let Some(ref input_fn) = step.input_fn {
                    match input_fn(ctx, data, &conv_data).await {
                        Ok(Some(val)) => {
                            conv_data.insert(step.name.clone(), val);
                            ctx.set("__conv_data", &conv_data);
                            return advance_conversation_step(conv, ctx, step_idx, conv_data).await;
                        }
                        Ok(None) => return Ok(()), // retry
                        Err(msg) => {
                            let _ = ctx.toast(format!("❌ {msg}")).await;
                            return Ok(());
                        }
                    }
                } else {
                    // No custom input fn — store callback data as value
                    conv_data.insert(step.name.clone(), serde_json::Value::String(data.clone()));
                    ctx.set("__conv_data", &conv_data);
                    return advance_conversation_step(conv, ctx, step_idx, conv_data).await;
                }
            }
        }

        UpdateKind::Message { text: Some(text) } => {
            if text.starts_with('/') {
                ctx.remove("__conv_id");
                ctx.remove("__conv_step");
                ctx.remove("__conv_data");
                return Ok(());
            }
            if step_idx < conv.steps.len() {
                let step = &conv.steps[step_idx];
                if let Some(ref input_fn) = step.input_fn {
                    match input_fn(ctx, text, &conv_data).await {
                        Ok(Some(val)) => {
                            conv_data.insert(step.name.clone(), val);
                            ctx.set("__conv_data", &conv_data);
                            return advance_conversation_step(conv, ctx, step_idx, conv_data).await;
                        }
                        Ok(None) => return Ok(()),
                        Err(msg) => {
                            if let Some(mid) = update.message_id {
                                let _ = ctx.delete_now(mid).await;
                                ctx.state.pending_user_messages.retain(|id| *id != mid);
                            }
                            let _ = ctx
                                .notify_temp(format!("❌ {msg}"), std::time::Duration::from_secs(3))
                                .await;
                            return Ok(());
                        }
                    }
                } else {
                    // No custom input fn — store text as value
                    conv_data.insert(step.name.clone(), serde_json::Value::String(text.clone()));
                    ctx.set("__conv_data", &conv_data);
                    return advance_conversation_step(conv, ctx, step_idx, conv_data).await;
                }
            }
        }

        _ => {}
    }
    Ok(())
}

async fn advance_conversation_step(
    conv: &Conversation,
    ctx: &mut Ctx,
    current_step: usize,
    conv_data: crate::conversation::ConversationData,
) -> HandlerResult {
    match conv.next_step(current_step, &conv_data) {
        Some(next_idx) => {
            ctx.set("__conv_step", &next_idx);
            let lang = ctx.lang().to_string();
            let screen = (conv.steps[next_idx].screen_fn)(&conv_data, &lang);
            ctx.navigate(screen).await
        }
        None => {
            // Conversation complete
            ctx.remove("__conv_id");
            ctx.remove("__conv_step");
            ctx.remove("__conv_data");
            (conv.on_complete)(ctx, conv_data).await
        }
    }
}
