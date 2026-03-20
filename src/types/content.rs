use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

use super::new_fixed_hasher;

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
    #[must_use]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
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
        /// Image source (file ID, URL, local path, or raw bytes).
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

    /// Deterministic hash of the entire content (type + text/caption + media + keyboard + formatting options).
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
                parse_mode,
            } => {
                source.hash(&mut hasher);
                caption.hash(&mut hasher);
                if let Some(kb) = keyboard {
                    kb.hash(&mut hasher);
                }
                spoiler.hash(&mut hasher);
                parse_mode.hash(&mut hasher);
            }
            Self::Video {
                source,
                caption,
                keyboard,
                spoiler,
                parse_mode,
            } => {
                source.hash(&mut hasher);
                caption.hash(&mut hasher);
                if let Some(kb) = keyboard {
                    kb.hash(&mut hasher);
                }
                spoiler.hash(&mut hasher);
                parse_mode.hash(&mut hasher);
            }
            Self::Animation {
                source,
                caption,
                keyboard,
                spoiler,
                parse_mode,
            } => {
                source.hash(&mut hasher);
                caption.hash(&mut hasher);
                if let Some(kb) = keyboard {
                    kb.hash(&mut hasher);
                }
                spoiler.hash(&mut hasher);
                parse_mode.hash(&mut hasher);
            }
            Self::Document {
                source,
                caption,
                keyboard,
                filename,
                parse_mode,
            } => {
                source.hash(&mut hasher);
                caption.hash(&mut hasher);
                if let Some(kb) = keyboard {
                    kb.hash(&mut hasher);
                }
                filename.hash(&mut hasher);
                parse_mode.hash(&mut hasher);
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

    /// Hash of the text body and parse mode (for [`Text`](Self::Text) variants).
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_hash_differs_on_parse_mode_photo() {
        let a = MessageContent::Photo {
            source: FileSource::FileId("abc".into()),
            caption: Some("cap".into()),
            parse_mode: ParseMode::Html,
            keyboard: None,
            spoiler: false,
        };
        let b = MessageContent::Photo {
            source: FileSource::FileId("abc".into()),
            caption: Some("cap".into()),
            parse_mode: ParseMode::MarkdownV2,
            keyboard: None,
            spoiler: false,
        };
        assert_ne!(
            a.content_hash(),
            b.content_hash(),
            "different parse_mode must produce different hash"
        );
    }

    #[test]
    fn content_hash_differs_on_parse_mode_video() {
        let a = MessageContent::Video {
            source: FileSource::FileId("v".into()),
            caption: None,
            parse_mode: ParseMode::Html,
            keyboard: None,
            spoiler: false,
        };
        let b = MessageContent::Video {
            source: FileSource::FileId("v".into()),
            caption: None,
            parse_mode: ParseMode::None,
            keyboard: None,
            spoiler: false,
        };
        assert_ne!(a.content_hash(), b.content_hash());
    }

    #[test]
    fn content_hash_differs_on_parse_mode_animation() {
        let a = MessageContent::Animation {
            source: FileSource::FileId("g".into()),
            caption: None,
            parse_mode: ParseMode::Html,
            keyboard: None,
            spoiler: false,
        };
        let b = MessageContent::Animation {
            source: FileSource::FileId("g".into()),
            caption: None,
            parse_mode: ParseMode::MarkdownV2,
            keyboard: None,
            spoiler: false,
        };
        assert_ne!(a.content_hash(), b.content_hash());
    }

    #[test]
    fn content_hash_differs_on_parse_mode_document() {
        let a = MessageContent::Document {
            source: FileSource::FileId("d".into()),
            caption: None,
            parse_mode: ParseMode::Html,
            keyboard: None,
            filename: None,
        };
        let b = MessageContent::Document {
            source: FileSource::FileId("d".into()),
            caption: None,
            parse_mode: ParseMode::None,
            keyboard: None,
            filename: None,
        };
        assert_ne!(a.content_hash(), b.content_hash());
    }
}
