//! Ctx — handler context. The single object handlers interact with.
//!
//! Works in all modes: private chat (full differ), group (inline edit), inline message.

use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use crate::bot_api::{BotApi, SendOptions};
use crate::differ::Differ;
use crate::error::{HandlerError, HandlerResult};
use crate::executor::DiffExecutor;
use crate::form::{Form, FormData};
use crate::i18n;
use crate::screen::Screen;
use crate::types::*;

/// Payment context fields, populated only for payment-related updates.
#[derive(Debug, Clone, Default)]
pub struct PaymentContext {
    /// Pre-checkout query ID.
    pub query_id: Option<String>,
    /// Invoice payload string.
    pub payload: Option<String>,
    /// Currency code (e.g. "USD", "XTR" for Stars).
    pub currency: Option<String>,
    /// Total amount in smallest units (cents, stars, etc.).
    pub total_amount: Option<i64>,
}

/// The handler context — your main interface to Telegram inside a handler.
pub struct Ctx {
    pub(crate) state: ChatState,
    pub(crate) bot: Arc<dyn BotApi>,
    pub(crate) callback_data: Option<String>,
    pub(crate) deep_link: Option<String>,
    pub(crate) message_text: Option<String>,
    pub(crate) inline_query_id: Option<String>,
    pub(crate) chosen_inline_result_id: Option<String>,
    pub(crate) incoming_message_id: Option<MessageId>,

    /// Payment context (only populated for payment updates).
    pub payment: PaymentContext,

    /// How this Ctx operates (auto-detected from update source).
    pub mode: CtxMode,

    /// Chat this handler is running for.
    pub chat_id: ChatId,
    /// User who triggered the update.
    pub user: UserInfo,

    /// Raw grammers client (for direct MTProto access).
    pub(crate) grammers_client: Option<grammers_client::Client>,
    /// Shared peer cache from GrammersAdapter.
    pub(crate) peer_cache:
        Option<std::sync::Arc<dashmap::DashMap<i64, grammers_session::types::PeerRef>>>,

    /// Abort handle for active progressive task (if any).
    pub(crate) progressive_abort: Option<tokio::task::AbortHandle>,

    /// Maximum number of keys allowed in `state.data`.
    pub(crate) max_state_keys: usize,
}

impl Ctx {
    pub(crate) fn new(
        state: ChatState,
        bot: Arc<dyn BotApi>,
        callback_data: Option<String>,
    ) -> Self {
        let chat_id = state.chat_id;
        let user = state.user.clone();
        Self {
            state,
            bot,
            callback_data,
            deep_link: None,
            message_text: None,
            inline_query_id: None,
            chosen_inline_result_id: None,
            incoming_message_id: None,
            payment: PaymentContext::default(),
            mode: CtxMode::Private,
            chat_id,
            user,
            grammers_client: None,
            peer_cache: None,
            progressive_abort: None,
            max_state_keys: 1000,
        }
    }

    // ─── Navigation (mode-aware) ───

    /// Navigate to a screen. Behavior depends on mode:
    /// - Private: full differ (delete/edit/send)
    /// - Group: edit the triggering message in-place
    /// - Inline: edit the inline message
    pub async fn navigate(&mut self, screen: Screen) -> HandlerResult {
        match &self.mode {
            CtxMode::Private => self.navigate_private(screen).await,
            CtxMode::Group { trigger_message_id } => {
                let msg_id = *trigger_message_id;
                self.navigate_group(screen, msg_id).await
            }
            CtxMode::Inline { inline_message_id } => {
                let imid = inline_message_id.clone();
                self.navigate_inline(screen, &imid).await
            }
        }
    }

    /// Private chat navigation — full differ.
    async fn navigate_private(&mut self, screen: Screen) -> HandlerResult {
        // Cancel any active progressive task to prevent concurrent edits.
        self.cancel_progressive();

        if let Some(action) = &screen.typing_action {
            let _ = self.bot.send_chat_action(self.chat_id, *action).await;
        }

        tracing::debug!(
            chat_id = self.chat_id.0,
            screen = %screen.id,
            old_tracked = self.state.active_bot_messages.len(),
            pending_user = self.state.pending_user_messages.len(),
            new_messages = screen.messages.len(),
            "navigate: diffing"
        );

        let ops = Differ::diff_with_frozen(
            &self.state.active_bot_messages,
            &screen,
            &self.state.pending_user_messages,
            &self.state.frozen_messages,
        );

        tracing::debug!(ops = ?ops.iter().map(|op| match op {
            crate::differ::DiffOp::Send { .. } => "Send",
            crate::differ::DiffOp::Edit { .. } => "Edit",
            crate::differ::DiffOp::Delete { .. } => "Delete",
        }).collect::<Vec<_>>(), "diff ops");

        DiffExecutor::execute(
            self.bot.as_ref(),
            self.chat_id,
            ops,
            &mut self.state.active_bot_messages,
        )
        .await
        .map_err(HandlerError::Api)?;

        self.state.pending_user_messages.clear();
        self.state.current_screen = screen.id;

        // Cap tracked messages to prevent unbounded growth.
        const MAX_TRACKED: usize = 100;
        if self.state.active_bot_messages.len() > MAX_TRACKED {
            let excess = self.state.active_bot_messages.len() - MAX_TRACKED;
            self.state.active_bot_messages.drain(..excess);
            tracing::warn!(
                chat_id = self.chat_id.0,
                evicted = excess,
                "active_bot_messages exceeded {MAX_TRACKED}, oldest evicted"
            );
        }

        tracing::debug!(
            chat_id = self.chat_id.0,
            tracked_after = self.state.active_bot_messages.len(),
            "navigate: done"
        );

        Ok(())
    }

