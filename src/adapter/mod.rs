//! BotApi implementation backed by grammers (pure Rust MTProto).
//!
//! Speaks MTProto directly to Telegram DC — no HTTP, no Bot API proxy.
//! Single binary, zero external processes.
//!
//! Split into sub-modules by domain:
//! - **send** — Core messaging (send, edit, delete, callback, inline, forward/copy)
//! - **media** — Media operations (download, poll, dice, contact, venue, sticker, location)
//! - **admin** — Chat administration (ban, unban, promote, invite links, join requests, etc.)
//! - **settings** — Bot settings (commands, descriptions, name, menu button)
//! - **forum** — Forum topic management
//! - **stars** — Telegram Stars API (transactions, refunds)

mod admin;
mod forum;
mod helpers;
mod media;
mod send;
mod settings;
mod stars;

use async_trait::async_trait;
use dashmap::DashMap;
use grammers_client::{
    Client, InvocationError,
    message::{Button, InputMessage, Key, ReplyMarkup},
    tl,
};
use grammers_session::types::PeerRef;
use std::sync::Arc;

use crate::bot_api::{BotApi, SendOptions};
use crate::error::ApiError;
use crate::keyboard::{ButtonAction, InlineKeyboard};
use crate::types::*;

/// Default Telegram Desktop API credentials (public, from TDesktop source).
pub const DEFAULT_API_ID: i32 = 2040;
/// Default Telegram API hash (from Telegram Desktop open-source client).
pub const DEFAULT_API_HASH: &str = "b18441a1ff607e10a989891a5462e627";

/// A [`BotApi`] implementation using grammers MTProto client.
#[derive(Clone)]
pub struct GrammersAdapter {
    client: Client,
    /// Cached PeerRefs, keyed by ChatId.0
    peers: Arc<DashMap<i64, PeerRef>>,
}

impl GrammersAdapter {
    /// Open or create a redb database at the given path.
    pub fn new(client: Client) -> Self {
        Self {
            client,
            peers: Arc::new(DashMap::new()),
        }
    }

    /// Client.
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Cache a PeerRef for later API calls. Must be called for every incoming update.
    pub fn cache_peer(&self, peer: PeerRef) {
        self.peers.insert(peer.id.bot_api_dialog_id(), peer);
    }

    /// Get peer.
    pub fn get_peer(&self, chat_id: ChatId) -> Option<PeerRef> {
        self.peers.get(&chat_id.0).map(|r| *r)
    }

    /// Serialize the peer cache for persistence.
    pub fn export_peers(&self) -> Vec<(i64, i64, i64)> {
        self.peers
            .iter()
            .map(|entry| {
                let id = *entry.key();
                let peer = entry.value();
                let bare_id = peer.id.bare_id();
                let access_hash = peer.auth.hash();
                (id, bare_id, access_hash)
            })
            .collect()
    }

    /// Restore the peer cache from persisted data.
    pub fn import_peers(&self, data: &[(i64, i64, i64)]) {
        use grammers_session::types::{PeerAuth, PeerId};
        for &(dialog_id, bare_id, access_hash) in data {
            let peer_id = if dialog_id > 0 {
                PeerId::user(bare_id)
            } else if dialog_id < -1_000_000_000 {
                PeerId::channel(bare_id)
            } else {
                PeerId::chat(bare_id)
            };
            if let Some(pid) = peer_id {
                let peer_ref = PeerRef {
                    id: pid,
                    auth: PeerAuth::from_hash(access_hash),
                };
                self.peers.insert(dialog_id, peer_ref);
            }
        }
    }

    fn resolve(&self, chat_id: ChatId) -> Result<PeerRef, ApiError> {
        self.get_peer(chat_id).ok_or(ApiError::ChatNotFound)
    }

    // ── Error conversion ──

