use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

/// Fixed-seed FNV-1a hasher for deterministic content hashing across restarts.
/// DefaultHasher uses random SipHash keys per process — breaks tracked message
/// comparison after restart.
fn new_fixed_hasher() -> FixedHasher {
    FixedHasher(0xcbf29ce484222325)
}

struct FixedHasher(u64);

impl Hasher for FixedHasher {
    fn finish(&self) -> u64 { self.0 }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 ^= b as u64;
            self.0 = self.0.wrapping_mul(0x100000001b3);
        }
    }
}
use std::path::PathBuf;

// ─── IDs ───

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MessageId(pub i32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChatId(pub i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UserId(pub u64);

// ─── Screen ───

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ScreenId(pub Cow<'static, str>);

impl From<&'static str> for ScreenId {
    fn from(s: &'static str) -> Self {
        Self(Cow::Borrowed(s))
    }
}

impl From<String> for ScreenId {
    fn from(s: String) -> Self {
        Self(Cow::Owned(s))
    }
}

impl std::fmt::Display for ScreenId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ─── Content Types ───

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ContentType {
    Text,
    Photo,
    Video,
    Animation,
    Document,
    Sticker,
    Voice,
    VideoNote,
    Audio,
    Location,
    Venue,
    Contact,
    Poll,
    Dice,
}

impl ContentType {
    /// Can we edit from self → target without delete+send?
    pub fn can_edit_to(&self, target: &ContentType) -> bool {
        use ContentType::*;
        match (self, target) {
            (Text, Text) => true,
            // Media ↔ Media via editMessageMedia
            (Photo | Video | Animation | Document, Photo | Video | Animation | Document) => true,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum ParseMode {
    #[default]
    Html,
    MarkdownV2,
    None,
}


#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum LinkPreview {
    Enabled,
    #[default]
    Disabled,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileSource {
    FileId(String),
    Url(String),
    LocalPath(PathBuf),
    Bytes { data: Vec<u8>, filename: String },
}

impl PartialEq for FileSource {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::FileId(a), Self::FileId(b)) => a == b,
            (Self::Url(a), Self::Url(b)) => a == b,
            (Self::LocalPath(a), Self::LocalPath(b)) => a == b,
            (Self::Bytes { data: d1, filename: f1 }, Self::Bytes { data: d2, filename: f2 }) => d1 == d2 && f1 == f2,
            _ => false,
        }
    }
}

impl Eq for FileSource {}

impl Hash for FileSource {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Self::FileId(id) => { 0u8.hash(state); id.hash(state); }
            Self::Url(url) => { 1u8.hash(state); url.hash(state); }
            Self::LocalPath(p) => { 2u8.hash(state); p.hash(state); }
            Self::Bytes { data, filename } => { 3u8.hash(state); data.hash(state); filename.hash(state); }
        }
    }
}

impl From<&str> for FileSource {
    fn from(s: &str) -> Self {
        if s.starts_with("http://") || s.starts_with("https://") {
            Self::Url(s.to_string())
        } else if s.contains('/') || s.contains('\\') {
            Self::LocalPath(PathBuf::from(s))
        } else {
            Self::FileId(s.to_string())
        }
    }
}

impl From<String> for FileSource {
    fn from(s: String) -> Self {
        Self::from(s.as_str())
    }
}

// ─── Message Content ───

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageContent {
    Text {
        text: String,
        parse_mode: ParseMode,
        keyboard: Option<crate::keyboard::InlineKeyboard>,
        link_preview: LinkPreview,
    },
    Photo {
        source: FileSource,
        caption: Option<String>,
        parse_mode: ParseMode,
        keyboard: Option<crate::keyboard::InlineKeyboard>,
        spoiler: bool,
    },
    Video {
        source: FileSource,
        caption: Option<String>,
        parse_mode: ParseMode,
        keyboard: Option<crate::keyboard::InlineKeyboard>,
        spoiler: bool,
    },
    Animation {
        source: FileSource,
        caption: Option<String>,
        parse_mode: ParseMode,
        keyboard: Option<crate::keyboard::InlineKeyboard>,
        spoiler: bool,
    },
    Document {
        source: FileSource,
        caption: Option<String>,
        parse_mode: ParseMode,
        keyboard: Option<crate::keyboard::InlineKeyboard>,
        filename: Option<String>,
    },
    Sticker {
        source: FileSource,
    },
    Location {
        latitude: f64,
        longitude: f64,
        keyboard: Option<crate::keyboard::InlineKeyboard>,
    },
}

