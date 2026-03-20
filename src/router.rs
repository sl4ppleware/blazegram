//! Router — dispatches incoming updates to handlers.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::ctx::Ctx;
use crate::error::HandlerResult;
use crate::types::*;

// Handler types use boxed futures with lifetime tied to &mut Ctx.
/// Boxed async handler: `(Ctx) -> HandlerResult`.
pub type BoxHandler =
    Arc<dyn Fn(&mut Ctx) -> Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>> + Send + Sync>;

/// Boxed async text input handler: `(Ctx, String) -> HandlerResult`.
pub type BoxInputHandler = Arc<
    dyn Fn(&mut Ctx, String) -> Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>>
        + Send
        + Sync,
>;

/// Boxed async media input handler: `(Ctx, ReceivedMedia) -> HandlerResult`.
pub type BoxMediaInputHandler = Arc<
    dyn Fn(&mut Ctx, ReceivedMedia) -> Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>>
        + Send
        + Sync,
>;

/// Boxed async text handler (catch-all): `(Ctx, String) -> HandlerResult`.
pub type BoxTextHandler = Arc<
    dyn Fn(&mut Ctx, String) -> Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>>
        + Send
        + Sync,
>;

/// Boxed async inline query handler: `(Ctx, query, offset) -> HandlerResult`.
pub type BoxInlineHandler = Arc<
    dyn Fn(&mut Ctx, String, String) -> Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>>
        + Send
        + Sync,
>;

/// A group of routes with their own middleware stack.
///
/// Groups allow organizing handlers into logical modules, each with
/// independent middleware. For example, an admin group can require
/// authentication while public commands remain open.
///
/// ```rust,ignore
/// let admin = RouterGroup::new()
///     .middleware(AuthMiddleware::new(vec![ADMIN_ID]))
///     .command("ban", handler!(ctx => { /* ... */ Ok(()) }))
///     .command("stats", handler!(ctx => { /* ... */ Ok(()) }));
///
/// App::builder("TOKEN")
///     .group(admin)
///     .command("start", handler!(ctx => { /* ... */ Ok(()) }))
///     .run().await;
/// ```
pub struct RouterGroup {
    commands: HashMap<String, BoxHandler>,
    callbacks: HashMap<String, BoxHandler>,
    text_inputs: HashMap<ScreenId, BoxInputHandler>,
    media_inputs: HashMap<ScreenId, BoxMediaInputHandler>,
    middlewares: Vec<Arc<dyn crate::middleware::Middleware>>,
}

impl RouterGroup {
    /// Create a new empty router group.
    pub fn new() -> Self {
        Self {
            commands: HashMap::new(),
            callbacks: HashMap::new(),
            text_inputs: HashMap::new(),
            media_inputs: HashMap::new(),
            middlewares: Vec::new(),
        }
    }

    /// Add a middleware to this group.
    pub fn middleware(mut self, m: impl crate::middleware::Middleware + 'static) -> Self {
        self.middlewares.push(Arc::new(m));
        self
    }

    /// Register a `/command` handler in this group.
    pub fn command(
        mut self,
        name: &str,
        handler: impl Fn(&mut Ctx) -> Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        let name = name.strip_prefix('/').unwrap_or(name).to_lowercase();
        self.commands.insert(name, Arc::new(handler));
        self
    }

    /// Register a callback-query prefix handler in this group.
    pub fn callback(
        mut self,
        prefix: &str,
        handler: impl Fn(&mut Ctx) -> Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.callbacks.insert(prefix.to_string(), Arc::new(handler));
        self
    }

    /// Register a text input handler for a specific screen in this group.
    pub fn on_input(
        mut self,
        screen_id: &str,
        handler: impl Fn(&mut Ctx, String) -> Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.text_inputs
            .insert(ScreenId::from(screen_id.to_string()), Arc::new(handler));
        self
    }

    /// Register a media input handler for a specific screen in this group.
    pub fn on_media_input(
        mut self,
        screen_id: &str,
        handler: impl Fn(
            &mut Ctx,
            ReceivedMedia,
        ) -> Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.media_inputs
            .insert(ScreenId::from(screen_id.to_string()), Arc::new(handler));
        self
    }
}

impl Default for RouterGroup {
    fn default() -> Self {
        Self::new()
    }
}

/// Central routing table that maps commands, callbacks, and inputs to handlers.
pub struct Router {
    commands: HashMap<String, BoxHandler>,
    callbacks: HashMap<String, BoxHandler>,
    text_inputs: HashMap<ScreenId, BoxInputHandler>,
    media_inputs: HashMap<ScreenId, BoxMediaInputHandler>,
    web_app_data_handler: Option<BoxInputHandler>,
    any_text_handler: Option<BoxTextHandler>,
    unrecognized_handler: Option<BoxHandler>,
    inline_handler: Option<BoxInlineHandler>,
    chosen_inline_handler: Option<BoxHandler>,
    message_edited_handler: Option<BoxInputHandler>,
    pre_checkout_handler: Option<BoxHandler>,
    successful_payment_handler: Option<BoxHandler>,
    member_joined_handler: Option<BoxHandler>,
    member_left_handler: Option<BoxHandler>,
    groups: Vec<RouterGroup>,
    /// When `true` (default), unrecognized messages are silently deleted
    /// if no `on_unrecognized` handler is registered. Set to `false` to
    /// leave user messages untouched.
    pub(crate) delete_unrecognized: bool,
}

impl Router {
    /// Create an empty router.
    pub fn new() -> Self {
        Self {
            commands: HashMap::new(),
            callbacks: HashMap::new(),
            text_inputs: HashMap::new(),
            media_inputs: HashMap::new(),
            web_app_data_handler: None,
            any_text_handler: None,
            unrecognized_handler: None,
            inline_handler: None,
            chosen_inline_handler: None,
            message_edited_handler: None,
            pre_checkout_handler: None,
            successful_payment_handler: None,
            member_joined_handler: None,
            member_left_handler: None,
            groups: Vec::new(),
            delete_unrecognized: true,
        }
    }

