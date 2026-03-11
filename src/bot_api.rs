use async_trait::async_trait;
use crate::error::ApiError;
use crate::keyboard::InlineKeyboard;
use crate::screen::ReplyKeyboardAction;
use crate::types::*;

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
/// All methods have default implementations that return `ApiError::Unknown("not implemented")`.
/// The core methods (send_message, edit_*, delete_*, answer_callback_query, send_chat_action,
/// answer_inline_query) are required. Everything else is opt-in — implement what you need.
#[async_trait]
pub trait BotApi: Send + Sync + 'static {

    // ─── Core (required) ───

    async fn send_message(
        &self, chat_id: ChatId, content: MessageContent, opts: SendOptions,
    ) -> Result<SentMessage, ApiError>;

    async fn edit_message_text(
        &self, chat_id: ChatId, message_id: MessageId,
        text: String, parse_mode: ParseMode,
        keyboard: Option<InlineKeyboard>, link_preview: bool,
    ) -> Result<(), ApiError>;

    async fn edit_message_caption(
        &self, chat_id: ChatId, message_id: MessageId,
        caption: Option<String>, parse_mode: ParseMode,
        keyboard: Option<InlineKeyboard>,
    ) -> Result<(), ApiError>;

    async fn edit_message_media(
        &self, chat_id: ChatId, message_id: MessageId,
        content: MessageContent, keyboard: Option<InlineKeyboard>,
    ) -> Result<(), ApiError>;

    async fn edit_message_keyboard(
        &self, chat_id: ChatId, message_id: MessageId,
        keyboard: Option<InlineKeyboard>,
    ) -> Result<(), ApiError>;

    async fn delete_messages(
        &self, chat_id: ChatId, message_ids: Vec<MessageId>,
    ) -> Result<(), ApiError>;

    async fn answer_callback_query(
        &self, id: String, text: Option<String>, show_alert: bool,
    ) -> Result<(), ApiError>;

    async fn send_chat_action(
        &self, chat_id: ChatId, action: ChatAction,
    ) -> Result<(), ApiError>;

    async fn answer_inline_query(
        &self, query_id: String, results: Vec<InlineQueryResult>,
        next_offset: Option<String>, cache_time: Option<i32>, is_personal: bool,
    ) -> Result<(), ApiError>;

    // ─── Forwarding & Copying ───

    /// Forward a message from one chat to another.
    async fn forward_message(
        &self, chat_id: ChatId, from_chat_id: ChatId, message_id: MessageId,
    ) -> Result<SentMessage, ApiError> {
        let _ = (chat_id, from_chat_id, message_id);
        Err(ApiError::Unknown("forward_message not implemented".into()))
    }

    /// Copy a message (re-send without "Forwarded from" header).
    async fn copy_message(
        &self, chat_id: ChatId, from_chat_id: ChatId, message_id: MessageId,
    ) -> Result<MessageId, ApiError> {
        let _ = (chat_id, from_chat_id, message_id);
        Err(ApiError::Unknown("copy_message not implemented".into()))
    }

    // ─── Media ───

    /// Send a group of photos/videos/documents as an album.
    async fn send_media_group(
        &self, chat_id: ChatId, media: Vec<MediaGroupItem>,
    ) -> Result<Vec<SentMessage>, ApiError> {
        let _ = (chat_id, media);
        Err(ApiError::Unknown("send_media_group not implemented".into()))
    }

    /// Download a file by its file_id. Returns raw bytes.
    async fn download_file(
        &self, file_id: &str,
    ) -> Result<DownloadedFile, ApiError> {
        let _ = file_id;
        Err(ApiError::Unknown("download_file not implemented".into()))
    }

    // ─── Fun & Interactive ───

    /// Send a poll.
    async fn send_poll(
        &self, chat_id: ChatId, poll: SendPoll,
    ) -> Result<SentMessage, ApiError> {
        let _ = (chat_id, poll);
        Err(ApiError::Unknown("send_poll not implemented".into()))
    }

    /// Stop a poll.
    async fn stop_poll(
        &self, chat_id: ChatId, message_id: MessageId,
    ) -> Result<(), ApiError> {
        let _ = (chat_id, message_id);
        Err(ApiError::Unknown("stop_poll not implemented".into()))
    }

    /// Send a dice animation.
    async fn send_dice(
        &self, chat_id: ChatId, emoji: DiceEmoji,
    ) -> Result<SentMessage, ApiError> {
        let _ = (chat_id, emoji);
        Err(ApiError::Unknown("send_dice not implemented".into()))
    }

    /// Send a contact.
    async fn send_contact(
        &self, chat_id: ChatId, contact: Contact,
    ) -> Result<SentMessage, ApiError> {
        let _ = (chat_id, contact);
        Err(ApiError::Unknown("send_contact not implemented".into()))
    }

    /// Send a venue.
    async fn send_venue(
        &self, chat_id: ChatId, venue: Venue,
    ) -> Result<SentMessage, ApiError> {
        let _ = (chat_id, venue);
        Err(ApiError::Unknown("send_venue not implemented".into()))
    }

    // ─── Payments ───

    /// Send an invoice for payment.
    async fn send_invoice(
        &self, chat_id: ChatId, invoice: Invoice,
    ) -> Result<SentMessage, ApiError> {
        let _ = (chat_id, invoice);
        Err(ApiError::Unknown("send_invoice not implemented".into()))
    }

    /// Answer a pre-checkout query (approve or decline).
    async fn answer_pre_checkout_query(
        &self, id: String, ok: bool, error_message: Option<String>,
    ) -> Result<(), ApiError> {
        let _ = (id, ok, error_message);
        Err(ApiError::Unknown("answer_pre_checkout_query not implemented".into()))
    }

    // ─── Chat Administration ───

    /// Ban a user from a chat.
    async fn ban_chat_member(
        &self, chat_id: ChatId, user_id: UserId,
    ) -> Result<(), ApiError> {
        let _ = (chat_id, user_id);
        Err(ApiError::Unknown("ban_chat_member not implemented".into()))
    }

    /// Unban a previously banned user.
    async fn unban_chat_member(
        &self, chat_id: ChatId, user_id: UserId,
    ) -> Result<(), ApiError> {
        let _ = (chat_id, user_id);
        Err(ApiError::Unknown("unban_chat_member not implemented".into()))
    }

    /// Restrict a user (set permissions).
    async fn restrict_chat_member(
        &self, chat_id: ChatId, user_id: UserId, permissions: ChatPermissions,
    ) -> Result<(), ApiError> {
        let _ = (chat_id, user_id, permissions);
        Err(ApiError::Unknown("restrict_chat_member not implemented".into()))
    }

    /// Promote a user to admin.
    async fn promote_chat_member(
        &self, chat_id: ChatId, user_id: UserId, permissions: ChatPermissions,
    ) -> Result<(), ApiError> {
        let _ = (chat_id, user_id, permissions);
        Err(ApiError::Unknown("promote_chat_member not implemented".into()))
    }

    /// Get info about a chat member.
    async fn get_chat_member(
        &self, chat_id: ChatId, user_id: UserId,
    ) -> Result<ChatMember, ApiError> {
        let _ = (chat_id, user_id);
        Err(ApiError::Unknown("get_chat_member not implemented".into()))
    }

    /// Get the number of members in a chat.
    async fn get_chat_member_count(
        &self, chat_id: ChatId,
    ) -> Result<i32, ApiError> {
        let _ = chat_id;
        Err(ApiError::Unknown("get_chat_member_count not implemented".into()))
    }

    /// Get chat info.
    async fn get_chat(
        &self, chat_id: ChatId,
    ) -> Result<ChatInfo, ApiError> {
        let _ = chat_id;
        Err(ApiError::Unknown("get_chat not implemented".into()))
    }

    /// Leave a chat.
    async fn leave_chat(
        &self, chat_id: ChatId,
    ) -> Result<(), ApiError> {
        let _ = chat_id;
        Err(ApiError::Unknown("leave_chat not implemented".into()))
    }

    /// Set chat permissions for all members.
    async fn set_chat_permissions(
        &self, chat_id: ChatId, permissions: ChatPermissions,
    ) -> Result<(), ApiError> {
        let _ = (chat_id, permissions);
        Err(ApiError::Unknown("set_chat_permissions not implemented".into()))
    }

    // ─── Bot Settings ───

    /// Set the bot's command list.
    async fn set_my_commands(
        &self, commands: Vec<BotCommand>,
    ) -> Result<(), ApiError> {
        let _ = commands;
        Err(ApiError::Unknown("set_my_commands not implemented".into()))
    }

    /// Delete the bot's command list.
    async fn delete_my_commands(&self) -> Result<(), ApiError> {
        Err(ApiError::Unknown("delete_my_commands not implemented".into()))
    }

    /// Get bot info (id, username, etc).
    async fn get_me(&self) -> Result<BotInfo, ApiError> {
        Err(ApiError::Unknown("get_me not implemented".into()))
    }

    // ─── Reactions ───

    /// Set a reaction on a message.
    async fn set_message_reaction(
        &self, chat_id: ChatId, message_id: MessageId, emoji: &str,
    ) -> Result<(), ApiError> {
        let _ = (chat_id, message_id, emoji);
        Err(ApiError::Unknown("set_message_reaction not implemented".into()))
    }

    // ─── Pinning ───

    /// Pin a message in a chat.
    async fn pin_chat_message(
        &self, chat_id: ChatId, message_id: MessageId, silent: bool,
    ) -> Result<(), ApiError> {
        let _ = (chat_id, message_id, silent);
        Err(ApiError::Unknown("pin_chat_message not implemented".into()))
    }

    /// Unpin a message in a chat.
    async fn unpin_chat_message(
        &self, chat_id: ChatId, message_id: MessageId,
    ) -> Result<(), ApiError> {
        let _ = (chat_id, message_id);
        Err(ApiError::Unknown("unpin_chat_message not implemented".into()))
    }

    /// Unpin all messages in a chat.
    async fn unpin_all_chat_messages(
        &self, chat_id: ChatId,
    ) -> Result<(), ApiError> {
        let _ = chat_id;
        Err(ApiError::Unknown("unpin_all_chat_messages not implemented".into()))
    }

    // ─── Invite Links ───

    /// Create a chat invite link.
    async fn create_chat_invite_link(
        &self, chat_id: ChatId, name: Option<&str>, expire_date: Option<i64>, member_limit: Option<i32>,
    ) -> Result<String, ApiError> {
        let _ = (chat_id, name, expire_date, member_limit);
        Err(ApiError::Unknown("create_chat_invite_link not implemented".into()))
    }

    /// Export the primary chat invite link.
    async fn export_chat_invite_link(
        &self, chat_id: ChatId,
    ) -> Result<String, ApiError> {
        let _ = chat_id;
        Err(ApiError::Unknown("export_chat_invite_link not implemented".into()))
    }

    /// Revoke a chat invite link.
    async fn revoke_chat_invite_link(
        &self, chat_id: ChatId, invite_link: &str,
    ) -> Result<ChatInviteLink, ApiError> {
        let _ = (chat_id, invite_link);
        Err(ApiError::Unknown("revoke_chat_invite_link not implemented".into()))
    }

    // ─── Chat Join Requests ───

    /// Approve a chat join request.
    async fn approve_chat_join_request(
        &self, chat_id: ChatId, user_id: UserId,
    ) -> Result<(), ApiError> {
        let _ = (chat_id, user_id);
        Err(ApiError::Unknown("approve_chat_join_request not implemented".into()))
    }

    /// Decline a chat join request.
    async fn decline_chat_join_request(
        &self, chat_id: ChatId, user_id: UserId,
    ) -> Result<(), ApiError> {
        let _ = (chat_id, user_id);
        Err(ApiError::Unknown("decline_chat_join_request not implemented".into()))
    }

    // ─── Chat Management ───

    /// Set the chat title.
    async fn set_chat_title(
        &self, chat_id: ChatId, title: &str,
    ) -> Result<(), ApiError> {
        let _ = (chat_id, title);
        Err(ApiError::Unknown("set_chat_title not implemented".into()))
    }

    /// Set the chat description.
    async fn set_chat_description(
        &self, chat_id: ChatId, description: Option<&str>,
    ) -> Result<(), ApiError> {
        let _ = (chat_id, description);
        Err(ApiError::Unknown("set_chat_description not implemented".into()))
    }

    /// Set the chat photo.
    async fn set_chat_photo(
        &self, chat_id: ChatId, photo: FileSource,
    ) -> Result<(), ApiError> {
        let _ = (chat_id, photo);
        Err(ApiError::Unknown("set_chat_photo not implemented".into()))
    }

    /// Delete the chat photo.
    async fn delete_chat_photo(
        &self, chat_id: ChatId,
    ) -> Result<(), ApiError> {
        let _ = chat_id;
        Err(ApiError::Unknown("delete_chat_photo not implemented".into()))
    }

    /// Get the list of chat administrators.
    async fn get_chat_administrators(
        &self, chat_id: ChatId,
    ) -> Result<Vec<ChatMember>, ApiError> {
        let _ = chat_id;
        Err(ApiError::Unknown("get_chat_administrators not implemented".into()))
    }

    /// Set a custom title for an admin in a supergroup.
    async fn set_chat_administrator_custom_title(
        &self, chat_id: ChatId, user_id: UserId, custom_title: &str,
    ) -> Result<(), ApiError> {
        let _ = (chat_id, user_id, custom_title);
        Err(ApiError::Unknown("set_chat_administrator_custom_title not implemented".into()))
    }

    // ─── User Info ───

    /// Get a user's profile photos.
    async fn get_user_profile_photos(
        &self, user_id: UserId, offset: Option<i32>, limit: Option<i32>,
    ) -> Result<UserProfilePhotos, ApiError> {
        let _ = (user_id, offset, limit);
        Err(ApiError::Unknown("get_user_profile_photos not implemented".into()))
    }

    // ─── Bot Settings (extended) ───

    /// Get the bot's command list.
    async fn get_my_commands(&self) -> Result<Vec<BotCommand>, ApiError> {
        Err(ApiError::Unknown("get_my_commands not implemented".into()))
    }

    /// Set the bot's description.
    async fn set_my_description(
        &self, description: Option<&str>, language_code: Option<&str>,
    ) -> Result<(), ApiError> {
        let _ = (description, language_code);
        Err(ApiError::Unknown("set_my_description not implemented".into()))
    }

    /// Get the bot's description.
    async fn get_my_description(
        &self, language_code: Option<&str>,
    ) -> Result<BotDescription, ApiError> {
        let _ = language_code;
        Err(ApiError::Unknown("get_my_description not implemented".into()))
    }

    /// Set the bot's short description.
    async fn set_my_short_description(
        &self, short_description: Option<&str>, language_code: Option<&str>,
    ) -> Result<(), ApiError> {
        let _ = (short_description, language_code);
        Err(ApiError::Unknown("set_my_short_description not implemented".into()))
    }

    /// Get the bot's short description.
    async fn get_my_short_description(
        &self, language_code: Option<&str>,
    ) -> Result<BotShortDescription, ApiError> {
        let _ = language_code;
        Err(ApiError::Unknown("get_my_short_description not implemented".into()))
    }

    /// Set the bot's name.
    async fn set_my_name(
        &self, name: Option<&str>, language_code: Option<&str>,
    ) -> Result<(), ApiError> {
        let _ = (name, language_code);
        Err(ApiError::Unknown("set_my_name not implemented".into()))
    }

    /// Get the bot's name.
    async fn get_my_name(
        &self, language_code: Option<&str>,
    ) -> Result<BotName, ApiError> {
        let _ = language_code;
        Err(ApiError::Unknown("get_my_name not implemented".into()))
    }

    // ─── Menu Button ───

    /// Set the bot's menu button for a specific chat or default.
    async fn set_chat_menu_button(
        &self, chat_id: Option<ChatId>, menu_button: MenuButton,
    ) -> Result<(), ApiError> {
        let _ = (chat_id, menu_button);
        Err(ApiError::Unknown("set_chat_menu_button not implemented".into()))
    }

    /// Get the bot's menu button for a specific chat or default.
    async fn get_chat_menu_button(
        &self, chat_id: Option<ChatId>,
    ) -> Result<MenuButton, ApiError> {
        let _ = chat_id;
        Err(ApiError::Unknown("get_chat_menu_button not implemented".into()))
    }

    // ─── Payments (extended) ───

    /// Answer a shipping query (for flexible pricing invoices).
    async fn answer_shipping_query(
        &self, shipping_query_id: String, ok: bool,
        shipping_options: Option<Vec<ShippingOption>>,
        error_message: Option<String>,
    ) -> Result<(), ApiError> {
        let _ = (shipping_query_id, ok, shipping_options, error_message);
        Err(ApiError::Unknown("answer_shipping_query not implemented".into()))
    }

    /// Create an invoice link for payments without sending a message.
    async fn create_invoice_link(
        &self, invoice: Invoice,
    ) -> Result<String, ApiError> {
        let _ = invoice;
        Err(ApiError::Unknown("create_invoice_link not implemented".into()))
    }

    // ─── Batch Operations ───

    /// Forward multiple messages at once.
    async fn forward_messages(
        &self, chat_id: ChatId, from_chat_id: ChatId, message_ids: Vec<MessageId>,
    ) -> Result<Vec<MessageId>, ApiError> {
        let _ = (chat_id, from_chat_id, message_ids);
        Err(ApiError::Unknown("forward_messages not implemented".into()))
    }

    /// Copy multiple messages at once.
    async fn copy_messages(
        &self, chat_id: ChatId, from_chat_id: ChatId, message_ids: Vec<MessageId>,
    ) -> Result<Vec<MessageId>, ApiError> {
        let _ = (chat_id, from_chat_id, message_ids);
        Err(ApiError::Unknown("copy_messages not implemented".into()))
    }

    // ─── Sticker ───

    /// Send a sticker (convenience — also available via send_message with MessageContent::Sticker).
    async fn send_sticker(
        &self, chat_id: ChatId, sticker: FileSource,
    ) -> Result<SentMessage, ApiError> {
        let _ = (chat_id, sticker);
        Err(ApiError::Unknown("send_sticker not implemented".into()))
    }

    // ─── Location ───

    /// Send a location.
    async fn send_location(
        &self, chat_id: ChatId, latitude: f64, longitude: f64,
    ) -> Result<SentMessage, ApiError> {
        let _ = (chat_id, latitude, longitude);
        Err(ApiError::Unknown("send_location not implemented".into()))
    }

    // ─── Forum Topics ───

    /// Create a forum topic in a supergroup.
    async fn create_forum_topic(
        &self, chat_id: ChatId, title: &str,
        icon_color: Option<i32>, icon_custom_emoji_id: Option<i64>,
    ) -> Result<ForumTopic, ApiError> {
        let _ = (chat_id, title, icon_color, icon_custom_emoji_id);
        Err(ApiError::Unknown("create_forum_topic not implemented".into()))
    }

    /// Edit a forum topic (title, icon, open/close, hide/show).
    async fn edit_forum_topic(
        &self, chat_id: ChatId, topic_id: i32,
        title: Option<&str>, icon_custom_emoji_id: Option<i64>,
        closed: Option<bool>, hidden: Option<bool>,
    ) -> Result<(), ApiError> {
        let _ = (chat_id, topic_id, title, icon_custom_emoji_id, closed, hidden);
        Err(ApiError::Unknown("edit_forum_topic not implemented".into()))
    }

    /// Close a forum topic.
    async fn close_forum_topic(
        &self, chat_id: ChatId, topic_id: i32,
    ) -> Result<(), ApiError> {
        self.edit_forum_topic(chat_id, topic_id, None, None, Some(true), None).await
    }

    /// Reopen a forum topic.
    async fn reopen_forum_topic(
        &self, chat_id: ChatId, topic_id: i32,
    ) -> Result<(), ApiError> {
        self.edit_forum_topic(chat_id, topic_id, None, None, Some(false), None).await
    }

    /// Delete a forum topic and all its messages.
    async fn delete_forum_topic(
        &self, chat_id: ChatId, topic_id: i32,
    ) -> Result<(), ApiError> {
        let _ = (chat_id, topic_id);
        Err(ApiError::Unknown("delete_forum_topic not implemented".into()))
    }

    /// Unpin all messages in a forum topic.
    async fn unpin_all_forum_topic_messages(
        &self, chat_id: ChatId, topic_id: i32,
    ) -> Result<(), ApiError> {
        let _ = (chat_id, topic_id);
        Err(ApiError::Unknown("unpin_all_forum_topic_messages not implemented".into()))
    }

    /// Hide the 'General' topic in a forum supergroup.
    async fn hide_general_forum_topic(
        &self, chat_id: ChatId,
    ) -> Result<(), ApiError> {
        // General topic id = 1, hidden = true
        self.edit_forum_topic(chat_id, 1, None, None, None, Some(true)).await
    }

    /// Unhide the 'General' topic in a forum supergroup.
    async fn unhide_general_forum_topic(
        &self, chat_id: ChatId,
    ) -> Result<(), ApiError> {
        self.edit_forum_topic(chat_id, 1, None, None, None, Some(false)).await
    }

    // ─── Stars API ───

    /// Get star transaction history for the bot.
    async fn get_star_transactions(
        &self, offset: Option<&str>, limit: Option<i32>,
    ) -> Result<StarTransactions, ApiError> {
        let _ = (offset, limit);
        Err(ApiError::Unknown("get_star_transactions not implemented".into()))
    }

    /// Refund a star payment by charge_id.
    async fn refund_star_payment(
        &self, user_id: UserId, charge_id: &str,
    ) -> Result<(), ApiError> {
        let _ = (user_id, charge_id);
        Err(ApiError::Unknown("refund_star_payment not implemented".into()))
    }
}