    /// Group navigation — edit the triggering message in-place.
    async fn navigate_group(
        &mut self,
        screen: Screen,
        trigger: Option<MessageId>,
    ) -> HandlerResult {
        let msg = screen
            .messages
            .first()
            .ok_or_else(|| HandlerError::Internal(anyhow::anyhow!("screen has no messages")))?;

        if let Some(msg_id) = trigger {
            // Edit the existing message
            match &msg.content {
                MessageContent::Text {
                    text,
                    parse_mode,
                    keyboard,
                    link_preview,
                } => {
                    self.bot
                        .edit_message_text(
                            self.chat_id,
                            msg_id,
                            text.clone(),
                            *parse_mode,
                            keyboard.clone(),
                            matches!(link_preview, LinkPreview::Enabled),
                        )
                        .await
                        .map_err(HandlerError::Api)?;
                }
                other => {
                    let kb = other.keyboard();
                    self.bot
                        .edit_message_media(self.chat_id, msg_id, other.clone(), kb)
                        .await
                        .map_err(HandlerError::Api)?;
                }
            }
        } else {
            // No trigger message (e.g., /command in group) — send new
            self.bot
                .send_message(
                    self.chat_id,
                    msg.content.clone(),
                    SendOptions {
                        reply_to: screen.reply_to,
                        ..Default::default()
                    },
                )
                .await
                .map_err(HandlerError::Api)?;
        }

        self.state.current_screen = screen.id;
        Ok(())
    }

    /// Inline navigation — edit the inline message.
    async fn navigate_inline(&mut self, screen: Screen, inline_message_id: &str) -> HandlerResult {
        let msg = screen
            .messages
            .first()
            .ok_or_else(|| HandlerError::Internal(anyhow::anyhow!("screen has no messages")))?;

        let client = self.require_client()?;

        // Parse the packed inline message ID (base64-encoded TL-serialized)
        let id_bytes = data_encoding::BASE64URL_NOPAD
            .decode(inline_message_id.as_bytes())
            .or_else(|_| data_encoding::BASE64.decode(inline_message_id.as_bytes()))
            .map_err(|_| {
                HandlerError::Internal(anyhow::anyhow!("invalid inline_message_id encoding"))
            })?;

        use grammers_client::tl;
        use grammers_tl_types::Deserializable;

        // Deserialize using TL — the bytes include constructor ID from serialization
        let id_tl = tl::enums::InputBotInlineMessageId::deserialize(
            &mut grammers_tl_types::Cursor::from_slice(&id_bytes),
        )
        .map_err(|e| {
            HandlerError::Internal(anyhow::anyhow!(
                "failed to deserialize inline_message_id: {e}"
            ))
        });

        let id_tl = match id_tl {
            Ok(v) => v,
            Err(_) => {
                // Fallback: try raw byte parsing (no constructor prefix)
                if id_bytes.len() >= 24 {
                    // SAFETY: length is checked above; slice sizes match the target arrays.
                    let dc =
                        i32::from_le_bytes(id_bytes[0..4].try_into().expect("4 bytes for i32"));
                    let owner =
                        i64::from_le_bytes(id_bytes[4..12].try_into().expect("8 bytes for i64"));
                    let msg_id =
                        i32::from_le_bytes(id_bytes[12..16].try_into().expect("4 bytes for i32"));
                    let ah =
                        i64::from_le_bytes(id_bytes[16..24].try_into().expect("8 bytes for i64"));
                    tl::types::InputBotInlineMessageId64 {
                        dc_id: dc,
                        owner_id: owner,
                        id: msg_id,
                        access_hash: ah,
                    }
                    .into()
                } else if id_bytes.len() >= 20 {
                    // SAFETY: length is checked above; slice sizes match the target arrays.
                    let dc =
                        i32::from_le_bytes(id_bytes[0..4].try_into().expect("4 bytes for i32"));
                    let msg_id =
                        i64::from_le_bytes(id_bytes[4..12].try_into().expect("8 bytes for i64"));
                    let ah =
                        i64::from_le_bytes(id_bytes[12..20].try_into().expect("8 bytes for i64"));
                    tl::types::InputBotInlineMessageId {
                        dc_id: dc,
                        id: msg_id,
                        access_hash: ah,
                    }
                    .into()
                } else {
                    return Err(HandlerError::Internal(anyhow::anyhow!(
                        "inline_message_id too short ({} bytes)",
                        id_bytes.len()
                    )));
                }
            }
        };

        // Build text + entities
        let (text, no_webpage, reply_markup) = match &msg.content {
            MessageContent::Text {
                text,
                parse_mode: _,
                keyboard,
                link_preview,
            } => {
                let markup = keyboard.as_ref().map(|kb| {
                    crate::grammers_adapter::GrammersAdapter::to_inline_markup_pub(kb).raw
                });
                (
                    text.clone(),
                    !matches!(link_preview, LinkPreview::Enabled),
                    markup,
                )
            }
            _ => {
                return Err(HandlerError::Internal(anyhow::anyhow!(
                    "inline edit only supports text content currently"
                )));
            }
        };

        client
            .invoke(&tl::functions::messages::EditInlineBotMessage {
                no_webpage,
                invert_media: false,
                id: id_tl,
                message: Some(text),
                media: None,
                reply_markup,
                entities: None,
            })
            .await
            .map_err(|e| {
                HandlerError::Api(crate::grammers_adapter::GrammersAdapter::convert_error_pub(
                    e,
                ))
            })?;

        self.state.current_screen = screen.id;
        Ok(())
    }

    // ─── Push / Pop ───

    /// Navigate with push: saves current screen on the stack for pop() later.
    /// Stack is capped at 20 levels (oldest dropped).
    pub async fn push(&mut self, screen: Screen) -> HandlerResult {
        if self.state.screen_stack.len() >= 20 {
            self.state.screen_stack.remove(0);
        }
        self.state
            .screen_stack
            .push(self.state.current_screen.clone());
        self.navigate(screen).await
    }