impl MessageContent {
    pub fn content_type(&self) -> ContentType {
        match self {
            Self::Text { .. } => ContentType::Text,
            Self::Photo { .. } => ContentType::Photo,
            Self::Video { .. } => ContentType::Video,
            Self::Animation { .. } => ContentType::Animation,
            Self::Document { .. } => ContentType::Document,
            Self::Sticker { .. } => ContentType::Sticker,
            Self::Location { .. } => ContentType::Location,
        }
    }

    pub fn content_hash(&self) -> u64 {
        
        let mut hasher = new_fixed_hasher();
        self.content_type().hash(&mut hasher);
        match self {
            Self::Text { text, parse_mode, keyboard, link_preview } => {
                text.hash(&mut hasher);
                parse_mode.hash(&mut hasher);
                if let Some(kb) = keyboard { kb.hash(&mut hasher); }
                link_preview.hash(&mut hasher);
            }
            Self::Photo { source, caption, keyboard, spoiler, .. } => {
                source.hash(&mut hasher);
                caption.hash(&mut hasher);
                if let Some(kb) = keyboard { kb.hash(&mut hasher); }
                spoiler.hash(&mut hasher);
            }
            Self::Video { source, caption, keyboard, spoiler, .. } => {
                source.hash(&mut hasher);
                caption.hash(&mut hasher);
                if let Some(kb) = keyboard { kb.hash(&mut hasher); }
                spoiler.hash(&mut hasher);
            }
            Self::Animation { source, caption, keyboard, spoiler, .. } => {
                source.hash(&mut hasher);
                caption.hash(&mut hasher);
                if let Some(kb) = keyboard { kb.hash(&mut hasher); }
                spoiler.hash(&mut hasher);
            }
            Self::Document { source, caption, keyboard, filename, .. } => {
                source.hash(&mut hasher);
                caption.hash(&mut hasher);
                if let Some(kb) = keyboard { kb.hash(&mut hasher); }
                filename.hash(&mut hasher);
            }
            Self::Sticker { source } => { source.hash(&mut hasher); }
            Self::Location { latitude, longitude, keyboard } => {
                latitude.to_bits().hash(&mut hasher);
                longitude.to_bits().hash(&mut hasher);
                if let Some(kb) = keyboard { kb.hash(&mut hasher); }
            }
        }
        hasher.finish()
    }

    pub fn text_hash(&self) -> u64 {
        let mut hasher = new_fixed_hasher();
        match self {
            Self::Text { text, parse_mode, .. } => {
                1u8.hash(&mut hasher); // discriminant: has text
                text.hash(&mut hasher);
                parse_mode.hash(&mut hasher);
            }
            _ => {
                0u8.hash(&mut hasher); // discriminant: no text
            }
        }
        hasher.finish()
    }

    pub fn caption(&self) -> Option<String> {
        match self {
            Self::Photo { caption, .. } | Self::Video { caption, .. }
            | Self::Animation { caption, .. } | Self::Document { caption, .. } => caption.clone(),
            _ => None,
        }
    }

    pub fn keyboard(&self) -> Option<crate::keyboard::InlineKeyboard> {
        match self {
            Self::Text { keyboard, .. } | Self::Photo { keyboard, .. }
            | Self::Video { keyboard, .. } | Self::Animation { keyboard, .. }
            | Self::Document { keyboard, .. } | Self::Location { keyboard, .. } => keyboard.clone(),
            _ => None,
        }
    }

    pub fn keyboard_hash(&self) -> u64 {
        let mut hasher = new_fixed_hasher();
        match self.keyboard() {
            Some(kb) => { 1u8.hash(&mut hasher); kb.hash(&mut hasher); }
            None => { 0u8.hash(&mut hasher); }
        }
        hasher.finish()
    }

