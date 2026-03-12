//! Virtual Chat Differ — the heart of Blazegram.
//!
//! Compares current tracked messages with the new Screen and produces
//! the minimal set of API operations for a transition.
//!
//! Key insight: **edit vs delete+send** depends on whether user messages
//! are pending (i.e. the user typed something). If the user sent a message,
//! the old bot message is scrolled up — editing it in-place is invisible.
//! We must delete it and send a fresh one at the bottom.
//!
//! - Callback (button press): no pending user messages → **edit** in place
//! - Message/command: pending user messages → **delete old + send new** at bottom

use crate::bot_api::SendOptions;
use crate::screen::{ReplyKeyboardAction, Screen};
use crate::types::*;

/// A single operation the differ wants the executor to perform.
#[derive(Debug, Clone)]
pub enum DiffOp {
    /// Send a new message.
    Send {
        /// The content to send.
        content: MessageContent,
        /// Delivery options (reply-to, protect content, etc.).
        send_options: SendOptions,
    },
    /// Edit an existing message in place.
    Edit {
        /// ID of the message to edit.
        message_id: MessageId,
        /// New content to replace the old.
        content: MessageContent,
        /// What kind of edit to perform.
        edit_type: EditType,
    },
    /// Delete one or more messages.
    Delete {
        /// IDs of the messages to delete.
        message_ids: Vec<MessageId>,
    },
}

/// What part of a message is being edited.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditType {
    /// Edit the text body via `editMessageText`.
    Text,
    /// Edit the caption via `editMessageCaption`.
    Caption,
    /// Replace the media via `editMessageMedia`.
    Media,
    /// Change only the inline keyboard via `editMessageReplyMarkup`.
    Keyboard,
}

/// The Virtual Chat Differ — computes the minimal set of Telegram API calls
/// to transition from the current on-screen messages to a new [`Screen`].
pub struct Differ;

impl Differ {
    /// Compute diff operations assuming no frozen messages.
    pub fn diff(
        old_messages: &[TrackedMessage],
        new_screen: &Screen,
        pending_user_messages: &[MessageId],
    ) -> Vec<DiffOp> {
        Self::diff_with_frozen(old_messages, new_screen, pending_user_messages, &[])
    }

    /// Compute diff operations, excluding `frozen_messages` from deletion.
    pub fn diff_with_frozen(
        old_messages: &[TrackedMessage],
        new_screen: &Screen,
        pending_user_messages: &[MessageId],
        frozen_messages: &[MessageId],
    ) -> Vec<DiffOp> {
        let has_user_messages = !pending_user_messages.is_empty();
        let new_messages = &new_screen.messages;

        let send_opts = SendOptions {
            protect_content: new_screen.protect_content,
            reply_keyboard: None, // attached below to specific ops
            reply_to: new_screen.reply_to,
            message_thread_id: None,
        };

        // Force delete+send when protect_content is set (edit API doesn't support it),
        // or when reply_keyboard is set (can only be sent with a new message, not edit).
        let force_send = new_screen.protect_content || new_screen.reply_keyboard.is_some();

        let mut ops = if has_user_messages || force_send {
            Self::diff_replace_all(
                old_messages,
                new_messages,
                pending_user_messages,
                frozen_messages,
                send_opts,
            )
        } else {
            Self::diff_edit_in_place(old_messages, new_messages, frozen_messages, send_opts)
        };

        // Attach reply_keyboard to a Send op that has no inline keyboard,
        // so they don't conflict over Telegram's reply_markup field.
        if let Some(ref action) = new_screen.reply_keyboard {
            let attached = Self::attach_reply_keyboard(&mut ops, action);
            if !attached {
                // All messages have inline keyboards — add a dedicated helper Send.
                ops.push(DiffOp::Send {
                    content: MessageContent::Text {
                        text: "\u{200B}".to_string(),
                        parse_mode: ParseMode::None,
                        keyboard: None,
                        link_preview: LinkPreview::Disabled,
                    },
                    send_options: SendOptions {
                        protect_content: false,
                        reply_keyboard: Some(action.clone()),
                        reply_to: None,
                        message_thread_id: None,
                    },
                });
            }
        }

        ops
    }

    /// Attach reply_keyboard to the first Send op whose content has no inline keyboard.
    /// Returns true if successfully attached.
    fn attach_reply_keyboard(ops: &mut [DiffOp], action: &ReplyKeyboardAction) -> bool {
        for op in ops.iter_mut() {
            if let DiffOp::Send {
                content,
                send_options,
            } = op
            {
                if content.keyboard().is_none() {
                    send_options.reply_keyboard = Some(action.clone());
                    return true;
                }
            }
        }
        false
    }