    /// Pop: navigates back to the previous screen from the stack.
    ///
    /// If the stack is empty, logs a warning and does nothing.
    /// The `screen_factory` receives the previous screen ID and must return
    /// the Screen to navigate to.
    pub async fn pop(&mut self, screen_factory: impl FnOnce(&ScreenId) -> Screen) -> HandlerResult {
        if let Some(prev) = self.state.screen_stack.pop() {
            let screen = screen_factory(&prev);
            self.navigate(screen).await
        } else {
            tracing::warn!(
                chat_id = self.chat_id.0,
                screen = %self.state.current_screen,
                "pop() called with empty screen stack — nowhere to go back"
            );
            Ok(())
        }
    }

    // ─── Permanent Messages (bypass differ) ───

    /// Send a message that is NOT tracked by the differ.
    /// It will persist across navigate() calls — the framework won't delete it.
    pub async fn send_permanent(&self, screen: Screen) -> Result<SentMessage, HandlerError> {
        let msg = screen
            .messages
            .first()
            .ok_or_else(|| HandlerError::Internal(anyhow::anyhow!("screen has no messages")))?;
        let sent = self
            .bot
            .send_message(
                self.chat_id,
                msg.content.clone(),
                SendOptions {
                    protect_content: screen.protect_content,
                    reply_to: screen.reply_to,
                    ..Default::default()
                },
            )
            .await
            .map_err(HandlerError::Api)?;
        Ok(sent)
    }

    // ─── Reply (conversation mode — bypass differ) ───

    /// Reply to the user. Bypasses the differ entirely.
    ///
    /// - First call in a handler → **sends** a new message
    /// - Subsequent calls → **edits** that same message
    /// - User messages are **not deleted**
    /// - Previous replies are **not deleted**
    /// - Next handler invocation → first `reply()` sends a new message again
    ///
    /// Perfect for LLM streaming, progress bars, conversational bots.
    ///
    /// ```rust,ignore
    /// ctx.reply(Screen::text("▌")).await?;           // sends new
    /// ctx.reply(Screen::text("thinking...▌")).await?; // edits
    /// ctx.reply(Screen::text("done!")).await?;        // edits (final)
    /// ```
    pub async fn reply(&mut self, screen: Screen) -> HandlerResult {
        // Don't delete user messages in reply mode
        self.state.pending_user_messages.clear();

        // Inline mode — always edit the inline message
        if let CtxMode::Inline {
            ref inline_message_id,
        } = self.mode
        {
            let imid = inline_message_id.clone();
            return self.navigate_inline(screen, &imid).await;
        }

        let msg = screen.messages.first().ok_or_else(|| {
            HandlerError::Internal(anyhow::anyhow!("reply screen has no messages"))
        })?;

        // If sealed from previous handler, clear — next reply() is a fresh send
        if self.state.reply_sealed {
            self.state.reply_message_id = None;
            self.state.reply_sealed = false;
        }

        match self.state.reply_message_id {
            Some(msg_id) => {
                // Edit the existing reply message
                match &msg.content {
                    MessageContent::Text {
                        text,
                        parse_mode,
                        keyboard,
                        link_preview,
                    } => {
                        match self
                            .bot
                            .edit_message_text(
                                self.chat_id,
                                msg_id,
                                text.clone(),
                                *parse_mode,
                                keyboard.clone(),
                                matches!(link_preview, LinkPreview::Enabled),
                            )
                            .await
                        {
                            Ok(()) => {}
                            Err(crate::error::ApiError::MessageNotModified) => {}
                            Err(e) => return Err(HandlerError::Api(e)),
                        }
                    }
                    MessageContent::Photo { keyboard, .. }
                    | MessageContent::Animation { keyboard, .. }
                    | MessageContent::Document { keyboard, .. }
                    | MessageContent::Video { keyboard, .. } => {
                        // Use edit_message_media for full media replacement
                        // (handles both media swap and caption-only changes)
                        match self
                            .bot
                            .edit_message_media(
                                self.chat_id,
                                msg_id,
                                msg.content.clone(),
                                keyboard.clone(),
                            )
                            .await
                        {
                            Ok(()) => {}
                            Err(crate::error::ApiError::MessageNotModified) => {}
                            Err(e) => return Err(HandlerError::Api(e)),
                        }
                    }
                    _ => {
                        tracing::warn!(
                            content_type = ?msg.content.content_type(),
                            "reply() edit: unsupported content type, skipping"
                        );
                    }
                }
            }
            None => {
                // Send a new reply message
                let sent = self
                    .bot
                    .send_message(
                        self.chat_id,
                        msg.content.clone(),
                        SendOptions {
                            protect_content: screen.protect_content,
                            reply_to: screen.reply_to,
                            ..Default::default()
                        },
                    )
                    .await
                    .map_err(HandlerError::Api)?;
                self.state.reply_message_id = Some(sent.message_id);
                // Track reply messages so navigate() can clean them up
                // if the bot switches from reply mode to screen mode.
                self.state
                    .active_bot_messages
                    .push(TrackedMessage::from_content(sent.message_id, &msg.content));
            }
        }
        Ok(())
    }

    /// Prevent pending user messages from being deleted on next `navigate()`.
    pub fn keep_user_messages(&mut self) {
        self.state.pending_user_messages.clear();
    }

    /// Freeze a message — the differ will never delete it.
    /// Useful for conversation history, receipts, pinned messages.
    /// Capped at 100 frozen messages (oldest evicted first).
    pub fn freeze_message(&mut self, message_id: MessageId) {
        if !self.state.frozen_messages.contains(&message_id) {
            if self.state.frozen_messages.len() >= 100 {
                self.state.frozen_messages.remove(0); // evict oldest
            }
            self.state.frozen_messages.push(message_id);
        }
    }

