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

/// Central routing table that maps commands, callbacks, and inputs to handlers.
pub struct Router {
    commands: HashMap<String, BoxHandler>,
    callbacks: HashMap<String, BoxHandler>,
    callback_order: Vec<String>,
    text_inputs: HashMap<ScreenId, BoxInputHandler>,
    media_inputs: HashMap<ScreenId, BoxMediaInputHandler>,
    any_text_handler: Option<BoxTextHandler>,
    unrecognized_handler: Option<BoxHandler>,
    inline_handler: Option<BoxInlineHandler>,
    chosen_inline_handler: Option<BoxHandler>,
    message_edited_handler: Option<BoxInputHandler>,
    pre_checkout_handler: Option<BoxHandler>,
    successful_payment_handler: Option<BoxHandler>,
    member_joined_handler: Option<BoxHandler>,
    member_left_handler: Option<BoxHandler>,
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
            callback_order: Vec::new(),
            text_inputs: HashMap::new(),
            media_inputs: HashMap::new(),
            any_text_handler: None,
            unrecognized_handler: None,
            inline_handler: None,
            chosen_inline_handler: None,
            message_edited_handler: None,
            pre_checkout_handler: None,
            successful_payment_handler: None,
            member_joined_handler: None,
            member_left_handler: None,
            delete_unrecognized: true,
        }
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
        for existing in &self.callback_order {
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
        self.callback_order.push(prefix.to_string());
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

                        if let Some(handler) = self.commands.get(&cmd) {
                            return handler(ctx).await;
                        }
                    }

                    let screen_id = ctx.state.current_screen.clone();

                    if let Some(handler) = self.text_inputs.get(&screen_id) {
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

            UpdateKind::Photo {
                file_id,
                file_unique_id,
                caption,
            } => {
                let screen_id = ctx.state.current_screen.clone();
                if let Some(handler) = self.media_inputs.get(&screen_id) {
                    let media = ReceivedMedia {
                        file_id: file_id.clone(),
                        file_unique_id: file_unique_id.clone(),
                        file_type: ContentType::Photo,
                        caption: caption.clone(),
                        filename: None,
                    };
                    return handler(ctx, media).await;
                }
                self.handle_unrecognized(ctx).await
            }

            UpdateKind::Document {
                file_id,
                file_unique_id,
                filename,
                caption,
            } => {
                let screen_id = ctx.state.current_screen.clone();
                if let Some(handler) = self.media_inputs.get(&screen_id) {
                    let media = ReceivedMedia {
                        file_id: file_id.clone(),
                        file_unique_id: file_unique_id.clone(),
                        file_type: ContentType::Document,
                        caption: caption.clone(),
                        filename: filename.clone(),
                    };
                    return handler(ctx, media).await;
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

            UpdateKind::Voice {
                file_id,
                file_unique_id,
                caption,
                ..
            } => {
                let screen_id = ctx.state.current_screen.clone();
                if let Some(handler) = self.media_inputs.get(&screen_id) {
                    let media = ReceivedMedia {
                        file_id: file_id.clone(),
                        file_unique_id: file_unique_id.clone(),
                        file_type: ContentType::Voice,
                        caption: caption.clone(),
                        filename: None,
                    };
                    return handler(ctx, media).await;
                }
                self.handle_unrecognized(ctx).await
            }

            UpdateKind::VideoNote {
                file_id,
                file_unique_id,
                ..
            } => {
                let screen_id = ctx.state.current_screen.clone();
                if let Some(handler) = self.media_inputs.get(&screen_id) {
                    let media = ReceivedMedia {
                        file_id: file_id.clone(),
                        file_unique_id: file_unique_id.clone(),
                        file_type: ContentType::VideoNote,
                        caption: None,
                        filename: None,
                    };
                    return handler(ctx, media).await;
                }
                self.handle_unrecognized(ctx).await
            }

            UpdateKind::Video {
                file_id,
                file_unique_id,
                caption,
            } => {
                let screen_id = ctx.state.current_screen.clone();
                if let Some(handler) = self.media_inputs.get(&screen_id) {
                    let media = ReceivedMedia {
                        file_id: file_id.clone(),
                        file_unique_id: file_unique_id.clone(),
                        file_type: ContentType::Video,
                        caption: caption.clone(),
                        filename: None,
                    };
                    return handler(ctx, media).await;
                }
                self.handle_unrecognized(ctx).await
            }

            UpdateKind::Sticker {
                file_id,
                file_unique_id,
            } => {
                let screen_id = ctx.state.current_screen.clone();
                if let Some(handler) = self.media_inputs.get(&screen_id) {
                    let media = ReceivedMedia {
                        file_id: file_id.clone(),
                        file_unique_id: file_unique_id.clone(),
                        file_type: ContentType::Sticker,
                        caption: None,
                        filename: None,
                    };
                    return handler(ctx, media).await;
                }
                self.handle_unrecognized(ctx).await
            }

            UpdateKind::ContactReceived { .. } | UpdateKind::LocationReceived { .. } => {
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

            UpdateKind::WebAppData { .. } => Ok(()),
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