    /// Used when user sent a message (old bot messages are scrolled up).
    fn diff_replace_all(
        old_messages: &[TrackedMessage],
        new_messages: &[crate::screen::ScreenMessage],
        pending_user_messages: &[MessageId],
        frozen_messages: &[MessageId],
        send_opts: SendOptions,
    ) -> Vec<DiffOp> {
        let mut ops = Vec::new();

        // Collect all message IDs to delete (excluding frozen)
        let mut to_delete: Vec<MessageId> = old_messages
            .iter()
            .filter(|m| !frozen_messages.contains(&m.message_id))
            .map(|m| m.message_id)
            .collect();
        to_delete.extend_from_slice(pending_user_messages);

        // Delete old + user messages FIRST to avoid flicker
        // (otherwise user momentarily sees both old and new messages)
        if !to_delete.is_empty() {
            ops.push(DiffOp::Delete {
                message_ids: to_delete,
            });
        }

        // Then send all new messages
        for msg in new_messages {
            ops.push(DiffOp::Send {
                content: msg.content.clone(),
                send_options: send_opts.clone(),
            });
        }

        ops
    }

    /// Strategy: edit messages in-place for minimal flicker.
    /// Used for callback-triggered navigation (no user messages to clean up).
    fn diff_edit_in_place(
        old_messages: &[TrackedMessage],
        new_messages: &[crate::screen::ScreenMessage],
        frozen_messages: &[MessageId],
        send_opts: SendOptions,
    ) -> Vec<DiffOp> {
        let old_len = old_messages.len();
        let new_len = new_messages.len();
        let common = old_len.min(new_len);

        let mut edit_ops: Vec<DiffOp> = Vec::new();
        let mut send_ops: Vec<DiffOp> = Vec::new();
        let mut to_delete: Vec<MessageId> = Vec::new();

        // Compare common positions — edit if different
        for i in 0..common {
            let old = &old_messages[i];
            let new = &new_messages[i];
            let new_hash = new.content.content_hash();

            if old.content_hash == new_hash {
                continue; // identical — zero API calls
            }

            let old_type = old.content_type;
            let new_type = new.content.content_type();

            if old_type.can_edit_to(&new_type) {
                let edit_type = determine_edit_type(old, &new.content);
                edit_ops.push(DiffOp::Edit {
                    message_id: old.message_id,
                    content: new.content.clone(),
                    edit_type,
                });
            } else {
                // Incompatible types — delete old, send new
                to_delete.push(old.message_id);
                send_ops.push(DiffOp::Send {
                    content: new.content.clone(),
                    send_options: send_opts.clone(),
                });
            }
        }

        // Extra old messages → delete (excluding frozen)
        for old in &old_messages[common..] {
            if !frozen_messages.contains(&old.message_id) {
                to_delete.push(old.message_id);
            }
        }

        // Extra new messages → send
        for new in &new_messages[common..] {
            send_ops.push(DiffOp::Send {
                content: new.content.clone(),
                send_options: send_opts.clone(),
            });
        }

        // Order: edits first (instant visual update), then deletes, then sends.
        // This minimises flicker: edits update in-place, deletes remove surplus,
        // sends add new messages at the bottom.
        let mut ops = edit_ops;
        if !to_delete.is_empty() {
            ops.push(DiffOp::Delete {
                message_ids: to_delete,
            });
        }
        ops.extend(send_ops);
        ops
    }
}

