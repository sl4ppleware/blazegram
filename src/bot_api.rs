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
}
