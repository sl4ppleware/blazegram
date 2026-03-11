//! Router — dispatches incoming updates to handlers.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::ctx::Ctx;
use crate::error::HandlerResult;
use crate::types::*;

// Handler types use boxed futures with lifetime tied to &mut Ctx.
pub type BoxHandler = Arc<
    dyn Fn(&mut Ctx) -> Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>>
        + Send
        + Sync,
>;

pub type BoxInputHandler = Arc<
    dyn Fn(&mut Ctx, String) -> Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>>
        + Send
        + Sync,
>;

pub type BoxMediaInputHandler = Arc<
    dyn Fn(&mut Ctx, ReceivedMedia) -> Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>>
        + Send
        + Sync,
>;

pub type BoxTextHandler = Arc<
    dyn Fn(&mut Ctx, String) -> Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>>
        + Send
        + Sync,
>;

pub type BoxInlineHandler = Arc<
    dyn Fn(&mut Ctx, String, String) -> Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>>
        + Send
        + Sync,
>;

pub struct Router {
    commands: HashMap<String, BoxHandler>,
    callbacks: Vec<(String, BoxHandler)>,
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
}

impl Router {
    pub fn new() -> Self {
        Self {
            commands: HashMap::new(),
            callbacks: Vec::new(),
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
        }
    }

    pub fn command(
        &mut self,
        name: &str,
        handler: impl Fn(&mut Ctx) -> Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>> + Send + Sync + 'static,
    ) {
        let name = name.strip_prefix('/').unwrap_or(name).to_lowercase();
        self.commands.insert(name, Arc::new(handler));
    }

    pub fn callback(
        &mut self,
        prefix: &str,
        handler: impl Fn(&mut Ctx) -> Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>> + Send + Sync + 'static,
    ) {
        // Warn about potential ambiguous callback prefixes
        for (existing, _) in &self.callbacks {
            if (prefix.starts_with(existing) || existing.starts_with(prefix))
                && prefix != existing {
                    tracing::warn!(
                        new = prefix,
                        existing = existing.as_str(),
                        "ambiguous callback prefix — one is a prefix of the other. \
                         Use ':' as separator (e.g. 'item:123') to avoid conflicts."
                    );
                }
        }
        self.callbacks.push((prefix.to_string(), Arc::new(handler)));
    }

    pub fn on_input(
        &mut self,
        screen_id: &str,
        handler: impl Fn(&mut Ctx, String) -> Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>> + Send + Sync + 'static,
    ) {
        self.text_inputs.insert(ScreenId::from(screen_id.to_string()), Arc::new(handler));
    }

    pub fn on_media_input(
        &mut self,
        screen_id: &str,
        handler: impl Fn(&mut Ctx, ReceivedMedia) -> Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>> + Send + Sync + 'static,
    ) {
        self.media_inputs.insert(ScreenId::from(screen_id.to_string()), Arc::new(handler));
    }

    /// Catch-all text handler — called for any non-command text when no screen-specific input handler matches.
    pub fn on_any_text(
        &mut self,
        handler: impl Fn(&mut Ctx, String) -> Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>> + Send + Sync + 'static,
    ) {
        self.any_text_handler = Some(Arc::new(handler));
    }

    pub fn on_unrecognized(
        &mut self,
        handler: impl Fn(&mut Ctx) -> Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>> + Send + Sync + 'static,
    ) {
        self.unrecognized_handler = Some(Arc::new(handler));
    }

    pub fn on_inline(
        &mut self,
        handler: impl Fn(&mut Ctx, String, String) -> Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>> + Send + Sync + 'static,
    ) {
        self.inline_handler = Some(Arc::new(handler));
    }

    pub fn on_chosen_inline(
        &mut self,
        handler: impl Fn(&mut Ctx) -> Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>> + Send + Sync + 'static,
    ) {
        self.chosen_inline_handler = Some(Arc::new(handler));
    }

    pub fn on_message_edited(
        &mut self,
        handler: impl Fn(&mut Ctx, String) -> Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>> + Send + Sync + 'static,
    ) {
        self.message_edited_handler = Some(Arc::new(handler));
    }

    pub fn on_pre_checkout(
        &mut self,
        handler: impl Fn(&mut Ctx) -> Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>> + Send + Sync + 'static,
    ) {
        self.pre_checkout_handler = Some(Arc::new(handler));
    }

    pub fn on_successful_payment(
        &mut self,
        handler: impl Fn(&mut Ctx) -> Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>> + Send + Sync + 'static,
    ) {
        self.successful_payment_handler = Some(Arc::new(handler));
    }

    pub fn on_member_joined(
        &mut self,
        handler: impl Fn(&mut Ctx) -> Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>> + Send + Sync + 'static,
    ) {
        self.member_joined_handler = Some(Arc::new(handler));
    }

    pub fn on_member_left(
        &mut self,
        handler: impl Fn(&mut Ctx) -> Pin<Box<dyn Future<Output = HandlerResult> + Send + '_>> + Send + Sync + 'static,
    ) {
        self.member_left_handler = Some(Arc::new(handler));
    }

    /// Dispatch an inline query directly (fast path, no serializer/state).
    pub async fn dispatch_inline(
        &self, ctx: &mut Ctx, query: String, offset: String,
    ) -> HandlerResult {
        if let Some(handler) = &self.inline_handler {
            return handler(ctx, query, offset).await;
        }
        Ok(())
    }

