//! Progressive Screens — stream content updates to a message in real-time.
//!
//! Used for LLM streaming, progress indicators, and any live-updating content.
//! Auto-throttles edits to respect Telegram rate limits.
//!
//! # Example
//!
//! ```ignore
//! let handle = start_progressive(bot.clone(), chat_id, initial_screen).await?;
//! for token in stream {
//!     accumulated.push_str(&token);
//!     handle.update(make_screen(&accumulated)).await;
//! }
//! handle.finalize(make_screen(&accumulated)).await?;
//! ```

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{mpsc, oneshot};
use tracing;

use crate::bot_api::BotApi;
use crate::error::{ApiError, HandlerError, HandlerResult};
use crate::screen::Screen;
use crate::types::*;

// ─── Edit Target ───

/// Identifies which message to edit.
#[derive(Clone, Debug)]
pub enum EditTarget {
    /// Regular message in a chat.
    Chat {
        /// The chat containing the message.
        chat_id: ChatId,
        /// The message to progressively update.
        message_id: MessageId,
    },
    /// Inline message (sent via inline mode).
    Inline {
        /// The packed inline message ID.
        inline_message_id: String,
    },
}

// ─── Editor function type ───

/// A boxed async function that edits a message with the given screen content.
pub type EditorFn =
    Arc<dyn Fn(Screen) -> Pin<Box<dyn Future<Output = Result<(), ApiError>> + Send>> + Send + Sync>;

// ─── Default intervals ───

/// Default minimum interval between edits for regular chat messages.
const DEFAULT_CHAT_INTERVAL: Duration = Duration::from_millis(1500);

/// Default minimum interval between edits for inline messages.
const DEFAULT_INLINE_INTERVAL: Duration = Duration::from_millis(3000);

// ─── Progressive Commands ───

enum ProgressiveCmd {
    /// Intermediate update — may be coalesced/dropped if we're rate-limited.
    Update(Screen),
    /// Final update — always delivered. The oneshot signals completion.
    Finalize(Screen, oneshot::Sender<Result<(), HandlerError>>),
}

// ─── Progressive Handle ───

/// Handle for progressively updating a message.
///
/// The handle sends updates to a background task that auto-throttles edits.
/// Intermediate updates are coalesced — only the latest state is sent.
/// Dropping the handle without calling [`finalize`](Self::finalize) will
/// cancel the background task (the message keeps its last-edited content).
pub struct ProgressiveHandle {
    tx: mpsc::UnboundedSender<ProgressiveCmd>,
    _task: tokio::task::JoinHandle<()>,
    abort_handle: tokio::task::AbortHandle,
}

impl ProgressiveHandle {
    /// Get an abort handle that can cancel this progressive task.
    /// Used internally by `Ctx::navigate()` to prevent concurrent edits.
    pub fn abort_handle(&self) -> tokio::task::AbortHandle {
        self.abort_handle.clone()
    }

    /// Update the message content. Auto-throttled.
    ///
    /// If called faster than the rate limit, intermediate updates are skipped —
    /// only the latest screen will be sent on the next edit cycle.
    pub async fn update(&self, screen: Screen) {
        // Fire-and-forget: if the receiver is gone we silently drop.
        let _ = self.tx.send(ProgressiveCmd::Update(screen));
    }

    /// Send the final version. Always delivered (waits for rate limit if needed).
    ///
    /// Returns an error only if the final edit itself fails with a non-recoverable
    /// error. `MessageNotModified` is treated as success.
    pub async fn finalize(self, screen: Screen) -> HandlerResult {
        let (done_tx, done_rx) = oneshot::channel();
        // Send the finalize command. If the background task is already gone,
        // we get a send error — treat it as success (message keeps last state).
        if self
            .tx
            .send(ProgressiveCmd::Finalize(screen, done_tx))
            .is_err()
        {
            return Ok(());
        }
        // Wait for the background task to confirm delivery.
        match done_rx.await {
            Ok(result) => result,
            Err(_) => Ok(()), // task dropped — nothing more to do
        }
    }
}

// ─── Background task ───