    /// Register a [`RouterGroup`] with its own middleware stack.
    pub fn group(&mut self, group: RouterGroup) {
        self.groups.push(group);
    }

    /// Register a `/command` handler.
    pub fn command(
        &mut self,
        name: &str,
        handler: impl Fn(&mut Ctx) -> Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>>
        + Send
        + Sync
        + 'static,
    ) {
        let name = name.strip_prefix('/').unwrap_or(name).to_lowercase();
        self.commands.insert(name, Arc::new(handler));
    }

    /// Register a callback-query prefix handler.
    pub fn callback(
        &mut self,
        prefix: &str,
        handler: impl Fn(&mut Ctx) -> Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>>
        + Send
        + Sync
        + 'static,
    ) {
        // Warn about potential ambiguous callback prefixes
        for existing in self.callbacks.keys() {
            if (prefix.starts_with(existing.as_str()) || existing.starts_with(prefix))
                && prefix != existing
            {
                tracing::warn!(
                    new = prefix,
                    existing = existing.as_str(),
                    "ambiguous callback prefix — one is a prefix of the other. \
                         Use ':' as separator (e.g. 'item:123') to avoid conflicts."
                );
            }
        }
        self.callbacks.insert(prefix.to_string(), Arc::new(handler));
    }

    /// Register a text input handler for a specific screen.
    pub fn on_input(
        &mut self,
        screen_id: &str,
        handler: impl Fn(&mut Ctx, String) -> Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>>
        + Send
        + Sync
        + 'static,
    ) {
        self.text_inputs
            .insert(ScreenId::from(screen_id.to_string()), Arc::new(handler));
    }

    /// Register a media input handler for a specific screen.
    pub fn on_media_input(
        &mut self,
        screen_id: &str,
        handler: impl Fn(
            &mut Ctx,
            ReceivedMedia,
        ) -> Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>>
        + Send
        + Sync
        + 'static,
    ) {
        self.media_inputs
            .insert(ScreenId::from(screen_id.to_string()), Arc::new(handler));
    }

    /// Catch-all text handler — called for any non-command text when no screen-specific input handler matches.
    pub fn on_any_text(
        &mut self,
        handler: impl Fn(&mut Ctx, String) -> Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>>
        + Send
        + Sync
        + 'static,
    ) {
        self.any_text_handler = Some(Arc::new(handler));
    }

    /// Set the catch-all handler for unrecognized messages.
    pub fn on_unrecognized(
        &mut self,
        handler: impl Fn(&mut Ctx) -> Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>>
        + Send
        + Sync
        + 'static,
    ) {
        self.unrecognized_handler = Some(Arc::new(handler));
    }

    /// Set the inline query handler.
    pub fn on_inline(
        &mut self,
        handler: impl Fn(
            &mut Ctx,
            String,
            String,
        ) -> Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>>
        + Send
        + Sync
        + 'static,
    ) {
        self.inline_handler = Some(Arc::new(handler));
    }

    /// Set the chosen inline result handler.
    pub fn on_chosen_inline(
        &mut self,
        handler: impl Fn(&mut Ctx) -> Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>>
        + Send
        + Sync
        + 'static,
    ) {
        self.chosen_inline_handler = Some(Arc::new(handler));
    }

    /// Set the message-edited handler.
    pub fn on_message_edited(
        &mut self,
        handler: impl Fn(&mut Ctx, String) -> Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>>
        + Send
        + Sync
        + 'static,
    ) {
        self.message_edited_handler = Some(Arc::new(handler));
    }

    /// Set the pre-checkout query handler.
    pub fn on_pre_checkout(
        &mut self,
        handler: impl Fn(&mut Ctx) -> Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>>
        + Send
        + Sync
        + 'static,
    ) {
        self.pre_checkout_handler = Some(Arc::new(handler));
    }

    /// Set the successful-payment handler.
    pub fn on_successful_payment(
        &mut self,
        handler: impl Fn(&mut Ctx) -> Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>>
        + Send
        + Sync
        + 'static,
    ) {
        self.successful_payment_handler = Some(Arc::new(handler));
    }

    /// Set the new-member handler.
    pub fn on_member_joined(
        &mut self,
        handler: impl Fn(&mut Ctx) -> Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>>
        + Send
        + Sync
        + 'static,
    ) {
        self.member_joined_handler = Some(Arc::new(handler));
    }

    /// Register a handler for [`WebAppData`](UpdateKind::WebAppData) updates.
    pub fn on_web_app_data(
        &mut self,
        handler: impl Fn(&mut Ctx, String) -> Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>>
        + Send
        + Sync
        + 'static,
    ) {
        self.web_app_data_handler = Some(Arc::new(handler));
    }

    /// Set the member-left handler.
    pub fn on_member_left(
        &mut self,
        handler: impl Fn(&mut Ctx) -> Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>>
        + Send
        + Sync
        + 'static,
    ) {
        self.member_left_handler = Some(Arc::new(handler));
    }

    /// Dispatch an inline query directly (fast path, no serializer/state).
    pub async fn dispatch_inline(
        &self,
        ctx: &mut Ctx,
        query: String,
        offset: String,
    ) -> HandlerResult {
        if let Some(handler) = &self.inline_handler {
            return handler(ctx, query, offset).await;
        }
        Ok(())
    }

    // ─── Routing ───

    /// Try to match a command in groups. Returns `Some(result)` if matched.
    async fn try_group_command(
        &self,
        ctx: &mut Ctx,
        cmd: &str,
        update: &IncomingUpdate,
    ) -> Option<HandlerResult> {
        for group in &self.groups {
            if let Some(handler) = group.commands.get(cmd) {
                for mw in &group.middlewares {
                    if !mw.before(ctx.chat_id, &ctx.user, update).await {
                        return Some(Ok(()));
                    }
                }
                return Some(handler(ctx).await);
            }
        }
        None
    }