    /// Unfreeze a message — allow the differ to delete it again.
    pub fn unfreeze_message(&mut self, message_id: MessageId) {
        self.state.frozen_messages.retain(|id| *id != message_id);
    }

    // ─── Forward ───

    /// Forward a message from another chat. Not tracked by differ.
    pub async fn forward(&self, from_chat_id: ChatId, message_id: MessageId) -> HandlerResult {
        let client = self.require_client()?;
        let cache = self.peer_cache.as_ref().ok_or_else(|| {
            HandlerError::Internal(anyhow::anyhow!("forward requires peer cache"))
        })?;
        let from_peer =
            crate::grammers_adapter::GrammersAdapter::resolve_from_cache(cache, from_chat_id)
                .ok_or_else(|| HandlerError::Api(crate::error::ApiError::ChatNotFound))?;
        let to_peer =
            crate::grammers_adapter::GrammersAdapter::resolve_from_cache(cache, self.chat_id)
                .ok_or_else(|| HandlerError::Api(crate::error::ApiError::ChatNotFound))?;

        use grammers_client::tl;
        client
            .invoke(&tl::functions::messages::ForwardMessages {
                silent: false,
                background: false,
                with_my_score: false,
                drop_author: false,
                drop_media_captions: false,
                noforwards: false,
                allow_paid_floodskip: false,
                from_peer: from_peer.into(),
                id: vec![message_id.0],
                random_id: vec![rand_i64()],
                to_peer: to_peer.into(),
                top_msg_id: None,
                reply_to: None,
                schedule_date: None,
                schedule_repeat_period: None,
                send_as: None,
                quick_reply_shortcut: None,
                effect: None,
                video_timestamp: None,
                allow_paid_stars: None,
                suggested_post: None,
            })
            .await
            .map_err(|e| {
                HandlerError::Api(crate::grammers_adapter::GrammersAdapter::convert_error_pub(
                    e,
                ))
            })?;

        Ok(())
    }

    // ─── Convenience methods (raw MTProto) ───

    /// Pin a message in this chat.
    pub async fn pin_message(&self, message_id: MessageId) -> HandlerResult {
        let client = self.require_client()?;
        let peer = self.require_peer()?;
        use grammers_client::tl;
        client
            .invoke(&tl::functions::messages::UpdatePinnedMessage {
                silent: true,
                unpin: false,
                pm_oneside: false,
                peer: peer.into(),
                id: message_id.0,
            })
            .await
            .map_err(|e| {
                HandlerError::Api(crate::grammers_adapter::GrammersAdapter::convert_error_pub(
                    e,
                ))
            })?;
        Ok(())
    }

    /// Unpin a message.
    pub async fn unpin_message(&self, message_id: MessageId) -> HandlerResult {
        let client = self.require_client()?;
        let peer = self.require_peer()?;
        use grammers_client::tl;
        client
            .invoke(&tl::functions::messages::UpdatePinnedMessage {
                silent: true,
                unpin: true,
                pm_oneside: false,
                peer: peer.into(),
                id: message_id.0,
            })
            .await
            .map_err(|e| {
                HandlerError::Api(crate::grammers_adapter::GrammersAdapter::convert_error_pub(
                    e,
                ))
            })?;
        Ok(())
    }

    /// Direct access to the raw [grammers `Client`](grammers_client::Client)
    /// for any MTProto method not wrapped by [`BotApi`].
    ///
    /// Returns `None` in tests (where no real connection exists) and when
    /// running with a non-grammers adapter.
    #[must_use]
    pub fn client(&self) -> Option<&grammers_client::Client> {
        self.grammers_client.as_ref()
    }

    /// The resolved peer reference for this chat, needed for raw TL calls.
    ///
    /// Returns `None` in tests, or if the peer hasn't been cached yet
    /// (extremely rare — the adapter caches on first message).
    #[must_use]
    pub fn peer_ref(&self) -> Option<grammers_session::types::PeerRef> {
        let cache = self.peer_cache.as_ref()?;
        crate::grammers_adapter::GrammersAdapter::resolve_from_cache(cache, self.chat_id)
    }

    /// Unwraps [`client()`](Self::client), returning an error if unavailable.
    fn require_client(&self) -> Result<&grammers_client::Client, HandlerError> {
        self.grammers_client.as_ref().ok_or_else(|| {
            HandlerError::Internal(anyhow::anyhow!("operation requires grammers client"))
        })
    }

    /// Unwraps [`peer_ref()`](Self::peer_ref), returning `ApiError::ChatNotFound` if unavailable.
    fn require_peer(&self) -> Result<grammers_session::types::PeerRef, HandlerError> {
        self.peer_ref()
            .ok_or_else(|| HandlerError::Api(crate::error::ApiError::ChatNotFound))
    }

    // ─── Toasts & Alerts ───

    /// Small popup notification (bottom of screen).
    ///
    /// Only works inside a callback handler. If called outside a callback context,
    /// logs a warning and does nothing (Telegram requires a callback query ID).
    pub async fn toast(&mut self, text: impl Into<String>) -> HandlerResult {
        if let Some(cb_id) = self.state.pending_callback_id.take() {
            self.bot
                .answer_callback_query(cb_id, Some(text.into()), false)
                .await
                .map_err(HandlerError::Api)?;
        } else {
            tracing::warn!("toast() called outside callback context — no callback query to answer");
        }
        Ok(())
    }

    /// Modal alert (with OK button).
    ///
    /// Only works inside a callback handler. If called outside a callback context,
    /// logs a warning and does nothing.
    pub async fn alert(&mut self, text: impl Into<String>) -> HandlerResult {
        if let Some(cb_id) = self.state.pending_callback_id.take() {
            self.bot
                .answer_callback_query(cb_id, Some(text.into()), true)
                .await
                .map_err(HandlerError::Api)?;
        } else {
            tracing::warn!("alert() called outside callback context — no callback query to answer");
        }
        Ok(())
    }

