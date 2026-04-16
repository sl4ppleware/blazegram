//! Broadcast — mass messaging with rate limiting and dismiss button.

use std::time::Duration;
use tokio::time::sleep;

use crate::bot_api::{BotApi, SendOptions};
use crate::error::ApiError;
use crate::i18n::ft;
use crate::keyboard::InlineKeyboard;
use crate::screen::Screen;
use crate::state::StateStore;
use crate::types::*;

/// Result of a broadcast operation.
#[derive(Debug, Clone, Default)]
pub struct BroadcastResult {
    /// Number of messages successfully delivered.
    pub sent: u32,
    /// Number of chats that blocked the bot.
    pub blocked: u32,
    /// Number of delivery failures.
    pub failed: u32,
}

/// Options for broadcast.
pub struct BroadcastOptions {
    /// Delay between messages (to avoid 429).
    pub delay: Duration,
    /// If true, adds a dismiss button (✖️) that deletes the message.
    pub hideable: bool,
    /// Custom dismiss button text.
    pub dismiss_text: String,
    /// Callback data prefix for dismiss. Default: "__dismiss".
    pub dismiss_callback: String,
}

impl Default for BroadcastOptions {
    fn default() -> Self {
        Self {
            delay: Duration::from_millis(35), // ~28 msg/s, under 30 rps limit
            hideable: false,
            dismiss_text: ft("en", "bg-dismiss"),
            dismiss_callback: "__dismiss".to_string(),
        }
    }
}

impl BroadcastOptions {
    /// If `true`, the message can be hidden by the user.
    pub fn hideable(mut self) -> Self {
        self.hideable = true;
        self
    }

    /// Per-message delay to stay within rate limits.
    pub fn delay(mut self, d: Duration) -> Self {
        self.delay = d;
        self
    }

    /// Dismiss text.
    pub fn dismiss_text(mut self, t: impl Into<String>) -> Self {
        self.dismiss_text = t.into();
        self
    }
}

/// Broadcast a screen to all known chats.
pub async fn broadcast(
    bot: &dyn BotApi,
    store: &dyn StateStore,
    screen: Screen,
    opts: BroadcastOptions,
) -> BroadcastResult {
    let chat_ids = match store.all_chat_ids().await {
        Ok(ids) => ids,
        Err(e) => {
            tracing::error!(error = %e, "broadcast: failed to load chat IDs");
            return BroadcastResult::default();
        }
    };
    let mut result = BroadcastResult::default();

    for chat_id in chat_ids {
        // Build content with optional dismiss button
        for msg in &screen.messages {
            let mut content = msg.content.clone();
            if opts.hideable {
                content = add_dismiss_button(content, &opts.dismiss_text, &opts.dismiss_callback);
            }
            match bot
                .send_message(chat_id, content.clone(), SendOptions::default())
                .await
            {
                Ok(_) => result.sent += 1,
                Err(ApiError::BotBlocked) => {
                    result.blocked += 1;
                    break; // skip remaining messages for this chat
                }
                Err(ApiError::TooManyRequests { retry_after }) => {
                    sleep(Duration::from_secs(retry_after as u64 + 1)).await;
                    // Retry once with the same content (including dismiss button)
                    match bot
                        .send_message(chat_id, content, SendOptions::default())
                        .await
                    {
                        Ok(_) => result.sent += 1,
                        Err(ApiError::BotBlocked | ApiError::ChatNotFound) => {
                            result.blocked += 1;
                            break;
                        }
                        Err(_) => result.failed += 1,
                    }
                }
                Err(ApiError::ChatNotFound) => {
                    result.blocked += 1;
                    break;
                }
                Err(_) => result.failed += 1,
            }
        }
        sleep(opts.delay).await;
    }

    result
}

/// Broadcast a text message to all chats.
pub async fn broadcast_text(
    bot: &dyn BotApi,
    store: &dyn StateStore,
    text: impl Into<String>,
    opts: BroadcastOptions,
) -> BroadcastResult {
    let screen = Screen::text("__broadcast", text).build();
    broadcast(bot, store, screen, opts).await
}