    /// Try to match a callback in groups. Returns `Some(result)` if matched.
    async fn try_group_callback(
        &self,
        ctx: &mut Ctx,
        data: &str,
        update: &IncomingUpdate,
    ) -> Option<HandlerResult> {
        for group in &self.groups {
            // Exact match
            if let Some(handler) = group.callbacks.get(data) {
                for mw in &group.middlewares {
                    if !mw.before(ctx.chat_id, &ctx.user, update).await {
                        return Some(Ok(()));
                    }
                }
                return Some(handler(ctx).await);
            }
            // Prefix match
            let mut remaining = data;
            while let Some(pos) = remaining.rfind(':') {
                remaining = &remaining[..pos];
                if let Some(handler) = group.callbacks.get(remaining) {
                    for mw in &group.middlewares {
                        if !mw.before(ctx.chat_id, &ctx.user, update).await {
                            return Some(Ok(()));
                        }
                    }
                    return Some(handler(ctx).await);
                }
            }
        }
        None
    }

    /// Try to match text input in groups. Returns `Some(result)` if matched.
    async fn try_group_text_input(
        &self,
        ctx: &mut Ctx,
        screen: &ScreenId,
        text: &str,
        update: &IncomingUpdate,
    ) -> Option<HandlerResult> {
        for group in &self.groups {
            if let Some(handler) = group.text_inputs.get(screen) {
                for mw in &group.middlewares {
                    if !mw.before(ctx.chat_id, &ctx.user, update).await {
                        return Some(Ok(()));
                    }
                }
                return Some(handler(ctx, text.to_string()).await);
            }
        }
        None
    }

    /// Try to match media input in groups. Returns `Some(result)` if matched.
    async fn try_group_media_input(
        &self,
        ctx: &mut Ctx,
        screen: &ScreenId,
        media: ReceivedMedia,
        update: &IncomingUpdate,
    ) -> Option<HandlerResult> {
        for group in &self.groups {
            if let Some(handler) = group.media_inputs.get(screen) {
                for mw in &group.middlewares {
                    if !mw.before(ctx.chat_id, &ctx.user, update).await {
                        return Some(Ok(()));
                    }
                }
                return Some(handler(ctx, media).await);
            }
        }
        None
    }

    pub(crate) async fn route(&self, ctx: &mut Ctx, update: &IncomingUpdate) -> HandlerResult {
        // Push message_id to pending_user_messages for message-type updates
        if let Some(mid) = update.message_id {
            match &update.kind {
                UpdateKind::Message { .. }
                | UpdateKind::Photo { .. }
                | UpdateKind::Document { .. }
                | UpdateKind::Voice { .. }
                | UpdateKind::VideoNote { .. }
                | UpdateKind::Video { .. }
                | UpdateKind::Sticker { .. }
                | UpdateKind::ContactReceived { .. }
                | UpdateKind::LocationReceived { .. } => {
                    ctx.state.pending_user_messages.push(mid);
                    // Cap pending user messages to prevent unbounded growth
                    const MAX_PENDING: usize = 100;
                    if ctx.state.pending_user_messages.len() > MAX_PENDING {
                        ctx.state.pending_user_messages.remove(0);
                    }
                }
                _ => {}
            }
        }

        match &update.kind {
            UpdateKind::Message { text } => {
                if let Some(text) = text {
                    if text.starts_with('/') {
                        // SAFETY: split_whitespace on non-empty always yields ≥1,
                        // strip_prefix succeeds because starts_with('/') is true,
                        // split('@') always yields ≥1 segment.
                        let cmd = text
                            .split_whitespace()
                            .next()
                            .expect("non-empty text has at least one word")
                            .strip_prefix('/')
                            .expect("starts_with('/') guarantees prefix")
                            .split('@')
                            .next()
                            .expect("split always yields at least one segment")
                            .to_lowercase();

                        if let Some(result) = self.try_group_command(ctx, &cmd, update).await {
                            return result;
                        }
                        if let Some(handler) = self.commands.get(&cmd) {
                            return handler(ctx).await;
                        }
                    }

                    if let Some(result) = self
                        .try_group_text_input(ctx, &ctx.state.current_screen.clone(), text, update)
                        .await
                    {
                        return result;
                    }
                    if let Some(handler) = self.text_inputs.get(&ctx.state.current_screen) {
                        return handler(ctx, text.clone()).await;
                    }

                    if let Some(handler) = &self.any_text_handler {
                        return handler(ctx, text.clone()).await;
                    }
                }

                self.handle_unrecognized(ctx).await
            }

            UpdateKind::CallbackQuery { id, data, .. } => {
                ctx.state.pending_callback_id = Some(id.clone());

                if let Some(data) = data {
                    ctx.callback_data = Some(data.clone());

                    // Check groups first
                    if let Some(result) = self.try_group_callback(ctx, data, update).await {
                        return result;
                    }

                    // O(1) lookup: try exact match, then progressively shorter prefixes
                    if let Some(handler) = self.callbacks.get(data.as_str()) {
                        return handler(ctx).await;
                    }
                    // Walk colons right-to-left: "a:b:c" → try "a:b", then "a"
                    let mut remaining = data.as_str();
                    while let Some(pos) = remaining.rfind(':') {
                        remaining = &remaining[..pos];
                        if let Some(handler) = self.callbacks.get(remaining) {
                            return handler(ctx).await;
                        }
                    }
                }

                Ok(())
            }

            UpdateKind::Photo { .. }
            | UpdateKind::Document { .. }
            | UpdateKind::Voice { .. }
            | UpdateKind::VideoNote { .. }
            | UpdateKind::Video { .. }
            | UpdateKind::Sticker { .. } => {
                let screen = ctx.state.current_screen.clone();
                if let Some(media) = update.kind.to_received_media() {
                    if let Some(result) = self
                        .try_group_media_input(ctx, &screen, media.clone(), update)
                        .await
                    {
                        return result;
                    }
                    if let Some(handler) = self.media_inputs.get(&screen) {
                        return handler(ctx, media).await;
                    }
                }
                self.handle_unrecognized(ctx).await
            }

            UpdateKind::InlineQuery { query, offset, .. } => {
                if let Some(handler) = &self.inline_handler {
                    return handler(ctx, query.clone(), offset.clone()).await;
                }
                Ok(())
            }

            UpdateKind::ChosenInlineResult {
                inline_message_id, ..
            } => {
                if let Some(imid) = inline_message_id {
                    ctx.mode = CtxMode::Inline {
                        inline_message_id: imid.clone(),
                    };
                }
                if let Some(handler) = &self.chosen_inline_handler {
                    return handler(ctx).await;
                }
                Ok(())
            }

            UpdateKind::MessageEdited { text } => {
                if let Some(handler) = &self.message_edited_handler {
                    return handler(ctx, text.clone().unwrap_or_default()).await;
                }
                Ok(())
            }

            UpdateKind::PreCheckoutQuery { .. } => {
                if let Some(handler) = &self.pre_checkout_handler {
                    return handler(ctx).await;
                }
                Ok(())
            }

            UpdateKind::SuccessfulPayment { .. } => {
                if let Some(handler) = &self.successful_payment_handler {
                    return handler(ctx).await;
                }
                Ok(())
            }

            UpdateKind::ContactReceived { .. } | UpdateKind::LocationReceived { .. } => {
                let screen = ctx.state.current_screen.clone();
                if let Some(media) = update.kind.to_received_media() {
                    if let Some(result) = self
                        .try_group_media_input(ctx, &screen, media.clone(), update)
                        .await
                    {
                        return result;
                    }
                    if let Some(handler) = self.media_inputs.get(&screen) {
                        return handler(ctx, media).await;
                    }
                }
                self.handle_unrecognized(ctx).await
            }

            UpdateKind::ChatMemberJoined => {
                if let Some(handler) = &self.member_joined_handler {
                    return handler(ctx).await;
                }
                Ok(())
            }

            UpdateKind::ChatMemberLeft => {
                if let Some(handler) = &self.member_left_handler {
                    return handler(ctx).await;
                }
                Ok(())
            }

            UpdateKind::WebAppData { data } => {
                if let Some(handler) = &self.web_app_data_handler {
                    return handler(ctx, data.clone()).await;
                }
                Ok(())
            }
        }
    }