    // ─── FSM Data ───

    /// Store a value in the per-chat state under the given key.
    ///
    /// The value is serialized to JSON. If serialization fails (e.g. a map
    /// with non-string keys), the call is silently ignored and an error is logged.
    ///
    /// If the number of keys exceeds `max_state_keys` (default 1000),
    /// an arbitrary non-internal key is evicted and a warning is logged.
    pub fn set<V: Serialize>(&mut self, key: &str, value: &V) {
        let val = match serde_json::to_value(value) {
            Ok(v) => v,
            Err(e) => {
                tracing::error!(key, error = %e, "failed to serialize state value — ignoring set()");
                return;
            }
        };

        // If key already exists, just overwrite — no size change.
        if self.state.data.contains_key(key) {
            self.state.data.insert(key.to_string(), val);
            return;
        }

        // Evict oldest non-internal key if at capacity.
        if self.state.data.len() >= self.max_state_keys {
            tracing::warn!(
                chat_id = self.chat_id.0,
                keys = self.state.data.len(),
                max = self.max_state_keys,
                "state key limit reached — evicting oldest entry"
            );
            // Find first key that doesn't start with "__" (internal).
            let victim = self
                .state
                .data
                .keys()
                .find(|k| !k.starts_with("__"))
                .cloned();
            if let Some(k) = victim {
                self.state.data.remove(&k);
            }
        }

        self.state.data.insert(key.to_string(), val);
    }

    /// Look up a value from per-chat state by key, deserializing into `V`.
    ///
    /// Returns `None` if the key does not exist or the stored value cannot
    /// be deserialized into `V`.
    pub fn get<V: DeserializeOwned>(&self, key: &str) -> Option<V> {
        self.state
            .data
            .get(key)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }

    /// Remove a value from per-chat state by key. No-op if the key doesn't exist.
    pub fn remove(&mut self, key: &str) {
        self.state.data.remove(key);
    }

    /// Clear all per-chat state data and the navigation stack.
    pub fn clear_data(&mut self) {
        self.state.data.clear();
        self.state.screen_stack.clear();
    }

    // ─── Typed State ───

    /// Retrieve the typed per-chat state, or `Default::default()` if not yet set.
    pub fn state<S: DeserializeOwned + Default>(&self) -> S {
        self.get::<S>("__state__").unwrap_or_default()
    }

    /// Replace the typed per-chat state with the given value.
    pub fn set_state<S: Serialize>(&mut self, s: &S) {
        self.set("__state__", s);
    }

    // ─── Callback Data ───

    /// Raw callback data string from the pressed inline button, if any.
    #[must_use]
    pub fn callback_data(&self) -> Option<&str> {
        self.callback_data.as_deref()
    }

    /// Callback data split by `:`, skipping the first segment (the action name).
    ///
    /// For callback data `"pick:a:b"`, returns `["a", "b"]`.
    #[must_use]
    pub fn callback_params(&self) -> Vec<String> {
        self.callback_data
            .as_ref()
            .map(|d| d.split(':').skip(1).map(String::from).collect())
            .unwrap_or_default()
    }

    /// First callback parameter (second segment after `:`), if any.
    ///
    /// For callback data `"pick:dark"`, returns `Some("dark")`.
    #[must_use]
    pub fn callback_param(&self) -> Option<String> {
        self.callback_params().into_iter().next()
    }

    /// First callback parameter parsed as type `T`. Returns `None` on missing or parse failure.
    #[must_use]
    pub fn callback_param_as<T: std::str::FromStr>(&self) -> Option<T> {
        self.callback_param()?.parse().ok()
    }

    // ─── Utilities ───

    /// Delete a specific message immediately. Not tracked by the differ.
    pub async fn delete_now(&self, message_id: MessageId) -> HandlerResult {
        self.bot
            .delete_messages(self.chat_id, vec![message_id])
            .await
            .map_err(HandlerError::Api)?;
        Ok(())
    }

    /// Send a "typing…" indicator to the chat. Disappears after ~5 seconds or on next message.
    pub async fn typing(&self) -> HandlerResult {
        self.bot
            .send_chat_action(self.chat_id, ChatAction::Typing)
            .await
            .map_err(HandlerError::Api)?;
        Ok(())
    }

    /// Send a quick text message (untracked — survives navigate).
    /// Returns the sent message for pinning, forwarding, etc.
    pub async fn send_text(&self, text: impl Into<String>) -> Result<SentMessage, HandlerError> {
        let sent = self
            .bot
            .send_message(
                self.chat_id,
                MessageContent::Text {
                    text: text.into(),
                    parse_mode: ParseMode::Html,
                    keyboard: None,
                    link_preview: LinkPreview::Disabled,
                },
                SendOptions::default(),
            )
            .await
            .map_err(HandlerError::Api)?;
        Ok(sent)
    }

    /// Send a notification that will be auto-deleted on next `navigate()` (private chat mode only).
    pub async fn notify(&mut self, text: impl Into<String>) -> HandlerResult {
        let sent = self
            .bot
            .send_message(
                self.chat_id,
                MessageContent::Text {
                    text: text.into(),
                    parse_mode: ParseMode::Html,
                    keyboard: None,
                    link_preview: LinkPreview::Disabled,
                },
                SendOptions::default(),
            )
            .await
            .map_err(HandlerError::Api)?;
        self.state.pending_user_messages.push(sent.message_id);
        Ok(())
    }