fn add_dismiss_button(content: MessageContent, text: &str, callback: &str) -> MessageContent {
    match content {
        MessageContent::Text {
            text: t,
            parse_mode,
            keyboard,
            link_preview,
        } => {
            let mut kb = keyboard.unwrap_or_else(|| InlineKeyboard { rows: vec![] });
            kb.rows.push(vec![crate::keyboard::InlineButton {
                text: text.to_string(),
                action: crate::keyboard::ButtonAction::Callback(callback.to_string()),
            }]);
            MessageContent::Text {
                text: t,
                parse_mode,
                keyboard: Some(kb),
                link_preview,
            }
        }
        MessageContent::Photo {
            source,
            caption,
            parse_mode,
            keyboard,
            spoiler,
        } => {
            let mut kb = keyboard.unwrap_or_else(|| InlineKeyboard { rows: vec![] });
            kb.rows.push(vec![crate::keyboard::InlineButton {
                text: text.to_string(),
                action: crate::keyboard::ButtonAction::Callback(callback.to_string()),
            }]);
            MessageContent::Photo {
                source,
                caption,
                parse_mode,
                keyboard: Some(kb),
                spoiler,
            }
        }
        MessageContent::Video {
            source,
            caption,
            parse_mode,
            keyboard,
            spoiler,
        } => {
            let mut kb = keyboard.unwrap_or_else(|| InlineKeyboard { rows: vec![] });
            kb.rows.push(vec![crate::keyboard::InlineButton {
                text: text.to_string(),
                action: crate::keyboard::ButtonAction::Callback(callback.to_string()),
            }]);
            MessageContent::Video {
                source,
                caption,
                parse_mode,
                keyboard: Some(kb),
                spoiler,
            }
        }
        MessageContent::Animation {
            source,
            caption,
            parse_mode,
            keyboard,
            spoiler,
        } => {
            let mut kb = keyboard.unwrap_or_else(|| InlineKeyboard { rows: vec![] });
            kb.rows.push(vec![crate::keyboard::InlineButton {
                text: text.to_string(),
                action: crate::keyboard::ButtonAction::Callback(callback.to_string()),
            }]);
            MessageContent::Animation {
                source,
                caption,
                parse_mode,
                keyboard: Some(kb),
                spoiler,
            }
        }
        MessageContent::Document {
            source,
            caption,
            parse_mode,
            keyboard,
            filename,
        } => {
            let mut kb = keyboard.unwrap_or_else(|| InlineKeyboard { rows: vec![] });
            kb.rows.push(vec![crate::keyboard::InlineButton {
                text: text.to_string(),
                action: crate::keyboard::ButtonAction::Callback(callback.to_string()),
            }]);
            MessageContent::Document {
                source,
                caption,
                parse_mode,
                keyboard: Some(kb),
                filename,
            }
        }
        other => other, // sticker, location — no keyboard support
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{LinkPreview, ParseMode};

    #[test]
    fn add_dismiss_to_text() {
        let content = MessageContent::Text {
            text: "Hello".into(),
            parse_mode: ParseMode::Html,
            keyboard: None,
            link_preview: LinkPreview::Disabled,
        };
        let result = add_dismiss_button(content, "Dismiss", "__dismiss");
        if let MessageContent::Text { keyboard, .. } = &result {
            let kb = keyboard.as_ref().unwrap();
            assert_eq!(kb.rows.len(), 1);
            assert_eq!(kb.rows[0][0].text, "Dismiss");
        } else {
            panic!("Expected Text content");
        }
    }

    #[test]
    fn add_dismiss_preserves_existing_keyboard() {
        let existing_kb = InlineKeyboard {
            rows: vec![vec![crate::keyboard::InlineButton {
                text: "Existing".into(),
                action: crate::keyboard::ButtonAction::Callback("existing".into()),
            }]],
        };
        let content = MessageContent::Text {
            text: "Hello".into(),
            parse_mode: ParseMode::Html,
            keyboard: Some(existing_kb),
            link_preview: LinkPreview::Disabled,
        };
        let result = add_dismiss_button(content, "Dismiss", "__dismiss");
        if let MessageContent::Text { keyboard, .. } = &result {
            let kb = keyboard.as_ref().unwrap();
            assert_eq!(kb.rows.len(), 2);
            assert_eq!(kb.rows[0][0].text, "Existing");
            assert_eq!(kb.rows[1][0].text, "Dismiss");
        } else {
            panic!("Expected Text content");
        }
    }

    #[test]
    fn add_dismiss_to_video() {
        let content = MessageContent::Video {
            source: FileSource::FileId("vid123".into()),
            caption: Some("Watch this".into()),
            parse_mode: ParseMode::Html,
            keyboard: None,
            spoiler: false,
        };
        let result = add_dismiss_button(content, "Dismiss", "__dismiss");
        if let MessageContent::Video {
            keyboard,
            caption,
            spoiler,
            ..
        } = &result
        {
            let kb = keyboard.as_ref().unwrap();
            assert_eq!(kb.rows.len(), 1);
            assert_eq!(kb.rows[0][0].text, "Dismiss");
            assert_eq!(caption.as_deref(), Some("Watch this"));
            assert!(!spoiler);
        } else {
            panic!("Expected Video content");
        }
    }

    #[test]
    fn broadcast_options_default() {
        let opts = BroadcastOptions::default();
        assert!(!opts.hideable);
        assert_eq!(opts.delay, Duration::from_millis(35));
    }

    #[test]
    fn broadcast_options_builder() {
        let opts = BroadcastOptions::default()
            .hideable()
            .delay(Duration::from_millis(100))
            .dismiss_text("Hide");
        assert!(opts.hideable);
        assert_eq!(opts.delay, Duration::from_millis(100));
        assert_eq!(opts.dismiss_text, "Hide");
    }
}
