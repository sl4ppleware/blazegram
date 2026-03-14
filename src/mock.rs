//! Mock BotApi for testing.

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

impl_mock_botapi! {
    ok_unit: [
        fn send_chat_action(chat_id: ChatId, action: ChatAction);
        fn ban_chat_member(chat_id: ChatId, user_id: UserId);
        fn unban_chat_member(chat_id: ChatId, user_id: UserId);
        fn leave_chat(chat_id: ChatId);
        fn set_my_commands(commands: Vec<BotCommand>);
        fn delete_my_commands();
        fn pin_chat_message(chat_id: ChatId, message_id: MessageId, silent: bool);
        fn unpin_chat_message(chat_id: ChatId, message_id: MessageId);
        fn set_message_reaction(chat_id: ChatId, message_id: MessageId, emoji: &str);
        fn answer_pre_checkout_query(id: String, ok: bool, error_message: Option<String>);
        fn set_chat_title(chat_id: ChatId, title: &str);
        fn set_chat_description(chat_id: ChatId, description: Option<&str>);
        fn delete_chat_photo(chat_id: ChatId);
        fn set_chat_administrator_custom_title(chat_id: ChatId, user_id: UserId, custom_title: &str);
        fn approve_chat_join_request(chat_id: ChatId, user_id: UserId);
        fn decline_chat_join_request(chat_id: ChatId, user_id: UserId);
        fn set_my_description(description: Option<&str>, language_code: Option<&str>);
        fn set_my_short_description(short_description: Option<&str>, language_code: Option<&str>);
        fn set_my_name(name: Option<&str>, language_code: Option<&str>);
        fn set_chat_menu_button(chat_id: Option<ChatId>, menu_button: MenuButton);
        fn answer_shipping_query(shipping_query_id: String, ok: bool, shipping_options: Option<Vec<ShippingOption>>, error_message: Option<String>);
        fn answer_inline_query(query_id: String, results: Vec<InlineQueryResult>, next_offset: Option<String>, cache_time: Option<i32>, is_personal: bool);
        fn edit_forum_topic(chat_id: ChatId, topic_id: i32, title: Option<&str>, icon_custom_emoji_id: Option<i64>, closed: Option<bool>, hidden: Option<bool>);
        fn delete_forum_topic(chat_id: ChatId, topic_id: i32);
        fn unpin_all_forum_topic_messages(chat_id: ChatId, topic_id: i32);
        fn refund_star_payment(user_id: UserId, charge_id: &str);
        fn restrict_chat_member(chat_id: ChatId, user_id: UserId, permissions: ChatPermissions);
        fn promote_chat_member(chat_id: ChatId, user_id: UserId, permissions: ChatPermissions);
        fn set_chat_permissions(chat_id: ChatId, permissions: ChatPermissions);
        fn unpin_all_chat_messages(chat_id: ChatId);
        fn stop_poll(chat_id: ChatId, message_id: MessageId);
        fn set_chat_photo(chat_id: ChatId, photo: FileSource);
    ]
    ok_sent: [
        fn send_poll(chat_id: ChatId, poll: SendPoll);
        fn send_dice(chat_id: ChatId, emoji: DiceEmoji);
        fn send_contact(chat_id: ChatId, contact: Contact);
        fn send_venue(chat_id: ChatId, venue: Venue);
        fn send_invoice(chat_id: ChatId, invoice: Invoice);
        fn send_sticker(chat_id: ChatId, sticker: FileSource);
        fn send_location(chat_id: ChatId, latitude: f64, longitude: f64);
    ]
    manual: {
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
            self.edits.lock().await.push((c, m, cap.unwrap_or_default()));
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

        async fn forward_message(
            &self,
            chat_id: ChatId,
            _from_chat_id: ChatId,
            _message_id: MessageId,
        ) -> Result<SentMessage, ApiError> {
            Ok(SentMessage {
                message_id: MessageId(self.next_id()),
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

        async fn get_chat_member_count(&self, _chat_id: ChatId) -> Result<i32, ApiError> {
            Ok(42)
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

        async fn get_chat_administrators(&self, _chat_id: ChatId) -> Result<Vec<ChatMember>, ApiError> {
            Ok(vec![])
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

        async fn get_my_description(&self, _lang: Option<&str>) -> Result<BotDescription, ApiError> {
            Ok(BotDescription {
                description: String::new(),
            })
        }

        async fn get_my_short_description(
            &self,
            _lang: Option<&str>,
        ) -> Result<BotShortDescription, ApiError> {
            Ok(BotShortDescription {
                short_description: String::new(),
            })
        }

        async fn get_my_name(&self, _lang: Option<&str>) -> Result<BotName, ApiError> {
            Ok(BotName {
                name: "MockBot".into(),
            })
        }

        async fn get_chat_menu_button(&self, _chat_id: Option<ChatId>) -> Result<MenuButton, ApiError> {
            Ok(MenuButton::Default)
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

        async fn send_media_group(
            &self,
            _chat_id: ChatId,
            _media: Vec<MediaGroupItem>,
        ) -> Result<Vec<SentMessage>, ApiError> {
            Ok(vec![])
        }

        async fn get_chat_member(
            &self,
            _chat_id: ChatId,
            user_id: UserId,
        ) -> Result<ChatMember, ApiError> {
            Ok(ChatMember {
                user: UserInfo {
                    id: user_id,
                    first_name: "Mock".to_string(),
                    last_name: None,
                    username: None,
                    language_code: None,
                },
                status: ChatMemberStatus::Member,
            })
        }

        async fn get_chat(&self, chat_id: ChatId) -> Result<ChatInfo, ApiError> {
            Ok(ChatInfo {
                id: chat_id,
                chat_type: ChatType::Private,
                title: Some("Mock Chat".to_string()),
                username: None,
                first_name: None,
                last_name: None,
                member_count: None,
            })
        }

        async fn create_chat_invite_link(
            &self,
            _chat_id: ChatId,
            _name: Option<&str>,
            _expire_date: Option<i64>,
            _member_limit: Option<i32>,
        ) -> Result<String, ApiError> {
            Ok("https://t.me/+mock_invite_link".to_string())
        }

        async fn export_chat_invite_link(&self, _chat_id: ChatId) -> Result<String, ApiError> {
            Ok("https://t.me/+mock_export_link".to_string())
        }
    }
}