    /// Send a temp notification that auto-deletes after duration.
    pub async fn notify_temp(&self, text: impl Into<String>, duration: Duration) -> HandlerResult {
        let sent = self
            .bot
            .send_message(
                self.chat_id,
                MessageContent::Text {
                    text: text.into(),
                    parse_mode: ParseMode::Html,
                    keyboard: None,
                    link_preview: LinkPreview::Disabled,
                },
                SendOptions::default(),
            )
            .await
            .map_err(HandlerError::Api)?;

        let bot = self.bot.clone();
        let chat_id = self.chat_id;
        let msg_id = sent.message_id;
        tokio::spawn(async move {
            tokio::time::sleep(duration).await;
            let _ = bot.delete_messages(chat_id, vec![msg_id]).await;
        });
        Ok(())
    }

    // ─── Progressive (streaming updates) ───

    /// Start a progressive update stream. Sends `initial` screen immediately,
    /// then returns a handle for streaming updates (e.g., LLM token streaming).
    ///
    /// Auto-throttles edits to respect Telegram rate limits.
    pub async fn progressive(
        &mut self,
        initial: Screen,
    ) -> Result<crate::progressive::ProgressiveHandle, HandlerError> {
        // Cancel any previous progressive task.
        self.cancel_progressive();

        match &self.mode {
            CtxMode::Private | CtxMode::Group { .. } => {
                let sent = self
                    .bot
                    .send_message(
                        self.chat_id,
                        initial
                            .messages
                            .first()
                            .ok_or_else(|| HandlerError::Internal(anyhow::anyhow!("empty screen")))?
                            .content
                            .clone(),
                        SendOptions::default(),
                    )
                    .await
                    .map_err(HandlerError::Api)?;

                self.state
                    .active_bot_messages
                    .push(TrackedMessage::from_content(
                        sent.message_id,
                        &initial.messages[0].content,
                    ));

                let bot = self.bot.clone();
                let chat_id = self.chat_id;
                let msg_id = sent.message_id;

                let editor: crate::progressive::EditorFn = Arc::new(move |screen| {
                    let bot = bot.clone();
                    Box::pin(async move {
                        let msg = screen.messages.first().ok_or_else(|| {
                            crate::error::ApiError::Unknown("empty screen".into())
                        })?;
                        match &msg.content {
                            MessageContent::Text {
                                text,
                                parse_mode,
                                keyboard,
                                link_preview,
                            } => {
                                bot.edit_message_text(
                                    chat_id,
                                    msg_id,
                                    text.clone(),
                                    *parse_mode,
                                    keyboard.clone(),
                                    matches!(link_preview, LinkPreview::Enabled),
                                )
                                .await
                            }
                            _ => Err(crate::error::ApiError::Unknown(
                                "progressive only supports text".into(),
                            )),
                        }
                    })
                });

                let handle = crate::progressive::start_progressive_with_editor(
                    editor,
                    std::time::Duration::from_millis(1500),
                );
                self.progressive_abort = Some(handle.abort_handle());
                Ok(handle)
            }
            CtxMode::Inline {
                inline_message_id: _,
            } => {
                // For inline, the message is already sent. Create editor for inline edit.
                // TODO: implement inline progressive
                Err(HandlerError::Internal(anyhow::anyhow!(
                    "progressive not yet supported for inline"
                )))
            }
        }
    }

    /// Cancel any active progressive task.
    /// Called automatically by navigate() to prevent concurrent edits.
    fn cancel_progressive(&mut self) {
        if let Some(handle) = self.progressive_abort.take() {
            handle.abort();
        }
    }

    /// The deep link parameter from `/start <payload>`, if present.
    ///
    /// Only populated when the incoming message is a `/start` command with
    /// a payload (e.g. `/start ref_123` → `Some("ref_123")`). Returns `None`
    /// for plain `/start` or any other command/message.
    #[must_use]
    pub fn deep_link(&self) -> Option<&str> {
        self.deep_link.as_deref()
    }

    /// The [`ScreenId`] currently displayed to this user.
    ///
    /// Starts as `"__initial__"` before the first [`navigate()`](Self::navigate) call.
    /// Used internally by the router to select screen-specific input handlers.
    #[must_use]
    pub fn current_screen(&self) -> &ScreenId {
        &self.state.current_screen
    }

    /// Access the underlying [`BotApi`] implementation.
    ///
    /// Useful for calling methods not wrapped by `Ctx` (e.g. `edit_message_text`,
    /// `get_chat_member_count`). In tests, this returns the [`MockBotApi`](crate::mock::MockBotApi).
    #[must_use]
    pub fn bot(&self) -> &dyn BotApi {
        self.bot.as_ref()
    }

    /// Full text of the incoming message, if any.
    ///
    /// Includes the command itself (e.g. `/start payload`). For callback queries,
    /// this is `None` — use [`callback_data()`](Self::callback_data) instead.
    #[must_use]
    pub fn text(&self) -> Option<&str> {
        self.message_text.as_deref()
    }

    /// The inline query ID, present only in [`on_inline`](crate::app::AppBuilder::on_inline)
    /// handlers. Use with [`answer_inline()`](Self::answer_inline) to send results.
    #[must_use]
    pub fn inline_query_id(&self) -> Option<&str> {
        self.inline_query_id.as_deref()
    }

    /// The chosen result ID, present only in
    /// [`on_chosen_inline`](crate::app::AppBuilder::on_chosen_inline) handlers.
    /// This is the `result_id` the user selected from the inline results list.
    #[must_use]
    pub fn chosen_inline_result_id(&self) -> Option<&str> {
        self.chosen_inline_result_id.as_deref()
    }

    /// ID of the incoming message that triggered this handler.
    ///
    /// Present for text messages, media, and edits. `None` for callback queries,
    /// inline queries, payment events, and member join/leave events.
    #[must_use]
    pub fn message_id(&self) -> Option<MessageId> {
        self.incoming_message_id
    }