    fn convert_error(e: InvocationError) -> ApiError {
        match &e {
            InvocationError::Rpc(rpc) => {
                let name = rpc.name.as_str();
                match name {
                    "MESSAGE_ID_INVALID" | "MESSAGE_DELETE_FORBIDDEN" => ApiError::MessageNotFound,
                    "MESSAGE_NOT_MODIFIED" => ApiError::MessageNotModified,
                    "FLOOD_WAIT" | "SLOWMODE_WAIT" => ApiError::TooManyRequests {
                        retry_after: rpc.value.unwrap_or(1),
                    },
                    "ENTITY_BOUNDS_INVALID" | "ENTITY_TEXTURL_INVALID" => {
                        ApiError::EntityBoundsInvalid
                    }
                    "USER_IS_BLOCKED" | "BOT_BLOCKED" => ApiError::BotBlocked,
                    "PEER_ID_INVALID"
                    | "CHAT_WRITE_FORBIDDEN"
                    | "CHANNEL_PRIVATE"
                    | "INPUT_USER_DEACTIVATED"
                    | "USER_DEACTIVATED" => ApiError::ChatNotFound,
                    _ if rpc.code == 403 => ApiError::Forbidden(name.to_string()),
                    _ => ApiError::Unknown(format!("{name} (code {})", rpc.code)),
                }
            }
            InvocationError::Io(_) | InvocationError::Dropped => ApiError::Network(e.to_string()),
            _ => ApiError::Unknown(e.to_string()),
        }
    }

    // ── Keyboard conversion ──

    fn to_inline_markup(kb: &InlineKeyboard) -> ReplyMarkup {
        let rows: Vec<Vec<Button>> = kb
            .rows
            .iter()
            .map(|row| {
                row.iter()
                    .map(|btn| match &btn.action {
                        ButtonAction::Callback(data) => Button::data(&btn.text, data.as_bytes()),
                        ButtonAction::Url(url) => Button::url(&btn.text, url),
                        ButtonAction::WebApp(url) => Button::webview(&btn.text, url),
                        ButtonAction::SwitchInline {
                            query,
                            current_chat,
                        } => {
                            if *current_chat {
                                Button::switch(&btn.text, query)
                            } else {
                                Button::switch_elsewhere(&btn.text, query)
                            }
                        }
                    })
                    .collect()
            })
            .collect();
        ReplyMarkup::from_buttons(&rows)
    }

    fn to_reply_markup(action: &crate::screen::ReplyKeyboardAction) -> ReplyMarkup {
        match action {
            crate::screen::ReplyKeyboardAction::Show {
                rows,
                resize,
                one_time,
                ..
            } => {
                let key_rows: Vec<Vec<Key>> = rows
                    .iter()
                    .map(|row| {
                        row.iter()
                            .map(|btn| {
                                if btn.request_contact {
                                    Key::request_phone(&btn.text)
                                } else if btn.request_location {
                                    Key::request_geo(&btn.text)
                                } else {
                                    Key::text(&btn.text)
                                }
                            })
                            .collect()
                    })
                    .collect();
                let mut m = ReplyMarkup::from_keys(&key_rows);
                if *resize {
                    m = m.fit_size();
                }
                if *one_time {
                    m = m.single_use();
                }
                m
            }
            crate::screen::ReplyKeyboardAction::Remove => ReplyMarkup::hide(),
        }
    }

    // ── Message building ──

    fn text_msg(text: &str, parse_mode: ParseMode, link_preview: bool) -> InputMessage {
        let msg = match parse_mode {
            ParseMode::Html => InputMessage::new().html(text),
            ParseMode::MarkdownV2 => InputMessage::new().markdown(text),
            ParseMode::None => InputMessage::new().text(text),
        };
        msg.link_preview(link_preview)
    }

    fn with_markup(
        msg: InputMessage,
        inline: Option<&InlineKeyboard>,
        reply: Option<&crate::screen::ReplyKeyboardAction>,
    ) -> InputMessage {
        if let Some(kb) = inline {
            msg.reply_markup(Self::to_inline_markup(kb))
        } else if let Some(rk) = reply {
            msg.reply_markup(Self::to_reply_markup(rk))
        } else {
            msg
        }
    }

