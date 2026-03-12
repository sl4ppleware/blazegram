use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::{MessageContent, MessageId, ScreenId};

fn default_true() -> bool {
    true
}

// ─── Chat Action ───

/// A "typing indicator" action shown in the chat header.
///
/// Send via [`BotApi::send_chat_action`](crate::bot_api::BotApi::send_chat_action)
/// to let the user know the bot is working on something.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserInfo {
    /// Telegram user ID.
    pub id: super::UserId,
    /// User's first name.
    pub first_name: String,
    /// User's last name (not everyone has one).
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackedMessage {
    /// ID of the sent message.
    pub message_id: MessageId,
    /// What kind of content this message carries.
    pub content_type: crate::types::ContentType,
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
#[derive(Debug, Clone, PartialEq)]
pub struct SentMessage {
    /// ID of the newly sent message.
    pub message_id: MessageId,
    /// Chat the message was sent to.
    pub chat_id: super::ChatId,
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
    pub chat_id: super::ChatId,
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
    ///
    /// Uses `serde_json::Value` for maximum flexibility (any `Serialize` type).
    /// This means postcard snapshots encode JSON-inside-binary, adding ~30% overhead
    /// compared to a uniform format. Acceptable for typical bot state sizes (<10 KB).
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
    pub fn new(chat_id: super::ChatId, user: UserInfo) -> Self {
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