    /// Return a copy with HTML tags stripped and ParseMode::None.
    /// Used as fallback when Telegram rejects entity boundaries.
    pub fn as_plain_text(&self) -> Self {
        fn strip(html: &str) -> String {
            let mut out = String::with_capacity(html.len());
            let mut inside_tag = false;
            for ch in html.chars() {
                match ch {
                    '<' => inside_tag = true,
                    '>' if inside_tag => inside_tag = false,
                    _ if !inside_tag => out.push(ch),
                    _ => {}
                }
            }
            // Unescape HTML entities
            out.replace("&lt;", "<")
               .replace("&gt;", ">")
               .replace("&amp;", "&")
               .replace("&quot;", "\"")
        }
        match self.clone() {
            Self::Text { text, keyboard, link_preview, .. } => Self::Text {
                text: strip(&text), parse_mode: ParseMode::None, keyboard, link_preview,
            },
            Self::Photo { source, caption, keyboard, spoiler, .. } => Self::Photo {
                source, caption: caption.map(|c| strip(&c)), parse_mode: ParseMode::None, keyboard, spoiler,
            },
            Self::Video { source, caption, keyboard, spoiler, .. } => Self::Video {
                source, caption: caption.map(|c| strip(&c)), parse_mode: ParseMode::None, keyboard, spoiler,
            },
            Self::Animation { source, caption, keyboard, spoiler, .. } => Self::Animation {
                source, caption: caption.map(|c| strip(&c)), parse_mode: ParseMode::None, keyboard, spoiler,
            },
            Self::Document { source, caption, keyboard, filename, .. } => Self::Document {
                source, caption: caption.map(|c| strip(&c)), parse_mode: ParseMode::None, keyboard, filename,
            },
            other => other, // Sticker, Location — no text
        }
    }

    pub fn caption_hash(&self) -> u64 {
        let mut hasher = new_fixed_hasher();
        match self.caption() {
            Some(cap) => { 1u8.hash(&mut hasher); cap.hash(&mut hasher); }
            None => { 0u8.hash(&mut hasher); }
        }
        hasher.finish()
    }

    pub fn file_hash(&self) -> u64 {
        let mut hasher = new_fixed_hasher();
        match self {
            Self::Photo { source, .. } | Self::Video { source, .. }
            | Self::Animation { source, .. } | Self::Document { source, .. }
            | Self::Sticker { source } => { 1u8.hash(&mut hasher); source.hash(&mut hasher); }
            _ => { 0u8.hash(&mut hasher); }
        }
        hasher.finish()
    }
}

// ─── Chat Action ───

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatAction {
    Typing,
    UploadPhoto,
    UploadVideo,
    UploadDocument,
    FindLocation,
    RecordVoice,
    RecordVideo,
}

// ─── Input Spec ───

/// What input the current screen expects from the user.
#[derive(Clone)]
pub enum InputSpec {
    Text {
        validator: Option<ValidatorFn>,
        placeholder: Option<String>,
    },
    Photo,
    Video,
    Document,
    Location,
    Contact,
    Choice {
        options: Vec<String>,
    },
}

impl std::fmt::Debug for InputSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Text { placeholder, .. } => f.debug_struct("Text").field("placeholder", placeholder).finish(),
            Self::Photo => write!(f, "Photo"),
            Self::Video => write!(f, "Video"),
            Self::Document => write!(f, "Document"),
            Self::Location => write!(f, "Location"),
            Self::Contact => write!(f, "Contact"),
            Self::Choice { options } => f.debug_struct("Choice").field("options", options).finish(),
        }
    }
}

pub type ValidatorFn = std::sync::Arc<dyn Fn(&str) -> Result<(), String> + Send + Sync>;

// ─── User Info ───

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub id: UserId,
    pub first_name: String,
    pub last_name: Option<String>,
    pub username: Option<String>,
    pub language_code: Option<String>,
}

impl UserInfo {
    pub fn full_name(&self) -> String {
        match &self.last_name {
            Some(last) => format!("{} {}", self.first_name, last),
            None => self.first_name.clone(),
        }
    }
}

// ─── Tracked Message ───

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackedMessage {
    pub message_id: MessageId,
    pub content_type: ContentType,
    pub content_hash: u64,
    pub text_hash: u64,
    pub caption_hash: u64,
    pub file_hash: u64,
    pub keyboard_hash: u64,
}

impl TrackedMessage {
    pub fn from_content(message_id: MessageId, content: &MessageContent) -> Self {
        Self {
            message_id,
            content_type: content.content_type(),
            content_hash: content.content_hash(),
            text_hash: content.text_hash(),
            caption_hash: content.caption_hash(),
            file_hash: content.file_hash(),
            keyboard_hash: content.keyboard_hash(),
        }
    }
}

// ─── Sent Message ───

