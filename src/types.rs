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
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 ^= b as u64;
            self.0 = self.0.wrapping_mul(0x100000001b3);
        }
    }
}
use std::path::PathBuf;

// ─── IDs ───

/// Telegram message identifier, unique within a single chat.
///
/// Wraps the raw `i32` that Telegram uses. Two messages in different chats
/// can share the same numeric ID — always pair with [`ChatId`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MessageId(
    /// Raw Telegram message ID.
    pub i32,
);

/// Telegram chat identifier.
///
/// Positive values represent users (private chats), negative values
/// represent groups and supergroups. Channel IDs start at `-100…`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChatId(
    /// Raw Telegram chat ID.
    pub i64,
);

/// Telegram user identifier (always positive).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UserId(
    /// Raw Telegram user ID.
    pub u64,
);

// ─── Screen ───

/// Unique identifier for a [`Screen`](crate::screen::Screen).
///
/// Used by the differ to match old and new screens and by the navigation
/// stack to remember which screen the user was on.
///
/// Accepts both `&'static str` and owned `String` via [`Cow`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ScreenId(
    /// The screen’s string identifier.
    pub Cow<'static, str>,
);

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

/// The kind of content a Telegram message carries.
///
/// The differ uses this to decide whether a transition can be done with
/// `editMessageText` / `editMessageMedia` or requires delete + send.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ContentType {
    /// Plain or HTML/Markdown text message.
    Text,
    /// Photo (compressed image).
    Photo,
    /// Video file.
    Video,
    /// GIF / MPEG-4 animation.
    Animation,
    /// Generic file attachment.
    Document,
    /// Sticker (WebP / TGS / WebM).
    Sticker,
    /// Voice message (OGG Opus).
    Voice,
    /// Round video note.
    VideoNote,
    /// Audio file with ID3 metadata.
    Audio,
    /// GPS location point.
    Location,
    /// Venue with address and optional Foursquare ID.
    Venue,
    /// Shared contact card.
    Contact,
    /// Native Telegram poll.
    Poll,
    /// Animated dice / darts / basketball emoji.
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

/// Text formatting mode for message bodies and captions.
///
/// Defaults to [`Html`](Self::Html) which supports `<b>`, `<i>`, `<code>`,
/// `<a href="...">`, etc.  See the
/// [Telegram formatting docs](https://core.telegram.org/bots/api#formatting-options).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum ParseMode {
    /// HTML tags (`<b>`, `<i>`, `<code>`, `<a>`, …).
    #[default]
    Html,
    /// MarkdownV2 syntax (`*bold*`, `_italic_`, `` `code` ``, …).
    MarkdownV2,
    /// No parsing — text is sent as-is.
    None,
}

/// Controls whether URL previews (link thumbnails) are shown in text messages.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum LinkPreview {
    /// Show URL preview / link thumbnail.
    Enabled,
    /// Suppress URL preview (default).
    #[default]
    Disabled,
}

/// Where a file comes from when sending media.
///
/// Telegram accepts four sources.  A bare string is auto-detected via
/// [`From<&str>`]: URLs become [`Url`](Self::Url), paths with `/` or `\`
/// become [`LocalPath`](Self::LocalPath), everything else is treated as a
/// [`FileId`](Self::FileId).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileSource {
    /// Reuse an already-uploaded file by its Telegram `file_id`.
    FileId(String),
    /// Download from an HTTP(S) URL (Telegram fetches it server-side).
    Url(String),
    /// Upload from a local filesystem path.
    LocalPath(PathBuf),
    /// Upload raw bytes with a filename.
    Bytes {
        /// File content.
        data: Vec<u8>,
        /// Filename shown to the user.
        filename: String,
    },
}

impl PartialEq for FileSource {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::FileId(a), Self::FileId(b)) => a == b,
            (Self::Url(a), Self::Url(b)) => a == b,
            (Self::LocalPath(a), Self::LocalPath(b)) => a == b,
            (
                Self::Bytes {
                    data: d1,
                    filename: f1,
                },
                Self::Bytes {
                    data: d2,
                    filename: f2,
                },
            ) => d1 == d2 && f1 == f2,
            _ => false,
        }
    }
}

impl Eq for FileSource {}