fn determine_edit_type(old: &TrackedMessage, new_content: &MessageContent) -> EditType {
    let new_type = new_content.content_type();

    if old.content_type == ContentType::Text && new_type == ContentType::Text {
        if old.text_hash == new_content.text_hash()
            && old.keyboard_hash != new_content.keyboard_hash()
        {
            return EditType::Keyboard;
        }
        return EditType::Text;
    }

    // Media → Media
    if old.file_hash == new_content.file_hash() {
        if old.keyboard_hash != new_content.keyboard_hash()
            && old.caption_hash == new_content.caption_hash()
        {
            return EditType::Keyboard;
        }
        return EditType::Caption;
    }

    EditType::Media
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screen::{Screen, ScreenMessage};

    fn text_content(s: &str) -> MessageContent {
        MessageContent::Text {
            text: s.into(),
            parse_mode: ParseMode::Html,
            keyboard: None,
            link_preview: LinkPreview::Disabled,
        }
    }

    #[test]
    fn test_same_content_no_ops() {
        let content = text_content("hello");
        let old = vec![TrackedMessage::from_content(MessageId(1), &content)];
        let screen = Screen {
            id: ScreenId::from("test"),
            messages: vec![ScreenMessage { content }],
            input: None,
            typing_action: None,
            protect_content: false,
            reply_keyboard: None,
            reply_to: None,
        };
        // No user messages → callback path → edit in place
        let ops = Differ::diff(&old, &screen, &[]);
        assert!(ops.is_empty(), "identical content should produce no ops");
    }

    #[test]
    fn test_callback_edits_in_place() {
        let old = vec![TrackedMessage::from_content(
            MessageId(1),
            &text_content("hello"),
        )];
        let screen = Screen {
            id: ScreenId::from("test"),
            messages: vec![ScreenMessage {
                content: text_content("world"),
            }],
            input: None,
            typing_action: None,
            protect_content: false,
            reply_keyboard: None,
            reply_to: None,
        };
        // No user messages → edit
        let ops = Differ::diff(&old, &screen, &[]);
        assert!(ops.iter().any(|op| matches!(op, DiffOp::Edit { .. })));
        assert!(!ops.iter().any(|op| matches!(op, DiffOp::Send { .. })));
    }

    #[test]
    fn test_message_deletes_old_and_sends_new() {
        let old = vec![TrackedMessage::from_content(
            MessageId(1),
            &text_content("hello"),
        )];
        let screen = Screen {
            id: ScreenId::from("test"),
            messages: vec![ScreenMessage {
                content: text_content("world"),
            }],
            input: None,
            typing_action: None,
            protect_content: false,
            reply_keyboard: None,
            reply_to: None,
        };
        // User messages present → delete + send
        let user_msgs = vec![MessageId(10)];
        let ops = Differ::diff(&old, &screen, &user_msgs);
        assert!(
            ops.iter().any(|op| matches!(op, DiffOp::Send { .. })),
            "should send new"
        );
        assert!(
            !ops.iter().any(|op| matches!(op, DiffOp::Edit { .. })),
            "should NOT edit"
        );
        let del = ops
            .iter()
            .find(|op| matches!(op, DiffOp::Delete { .. }))
            .unwrap();
        if let DiffOp::Delete { message_ids } = del {
            assert!(
                message_ids.contains(&MessageId(1)),
                "should delete old bot msg"
            );
            assert!(
                message_ids.contains(&MessageId(10)),
                "should delete user msg"
            );
        }
    }

    #[test]
    fn test_empty_to_new_sends() {
        let screen = Screen::builder("test").text("hello").build();
        let ops = Differ::diff(&[], &screen, &[]);
        assert_eq!(
            ops.iter()
                .filter(|op| matches!(op, DiffOp::Send { .. }))
                .count(),
            1
        );
    }

    #[test]
    fn test_frozen_messages_not_deleted() {
        let content = text_content("hello");
        let old = vec![
            TrackedMessage::from_content(MessageId(1), &content),
            TrackedMessage::from_content(MessageId(2), &text_content("frozen")),
        ];
        let screen = Screen {
            id: ScreenId::from("test"),
            messages: vec![ScreenMessage {
                content: text_content("new"),
            }],
            input: None,
            typing_action: None,
            protect_content: false,
            reply_keyboard: None,
            reply_to: None,
        };
        let frozen = vec![MessageId(2)];
        let ops = Differ::diff_with_frozen(&old, &screen, &[MessageId(10)], &frozen);
        // MessageId(2) should NOT be in any delete op
        for op in &ops {
            if let DiffOp::Delete { message_ids } = op {
                assert!(
                    !message_ids.contains(&MessageId(2)),
                    "frozen message should not be deleted"
                );
            }
        }
    }

    #[test]
    fn test_same_content_with_user_msg_still_resends() {
        // Even if content is identical, if user sent a message,
        // we delete+resend to keep message at bottom
        let content = text_content("hello");
        let old = vec![TrackedMessage::from_content(MessageId(1), &content)];
        let screen = Screen {
            id: ScreenId::from("test"),
            messages: vec![ScreenMessage { content }],
            input: None,
            typing_action: None,
            protect_content: false,
            reply_keyboard: None,
            reply_to: None,
        };
        let user_msgs = vec![MessageId(10)];
        let ops = Differ::diff(&old, &screen, &user_msgs);
        assert!(
            ops.iter().any(|op| matches!(op, DiffOp::Send { .. })),
            "should resend even if same content"
        );
    }
}
