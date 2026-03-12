//! DiffOp executor — applies diff operations via BotApi.
//!
//! Automatically retries on FLOOD_WAIT and falls back to plain text on ENTITY_BOUNDS_INVALID.
//! Handles MessageNotFound (re-send) and MessageNotModified (sync hashes) gracefully.

use crate::bot_api::BotApi;
use crate::differ::{DiffOp, EditType};
use crate::error::ApiError;
use crate::types::*;

/// Executes [`DiffOp`] operations against the Telegram API.
pub struct DiffExecutor;

/// Maximum FLOOD_WAIT retries per operation.
const MAX_FLOOD_RETRIES: u32 = 2;

impl DiffExecutor {
    /// Execute diff ops against Telegram API.
    /// Updates `tracked` in-place to reflect the new chat state.
    pub async fn execute(
        bot: &dyn BotApi,
        chat_id: ChatId,
        ops: Vec<DiffOp>,
        tracked: &mut Vec<TrackedMessage>,
    ) -> Result<(), ApiError> {
        if ops.is_empty() {
            return Ok(());
        }

        for op in ops {
            match op {
                DiffOp::Send {
                    content,
                    send_options,
                } => {
                    let result = Self::with_retry(|| {
                        bot.send_message(chat_id, content.clone(), send_options.clone())
                    })
                    .await;

                    match result {
                        Ok(sent) => {
                            tracing::debug!(msg_id = sent.message_id.0, "sent new message");
                            tracked.push(TrackedMessage::from_content(sent.message_id, &content));
                        }
                        Err(ApiError::EntityBoundsInvalid) => {
                            tracing::warn!("ENTITY_BOUNDS_INVALID on send, retrying as plain text");
                            let plain = content.as_plain_text();
                            match bot
                                .send_message(chat_id, plain.clone(), send_options.clone())
                                .await
                            {
                                Ok(sent) => {
                                    tracked.push(TrackedMessage::from_content(
                                        sent.message_id,
                                        &plain,
                                    ));
                                }
                                Err(e) => {
                                    tracing::error!(error = %e, "failed to send (plain fallback)");
                                    return Err(e);
                                }
                            }
                        }
                        Err(ApiError::BotBlocked) => return Err(ApiError::BotBlocked),
                        Err(e) => {
                            tracing::error!(error = %e, "failed to send message");
                            return Err(e);
                        }
                    }
                }

                DiffOp::Edit {
                    message_id,
                    content,
                    edit_type,
                } => {
                    let result =
                        Self::execute_edit(bot, chat_id, message_id, &content, edit_type).await;

                    match result {
                        Ok(()) => {
                            if let Some(t) = tracked.iter_mut().find(|t| t.message_id == message_id)
                            {
                                *t = TrackedMessage::from_content(message_id, &content);
                            }
                            tracing::debug!(msg_id = message_id.0, ?edit_type, "edited message");
                        }
                        Err(ApiError::MessageNotModified) => {
                            // Content was already up-to-date on Telegram's side.
                            // Update tracked hashes to stay in sync.
                            if let Some(t) = tracked.iter_mut().find(|t| t.message_id == message_id)
                            {
                                *t = TrackedMessage::from_content(message_id, &content);
                            }
                            tracing::debug!(
                                msg_id = message_id.0,
                                "not modified (already up to date)"
                            );
                        }
                        Err(ApiError::MessageNotFound) => {
                            tracing::warn!(msg_id = message_id.0, "not found, re-sending");
                            tracked.retain(|t| t.message_id != message_id);
                            if let Ok(sent) = bot
                                .send_message(chat_id, content.clone(), Default::default())
                                .await
                            {
                                tracked
                                    .push(TrackedMessage::from_content(sent.message_id, &content));
                            }
                        }
                        Err(ApiError::EntityBoundsInvalid) => {
                            tracing::warn!(
                                msg_id = message_id.0,
                                "ENTITY_BOUNDS_INVALID on edit, retrying as plain text"
                            );
                            let plain = content.as_plain_text();
                            let retry =
                                Self::execute_edit(bot, chat_id, message_id, &plain, edit_type)
                                    .await;
                            match retry {
                                Ok(()) => {
                                    if let Some(t) =
                                        tracked.iter_mut().find(|t| t.message_id == message_id)
                                    {
                                        *t = TrackedMessage::from_content(message_id, &plain);
                                    }
                                    tracing::debug!(
                                        msg_id = message_id.0,
                                        "edited (plain fallback)"
                                    );
                                }
                                Err(e) => {
                                    tracing::error!(msg_id = message_id.0, error = %e, "failed to edit (plain fallback)");
                                    return Err(e);
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!(msg_id = message_id.0, error = %e, "failed to edit");
                            return Err(e);
                        }
                    }
                }

                DiffOp::Delete { message_ids } => {
                    tracing::debug!(count = message_ids.len(), "deleting messages");
                    // Remove from tracked before deleting
                    for del_id in &message_ids {
                        tracked.retain(|t| t.message_id != *del_id);
                    }
                    // Best effort — messages may already be deleted
                    let _ = bot.delete_messages(chat_id, message_ids).await;
                }
            }
        }

        Ok(())
    }

    async fn execute_edit(
        bot: &dyn BotApi,
        chat_id: ChatId,
        message_id: MessageId,
        content: &MessageContent,
        edit_type: EditType,
    ) -> Result<(), ApiError> {
        Self::with_retry(|| async {
            match edit_type {
                EditType::Text => {
                    if let MessageContent::Text {
                        text,
                        parse_mode,
                        keyboard,
                        link_preview,
                    } = content
                    {
                        bot.edit_message_text(
                            chat_id,
                            message_id,
                            text.clone(),
                            *parse_mode,
                            keyboard.clone(),
                            matches!(link_preview, LinkPreview::Enabled),
                        )
                        .await
                    } else {
                        unreachable!("EditType::Text with non-Text content")
                    }
                }
                EditType::Caption => {
                    let caption = content.caption();
                    let keyboard = content.keyboard();
                    let pm = match content {
                        MessageContent::Photo { parse_mode, .. }
                        | MessageContent::Video { parse_mode, .. }
                        | MessageContent::Animation { parse_mode, .. }
                        | MessageContent::Document { parse_mode, .. } => *parse_mode,
                        _ => ParseMode::Html,
                    };
                    bot.edit_message_caption(chat_id, message_id, caption, pm, keyboard)
                        .await
                }
                EditType::Media => {
                    let keyboard = content.keyboard();
                    bot.edit_message_media(chat_id, message_id, content.clone(), keyboard)
                        .await
                }
                EditType::Keyboard => {
                    let keyboard = content.keyboard();
                    bot.edit_message_keyboard(chat_id, message_id, keyboard)
                        .await
                }
            }
        })
        .await
    }

    /// Retry an operation on FLOOD_WAIT, up to MAX_FLOOD_RETRIES times.
    async fn with_retry<F, Fut, T>(f: F) -> Result<T, ApiError>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<T, ApiError>>,
    {
        for attempt in 0..=MAX_FLOOD_RETRIES {
            match f().await {
                Err(ApiError::TooManyRequests { retry_after }) if attempt < MAX_FLOOD_RETRIES => {
                    let wait = (retry_after as u64).min(30);
                    tracing::warn!(retry_after = wait, attempt, "FLOOD_WAIT, backing off");
                    tokio::time::sleep(tokio::time::Duration::from_secs(wait)).await;
                }
                other => return other,
            }
        }
        unreachable!("loop with 0..=MAX_FLOOD_RETRIES always returns")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bot_api::SendOptions;
    use crate::mock::MockBotApi;

    fn text_content(t: &str) -> MessageContent {
        MessageContent::Text {
            text: t.to_string(),
            parse_mode: ParseMode::Html,
            keyboard: None,
            link_preview: LinkPreview::Disabled,
        }
    }

    #[tokio::test]
    async fn execute_send() {
        let bot = MockBotApi::new();
        let chat = ChatId(1);
        let content = text_content("Hello");
        let ops = vec![DiffOp::Send {
            content: content.clone(),
            send_options: SendOptions::default(),
        }];
        let mut tracked = Vec::new();
        DiffExecutor::execute(&bot, chat, ops, &mut tracked)
            .await
            .unwrap();
        assert_eq!(tracked.len(), 1);
        assert_eq!(bot.call_count_async().await, 1);
    }

    #[tokio::test]
    async fn execute_delete() {
        let bot = MockBotApi::new();
        let chat = ChatId(1);
        let msg_id = MessageId(42);
        let content = text_content("Old");
        let mut tracked = vec![TrackedMessage::from_content(msg_id, &content)];
        let ops = vec![DiffOp::Delete {
            message_ids: vec![msg_id],
        }];
        DiffExecutor::execute(&bot, chat, ops, &mut tracked)
            .await
            .unwrap();
        assert!(tracked.is_empty());
    }

    #[tokio::test]
    async fn execute_edit_text() {
        let bot = MockBotApi::new();
        let chat = ChatId(1);
        let msg_id = MessageId(10);
        let old_content = text_content("Old");
        let new_content = text_content("New");
        let mut tracked = vec![TrackedMessage::from_content(msg_id, &old_content)];
        let ops = vec![DiffOp::Edit {
            message_id: msg_id,
            content: new_content.clone(),
            edit_type: EditType::Text,
        }];
        DiffExecutor::execute(&bot, chat, ops, &mut tracked)
            .await
            .unwrap();
        assert_eq!(tracked.len(), 1);
        assert_eq!(tracked[0].content_hash, new_content.content_hash());
    }

    #[tokio::test]
    async fn execute_empty_ops() {
        let bot = MockBotApi::new();
        let mut tracked = Vec::new();
        DiffExecutor::execute(&bot, ChatId(1), vec![], &mut tracked)
            .await
            .unwrap();
        assert_eq!(bot.call_count_async().await, 0);
    }

    #[tokio::test]
    async fn execute_edit_keyboard_only() {
        let bot = MockBotApi::new();
        let chat = ChatId(1);
        let msg_id = MessageId(5);
        let content = text_content("Same text");
        let mut tracked = vec![TrackedMessage::from_content(msg_id, &content)];
        let ops = vec![DiffOp::Edit {
            message_id: msg_id,
            content: content.clone(),
            edit_type: EditType::Keyboard,
        }];
        DiffExecutor::execute(&bot, chat, ops, &mut tracked)
            .await
            .unwrap();
        assert_eq!(tracked.len(), 1);
    }

    #[tokio::test]
    async fn execute_multiple_ops() {
        let bot = MockBotApi::new();
        let chat = ChatId(1);
        let ops = vec![
            DiffOp::Send {
                content: text_content("A"),
                send_options: SendOptions::default(),
            },
            DiffOp::Send {
                content: text_content("B"),
                send_options: SendOptions::default(),
            },
        ];
        let mut tracked = Vec::new();
        DiffExecutor::execute(&bot, chat, ops, &mut tracked)
            .await
            .unwrap();
        assert_eq!(tracked.len(), 2);
    }
}