impl Hash for FileSource {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Self::FileId(id) => {
                0u8.hash(state);
                id.hash(state);
            }
            Self::Url(url) => {
                1u8.hash(state);
                url.hash(state);
            }
            Self::LocalPath(p) => {
                2u8.hash(state);
                p.hash(state);
            }
            Self::Bytes { data, filename } => {
                3u8.hash(state);
                data.hash(state);
                filename.hash(state);
            }
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

/// The full content of a single bot message, ready to be sent or diffed.
///
/// Each variant maps to a specific Telegram `send*` / `edit*` API call.
/// The differ compares [`content_hash`](Self::content_hash) values of old
/// and new content to decide which API calls are actually needed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageContent {
    /// Plain or formatted text message (no media).
    Text {
        /// Message body (may contain HTML / MarkdownV2 tags).
        text: String,
        /// How Telegram should parse formatting tags in `text`.
        parse_mode: ParseMode,
        /// Optional inline keyboard attached below the message.
        keyboard: Option<crate::keyboard::InlineKeyboard>,
        /// Whether to show a URL preview card.
        link_preview: LinkPreview,
    },
    /// Compressed image with optional caption.
    Photo {
        /// Image source (file ID, URL, local path, or raw bytes).
        source: FileSource,
        /// Optional caption shown below the photo.
        caption: Option<String>,
        /// Formatting mode for the caption.
        parse_mode: ParseMode,
        /// Optional inline keyboard.
        keyboard: Option<crate::keyboard::InlineKeyboard>,
        /// Send the photo under a click-to-reveal spoiler.
        spoiler: bool,
    },
    /// Video file with optional caption.
    Video {
        /// Video source.
        source: FileSource,
        /// Optional caption.
        caption: Option<String>,
        /// Formatting mode for the caption.
        parse_mode: ParseMode,
        /// Optional inline keyboard.
        keyboard: Option<crate::keyboard::InlineKeyboard>,
        /// Send under a spoiler overlay.
        spoiler: bool,
    },
    /// GIF / MPEG-4 animation with optional caption.
    Animation {
        /// Animation source.
        source: FileSource,
        /// Optional caption.
        caption: Option<String>,
        /// Formatting mode for the caption.
        parse_mode: ParseMode,
        /// Optional inline keyboard.
        keyboard: Option<crate::keyboard::InlineKeyboard>,
        /// Send under a spoiler overlay.
        spoiler: bool,
    },
    /// Generic document / file attachment.
    Document {
        /// Document source.
        source: FileSource,
        /// Optional caption.
        caption: Option<String>,
        /// Formatting mode for the caption.
        parse_mode: ParseMode,
        /// Optional inline keyboard.
        keyboard: Option<crate::keyboard::InlineKeyboard>,
        /// Override the filename shown in the Telegram client.
        filename: Option<String>,
    },
    /// Sticker message (WebP / TGS / WebM).
    Sticker {
        /// Sticker source.
        source: FileSource,
    },
    /// GPS location pin.
    Location {
        /// Latitude in degrees.
        latitude: f64,
        /// Longitude in degrees.
        longitude: f64,
        /// Optional inline keyboard.
        keyboard: Option<crate::keyboard::InlineKeyboard>,
    },
}

impl MessageContent {
    /// Returns the [`ContentType`] discriminant for this content.
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

    /// Deterministic hash of the entire content (type + text + media + keyboard).
    ///
    /// If two messages have the same `content_hash`, the differ skips the
    /// transition entirely — zero API calls.
    pub fn content_hash(&self) -> u64 {
        let mut hasher = new_fixed_hasher();
        self.content_type().hash(&mut hasher);
        match self {
            Self::Text {
                text,
                parse_mode,
                keyboard,
                link_preview,
            } => {
                text.hash(&mut hasher);
                parse_mode.hash(&mut hasher);
                if let Some(kb) = keyboard {
                    kb.hash(&mut hasher);
                }
                link_preview.hash(&mut hasher);
            }
            Self::Photo {
                source,
                caption,
                keyboard,
                spoiler,
                ..
            } => {
                source.hash(&mut hasher);
                caption.hash(&mut hasher);
                if let Some(kb) = keyboard {
                    kb.hash(&mut hasher);
                }
                spoiler.hash(&mut hasher);
            }
            Self::Video {
                source,
                caption,
                keyboard,
                spoiler,
                ..
            } => {
                source.hash(&mut hasher);
                caption.hash(&mut hasher);
                if let Some(kb) = keyboard {
                    kb.hash(&mut hasher);
                }
                spoiler.hash(&mut hasher);
            }
            Self::Animation {
                source,
                caption,
                keyboard,
                spoiler,
                ..
            } => {
                source.hash(&mut hasher);
                caption.hash(&mut hasher);
                if let Some(kb) = keyboard {
                    kb.hash(&mut hasher);
                }
                spoiler.hash(&mut hasher);
            }
            Self::Document {
                source,
                caption,
                keyboard,
                filename,
                ..
            } => {
                source.hash(&mut hasher);
                caption.hash(&mut hasher);
                if let Some(kb) = keyboard {
                    kb.hash(&mut hasher);
                }
                filename.hash(&mut hasher);
            }
            Self::Sticker { source } => {
                source.hash(&mut hasher);
            }
            Self::Location {
                latitude,
                longitude,
                keyboard,
            } => {
                latitude.to_bits().hash(&mut hasher);
                longitude.to_bits().hash(&mut hasher);
                if let Some(kb) = keyboard {
                    kb.hash(&mut hasher);
                }
            }
        }
        hasher.finish()
    }

