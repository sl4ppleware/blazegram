use crate::error::ApiError;
use crate::keyboard::InlineKeyboard;
use crate::screen::ReplyKeyboardAction;
use crate::types::*;
use async_trait::async_trait;

/// Suppress unused-variable warnings and return a "not implemented" error.
macro_rules! not_implemented {
    ($name:expr, $($arg:expr),* $(,)?) => {{
        let _ = ($($arg,)*);
        Err(ApiError::Unknown(concat!($name, " not implemented").into()))
    }};
}

/// Options for sending a new message.
#[derive(Debug, Clone, Default)]
pub struct SendOptions {
    /// Forbid forwarding/saving.
    pub protect_content: bool,
    /// Reply (bottom) keyboard action.
    pub reply_keyboard: Option<ReplyKeyboardAction>,
    /// Reply to a specific message.
    pub reply_to: Option<MessageId>,
    /// Forum topic ID (message_thread_id). When set, the message is sent to that topic.
    pub message_thread_id: Option<i32>,
}

/// The full Telegram Bot API abstraction.
///
/// Core methods are required (no default). Optional methods return `ApiError::Unknown("not implemented")` by default.
/// The core methods (send_message, edit_*, delete_*, answer_callback_query, send_chat_action,
/// answer_inline_query) are required. Everything else is opt-in — implement what you need.
#[async_trait]
pub trait BotApi: Send + Sync + 'static {
    // ─── Core (required) ───

    /// Send a message with the given content and options.
    async fn send_message(
        &self,
        chat_id: ChatId,
        content: MessageContent,
        opts: SendOptions,
    ) -> Result<SentMessage, ApiError>;

    /// Edit a text message in place.
    async fn edit_message_text(
        &self,
        chat_id: ChatId,
        message_id: MessageId,
        text: String,
        parse_mode: ParseMode,
        keyboard: Option<InlineKeyboard>,
        link_preview: bool,
    ) -> Result<(), ApiError>;

    /// Edit the caption of a media message.
    async fn edit_message_caption(
        &self,
        chat_id: ChatId,
        message_id: MessageId,
        caption: Option<String>,
        parse_mode: ParseMode,
        keyboard: Option<InlineKeyboard>,
    ) -> Result<(), ApiError>;

    /// Replace the media of a message (photo ↔ video ↔ document ↔ animation).
    async fn edit_message_media(
        &self,
        chat_id: ChatId,
        message_id: MessageId,
        content: MessageContent,
        keyboard: Option<InlineKeyboard>,
    ) -> Result<(), ApiError>;

    /// Replace only the inline keyboard of a message.
    async fn edit_message_keyboard(
        &self,
        chat_id: ChatId,
        message_id: MessageId,
        keyboard: Option<InlineKeyboard>,
    ) -> Result<(), ApiError>;

    /// Delete one or more messages by ID.
    async fn delete_messages(
        &self,
        chat_id: ChatId,
        message_ids: Vec<MessageId>,
    ) -> Result<(), ApiError>;

    /// Answer a callback query (dismiss the loading spinner on the button).
    async fn answer_callback_query(
        &self,
        id: String,
        text: Option<String>,
        show_alert: bool,
    ) -> Result<(), ApiError>;

    /// Send a chat action ("typing…", "uploading photo…", etc.).
    async fn send_chat_action(&self, chat_id: ChatId, action: ChatAction) -> Result<(), ApiError>;

    /// Answer an inline query with a list of results.
    async fn answer_inline_query(
        &self,
        query_id: String,
        results: Vec<InlineQueryResult>,
        next_offset: Option<String>,
        cache_time: Option<i32>,
        is_personal: bool,
    ) -> Result<(), ApiError>;

    // ─── Forwarding & Copying ───

    /// Forward a message from one chat to another.
    async fn forward_message(
        &self,
        chat_id: ChatId,
        from_chat_id: ChatId,
        message_id: MessageId,
    ) -> Result<SentMessage, ApiError> {
        not_implemented!("forward_message", chat_id, from_chat_id, message_id)
    }

    /// Copy a message (re-send without "Forwarded from" header).
    async fn copy_message(
        &self,
        chat_id: ChatId,
        from_chat_id: ChatId,
        message_id: MessageId,
    ) -> Result<MessageId, ApiError> {
        not_implemented!("copy_message", chat_id, from_chat_id, message_id)
    }

    // ─── Media ───

    /// Send a group of photos/videos/documents as an album.
    async fn send_media_group(
        &self,
        chat_id: ChatId,
        media: Vec<MediaGroupItem>,
    ) -> Result<Vec<SentMessage>, ApiError> {
        not_implemented!("send_media_group", chat_id, media)
    }

    /// Download a file by its file_id. Returns a `DownloadedFile` with raw bytes and optional size.
    async fn download_file(&self, file_id: &str) -> Result<DownloadedFile, ApiError> {
        not_implemented!("download_file", file_id)
    }

    // ─── Fun & Interactive ───

    /// Send a poll.
    async fn send_poll(&self, chat_id: ChatId, poll: SendPoll) -> Result<SentMessage, ApiError> {
        not_implemented!("send_poll", chat_id, poll)
    }

    /// Stop a poll.
    async fn stop_poll(&self, chat_id: ChatId, message_id: MessageId) -> Result<(), ApiError> {
        not_implemented!("stop_poll", chat_id, message_id)
    }

    /// Send a dice animation.
    async fn send_dice(&self, chat_id: ChatId, emoji: DiceEmoji) -> Result<SentMessage, ApiError> {
        not_implemented!("send_dice", chat_id, emoji)
    }

    /// Send a contact.
    async fn send_contact(
        &self,
        chat_id: ChatId,
        contact: Contact,
    ) -> Result<SentMessage, ApiError> {
        not_implemented!("send_contact", chat_id, contact)
    }

    /// Send a venue.
    async fn send_venue(&self, chat_id: ChatId, venue: Venue) -> Result<SentMessage, ApiError> {
        not_implemented!("send_venue", chat_id, venue)
    }

    // ─── Payments ───

    /// Send an invoice for payment.
    async fn send_invoice(
        &self,
        chat_id: ChatId,
        invoice: Invoice,
    ) -> Result<SentMessage, ApiError> {
        not_implemented!("send_invoice", chat_id, invoice)
    }

    /// Answer a pre-checkout query (approve or decline).
    async fn answer_pre_checkout_query(
        &self,
        id: String,
        ok: bool,
        error_message: Option<String>,
    ) -> Result<(), ApiError> {
        not_implemented!("answer_pre_checkout_query", id, ok, error_message)
    }

    // ─── Chat Administration ───

    /// Ban a user from a chat.
    async fn ban_chat_member(&self, chat_id: ChatId, user_id: UserId) -> Result<(), ApiError> {
        not_implemented!("ban_chat_member", chat_id, user_id)
    }

    /// Unban a previously banned user.
    async fn unban_chat_member(&self, chat_id: ChatId, user_id: UserId) -> Result<(), ApiError> {
        not_implemented!("unban_chat_member", chat_id, user_id)
    }

    /// Restrict a user (set permissions).
    async fn restrict_chat_member(
        &self,
        chat_id: ChatId,
        user_id: UserId,
        permissions: ChatPermissions,
    ) -> Result<(), ApiError> {
        not_implemented!("restrict_chat_member", chat_id, user_id, permissions)
    }

    /// Promote a user to admin.
    async fn promote_chat_member(
        &self,
        chat_id: ChatId,
        user_id: UserId,
        permissions: ChatPermissions,
    ) -> Result<(), ApiError> {
        not_implemented!("promote_chat_member", chat_id, user_id, permissions)
    }

    /// Get info about a chat member.
    async fn get_chat_member(
        &self,
        chat_id: ChatId,
        user_id: UserId,
    ) -> Result<ChatMember, ApiError> {
        not_implemented!("get_chat_member", chat_id, user_id)
    }

    /// Get the number of members in a chat.
    async fn get_chat_member_count(&self, chat_id: ChatId) -> Result<i32, ApiError> {
        not_implemented!("get_chat_member_count", chat_id)
    }

    /// Get chat info.
    async fn get_chat(&self, chat_id: ChatId) -> Result<ChatInfo, ApiError> {
        not_implemented!("get_chat", chat_id)
    }

    /// Leave a chat.
    async fn leave_chat(&self, chat_id: ChatId) -> Result<(), ApiError> {
        not_implemented!("leave_chat", chat_id)
    }

    /// Set chat permissions for all members.
    async fn set_chat_permissions(
        &self,
        chat_id: ChatId,
        permissions: ChatPermissions,
    ) -> Result<(), ApiError> {
        not_implemented!("set_chat_permissions", chat_id, permissions)
    }

    // ─── Bot Settings ───

    /// Set the bot's command list.
    async fn set_my_commands(&self, commands: Vec<BotCommand>) -> Result<(), ApiError> {
        not_implemented!("set_my_commands", commands)
    }

    /// Delete the bot's command list.
    async fn delete_my_commands(&self) -> Result<(), ApiError> {
        not_implemented!("delete_my_commands",)
    }

    /// Get bot info (id, username, etc).
    async fn get_me(&self) -> Result<BotInfo, ApiError> {
        not_implemented!("get_me",)
    }

    // ─── Reactions ───

    /// Set a reaction on a message.
    async fn set_message_reaction(
        &self,
        chat_id: ChatId,
        message_id: MessageId,
        emoji: &str,
    ) -> Result<(), ApiError> {
        not_implemented!("set_message_reaction", chat_id, message_id, emoji)
    }

    // ─── Pinning ───

    /// Pin a message in a chat.
    async fn pin_chat_message(
        &self,
        chat_id: ChatId,
        message_id: MessageId,
        silent: bool,
    ) -> Result<(), ApiError> {
        not_implemented!("pin_chat_message", chat_id, message_id, silent)
    }

    /// Unpin a message in a chat.
    async fn unpin_chat_message(
        &self,
        chat_id: ChatId,
        message_id: MessageId,
    ) -> Result<(), ApiError> {
        not_implemented!("unpin_chat_message", chat_id, message_id)
    }

    /// Unpin all messages in a chat.
    async fn unpin_all_chat_messages(&self, chat_id: ChatId) -> Result<(), ApiError> {
        not_implemented!("unpin_all_chat_messages", chat_id)
    }

    // ─── Invite Links ───

    /// Create a chat invite link.
    async fn create_chat_invite_link(
        &self,
        chat_id: ChatId,
        name: Option<&str>,
        expire_date: Option<i64>,
        member_limit: Option<i32>,
    ) -> Result<String, ApiError> {
        not_implemented!(
            "create_chat_invite_link",
            chat_id,
            name,
            expire_date,
            member_limit
        )
    }

    /// Export the primary chat invite link.
    async fn export_chat_invite_link(&self, chat_id: ChatId) -> Result<String, ApiError> {
        not_implemented!("export_chat_invite_link", chat_id)
    }

    /// Revoke a chat invite link.
    async fn revoke_chat_invite_link(
        &self,
        chat_id: ChatId,
        invite_link: &str,
    ) -> Result<ChatInviteLink, ApiError> {
        not_implemented!("revoke_chat_invite_link", chat_id, invite_link)
    }

    // ─── Chat Join Requests ───

    /// Approve a chat join request.
    async fn approve_chat_join_request(
        &self,
        chat_id: ChatId,
        user_id: UserId,
    ) -> Result<(), ApiError> {
        not_implemented!("approve_chat_join_request", chat_id, user_id)
    }

    /// Decline a chat join request.
    async fn decline_chat_join_request(
        &self,
        chat_id: ChatId,
        user_id: UserId,
    ) -> Result<(), ApiError> {
        not_implemented!("decline_chat_join_request", chat_id, user_id)
    }

    // ─── Chat Management ───

    /// Set the chat title.
    async fn set_chat_title(&self, chat_id: ChatId, title: &str) -> Result<(), ApiError> {
        not_implemented!("set_chat_title", chat_id, title)
    }

    /// Set the chat description.
    async fn set_chat_description(
        &self,
        chat_id: ChatId,
        description: Option<&str>,
    ) -> Result<(), ApiError> {
        not_implemented!("set_chat_description", chat_id, description)
    }

    /// Set the chat photo.
    async fn set_chat_photo(&self, chat_id: ChatId, photo: FileSource) -> Result<(), ApiError> {
        not_implemented!("set_chat_photo", chat_id, photo)
    }

    /// Delete the chat photo.
    async fn delete_chat_photo(&self, chat_id: ChatId) -> Result<(), ApiError> {
        not_implemented!("delete_chat_photo", chat_id)
    }

    /// Get the list of chat administrators.
    async fn get_chat_administrators(&self, chat_id: ChatId) -> Result<Vec<ChatMember>, ApiError> {
        not_implemented!("get_chat_administrators", chat_id)
    }

    /// Set a custom title for an admin in a supergroup.
    async fn set_chat_administrator_custom_title(
        &self,
        chat_id: ChatId,
        user_id: UserId,
        custom_title: &str,
    ) -> Result<(), ApiError> {
        not_implemented!(
            "set_chat_administrator_custom_title",
            chat_id,
            user_id,
            custom_title
        )
    }

    // ─── User Info ───

    /// Get a user's profile photos.
    async fn get_user_profile_photos(
        &self,
        user_id: UserId,
        offset: Option<i32>,
        limit: Option<i32>,
    ) -> Result<UserProfilePhotos, ApiError> {
        not_implemented!("get_user_profile_photos", user_id, offset, limit)
    }

    // ─── Bot Settings (extended) ───

    /// Get the bot's command list.
    async fn get_my_commands(&self) -> Result<Vec<BotCommand>, ApiError> {
        not_implemented!("get_my_commands",)
    }

    /// Set the bot's description.
    async fn set_my_description(
        &self,
        description: Option<&str>,
        language_code: Option<&str>,
    ) -> Result<(), ApiError> {
        not_implemented!("set_my_description", description, language_code)
    }

    /// Get the bot's description.
    async fn get_my_description(
        &self,
        language_code: Option<&str>,
    ) -> Result<BotDescription, ApiError> {
        not_implemented!("get_my_description", language_code)
    }

    /// Set the bot's short description.
    async fn set_my_short_description(
        &self,
        short_description: Option<&str>,
        language_code: Option<&str>,
    ) -> Result<(), ApiError> {
        not_implemented!("set_my_short_description", short_description, language_code)
    }

    /// Get the bot's short description.
    async fn get_my_short_description(
        &self,
        language_code: Option<&str>,
    ) -> Result<BotShortDescription, ApiError> {
        not_implemented!("get_my_short_description", language_code)
    }

    /// Set the bot's name.
    async fn set_my_name(
        &self,
        name: Option<&str>,
        language_code: Option<&str>,
    ) -> Result<(), ApiError> {
        not_implemented!("set_my_name", name, language_code)
    }

    /// Get the bot's name.
    async fn get_my_name(&self, language_code: Option<&str>) -> Result<BotName, ApiError> {
        not_implemented!("get_my_name", language_code)
    }

    // ─── Menu Button ───

    /// Set the bot's menu button for a specific chat or default.
    async fn set_chat_menu_button(
        &self,
        chat_id: Option<ChatId>,
        menu_button: MenuButton,
    ) -> Result<(), ApiError> {
        not_implemented!("set_chat_menu_button", chat_id, menu_button)
    }

    /// Get the bot's menu button for a specific chat or default.
    async fn get_chat_menu_button(&self, chat_id: Option<ChatId>) -> Result<MenuButton, ApiError> {
        not_implemented!("get_chat_menu_button", chat_id)
    }

    // ─── Payments (extended) ───

    /// Answer a shipping query (for flexible pricing invoices).
    async fn answer_shipping_query(
        &self,
        shipping_query_id: String,
        ok: bool,
        shipping_options: Option<Vec<ShippingOption>>,
        error_message: Option<String>,
    ) -> Result<(), ApiError> {
        not_implemented!(
            "answer_shipping_query",
            shipping_query_id,
            ok,
            shipping_options,
            error_message
        )
    }

    /// Create an invoice link for payments without sending a message.
    async fn create_invoice_link(&self, invoice: Invoice) -> Result<String, ApiError> {
        not_implemented!("create_invoice_link", invoice)
    }

    // ─── Batch Operations ───

    /// Forward multiple messages at once.
    async fn forward_messages(
        &self,
        chat_id: ChatId,
        from_chat_id: ChatId,
        message_ids: Vec<MessageId>,
    ) -> Result<Vec<MessageId>, ApiError> {
        not_implemented!("forward_messages", chat_id, from_chat_id, message_ids)
    }

    /// Copy multiple messages at once.
    async fn copy_messages(
        &self,
        chat_id: ChatId,
        from_chat_id: ChatId,
        message_ids: Vec<MessageId>,
    ) -> Result<Vec<MessageId>, ApiError> {
        not_implemented!("copy_messages", chat_id, from_chat_id, message_ids)
    }

    // ─── Sticker ───

    /// Send a sticker (convenience — also available via send_message with MessageContent::Sticker).
    async fn send_sticker(
        &self,
        chat_id: ChatId,
        sticker: FileSource,
    ) -> Result<SentMessage, ApiError> {
        not_implemented!("send_sticker", chat_id, sticker)
    }

    // ─── Location ───

    /// Send a location.
    async fn send_location(
        &self,
        chat_id: ChatId,
        latitude: f64,
        longitude: f64,
    ) -> Result<SentMessage, ApiError> {
        not_implemented!("send_location", chat_id, latitude, longitude)
    }

    // ─── Forum Topics ───

    /// Create a forum topic in a supergroup.
    async fn create_forum_topic(
        &self,
        chat_id: ChatId,
        title: &str,
        icon_color: Option<i32>,
        icon_custom_emoji_id: Option<i64>,
    ) -> Result<ForumTopic, ApiError> {
        not_implemented!(
            "create_forum_topic",
            chat_id,
            title,
            icon_color,
            icon_custom_emoji_id
        )
    }

    /// Edit a forum topic (title, icon, open/close, hide/show).
    async fn edit_forum_topic(
        &self,
        chat_id: ChatId,
        topic_id: i32,
        title: Option<&str>,
        icon_custom_emoji_id: Option<i64>,
        closed: Option<bool>,
        hidden: Option<bool>,
    ) -> Result<(), ApiError> {
        not_implemented!(
            "edit_forum_topic",
            chat_id,
            topic_id,
            title,
            icon_custom_emoji_id,
            closed,
            hidden
        )
    }

    /// Close a forum topic.
    async fn close_forum_topic(&self, chat_id: ChatId, topic_id: i32) -> Result<(), ApiError> {
        self.edit_forum_topic(chat_id, topic_id, None, None, Some(true), None)
            .await
    }

    /// Reopen a forum topic.
    async fn reopen_forum_topic(&self, chat_id: ChatId, topic_id: i32) -> Result<(), ApiError> {
        self.edit_forum_topic(chat_id, topic_id, None, None, Some(false), None)
            .await
    }

    /// Delete a forum topic and all its messages.
    async fn delete_forum_topic(&self, chat_id: ChatId, topic_id: i32) -> Result<(), ApiError> {
        not_implemented!("delete_forum_topic", chat_id, topic_id)
    }

    /// Unpin all messages in a forum topic.
    async fn unpin_all_forum_topic_messages(
        &self,
        chat_id: ChatId,
        topic_id: i32,
    ) -> Result<(), ApiError> {
        not_implemented!("unpin_all_forum_topic_messages", chat_id, topic_id)
    }

    /// Hide the 'General' topic in a forum supergroup.
    async fn hide_general_forum_topic(&self, chat_id: ChatId) -> Result<(), ApiError> {
        // General topic id = 1, hidden = true
        self.edit_forum_topic(chat_id, 1, None, None, None, Some(true))
            .await
    }

    /// Unhide the 'General' topic in a forum supergroup.
    async fn unhide_general_forum_topic(&self, chat_id: ChatId) -> Result<(), ApiError> {
        self.edit_forum_topic(chat_id, 1, None, None, None, Some(false))
            .await
    }

    // ─── Stars API ───

    /// Get star transaction history for the bot.
    async fn get_star_transactions(
        &self,
        offset: Option<&str>,
        limit: Option<i32>,
    ) -> Result<StarTransactions, ApiError> {
        not_implemented!("get_star_transactions", offset, limit)
    }

    /// Refund a star payment by charge_id.
    async fn refund_star_payment(&self, user_id: UserId, charge_id: &str) -> Result<(), ApiError> {
        not_implemented!("refund_star_payment", user_id, charge_id)
    }

    // ─── Core Utility ───

    /// Log out from the cloud Bot API server before launching the bot locally.
    /// After a successful call, you can immediately log in on a local server, but
    /// will not be able to log in back to the cloud Bot API server for 10 minutes.
    async fn log_out(&self) -> Result<(), ApiError> {
        not_implemented!("log_out",)
    }

    /// Close the bot instance before moving it from one local server to another.
    /// The method will return error 429 in the first 10 minutes after the bot is launched.
    async fn close(&self) -> Result<(), ApiError> {
        not_implemented!("close",)
    }

    // ─── Convenience Media Sending ───
    //
    // These have DEFAULT implementations that build MessageContent and delegate
    // to self.send_message(), so any BotApi implementor gets them for free.

    /// Send a photo. Convenience wrapper around [`send_message`] with [`MessageContent::Photo`].
    async fn send_photo(
        &self,
        chat_id: ChatId,
        photo: FileSource,
        caption: Option<String>,
        parse_mode: ParseMode,
        opts: SendOptions,
    ) -> Result<SentMessage, ApiError> {
        self.send_message(
            chat_id,
            MessageContent::Photo {
                source: photo,
                caption,
                parse_mode,
                keyboard: None,
                spoiler: false,
            },
            opts,
        )
        .await
    }

    /// Send an audio file. Convenience wrapper around [`send_message`].
    ///
    /// Note: delegates to `MessageContent::Document` until a dedicated Audio variant is added.
    async fn send_audio(
        &self,
        chat_id: ChatId,
        audio: FileSource,
        caption: Option<String>,
        parse_mode: ParseMode,
        opts: SendOptions,
    ) -> Result<SentMessage, ApiError> {
        self.send_message(
            chat_id,
            MessageContent::Document {
                source: audio,
                caption,
                parse_mode,
                keyboard: None,
                filename: None,
            },
            opts,
        )
        .await
    }

    /// Send a document. Convenience wrapper around [`send_message`] with [`MessageContent::Document`].
    async fn send_document(
        &self,
        chat_id: ChatId,
        document: FileSource,
        caption: Option<String>,
        parse_mode: ParseMode,
        opts: SendOptions,
    ) -> Result<SentMessage, ApiError> {
        self.send_message(
            chat_id,
            MessageContent::Document {
                source: document,
                caption,
                parse_mode,
                keyboard: None,
                filename: None,
            },
            opts,
        )
        .await
    }

    /// Send a video. Convenience wrapper around [`send_message`] with [`MessageContent::Video`].
    async fn send_video(
        &self,
        chat_id: ChatId,
        video: FileSource,
        caption: Option<String>,
        parse_mode: ParseMode,
        opts: SendOptions,
    ) -> Result<SentMessage, ApiError> {
        self.send_message(
            chat_id,
            MessageContent::Video {
                source: video,
                caption,
                parse_mode,
                keyboard: None,
                spoiler: false,
            },
            opts,
        )
        .await
    }

    /// Send an animation (GIF / MPEG4). Convenience wrapper around [`send_message`] with [`MessageContent::Animation`].
    async fn send_animation(
        &self,
        chat_id: ChatId,
        animation: FileSource,
        caption: Option<String>,
        parse_mode: ParseMode,
        opts: SendOptions,
    ) -> Result<SentMessage, ApiError> {
        self.send_message(
            chat_id,
            MessageContent::Animation {
                source: animation,
                caption,
                parse_mode,
                keyboard: None,
                spoiler: false,
            },
            opts,
        )
        .await
    }

    /// Send a voice message (OGG Opus). Convenience wrapper that delegates to [`send_message`].
    ///
    /// Note: delegates to `MessageContent::Document` until a dedicated Voice variant is added.
    async fn send_voice(
        &self,
        chat_id: ChatId,
        voice: FileSource,
        caption: Option<String>,
        parse_mode: ParseMode,
        opts: SendOptions,
    ) -> Result<SentMessage, ApiError> {
        self.send_message(
            chat_id,
            MessageContent::Document {
                source: voice,
                caption,
                parse_mode,
                keyboard: None,
                filename: None,
            },
            opts,
        )
        .await
    }

    /// Send a video note (round video). Convenience wrapper that delegates to [`send_message`].
    ///
    /// Note: delegates to `MessageContent::Document` until a dedicated VideoNote variant is added.
    async fn send_video_note(
        &self,
        chat_id: ChatId,
        video_note: FileSource,
        opts: SendOptions,
    ) -> Result<SentMessage, ApiError> {
        self.send_message(
            chat_id,
            MessageContent::Document {
                source: video_note,
                caption: None,
                parse_mode: ParseMode::None,
                keyboard: None,
                filename: None,
            },
            opts,
        )
        .await
    }

    /// Send paid media (Telegram Stars). Requires star_count > 0.
    async fn send_paid_media(
        &self,
        chat_id: ChatId,
        star_count: i64,
        media: Vec<PaidMediaInput>,
        caption: Option<String>,
        parse_mode: ParseMode,
        opts: SendOptions,
    ) -> Result<SentMessage, ApiError> {
        not_implemented!(
            "send_paid_media",
            chat_id,
            star_count,
            media,
            caption,
            parse_mode,
            opts
        )
    }

    // ─── Live Location ───

    /// Send a live location that can be updated in real-time.
    ///
    /// `live_period` is the duration in seconds (60–86400) for which the location will be updated.
    async fn send_live_location(
        &self,
        chat_id: ChatId,
        latitude: f64,
        longitude: f64,
        live_period: i32,
        opts: SendOptions,
    ) -> Result<SentMessage, ApiError> {
        not_implemented!(
            "send_live_location",
            chat_id,
            latitude,
            longitude,
            live_period,
            opts
        )
    }

    /// Update a live location message.
    async fn edit_message_live_location(
        &self,
        chat_id: ChatId,
        message_id: MessageId,
        latitude: f64,
        longitude: f64,
    ) -> Result<(), ApiError> {
        not_implemented!(
            "edit_message_live_location",
            chat_id,
            message_id,
            latitude,
            longitude
        )
    }

    /// Stop updating a live location message.
    async fn stop_message_live_location(
        &self,
        chat_id: ChatId,
        message_id: MessageId,
    ) -> Result<(), ApiError> {
        not_implemented!("stop_message_live_location", chat_id, message_id)
    }

    // ─── Checklist (Bot API 9.5+) ───

    /// Send a checklist message.
    async fn send_checklist(
        &self,
        chat_id: ChatId,
        title: String,
        items: Vec<ChecklistItem>,
        opts: SendOptions,
    ) -> Result<SentMessage, ApiError> {
        not_implemented!("send_checklist", chat_id, title, items, opts)
    }

    /// Edit an existing checklist message.
    async fn edit_message_checklist(
        &self,
        chat_id: ChatId,
        message_id: MessageId,
        title: String,
        items: Vec<ChecklistItem>,
    ) -> Result<(), ApiError> {
        not_implemented!("edit_message_checklist", chat_id, message_id, title, items)
    }

    // ─── Message Draft (Bot API 9.5+) ───

    /// Send a pre-filled message draft to a user's input field.
    /// The user still has to send it manually.
    async fn send_message_draft(
        &self,
        chat_id: ChatId,
        text: String,
        parse_mode: ParseMode,
    ) -> Result<(), ApiError> {
        not_implemented!("send_message_draft", chat_id, text, parse_mode)
    }

    // ─── User Emoji Status ───

    /// Set the emoji status of a user (requires appropriate bot privileges).
    async fn set_user_emoji_status(
        &self,
        user_id: UserId,
        emoji_status_custom_emoji_id: Option<String>,
        emoji_status_expiration_date: Option<i64>,
    ) -> Result<(), ApiError> {
        not_implemented!(
            "set_user_emoji_status",
            user_id,
            emoji_status_custom_emoji_id,
            emoji_status_expiration_date
        )
    }

    // ─── User Profile Audios (Bot API 9.4+) ───

    /// Get a list of profile audios for a user.
    async fn get_user_profile_audios(
        &self,
        user_id: UserId,
        offset: Option<i32>,
        limit: Option<i32>,
    ) -> Result<UserProfileAudios, ApiError> {
        not_implemented!("get_user_profile_audios", user_id, offset, limit)
    }

    // ─── Admin Extras ───

    /// Ban a channel chat in a supergroup or channel.
    async fn ban_chat_sender_chat(
        &self,
        chat_id: ChatId,
        sender_chat_id: ChatId,
    ) -> Result<(), ApiError> {
        not_implemented!("ban_chat_sender_chat", chat_id, sender_chat_id)
    }

    /// Unban a previously banned channel chat.
    async fn unban_chat_sender_chat(
        &self,
        chat_id: ChatId,
        sender_chat_id: ChatId,
    ) -> Result<(), ApiError> {
        not_implemented!("unban_chat_sender_chat", chat_id, sender_chat_id)
    }

    /// Set or remove a custom tag for a chat member (visible only to admins).
    async fn set_chat_member_tag(
        &self,
        chat_id: ChatId,
        user_id: UserId,
        tag: Option<String>,
    ) -> Result<(), ApiError> {
        not_implemented!("set_chat_member_tag", chat_id, user_id, tag)
    }

    /// Edit an existing chat invite link.
    async fn edit_chat_invite_link(
        &self,
        chat_id: ChatId,
        invite_link: &str,
        name: Option<&str>,
        expire_date: Option<i64>,
        member_limit: Option<i32>,
    ) -> Result<ChatInviteLink, ApiError> {
        not_implemented!(
            "edit_chat_invite_link",
            chat_id,
            invite_link,
            name,
            expire_date,
            member_limit
        )
    }

    /// Create a subscription invite link for a channel chat.
    async fn create_chat_subscription_invite_link(
        &self,
        chat_id: ChatId,
        name: Option<&str>,
        subscription_period: i32,
        subscription_price: i64,
    ) -> Result<ChatInviteLink, ApiError> {
        not_implemented!(
            "create_chat_subscription_invite_link",
            chat_id,
            name,
            subscription_period,
            subscription_price
        )
    }

    /// Edit a subscription invite link.
    async fn edit_chat_subscription_invite_link(
        &self,
        chat_id: ChatId,
        invite_link: &str,
        name: Option<&str>,
    ) -> Result<ChatInviteLink, ApiError> {
        not_implemented!(
            "edit_chat_subscription_invite_link",
            chat_id,
            invite_link,
            name
        )
    }

    /// Get the list of boosts a user has applied to a chat.
    async fn get_user_chat_boosts(
        &self,
        chat_id: ChatId,
        user_id: UserId,
    ) -> Result<UserChatBoosts, ApiError> {
        not_implemented!("get_user_chat_boosts", chat_id, user_id)
    }

    /// Set the bot's default administrator rights.
    async fn set_my_default_administrator_rights(
        &self,
        rights: Option<ChatPermissions>,
        for_channels: Option<bool>,
    ) -> Result<(), ApiError> {
        not_implemented!("set_my_default_administrator_rights", rights, for_channels)
    }

    /// Get the bot's default administrator rights.
    async fn get_my_default_administrator_rights(
        &self,
        for_channels: Option<bool>,
    ) -> Result<ChatPermissions, ApiError> {
        not_implemented!("get_my_default_administrator_rights", for_channels)
    }

    /// Set the bot's profile photo.
    async fn set_my_profile_photo(
        &self,
        photo: FileSource,
        is_public: Option<bool>,
    ) -> Result<(), ApiError> {
        not_implemented!("set_my_profile_photo", photo, is_public)
    }

    /// Remove the bot's profile photo.
    async fn remove_my_profile_photo(&self, file_id: Option<String>) -> Result<(), ApiError> {
        not_implemented!("remove_my_profile_photo", file_id)
    }

    // ─── Forum Extras ───

    /// Edit the name of the 'General' topic in a forum supergroup.
    async fn edit_general_forum_topic(&self, chat_id: ChatId, title: &str) -> Result<(), ApiError> {
        self.edit_forum_topic(chat_id, 1, Some(title), None, None, None)
            .await
    }

    /// Close the 'General' topic in a forum supergroup.
    async fn close_general_forum_topic(&self, chat_id: ChatId) -> Result<(), ApiError> {
        self.edit_forum_topic(chat_id, 1, None, None, Some(true), None)
            .await
    }

    /// Reopen the 'General' topic in a forum supergroup.
    async fn reopen_general_forum_topic(&self, chat_id: ChatId) -> Result<(), ApiError> {
        self.edit_forum_topic(chat_id, 1, None, None, Some(false), None)
            .await
    }

    /// Unpin all messages in the 'General' forum topic.
    async fn unpin_all_general_forum_topic_messages(
        &self,
        chat_id: ChatId,
    ) -> Result<(), ApiError> {
        self.unpin_all_forum_topic_messages(chat_id, 1).await
    }

    // ─── Verification ───

    /// Verify a user on behalf of the organization the bot represents.
    async fn verify_user(
        &self,
        user_id: UserId,
        custom_description: Option<String>,
    ) -> Result<(), ApiError> {
        not_implemented!("verify_user", user_id, custom_description)
    }

    /// Verify a chat on behalf of the organization the bot represents.
    async fn verify_chat(
        &self,
        chat_id: ChatId,
        custom_description: Option<String>,
    ) -> Result<(), ApiError> {
        not_implemented!("verify_chat", chat_id, custom_description)
    }

    /// Remove verification from a user.
    async fn remove_user_verification(&self, user_id: UserId) -> Result<(), ApiError> {
        not_implemented!("remove_user_verification", user_id)
    }

    /// Remove verification from a chat.
    async fn remove_chat_verification(&self, chat_id: ChatId) -> Result<(), ApiError> {
        not_implemented!("remove_chat_verification", chat_id)
    }

    // ─── Business ───

    /// Get information about a business connection.
    async fn get_business_connection(
        &self,
        business_connection_id: &str,
    ) -> Result<BusinessConnection, ApiError> {
        not_implemented!("get_business_connection", business_connection_id)
    }

    /// Mark a message as read in a business chat.
    async fn read_business_message(
        &self,
        business_connection_id: &str,
        chat_id: ChatId,
        message_id: MessageId,
    ) -> Result<(), ApiError> {
        not_implemented!(
            "read_business_message",
            business_connection_id,
            chat_id,
            message_id
        )
    }

    /// Delete messages from a business chat.
    async fn delete_business_messages(
        &self,
        business_connection_id: &str,
        message_ids: Vec<MessageId>,
    ) -> Result<(), ApiError> {
        not_implemented!(
            "delete_business_messages",
            business_connection_id,
            message_ids
        )
    }

    /// Set the business account's name.
    async fn set_business_account_name(
        &self,
        business_connection_id: &str,
        first_name: &str,
        last_name: Option<&str>,
    ) -> Result<(), ApiError> {
        not_implemented!(
            "set_business_account_name",
            business_connection_id,
            first_name,
            last_name
        )
    }

    /// Set the business account's username.
    async fn set_business_account_username(
        &self,
        business_connection_id: &str,
        username: Option<&str>,
    ) -> Result<(), ApiError> {
        not_implemented!(
            "set_business_account_username",
            business_connection_id,
            username
        )
    }

    /// Set the business account's bio.
    async fn set_business_account_bio(
        &self,
        business_connection_id: &str,
        bio: Option<&str>,
    ) -> Result<(), ApiError> {
        not_implemented!("set_business_account_bio", business_connection_id, bio)
    }

    /// Set the business account's profile photo.
    async fn set_business_account_profile_photo(
        &self,
        business_connection_id: &str,
        photo: FileSource,
        is_public: Option<bool>,
    ) -> Result<(), ApiError> {
        not_implemented!(
            "set_business_account_profile_photo",
            business_connection_id,
            photo,
            is_public
        )
    }

    /// Remove the business account's profile photo.
    async fn remove_business_account_profile_photo(
        &self,
        business_connection_id: &str,
        is_public: Option<bool>,
    ) -> Result<(), ApiError> {
        not_implemented!(
            "remove_business_account_profile_photo",
            business_connection_id,
            is_public
        )
    }

    /// Set the business account's gift settings.
    async fn set_business_account_gift_settings(
        &self,
        business_connection_id: &str,
        show_gift_button: bool,
        accepted_gift_types: AcceptedGiftTypes,
    ) -> Result<(), ApiError> {
        not_implemented!(
            "set_business_account_gift_settings",
            business_connection_id,
            show_gift_button,
            accepted_gift_types
        )
    }

    /// Get the business account's current Stars balance.
    async fn get_business_account_star_balance(
        &self,
        business_connection_id: &str,
    ) -> Result<StarBalance, ApiError> {
        not_implemented!("get_business_account_star_balance", business_connection_id)
    }

    /// Transfer Stars from the business account to the bot.
    async fn transfer_business_account_stars(
        &self,
        business_connection_id: &str,
        star_count: i64,
    ) -> Result<(), ApiError> {
        not_implemented!(
            "transfer_business_account_stars",
            business_connection_id,
            star_count
        )
    }

    /// Get the gifts owned by a business account.
    async fn get_business_account_gifts(
        &self,
        business_connection_id: &str,
        exclude_unsaved: Option<bool>,
        exclude_saved: Option<bool>,
        exclude_unlimited: Option<bool>,
        exclude_limited: Option<bool>,
        exclude_unique: Option<bool>,
        sort_by_price: Option<bool>,
        offset: Option<&str>,
        limit: Option<i32>,
    ) -> Result<OwnedGifts, ApiError> {
        not_implemented!(
            "get_business_account_gifts",
            business_connection_id,
            exclude_unsaved,
            exclude_saved,
            exclude_unlimited,
            exclude_limited,
            exclude_unique,
            sort_by_price,
            offset,
            limit
        )
    }

    // ─── Gifts ───

    /// Get the list of gifts that can be sent by the bot.
    async fn get_available_gifts(&self) -> Result<Vec<Gift>, ApiError> {
        not_implemented!("get_available_gifts",)
    }

    /// Send a gift to a user.
    async fn send_gift(
        &self,
        user_id: UserId,
        gift_id: String,
        text: Option<String>,
        text_parse_mode: Option<ParseMode>,
    ) -> Result<(), ApiError> {
        not_implemented!("send_gift", user_id, gift_id, text, text_parse_mode)
    }

    /// Gift a Telegram Premium subscription to a user.
    async fn gift_premium_subscription(
        &self,
        user_id: UserId,
        month_count: i32,
        star_count: i64,
        text: Option<String>,
        text_parse_mode: Option<ParseMode>,
    ) -> Result<(), ApiError> {
        not_implemented!(
            "gift_premium_subscription",
            user_id,
            month_count,
            star_count,
            text,
            text_parse_mode
        )
    }

    /// Get the list of gifts owned by a user.
    async fn get_user_gifts(
        &self,
        user_id: UserId,
        offset: Option<&str>,
        limit: Option<i32>,
    ) -> Result<OwnedGifts, ApiError> {
        not_implemented!("get_user_gifts", user_id, offset, limit)
    }

    /// Get the list of gifts received by a chat.
    async fn get_chat_gifts(
        &self,
        chat_id: ChatId,
        offset: Option<&str>,
        limit: Option<i32>,
    ) -> Result<OwnedGifts, ApiError> {
        not_implemented!("get_chat_gifts", chat_id, offset, limit)
    }

    /// Convert a gift to Telegram Stars.
    async fn convert_gift_to_stars(
        &self,
        business_connection_id: Option<&str>,
        owned_gift_id: &str,
    ) -> Result<(), ApiError> {
        not_implemented!(
            "convert_gift_to_stars",
            business_connection_id,
            owned_gift_id
        )
    }

    /// Upgrade a gift to a unique gift.
    async fn upgrade_gift(
        &self,
        business_connection_id: Option<&str>,
        owned_gift_id: &str,
        keep_original_details: Option<bool>,
        star_count: Option<i64>,
    ) -> Result<(), ApiError> {
        not_implemented!(
            "upgrade_gift",
            business_connection_id,
            owned_gift_id,
            keep_original_details,
            star_count
        )
    }

    /// Transfer a gift to another user or channel.
    async fn transfer_gift(
        &self,
        business_connection_id: Option<&str>,
        owned_gift_id: &str,
        new_owner_chat_id: ChatId,
        star_count: Option<i64>,
    ) -> Result<(), ApiError> {
        not_implemented!(
            "transfer_gift",
            business_connection_id,
            owned_gift_id,
            new_owner_chat_id,
            star_count
        )
    }

    // ─── Stories ───

    /// Post a story on behalf of a channel chat.
    async fn post_story(
        &self,
        chat_id: ChatId,
        content: StoryContent,
        active_period: i32,
        caption: Option<String>,
        parse_mode: Option<ParseMode>,
    ) -> Result<Story, ApiError> {
        not_implemented!(
            "post_story",
            chat_id,
            content,
            active_period,
            caption,
            parse_mode
        )
    }

    /// Edit a previously posted story.
    async fn edit_story(
        &self,
        chat_id: ChatId,
        story_id: i32,
        content: Option<StoryContent>,
        caption: Option<String>,
        parse_mode: Option<ParseMode>,
    ) -> Result<Story, ApiError> {
        not_implemented!(
            "edit_story",
            chat_id,
            story_id,
            content,
            caption,
            parse_mode
        )
    }

    /// Delete a previously posted story.
    async fn delete_story(&self, chat_id: ChatId, story_id: i32) -> Result<(), ApiError> {
        not_implemented!("delete_story", chat_id, story_id)
    }

    // ─── Stars Extras ───

    /// Get the bot's current Stars balance.
    async fn get_my_star_balance(&self) -> Result<StarBalance, ApiError> {
        not_implemented!("get_my_star_balance",)
    }

    /// Edit a user's star subscription (cancel or re-enable).
    async fn edit_user_star_subscription(
        &self,
        user_id: UserId,
        telegram_payment_charge_id: &str,
        is_canceled: bool,
    ) -> Result<(), ApiError> {
        not_implemented!(
            "edit_user_star_subscription",
            user_id,
            telegram_payment_charge_id,
            is_canceled
        )
    }

    // ─── Managed Bots (Bot API 9.6+) ───

    /// Get the current token for a bot managed by the calling bot.
    async fn get_managed_bot_token(&self, bot_id: UserId) -> Result<String, ApiError> {
        not_implemented!("get_managed_bot_token", bot_id)
    }

    /// Generate a new token for a bot managed by the calling bot.
    async fn replace_managed_bot_token(&self, bot_id: UserId) -> Result<String, ApiError> {
        not_implemented!("replace_managed_bot_token", bot_id)
    }

    // ─── Prepared Keyboard Button (Bot API 9.6+) ───

    /// Save a prepared keyboard button for a user. Returns the stored button with ID + expiry.
    async fn save_prepared_keyboard_button(
        &self,
        user_id: UserId,
        button: PreparedKeyboardButtonData,
    ) -> Result<PreparedKeyboardButton, ApiError> {
        not_implemented!("save_prepared_keyboard_button", user_id, button)
    }

    // ─── Sticker Management ───

    /// Get a sticker set by name.
    async fn get_sticker_set(&self, name: &str) -> Result<StickerSet, ApiError> {
        not_implemented!("get_sticker_set", name)
    }

    /// Get information about custom emoji stickers by their identifiers.
    async fn get_custom_emoji_stickers(
        &self,
        custom_emoji_ids: Vec<String>,
    ) -> Result<Vec<StickerInfo>, ApiError> {
        not_implemented!("get_custom_emoji_stickers", custom_emoji_ids)
    }

    /// Upload a sticker file for later use in `create_new_sticker_set` / `add_sticker_to_set`.
    async fn upload_sticker_file(
        &self,
        user_id: UserId,
        sticker: FileSource,
        sticker_format: StickerFormat,
    ) -> Result<TelegramFile, ApiError> {
        not_implemented!("upload_sticker_file", user_id, sticker, sticker_format)
    }

    /// Create a new sticker set owned by a user.
    async fn create_new_sticker_set(
        &self,
        user_id: UserId,
        name: String,
        title: String,
        stickers: Vec<InputSticker>,
        sticker_type: Option<StickerType>,
    ) -> Result<(), ApiError> {
        not_implemented!(
            "create_new_sticker_set",
            user_id,
            name,
            title,
            stickers,
            sticker_type
        )
    }

    /// Add a sticker to an existing set.
    async fn add_sticker_to_set(
        &self,
        user_id: UserId,
        name: &str,
        sticker: InputSticker,
    ) -> Result<(), ApiError> {
        not_implemented!("add_sticker_to_set", user_id, name, sticker)
    }

    /// Move a sticker in its set to a specific position (0-indexed).
    async fn set_sticker_position_in_set(
        &self,
        sticker: &str,
        position: i32,
    ) -> Result<(), ApiError> {
        not_implemented!("set_sticker_position_in_set", sticker, position)
    }

    /// Delete a sticker from its set.
    async fn delete_sticker_from_set(&self, sticker: &str) -> Result<(), ApiError> {
        not_implemented!("delete_sticker_from_set", sticker)
    }

    /// Replace an existing sticker in a set with a new one.
    async fn replace_sticker_in_set(
        &self,
        user_id: UserId,
        name: &str,
        old_sticker: &str,
        sticker: InputSticker,
    ) -> Result<(), ApiError> {
        not_implemented!(
            "replace_sticker_in_set",
            user_id,
            name,
            old_sticker,
            sticker
        )
    }

    /// Change the emoji list associated with a sticker.
    async fn set_sticker_emoji_list(
        &self,
        sticker: &str,
        emoji_list: Vec<String>,
    ) -> Result<(), ApiError> {
        not_implemented!("set_sticker_emoji_list", sticker, emoji_list)
    }

    /// Change search keywords for a sticker.
    async fn set_sticker_keywords(
        &self,
        sticker: &str,
        keywords: Vec<String>,
    ) -> Result<(), ApiError> {
        not_implemented!("set_sticker_keywords", sticker, keywords)
    }

    /// Change the mask position of a mask sticker.
    async fn set_sticker_mask_position(
        &self,
        sticker: &str,
        mask_position: Option<MaskPosition>,
    ) -> Result<(), ApiError> {
        not_implemented!("set_sticker_mask_position", sticker, mask_position)
    }

    /// Set the title of a sticker set.
    async fn set_sticker_set_title(&self, name: &str, title: &str) -> Result<(), ApiError> {
        not_implemented!("set_sticker_set_title", name, title)
    }

    /// Set the thumbnail of a sticker set.
    async fn set_sticker_set_thumbnail(
        &self,
        name: &str,
        user_id: UserId,
        thumbnail: Option<FileSource>,
        format: StickerFormat,
    ) -> Result<(), ApiError> {
        not_implemented!(
            "set_sticker_set_thumbnail",
            name,
            user_id,
            thumbnail,
            format
        )
    }

    /// Set the thumbnail of a custom emoji sticker set.
    async fn set_custom_emoji_sticker_set_thumbnail(
        &self,
        name: &str,
        custom_emoji_id: Option<String>,
    ) -> Result<(), ApiError> {
        not_implemented!(
            "set_custom_emoji_sticker_set_thumbnail",
            name,
            custom_emoji_id
        )
    }

    /// Delete a sticker set.
    async fn delete_sticker_set(&self, name: &str) -> Result<(), ApiError> {
        not_implemented!("delete_sticker_set", name)
    }

    /// Get stickers that can be used as forum topic icons.
    async fn get_forum_topic_icon_stickers(&self) -> Result<Vec<StickerInfo>, ApiError> {
        not_implemented!("get_forum_topic_icon_stickers",)
    }

    /// Set a group sticker set for a supergroup.
    async fn set_chat_sticker_set(
        &self,
        chat_id: ChatId,
        sticker_set_name: &str,
    ) -> Result<(), ApiError> {
        not_implemented!("set_chat_sticker_set", chat_id, sticker_set_name)
    }

    /// Delete a group sticker set from a supergroup.
    async fn delete_chat_sticker_set(&self, chat_id: ChatId) -> Result<(), ApiError> {
        not_implemented!("delete_chat_sticker_set", chat_id)
    }

    // ─── Games ───

    /// Send a game.
    async fn send_game(
        &self,
        chat_id: ChatId,
        game_short_name: &str,
        opts: SendOptions,
    ) -> Result<SentMessage, ApiError> {
        not_implemented!("send_game", chat_id, game_short_name, opts)
    }

    /// Set the score for a game.
    async fn set_game_score(
        &self,
        user_id: UserId,
        score: i64,
        chat_id: ChatId,
        message_id: MessageId,
        force: bool,
        disable_edit_message: bool,
    ) -> Result<(), ApiError> {
        not_implemented!(
            "set_game_score",
            user_id,
            score,
            chat_id,
            message_id,
            force,
            disable_edit_message
        )
    }

    /// Get game high scores for a user.
    async fn get_game_high_scores(
        &self,
        user_id: UserId,
        chat_id: ChatId,
        message_id: MessageId,
    ) -> Result<Vec<GameHighScore>, ApiError> {
        not_implemented!("get_game_high_scores", user_id, chat_id, message_id)
    }

    // ─── Inline Extras ───

    /// Set the result of an interaction with a Web App.
    async fn answer_web_app_query(
        &self,
        web_app_query_id: &str,
        result: InlineQueryResult,
    ) -> Result<SentWebAppMessage, ApiError> {
        not_implemented!("answer_web_app_query", web_app_query_id, result)
    }

    /// Store a message that can be sent by a user of a Mini App.
    async fn save_prepared_inline_message(
        &self,
        user_id: UserId,
        result: InlineQueryResult,
        allow_user_chats: Option<bool>,
        allow_bot_chats: Option<bool>,
        allow_group_chats: Option<bool>,
        allow_channel_chats: Option<bool>,
    ) -> Result<PreparedInlineMessage, ApiError> {
        not_implemented!(
            "save_prepared_inline_message",
            user_id,
            result,
            allow_user_chats,
            allow_bot_chats,
            allow_group_chats,
            allow_channel_chats
        )
    }

    // ─── Suggested Posts (Bot API 9.6+) ───

    /// Approve a suggested post in a channel managed by the bot.
    async fn approve_suggested_post(
        &self,
        chat_id: ChatId,
        message_id: MessageId,
    ) -> Result<(), ApiError> {
        not_implemented!("approve_suggested_post", chat_id, message_id)
    }

    /// Decline a suggested post in a channel managed by the bot.
    async fn decline_suggested_post(
        &self,
        chat_id: ChatId,
        message_id: MessageId,
    ) -> Result<(), ApiError> {
        not_implemented!("decline_suggested_post", chat_id, message_id)
    }

    // ─── Telegram Passport ───

    /// Inform a user that some of the Telegram Passport elements they provided contain errors.
    async fn set_passport_data_errors(
        &self,
        user_id: UserId,
        errors: Vec<PassportElementError>,
    ) -> Result<(), ApiError> {
        not_implemented!("set_passport_data_errors", user_id, errors)
    }
}
