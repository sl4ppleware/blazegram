//! App — the main entry point for building a Blazegram bot.
//!
//! Uses grammers (pure Rust MTProto) for direct connection to Telegram DC.

use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;

use grammers_client::{
    Client,
    client::UpdatesConfiguration,
    update::Update,
    tl,
};
use grammers_mtsender::SenderPool;
use grammers_session::storages::SqliteSession;
use grammers_session::types::PeerRef;

use crate::bot_api::BotApi;
use crate::ctx::Ctx;
use crate::error::{HandlerError, HandlerResult};
use crate::form::{Form, FormData};
use crate::grammers_adapter::{GrammersAdapter, DEFAULT_API_ID, DEFAULT_API_HASH};
use crate::i18n::{self, I18n};
use crate::metrics::metrics;
use crate::middleware::Middleware;
use crate::router::Router;
use crate::serializer::ChatSerializer;
use crate::state::{InMemoryStore, StateStore};
use crate::types::*;

pub struct App;

pub struct AppBuilder {
    token: String,
    api_id: i32,
    api_hash: String,
    session_file: String,
    router: Router,
    store: Option<Arc<dyn StateStore>>,
    middlewares: Vec<Arc<dyn Middleware>>,
    forms: HashMap<String, Form>,
    rate_limit_rps: Option<u32>,
    on_error: Option<Arc<ErrorHandler>>,
    snapshot_path: Option<String>,
    snapshot_interval: std::time::Duration,
}

type ErrorHandler = dyn Fn(ChatId, HandlerError) + Send + Sync;

impl App {
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
            rate_limit_rps: None,
            on_error: None,
            snapshot_path: None,
            snapshot_interval: std::time::Duration::from_secs(300),
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

    pub fn store(mut self, store: impl StateStore + 'static) -> Self {
        self.store = Some(Arc::new(store));
        self
    }

    pub fn middleware(mut self, m: impl Middleware + 'static) -> Self {
        self.middlewares.push(Arc::new(m));
        self
    }

    pub fn command(
        mut self, name: &str,
        handler: impl Fn(&mut Ctx) -> std::pin::Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>> + Send + Sync + 'static,
    ) -> Self {
        self.router.command(name, handler);
        self
    }

    pub fn callback(
        mut self, prefix: &str,
        handler: impl Fn(&mut Ctx) -> std::pin::Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>> + Send + Sync + 'static,
    ) -> Self {
        self.router.callback(prefix, handler);
        self
    }

    pub fn on_input(
        mut self, screen_id: &str,
        handler: impl Fn(&mut Ctx, String) -> std::pin::Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>> + Send + Sync + 'static,
    ) -> Self {
        self.router.on_input(screen_id, handler);
        self
    }

    pub fn on_media_input(
        mut self, screen_id: &str,
        handler: impl Fn(&mut Ctx, ReceivedMedia) -> std::pin::Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>> + Send + Sync + 'static,
    ) -> Self {
        self.router.on_media_input(screen_id, handler);
        self
    }

    /// Catch-all handler for any text message not matched by screen-specific input handlers.
    pub fn on_any_text(
        mut self,
        handler: impl Fn(&mut Ctx, String) -> std::pin::Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>> + Send + Sync + 'static,
    ) -> Self {
        self.router.on_any_text(handler);
        self
    }

    pub fn on_unrecognized(
        mut self,
        handler: impl Fn(&mut Ctx) -> std::pin::Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>> + Send + Sync + 'static,
    ) -> Self {
        self.router.on_unrecognized(handler);
        self
    }

    pub fn on_inline(
        mut self,
        handler: impl Fn(&mut Ctx, String, String) -> std::pin::Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>> + Send + Sync + 'static,
    ) -> Self {
        self.router.on_inline(handler);
        self
    }

    pub fn on_chosen_inline(
        mut self,
        handler: impl Fn(&mut Ctx) -> std::pin::Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>> + Send + Sync + 'static,
    ) -> Self {
        self.router.on_chosen_inline(handler);
        self
    }

