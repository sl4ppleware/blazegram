use super::Contact;
use super::chat::UserInfo;
use super::content::ContentType;

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
    pub chat_id: super::ChatId,
    /// User who triggered the update.
    pub user: UserInfo,
    /// Message ID, if applicable to this update type.
    pub message_id: Option<super::MessageId>,
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
        /// Original filename reported by the sender's client.
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
        /// Total amount in the smallest currency unit (e.g. cents).
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
    pub fn chat_id(&self) -> super::ChatId {
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
                // Must be exactly /start, /start payload, or /start@bot payload
                let rest = if rest.is_empty() {
                    return None; // bare /start — no payload
                } else if let Some(after_at) = rest.strip_prefix('@') {
                    // /start@botname payload
                    after_at.find(' ').map(|i| &after_at[i..]).unwrap_or("")
                } else if rest.starts_with(' ') {
                    rest
                } else {
                    return None; // /starting, /starter etc — not a deep link
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

    /// Convert media-bearing variants to a normalized [`ReceivedMedia`].
    ///
    /// Returns `None` for non-media variants (text messages, callbacks, etc.).
    pub fn to_received_media(&self) -> Option<ReceivedMedia> {
        match self {
            Self::Photo {
                file_id,
                file_unique_id,
                caption,
            } => Some(ReceivedMedia {
                file_id: file_id.clone(),
                file_unique_id: file_unique_id.clone(),
                file_type: ContentType::Photo,
                caption: caption.clone(),
                filename: None,
            }),
            Self::Document {
                file_id,
                file_unique_id,
                filename,
                caption,
            } => Some(ReceivedMedia {
                file_id: file_id.clone(),
                file_unique_id: file_unique_id.clone(),
                file_type: ContentType::Document,
                caption: caption.clone(),
                filename: filename.clone(),
            }),
            Self::Voice {
                file_id,
                file_unique_id,
                caption,
                ..
            } => Some(ReceivedMedia {
                file_id: file_id.clone(),
                file_unique_id: file_unique_id.clone(),
                file_type: ContentType::Voice,
                caption: caption.clone(),
                filename: None,
            }),
            Self::VideoNote {
                file_id,
                file_unique_id,
                ..
            } => Some(ReceivedMedia {
                file_id: file_id.clone(),
                file_unique_id: file_unique_id.clone(),
                file_type: ContentType::VideoNote,
                caption: None,
                filename: None,
            }),
            Self::Video {
                file_id,
                file_unique_id,
                caption,
            } => Some(ReceivedMedia {
                file_id: file_id.clone(),
                file_unique_id: file_unique_id.clone(),
                file_type: ContentType::Video,
                caption: caption.clone(),
                filename: None,
            }),
            Self::Sticker {
                file_id,
                file_unique_id,
            } => Some(ReceivedMedia {
                file_id: file_id.clone(),
                file_unique_id: file_unique_id.clone(),
                file_type: ContentType::Sticker,
                caption: None,
                filename: None,
            }),
            _ => None,
        }
    }
}

// ─── Received Media (for input handlers) ───

/// A media file received from the user, normalized across photo / video /
/// document / voice / etc. update kinds.
///
/// Passed to media input handlers registered with
/// [`App::on_media_input`](crate::app::App) or [`Form`](crate::form::Form) photo steps.
#[derive(Debug, Clone, PartialEq)]
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

// InputSpec needs ValidatorFn which is already defined in chat.rs, re-export not needed
// since we use super::chat::ValidatorFn. But the type alias ValidatorFn is in chat.rs.
// Actually we don't use ValidatorFn directly in this file — it's only used via InputSpec.
// InputSpec is in chat.rs, so this file is fine.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ChatId, MessageId, UserId};

    fn make_msg(text: &str) -> IncomingUpdate {
        IncomingUpdate {
            chat_id: ChatId(1),
            user: UserInfo {
                id: UserId(1),
                first_name: "Test".into(),
                last_name: None,
                username: None,
                language_code: None,
            },
            message_id: Some(MessageId(1)),
            kind: UpdateKind::Message {
                text: Some(text.into()),
            },
        }
    }

    #[test]
    fn deep_link_with_payload() {
        assert_eq!(make_msg("/start payload").deep_link(), Some("payload"));
    }

    #[test]
    fn deep_link_with_bot_name() {
        assert_eq!(make_msg("/start@bot payload").deep_link(), Some("payload"));
    }

    #[test]
    fn deep_link_bare_start_is_none() {
        assert_eq!(make_msg("/start").deep_link(), None);
    }

    #[test]
    fn deep_link_starting_not_matched() {
        assert_eq!(make_msg("/starting something").deep_link(), None);
    }

    #[test]
    fn deep_link_starter_not_matched() {
        assert_eq!(make_msg("/starter foo").deep_link(), None);
    }
}