#[derive(Debug, Clone)]
pub struct SentMessage {
    pub message_id: MessageId,
    pub chat_id: ChatId,
}

// ─── Chat State ───

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatState {
    pub chat_id: ChatId,
    pub current_screen: ScreenId,
    pub active_bot_messages: Vec<TrackedMessage>,
    pub pending_user_messages: Vec<MessageId>,
    #[serde(skip)]
    pub pending_callback_id: Option<String>,
    pub data: HashMap<String, serde_json::Value>,
    pub screen_stack: Vec<ScreenId>,
    pub user: UserInfo,
    /// Message IDs that are "frozen" — the differ will not delete them.
    /// Used for conversation history, receipts, etc.
    #[serde(default)]
    pub frozen_messages: Vec<MessageId>,

    /// Current reply message (for ctx.reply() — send once, edit on repeat).
    #[serde(default)]
    pub(crate) reply_message_id: Option<MessageId>,
    /// Whether the reply from the previous handler call is sealed (next reply() sends new).
    #[serde(default = "default_true")]
    pub(crate) reply_sealed: bool,
}

impl ChatState {
    pub fn new(chat_id: ChatId, user: UserInfo) -> Self {
        Self {
            chat_id,
            current_screen: ScreenId::from("__initial__"),
            active_bot_messages: Vec::new(),
            pending_user_messages: Vec::new(),
            pending_callback_id: None,
            data: HashMap::new(),
            screen_stack: Vec::new(),
            user,
            frozen_messages: Vec::new(),
            reply_message_id: None,
            reply_sealed: true,
        }
    }
}

// ─── Incoming Update ───

#[derive(Debug, Clone)]
pub enum IncomingUpdate {
    Message {
        message_id: MessageId,
        chat_id: ChatId,
        user: UserInfo,
        text: Option<String>,
    },
    CallbackQuery {
        id: String,
        chat_id: ChatId,
        user: UserInfo,
        data: Option<String>,
        message_id: Option<MessageId>,
        /// For callbacks on inline messages — the packed inline message ID.
        inline_message_id: Option<String>,
    },
    Photo {
        message_id: MessageId,
        chat_id: ChatId,
        user: UserInfo,
        file_id: String,
        file_unique_id: String,
        caption: Option<String>,
    },
    Document {
        message_id: MessageId,
        chat_id: ChatId,
        user: UserInfo,
        file_id: String,
        file_unique_id: String,
        filename: Option<String>,
        caption: Option<String>,
    },
    InlineQuery {
        id: String,
        user: UserInfo,
        query: String,
        offset: String,
    },
    ChosenInlineResult {
        result_id: String,
        user: UserInfo,
        inline_message_id: Option<String>,
        query: String,
    },
    PreCheckoutQuery {
        id: String,
        chat_id: ChatId,
        user: UserInfo,
        currency: String,
        total_amount: i64,
        payload: String,
    },
    SuccessfulPayment {
        chat_id: ChatId,
        user: UserInfo,
        currency: String,
        total_amount: i64,
        payload: String,
    },
    WebAppData {
        chat_id: ChatId,
        user: UserInfo,
        data: String,
    },
    /// A message was edited by the user.
    MessageEdited {
        message_id: MessageId,
        chat_id: ChatId,
        user: UserInfo,
        text: Option<String>,
    },
    /// Voice message received.
    Voice {
        message_id: MessageId,
        chat_id: ChatId,
        user: UserInfo,
        file_id: String,
        file_unique_id: String,
        duration: i32,
        caption: Option<String>,
    },
    /// Video note (round video) received.
    VideoNote {
        message_id: MessageId,
        chat_id: ChatId,
        user: UserInfo,
        file_id: String,
        file_unique_id: String,
        duration: i32,
    },
    /// Video received.
    Video {
        message_id: MessageId,
        chat_id: ChatId,
        user: UserInfo,
        file_id: String,
        file_unique_id: String,
        caption: Option<String>,
    },
    /// Sticker received.
    Sticker {
        message_id: MessageId,
        chat_id: ChatId,
        user: UserInfo,
        file_id: String,
        file_unique_id: String,
    },
    /// Contact shared by the user.
    ContactReceived {
        message_id: MessageId,
        chat_id: ChatId,
        user: UserInfo,
        contact: Contact,
    },
    /// Location shared by the user.
    LocationReceived {
        message_id: MessageId,
        chat_id: ChatId,
        user: UserInfo,
        latitude: f64,
        longitude: f64,
    },
    /// A new member joined the chat (including the bot itself).
    ChatMemberJoined {
        chat_id: ChatId,
        user: UserInfo,
    },
    /// A member left the chat (including the bot itself).
    ChatMemberLeft {
        chat_id: ChatId,
        user: UserInfo,
    },
}

