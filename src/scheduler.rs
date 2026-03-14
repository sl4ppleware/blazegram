//! Delayed action scheduler — fire callbacks after a duration.
//!
//! Provides `SchedulerHandle` for enqueuing time-delayed operations like
//! auto-deleting temporary messages or firing synthetic callback queries.
//!
//! The scheduler runs as a background tokio task and spawns individual timers
//! for each scheduled action.

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::sleep;

use crate::types::*;

/// A handle to the background scheduler for enqueuing delayed actions.
#[derive(Clone)]
pub struct SchedulerHandle {
    tx: mpsc::UnboundedSender<ScheduledAction>,
}

/// An action to execute after a delay.
struct ScheduledAction {
    chat_id: ChatId,
    delay: Duration,
    kind: ScheduledKind,
}

/// What to do when the timer fires.
#[derive(Clone, Debug)]
pub(crate) enum ScheduledKind {
    /// Delete specific messages.
    DeleteMessages(Vec<MessageId>),
    /// Fire a synthetic callback (routed through the normal callback pipeline).
    Callback(String),
}

impl SchedulerHandle {
    /// Schedule message deletion after a delay.
    pub fn delete_later(&self, chat_id: ChatId, message_ids: Vec<MessageId>, delay: Duration) {
        let _ = self.tx.send(ScheduledAction {
            chat_id,
            delay,
            kind: ScheduledKind::DeleteMessages(message_ids),
        });
    }

    /// Schedule a synthetic callback to fire after a delay.
    /// The callback data is routed through the normal callback pipeline.
    pub fn callback_later(&self, chat_id: ChatId, data: String, delay: Duration) {
        let _ = self.tx.send(ScheduledAction {
            chat_id,
            delay,
            kind: ScheduledKind::Callback(data),
        });
    }
}

/// Spawn the scheduler background task. Returns a handle for enqueuing actions.
pub(crate) fn spawn_scheduler(
    bot_api: Arc<dyn crate::bot_api::BotApi>,
    callback_tx: mpsc::UnboundedSender<(ChatId, ScheduledKind)>,
) -> SchedulerHandle {
    let (tx, mut rx) = mpsc::unbounded_channel::<ScheduledAction>();

    tokio::spawn(async move {
        while let Some(action) = rx.recv().await {
            let bot = bot_api.clone();
            let cb_tx = callback_tx.clone();
            tokio::spawn(async move {
                sleep(action.delay).await;
                match action.kind {
                    ScheduledKind::DeleteMessages(ids) => {
                        let _ = bot.delete_messages(action.chat_id, ids).await;
                    }
                    ScheduledKind::Callback(data) => {
                        let _ = cb_tx.send((action.chat_id, ScheduledKind::Callback(data)));
                    }
                }
            });
        }
    });

    SchedulerHandle { tx }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scheduler_handle_is_clone() {
        // Compile-time test: SchedulerHandle must be Clone
        fn assert_clone<T: Clone>() {}
        assert_clone::<SchedulerHandle>();
    }

    #[tokio::test]
    async fn delete_later_sends_action() {
        let (cb_tx, _cb_rx) = mpsc::unbounded_channel();
        let bot: Arc<dyn crate::bot_api::BotApi> = Arc::new(crate::mock::MockBotApi::new());
        let handle = spawn_scheduler(bot.clone(), cb_tx);

        // Should not panic
        handle.delete_later(ChatId(1), vec![MessageId(1)], Duration::from_millis(10));
        // Let the timer fire
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    #[tokio::test]
    async fn callback_later_fires() {
        let (cb_tx, mut cb_rx) = mpsc::unbounded_channel();
        let bot: Arc<dyn crate::bot_api::BotApi> = Arc::new(crate::mock::MockBotApi::new());
        let handle = spawn_scheduler(bot, cb_tx);

        handle.callback_later(
            ChatId(42),
            "test:fire".to_string(),
            Duration::from_millis(10),
        );

        let (chat_id, kind) = tokio::time::timeout(Duration::from_secs(1), cb_rx.recv())
            .await
            .expect("timeout")
            .expect("channel closed");

        assert_eq!(chat_id, ChatId(42));
        assert!(matches!(kind, ScheduledKind::Callback(ref d) if d == "test:fire"));
    }
}