/// Runs the throttle loop. Receives commands from the handle, coalesces
/// intermediate updates, and calls the editor at most once per `min_interval`.
async fn progressive_task(
    mut rx: mpsc::UnboundedReceiver<ProgressiveCmd>,
    editor: EditorFn,
    min_interval: Duration,
) {
    let mut last_edit = Instant::now() - min_interval; // allow immediate first edit
    let mut pending: Option<Screen> = None;

    loop {
        // If we have a pending update but can't send yet, wait with a timeout.
        let cmd = if pending.is_some() {
            let elapsed = last_edit.elapsed();
            if elapsed < min_interval {
                let remaining = min_interval - elapsed;
                tokio::select! {
                    cmd = rx.recv() => cmd,
                    _ = tokio::time::sleep(remaining) => {
                        // Timer fired — flush the pending update.
                        if let Some(screen) = pending.take() {
                            do_edit(&editor, screen, &mut last_edit).await;
                        }
                        continue;
                    }
                }
            } else {
                // We can flush immediately before waiting for the next command.
                if let Some(screen) = pending.take() {
                    do_edit(&editor, screen, &mut last_edit).await;
                }
                rx.recv().await
            }
        } else {
            rx.recv().await
        };

        match cmd {
            None => {
                // Channel closed (handle dropped). Flush any pending update.
                if let Some(screen) = pending.take() {
                    do_edit(&editor, screen, &mut last_edit).await;
                }
                return;
            }
            Some(ProgressiveCmd::Update(screen)) => {
                // Can we send right now?
                let elapsed = last_edit.elapsed();
                if elapsed >= min_interval {
                    do_edit(&editor, screen, &mut last_edit).await;
                } else {
                    // Coalesce: store as pending, replacing any previous.
                    pending = Some(screen);
                }
            }
            Some(ProgressiveCmd::Finalize(screen, done_tx)) => {
                // Drop any pending intermediate — the final version supersedes it.
                let _ = pending.take();

                // Wait for rate limit if needed.
                let elapsed = last_edit.elapsed();
                if elapsed < min_interval {
                    tokio::time::sleep(min_interval - elapsed).await;
                }

                let result = do_edit_result(&editor, screen).await;
                let _ = done_tx.send(result);
                return;
            }
        }
    }
}

/// Perform an edit, handling recoverable errors silently.
async fn do_edit(editor: &EditorFn, screen: Screen, last_edit: &mut Instant) {
    match editor(screen).await {
        Ok(()) => {
            *last_edit = Instant::now();
        }
        Err(ApiError::MessageNotModified) => {
            // Content hasn't changed — no problem, update the timestamp.
            *last_edit = Instant::now();
        }
        Err(ApiError::TooManyRequests { retry_after }) => {
            tracing::warn!("progressive edit rate-limited, waiting {}s", retry_after);
            tokio::time::sleep(Duration::from_secs(retry_after as u64)).await;
            *last_edit = Instant::now();
        }
        Err(e) => {
            tracing::error!("progressive edit failed: {}", e);
            *last_edit = Instant::now();
        }
    }
}

/// Perform an edit and return the result (for finalize). Retries once on FLOOD_WAIT.
async fn do_edit_result(editor: &EditorFn, screen: Screen) -> HandlerResult {
    match editor(screen.clone()).await {
        Ok(()) => Ok(()),
        Err(ApiError::MessageNotModified) => Ok(()),
        Err(ApiError::TooManyRequests { retry_after }) => {
            tracing::warn!(
                "progressive finalize rate-limited, waiting {}s then retrying",
                retry_after
            );
            tokio::time::sleep(Duration::from_secs(retry_after.min(30) as u64)).await;
            // Retry the edit after waiting
            match editor(screen).await {
                Ok(()) => Ok(()),
                Err(ApiError::MessageNotModified) => Ok(()),
                Err(e) => Err(HandlerError::Api(e)),
            }
        }
        Err(e) => Err(HandlerError::Api(e)),
    }
}

// ─── Constructors ───