    pub fn on_message_edited(
        mut self,
        handler: impl Fn(&mut Ctx, String) -> std::pin::Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>> + Send + Sync + 'static,
    ) -> Self {
        self.router.on_message_edited(handler);
        self
    }

    /// Handler for pre-checkout queries (payment flow).
    /// The handler should call `ctx.bot().answer_pre_checkout_query()` to approve/decline.
    pub fn on_pre_checkout(
        mut self,
        handler: impl Fn(&mut Ctx) -> std::pin::Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>> + Send + Sync + 'static,
    ) -> Self {
        self.router.on_pre_checkout(handler);
        self
    }

    /// Handler for successful payments.
    pub fn on_successful_payment(
        mut self,
        handler: impl Fn(&mut Ctx) -> std::pin::Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>> + Send + Sync + 'static,
    ) -> Self {
        self.router.on_successful_payment(handler);
        self
    }

    /// Handler for new members joining the chat.
    pub fn on_member_joined(
        mut self,
        handler: impl Fn(&mut Ctx) -> std::pin::Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>> + Send + Sync + 'static,
    ) -> Self {
        self.router.on_member_joined(handler);
        self
    }

    /// Handler for members leaving the chat.
    pub fn on_member_left(
        mut self,
        handler: impl Fn(&mut Ctx) -> std::pin::Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>> + Send + Sync + 'static,
    ) -> Self {
        self.router.on_member_left(handler);
        self
    }

    pub fn form(mut self, form: Form) -> Self {
        self.forms.insert(form.id.clone(), form);
        self
    }

    pub fn rate_limit(mut self, rps: u32) -> Self {
        self.rate_limit_rps = Some(rps);
        self
    }

    pub fn i18n(self, i: I18n) -> Self {
        i18n::set_i18n(i);
        self
    }

    pub fn locales(self, dir: &str, default_lang: &str) -> Self {
        let i = I18n::load(dir, default_lang).expect("failed to load locales");
        i18n::set_i18n(i);
        self
    }

    #[cfg(feature = "redis")]
    pub fn redis_store(self, url: &str) -> Self {
        let store = crate::redis_store::RedisStore::new(url)
            .expect("failed to connect to Redis");
        self.store(store)
    }

    pub fn sqlite_store(self, path: &str) -> Self {
        let store = crate::sqlite_store::SqliteStore::open(path)
            .expect("failed to open SQLite store");
        self.store(store)
    }