    async fn build_input(
        &self,
        content: &MessageContent,
        reply_kb: Option<&crate::screen::ReplyKeyboardAction>,
    ) -> Result<InputMessage, ApiError> {
        match content {
            MessageContent::Text {
                text,
                parse_mode,
                keyboard,
                link_preview,
            } => {
                let lp = matches!(link_preview, LinkPreview::Enabled);
                let msg = Self::text_msg(text, *parse_mode, lp);
                Ok(Self::with_markup(msg, keyboard.as_ref(), reply_kb))
            }
            MessageContent::Photo {
                source,
                caption,
                parse_mode,
                keyboard,
                ..
            } => {
                let msg = self
                    .media_msg(caption.as_deref().unwrap_or(""), *parse_mode, source, true)
                    .await?;
                Ok(Self::with_markup(msg, keyboard.as_ref(), reply_kb))
            }
            MessageContent::Video {
                source,
                caption,
                parse_mode,
                keyboard,
                ..
            }
            | MessageContent::Animation {
                source,
                caption,
                parse_mode,
                keyboard,
                ..
            }
            | MessageContent::Document {
                source,
                caption,
                parse_mode,
                keyboard,
                ..
            } => {
                let msg = self
                    .media_msg(caption.as_deref().unwrap_or(""), *parse_mode, source, false)
                    .await?;
                Ok(Self::with_markup(msg, keyboard.as_ref(), reply_kb))
            }
            MessageContent::Sticker { source } => {
                let msg = self.media_msg("", ParseMode::None, source, false).await?;
                Ok(msg)
            }
            MessageContent::Location {
                latitude,
                longitude,
                keyboard,
            } => {
                let media: tl::enums::InputMedia = tl::types::InputMediaGeoPoint {
                    geo_point: tl::types::InputGeoPoint {
                        lat: *latitude,
                        long: *longitude,
                        accuracy_radius: None,
                    }
                    .into(),
                }
                .into();
                let msg = InputMessage::new().media(media);
                Ok(Self::with_markup(msg, keyboard.as_ref(), reply_kb))
            }
        }
    }

    async fn media_msg(
        &self,
        caption: &str,
        parse_mode: ParseMode,
        source: &FileSource,
        is_photo: bool,
    ) -> Result<InputMessage, ApiError> {
        let base = Self::text_msg(caption, parse_mode, false);
        match source {
            FileSource::Url(url) => Ok(if is_photo {
                base.photo_url(url)
            } else {
                base.document_url(url)
            }),
            FileSource::LocalPath(path) => {
                let uploaded = self
                    .client
                    .upload_file(path)
                    .await
                    .map_err(|e| ApiError::Unknown(format!("upload: {e}")))?;
                Ok(if is_photo {
                    base.photo(uploaded)
                } else {
                    base.document(uploaded)
                })
            }
            FileSource::Bytes { data, filename } => {
                let mut cursor = std::io::Cursor::new(data.clone());
                let uploaded = self
                    .client
                    .upload_stream(&mut cursor, data.len(), filename.clone())
                    .await
                    .map_err(|e| ApiError::Unknown(format!("upload: {e}")))?;
                Ok(if is_photo {
                    base.photo(uploaded)
                } else {
                    base.document(uploaded)
                })
            }
            FileSource::FileId(file_id) => Err(ApiError::Unknown(format!(
                "FileId '{}...' cannot be sent via MTProto. Use FileSource::Url or \
                     FileSource::LocalPath instead, or cache the Uploaded file.",
                &file_id[..file_id.len().min(20)]
            ))),
        }
    }

    // ── Public helpers for Ctx ──

    /// Public wrapper for keyboard conversion (used by ctx.rs for inline edits).
    pub fn to_inline_markup_pub(kb: &InlineKeyboard) -> ReplyMarkup {
        Self::to_inline_markup(kb)
    }

    /// Public wrapper for error conversion.
    pub fn convert_error_pub(e: InvocationError) -> ApiError {
        Self::convert_error(e)
    }

    /// Resolve a ChatId from the shared peer cache.
    pub fn resolve_from_cache(peers: &DashMap<i64, PeerRef>, chat_id: ChatId) -> Option<PeerRef> {
        peers.get(&chat_id.0).map(|r| *r)
    }

    /// Get a reference to the peer cache (for sharing with Ctx).
    pub fn peer_cache(&self) -> Arc<DashMap<i64, PeerRef>> {
        Arc::clone(&self.peers)
    }

