//! Testing utilities — simulate bot interactions without Telegram.

use std::sync::Arc;

use crate::bot_api::BotApi;
use crate::ctx::Ctx;
use crate::error::{HandlerError, HandlerResult};
use crate::mock::MockBotApi;
use crate::router::Router;
use crate::serializer::ChatSerializer;
use crate::state::{InMemoryStore, StateStore};
use crate::types::*;

/// Test harness for Blazegram bots.
pub struct TestApp {
    /// The mock bot API instance.
    pub bot: Arc<MockBotApi>,
    /// The state store backing this test app.
    pub store: Arc<dyn StateStore>,
    /// The router with registered handlers.
    pub router: Arc<Router>,
    /// Per-chat serializer for sequential processing.
    pub serializer: Arc<ChatSerializer>,
}

impl TestApp {
    /// Open or create a redb database at the given path.
    pub fn new(router: Router) -> Self {
        let store: Arc<dyn StateStore> = Arc::new(InMemoryStore::new());
        let bot = Arc::new(MockBotApi::new());
        let serializer = Arc::new(ChatSerializer::new(store.clone()));
        Self {
            bot,
            store,
            router: Arc::new(router),
            serializer,
        }
    }

    /// Simulate a text message from a user.
    pub async fn send_message(&self, chat_id: i64, text: &str) -> HandlerResult {
        let msg_id = MessageId(self.bot.next_id());
        let update = IncomingUpdate {
            chat_id: ChatId(chat_id),
            user: test_user(),
            message_id: Some(msg_id),
            kind: UpdateKind::Message {
                text: Some(text.to_string()),
            },
        };
        self.process(update).await
    }

    /// Simulate a callback button press.
    pub async fn send_callback(&self, chat_id: i64, data: &str) -> HandlerResult {
        let update = IncomingUpdate {
            chat_id: ChatId(chat_id),
            user: test_user(),
            message_id: Some(MessageId(1)),
            kind: UpdateKind::CallbackQuery {
                id: format!("cb_{}", self.bot.next_id()),
                data: Some(data.to_string()),
                inline_message_id: None,
            },
        };
        self.process(update).await
    }

    /// Simulate a photo message.
    pub async fn send_photo(
        &self,
        chat_id: i64,
        file_id: &str,
        caption: Option<&str>,
    ) -> HandlerResult {
        let msg_id = MessageId(self.bot.next_id());
        let update = IncomingUpdate {
            chat_id: ChatId(chat_id),
            user: test_user(),
            message_id: Some(msg_id),
            kind: UpdateKind::Photo {
                file_id: file_id.to_string(),
                file_unique_id: file_id.to_string(),
                caption: caption.map(String::from),
            },
        };
        self.process(update).await
    }

    /// Simulate a document message.
    pub async fn send_document(
        &self,
        chat_id: i64,
        file_id: &str,
        filename: Option<&str>,
    ) -> HandlerResult {
        let msg_id = MessageId(self.bot.next_id());
        let update = IncomingUpdate {
            chat_id: ChatId(chat_id),
            user: test_user(),
            message_id: Some(msg_id),
            kind: UpdateKind::Document {
                file_id: file_id.to_string(),
                file_unique_id: file_id.to_string(),
                filename: filename.map(String::from),
                caption: None,
            },
        };
        self.process(update).await
    }

    async fn process(&self, incoming: IncomingUpdate) -> HandlerResult {
        let chat_id = incoming.chat_id;
        let user = incoming.user.clone();
        let router = self.router.clone();
        let bot = self.bot.clone();
        let incoming2 = incoming.clone();

        let (err_tx, err_rx) = tokio::sync::oneshot::channel::<Option<HandlerError>>();

        self.serializer
            .serialize(chat_id, &user, |state| {
                let router = router.clone();
                let bot: Arc<dyn BotApi> = bot.clone();
                let incoming = incoming2.clone();

                async move {
                    let cb_data = match &incoming.kind {
                        UpdateKind::CallbackQuery { data, .. } => data.clone(),
                        _ => None,
                    };
                    let mut ctx = Ctx::new(state, bot.clone(), cb_data);
                    match &incoming.kind {
                        UpdateKind::Message { .. } => {
                            ctx.message_text = match &incoming.kind {
                                UpdateKind::Message { text, .. } => text.clone(),
                                _ => None,
                            };
                            ctx.incoming_message_id = incoming.message_id;
                        }
                        UpdateKind::CallbackQuery { id, .. } => {
                            ctx.state.pending_callback_id = Some(id.clone());
                            ctx.incoming_message_id = incoming.message_id;
                        }
                        UpdateKind::Photo { .. } | UpdateKind::Document { .. } => {
                            ctx.incoming_message_id = incoming.message_id;
                        }
                        _ => {}
                    }
                    let result = router.route(&mut ctx, &incoming).await;
                    if let Some(cb_id) = ctx.state.pending_callback_id.take() {
                        let _ = bot.answer_callback_query(cb_id, None, false).await;
                    }
                    let _ = err_tx.send(result.err());
                    ctx.state
                }
            })
            .await;

        match err_rx.await {
            Ok(Some(e)) => Err(e),
            _ => Ok(()),
        }
    }

    /// Get all messages sent by the bot (async-safe).
    pub async fn sent_messages(&self) -> Vec<(ChatId, MessageContent)> {
        self.bot.sent_messages_async().await
    }

    /// Get the number of messages sent (async-safe).
    pub async fn sent_count(&self) -> usize {
        self.bot.call_count_async().await
    }

    /// Get current chat state.
    pub async fn state(&self, chat_id: i64) -> Option<ChatState> {
        self.store.load(ChatId(chat_id)).await
    }
}

fn test_user() -> UserInfo {
    UserInfo {
        id: UserId(12345),
        first_name: "Test".to_string(),
        last_name: None,
        username: Some("testuser".to_string()),
        language_code: Some("en".to_string()),
    }
}
