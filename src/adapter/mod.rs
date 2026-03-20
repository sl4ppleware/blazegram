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
pub(crate) mod helpers;
mod media;
mod send;
mod settings;
mod stars;

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
    /// Create a new adapter wrapping the given client.
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

impl_adapter_botapi! {
    delegate: [
        fn send_message => impl_send_message(chat_id: ChatId, content: MessageContent, opts: SendOptions) -> SentMessage;
        fn edit_message_text => impl_edit_message_text(chat_id: ChatId, message_id: MessageId, text: String, parse_mode: ParseMode, keyboard: Option<InlineKeyboard>, link_preview: bool) -> ();
        fn edit_message_caption => impl_edit_message_caption(chat_id: ChatId, message_id: MessageId, caption: Option<String>, parse_mode: ParseMode, keyboard: Option<InlineKeyboard>) -> ();
        fn edit_message_media => impl_edit_message_media(chat_id: ChatId, message_id: MessageId, content: MessageContent, keyboard: Option<InlineKeyboard>) -> ();
        fn edit_message_keyboard => impl_edit_message_keyboard(chat_id: ChatId, message_id: MessageId, keyboard: Option<InlineKeyboard>) -> ();
        fn delete_messages => impl_delete_messages(chat_id: ChatId, message_ids: Vec<MessageId>) -> ();
        fn answer_callback_query => impl_answer_callback_query(id: String, text: Option<String>, show_alert: bool) -> ();
        fn send_chat_action => impl_send_chat_action(chat_id: ChatId, action: ChatAction) -> ();
        fn answer_inline_query => impl_answer_inline_query(query_id: String, results: Vec<InlineQueryResult>, next_offset: Option<String>, cache_time: Option<i32>, is_personal: bool) -> ();
        fn forward_message => impl_forward_message(chat_id: ChatId, from_chat_id: ChatId, message_id: MessageId) -> SentMessage;
        fn copy_message => impl_copy_message(chat_id: ChatId, from_chat_id: ChatId, message_id: MessageId) -> MessageId;
        fn download_file => impl_download_file(file_id: &str) -> DownloadedFile;
        fn send_poll => impl_send_poll(chat_id: ChatId, poll: SendPoll) -> SentMessage;
        fn stop_poll => impl_stop_poll(chat_id: ChatId, message_id: MessageId) -> ();
        fn send_dice => impl_send_dice(chat_id: ChatId, emoji: DiceEmoji) -> SentMessage;
        fn send_contact => impl_send_contact(chat_id: ChatId, contact: Contact) -> SentMessage;
        fn send_venue => impl_send_venue(chat_id: ChatId, venue: Venue) -> SentMessage;
        fn ban_chat_member => impl_ban_chat_member(chat_id: ChatId, user_id: UserId) -> ();
        fn unban_chat_member => impl_unban_chat_member(chat_id: ChatId, user_id: UserId) -> ();
        fn get_chat_member_count => impl_get_chat_member_count(chat_id: ChatId) -> i32;
        fn leave_chat => impl_leave_chat(chat_id: ChatId) -> ();
        fn set_my_commands => impl_set_my_commands(commands: Vec<BotCommand>) -> ();
        fn delete_my_commands => impl_delete_my_commands() -> ();
        fn get_me => impl_get_me() -> BotInfo;
        fn set_message_reaction => impl_set_message_reaction(chat_id: ChatId, message_id: MessageId, emoji: &str) -> ();
        fn export_chat_invite_link => impl_export_chat_invite_link(chat_id: ChatId) -> String;
        fn answer_pre_checkout_query => impl_answer_pre_checkout_query(id: String, ok: bool, error_message: Option<String>) -> ();
        fn set_chat_title => impl_set_chat_title(chat_id: ChatId, title: &str) -> ();
        fn set_chat_description => impl_set_chat_description(chat_id: ChatId, description: Option<&str>) -> ();
        fn delete_chat_photo => impl_delete_chat_photo(chat_id: ChatId) -> ();
        fn get_chat_administrators => impl_get_chat_administrators(chat_id: ChatId) -> Vec<ChatMember>;
        fn set_chat_administrator_custom_title => impl_set_chat_administrator_custom_title(chat_id: ChatId, user_id: UserId, custom_title: &str) -> ();
        fn approve_chat_join_request => impl_approve_chat_join_request(chat_id: ChatId, user_id: UserId) -> ();
        fn decline_chat_join_request => impl_decline_chat_join_request(chat_id: ChatId, user_id: UserId) -> ();
        fn get_user_profile_photos => impl_get_user_profile_photos(user_id: UserId, offset: Option<i32>, limit: Option<i32>) -> UserProfilePhotos;
        fn get_my_commands => impl_get_my_commands() -> Vec<BotCommand>;
        fn set_my_description => impl_set_my_description(description: Option<&str>, language_code: Option<&str>) -> ();
        fn get_my_description => impl_get_my_description(language_code: Option<&str>) -> BotDescription;
        fn set_my_short_description => impl_set_my_short_description(short_description: Option<&str>, language_code: Option<&str>) -> ();
        fn get_my_short_description => impl_get_my_short_description(language_code: Option<&str>) -> BotShortDescription;
        fn set_my_name => impl_set_my_name(name: Option<&str>, language_code: Option<&str>) -> ();
        fn get_my_name => impl_get_my_name(language_code: Option<&str>) -> BotName;
        fn set_chat_menu_button => impl_set_chat_menu_button(chat_id: Option<ChatId>, menu_button: MenuButton) -> ();
        fn get_chat_menu_button => impl_get_chat_menu_button(chat_id: Option<ChatId>) -> MenuButton;
        fn forward_messages => impl_forward_messages(chat_id: ChatId, from_chat_id: ChatId, message_ids: Vec<MessageId>) -> Vec<MessageId>;
        fn copy_messages => impl_copy_messages(chat_id: ChatId, from_chat_id: ChatId, message_ids: Vec<MessageId>) -> Vec<MessageId>;
        fn send_sticker => impl_send_sticker(chat_id: ChatId, sticker: FileSource) -> SentMessage;
        fn send_location => impl_send_location(chat_id: ChatId, latitude: f64, longitude: f64) -> SentMessage;
        fn create_forum_topic => impl_create_forum_topic(chat_id: ChatId, title: &str, icon_color: Option<i32>, icon_custom_emoji_id: Option<i64>) -> ForumTopic;
        fn edit_forum_topic => impl_edit_forum_topic(chat_id: ChatId, topic_id: i32, title: Option<&str>, icon_custom_emoji_id: Option<i64>, closed: Option<bool>, hidden: Option<bool>) -> ();
        fn delete_forum_topic => impl_delete_forum_topic(chat_id: ChatId, topic_id: i32) -> ();
        fn unpin_all_forum_topic_messages => impl_unpin_all_forum_topic_messages(chat_id: ChatId, topic_id: i32) -> ();
        fn get_star_transactions => impl_get_star_transactions(offset: Option<&str>, limit: Option<i32>) -> StarTransactions;
        fn refund_star_payment => impl_refund_star_payment(user_id: UserId, charge_id: &str) -> ();
        fn send_media_group => impl_send_media_group(chat_id: ChatId, media: Vec<MediaGroupItem>) -> Vec<SentMessage>;
        fn send_invoice => impl_send_invoice(chat_id: ChatId, invoice: Invoice) -> SentMessage;
        fn get_chat_member => impl_get_chat_member(chat_id: ChatId, user_id: UserId) -> ChatMember;
        fn get_chat => impl_get_chat(chat_id: ChatId) -> ChatInfo;
        fn set_chat_photo => impl_set_chat_photo(chat_id: ChatId, photo: FileSource) -> ();
        fn unpin_all_chat_messages => impl_unpin_all_chat_messages(chat_id: ChatId) -> ();
        fn create_chat_invite_link => impl_create_chat_invite_link(chat_id: ChatId, name: Option<&str>, expire_date: Option<i64>, member_limit: Option<i32>) -> String;
        fn revoke_chat_invite_link => impl_revoke_chat_invite_link(chat_id: ChatId, invite_link: &str) -> ChatInviteLink;
        fn answer_shipping_query => impl_answer_shipping_query(shipping_query_id: String, ok: bool, shipping_options: Option<Vec<ShippingOption>>, error_message: Option<String>) -> ();
        fn create_invoice_link => impl_create_invoice_link(invoice: Invoice) -> String;
    ]
    manual: {
        // These pass `&permissions` instead of owned `permissions`
        async fn restrict_chat_member(&self, chat_id: ChatId, user_id: UserId, permissions: ChatPermissions) -> Result<(), ApiError> {
            self.impl_restrict_chat_member(chat_id, user_id, &permissions).await
        }
        async fn promote_chat_member(&self, chat_id: ChatId, user_id: UserId, permissions: ChatPermissions) -> Result<(), ApiError> {
            self.impl_promote_chat_member(chat_id, user_id, &permissions).await
        }
        async fn set_chat_permissions(&self, chat_id: ChatId, permissions: ChatPermissions) -> Result<(), ApiError> {
            self.impl_set_chat_permissions(chat_id, &permissions).await
        }
        // pin/unpin pass silent/message_id differently
        async fn pin_chat_message(&self, chat_id: ChatId, message_id: MessageId, silent: bool) -> Result<(), ApiError> {
            self.impl_pin_chat_message(chat_id, message_id, silent).await
        }
        async fn unpin_chat_message(&self, chat_id: ChatId, message_id: MessageId) -> Result<(), ApiError> {
            self.impl_unpin_chat_message(chat_id, message_id).await
        }
    }
}