    /// Hash of the text body only (for [`Text`](Self::Text) variants).
    ///
    /// Non-text variants all hash to the same constant, so comparing
    /// `text_hash` alone is only meaningful for text messages.
    pub fn text_hash(&self) -> u64 {
        let mut hasher = new_fixed_hasher();
        match self {
            Self::Text {
                text, parse_mode, ..
            } => {
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

    /// Returns the caption for media variants, or `None` for text/sticker/location.
    pub fn caption(&self) -> Option<String> {
        match self {
            Self::Photo { caption, .. }
            | Self::Video { caption, .. }
            | Self::Animation { caption, .. }
            | Self::Document { caption, .. } => caption.clone(),
            _ => None,
        }
    }

    /// Returns the inline keyboard, if any.
    pub fn keyboard(&self) -> Option<crate::keyboard::InlineKeyboard> {
        match self {
            Self::Text { keyboard, .. }
            | Self::Photo { keyboard, .. }
            | Self::Video { keyboard, .. }
            | Self::Animation { keyboard, .. }
            | Self::Document { keyboard, .. }
            | Self::Location { keyboard, .. } => keyboard.clone(),
            _ => None,
        }
    }

    /// Deterministic hash of the inline keyboard alone.
    pub fn keyboard_hash(&self) -> u64 {
        let mut hasher = new_fixed_hasher();
        match self.keyboard() {
            Some(kb) => {
                1u8.hash(&mut hasher);
                kb.hash(&mut hasher);
            }
            None => {
                0u8.hash(&mut hasher);
            }
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
            Self::Text {
                text,
                keyboard,
                link_preview,
                ..
            } => Self::Text {
                text: strip(&text),
                parse_mode: ParseMode::None,
                keyboard,
                link_preview,
            },
            Self::Photo {
                source,
                caption,
                keyboard,
                spoiler,
                ..
            } => Self::Photo {
                source,
                caption: caption.map(|c| strip(&c)),
                parse_mode: ParseMode::None,
                keyboard,
                spoiler,
            },
            Self::Video {
                source,
                caption,
                keyboard,
                spoiler,
                ..
            } => Self::Video {
                source,
                caption: caption.map(|c| strip(&c)),
                parse_mode: ParseMode::None,
                keyboard,
                spoiler,
            },
            Self::Animation {
                source,
                caption,
                keyboard,
                spoiler,
                ..
            } => Self::Animation {
                source,
                caption: caption.map(|c| strip(&c)),
                parse_mode: ParseMode::None,
                keyboard,
                spoiler,
            },
            Self::Document {
                source,
                caption,
                keyboard,
                filename,
                ..
            } => Self::Document {
                source,
                caption: caption.map(|c| strip(&c)),
                parse_mode: ParseMode::None,
                keyboard,
                filename,
            },
            other => other, // Sticker, Location — no text
        }
    }

    /// Deterministic hash of the caption string alone.
    pub fn caption_hash(&self) -> u64 {
        let mut hasher = new_fixed_hasher();
        match self.caption() {
            Some(cap) => {
                1u8.hash(&mut hasher);
                cap.hash(&mut hasher);
            }
            None => {
                0u8.hash(&mut hasher);
            }
        }
        hasher.finish()
    }

    /// Deterministic hash of the file source alone.
    pub fn file_hash(&self) -> u64 {
        let mut hasher = new_fixed_hasher();
        match self {
            Self::Photo { source, .. }
            | Self::Video { source, .. }
            | Self::Animation { source, .. }
            | Self::Document { source, .. }
            | Self::Sticker { source } => {
                1u8.hash(&mut hasher);
                source.hash(&mut hasher);
            }
            _ => {
                0u8.hash(&mut hasher);
            }
        }
        hasher.finish()
    }
}

// ─── Chat Action ───

/// A "typing indicator" action shown in the chat header.
///
/// Send via [`BotApi::send_chat_action`](crate::bot_api::BotApi::send_chat_action)
/// to let the user know the bot is working on something.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatAction {
    /// "typing…" indicator.
    Typing,
    /// "sending photo…" indicator.
    UploadPhoto,
    /// "sending video…" indicator.
    UploadVideo,
    /// "sending file…" indicator.
    UploadDocument,
    /// "choosing location…" indicator.
    FindLocation,
    /// "recording voice…" indicator.
    RecordVoice,
    /// "recording video…" indicator.
    RecordVideo,
}

// ─── Input Spec ───

/// What input the current screen expects from the user.
#[derive(Clone)]
pub enum InputSpec {
    /// Free-form text input, optionally validated.
    Text {
        /// Validation function; return `Err(message)` to reject input.
        validator: Option<ValidatorFn>,
        /// Placeholder text shown in the input field on mobile clients.
        placeholder: Option<String>,
    },
    /// Expect a photo from the user.
    Photo,
    /// Expect a video from the user.
    Video,
    /// Expect a document (any file) from the user.
    Document,
    /// Expect a shared GPS location.
    Location,
    /// Expect a shared contact card.
    Contact,
    /// Present a fixed list of choices (rendered as a reply keyboard).
    Choice {
        /// The allowed option strings.
        options: Vec<String>,
    },
}

impl std::fmt::Debug for InputSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Text { placeholder, .. } => f
                .debug_struct("Text")
                .field("placeholder", placeholder)
                .finish(),
            Self::Photo => write!(f, "Photo"),
            Self::Video => write!(f, "Video"),
            Self::Document => write!(f, "Document"),
            Self::Location => write!(f, "Location"),
            Self::Contact => write!(f, "Contact"),
            Self::Choice { options } => f.debug_struct("Choice").field("options", options).finish(),
        }
    }
}

/// A thread-safe text validation function.
///
/// Return `Ok(())` to accept the input, or `Err(message)` to show the user
/// a transient error toast and ask them to retry.
pub type ValidatorFn = std::sync::Arc<dyn Fn(&str) -> Result<(), String> + Send + Sync>;

// ─── User Info ───