/// Create a progressive handle for a regular chat message.
///
/// Sends the initial screen immediately as a new message, then returns a handle
/// for streaming updates to that message.
///
/// The initial screen's **first** message content is sent. Multi-message screens
/// are not supported for progressive updates (only the first message is used).
pub async fn start_progressive(
    bot: Arc<dyn BotApi>,
    chat_id: ChatId,
    initial: Screen,
) -> Result<ProgressiveHandle, ApiError> {
    // Send the initial message.
    let first_content = initial
        .messages
        .into_iter()
        .next()
        .map(|m| m.content)
        .unwrap_or_else(|| MessageContent::Text {
            text: "…".to_string(),
            parse_mode: ParseMode::Html,
            keyboard: None,
            link_preview: LinkPreview::Disabled,
        });

    let sent = bot
        .send_message(
            chat_id,
            first_content,
            crate::bot_api::SendOptions::default(),
        )
        .await?;

    let message_id = sent.message_id;
    let target = EditTarget::Chat {
        chat_id,
        message_id,
    };

    let editor = make_bot_editor(bot, target);
    Ok(spawn_progressive(editor, DEFAULT_CHAT_INTERVAL))
}

/// Create a progressive handle for an already-sent inline message.
///
/// Since inline messages don't go through normal `send_message`, the caller is
/// responsible for providing the inline_message_id. Uses a custom editor closure.
pub fn start_progressive_inline(editor: EditorFn) -> ProgressiveHandle {
    spawn_progressive(editor, DEFAULT_INLINE_INTERVAL)
}

/// Create a progressive handle with a custom editor function and interval.
///
/// This is the most flexible constructor — the caller provides the edit logic.
/// Useful for inline messages, or any custom editing strategy.
pub fn start_progressive_with_editor(
    editor: EditorFn,
    min_interval: Duration,
) -> ProgressiveHandle {
    spawn_progressive(editor, min_interval)
}

/// Build an [`EditorFn`] from a `BotApi` + `EditTarget`.
///
/// Extracts text, parse_mode, keyboard, and link_preview from the screen's first
/// message and calls the appropriate `BotApi` edit method.
fn make_bot_editor(bot: Arc<dyn BotApi>, target: EditTarget) -> EditorFn {
    Arc::new(move |screen: Screen| {
        let bot = bot.clone();
        let target = target.clone();
        Box::pin(async move {
            let first = screen
                .messages
                .into_iter()
                .next()
                .map(|m| m.content)
                .unwrap_or_else(|| MessageContent::Text {
                    text: "…".to_string(),
                    parse_mode: ParseMode::Html,
                    keyboard: None,
                    link_preview: LinkPreview::Disabled,
                });

            match target {
                EditTarget::Chat {
                    chat_id,
                    message_id,
                } => edit_chat_message(&*bot, chat_id, message_id, first).await,
                EditTarget::Inline {
                    inline_message_id: _,
                } => {
                    // NOTE: Inline progressive editing is not supported via the generic
                    // BotApi trait — it would require `edit_inline_message_text` which
                    // grammers handles differently. Use `start_progressive_with_editor`
                    // and provide a custom editor closure for inline messages.
                    tracing::warn!(
                        "EditTarget::Inline not yet supported via make_bot_editor; \
                         use start_progressive_with_editor instead"
                    );
                    Ok(())
                }
            }
        })
    })
}

/// Edit a regular chat message based on its content type.
async fn edit_chat_message(
    bot: &dyn BotApi,
    chat_id: ChatId,
    message_id: MessageId,
    content: MessageContent,
) -> Result<(), ApiError> {
    match content {
        MessageContent::Text {
            text,
            parse_mode,
            keyboard,
            link_preview,
        } => {
            bot.edit_message_text(
                chat_id,
                message_id,
                text,
                parse_mode,
                keyboard,
                link_preview == LinkPreview::Enabled,
            )
            .await
        }
        MessageContent::Photo { ref keyboard, .. }
        | MessageContent::Video { ref keyboard, .. }
        | MessageContent::Animation { ref keyboard, .. }
        | MessageContent::Document { ref keyboard, .. } => {
            // For media messages, use edit_message_media.
            let kb = keyboard.clone();
            bot.edit_message_media(chat_id, message_id, content_with_no_keyboard(content), kb)
                .await
        }
        other => {
            tracing::warn!(
                "progressive edit: unsupported content type {:?}, skipping",
                other.content_type()
            );
            Ok(())
        }
    }
}