    pub fn on_error(mut self, handler: impl Fn(ChatId, HandlerError) + Send + Sync + 'static) -> Self {
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

    /// Build and run the bot. Connects via MTProto, streams updates.
    pub async fn run(self) {
        // Build store. If snapshot is enabled and no custom store, use InMemoryStore with snapshots.
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
        let on_error = self.on_error;

        tracing::info!("Blazegram: connecting via MTProto...");

        // ── Create grammers session & client ──
        let session = Arc::new(
            SqliteSession::open(&self.session_file)
                .await
                .expect("failed to open session file"),
        );

        let SenderPool { runner, updates, handle } =
            SenderPool::new(Arc::clone(&session), self.api_id);
        let client = Client::new(handle.clone());

        // Spawn the sender pool runner
        let pool_task = tokio::spawn(runner.run());

        // ── Bot sign-in ──
        if !client.is_authorized().await.expect("auth check failed") {
            tracing::info!("Signing in as bot...");
            client
                .bot_sign_in(&self.token, &self.api_hash)
                .await
                .expect("bot sign-in failed");
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
                if let Ok(peers) = bincode::deserialize::<Vec<(i64, i64, i64)>>(&bytes) {
                    let count = peers.len();
                    adapter.import_peers(&peers);
                    tracing::info!(count, "Restored peer cache from disk");
                }
            }
        }
        let bot_api: Arc<dyn BotApi> = if let Some(rps) = self.rate_limit_rps {
            Arc::new(crate::rate_limiter::RateLimitedBotApi::new(adapter.clone(), rps))
        } else {
            Arc::new(adapter.clone())
        };

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
        if let (Some(ref mem_store), Some(ref snap_path)) = (&snapshot_store, &self.snapshot_path) {
            mem_store.start_snapshot_task(snap_path.clone(), self.snapshot_interval);
            tracing::info!(interval_secs = self.snapshot_interval.as_secs(), "Snapshot task started");
        }

        // ── Stream updates ──
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

        let mut sigterm = tokio::signal::unix::signal(
            tokio::signal::unix::SignalKind::terminate()
        ).expect("failed to register SIGTERM");

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
                        if matches!(&incoming, IncomingUpdate::InlineQuery { .. } | IncomingUpdate::ChosenInlineResult { .. }) {
                            let bot = bot_api.clone();
                            let router = router.clone();
                            tokio::spawn(async move {
                                handle_inline_fast(incoming, bot, router).await;
                            });
                            continue;
                        }

                        let bot = bot_api.clone();
                        let router = router.clone();
                        let serializer = serializer.clone();
                        let middlewares = middlewares.clone();
                        let forms = forms.clone();
                        let gc = client.clone();
                        let pc = adapter.peer_cache();
                        let on_err = on_error.clone();

                        tokio::spawn(async move {
                            process_update(incoming, bot, router, serializer, middlewares, forms, gc, pc, on_err).await;
                        });
                    }
                }
            }
        }

        // ── Graceful shutdown ──
        tracing::info!("Syncing update state...");
        update_stream.sync_update_state().await;

        // Final snapshot before exit
        if let (Some(ref mem_store), Some(ref snap_path)) = (&snapshot_store, &self.snapshot_path) {
            tracing::info!("Saving final snapshot...");
            if let Err(e) = mem_store.snapshot(snap_path).await {
                tracing::error!(error = %e, "Failed to save final snapshot");
            } else {
                tracing::info!(chats = mem_store.len(), "Snapshot saved");
            }
            // Persist peer cache alongside snapshot
            let peers_path = format!("{}.peers", snap_path);
            let peers = adapter.export_peers();
            if let Ok(bytes) = bincode::serialize(&peers) {
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

// ── Update conversion ──

async fn convert_update(update: &Update) -> Option<(IncomingUpdate, PeerRef)> {
    match update {
        Update::NewMessage(msg) => {
            if msg.outgoing() { return None; }
            let sender = msg.sender()?;
            let peer_ref = msg.peer_ref().await?;
            let user = user_from_peer(sender);
            let chat_id = ChatId(peer_ref.id.bot_api_dialog_id());
            let message_id = MessageId(msg.id());

            // Check media types via raw TL
            let raw_msg = &msg.raw;
            if let tl::enums::Update::NewMessage(tl::types::UpdateNewMessage { message, .. })
                | tl::enums::Update::NewChannelMessage(tl::types::UpdateNewChannelMessage { message, .. }) = raw_msg
            {
                if let tl::enums::Message::Message(m) = message {
                    // Photo
                    if let Some(tl::enums::MessageMedia::Photo(photo)) = &m.media {
                        if let Some(tl::enums::Photo::Photo(p)) = &photo.photo {
                            return Some((
                                IncomingUpdate::Photo {
                                    message_id, chat_id, user,
                                    file_id: p.id.to_string(),
                                    file_unique_id: p.id.to_string(),
                                    caption: if m.message.is_empty() { None } else { Some(m.message.clone()) },
                                },
                                peer_ref,
                            ));
                        }
                    }
                    // Document
                    if let Some(tl::enums::MessageMedia::Document(doc)) = &m.media {
                        if let Some(tl::enums::Document::Document(d)) = &doc.document {
                            let filename = d.attributes.iter().find_map(|a| {
                                if let tl::enums::DocumentAttribute::Filename(f) = a {
                                    Some(f.file_name.clone())
                                } else { None }
                            });
                            return Some((
                                IncomingUpdate::Document {
                                    message_id, chat_id, user,
                                    file_id: d.id.to_string(),
                                    file_unique_id: d.id.to_string(),
                                    filename,
                                    caption: if m.message.is_empty() { None } else { Some(m.message.clone()) },
                                },
                                peer_ref,
                            ));
                        }
                    }
                }
            }

            // Text message (default)
            let text = {
                let t = msg.text().to_string();
                if t.is_empty() { None } else { Some(t) }
            };
            Some((IncomingUpdate::Message { message_id, chat_id, user, text }, peer_ref))
        }

        Update::CallbackQuery(query) => {
            let peer_ref = query.peer_ref().await?;
            let sender = query.sender()?;
            let user = user_from_peer(sender);
            let chat_id = ChatId(peer_ref.id.bot_api_dialog_id());

            let query_id = match &query.raw {
                tl::enums::Update::BotCallbackQuery(u) => u.query_id.to_string(),
                tl::enums::Update::InlineBotCallbackQuery(u) => u.query_id.to_string(),
                _ => return None,
            };

            let msg_id = match &query.raw {
                tl::enums::Update::BotCallbackQuery(u) => Some(MessageId(u.msg_id)),
                _ => None,
            };

            // Extract inline_message_id for callbacks on inline messages
            let inline_msg_id = match &query.raw {
                tl::enums::Update::InlineBotCallbackQuery(u) => {
                    // Serialize the InputBotInlineMessageId to base64 for later use
                    use grammers_tl_types::Serializable;
                    let mut buf = Vec::new();
                    u.msg_id.serialize(&mut buf);
                    Some(data_encoding::BASE64URL_NOPAD.encode(&buf))
                }
                _ => None,
            };

            let data = {
                let bytes = query.data();
                if bytes.is_empty() { None } else { String::from_utf8(bytes.to_vec()).ok() }
            };

            Some((
                IncomingUpdate::CallbackQuery { id: query_id, chat_id, user, data, message_id: msg_id, inline_message_id: inline_msg_id },
                peer_ref,
            ))
        }

        Update::InlineQuery(query) => {
            tracing::debug!(query_text = %query.text(), "received inline query from grammers");
            // Build user info — sender() may return None if peer not in cache
            let user = match query.sender() {
                Some(s) => user_from_grammers_user(s),
                None => {
                    tracing::debug!("inline query: sender not cached, using raw user_id");
                    UserInfo {
                        id: UserId(query.sender_id().bare_id() as u64),
                        first_name: String::new(),
                        last_name: None,
                        username: None,
                        language_code: None,
                    }
                }
            };
            let (id, q, offset) = match &query.raw {
                tl::enums::Update::BotInlineQuery(u) => {
                    (u.query_id.to_string(), u.query.clone(), u.offset.clone())
                }
                _ => return None,
            };
            // peer_ref may also be None — for inline we don't strictly need it
            let peer_ref = match query.sender_ref().await {
                Some(pr) => pr,
                None => {
                    tracing::debug!("inline query: no peer_ref, using dummy");
                    // Return without peer_ref — inline fast path doesn't need peer cache
                    return Some((
                        IncomingUpdate::InlineQuery { id, user, query: q, offset },
                        // Minimal PeerRef — inline fast path doesn't use peer cache
                        PeerRef {
                            id: query.sender_id(),
                            auth: grammers_session::types::PeerAuth::from_hash(0),
                        },
                    ));
                }
            };
            Some((
                IncomingUpdate::InlineQuery { id, user, query: q, offset },
                peer_ref,
            ))
        }

        Update::MessageEdited(msg) => {
            if msg.outgoing() { return None; }
            let sender = msg.sender()?;
            let peer_ref = msg.peer_ref().await?;
            let user = user_from_peer(sender);
            let chat_id = ChatId(peer_ref.id.bot_api_dialog_id());
            let message_id = MessageId(msg.id());
            let text = {
                let t = msg.text().to_string();
                if t.is_empty() { None } else { Some(t) }
            };
            Some((IncomingUpdate::MessageEdited { message_id, chat_id, user, text }, peer_ref))
        }

        Update::InlineSend(inline_send) => {
            let user = match inline_send.sender() {
                Some(s) => user_from_grammers_user(s),
                None => UserInfo {
                    id: UserId(inline_send.sender_id().bare_id() as u64),
                    first_name: String::new(),
                    last_name: None,
                    username: None,
                    language_code: None,
                },
            };
            let result_id = inline_send.result_id().to_string();
            let query = inline_send.text().to_string();
            // Extract inline_message_id as base64-encoded bytes
            let inline_message_id = inline_send.message_id().map(|id| {
                use grammers_client::tl;
                match id {
                    tl::enums::InputBotInlineMessageId::Id64(id64) => {
                        let mut bytes = Vec::with_capacity(24);
                        bytes.extend_from_slice(&id64.dc_id.to_le_bytes());
                        bytes.extend_from_slice(&id64.owner_id.to_le_bytes());
                        bytes.extend_from_slice(&id64.id.to_le_bytes());
                        bytes.extend_from_slice(&id64.access_hash.to_le_bytes());
                        data_encoding::BASE64URL_NOPAD.encode(&bytes)
                    }
                    tl::enums::InputBotInlineMessageId::Id(id) => {
                        let mut bytes = Vec::with_capacity(20);
                        bytes.extend_from_slice(&id.dc_id.to_le_bytes());
                        bytes.extend_from_slice(&id.id.to_le_bytes());
                        bytes.extend_from_slice(&id.access_hash.to_le_bytes());
                        data_encoding::BASE64URL_NOPAD.encode(&bytes)
                    }
                }
            });
            let peer_ref = match inline_send.sender_ref().await {
                Some(pr) => pr,
                None => PeerRef {
                    id: inline_send.sender_id(),
                    auth: grammers_session::types::PeerAuth::from_hash(0),
                },
            };
            Some((
                IncomingUpdate::ChosenInlineResult {
                    result_id,
                    user,
                    inline_message_id,
                    query,
                },
                peer_ref,
            ))
        }

        _ => None,
    }
}

fn user_from_peer(peer: &grammers_client::peer::Peer) -> UserInfo {
    use grammers_client::peer::Peer;
    match peer {
        Peer::User(u) => user_from_grammers_user(u),
        _ => UserInfo {
            id: UserId(peer.id().bare_id() as u64),
            first_name: peer.name().unwrap_or_default().to_string(),
            last_name: None,
            username: peer.username().map(String::from),
            language_code: None,
        },
    }
}

fn user_from_grammers_user(u: &grammers_client::peer::User) -> UserInfo {
    UserInfo {
        id: UserId(u.id().bare_id() as u64),
        first_name: u.first_name().unwrap_or_default().to_string(),
        last_name: u.last_name().map(String::from),
        username: u.username().map(String::from),
        language_code: None, // MTProto doesn't provide lang in updates
    }
}

// ── Inline query: fast path (no chat state, no serializer) ──

async fn handle_inline_fast(
    incoming: IncomingUpdate,
    bot_api: Arc<dyn BotApi>,
    router: Arc<Router>,
) {
    let user = incoming.user().clone();
    let dummy_state = ChatState::new(ChatId(user.id.0 as i64), user.clone());
    let mut ctx = Ctx::new(dummy_state, bot_api.clone(), None);

    match &incoming {
        IncomingUpdate::InlineQuery { query, offset, id, .. } => {
            ctx.inline_query_id = Some(id.clone());
            tracing::debug!(query_id = %id, query = %query, "dispatching inline query to handler");
            match router.dispatch_inline(&mut ctx, query.clone(), offset.clone()).await {
                Ok(()) => tracing::debug!("inline query handler completed OK"),
                Err(e) => tracing::error!(error = %e, "inline query handler error"),
            }
        }
        IncomingUpdate::ChosenInlineResult { result_id, inline_message_id, .. } => {
            ctx.chosen_inline_result_id = Some(result_id.clone());
            if let Some(ref imid) = inline_message_id {
                ctx.mode = CtxMode::Inline { inline_message_id: imid.clone() };
            }
            if let Err(e) = router.route(&mut ctx, &incoming).await {
                tracing::error!(error = %e, "chosen inline result handler error");
            }
        }
        _ => {}
    }
}

// ── Process update ──

#[tracing::instrument(skip_all, fields(chat_id = %incoming.chat_id().0))]
#[allow(clippy::too_many_arguments)]
async fn process_update(
    incoming: IncomingUpdate,
    bot_api: Arc<dyn BotApi>,
    router: Arc<Router>,
    serializer: Arc<ChatSerializer>,
    middlewares: Arc<Vec<Arc<dyn Middleware>>>,
    forms: Arc<HashMap<String, Form>>,
    grammers_client: grammers_client::Client,
    peer_cache: Arc<dashmap::DashMap<i64, grammers_session::types::PeerRef>>,
    on_error: Option<Arc<ErrorHandler>>,
) {
    metrics().inc_updates();
    let _timer = metrics().timer("update");

    let chat_id = incoming.chat_id();
    let user = incoming.user().clone();

    // (inline queries use user_id as pseudo chat_id)

    for mw in middlewares.iter() {
        if !mw.before(chat_id, &user, &incoming).await { return; }
    }

    serializer.serialize(chat_id, &user, |state| {
        let router = router.clone();
        let bot = bot_api.clone();
        let forms = forms.clone();
        let incoming = incoming.clone();
        let middlewares = middlewares.clone();
        let on_error = on_error.clone();

        async move {
            let callback_data = match &incoming {
                IncomingUpdate::CallbackQuery { data, .. } => data.clone(),
                _ => None,
            };

            let mut ctx = Ctx::new(state, bot.clone(), callback_data);
            ctx.grammers_client = Some(grammers_client.clone());
            ctx.peer_cache = Some(peer_cache.clone());

            // Determine CtxMode
            let cid = incoming.chat_id();
            // Check for inline callback first (callback on an inline message)
            if let IncomingUpdate::CallbackQuery { inline_message_id: Some(ref imid), .. } = incoming {
                ctx.mode = CtxMode::Inline { inline_message_id: imid.clone() };
                tracing::debug!(imid = %imid, "inline callback detected");
            } else if cid.0 < 0 {
                // Group/supergroup/channel
                let trigger = match &incoming {
                    IncomingUpdate::CallbackQuery { message_id, .. } => *message_id,
                    _ => None,
                };
                ctx.mode = CtxMode::Group { trigger_message_id: trigger };
            }

            if let IncomingUpdate::CallbackQuery { id, .. } = &incoming {
                ctx.state.pending_callback_id = Some(id.clone());
            }
            ctx.deep_link = incoming.deep_link().map(String::from);

            // Set context fields from incoming update
            match &incoming {
                IncomingUpdate::Message { text, message_id, .. } => {
                    ctx.message_text = text.clone();
                    ctx.incoming_message_id = Some(*message_id);
                }
                IncomingUpdate::CallbackQuery { message_id, .. } => {
                    ctx.incoming_message_id = *message_id;
                }
                IncomingUpdate::Photo { message_id, .. }
                | IncomingUpdate::Document { message_id, .. }
                | IncomingUpdate::MessageEdited { message_id, .. } => {
                    ctx.incoming_message_id = Some(*message_id);
                }
                IncomingUpdate::InlineQuery { id, .. } => {
                    ctx.inline_query_id = Some(id.clone());
                }
                IncomingUpdate::ChosenInlineResult { result_id, inline_message_id, .. } => {
                    ctx.chosen_inline_result_id = Some(result_id.clone());
                    if let Some(ref imid) = inline_message_id {
                        ctx.mode = CtxMode::Inline { inline_message_id: imid.clone() };
                    }
                }
                IncomingUpdate::PreCheckoutQuery { id, currency, total_amount, payload, .. } => {
                    ctx.pre_checkout_query_id = Some(id.clone());
                    ctx.payment_payload = Some(payload.clone());
                    ctx.payment_currency = Some(currency.clone());
                    ctx.payment_total_amount = Some(*total_amount);
                }
                IncomingUpdate::SuccessfulPayment { currency, total_amount, payload, .. } => {
                    ctx.payment_payload = Some(payload.clone());
                    ctx.payment_currency = Some(currency.clone());
                    ctx.payment_total_amount = Some(*total_amount);
                }
                _ => {}
            }

            // Built-in: dismiss button
            if let IncomingUpdate::CallbackQuery { data: Some(ref d), .. } = incoming {
                if d == "__dismiss" {
                    if let IncomingUpdate::CallbackQuery { message_id: Some(mid), .. } = &incoming {
                        let _ = bot.delete_messages(incoming.chat_id(), vec![*mid]).await;
                        ctx.state.active_bot_messages.retain(|t| t.message_id != *mid);
                    }
                    if let Some(cb_id) = ctx.state.pending_callback_id.take() {
                        let _ = bot.answer_callback_query(cb_id, None, false).await;
                    }
                    return ctx.state;
                }
            }

            let result = {
                let handler_fut = handle_form_or_route(&forms, &router, &mut ctx, &incoming);
                match tokio::time::timeout(std::time::Duration::from_secs(120), handler_fut).await {
                    Ok(r) => r,
                    Err(_) => {
                        tracing::error!(chat_id = chat_id.0, "handler timed out (120s)");
                        Err(HandlerError::Internal(anyhow::anyhow!("handler timed out")))
                    }
                }
            };

            if let Some(cb_id) = ctx.state.pending_callback_id.take() {
                let _ = bot.answer_callback_query(cb_id, None, false).await;
            }

            for mw in middlewares.iter() {
                mw.after(chat_id, &ctx.state.user, &incoming, &result).await;
            }

            if let Err(ref e) = result {
                metrics().inc_errors();
                tracing::error!(chat_id = chat_id.0, error = %e, "handler error");
            }
            if let Err(e) = result {
                if let Some(ref on_err) = on_error {
                    on_err(chat_id, e);
                }
            }

            // Seal reply — next handler call's reply() will send a new message
            ctx.state.reply_sealed = true;

            ctx.state
        }
    }).await;
}

async fn handle_form_or_route(
    forms: &HashMap<String, Form>,
    router: &Router,
    ctx: &mut Ctx,
    update: &IncomingUpdate,
) -> HandlerResult {
    let form_id: Option<String> = ctx.get("__form_id");
    if let Some(form_id) = form_id {
        if let Some(form) = forms.get(&form_id) {
            return run_form_step(form, ctx, update).await;
        } else {
            ctx.remove("__form_id");
        }
    }
    router.route(ctx, update).await
}

async fn run_form_step(
    form: &Form, ctx: &mut Ctx, update: &IncomingUpdate,
) -> HandlerResult {
    let step_idx: usize = ctx.get("__form_step").unwrap_or(0);
    let mut form_data: FormData = ctx.get("__form_data").unwrap_or_default();

    match update {
        IncomingUpdate::CallbackQuery { data: Some(data), id, .. } => {
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

        IncomingUpdate::Message { text: Some(text), message_id, .. } => {
            ctx.state.pending_user_messages.push(*message_id);
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
                        let _ = ctx.delete_now(*message_id).await;
                        ctx.state.pending_user_messages.retain(|id| id != message_id);
                        let _ = ctx.notify_temp(
                            format!("❌ {}", err),
                            std::time::Duration::from_secs(3),
                        ).await;
                        return Ok(());
                    }
                }
            }
        }

        IncomingUpdate::Photo { message_id, file_id, .. } => {
            ctx.state.pending_user_messages.push(*message_id);
            if step_idx < form.steps.len() {
                let step = &form.steps[step_idx];
                if matches!(step.parser, crate::form::FieldParser::Photo) {
                    form_data.insert(step.field.clone(), serde_json::Value::String(file_id.clone()));
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
    form: &Form, ctx: &mut Ctx, next_step: usize, form_data: FormData,
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

impl Ctx {
    pub async fn start_form(&mut self, form_id: &str, forms: &HashMap<String, Form>) -> HandlerResult {
        let form = forms.get(form_id).ok_or_else(|| {
            HandlerError::Internal(anyhow::anyhow!("form '{}' not found", form_id))
        })?;
        self.set("__form_id", &form_id.to_string());
        self.set("__form_step", &0usize);
        self.set("__form_data", &FormData::new());
        let data = FormData::new();
        let lang = self.lang().to_string();
        let screen = (form.steps[0].screen_fn)(&data, &lang);
        self.navigate(screen).await
    }
}