/// Information about a Telegram user extracted from an incoming update.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    /// Telegram user ID.
    pub id: UserId,
    /// User’s first name.
    pub first_name: String,
    /// User’s last name (not everyone has one).
    pub last_name: Option<String>,
    /// `@username` without the leading `@` (optional).
    pub username: Option<String>,
    /// IETF language tag reported by the Telegram client (e.g. `"en"`, `"ru"`).
    pub language_code: Option<String>,
}

impl UserInfo {
    /// Returns `"First Last"` or just `"First"` if no last name is set.
    pub fn full_name(&self) -> String {
        match &self.last_name {
            Some(last) => format!("{} {}", self.first_name, last),
            None => self.first_name.clone(),
        }
    }
}

// ─── Tracked Message ───

/// A message the bot previously sent, tracked for diffing.
///
/// Stores pre-computed hashes so the differ can decide which parts changed
/// without holding the full [`MessageContent`] in memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackedMessage {
    /// ID of the sent message.
    pub message_id: MessageId,
    /// What kind of content this message carries.
    pub content_type: ContentType,
    /// Hash of the full content (type + text + media + keyboard).
    pub content_hash: u64,
    /// Hash of the text body only.
    pub text_hash: u64,
    /// Hash of the caption only.
    pub caption_hash: u64,
    /// Hash of the file source only.
    pub file_hash: u64,
    /// Hash of the inline keyboard only.
    pub keyboard_hash: u64,
}

impl TrackedMessage {
    /// Build a [`TrackedMessage`] by computing all hashes from `content`.
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

/// Result of a successful send operation.
#[derive(Debug, Clone)]
pub struct SentMessage {
    /// ID of the newly sent message.
    pub message_id: MessageId,
    /// Chat the message was sent to.
    pub chat_id: ChatId,
}

// ─── Chat State ───

/// Per-chat persistent state managed by blazegram.
///
/// Tracks which messages the bot currently has on screen, what the user
/// has sent since the last navigation, the navigation stack, and any
/// user-defined key–value data.
///
/// Serialized to the configured [`StateStore`](crate::state::StateStore)
/// after every update.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatState {
    /// The chat this state belongs to.
    pub chat_id: ChatId,
    /// [`ScreenId`] of the screen currently displayed.
    pub current_screen: ScreenId,
    /// Messages the bot sent that are still visible and tracked by the differ.
    pub active_bot_messages: Vec<TrackedMessage>,
    /// User messages received since the last `navigate()` (will be deleted on next transition).
    pub pending_user_messages: Vec<MessageId>,
    /// Callback query ID that must be answered before this update finishes.
    #[serde(skip)]
    pub pending_callback_id: Option<String>,
    /// Arbitrary user-defined JSON data (see [`Ctx::get`](crate::ctx::Ctx::get) / [`Ctx::set`](crate::ctx::Ctx::set)).
    pub data: HashMap<String, serde_json::Value>,
    /// Navigation stack for [`Ctx::push`](crate::ctx::Ctx::push) / [`Ctx::pop`](crate::ctx::Ctx::pop) (max depth 20).
    pub screen_stack: Vec<ScreenId>,
    /// The user who owns this chat state.
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
    /// Create a fresh state for a chat that the bot has never seen before.
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

/// A parsed Telegram update with common fields extracted.
///
/// `chat_id`, `user`, and `message_id` are lifted to top-level fields
/// to avoid duplication across every variant. Variant-specific data
/// lives in [`UpdateKind`].
#[derive(Debug, Clone)]
pub struct IncomingUpdate {
    /// Chat this update belongs to.
    /// For inline queries, synthesized from `user.id`.
    pub chat_id: ChatId,
    /// User who triggered the update.
    pub user: UserInfo,
    /// Message ID, if applicable to this update type.
    pub message_id: Option<MessageId>,
    /// Variant-specific payload.
    pub kind: UpdateKind,
}

/// Variant-specific data for an incoming update.
#[derive(Debug, Clone)]
pub enum UpdateKind {
    /// A regular text message (or a message with no recognized media).
    Message {
        /// Message text, if any. `None` for media-only messages.
        text: Option<String>,
    },
    /// User pressed an inline keyboard button.
    CallbackQuery {
        /// Unique callback query ID (must be answered within 10 s).
        id: String,
        /// Callback data string attached to the button.
        data: Option<String>,
        /// For callbacks on inline messages — the packed inline message ID.
        inline_message_id: Option<String>,
    },
    /// User sent a photo.
    Photo {
        /// Telegram file ID of the largest photo size.
        file_id: String,
        /// Unique file identifier (stable across re-uploads).
        file_unique_id: String,
        /// Photo caption, if any.
        caption: Option<String>,
    },
    /// User sent a document (generic file).
    Document {
        /// Telegram file ID.
        file_id: String,
        /// Unique file identifier.
        file_unique_id: String,
        /// Original filename reported by the sender’s client.
        filename: Option<String>,
        /// Document caption, if any.
        caption: Option<String>,
    },
    /// An incoming [inline query](https://core.telegram.org/bots/inline).
    InlineQuery {
        /// Unique query ID (must be answered within 30 s).
        id: String,
        /// Text of the query typed by the user.
        query: String,
        /// Offset for pagination (empty on the first page).
        offset: String,
    },
    /// A result from an inline query was chosen by the user.
    ChosenInlineResult {
        /// The `id` of the chosen [`InlineResult`](crate::inline::InlineResult).
        result_id: String,
        /// Inline message ID, present if the result was sent with an inline keyboard.
        inline_message_id: Option<String>,
        /// The original query that produced this result.
        query: String,
    },
    /// Pre-checkout validation step for Telegram Payments.
    PreCheckoutQuery {
        /// Unique query ID.
        id: String,
        /// Three-letter ISO 4217 currency code (or `"XTR"` for Stars).
        currency: String,
        /// Total amount in the smallest currency unit (e.g. cents).
        total_amount: i64,
        /// Bot-defined invoice payload.
        payload: String,
    },
    /// Payment completed successfully.
    SuccessfulPayment {
        /// Currency code.
        currency: String,
        /// Total amount charged.
        total_amount: i64,
        /// Bot-defined invoice payload.
        payload: String,
    },
    /// Data sent from a [Web App](https://core.telegram.org/bots/webapps).
    WebAppData {
        /// The data string sent by the Web App.
        data: String,
    },
    /// A message was edited by the user.
    MessageEdited {
        /// New text after the edit, if the message has text.
        text: Option<String>,
    },
    /// Voice message received.
    Voice {
        /// Telegram file ID.
        file_id: String,
        /// Unique file identifier.
        file_unique_id: String,
        /// Duration in seconds.
        duration: i32,
        /// Voice caption, if any.
        caption: Option<String>,
    },
    /// Video note (round video) received.
    VideoNote {
        /// Telegram file ID.
        file_id: String,
        /// Unique file identifier.
        file_unique_id: String,
        /// Duration in seconds.
        duration: i32,
    },
    /// Video received.
    Video {
        /// Telegram file ID.
        file_id: String,
        /// Unique file identifier.
        file_unique_id: String,
        /// Video caption, if any.
        caption: Option<String>,
    },
    /// Sticker received.
    Sticker {
        /// Telegram file ID.
        file_id: String,
        /// Unique file identifier.
        file_unique_id: String,
    },
    /// Contact shared by the user.
    ContactReceived {
        /// The shared contact.
        contact: Contact,
    },
    /// Location shared by the user.
    LocationReceived {
        /// Latitude in degrees.
        latitude: f64,
        /// Longitude in degrees.
        longitude: f64,
    },
    /// A new member joined the chat (including the bot itself).
    ChatMemberJoined,
    /// A member left the chat (including the bot itself).
    ChatMemberLeft,
}

impl IncomingUpdate {
    /// Convenience: direct access to chat_id.
    pub fn chat_id(&self) -> ChatId {
        self.chat_id
    }