    /// ID of the last message sent via [`reply()`](Self::reply), if any.
    ///
    /// Useful for later editing or referencing the bot's reply message.
    #[must_use]
    pub fn reply_message_id(&self) -> Option<MessageId> {
        self.state.reply_message_id
    }

    /// Pre-checkout query ID, present only in
    /// [`on_pre_checkout`](crate::app::AppBuilder::on_pre_checkout) handlers.
    /// Use with [`approve_checkout()`](Self::approve_checkout) or
    /// [`decline_checkout()`](Self::decline_checkout).
    #[must_use]
    pub fn pre_checkout_id(&self) -> Option<&str> {
        self.payment.query_id.as_deref()
    }

    /// The `payload` string from the invoice, present in payment handlers.
    /// Use to identify what the user is paying for.
    #[must_use]
    pub fn payment_payload(&self) -> Option<&str> {
        self.payment.payload.as_deref()
    }

    /// Three-letter ISO 4217 currency code (e.g. `"USD"`, `"XTR"` for Stars).
    /// Present in both pre-checkout and successful-payment handlers.
    #[must_use]
    pub fn payment_currency(&self) -> Option<&str> {
        self.payment.currency.as_deref()
    }

    /// Payment total amount in the smallest currency unit (e.g. cents for USD,
    /// stars for `XTR`). Present in payment handlers.
    #[must_use]
    pub fn payment_total_amount(&self) -> Option<i64> {
        self.payment.total_amount
    }

    /// Approve a pre-checkout query (payment flow).
    pub async fn approve_checkout(&self) -> HandlerResult {
        let id = self
            .payment
            .query_id
            .clone()
            .ok_or_else(|| HandlerError::User("no pre-checkout query to answer".into()))?;
        self.bot
            .answer_pre_checkout_query(id, true, None)
            .await
            .map_err(HandlerError::Api)
    }

    /// Decline a pre-checkout query with a reason.
    pub async fn decline_checkout(&self, reason: impl Into<String>) -> HandlerResult {
        let id = self
            .payment
            .query_id
            .clone()
            .ok_or_else(|| HandlerError::User("no pre-checkout query to answer".into()))?;
        self.bot
            .answer_pre_checkout_query(id, false, Some(reason.into()))
            .await
            .map_err(HandlerError::Api)
    }

    /// Answer an inline query with results.
    pub async fn answer_inline(
        &self,
        results: Vec<InlineQueryResult>,
        next_offset: Option<String>,
        cache_time: Option<i32>,
        is_personal: bool,
    ) -> HandlerResult {
        let query_id = self
            .inline_query_id
            .clone()
            .ok_or_else(|| HandlerError::User("no inline query to answer".into()))?;
        tracing::debug!(query_id = %query_id, result_count = results.len(), "answering inline query");
        match self
            .bot
            .answer_inline_query(query_id, results, next_offset, cache_time, is_personal)
            .await
        {
            Ok(()) => {
                tracing::debug!("answer_inline_query OK");
                Ok(())
            }
            Err(e) => {
                tracing::error!(error = %e, "answer_inline_query FAILED");
                Err(HandlerError::Api(e))
            }
        }
    }

    // ─── I18n ───

    /// User's language code, or the I18n default if not set.
    #[must_use]
    pub fn lang(&self) -> &str {
        self.user
            .language_code
            .as_deref()
            .unwrap_or_else(|| i18n::i18n().default_lang())
    }

    /// Translate a key using the user's language.
    pub fn t(&self, key: &str) -> String {
        i18n::i18n().t(self.lang(), key)
    }

    /// Translate with variable substitutions. Each `(name, value)` pair replaces `{ $name }` in the message.
    pub fn t_with(&self, key: &str, args: &[(&str, &str)]) -> String {
        i18n::i18n().t_with(self.lang(), key, args)
    }

    // ─── Convenience: forwarding & copying ───

    /// Copy a message to this chat (re-send without "Forwarded from" header).
    pub async fn copy_here(
        &self,
        from_chat_id: ChatId,
        message_id: MessageId,
    ) -> Result<MessageId, HandlerError> {
        self.bot
            .copy_message(self.chat_id, from_chat_id, message_id)
            .await
            .map_err(HandlerError::Api)
    }

    /// Forward a message to this chat.
    pub async fn forward_here(
        &self,
        from_chat_id: ChatId,
        message_id: MessageId,
    ) -> Result<SentMessage, HandlerError> {
        self.bot
            .forward_message(self.chat_id, from_chat_id, message_id)
            .await
            .map_err(HandlerError::Api)
    }

    // ─── Convenience: media ───

    /// Download a file by its file_id.
    pub async fn download(&self, file_id: &str) -> Result<DownloadedFile, HandlerError> {
        self.bot
            .download_file(file_id)
            .await
            .map_err(HandlerError::Api)
    }

    // ─── Convenience: fun ───

    /// Send a dice animation. Returns the sent message.
    pub async fn send_dice(&self, emoji: DiceEmoji) -> Result<SentMessage, HandlerError> {
        self.bot
            .send_dice(self.chat_id, emoji)
            .await
            .map_err(HandlerError::Api)
    }

    /// Send a poll.
    pub async fn send_poll(&self, poll: SendPoll) -> Result<SentMessage, HandlerError> {
        self.bot
            .send_poll(self.chat_id, poll)
            .await
            .map_err(HandlerError::Api)
    }

    // ─── Convenience: reactions ───

    /// React to a message with an emoji.
    pub async fn react(&self, message_id: MessageId, emoji: &str) -> HandlerResult {
        self.bot
            .set_message_reaction(self.chat_id, message_id, emoji)
            .await
            .map_err(HandlerError::Api)
    }

    /// React to the incoming message.
    pub async fn react_incoming(&self, emoji: &str) -> HandlerResult {
        if let Some(msg_id) = self.incoming_message_id {
            self.react(msg_id, emoji).await
        } else {
            tracing::warn!("react_incoming() called with no incoming message");
            Ok(())
        }
    }