impl IncomingUpdate {
    pub fn chat_id(&self) -> ChatId {
        match self {
            Self::Message { chat_id, .. }
            | Self::CallbackQuery { chat_id, .. }
            | Self::Photo { chat_id, .. }
            | Self::Document { chat_id, .. }
            | Self::PreCheckoutQuery { chat_id, .. }
            | Self::SuccessfulPayment { chat_id, .. }
            | Self::WebAppData { chat_id, .. } => *chat_id,
            Self::InlineQuery { user, .. }
            | Self::ChosenInlineResult { user, .. } => ChatId(user.id.0 as i64),
            Self::MessageEdited { chat_id, .. }
            | Self::Voice { chat_id, .. }
            | Self::VideoNote { chat_id, .. }
            | Self::Video { chat_id, .. }
            | Self::Sticker { chat_id, .. }
            | Self::ContactReceived { chat_id, .. }
            | Self::LocationReceived { chat_id, .. }
            | Self::ChatMemberJoined { chat_id, .. }
            | Self::ChatMemberLeft { chat_id, .. } => *chat_id,
        }
    }

    pub fn user(&self) -> &UserInfo {
        match self {
            Self::Message { user, .. }
            | Self::CallbackQuery { user, .. }
            | Self::Photo { user, .. }
            | Self::Document { user, .. }
            | Self::InlineQuery { user, .. }
            | Self::ChosenInlineResult { user, .. }
            | Self::PreCheckoutQuery { user, .. }
            | Self::SuccessfulPayment { user, .. }
            | Self::WebAppData { user, .. }
            | Self::MessageEdited { user, .. }
            | Self::Voice { user, .. }
            | Self::VideoNote { user, .. }
            | Self::Video { user, .. }
            | Self::Sticker { user, .. }
            | Self::ContactReceived { user, .. }
            | Self::LocationReceived { user, .. }
            | Self::ChatMemberJoined { user, .. }
            | Self::ChatMemberLeft { user, .. } => user,
        }
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Message { .. } => "message",
            Self::CallbackQuery { .. } => "callback_query",
            Self::Photo { .. } => "photo",
            Self::Document { .. } => "document",
            Self::InlineQuery { .. } => "inline_query",
            Self::ChosenInlineResult { .. } => "chosen_inline_result",
            Self::PreCheckoutQuery { .. } => "pre_checkout_query",
            Self::SuccessfulPayment { .. } => "successful_payment",
            Self::WebAppData { .. } => "web_app_data",
            Self::MessageEdited { .. } => "message_edited",
            Self::Voice { .. } => "voice",
            Self::VideoNote { .. } => "video_note",
            Self::Video { .. } => "video",
            Self::Sticker { .. } => "sticker",
            Self::ContactReceived { .. } => "contact",
            Self::LocationReceived { .. } => "location",
            Self::ChatMemberJoined { .. } => "chat_member_joined",
            Self::ChatMemberLeft { .. } => "chat_member_left",
        }
    }

    /// Extract deep link payload from /start command.
    /// e.g. "/start ref_123" → Some("ref_123")
    /// Extract deep link payload from /start command.
    /// Handles both `/start ref_123` and `/start@botname ref_123`.
    pub fn deep_link(&self) -> Option<&str> {
        match self {
            Self::Message { text: Some(text), .. } => {
                let text = text.trim();
                let rest = text.strip_prefix("/start")?;
                // Skip optional @botname
                let rest = if let Some(after_at) = rest.strip_prefix('@') {
                    after_at.find(' ').map(|i| &after_at[i..]).unwrap_or("")
                } else {
                    rest
                };
                let payload = rest.trim();
                if payload.is_empty() { None } else { Some(payload) }
            }
            _ => None,
        }
    }
}

// ─── Received Media (for input handlers) ───

#[derive(Debug, Clone)]
pub struct ReceivedMedia {
    pub file_id: String,
    pub file_unique_id: String,
    pub file_type: ContentType,
    pub caption: Option<String>,
    pub filename: Option<String>,
}