    /// Convenience: direct access to user.
    pub fn user(&self) -> &UserInfo {
        &self.user
    }

    /// Short human-readable name of the update kind (e.g. `"message"`,
    /// `"callback_query"`). Delegates to [`UpdateKind::type_name`].
    pub fn type_name(&self) -> &'static str {
        self.kind.type_name()
    }

    /// Extract deep link payload from /start command.
    /// Handles both `/start ref_123` and `/start@botname ref_123`.
    pub fn deep_link(&self) -> Option<&str> {
        match &self.kind {
            UpdateKind::Message { text: Some(text) } => {
                let text = text.trim();
                let rest = text.strip_prefix("/start")?;
                let rest = if let Some(after_at) = rest.strip_prefix('@') {
                    after_at.find(' ').map(|i| &after_at[i..]).unwrap_or("")
                } else {
                    rest
                };
                let payload = rest.trim();
                if payload.is_empty() {
                    None
                } else {
                    Some(payload)
                }
            }
            _ => None,
        }
    }
}

impl UpdateKind {
    /// Short human-readable name for logging and metrics (e.g. `"message"`).
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
            Self::ChatMemberJoined => "chat_member_joined",
            Self::ChatMemberLeft => "chat_member_left",
        }
    }
}

// ─── Received Media (for input handlers) ───

/// A media file received from the user, normalized across photo / video /
/// document / voice / etc. update kinds.
///
/// Passed to input handlers registered with
/// [`App::on_input`](crate::app::App) or [`Form`](crate::form::Form) photo steps.
#[derive(Debug, Clone)]
pub struct ReceivedMedia {
    /// Telegram file ID (use with [`BotApi::download_file`](crate::bot_api::BotApi::download_file)).
    pub file_id: String,
    /// Stable unique file identifier.
    pub file_unique_id: String,
    /// What kind of media this is.
    pub file_type: ContentType,
    /// Caption attached to the media, if any.
    pub caption: Option<String>,
    /// Original filename (documents only).
    pub filename: Option<String>,
}

/// Convenience alias for a JSON object map (`HashMap<String, serde_json::Value>`).
pub type JsonMap = HashMap<String, serde_json::Value>;

fn default_true() -> bool {
    true
}

// ─── Inline Query Result ───

