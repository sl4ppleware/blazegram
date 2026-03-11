//! Mock BotApi for testing.

use async_trait::async_trait;
use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};
use tokio::sync::Mutex;

use crate::bot_api::{BotApi, SendOptions};
use crate::error::ApiError;
use crate::keyboard::InlineKeyboard;
use crate::types::*;

#[derive(Clone)]
#[allow(clippy::type_complexity)]
/// A mock implementation of [`BotApi`] that records all calls for testing.
pub struct MockBotApi {
    counter: Arc<AtomicI32>,
    messages: Arc<Mutex<Vec<(ChatId, MessageContent)>>>,
    deleted: Arc<Mutex<Vec<(ChatId, Vec<MessageId>)>>>,
    edits: Arc<Mutex<Vec<(ChatId, MessageId, String)>>>,
    answers: Arc<Mutex<Vec<(String, Option<String>, bool)>>>,
}

impl MockBotApi {
    /// Create a new instance.
    pub fn new() -> Self {
        Self {
            counter: Arc::new(AtomicI32::new(100)),
            messages: Arc::new(Mutex::new(Vec::new())),
            deleted: Arc::new(Mutex::new(Vec::new())),
            edits: Arc::new(Mutex::new(Vec::new())),
            answers: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Next id.
    pub fn next_id(&self) -> i32 {
        self.counter.fetch_add(1, Ordering::SeqCst)
    }

    /// Get sent messages. Use `sent_messages_async` from async contexts.
    pub fn sent_messages(&self) -> Vec<(ChatId, MessageContent)> {
        self.messages.blocking_lock().clone()
    }

    /// Async-safe accessor for sent messages.
    pub async fn sent_messages_async(&self) -> Vec<(ChatId, MessageContent)> {
        self.messages.lock().await.clone()
    }

    /// Get deleted messages. Use `deleted_messages_async` from async contexts.
    pub fn deleted_messages(&self) -> Vec<(ChatId, Vec<MessageId>)> {
        self.deleted.blocking_lock().clone()
    }

    /// Async-safe accessor for deleted messages.
    pub async fn deleted_messages_async(&self) -> Vec<(ChatId, Vec<MessageId>)> {
        self.deleted.lock().await.clone()
    }

    /// Get sent message count. Use `call_count_async` from async contexts.
    pub fn call_count(&self) -> usize {
        self.messages.blocking_lock().len()
    }

    /// Async-safe accessor for call count.
    pub async fn call_count_async(&self) -> usize {
        self.messages.lock().await.len()
    }

    /// Get edit records. Use from async context.
    pub async fn edits_async(&self) -> Vec<(ChatId, MessageId, String)> {
        self.edits.lock().await.clone()
    }

    /// Get callback answers. Use from async context.
    pub async fn answers_async(&self) -> Vec<(String, Option<String>, bool)> {
        self.answers.lock().await.clone()
    }
}

impl Default for MockBotApi {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BotApi for MockBotApi {
    async fn send_message(
        &self,
        chat_id: ChatId,
        content: MessageContent,
        _opts: SendOptions,
    ) -> Result<SentMessage, ApiError> {
        let id = self.next_id();
        self.messages.lock().await.push((chat_id, content));
        Ok(SentMessage {
            message_id: MessageId(id),
            chat_id,
        })
    }

    async fn edit_message_text(
        &self,
        c: ChatId,
        m: MessageId,
        text: String,
        _pm: ParseMode,
        _kb: Option<InlineKeyboard>,
        _lp: bool,
    ) -> Result<(), ApiError> {
        self.edits.lock().await.push((c, m, text));
        Ok(())
    }

    async fn edit_message_caption(
        &self,
        c: ChatId,
        m: MessageId,
        cap: Option<String>,
        _pm: ParseMode,
        _kb: Option<InlineKeyboard>,
    ) -> Result<(), ApiError> {
        self.edits
            .lock()
            .await
            .push((c, m, cap.unwrap_or_default()));
        Ok(())
    }

    async fn edit_message_media(
        &self,
        c: ChatId,
        m: MessageId,
        _content: MessageContent,
        _kb: Option<InlineKeyboard>,
    ) -> Result<(), ApiError> {
        self.edits.lock().await.push((c, m, "media".into()));
        Ok(())
    }

    async fn edit_message_keyboard(
        &self,
        c: ChatId,
        m: MessageId,
        _kb: Option<InlineKeyboard>,
    ) -> Result<(), ApiError> {
        self.edits.lock().await.push((c, m, "keyboard".into()));
        Ok(())
    }

    async fn delete_messages(&self, c: ChatId, ids: Vec<MessageId>) -> Result<(), ApiError> {
        self.deleted.lock().await.push((c, ids));
        Ok(())
    }

    async fn answer_callback_query(
        &self,
        id: String,
        text: Option<String>,
        alert: bool,
    ) -> Result<(), ApiError> {
        self.answers.lock().await.push((id, text, alert));
        Ok(())
    }

    async fn send_chat_action(&self, _c: ChatId, _a: ChatAction) -> Result<(), ApiError> {
        Ok(())
    }

    async fn answer_inline_query(
        &self,
        _query_id: String,
        _results: Vec<InlineQueryResult>,
        _next_offset: Option<String>,
        _cache_time: Option<i32>,
        _is_personal: bool,
    ) -> Result<(), ApiError> {
        Ok(())
    }

    async fn forward_message(
        &self,
        chat_id: ChatId,
        _from_chat_id: ChatId,
        _message_id: MessageId,
    ) -> Result<SentMessage, ApiError> {
        let id = self.next_id();
        Ok(SentMessage {
            message_id: MessageId(id),
            chat_id,
        })
    }

    async fn copy_message(
        &self,
        _chat_id: ChatId,
        _from_chat_id: ChatId,
        _message_id: MessageId,
    ) -> Result<MessageId, ApiError> {
        Ok(MessageId(self.next_id()))
    }

    async fn download_file(&self, _file_id: &str) -> Result<DownloadedFile, ApiError> {
        Ok(DownloadedFile {
            data: vec![0xFF, 0xD8],
            file_size: Some(2),
        })
    }

    async fn send_poll(&self, chat_id: ChatId, _poll: SendPoll) -> Result<SentMessage, ApiError> {
        let id = self.next_id();
        Ok(SentMessage {
            message_id: MessageId(id),
            chat_id,
        })
    }

    async fn send_dice(&self, chat_id: ChatId, _emoji: DiceEmoji) -> Result<SentMessage, ApiError> {
        let id = self.next_id();
        Ok(SentMessage {
            message_id: MessageId(id),
            chat_id,
        })
    }

    async fn send_contact(
        &self,
        chat_id: ChatId,
        _contact: Contact,
    ) -> Result<SentMessage, ApiError> {
        let id = self.next_id();
        Ok(SentMessage {
            message_id: MessageId(id),
            chat_id,
        })
    }

    async fn send_venue(&self, chat_id: ChatId, _venue: Venue) -> Result<SentMessage, ApiError> {
        let id = self.next_id();
        Ok(SentMessage {
            message_id: MessageId(id),
            chat_id,
        })
    }

    async fn ban_chat_member(&self, _chat_id: ChatId, _user_id: UserId) -> Result<(), ApiError> {
        Ok(())
    }
    async fn unban_chat_member(&self, _chat_id: ChatId, _user_id: UserId) -> Result<(), ApiError> {
        Ok(())
    }
    async fn leave_chat(&self, _chat_id: ChatId) -> Result<(), ApiError> {
        Ok(())
    }
    async fn get_chat_member_count(&self, _chat_id: ChatId) -> Result<i32, ApiError> {
        Ok(42)
    }
    async fn set_my_commands(&self, _commands: Vec<BotCommand>) -> Result<(), ApiError> {
        Ok(())
    }
    async fn delete_my_commands(&self) -> Result<(), ApiError> {
        Ok(())
    }

    async fn get_me(&self) -> Result<BotInfo, ApiError> {
        Ok(BotInfo {
            id: UserId(0),
            username: "mock_bot".into(),
            first_name: "MockBot".into(),
            can_join_groups: true,
            can_read_all_group_messages: false,
            supports_inline_queries: false,
        })
    }

    async fn pin_chat_message(
        &self,
        _chat_id: ChatId,
        _message_id: MessageId,
        _silent: bool,
    ) -> Result<(), ApiError> {
        Ok(())
    }
    async fn unpin_chat_message(
        &self,
        _chat_id: ChatId,
        _message_id: MessageId,
    ) -> Result<(), ApiError> {
        Ok(())
    }
    async fn set_message_reaction(
        &self,
        _chat_id: ChatId,
        _message_id: MessageId,
        _emoji: &str,
    ) -> Result<(), ApiError> {
        Ok(())
    }

    async fn answer_pre_checkout_query(
        &self,
        _id: String,
        _ok: bool,
        _error_message: Option<String>,
    ) -> Result<(), ApiError> {
        Ok(())
    }

    async fn set_chat_title(&self, _chat_id: ChatId, _title: &str) -> Result<(), ApiError> {
        Ok(())
    }
    async fn set_chat_description(
        &self,
        _chat_id: ChatId,
        _description: Option<&str>,
    ) -> Result<(), ApiError> {
        Ok(())
    }
    async fn delete_chat_photo(&self, _chat_id: ChatId) -> Result<(), ApiError> {
        Ok(())
    }

    async fn get_chat_administrators(&self, _chat_id: ChatId) -> Result<Vec<ChatMember>, ApiError> {
        Ok(vec![])
    }

    async fn set_chat_administrator_custom_title(
        &self,
        _chat_id: ChatId,
        _user_id: UserId,
        _title: &str,
    ) -> Result<(), ApiError> {
        Ok(())
    }

    async fn approve_chat_join_request(
        &self,
        _chat_id: ChatId,
        _user_id: UserId,
    ) -> Result<(), ApiError> {
        Ok(())
    }
    async fn decline_chat_join_request(
        &self,
        _chat_id: ChatId,
        _user_id: UserId,
    ) -> Result<(), ApiError> {
        Ok(())
    }

    async fn get_user_profile_photos(
        &self,
        _user_id: UserId,
        _offset: Option<i32>,
        _limit: Option<i32>,
    ) -> Result<UserProfilePhotos, ApiError> {
        Ok(UserProfilePhotos {
            total_count: 0,
            photos: vec![],
        })
    }

    async fn get_my_commands(&self) -> Result<Vec<BotCommand>, ApiError> {
        Ok(vec![])
    }

    async fn set_my_description(
        &self,
        _desc: Option<&str>,
        _lang: Option<&str>,
    ) -> Result<(), ApiError> {
        Ok(())
    }
    async fn get_my_description(&self, _lang: Option<&str>) -> Result<BotDescription, ApiError> {
        Ok(BotDescription {
            description: String::new(),
        })
    }
    async fn set_my_short_description(
        &self,
        _desc: Option<&str>,
        _lang: Option<&str>,
    ) -> Result<(), ApiError> {
        Ok(())
    }
    async fn get_my_short_description(
        &self,
        _lang: Option<&str>,
    ) -> Result<BotShortDescription, ApiError> {
        Ok(BotShortDescription {
            short_description: String::new(),
        })
    }
    async fn set_my_name(&self, _name: Option<&str>, _lang: Option<&str>) -> Result<(), ApiError> {
        Ok(())
    }
    async fn get_my_name(&self, _lang: Option<&str>) -> Result<BotName, ApiError> {
        Ok(BotName {
            name: "MockBot".into(),
        })
    }

    async fn set_chat_menu_button(
        &self,
        _chat_id: Option<ChatId>,
        _button: MenuButton,
    ) -> Result<(), ApiError> {
        Ok(())
    }
    async fn get_chat_menu_button(&self, _chat_id: Option<ChatId>) -> Result<MenuButton, ApiError> {
        Ok(MenuButton::Default)
    }

    async fn answer_shipping_query(
        &self,
        _id: String,
        _ok: bool,
        _opts: Option<Vec<ShippingOption>>,
        _err: Option<String>,
    ) -> Result<(), ApiError> {
        Ok(())
    }

    async fn create_invoice_link(&self, _invoice: Invoice) -> Result<String, ApiError> {
        Ok("https://t.me/$mock_invoice_link".into())
    }

    async fn forward_messages(
        &self,
        _chat_id: ChatId,
        _from: ChatId,
        ids: Vec<MessageId>,
    ) -> Result<Vec<MessageId>, ApiError> {
        Ok(ids.iter().map(|_| MessageId(self.next_id())).collect())
    }

    async fn copy_messages(
        &self,
        _chat_id: ChatId,
        _from: ChatId,
        ids: Vec<MessageId>,
    ) -> Result<Vec<MessageId>, ApiError> {
        Ok(ids.iter().map(|_| MessageId(self.next_id())).collect())
    }

    async fn send_sticker(
        &self,
        chat_id: ChatId,
        _sticker: FileSource,
    ) -> Result<SentMessage, ApiError> {
        Ok(SentMessage {
            message_id: MessageId(self.next_id()),
            chat_id,
        })
    }

    async fn send_location(
        &self,
        chat_id: ChatId,
        _lat: f64,
        _lon: f64,
    ) -> Result<SentMessage, ApiError> {
        Ok(SentMessage {
            message_id: MessageId(self.next_id()),
            chat_id,
        })
    }

    async fn revoke_chat_invite_link(
        &self,
        _chat_id: ChatId,
        link: &str,
    ) -> Result<ChatInviteLink, ApiError> {
        Ok(ChatInviteLink {
            invite_link: link.to_string(),
            creator: None,
            creates_join_request: false,
            is_primary: false,
            is_revoked: true,
            name: None,
            expire_date: None,
            member_limit: None,
            pending_join_request_count: None,
        })
    }

    // Forum Topics

    async fn create_forum_topic(
        &self,
        _chat_id: ChatId,
        title: &str,
        icon_color: Option<i32>,
        icon_custom_emoji_id: Option<i64>,
    ) -> Result<ForumTopic, ApiError> {
        Ok(ForumTopic {
            id: self.next_id(),
            title: title.to_string(),
            icon_color,
            icon_custom_emoji_id: icon_custom_emoji_id.map(|id| id.to_string()),
            is_closed: false,
            is_hidden: false,
        })
    }

    async fn edit_forum_topic(
        &self,
        _chat_id: ChatId,
        _topic_id: i32,
        _title: Option<&str>,
        _icon: Option<i64>,
        _closed: Option<bool>,
        _hidden: Option<bool>,
    ) -> Result<(), ApiError> {
        Ok(())
    }

    async fn delete_forum_topic(&self, _chat_id: ChatId, _topic_id: i32) -> Result<(), ApiError> {
        Ok(())
    }

    async fn unpin_all_forum_topic_messages(
        &self,
        _chat_id: ChatId,
        _topic_id: i32,
    ) -> Result<(), ApiError> {
        Ok(())
    }

    // Stars API

    async fn get_star_transactions(
        &self,
        _offset: Option<&str>,
        _limit: Option<i32>,
    ) -> Result<StarTransactions, ApiError> {
        Ok(StarTransactions {
            balance: StarBalance {
                amount: 1000,
                nanos: 0,
            },
            transactions: vec![],
            next_offset: None,
        })
    }

    async fn refund_star_payment(
        &self,
        _user_id: UserId,
        _charge_id: &str,
    ) -> Result<(), ApiError> {
        Ok(())
    }
}