    async fn handle_unrecognized(&self, ctx: &mut Ctx) -> HandlerResult {
        if let Some(handler) = &self.unrecognized_handler {
            handler(ctx).await
        } else if self.delete_unrecognized {
            // Default: silently delete the unrecognized message to keep chat clean.
            // Disable with `.delete_unrecognized(false)` on AppBuilder.
            if let Some(&msg_id) = ctx.state.pending_user_messages.last() {
                let _ = ctx.delete_now(msg_id).await;
                ctx.state.pending_user_messages.retain(|id| *id != msg_id);
            }
            Ok(())
        } else {
            Ok(())
        }
    }
}

impl Default for Router {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bot_api::BotApi;
    use crate::mock::MockBotApi;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    fn test_user() -> UserInfo {
        UserInfo {
            id: UserId(1),
            first_name: "Test".into(),
            last_name: None,
            username: None,
            language_code: Some("en".into()),
        }
    }

    fn make_ctx() -> Ctx {
        let state = ChatState::new(ChatId(1), test_user());
        let bot: Arc<dyn BotApi> = Arc::new(MockBotApi::new());
        Ctx::new(state, bot, None)
    }

    fn make_update_text(text: &str) -> IncomingUpdate {
        IncomingUpdate {
            chat_id: ChatId(1),
            user: test_user(),
            message_id: Some(MessageId(100)),
            kind: UpdateKind::Message {
                text: Some(text.to_string()),
            },
        }
    }

    fn make_update_callback(data: &str) -> IncomingUpdate {
        IncomingUpdate {
            chat_id: ChatId(1),
            user: test_user(),
            message_id: Some(MessageId(100)),
            kind: UpdateKind::CallbackQuery {
                id: "cb_1".into(),
                data: Some(data.to_string()),
                inline_message_id: None,
            },
        }
    }

    fn make_update_photo() -> IncomingUpdate {
        IncomingUpdate {
            chat_id: ChatId(1),
            user: test_user(),
            message_id: Some(MessageId(100)),
            kind: UpdateKind::Photo {
                file_id: "photo_123".into(),
                file_unique_id: "uniq_123".into(),
                caption: None,
            },
        }
    }

    /// Shared flag for tracking whether a handler was invoked.
    fn handler_flag() -> Arc<AtomicBool> {
        Arc::new(AtomicBool::new(false))
    }

    // ─── Command registration & dispatch ───

    #[tokio::test]
    async fn command_dispatch_start() {
        let called = handler_flag();
        let called2 = called.clone();
        let mut router = Router::new();
        router.command("start", move |_ctx: &mut Ctx| {
            let c = called2.clone();
            Box::pin(async move {
                c.store(true, Ordering::SeqCst);
                Ok(())
            })
        });
        let mut ctx = make_ctx();
        let update = make_update_text("/start");
        router.route(&mut ctx, &update).await.unwrap();
        assert!(
            called.load(Ordering::SeqCst),
            "handler should have been called"
        );
    }