/// A single result for answering an inline query.
#[derive(Debug, Clone)]
pub struct InlineQueryResult {
    /// Unique result identifier (1–64 bytes).
    pub id: String,
    /// The kind of inline result (article, photo, GIF).
    pub kind: InlineResultKind,
    /// Title shown in the result list.
    pub title: Option<String>,
    /// Short description shown below the title.
    pub description: Option<String>,
    /// Thumbnail URL for the result list.
    pub thumb_url: Option<String>,
    /// Message content (text + keyboard).
    pub message_text: Option<String>,
    /// Formatting mode for `message_text`.
    pub parse_mode: ParseMode,
    /// Inline keyboard attached to the sent message.
    pub keyboard: Option<crate::keyboard::InlineKeyboard>,
}

/// Discriminant for the type of an [`InlineQueryResult`].
#[derive(Debug, Clone)]
pub enum InlineResultKind {
    /// A text-only result (no media).
    Article,
    /// A photo result with a URL and optional dimensions.
    Photo {
        /// Direct URL to the full-size photo.
        photo_url: String,
        /// Photo width in pixels (helps Telegram layout).
        width: Option<i32>,
        /// Photo height in pixels.
        height: Option<i32>,
    },
    /// An animated GIF result.
    Gif {
        /// Direct URL to the GIF file.
        gif_url: String,
    },
}

// ─── Ctx Mode ───

/// How the Ctx operates — determined automatically from the update source.
#[derive(Debug, Clone, Default)]
pub enum CtxMode {
    /// Private chat — full differ (delete/edit/send).
    #[default]
    Private,
    /// Group/supergroup — edit in-place, no deletion of other messages.
    Group {
        /// Message ID that triggered this handler (for reply targeting).
        trigger_message_id: Option<MessageId>,
    },
    /// Inline message — edit via `inline_message_id`.
    Inline {
        /// The packed inline message ID from the callback query.
        inline_message_id: String,
    },
}

// ─── Poll ───

/// A single option in a Telegram poll.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollOption {
    /// The option’s label text.
    pub text: String,
}

/// Configuration for sending a native Telegram poll.
///
/// Build one and pass to [`BotApi::send_poll`](crate::bot_api::BotApi::send_poll).
#[derive(Debug, Clone)]
pub struct SendPoll {
    /// The poll question (1–300 characters).
    pub question: String,
    /// Answer options (2–10 strings, each 1–100 characters).
    pub options: Vec<String>,
    /// Whether voters are anonymous (default `true`).
    pub is_anonymous: bool,
    /// Regular poll or quiz (single correct answer).
    pub poll_type: PollType,
    /// For quiz polls: zero-based index of the correct option.
    pub correct_option_id: Option<usize>,
    /// Explanation shown after the user answers a quiz poll (0–200 chars).
    pub explanation: Option<String>,
    /// Time in seconds the poll will be active (5–600), or `None` for unlimited.
    pub open_period: Option<i32>,
    /// Whether the poll allows selecting multiple answers.
    pub allows_multiple_answers: bool,
}

/// Whether a poll is a regular vote or a quiz with one correct answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PollType {
    /// Regular poll — users pick one or more options.
    Regular,
    /// Quiz — exactly one correct answer, shown after voting.
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DiceEmoji {
    /// 🎲 standard die (values 1–6).
    #[default]
    Dice,
    /// 🎯 darts (values 1–6).
    Darts,
    /// 🏀 basketball (values 1–5).
    Basketball,
    /// ⚽ football (values 1–5).
    Football,
    /// 🎰 slot machine (values 1–64).
    SlotMachine,
    /// 🎳 bowling (values 1–6).
    Bowling,
}

impl DiceEmoji {
    /// Returns the emoji character that Telegram expects in the API call.
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

/// A shared contact card.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contact {
    /// Phone number in international format.
    pub phone_number: String,
    /// Contact’s first name.
    pub first_name: String,
    /// Contact’s last name.
    pub last_name: Option<String>,
    /// Telegram user ID of the contact, if known.
    pub user_id: Option<u64>,
    /// vCard string with additional info.
    pub vcard: Option<String>,
}

// ─── Venue ───

/// A venue (place) with coordinates and address.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Venue {
    /// Latitude in degrees.
    pub latitude: f64,
    /// Longitude in degrees.
    pub longitude: f64,
    /// Name of the venue.
    pub title: String,
    /// Street address.
    pub address: String,
    /// Foursquare venue identifier.
    pub foursquare_id: Option<String>,
    /// Foursquare venue type (e.g. `"arts_entertainment/default"`).
    pub foursquare_type: Option<String>,
}

// ─── Chat Member ───

/// A user’s membership record in a chat.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMember {
    /// The user.
    pub user: UserInfo,
    /// Their current status in the chat.
    pub status: ChatMemberStatus,
}

/// Possible membership statuses in a Telegram chat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChatMemberStatus {
    /// Chat owner (has all permissions, cannot be restricted).
    Creator,
    /// Promoted administrator.
    Administrator,
    /// Regular member.
    Member,
    /// Restricted member (some permissions revoked).
    Restricted,
    /// Not in the chat (left voluntarily).
    Left,
    /// Banned from the chat.
    Banned,
}

impl ChatMemberStatus {
    /// Returns `true` for [`Creator`](Self::Creator) and [`Administrator`](Self::Administrator).
    pub fn is_admin(&self) -> bool {
        matches!(self, Self::Creator | Self::Administrator)
    }