    // ─── Routing ───

    pub(crate) async fn route(
        &self,
        ctx: &mut Ctx,
        update: &IncomingUpdate,
    ) -> HandlerResult {
        match update {
            IncomingUpdate::Message { text, message_id, .. } => {
                ctx.state.pending_user_messages.push(*message_id);

                if let Some(text) = text {
                    // Check command
                    if text.starts_with('/') {
                        let cmd = text
                            .split_whitespace()
                            .next()
                            .unwrap()
                            .strip_prefix('/')
                            .unwrap()
                            .split('@')
                            .next()
                            .unwrap()
                            .to_lowercase();

                        if let Some(handler) = self.commands.get(&cmd) {
                            return handler(ctx).await;
                        }
                    }

                    // Check text input handler for current screen
                    let screen_id = ctx.state.current_screen.clone();

                    if let Some(handler) = self.text_inputs.get(&screen_id) {
                        return handler(ctx, text.clone()).await;
                    }

                    // Catch-all text handler (any screen)
                    if let Some(handler) = &self.any_text_handler {
                        return handler(ctx, text.clone()).await;
                    }
                }

                self.handle_unrecognized(ctx).await
            }

            IncomingUpdate::CallbackQuery { id, data, .. } => {
                ctx.state.pending_callback_id = Some(id.clone());

                if let Some(data) = data {
                    ctx.callback_data = Some(data.clone());

                    for (prefix, handler) in &self.callbacks {
                        if data == prefix || data.starts_with(&format!("{}:", prefix)) {
                            let result = handler(ctx).await;
                            return result;
                        }
                    }
                }

                // Unknown callback — answer via pending_callback_id (app.rs will handle it)
                Ok(())
            }

            IncomingUpdate::Photo { message_id, file_id, file_unique_id, caption, .. } => {
                ctx.state.pending_user_messages.push(*message_id);
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

            IncomingUpdate::Document { message_id, file_id, file_unique_id, filename, caption, .. } => {
                ctx.state.pending_user_messages.push(*message_id);
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

            IncomingUpdate::InlineQuery { query, offset, .. } => {
                if let Some(handler) = &self.inline_handler {
                    return handler(ctx, query.clone(), offset.clone()).await;
                }
                Ok(())
            }

            IncomingUpdate::ChosenInlineResult { inline_message_id, .. } => {
                // Set CtxMode::Inline if we have an inline_message_id
                if let Some(ref imid) = inline_message_id {
                    ctx.mode = CtxMode::Inline { inline_message_id: imid.clone() };
                }
                if let Some(handler) = &self.chosen_inline_handler {
                    return handler(ctx).await;
                }
                Ok(())
            }

            IncomingUpdate::MessageEdited { text, .. } => {
                if let Some(handler) = &self.message_edited_handler {
                    return handler(ctx, text.clone().unwrap_or_default()).await;
                }
                Ok(())
            }

            IncomingUpdate::PreCheckoutQuery { .. } => {
                if let Some(handler) = &self.pre_checkout_handler {
                    return handler(ctx).await;
                }
                Ok(())
            }

            IncomingUpdate::SuccessfulPayment { .. } => {
                if let Some(handler) = &self.successful_payment_handler {
                    return handler(ctx).await;
                }
                Ok(())
            }

            IncomingUpdate::Voice { message_id, file_id, file_unique_id, caption, .. } => {
                ctx.state.pending_user_messages.push(*message_id);
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

            IncomingUpdate::VideoNote { message_id, file_id, file_unique_id, .. } => {
                ctx.state.pending_user_messages.push(*message_id);
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

            IncomingUpdate::Video { message_id, file_id, file_unique_id, caption, .. } => {
                ctx.state.pending_user_messages.push(*message_id);
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

            IncomingUpdate::Sticker { message_id, file_id, file_unique_id, .. } => {
                ctx.state.pending_user_messages.push(*message_id);
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

            IncomingUpdate::ContactReceived { message_id, .. }
            | IncomingUpdate::LocationReceived { message_id, .. } => {
                ctx.state.pending_user_messages.push(*message_id);
                self.handle_unrecognized(ctx).await
            }

            IncomingUpdate::ChatMemberJoined { .. } => {
                if let Some(handler) = &self.member_joined_handler {
                    return handler(ctx).await;
                }
                Ok(())
            }

            IncomingUpdate::ChatMemberLeft { .. } => {
                if let Some(handler) = &self.member_left_handler {
                    return handler(ctx).await;
                }
                Ok(())
            }

            IncomingUpdate::WebAppData { .. } => {
                Ok(())
            }
        }
    }

    async fn handle_unrecognized(&self, ctx: &mut Ctx) -> HandlerResult {
        if let Some(handler) = &self.unrecognized_handler {
            handler(ctx).await
        } else {
            // Default: silently delete the junk message
            if let Some(&msg_id) = ctx.state.pending_user_messages.last() {
                let _ = ctx.delete_now(msg_id).await;
                ctx.state.pending_user_messages.retain(|id| *id != msg_id);
            }
            Ok(())
        }
    }
}

impl Default for Router {
    fn default() -> Self {
        Self::new()
    }
}