    fn tl_action(action: ChatAction) -> tl::enums::SendMessageAction {
        match action {
            ChatAction::Typing => tl::types::SendMessageTypingAction {}.into(),
            ChatAction::UploadPhoto => {
                tl::types::SendMessageUploadPhotoAction { progress: 0 }.into()
            }
            ChatAction::UploadVideo => {
                tl::types::SendMessageUploadVideoAction { progress: 0 }.into()
            }
            ChatAction::UploadDocument => {
                tl::types::SendMessageUploadDocumentAction { progress: 0 }.into()
            }
            ChatAction::FindLocation => tl::types::SendMessageGeoLocationAction {}.into(),
            ChatAction::RecordVoice => tl::types::SendMessageRecordAudioAction {}.into(),
            ChatAction::RecordVideo => tl::types::SendMessageRecordVideoAction {}.into(),
        }
    }
}

// ─── BotApi trait implementation ───
// Each method delegates to the corresponding impl_ method in the sub-module.

#[async_trait]
impl BotApi for GrammersAdapter {
    async fn send_message(
        &self,
        chat_id: ChatId,
        content: MessageContent,
        opts: SendOptions,
    ) -> Result<SentMessage, ApiError> {
        self.impl_send_message(chat_id, content, opts).await
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
        self.impl_edit_message_text(
            chat_id,
            message_id,
            text,
            parse_mode,
            keyboard,
            link_preview,
        )
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
        self.impl_edit_message_caption(chat_id, message_id, caption, parse_mode, keyboard)
            .await
    }
    async fn edit_message_media(
        &self,
        chat_id: ChatId,
        message_id: MessageId,
        content: MessageContent,
        keyboard: Option<InlineKeyboard>,
    ) -> Result<(), ApiError> {
        self.impl_edit_message_media(chat_id, message_id, content, keyboard)
            .await
    }
    async fn edit_message_keyboard(
        &self,
        chat_id: ChatId,
        message_id: MessageId,
        keyboard: Option<InlineKeyboard>,
    ) -> Result<(), ApiError> {
        self.impl_edit_message_keyboard(chat_id, message_id, keyboard)
            .await
    }
    async fn delete_messages(
        &self,
        chat_id: ChatId,
        message_ids: Vec<MessageId>,
    ) -> Result<(), ApiError> {
        self.impl_delete_messages(chat_id, message_ids).await
    }
    async fn answer_callback_query(
        &self,
        id: String,
        text: Option<String>,
        show_alert: bool,
    ) -> Result<(), ApiError> {
        self.impl_answer_callback_query(id, text, show_alert).await
    }
    async fn send_chat_action(&self, chat_id: ChatId, action: ChatAction) -> Result<(), ApiError> {
        self.impl_send_chat_action(chat_id, action).await
    }
    async fn answer_inline_query(
        &self,
        query_id: String,
        results: Vec<InlineQueryResult>,
        next_offset: Option<String>,
        cache_time: Option<i32>,
        is_personal: bool,
    ) -> Result<(), ApiError> {
        self.impl_answer_inline_query(query_id, results, next_offset, cache_time, is_personal)
            .await
    }
    async fn forward_message(
        &self,
        chat_id: ChatId,
        from_chat_id: ChatId,
        message_id: MessageId,
    ) -> Result<SentMessage, ApiError> {
        self.impl_forward_message(chat_id, from_chat_id, message_id)
            .await
    }
    async fn copy_message(
        &self,
        chat_id: ChatId,
        from_chat_id: ChatId,
        message_id: MessageId,
    ) -> Result<MessageId, ApiError> {
        self.impl_copy_message(chat_id, from_chat_id, message_id)
            .await
    }
    async fn download_file(&self, file_id: &str) -> Result<DownloadedFile, ApiError> {
        self.impl_download_file(file_id).await
    }
    async fn send_poll(&self, chat_id: ChatId, poll: SendPoll) -> Result<SentMessage, ApiError> {
        self.impl_send_poll(chat_id, poll).await
    }
    async fn stop_poll(&self, chat_id: ChatId, message_id: MessageId) -> Result<(), ApiError> {
        self.impl_stop_poll(chat_id, message_id).await
    }
    async fn send_dice(&self, chat_id: ChatId, emoji: DiceEmoji) -> Result<SentMessage, ApiError> {
        self.impl_send_dice(chat_id, emoji).await
    }
    async fn send_contact(
        &self,
        chat_id: ChatId,
        contact: Contact,
    ) -> Result<SentMessage, ApiError> {
        self.impl_send_contact(chat_id, contact).await
    }
    async fn send_venue(&self, chat_id: ChatId, venue: Venue) -> Result<SentMessage, ApiError> {
        self.impl_send_venue(chat_id, venue).await
    }
    async fn ban_chat_member(&self, chat_id: ChatId, user_id: UserId) -> Result<(), ApiError> {
        self.impl_ban_chat_member(chat_id, user_id).await
    }
    async fn unban_chat_member(&self, chat_id: ChatId, user_id: UserId) -> Result<(), ApiError> {
        self.impl_unban_chat_member(chat_id, user_id).await
    }
    async fn get_chat_member_count(&self, chat_id: ChatId) -> Result<i32, ApiError> {
        self.impl_get_chat_member_count(chat_id).await
    }
    async fn leave_chat(&self, chat_id: ChatId) -> Result<(), ApiError> {
        self.impl_leave_chat(chat_id).await
    }
    async fn set_my_commands(&self, commands: Vec<BotCommand>) -> Result<(), ApiError> {
        self.impl_set_my_commands(commands).await
    }
    async fn delete_my_commands(&self) -> Result<(), ApiError> {
        self.impl_delete_my_commands().await
    }
    async fn get_me(&self) -> Result<BotInfo, ApiError> {
        self.impl_get_me().await
    }
    async fn pin_chat_message(
        &self,
        chat_id: ChatId,
        message_id: MessageId,
        silent: bool,
    ) -> Result<(), ApiError> {
        self.impl_pin_chat_message(chat_id, message_id, silent)
            .await
    }
    async fn unpin_chat_message(
        &self,
        chat_id: ChatId,
        message_id: MessageId,
    ) -> Result<(), ApiError> {
        self.impl_unpin_chat_message(chat_id, message_id).await
    }
    async fn set_message_reaction(
        &self,
        chat_id: ChatId,
        message_id: MessageId,
        emoji: &str,
    ) -> Result<(), ApiError> {
        self.impl_set_message_reaction(chat_id, message_id, emoji)
            .await
    }
    async fn export_chat_invite_link(&self, chat_id: ChatId) -> Result<String, ApiError> {
        self.impl_export_chat_invite_link(chat_id).await
    }
    async fn answer_pre_checkout_query(
        &self,
        id: String,
        ok: bool,
        error_message: Option<String>,
    ) -> Result<(), ApiError> {
        self.impl_answer_pre_checkout_query(id, ok, error_message)
            .await
    }
    async fn set_chat_title(&self, chat_id: ChatId, title: &str) -> Result<(), ApiError> {
        self.impl_set_chat_title(chat_id, title).await
    }
    async fn set_chat_description(
        &self,
        chat_id: ChatId,
        description: Option<&str>,
    ) -> Result<(), ApiError> {
        self.impl_set_chat_description(chat_id, description).await
    }
    async fn delete_chat_photo(&self, chat_id: ChatId) -> Result<(), ApiError> {
        self.impl_delete_chat_photo(chat_id).await
    }
    async fn get_chat_administrators(&self, chat_id: ChatId) -> Result<Vec<ChatMember>, ApiError> {
        self.impl_get_chat_administrators(chat_id).await
    }
    async fn set_chat_administrator_custom_title(
        &self,
        chat_id: ChatId,
        user_id: UserId,
        custom_title: &str,
    ) -> Result<(), ApiError> {
        self.impl_set_chat_administrator_custom_title(chat_id, user_id, custom_title)
            .await
    }
    async fn approve_chat_join_request(
        &self,
        chat_id: ChatId,
        user_id: UserId,
    ) -> Result<(), ApiError> {
        self.impl_approve_chat_join_request(chat_id, user_id).await
    }
    async fn decline_chat_join_request(
        &self,
        chat_id: ChatId,
        user_id: UserId,
    ) -> Result<(), ApiError> {
        self.impl_decline_chat_join_request(chat_id, user_id).await
    }
    async fn get_user_profile_photos(
        &self,
        user_id: UserId,
        offset: Option<i32>,
        limit: Option<i32>,
    ) -> Result<UserProfilePhotos, ApiError> {
        self.impl_get_user_profile_photos(user_id, offset, limit)
            .await
    }
    async fn get_my_commands(&self) -> Result<Vec<BotCommand>, ApiError> {
        self.impl_get_my_commands().await
    }
    async fn set_my_description(
        &self,
        description: Option<&str>,
        language_code: Option<&str>,
    ) -> Result<(), ApiError> {
        self.impl_set_my_description(description, language_code)
            .await
    }
    async fn get_my_description(
        &self,
        language_code: Option<&str>,
    ) -> Result<BotDescription, ApiError> {
        self.impl_get_my_description(language_code).await
    }
    async fn set_my_short_description(
        &self,
        short_description: Option<&str>,
        language_code: Option<&str>,
    ) -> Result<(), ApiError> {
        self.impl_set_my_short_description(short_description, language_code)
            .await
    }
    async fn get_my_short_description(
        &self,
        language_code: Option<&str>,
    ) -> Result<BotShortDescription, ApiError> {
        self.impl_get_my_short_description(language_code).await
    }
    async fn set_my_name(
        &self,
        name: Option<&str>,
        language_code: Option<&str>,
    ) -> Result<(), ApiError> {
        self.impl_set_my_name(name, language_code).await
    }
    async fn get_my_name(&self, language_code: Option<&str>) -> Result<BotName, ApiError> {
        self.impl_get_my_name(language_code).await
    }
    async fn set_chat_menu_button(
        &self,
        chat_id: Option<ChatId>,
        menu_button: MenuButton,
    ) -> Result<(), ApiError> {
        self.impl_set_chat_menu_button(chat_id, menu_button).await
    }
    async fn get_chat_menu_button(&self, chat_id: Option<ChatId>) -> Result<MenuButton, ApiError> {
        self.impl_get_chat_menu_button(chat_id).await
    }
    async fn forward_messages(
        &self,
        chat_id: ChatId,
        from_chat_id: ChatId,
        message_ids: Vec<MessageId>,
    ) -> Result<Vec<MessageId>, ApiError> {
        self.impl_forward_messages(chat_id, from_chat_id, message_ids)
            .await
    }
    async fn copy_messages(
        &self,
        chat_id: ChatId,
        from_chat_id: ChatId,
        message_ids: Vec<MessageId>,
    ) -> Result<Vec<MessageId>, ApiError> {
        self.impl_copy_messages(chat_id, from_chat_id, message_ids)
            .await
    }
    async fn send_sticker(
        &self,
        chat_id: ChatId,
        sticker: FileSource,
    ) -> Result<SentMessage, ApiError> {
        self.impl_send_sticker(chat_id, sticker).await
    }
    async fn send_location(
        &self,
        chat_id: ChatId,
        latitude: f64,
        longitude: f64,
    ) -> Result<SentMessage, ApiError> {
        self.impl_send_location(chat_id, latitude, longitude).await
    }
    async fn create_forum_topic(
        &self,
        chat_id: ChatId,
        title: &str,
        icon_color: Option<i32>,
        icon_custom_emoji_id: Option<i64>,
    ) -> Result<ForumTopic, ApiError> {
        self.impl_create_forum_topic(chat_id, title, icon_color, icon_custom_emoji_id)
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
        self.impl_edit_forum_topic(
            chat_id,
            topic_id,
            title,
            icon_custom_emoji_id,
            closed,
            hidden,
        )
        .await
    }
    async fn delete_forum_topic(&self, chat_id: ChatId, topic_id: i32) -> Result<(), ApiError> {
        self.impl_delete_forum_topic(chat_id, topic_id).await
    }
    async fn unpin_all_forum_topic_messages(
        &self,
        chat_id: ChatId,
        topic_id: i32,
    ) -> Result<(), ApiError> {
        self.impl_unpin_all_forum_topic_messages(chat_id, topic_id)
            .await
    }
    async fn get_star_transactions(
        &self,
        offset: Option<&str>,
        limit: Option<i32>,
    ) -> Result<StarTransactions, ApiError> {
        self.impl_get_star_transactions(offset, limit).await
    }
    async fn refund_star_payment(&self, user_id: UserId, charge_id: &str) -> Result<(), ApiError> {
        self.impl_refund_star_payment(user_id, charge_id).await
    }
}