    /// Returns `true` if the user is currently in the chat
    /// (creator, admin, member, or restricted).
    pub fn is_member(&self) -> bool {
        matches!(
            self,
            Self::Creator | Self::Administrator | Self::Member | Self::Restricted
        )
    }
}

// ─── Chat Info ───

/// Full information about a Telegram chat.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatInfo {
    /// Chat identifier.
    pub id: ChatId,
    /// Type of chat (private, group, supergroup, channel).
    pub chat_type: ChatType,
    /// Chat title (groups, supergroups, channels).
    pub title: Option<String>,
    /// `@username` of the chat (if set).
    pub username: Option<String>,
    /// First name (private chats only).
    pub first_name: Option<String>,
    /// Last name (private chats only).
    pub last_name: Option<String>,
    /// Number of members (may require admin privileges to read).
    pub member_count: Option<i32>,
}

/// The type of a Telegram chat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChatType {
    /// One-on-one conversation with the bot.
    Private,
    /// Classic group (up to 200 members, no supergroup features).
    Group,
    /// Supergroup (up to 200 000 members, persistent history, admin tools).
    Supergroup,
    /// Broadcast channel.
    Channel,
}

// ─── Bot Command ───

/// A bot command entry for the menu / command list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotCommand {
    /// The command string without the leading `/` (e.g. `"start"`).
    pub command: String,
    /// Human-readable description (1–256 characters).
    pub description: String,
}

// ─── Permissions ───

/// Fine-grained chat permissions that can be set per-user or for the whole group.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChatPermissions {
    /// Can send text messages, contacts, invoices, locations, venues.
    pub can_send_messages: Option<bool>,
    /// Can send audios, documents, photos, videos, video notes, voice notes.
    pub can_send_media_messages: Option<bool>,
    /// Can send polls.
    pub can_send_polls: Option<bool>,
    /// Can send stickers, GIFs, games, use inline bots.
    pub can_send_other_messages: Option<bool>,
    /// Can add web page previews to messages.
    pub can_add_web_page_previews: Option<bool>,
    /// Can change the chat title, photo, and other settings.
    pub can_change_info: Option<bool>,
    /// Can invite new users to the chat.
    pub can_invite_users: Option<bool>,
    /// Can pin messages (supergroups only).
    pub can_pin_messages: Option<bool>,
}

// ─── Invoice / Payment ───

/// A Telegram Payments invoice ready to be sent.
///
/// Use `provider_token: None` + `currency: "XTR"` for Telegram Stars payments.
#[derive(Debug, Clone)]
pub struct Invoice {
    /// Product name (1–32 characters).
    pub title: String,
    /// Product description (1–255 characters).
    pub description: String,
    /// Bot-defined payload (not shown to the user, returned in callbacks).
    pub payload: String,
    /// Payment provider token from @BotFather. `None` for Stars.
    pub provider_token: Option<String>,
    /// Three-letter ISO 4217 currency code (`"USD"`, `"EUR"`, `"XTR"` for Stars).
    pub currency: String,
    /// Price breakdown: `Vec<(label, amount)>` where amount is in the smallest
    /// currency unit (e.g. cents for USD, Stars for XTR).
    pub prices: Vec<(String, i64)>,
    /// Deep link parameter for the `/start` command when paying via link.
    pub start_parameter: Option<String>,
    /// URL of the product photo for the invoice.
    pub photo_url: Option<String>,
    /// Request the user’s full name.
    pub need_name: bool,
    /// Request the user’s phone number.
    pub need_phone_number: bool,
    /// Request the user’s email address.
    pub need_email: bool,
    /// Request the user’s shipping address.
    pub need_shipping_address: bool,
    /// Whether the final price depends on the shipping method.
    pub is_flexible: bool,
}

// ─── Media Group Item ───

/// A single item in a media group (album).
///
/// Send a `Vec<MediaGroupItem>` via
/// [`BotApi::send_media_group`](crate::bot_api::BotApi::send_media_group).
#[derive(Debug, Clone)]
pub enum MediaGroupItem {
    /// A photo in the album.
    Photo {
        /// Image source.
        source: FileSource,
        /// Optional caption (only the first item’s caption is shown by Telegram).
        caption: Option<String>,
        /// Formatting mode for the caption.
        parse_mode: ParseMode,
        /// Send under a spoiler overlay.
        spoiler: bool,
    },
    /// A video in the album.
    Video {
        /// Video source.
        source: FileSource,
        /// Optional caption.
        caption: Option<String>,
        /// Formatting mode for the caption.
        parse_mode: ParseMode,
        /// Send under a spoiler overlay.
        spoiler: bool,
    },
    /// A document in the album.
    Document {
        /// Document source.
        source: FileSource,
        /// Optional caption.
        caption: Option<String>,
        /// Formatting mode for the caption.
        parse_mode: ParseMode,
    },
    /// An audio track in the album.
    Audio {
        /// Audio source.
        source: FileSource,
        /// Optional caption.
        caption: Option<String>,
        /// Formatting mode for the caption.
        parse_mode: ParseMode,
    },
}

// ─── Downloaded File ───

/// Raw bytes of a file downloaded from Telegram servers.
#[derive(Debug, Clone)]
pub struct DownloadedFile {
    /// The file’s raw bytes.
    pub data: Vec<u8>,
    /// File size in bytes, if reported by Telegram.
    pub file_size: Option<usize>,
}