/// Strip the keyboard from content so it can be passed separately to edit_message_media.
fn content_with_no_keyboard(content: MessageContent) -> MessageContent {
    match content {
        MessageContent::Photo {
            source,
            caption,
            parse_mode,
            spoiler,
            ..
        } => MessageContent::Photo {
            source,
            caption,
            parse_mode,
            keyboard: None,
            spoiler,
        },
        MessageContent::Video {
            source,
            caption,
            parse_mode,
            spoiler,
            ..
        } => MessageContent::Video {
            source,
            caption,
            parse_mode,
            keyboard: None,
            spoiler,
        },
        MessageContent::Animation {
            source,
            caption,
            parse_mode,
            spoiler,
            ..
        } => MessageContent::Animation {
            source,
            caption,
            parse_mode,
            keyboard: None,
            spoiler,
        },
        MessageContent::Document {
            source,
            caption,
            parse_mode,
            filename,
            ..
        } => MessageContent::Document {
            source,
            caption,
            parse_mode,
            keyboard: None,
            filename,
        },
        other => other,
    }
}

/// Spawn the background task and return a handle.
fn spawn_progressive(editor: EditorFn, min_interval: Duration) -> ProgressiveHandle {
    let (tx, rx) = mpsc::unbounded_channel();
    let task = tokio::spawn(progressive_task(rx, editor, min_interval));
    let abort_handle = task.abort_handle();
    ProgressiveHandle {
        tx,
        _task: task,
        abort_handle,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screen::Screen;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn dummy_screen(text: &str) -> Screen {
        Screen::text("test", text).build()
    }

    #[tokio::test]
    async fn finalize_always_delivers() {
        let call_count = Arc::new(AtomicUsize::new(0));
        let last_text = Arc::new(tokio::sync::Mutex::new(String::new()));

        let cc = call_count.clone();
        let lt = last_text.clone();
        let editor: EditorFn = Arc::new(move |screen: Screen| {
            let cc = cc.clone();
            let lt = lt.clone();
            Box::pin(async move {
                cc.fetch_add(1, Ordering::SeqCst);
                if let Some(msg) = screen.messages.first() {
                    if let MessageContent::Text { text, .. } = &msg.content {
                        *lt.lock().await = text.clone();
                    }
                }
                Ok(())
            })
        });

        let handle = start_progressive_with_editor(editor, Duration::from_millis(50));

        // Send a bunch of rapid updates — most should be coalesced.
        for i in 0..10 {
            handle.update(dummy_screen(&format!("update {}", i))).await;
        }

        // Finalize should always deliver.
        let result = handle.finalize(dummy_screen("final")).await;
        assert!(result.is_ok());

        let text = last_text.lock().await.clone();
        assert_eq!(text, "final");

        // We should have fewer than 10+1 edits thanks to coalescing.
        let count = call_count.load(Ordering::SeqCst);
        assert!(count >= 1, "at least the finalize should be delivered");
        assert!(count <= 11, "should not exceed total updates + finalize");
    }

    #[tokio::test]
    async fn coalesces_rapid_updates() {
        let call_count = Arc::new(AtomicUsize::new(0));

        let cc = call_count.clone();
        let editor: EditorFn = Arc::new(move |_screen: Screen| {
            let cc = cc.clone();
            Box::pin(async move {
                cc.fetch_add(1, Ordering::SeqCst);
                // Simulate a slow edit.
                tokio::time::sleep(Duration::from_millis(10)).await;
                Ok(())
            })
        });

        let handle = start_progressive_with_editor(editor, Duration::from_millis(100));

        // Fire 50 rapid updates.
        for i in 0..50 {
            handle.update(dummy_screen(&format!("u{}", i))).await;
        }

        // Wait for coalescing to settle.
        tokio::time::sleep(Duration::from_millis(500)).await;

        handle.finalize(dummy_screen("done")).await.unwrap();

        let count = call_count.load(Ordering::SeqCst);
        // With 100ms interval and 50 rapid-fire updates, we should see
        // significantly fewer than 50 actual edits.
        assert!(
            count < 50,
            "expected coalescing to reduce edits, got {}",
            count
        );
    }

    #[tokio::test]
    async fn handles_message_not_modified() {
        let editor: EditorFn =
            Arc::new(|_screen: Screen| Box::pin(async move { Err(ApiError::MessageNotModified) }));

        let handle = start_progressive_with_editor(editor, Duration::from_millis(10));
        handle.update(dummy_screen("same")).await;
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Finalize should still succeed even though edits return MessageNotModified.
        let result = handle.finalize(dummy_screen("same")).await;
        assert!(result.is_ok());
    }
}