    // ─── Convenience: admin ───

    /// Ban a user from this chat.
    pub async fn ban(&self, user_id: UserId) -> HandlerResult {
        self.bot
            .ban_chat_member(self.chat_id, user_id)
            .await
            .map_err(HandlerError::Api)
    }

    /// Unban a user in this chat.
    pub async fn unban(&self, user_id: UserId) -> HandlerResult {
        self.bot
            .unban_chat_member(self.chat_id, user_id)
            .await
            .map_err(HandlerError::Api)
    }

    /// Get the member count for this chat.
    pub async fn member_count(&self) -> Result<i32, HandlerError> {
        self.bot
            .get_chat_member_count(self.chat_id)
            .await
            .map_err(HandlerError::Api)
    }

    // ─── Convenience: payments ───

    /// Send an invoice to this chat.
    pub async fn send_invoice(&self, invoice: Invoice) -> Result<SentMessage, HandlerError> {
        self.bot
            .send_invoice(self.chat_id, invoice)
            .await
            .map_err(HandlerError::Api)
    }

    /// Start a multi-step form wizard.
    ///
    /// Looks up the form by `form_id` in the registered forms map, initialises
    /// form state in the chat, and navigates to the first step.
    pub async fn start_form(
        &mut self,
        form_id: &str,
        forms: &HashMap<String, Form>,
    ) -> HandlerResult {
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

fn rand_i64() -> i64 {
    fastrand::i64(..)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::MockBotApi;
    use std::sync::Arc;

    fn make_ctx() -> Ctx {
        let user = UserInfo {
            id: UserId(123),
            first_name: "Test".into(),
            last_name: None,
            username: Some("testuser".into()),
            language_code: Some("en".into()),
        };
        let state = ChatState::new(ChatId(123), user);
        let bot: Arc<dyn BotApi> = Arc::new(MockBotApi::new());
        Ctx::new(state, bot, None)
    }

    #[test]
    fn ctx_set_get_remove() {
        let mut ctx = make_ctx();
        ctx.set("name", &"Alice");
        let name: String = ctx.get("name").unwrap();
        assert_eq!(name, "Alice");
        ctx.remove("name");
        assert!(ctx.get::<String>("name").is_none());
    }

    #[test]
    fn ctx_set_get_complex_types() {
        let mut ctx = make_ctx();
        ctx.set("count", &42i64);
        let count: i64 = ctx.get("count").unwrap();
        assert_eq!(count, 42);
        ctx.set("items", &vec!["a", "b", "c"]);
        let items: Vec<String> = ctx.get("items").unwrap();
        assert_eq!(items.len(), 3);
    }

    #[test]
    fn ctx_clear_data() {
        let mut ctx = make_ctx();
        ctx.set("a", &1);
        ctx.set("b", &2);
        ctx.clear_data();
        assert!(ctx.get::<i32>("a").is_none());
        assert!(ctx.get::<i32>("b").is_none());
    }

    #[test]
    fn ctx_state_and_set_state() {
        let mut ctx = make_ctx();
        let st: String = ctx.state();
        assert_eq!(st, ""); // default for String
        ctx.set_state(&"home".to_string());
        let st2: String = ctx.state();
        assert_eq!(st2, "home");
    }

    #[test]
    fn ctx_user_info() {
        let ctx = make_ctx();
        assert_eq!(ctx.user.first_name, "Test");
        assert_eq!(ctx.chat_id, ChatId(123));
    }

    #[test]
    fn ctx_callback_data() {
        let user = UserInfo {
            id: UserId(1),
            first_name: "U".into(),
            last_name: None,
            username: None,
            language_code: None,
        };
        let state = ChatState::new(ChatId(1), user);
        let bot: Arc<dyn BotApi> = Arc::new(MockBotApi::new());
        let ctx = Ctx::new(state, bot, Some("action:view:42".into()));
        assert_eq!(ctx.callback_data(), Some("action:view:42"));
        // callback_params skips first segment
        assert_eq!(ctx.callback_params(), vec!["view", "42"]);
        // callback_param returns the first param (second segment)
        assert_eq!(ctx.callback_param(), Some("view".to_string()));
    }

    #[test]
    fn ctx_deep_link() {
        let mut ctx = make_ctx();
        assert!(ctx.deep_link().is_none());
        ctx.deep_link = Some("ref_abc".into());
        assert_eq!(ctx.deep_link(), Some("ref_abc"));
    }

    #[test]
    fn ctx_text() {
        let mut ctx = make_ctx();
        assert!(ctx.text().is_none());
        ctx.message_text = Some("hello world".into());
        assert_eq!(ctx.text(), Some("hello world"));
    }

    #[test]
    fn ctx_freeze_unfreeze() {
        let mut ctx = make_ctx();
        let mid = MessageId(10);
        ctx.freeze_message(mid);
        assert!(ctx.state.frozen_messages.contains(&mid));
        ctx.unfreeze_message(mid);
        assert!(!ctx.state.frozen_messages.contains(&mid));
    }

    #[test]
    fn ctx_lang() {
        let ctx = make_ctx();
        assert_eq!(ctx.lang(), "en");
    }

    #[test]
    fn ctx_current_screen() {
        let ctx = make_ctx();
        assert_eq!(*ctx.current_screen(), ScreenId::from("__initial__"));
    }

    #[test]
    fn ctx_max_state_keys_enforced() {
        let mut ctx = make_ctx();
        ctx.max_state_keys = 2;
        ctx.set("a", &1);
        ctx.set("b", &2);
        // Third key evicts the oldest
        ctx.set("c", &3);
        assert_eq!(ctx.state.data.len(), 2);
        assert!(ctx.get::<i32>("c").is_some());
    }
}