// ─── Bot Info ───

/// Information about the bot itself, returned by `get_me`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotInfo {
    /// Bot’s user ID.
    pub id: UserId,
    /// Bot’s `@username` (without the `@`).
    pub username: String,
    /// Bot’s display name.
    pub first_name: String,
    /// Whether the bot can be added to groups.
    pub can_join_groups: bool,
    /// Whether the bot has [privacy mode](https://core.telegram.org/bots/features#privacy-mode) disabled.
    pub can_read_all_group_messages: bool,
    /// Whether the bot supports inline queries.
    pub supports_inline_queries: bool,
}

// ─── User Profile Photos ───

/// A user’s profile photo collection.
#[derive(Debug, Clone)]
pub struct UserProfilePhotos {
    /// Total number of profile photos.
    pub total_count: i32,
    /// File IDs of the photos (largest size per photo).
    pub photos: Vec<String>,
}

// ─── Chat Invite Link ───

/// A chat invite link created by the bot or an admin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatInviteLink {
    /// The invite link URL.
    pub invite_link: String,
    /// Who created this link.
    pub creator: Option<UserInfo>,
    /// Whether users joining via this link need admin approval.
    pub creates_join_request: bool,
    /// Whether this is the chat’s primary invite link.
    pub is_primary: bool,
    /// Whether the link has been revoked.
    pub is_revoked: bool,
    /// Human-readable link name.
    pub name: Option<String>,
    /// Unix timestamp when the link expires.
    pub expire_date: Option<i64>,
    /// Maximum number of users that can join via this link.
    pub member_limit: Option<i32>,
    /// Number of pending join requests using this link.
    pub pending_join_request_count: Option<i32>,
}

// ─── Menu Button ───

/// The bot’s menu button in the chat input area.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MenuButton {
    /// Default behavior (Telegram decides).
    Default,
    /// Open bot’s list of commands.
    Commands,
    /// Open a Web App.
    WebApp {
        /// Button label.
        text: String,
        /// Web App URL.
        url: String,
    },
}

// ─── Bot Description ───

/// The bot’s long description (shown in the bot’s profile and on the start screen).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotDescription {
    /// The description text.
    pub description: String,
}

/// The bot’s short description (shown in inline mode search results).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotShortDescription {
    /// The short description text.
    pub short_description: String,
}

/// The bot’s display name.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotName {
    /// The bot’s name.
    pub name: String,
}

// ─── Shipping Query ───

/// A shipping option offered to the buyer during Telegram Payments checkout.
#[derive(Debug, Clone)]
pub struct ShippingOption {
    /// Unique option identifier.
    pub id: String,
    /// Human-readable option title.
    pub title: String,
    /// Prices in smallest units: `Vec<(label, amount)>`.
    pub prices: Vec<(String, i64)>,
}

// ─── Forum Topics ───

/// Represents a forum topic in a supergroup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForumTopic {
    /// Topic ID (same as the `message_id` of the topic’s creation service message).
    pub id: i32,
    /// Topic name.
    pub title: String,
    /// RGB icon color as a 24-bit integer, or `None` for custom emoji icons.
    pub icon_color: Option<i32>,
    /// Custom emoji ID used as the topic icon.
    pub icon_custom_emoji_id: Option<String>,
    /// Whether the topic is closed (no new messages).
    pub is_closed: bool,
    /// Whether the topic is hidden from the topic list (only General topic can be hidden).
    pub is_hidden: bool,
}

// ─── Stars / Payments (extended) ───

/// A Telegram Stars transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StarTransaction {
    /// Unique transaction identifier.
    pub id: String,
    /// Amount in Stars (can be negative for outbound).
    pub amount: i64,
    /// Fractional nanos (for sub-star precision).
    pub nanos: i32,
    /// Unix timestamp of the transaction.
    pub date: i32,
    /// The other party in the transaction.
    pub source: StarTransactionPeer,
    /// Product / purchase title.
    pub title: Option<String>,
    /// Product description.
    pub description: Option<String>,
    /// Whether this transaction is a refund of a previous one.
    pub is_refund: bool,
}

/// Who is on the other side of a star transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StarTransactionPeer {
    /// A Telegram user paid or was refunded.
    User(UserId),
    /// Transaction through the App Store.
    AppStore,
    /// Transaction through Google Play.
    PlayMarket,
    /// Transaction through Fragment.
    Fragment,
    /// Telegram Premium bot subscription.
    PremiumBot,
    /// Telegram Ads platform.
    Ads,
    /// Bot API (e.g. `refundStarPayment`).
    Api,
    /// Source not recognized by this version of blazegram.
    Unknown,
}

/// The bot’s current Telegram Stars balance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StarBalance {
    /// Whole Stars amount.
    pub amount: i64,
    /// Fractional nanos (for sub-star precision).
    pub nanos: i32,
}

/// Response from `get_star_transactions`.
#[derive(Debug, Clone)]
pub struct StarTransactions {
    /// Current balance at the time of the query.
    pub balance: StarBalance,
    /// List of transactions.
    pub transactions: Vec<StarTransaction>,
    /// Offset for the next page, or `None` if there are no more.
    pub next_offset: Option<String>,
}
