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
        fn edit_message_live_location(chat_id: ChatId, message_id: MessageId, latitude: f64, longitude: f64);
        fn stop_message_live_location(chat_id: ChatId, message_id: MessageId);
        fn edit_message_checklist(chat_id: ChatId, message_id: MessageId, title: String, items: Vec<ChecklistItem>);
        fn send_message_draft(chat_id: ChatId, text: String, parse_mode: ParseMode);
        fn set_user_emoji_status(user_id: UserId, emoji_status_custom_emoji_id: Option<String>, emoji_status_expiration_date: Option<i64>);
        fn log_out();
        fn close();
        fn restrict_chat_member(chat_id: ChatId, user_id: UserId, permissions: ChatPermissions);
        fn promote_chat_member(chat_id: ChatId, user_id: UserId, permissions: ChatPermissions);
        fn set_chat_permissions(chat_id: ChatId, permissions: ChatPermissions);
        fn unpin_all_chat_messages(chat_id: ChatId);
        fn stop_poll(chat_id: ChatId, message_id: MessageId);
        fn set_chat_photo(chat_id: ChatId, photo: FileSource);
        fn ban_chat_sender_chat(chat_id: ChatId, sender_chat_id: ChatId);
        fn unban_chat_sender_chat(chat_id: ChatId, sender_chat_id: ChatId);
        fn set_chat_member_tag(chat_id: ChatId, user_id: UserId, tag: Option<String>);
        fn verify_user(user_id: UserId, custom_description: Option<String>);
        fn verify_chat(chat_id: ChatId, custom_description: Option<String>);
        fn remove_user_verification(user_id: UserId);
        fn remove_chat_verification(chat_id: ChatId);
        fn read_business_message(business_connection_id: &str, chat_id: ChatId, message_id: MessageId);
        fn delete_business_messages(business_connection_id: &str, message_ids: Vec<MessageId>);
        fn set_business_account_name(business_connection_id: &str, first_name: &str, last_name: Option<&str>);
        fn set_business_account_username(business_connection_id: &str, username: Option<&str>);
        fn set_business_account_bio(business_connection_id: &str, bio: Option<&str>);
        fn set_business_account_profile_photo(business_connection_id: &str, photo: FileSource, is_public: Option<bool>);
        fn remove_business_account_profile_photo(business_connection_id: &str, is_public: Option<bool>);
        fn set_business_account_gift_settings(business_connection_id: &str, show_gift_button: bool, accepted_gift_types: AcceptedGiftTypes);
        fn transfer_business_account_stars(business_connection_id: &str, star_count: i64);
        fn send_gift(user_id: UserId, gift_id: String, text: Option<String>, text_parse_mode: Option<ParseMode>);
        fn gift_premium_subscription(user_id: UserId, month_count: i32, star_count: i64, text: Option<String>, text_parse_mode: Option<ParseMode>);
        fn convert_gift_to_stars(business_connection_id: Option<&str>, owned_gift_id: &str);
        fn upgrade_gift(business_connection_id: Option<&str>, owned_gift_id: &str, keep_original_details: Option<bool>, star_count: Option<i64>);
        fn transfer_gift(business_connection_id: Option<&str>, owned_gift_id: &str, new_owner_chat_id: ChatId, star_count: Option<i64>);
        fn delete_story(chat_id: ChatId, story_id: i32);
        fn edit_user_star_subscription(user_id: UserId, telegram_payment_charge_id: &str, is_canceled: bool);
        fn set_my_default_administrator_rights(rights: Option<ChatPermissions>, for_channels: Option<bool>);
        fn set_my_profile_photo(photo: FileSource, is_public: Option<bool>);
        fn remove_my_profile_photo(file_id: Option<String>);
        fn set_sticker_position_in_set(sticker: &str, position: i32);
        fn delete_sticker_from_set(sticker: &str);
        fn replace_sticker_in_set(user_id: UserId, name: &str, old_sticker: &str, sticker: InputSticker);
        fn set_sticker_emoji_list(sticker: &str, emoji_list: Vec<String>);
        fn set_sticker_keywords(sticker: &str, keywords: Vec<String>);
        fn set_sticker_mask_position(sticker: &str, mask_position: Option<MaskPosition>);
        fn set_sticker_set_title(name: &str, title: &str);
        fn set_sticker_set_thumbnail(name: &str, user_id: UserId, thumbnail: Option<FileSource>, format: StickerFormat);
        fn set_custom_emoji_sticker_set_thumbnail(name: &str, custom_emoji_id: Option<String>);
        fn delete_sticker_set(name: &str);
        fn create_new_sticker_set(user_id: UserId, name: String, title: String, stickers: Vec<InputSticker>, sticker_type: Option<StickerType>);
        fn add_sticker_to_set(user_id: UserId, name: &str, sticker: InputSticker);
        fn set_chat_sticker_set(chat_id: ChatId, sticker_set_name: &str);
        fn delete_chat_sticker_set(chat_id: ChatId);
        fn set_game_score(user_id: UserId, score: i64, chat_id: ChatId, message_id: MessageId, force: bool, disable_edit_message: bool);
        fn approve_suggested_post(chat_id: ChatId, message_id: MessageId);
        fn decline_suggested_post(chat_id: ChatId, message_id: MessageId);
        fn set_passport_data_errors(user_id: UserId, errors: Vec<PassportElementError>);
    ]
    ok_sent: [
        fn send_poll(chat_id: ChatId, poll: SendPoll);
        fn send_dice(chat_id: ChatId, emoji: DiceEmoji);
        fn send_contact(chat_id: ChatId, contact: Contact);
        fn send_venue(chat_id: ChatId, venue: Venue);
        fn send_invoice(chat_id: ChatId, invoice: Invoice);
        fn send_sticker(chat_id: ChatId, sticker: FileSource);
        fn send_location(chat_id: ChatId, latitude: f64, longitude: f64);
        fn send_paid_media(chat_id: ChatId, star_count: i64, media: Vec<PaidMediaInput>, caption: Option<String>, parse_mode: ParseMode, opts: SendOptions);
        fn send_live_location(chat_id: ChatId, latitude: f64, longitude: f64, live_period: i32, opts: SendOptions);
        fn send_checklist(chat_id: ChatId, title: String, items: Vec<ChecklistItem>, opts: SendOptions);
        fn send_game(chat_id: ChatId, game_short_name: &str, opts: SendOptions);
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

        async fn get_user_profile_audios(
            &self,
            _user_id: UserId,
            _offset: Option<i32>,
            _limit: Option<i32>,
        ) -> Result<UserProfileAudios, ApiError> {
            Ok(UserProfileAudios {
                total_count: 0,
                audios: vec![],
            })
        }

        async fn edit_chat_invite_link(
            &self,
            _chat_id: ChatId,
            link: &str,
            name: Option<&str>,
            _expire_date: Option<i64>,
            _member_limit: Option<i32>,
        ) -> Result<ChatInviteLink, ApiError> {
            Ok(ChatInviteLink {
                invite_link: link.to_string(),
                creator: None,
                creates_join_request: false,
                is_primary: false,
                is_revoked: false,
                name: name.map(|s| s.to_string()),
                expire_date: None,
                member_limit: None,
                pending_join_request_count: None,
            })
        }

        async fn create_chat_subscription_invite_link(
            &self,
            _chat_id: ChatId,
            name: Option<&str>,
            _subscription_period: i32,
            _subscription_price: i64,
        ) -> Result<ChatInviteLink, ApiError> {
            Ok(ChatInviteLink {
                invite_link: "https://t.me/+mock_sub_link".to_string(),
                creator: None,
                creates_join_request: false,
                is_primary: false,
                is_revoked: false,
                name: name.map(|s| s.to_string()),
                expire_date: None,
                member_limit: None,
                pending_join_request_count: None,
            })
        }

        async fn edit_chat_subscription_invite_link(
            &self,
            _chat_id: ChatId,
            link: &str,
            name: Option<&str>,
        ) -> Result<ChatInviteLink, ApiError> {
            Ok(ChatInviteLink {
                invite_link: link.to_string(),
                creator: None,
                creates_join_request: false,
                is_primary: false,
                is_revoked: false,
                name: name.map(|s| s.to_string()),
                expire_date: None,
                member_limit: None,
                pending_join_request_count: None,
            })
        }

        async fn get_user_chat_boosts(
            &self,
            _chat_id: ChatId,
            _user_id: UserId,
        ) -> Result<UserChatBoosts, ApiError> {
            Ok(UserChatBoosts { boosts: vec![] })
        }

        async fn get_my_default_administrator_rights(
            &self,
            _for_channels: Option<bool>,
        ) -> Result<ChatPermissions, ApiError> {
            Ok(ChatPermissions::default())
        }

        async fn get_business_connection(
            &self,
            id: &str,
        ) -> Result<BusinessConnection, ApiError> {
            Ok(BusinessConnection {
                id: id.to_string(),
                user: UserInfo {
                    id: UserId(1),
                    first_name: "Mock".to_string(),
                    last_name: None,
                    username: None,
                    language_code: None,
                },
                user_chat_id: ChatId(1),
                date: 0,
                can_reply: true,
                is_enabled: true,
            })
        }

        async fn get_business_account_star_balance(
            &self,
            _business_connection_id: &str,
        ) -> Result<StarBalance, ApiError> {
            Ok(StarBalance { amount: 100, nanos: 0 })
        }

        async fn get_business_account_gifts(
            &self,
            _business_connection_id: &str,
            _exclude_unsaved: Option<bool>,
            _exclude_saved: Option<bool>,
            _exclude_unlimited: Option<bool>,
            _exclude_limited: Option<bool>,
            _exclude_unique: Option<bool>,
            _sort_by_price: Option<bool>,
            _offset: Option<&str>,
            _limit: Option<i32>,
        ) -> Result<OwnedGifts, ApiError> {
            Ok(OwnedGifts { total_count: 0, gifts: vec![], next_offset: None })
        }

        async fn get_available_gifts(&self) -> Result<Vec<Gift>, ApiError> {
            Ok(vec![])
        }

        async fn get_user_gifts(
            &self,
            _user_id: UserId,
            _offset: Option<&str>,
            _limit: Option<i32>,
        ) -> Result<OwnedGifts, ApiError> {
            Ok(OwnedGifts { total_count: 0, gifts: vec![], next_offset: None })
        }

        async fn get_chat_gifts(
            &self,
            _chat_id: ChatId,
            _offset: Option<&str>,
            _limit: Option<i32>,
        ) -> Result<OwnedGifts, ApiError> {
            Ok(OwnedGifts { total_count: 0, gifts: vec![], next_offset: None })
        }

        async fn post_story(
            &self,
            chat_id: ChatId,
            _content: StoryContent,
            _active_period: i32,
            _caption: Option<String>,
            _parse_mode: Option<ParseMode>,
        ) -> Result<Story, ApiError> {
            Ok(Story { id: self.next_id(), chat_id, date: 0 })
        }

        async fn edit_story(
            &self,
            chat_id: ChatId,
            story_id: i32,
            _content: Option<StoryContent>,
            _caption: Option<String>,
            _parse_mode: Option<ParseMode>,
        ) -> Result<Story, ApiError> {
            Ok(Story { id: story_id, chat_id, date: 0 })
        }

        async fn get_my_star_balance(&self) -> Result<StarBalance, ApiError> {
            Ok(StarBalance { amount: 1000, nanos: 0 })
        }

        async fn get_managed_bot_token(&self, _bot_id: UserId) -> Result<String, ApiError> {
            Ok("mock_token_123".to_string())
        }

        async fn replace_managed_bot_token(&self, _bot_id: UserId) -> Result<String, ApiError> {
            Ok("mock_new_token_456".to_string())
        }

        async fn save_prepared_keyboard_button(
            &self,
            _user_id: UserId,
            _button: PreparedKeyboardButtonData,
        ) -> Result<PreparedKeyboardButton, ApiError> {
            Ok(PreparedKeyboardButton { id: "pkb_1".to_string(), expiration_date: 9999999999 })
        }

        async fn get_sticker_set(&self, name: &str) -> Result<StickerSet, ApiError> {
            Ok(StickerSet {
                name: name.to_string(),
                title: "Mock Set".to_string(),
                sticker_type: StickerType::Regular,
                stickers: vec![],
            })
        }

        async fn get_custom_emoji_stickers(&self, _ids: Vec<String>) -> Result<Vec<StickerInfo>, ApiError> {
            Ok(vec![])
        }

        async fn upload_sticker_file(
            &self,
            _user_id: UserId,
            _sticker: FileSource,
            _format: StickerFormat,
        ) -> Result<TelegramFile, ApiError> {
            Ok(TelegramFile {
                file_id: "mock_file_id".to_string(),
                file_unique_id: "mock_unique".to_string(),
                file_size: Some(1024),
                file_path: Some("stickers/mock.webp".to_string()),
            })
        }

        async fn get_forum_topic_icon_stickers(&self) -> Result<Vec<StickerInfo>, ApiError> {
            Ok(vec![])
        }

        async fn get_game_high_scores(
            &self,
            _user_id: UserId,
            _chat_id: ChatId,
            _message_id: MessageId,
        ) -> Result<Vec<GameHighScore>, ApiError> {
            Ok(vec![])
        }

        async fn answer_web_app_query(
            &self,
            _web_app_query_id: &str,
            _result: InlineQueryResult,
        ) -> Result<SentWebAppMessage, ApiError> {
            Ok(SentWebAppMessage { inline_message_id: Some("mock_inline_msg".to_string()) })
        }

        async fn save_prepared_inline_message(
            &self,
            _user_id: UserId,
            _result: InlineQueryResult,
            _allow_user_chats: Option<bool>,
            _allow_bot_chats: Option<bool>,
            _allow_group_chats: Option<bool>,
            _allow_channel_chats: Option<bool>,
        ) -> Result<PreparedInlineMessage, ApiError> {
            Ok(PreparedInlineMessage {
                id: "mock_prepared_1".to_string(),
                expiration_date: 9999999999,
            })
        }
    }
}