    #[tokio::test]
    async fn command_strips_slash_and_lowercases() {
        let called = handler_flag();
        let called2 = called.clone();
        let mut router = Router::new();
        router.command("/Help", move |_ctx: &mut Ctx| {
            let c = called2.clone();
            Box::pin(async move {
                c.store(true, Ordering::SeqCst);
                Ok(())
            })
        });
        let mut ctx = make_ctx();
        let update = make_update_text("/HELP");
        router.route(&mut ctx, &update).await.unwrap();
        assert!(called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn command_strips_bot_mention() {
        let called = handler_flag();
        let called2 = called.clone();
        let mut router = Router::new();
        router.command("start", move |_ctx: &mut Ctx| {
            let c = called2.clone();
            Box::pin(async move {
                c.store(true, Ordering::SeqCst);
                Ok(())
            })
        });
        let mut ctx = make_ctx();
        let update = make_update_text("/start@MyBot");
        router.route(&mut ctx, &update).await.unwrap();
        assert!(called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn command_with_args_dispatches() {
        let called = handler_flag();
        let called2 = called.clone();
        let mut router = Router::new();
        router.command("start", move |_ctx: &mut Ctx| {
            let c = called2.clone();
            Box::pin(async move {
                c.store(true, Ordering::SeqCst);
                Ok(())
            })
        });
        let mut ctx = make_ctx();
        let update = make_update_text("/start deep_link_payload");
        router.route(&mut ctx, &update).await.unwrap();
        assert!(called.load(Ordering::SeqCst));
    }

    // ─── Callback routing ───

    #[tokio::test]
    async fn callback_exact_match() {
        let called = handler_flag();
        let called2 = called.clone();
        let mut router = Router::new();
        router.callback("action", move |_ctx: &mut Ctx| {
            let c = called2.clone();
            Box::pin(async move {
                c.store(true, Ordering::SeqCst);
                Ok(())
            })
        });
        let mut ctx = make_ctx();
        let update = make_update_callback("action");
        router.route(&mut ctx, &update).await.unwrap();
        assert!(called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn callback_prefix_match_with_colon() {
        let called = handler_flag();
        let called2 = called.clone();
        let mut router = Router::new();
        router.callback("action", move |_ctx: &mut Ctx| {
            let c = called2.clone();
            Box::pin(async move {
                c.store(true, Ordering::SeqCst);
                Ok(())
            })
        });
        let mut ctx = make_ctx();
        let update = make_update_callback("action:view:42");
        router.route(&mut ctx, &update).await.unwrap();
        assert!(called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn callback_longest_prefix_wins() {
        let short = handler_flag();
        let long = handler_flag();
        let short2 = short.clone();
        let long2 = long.clone();

        let mut router = Router::new();
        router.callback("a", move |_ctx: &mut Ctx| {
            let c = short2.clone();
            Box::pin(async move {
                c.store(true, Ordering::SeqCst);
                Ok(())
            })
        });
        router.callback("a:b", move |_ctx: &mut Ctx| {
            let c = long2.clone();
            Box::pin(async move {
                c.store(true, Ordering::SeqCst);
                Ok(())
            })
        });

        let mut ctx = make_ctx();
        // "a:b:c" should match "a:b" (longest prefix), not "a"
        let update = make_update_callback("a:b:c");
        router.route(&mut ctx, &update).await.unwrap();
        assert!(long.load(Ordering::SeqCst), "longer prefix should match");
        assert!(
            !short.load(Ordering::SeqCst),
            "shorter prefix should NOT match"
        );
    }

    #[tokio::test]
    async fn callback_no_match_is_ok() {
        let router = Router::new();
        let mut ctx = make_ctx();
        let update = make_update_callback("unknown:action");
        let result = router.route(&mut ctx, &update).await;
        assert!(result.is_ok(), "unmatched callback should not error");
    }

    #[tokio::test]
    async fn callback_sets_callback_data_on_ctx() {
        let mut router = Router::new();
        router.callback("pick", move |ctx: &mut Ctx| {
            Box::pin(async move {
                assert_eq!(ctx.callback_data(), Some("pick:dark"));
                assert_eq!(ctx.callback_params(), vec!["dark"]);
                Ok(())
            })
        });
        let mut ctx = make_ctx();
        let update = make_update_callback("pick:dark");
        router.route(&mut ctx, &update).await.unwrap();
    }

    // ─── Text input routing ───

    #[tokio::test]
    async fn text_input_for_current_screen() {
        let called = handler_flag();
        let called2 = called.clone();
        let mut router = Router::new();
        router.on_input("ask_name", move |_ctx: &mut Ctx, text: String| {
            let c = called2.clone();
            Box::pin(async move {
                assert_eq!(text, "Alice");
                c.store(true, Ordering::SeqCst);
                Ok(())
            })
        });

        let mut ctx = make_ctx();
        ctx.state.current_screen = ScreenId::from("ask_name".to_string());
        let update = make_update_text("Alice");
        router.route(&mut ctx, &update).await.unwrap();
        assert!(called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn text_input_wrong_screen_goes_to_unrecognized() {
        let input_called = handler_flag();
        let input_called2 = input_called.clone();
        let mut router = Router::new();
        router.on_input("ask_name", move |_ctx: &mut Ctx, _text: String| {
            let c = input_called2.clone();
            Box::pin(async move {
                c.store(true, Ordering::SeqCst);
                Ok(())
            })
        });
        router.delete_unrecognized = false;

        let mut ctx = make_ctx();
        ctx.state.current_screen = ScreenId::from("other_screen".to_string());
        let update = make_update_text("Alice");
        router.route(&mut ctx, &update).await.unwrap();
        assert!(
            !input_called.load(Ordering::SeqCst),
            "input handler should NOT run for wrong screen"
        );
    }

    // ─── Any-text handler ───

    #[tokio::test]
    async fn any_text_handler_catches_non_command() {
        let called = handler_flag();
        let called2 = called.clone();
        let mut router = Router::new();
        router.on_any_text(move |_ctx: &mut Ctx, text: String| {
            let c = called2.clone();
            Box::pin(async move {
                assert_eq!(text, "hello world");
                c.store(true, Ordering::SeqCst);
                Ok(())
            })
        });
        let mut ctx = make_ctx();
        let update = make_update_text("hello world");
        router.route(&mut ctx, &update).await.unwrap();
        assert!(called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn screen_input_takes_priority_over_any_text() {
        let input_called = handler_flag();
        let any_called = handler_flag();
        let input2 = input_called.clone();
        let any2 = any_called.clone();

        let mut router = Router::new();
        router.on_input("ask", move |_ctx: &mut Ctx, _text: String| {
            let c = input2.clone();
            Box::pin(async move {
                c.store(true, Ordering::SeqCst);
                Ok(())
            })
        });
        router.on_any_text(move |_ctx: &mut Ctx, _text: String| {
            let c = any2.clone();
            Box::pin(async move {
                c.store(true, Ordering::SeqCst);
                Ok(())
            })
        });

        let mut ctx = make_ctx();
        ctx.state.current_screen = ScreenId::from("ask".to_string());
        let update = make_update_text("some text");
        router.route(&mut ctx, &update).await.unwrap();
        assert!(
            input_called.load(Ordering::SeqCst),
            "screen input should win"
        );
        assert!(
            !any_called.load(Ordering::SeqCst),
            "any_text should NOT be called"
        );
    }

    // ─── Unrecognized ───

    #[tokio::test]
    async fn unrecognized_handler_called_for_no_match() {
        let called = handler_flag();
        let called2 = called.clone();
        let mut router = Router::new();
        router.on_unrecognized(move |_ctx: &mut Ctx| {
            let c = called2.clone();
            Box::pin(async move {
                c.store(true, Ordering::SeqCst);
                Ok(())
            })
        });
        let mut ctx = make_ctx();
        let update = make_update_text("random text");
        router.route(&mut ctx, &update).await.unwrap();
        assert!(called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn delete_unrecognized_removes_from_pending() {
        let router = Router::new(); // delete_unrecognized = true by default
        let mut ctx = make_ctx();
        let update = make_update_text("random junk");
        router.route(&mut ctx, &update).await.unwrap();
        // delete_unrecognized calls delete_now and removes from pending
        assert!(
            !ctx.state.pending_user_messages.contains(&MessageId(100)),
            "deleted message should be removed from pending"
        );
    }

    #[tokio::test]
    async fn delete_unrecognized_false_keeps_message() {
        let mut router = Router::new();
        router.delete_unrecognized = false;
        let mut ctx = make_ctx();
        let update = make_update_text("random text");
        router.route(&mut ctx, &update).await.unwrap();
        // With delete_unrecognized=false, message stays in pending
        assert!(
            ctx.state.pending_user_messages.contains(&MessageId(100)),
            "message should remain in pending when flag is false"
        );
    }

    // ─── Media input routing ───

    #[tokio::test]
    async fn media_input_dispatches_for_screen() {
        let called = handler_flag();
        let called2 = called.clone();
        let mut router = Router::new();
        router.on_media_input("upload", move |_ctx: &mut Ctx, media: ReceivedMedia| {
            let c = called2.clone();
            Box::pin(async move {
                assert_eq!(media.file_id, "photo_123");
                c.store(true, Ordering::SeqCst);
                Ok(())
            })
        });

        let mut ctx = make_ctx();
        ctx.state.current_screen = ScreenId::from("upload".to_string());
        let update = make_update_photo();
        router.route(&mut ctx, &update).await.unwrap();
        assert!(called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn media_input_wrong_screen_goes_to_unrecognized() {
        let called = handler_flag();
        let called2 = called.clone();
        let mut router = Router::new();
        router.on_media_input("upload", move |_ctx: &mut Ctx, _media: ReceivedMedia| {
            let c = called2.clone();
            Box::pin(async move {
                c.store(true, Ordering::SeqCst);
                Ok(())
            })
        });
        router.delete_unrecognized = false;

        let mut ctx = make_ctx();
        ctx.state.current_screen = ScreenId::from("other".to_string());
        let update = make_update_photo();
        router.route(&mut ctx, &update).await.unwrap();
        assert!(!called.load(Ordering::SeqCst));
    }

    // ─── Message pending tracking ───

    #[tokio::test]
    async fn message_id_pushed_to_pending() {
        let mut router = Router::new();
        router.delete_unrecognized = false;
        let mut ctx = make_ctx();
        let update = make_update_text("hello");
        router.route(&mut ctx, &update).await.unwrap();
        assert!(ctx.state.pending_user_messages.contains(&MessageId(100)));
    }

    // ─── Inline dispatch ───

    #[tokio::test]
    async fn inline_dispatch_calls_handler() {
        let called = handler_flag();
        let called2 = called.clone();
        let mut router = Router::new();
        router.on_inline(move |_ctx: &mut Ctx, query: String, _offset: String| {
            let c = called2.clone();
            Box::pin(async move {
                assert_eq!(query, "search term");
                c.store(true, Ordering::SeqCst);
                Ok(())
            })
        });
        let mut ctx = make_ctx();
        router
            .dispatch_inline(&mut ctx, "search term".into(), "".into())
            .await
            .unwrap();
        assert!(called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn inline_dispatch_no_handler_is_ok() {
        let router = Router::new();
        let mut ctx = make_ctx();
        let result = router
            .dispatch_inline(&mut ctx, "q".into(), "".into())
            .await;
        assert!(result.is_ok());
    }

    // ─── Member joined/left ───

    #[tokio::test]
    async fn member_joined_dispatch() {
        let called = handler_flag();
        let called2 = called.clone();
        let mut router = Router::new();
        router.on_member_joined(move |_ctx: &mut Ctx| {
            let c = called2.clone();
            Box::pin(async move {
                c.store(true, Ordering::SeqCst);
                Ok(())
            })
        });
        let mut ctx = make_ctx();
        let update = IncomingUpdate {
            chat_id: ChatId(1),
            user: test_user(),
            message_id: None,
            kind: UpdateKind::ChatMemberJoined,
        };
        router.route(&mut ctx, &update).await.unwrap();
        assert!(called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn member_left_dispatch() {
        let called = handler_flag();
        let called2 = called.clone();
        let mut router = Router::new();
        router.on_member_left(move |_ctx: &mut Ctx| {
            let c = called2.clone();
            Box::pin(async move {
                c.store(true, Ordering::SeqCst);
                Ok(())
            })
        });
        let mut ctx = make_ctx();
        let update = IncomingUpdate {
            chat_id: ChatId(1),
            user: test_user(),
            message_id: None,
            kind: UpdateKind::ChatMemberLeft,
        };
        router.route(&mut ctx, &update).await.unwrap();
        assert!(called.load(Ordering::SeqCst));
    }

    // ─── Pre-checkout / payment ───

    #[tokio::test]
    async fn pre_checkout_dispatch() {
        let called = handler_flag();
        let called2 = called.clone();
        let mut router = Router::new();
        router.on_pre_checkout(move |_ctx: &mut Ctx| {
            let c = called2.clone();
            Box::pin(async move {
                c.store(true, Ordering::SeqCst);
                Ok(())
            })
        });
        let mut ctx = make_ctx();
        let update = IncomingUpdate {
            chat_id: ChatId(1),
            user: test_user(),
            message_id: None,
            kind: UpdateKind::PreCheckoutQuery {
                id: "pq_1".into(),
                currency: "XTR".into(),
                total_amount: 100,
                payload: "test".into(),
            },
        };
        router.route(&mut ctx, &update).await.unwrap();
        assert!(called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn successful_payment_dispatch() {
        let called = handler_flag();
        let called2 = called.clone();
        let mut router = Router::new();
        router.on_successful_payment(move |_ctx: &mut Ctx| {
            let c = called2.clone();
            Box::pin(async move {
                c.store(true, Ordering::SeqCst);
                Ok(())
            })
        });
        let mut ctx = make_ctx();
        let update = IncomingUpdate {
            chat_id: ChatId(1),
            user: test_user(),
            message_id: None,
            kind: UpdateKind::SuccessfulPayment {
                currency: "XTR".into(),
                total_amount: 100,
                payload: "test".into(),
            },
        };
        router.route(&mut ctx, &update).await.unwrap();
        assert!(called.load(Ordering::SeqCst));
    }

    // ─── Message edited ───

    #[tokio::test]
    async fn message_edited_dispatch() {
        let called = handler_flag();
        let called2 = called.clone();
        let mut router = Router::new();
        router.on_message_edited(move |_ctx: &mut Ctx, text: String| {
            let c = called2.clone();
            Box::pin(async move {
                assert_eq!(text, "edited text");
                c.store(true, Ordering::SeqCst);
                Ok(())
            })
        });
        let mut ctx = make_ctx();
        let update = IncomingUpdate {
            chat_id: ChatId(1),
            user: test_user(),
            message_id: Some(MessageId(100)),
            kind: UpdateKind::MessageEdited {
                text: Some("edited text".into()),
            },
        };
        router.route(&mut ctx, &update).await.unwrap();
        assert!(called.load(Ordering::SeqCst));
    }

    // ─── Chosen inline result ───

    #[tokio::test]
    async fn chosen_inline_result_dispatch() {
        let called = handler_flag();
        let called2 = called.clone();
        let mut router = Router::new();
        router.on_chosen_inline(move |_ctx: &mut Ctx| {
            let c = called2.clone();
            Box::pin(async move {
                c.store(true, Ordering::SeqCst);
                Ok(())
            })
        });
        let mut ctx = make_ctx();
        let update = IncomingUpdate {
            chat_id: ChatId(1),
            user: test_user(),
            message_id: None,
            kind: UpdateKind::ChosenInlineResult {
                result_id: "r_1".into(),
                inline_message_id: Some("im_1".into()),
                query: "q".into(),
            },
        };
        router.route(&mut ctx, &update).await.unwrap();
        assert!(called.load(Ordering::SeqCst));
        assert_eq!(
            ctx.mode,
            CtxMode::Inline {
                inline_message_id: "im_1".into()
            }
        );
    }

    // ─── Router groups ───

    #[tokio::test]
    async fn group_command_dispatches() {
        let called = handler_flag();
        let c = called.clone();
        let mut router = Router::new();
        router.group(RouterGroup::new().command("admin", move |_ctx: &mut Ctx| {
            let c = c.clone();
            Box::pin(async move {
                c.store(true, Ordering::SeqCst);
                Ok(())
            })
        }));
        let mut ctx = make_ctx();
        let update = make_update_text("/admin");
        router.route(&mut ctx, &update).await.unwrap();
        assert!(called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn group_callback_dispatches() {
        let called = handler_flag();
        let c = called.clone();
        let mut router = Router::new();
        router.group(RouterGroup::new().callback("grp", move |_ctx: &mut Ctx| {
            let c = c.clone();
            Box::pin(async move {
                c.store(true, Ordering::SeqCst);
                Ok(())
            })
        }));
        let mut ctx = make_ctx();
        let update = make_update_callback("grp:42");
        router.route(&mut ctx, &update).await.unwrap();
        assert!(called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn group_middleware_blocks() {
        let handler_called = handler_flag();
        let hc = handler_called.clone();
        let mut router = Router::new();
        // AuthMiddleware only allows user 999, our test_user is 42
        router.group(
            RouterGroup::new()
                .middleware(crate::middleware::AuthMiddleware::new(vec![999]))
                .command("secret", move |_ctx: &mut Ctx| {
                    let c = hc.clone();
                    Box::pin(async move {
                        c.store(true, Ordering::SeqCst);
                        Ok(())
                    })
                }),
        );
        let mut ctx = make_ctx();
        let update = make_update_text("/secret");
        router.route(&mut ctx, &update).await.unwrap();
        assert!(
            !handler_called.load(Ordering::SeqCst),
            "middleware should have blocked"
        );
    }

    #[tokio::test]
    async fn group_middleware_allows() {
        let handler_called = handler_flag();
        let hc = handler_called.clone();
        let mut router = Router::new();
        // test_user is UserId(1)
        router.group(
            RouterGroup::new()
                .middleware(crate::middleware::AuthMiddleware::new(vec![1]))
                .command("secret", move |_ctx: &mut Ctx| {
                    let c = hc.clone();
                    Box::pin(async move {
                        c.store(true, Ordering::SeqCst);
                        Ok(())
                    })
                }),
        );
        let mut ctx = make_ctx();
        let update = make_update_text("/secret");
        router.route(&mut ctx, &update).await.unwrap();
        assert!(handler_called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn group_text_input_dispatches() {
        let called = handler_flag();
        let c = called.clone();
        let mut router = Router::new();
        router.group(RouterGroup::new().on_input(
            "grp_screen",
            move |_ctx: &mut Ctx, _text: String| {
                let c = c.clone();
                Box::pin(async move {
                    c.store(true, Ordering::SeqCst);
                    Ok(())
                })
            },
        ));
        let mut ctx = make_ctx();
        ctx.state.current_screen = ScreenId::from("grp_screen");
        let update = make_update_text("hello");
        router.route(&mut ctx, &update).await.unwrap();
        assert!(called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn group_media_input_dispatches() {
        let called = handler_flag();
        let c = called.clone();
        let mut router = Router::new();
        router.group(RouterGroup::new().on_media_input(
            "grp_media",
            move |_ctx: &mut Ctx, _media: ReceivedMedia| {
                let c = c.clone();
                Box::pin(async move {
                    c.store(true, Ordering::SeqCst);
                    Ok(())
                })
            },
        ));
        let mut ctx = make_ctx();
        ctx.state.current_screen = ScreenId::from("grp_media");
        let update = make_update_photo();
        router.route(&mut ctx, &update).await.unwrap();
        assert!(called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn group_falls_through_to_main_router() {
        let main_called = handler_flag();
        let mc = main_called.clone();
        let mut router = Router::new();
        router.group(
            RouterGroup::new().command("admin", |_ctx: &mut Ctx| Box::pin(async move { Ok(()) })),
        );
        router.command("help", move |_ctx: &mut Ctx| {
            let c = mc.clone();
            Box::pin(async move {
                c.store(true, Ordering::SeqCst);
                Ok(())
            })
        });
        let mut ctx = make_ctx();
        let update = make_update_text("/help");
        router.route(&mut ctx, &update).await.unwrap();
        assert!(main_called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn group_takes_priority_over_main_router() {
        let group_called = handler_flag();
        let gc = group_called.clone();
        let main_called = handler_flag();
        let mc = main_called.clone();
        let mut router = Router::new();
        router.group(RouterGroup::new().command("start", move |_ctx: &mut Ctx| {
            let c = gc.clone();
            Box::pin(async move {
                c.store(true, Ordering::SeqCst);
                Ok(())
            })
        }));
        router.command("start", move |_ctx: &mut Ctx| {
            let c = mc.clone();
            Box::pin(async move {
                c.store(true, Ordering::SeqCst);
                Ok(())
            })
        });
        let mut ctx = make_ctx();
        let update = make_update_text("/start");
        router.route(&mut ctx, &update).await.unwrap();
        assert!(group_called.load(Ordering::SeqCst), "group should win");
        assert!(
            !main_called.load(Ordering::SeqCst),
            "main should NOT be called"
        );
    }

    #[tokio::test]
    async fn router_group_default_impl() {
        let group = RouterGroup::default();
        assert!(group.commands.is_empty());
        assert!(group.middlewares.is_empty());
    }

    // ─── WebAppData handler ───

    #[tokio::test]
    async fn web_app_data_dispatches() {
        let called = handler_flag();
        let called2 = called.clone();
        let mut router = Router::new();
        router.on_web_app_data(move |_ctx: &mut Ctx, data: String| {
            let c = called2.clone();
            Box::pin(async move {
                assert_eq!(data, "payload123");
                c.store(true, Ordering::SeqCst);
                Ok(())
            })
        });
        let mut ctx = make_ctx();
        let update = IncomingUpdate {
            chat_id: ChatId(1),
            user: test_user(),
            message_id: None,
            kind: UpdateKind::WebAppData {
                data: "payload123".into(),
            },
        };
        router.route(&mut ctx, &update).await.unwrap();
        assert!(called.load(Ordering::SeqCst));
    }

    // ─── Pending user messages cap ───

    #[tokio::test]
    async fn pending_user_messages_capped_at_100() {
        let mut router = Router::new();
        router.delete_unrecognized = false;
        let mut ctx = make_ctx();
        // Pre-fill with 100 messages
        for i in 0..100 {
            ctx.state.pending_user_messages.push(MessageId(i));
        }
        assert_eq!(ctx.state.pending_user_messages.len(), 100);
        let update = IncomingUpdate {
            chat_id: ChatId(1),
            user: test_user(),
            message_id: Some(MessageId(999)),
            kind: UpdateKind::Message {
                text: Some("hello".into()),
            },
        };
        router.route(&mut ctx, &update).await.unwrap();
        // Should still be 100, oldest evicted
        assert_eq!(ctx.state.pending_user_messages.len(), 100);
        assert!(!ctx.state.pending_user_messages.contains(&MessageId(0)));
        assert!(ctx.state.pending_user_messages.contains(&MessageId(999)));
    }

    // ─── Default (no handler) ───

    #[tokio::test]
    async fn default_router_handles_all_update_kinds_without_panic() {
        let router = Router::new();
        let kinds = vec![
            UpdateKind::ChatMemberJoined,
            UpdateKind::ChatMemberLeft,
            UpdateKind::MessageEdited { text: None },
            UpdateKind::WebAppData {
                data: "test".into(),
            },
        ];
        for kind in kinds {
            let mut ctx = make_ctx();
            let update = IncomingUpdate {
                chat_id: ChatId(1),
                user: test_user(),
                message_id: None,
                kind,
            };
            router.route(&mut ctx, &update).await.unwrap();
        }
    }
}
