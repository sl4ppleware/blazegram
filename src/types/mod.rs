//! Core types: ChatId, MessageId, Screen identifiers, message content, parsed updates.

use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::hash::Hasher;

mod chat;
mod content;
mod telegram;
mod update;

// Re-export everything publicly so `use crate::types::*` still works.
pub use chat::*;
pub use content::*;
pub use telegram::*;
pub use update::*;

// ─── Fixed Hasher ───

/// Fixed-seed FNV-1a hasher for deterministic content hashing across restarts.
/// DefaultHasher uses random SipHash keys per process — breaks tracked message
/// comparison after restart.
pub(crate) fn new_fixed_hasher() -> FixedHasher {
    FixedHasher(0xcbf29ce484222325)
}

pub(crate) struct FixedHasher(u64);

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
    /// The screen's string identifier.
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

// ─── Inline Query Result (BotApi-level) ───

/// A single result for answering an inline query.
#[derive(Debug, Clone)]
pub struct InlineQueryResult {
    /// Unique result identifier (1–64 bytes).
    pub id: String,
    /// The kind of inline result.
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
    /// A photo result.
    Photo {
        /// Direct URL to the photo.
        url: String,
    },
    /// An animated GIF result.
    Gif {
        /// Direct URL to the GIF file.
        url: String,
    },
    /// A video result.
    Video {
        /// Direct URL to the video.
        url: String,
        /// MIME type (e.g. `"video/mp4"`).
        mime: String,
    },
    /// A voice message result.
    Voice {
        /// Direct URL to the OGG audio.
        url: String,
    },
    /// A document / file result.
    Document {
        /// Direct URL to the document.
        url: String,
        /// MIME type.
        mime: String,
    },
}