pub type JsonMap = HashMap<String, serde_json::Value>;

fn default_true() -> bool { true }

// ─── Inline Query Result ───

/// A single result for answering an inline query.
#[derive(Debug, Clone)]
pub struct InlineQueryResult {
    pub id: String,
    pub kind: InlineResultKind,
    pub title: Option<String>,
    pub description: Option<String>,
    pub thumb_url: Option<String>,
    /// Message content (text + keyboard).
    pub message_text: Option<String>,
    pub parse_mode: ParseMode,
    pub keyboard: Option<crate::keyboard::InlineKeyboard>,
}

#[derive(Debug, Clone)]
pub enum InlineResultKind {
    Article,
    Photo { photo_url: String, width: Option<i32>, height: Option<i32> },
    Gif { gif_url: String },
}

// ─── Ctx Mode ───

/// How the Ctx operates — determined automatically from the update source.
#[derive(Debug, Clone)]
#[derive(Default)]
pub enum CtxMode {
    /// Private chat — full differ (delete/edit/send).
    #[default]
    Private,
    /// Group/supergroup — edit in-place, no deletion of other messages.
    Group { trigger_message_id: Option<MessageId> },
    /// Inline message — edit via inline_message_id.
    Inline { inline_message_id: String },
}


// ─── Poll ───

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollOption {
    pub text: String,
}

/// Configuration for sending a poll.
#[derive(Debug, Clone)]
pub struct SendPoll {
    pub question: String,
    pub options: Vec<String>,
    pub is_anonymous: bool,
    /// "regular" or "quiz"
    pub poll_type: PollType,
    /// For quiz: index of the correct option.
    pub correct_option_id: Option<usize>,
    pub explanation: Option<String>,
    /// 0 = no limit
    pub open_period: Option<i32>,
    pub allows_multiple_answers: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PollType {
    Regular,
    Quiz,
}

impl Default for SendPoll {
    fn default() -> Self {
        Self {
            question: String::new(),
            options: Vec::new(),
            is_anonymous: true,
            poll_type: PollType::Regular,
            correct_option_id: None,
            explanation: None,
            open_period: None,
            allows_multiple_answers: false,
        }
    }
}

// ─── Dice ───

/// Dice emoji type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
pub enum DiceEmoji {
    #[default]
    Dice,
    Darts,
    Basketball,
    Football,
    SlotMachine,
    Bowling,
}

impl DiceEmoji {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Dice => "\u{1f3b2}",
            Self::Darts => "\u{1f3af}",
            Self::Basketball => "\u{1f3c0}",
            Self::Football => "\u{26bd}",
            Self::SlotMachine => "\u{1f3b0}",
            Self::Bowling => "\u{1f3b3}",
        }
    }
}


// ─── Contact ───

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contact {
    pub phone_number: String,
    pub first_name: String,
    pub last_name: Option<String>,
    pub user_id: Option<u64>,
    pub vcard: Option<String>,
}

// ─── Venue ───

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Venue {
    pub latitude: f64,
    pub longitude: f64,
    pub title: String,
    pub address: String,
    pub foursquare_id: Option<String>,
    pub foursquare_type: Option<String>,
}

// ─── Chat Member ───

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMember {
    pub user: UserInfo,
    pub status: ChatMemberStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChatMemberStatus {
    Creator,
    Administrator,
    Member,
    Restricted,
    Left,
    Banned,
}

impl ChatMemberStatus {
    pub fn is_admin(&self) -> bool {
        matches!(self, Self::Creator | Self::Administrator)
    }

    pub fn is_member(&self) -> bool {
        matches!(self, Self::Creator | Self::Administrator | Self::Member | Self::Restricted)
    }
}

// ─── Chat Info ───

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatInfo {
    pub id: ChatId,
    pub chat_type: ChatType,
    pub title: Option<String>,
    pub username: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub member_count: Option<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChatType {
    Private,
    Group,
    Supergroup,
    Channel,
}

// ─── Bot Command ───

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotCommand {
    pub command: String,
    pub description: String,
}

// ─── Permissions ───

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChatPermissions {
    pub can_send_messages: Option<bool>,
    pub can_send_media_messages: Option<bool>,
    pub can_send_polls: Option<bool>,
    pub can_send_other_messages: Option<bool>,
    pub can_add_web_page_previews: Option<bool>,
    pub can_change_info: Option<bool>,
    pub can_invite_users: Option<bool>,
    pub can_pin_messages: Option<bool>,
}

// ─── Invoice / Payment ───

#[derive(Debug, Clone)]
pub struct Invoice {
    pub title: String,
    pub description: String,
    pub payload: String,
    pub provider_token: Option<String>,
    pub currency: String,
    /// Price in smallest units (e.g. cents). Vec of (label, amount).
    pub prices: Vec<(String, i64)>,
    pub start_parameter: Option<String>,
    pub photo_url: Option<String>,
    pub need_name: bool,
    pub need_phone_number: bool,
    pub need_email: bool,
    pub need_shipping_address: bool,
    pub is_flexible: bool,
}

// ─── Media Group Item ───

#[derive(Debug, Clone)]
pub enum MediaGroupItem {
    Photo {
        source: FileSource,
        caption: Option<String>,
        parse_mode: ParseMode,
        spoiler: bool,
    },
    Video {
        source: FileSource,
        caption: Option<String>,
        parse_mode: ParseMode,
        spoiler: bool,
    },
    Document {
        source: FileSource,
        caption: Option<String>,
        parse_mode: ParseMode,
    },
    Audio {
        source: FileSource,
        caption: Option<String>,
        parse_mode: ParseMode,
    },
}

// ─── Downloaded File ───

#[derive(Debug, Clone)]
pub struct DownloadedFile {
    pub data: Vec<u8>,
    pub file_size: Option<usize>,
}

// ─── Bot Info ───

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotInfo {
    pub id: UserId,
    pub username: String,
    pub first_name: String,
    pub can_join_groups: bool,
    pub can_read_all_group_messages: bool,
    pub supports_inline_queries: bool,
}

// ─── User Profile Photos ───

#[derive(Debug, Clone)]
pub struct UserProfilePhotos {
    pub total_count: i32,
    /// File IDs of the photos (largest size per photo).
    pub photos: Vec<String>,
}

// ─── Chat Invite Link ───

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatInviteLink {
    pub invite_link: String,
    pub creator: Option<UserInfo>,
    pub creates_join_request: bool,
    pub is_primary: bool,
    pub is_revoked: bool,
    pub name: Option<String>,
    pub expire_date: Option<i64>,
    pub member_limit: Option<i32>,
    pub pending_join_request_count: Option<i32>,
}

// ─── Menu Button ───

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MenuButton {
    /// Default behavior.
    Default,
    /// Open bot's list of commands.
    Commands,
    /// Open a web app.
    WebApp { text: String, url: String },
}

// ─── Bot Description ───

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotDescription {
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotShortDescription {
    pub short_description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotName {
    pub name: String,
}

// ─── Shipping Query ───

#[derive(Debug, Clone)]
pub struct ShippingOption {
    pub id: String,
    pub title: String,
    /// Prices in smallest units: Vec<(label, amount)>.
    pub prices: Vec<(String, i64)>,
}

// ─── Forum Topics ───

/// Represents a forum topic in a supergroup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForumTopic {
    /// Topic ID (same as the message_id of the topic's creation service message).
    pub id: i32,
    pub title: String,
    pub icon_color: Option<i32>,
    pub icon_custom_emoji_id: Option<String>,
    pub is_closed: bool,
    pub is_hidden: bool,
}

// ─── Stars / Payments (extended) ───

/// A Telegram Stars transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StarTransaction {
    pub id: String,
    /// Amount in Stars (can be negative for outbound).
    pub amount: i64,
    /// Fractional nanos (for sub-star precision).
    pub nanos: i32,
    pub date: i32,
    pub source: StarTransactionPeer,
    pub title: Option<String>,
    pub description: Option<String>,
    pub is_refund: bool,
}

/// Who is on the other side of a star transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StarTransactionPeer {
    User(UserId),
    AppStore,
    PlayMarket,
    Fragment,
    PremiumBot,
    Ads,
    Api,
    Unknown,
}

/// Star balance info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StarBalance {
    pub amount: i64,
    pub nanos: i32,
}

/// Response from get_star_transactions.
#[derive(Debug, Clone)]
pub struct StarTransactions {
    pub balance: StarBalance,
    pub transactions: Vec<StarTransaction>,
    pub next_offset: Option<String>,
}
