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
    /// Create a new test harness with the given router.
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

    /// Simulate a voice message.
    pub async fn send_voice(&self, chat_id: i64, file_id: &str, duration: i32) -> HandlerResult {
        let msg_id = MessageId(self.bot.next_id());
        let update = IncomingUpdate {
            chat_id: ChatId(chat_id),
            user: test_user(),
            message_id: Some(msg_id),
            kind: UpdateKind::Voice {
                file_id: file_id.to_string(),
                file_unique_id: file_id.to_string(),
                duration,
                caption: None,
            },
        };
        self.process(update).await
    }

    /// Simulate a video message.
    pub async fn send_video(
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
            kind: UpdateKind::Video {
                file_id: file_id.to_string(),
                file_unique_id: file_id.to_string(),
                caption: caption.map(String::from),
            },
        };
        self.process(update).await
    }

    /// Simulate a sticker message.
    pub async fn send_sticker(&self, chat_id: i64, file_id: &str) -> HandlerResult {
        let msg_id = MessageId(self.bot.next_id());
        let update = IncomingUpdate {
            chat_id: ChatId(chat_id),
            user: test_user(),
            message_id: Some(msg_id),
            kind: UpdateKind::Sticker {
                file_id: file_id.to_string(),
                file_unique_id: file_id.to_string(),
            },
        };
        self.process(update).await
    }

    /// Simulate a location message.
    pub async fn send_location(
        &self,
        chat_id: i64,
        latitude: f64,
        longitude: f64,
    ) -> HandlerResult {
        let msg_id = MessageId(self.bot.next_id());
        let update = IncomingUpdate {
            chat_id: ChatId(chat_id),
            user: test_user(),
            message_id: Some(msg_id),
            kind: UpdateKind::LocationReceived {
                latitude,
                longitude,
            },
        };
        self.process(update).await
    }

    /// Simulate a contact message.
    pub async fn send_contact(&self, chat_id: i64, phone: &str, first_name: &str) -> HandlerResult {
        let msg_id = MessageId(self.bot.next_id());
        let update = IncomingUpdate {
            chat_id: ChatId(chat_id),
            user: test_user(),
            message_id: Some(msg_id),
            kind: UpdateKind::ContactReceived {
                contact: Contact {
                    phone_number: phone.to_string(),
                    first_name: first_name.to_string(),
                    last_name: None,
                    user_id: None,
                    vcard: None,
                },
            },
        };
        self.process(update).await
    }

    /// Simulate a member joining the chat.
    pub async fn simulate_member_joined(&self, chat_id: i64) -> HandlerResult {
        let update = IncomingUpdate {
            chat_id: ChatId(chat_id),
            user: test_user(),
            message_id: None,
            kind: UpdateKind::ChatMemberJoined,
        };
        self.process(update).await
    }

    /// Simulate a member leaving the chat.
    pub async fn simulate_member_left(&self, chat_id: i64) -> HandlerResult {
        let update = IncomingUpdate {
            chat_id: ChatId(chat_id),
            user: test_user(),
            message_id: None,
            kind: UpdateKind::ChatMemberLeft,
        };
        self.process(update).await
    }

    /// Simulate a pre-checkout query.
    pub async fn simulate_pre_checkout(
        &self,
        chat_id: i64,
        currency: &str,
        total_amount: i64,
        payload: &str,
    ) -> HandlerResult {
        let update = IncomingUpdate {
            chat_id: ChatId(chat_id),
            user: test_user(),
            message_id: None,
            kind: UpdateKind::PreCheckoutQuery {
                id: format!("pq_{}", self.bot.next_id()),
                currency: currency.to_string(),
                total_amount,
                payload: payload.to_string(),
            },
        };
        self.process(update).await
    }

    /// Simulate a successful payment.
    pub async fn simulate_successful_payment(
        &self,
        chat_id: i64,
        currency: &str,
        total_amount: i64,
        payload: &str,
    ) -> HandlerResult {
        let msg_id = MessageId(self.bot.next_id());
        let update = IncomingUpdate {
            chat_id: ChatId(chat_id),
            user: test_user(),
            message_id: Some(msg_id),
            kind: UpdateKind::SuccessfulPayment {
                currency: currency.to_string(),
                total_amount,
                payload: payload.to_string(),
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
                        UpdateKind::Photo { .. }
                        | UpdateKind::Document { .. }
                        | UpdateKind::Voice { .. }
                        | UpdateKind::Video { .. }
                        | UpdateKind::VideoNote { .. }
                        | UpdateKind::Sticker { .. }
                        | UpdateKind::ContactReceived { .. }
                        | UpdateKind::LocationReceived { .. } => {
                            ctx.incoming_message_id = incoming.message_id;
                        }
                        UpdateKind::PreCheckoutQuery {
                            id,
                            currency,
                            total_amount,
                            payload,
                        } => {
                            ctx.payment = crate::ctx::PaymentContext {
                                query_id: Some(id.clone()),
                                payload: Some(payload.clone()),
                                currency: Some(currency.clone()),
                                total_amount: Some(*total_amount),
                            };
                        }
                        UpdateKind::SuccessfulPayment {
                            currency,
                            total_amount,
                            payload,
                        } => {
                            ctx.payment = crate::ctx::PaymentContext {
                                query_id: None,
                                payload: Some(payload.clone()),
                                currency: Some(currency.clone()),
                                total_amount: Some(*total_amount),
                            };
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

    /// Get all deleted message IDs (async-safe).
    pub async fn deleted_messages(&self) -> Vec<(ChatId, Vec<MessageId>)> {
        self.bot.deleted_messages_async().await
    }

    /// Get all message edits (async-safe).
    pub async fn edits(&self) -> Vec<(ChatId, MessageId, String)> {
        self.bot.edits_async().await
    }

    /// Get all callback query answers (async-safe).
    pub async fn callback_answers(&self) -> Vec<(String, Option<String>, bool)> {
        self.bot.answers_async().await
    }

    /// Get current chat state.
    pub async fn state(&self, chat_id: i64) -> Option<ChatState> {
        self.store.load(ChatId(chat_id)).await.unwrap_or(None)
    }

    // ─── Assertion Helpers ───

    /// Assert that the bot navigated to a specific screen.
    pub async fn assert_screen(&self, chat_id: i64, screen_id: &str) {
        let state = self.state(chat_id).await.expect("no state for chat");
        assert_eq!(
            state.current_screen,
            ScreenId::from(screen_id.to_string()),
            "expected screen '{}', got '{}'",
            screen_id,
            state.current_screen
        );
    }

    /// Assert the last sent message contains the given text substring.
    pub async fn assert_sent_text(&self, substring: &str) {
        let msgs = self.sent_messages().await;
        let found = msgs.iter().any(|(_, content)| match content {
            MessageContent::Text { text, .. } => text.contains(substring),
            _ => false,
        });
        assert!(
            found,
            "no sent message contains '{}'. Messages: {:?}",
            substring,
            msgs.iter()
                .filter_map(|(_, c)| match c {
                    MessageContent::Text { text, .. } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
        );
    }

    /// Assert a specific number of messages were sent.
    pub async fn assert_sent_count(&self, count: usize) {
        let actual = self.sent_count().await;
        assert_eq!(
            actual, count,
            "expected {} sent messages, got {}",
            count, actual
        );
    }

    /// Assert no messages were sent.
    pub async fn assert_no_messages(&self) {
        let actual = self.sent_count().await;
        assert_eq!(actual, 0, "expected no messages, got {}", actual);
    }

    /// Assert messages were deleted.
    pub async fn assert_deleted(&self) {
        let deleted = self.deleted_messages().await;
        assert!(
            !deleted.is_empty(),
            "expected some messages to be deleted, none were"
        );
    }

    /// Get the current screen ID from the chat state.
    pub async fn current_screen(&self, chat_id: i64) -> String {
        let state = self.state(chat_id).await.expect("no state for chat");
        state.current_screen.to_string()
    }

    /// Simulate a scheduled callback firing (processes a synthetic callback update).
    pub async fn fire_scheduled_callback(&self, chat_id: i64, data: &str) -> HandlerResult {
        self.send_callback(chat_id, data).await
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ctx::Ctx;
    use crate::router::RouterGroup;
    use crate::screen::Screen;

    fn make_router() -> Router {
        let mut router = Router::new();

        router.command(
            "start",
            handler!(ctx => {
                ctx.navigate(Screen::text("home", "Welcome!")
                    .keyboard(|kb| kb.button("Count", "counter:0"))
                    .build()
                ).await?;
                Ok(())
            }),
        );

        router.callback(
            "counter",
            handler!(ctx => {
                let count: i32 = ctx.callback_param_as().unwrap_or(0);
                let next = count + 1;
                ctx.navigate(Screen::text("counter", format!("Count: {}", count))
                    .keyboard(|kb| kb.button("Next", format!("counter:{}", next)))
                    .build()
                ).await?;
                Ok(())
            }),
        );

        router.on_input(
            "input_screen",
            handler!(ctx, text => {
                ctx.set("last_input", &text);
                Ok(())
            }),
        );

        router.on_media_input(
            "media_screen",
            handler!(ctx, media => {
                ctx.set("last_media_type", &format!("{:?}", media.file_type));
                Ok(())
            }),
        );

        router.on_pre_checkout(handler!(ctx => {
            ctx.approve_checkout().await?;
            Ok(())
        }));

        router.on_successful_payment(handler!(ctx => {
            ctx.set("paid", &true);
            Ok(())
        }));

        router.on_member_joined(handler!(ctx => {
            ctx.set("member_joined", &true);
            Ok(())
        }));

        router.on_member_left(handler!(ctx => {
            ctx.set("member_left", &true);
            Ok(())
        }));

        router
    }

    /// Extract the text from a MessageContent::Text.
    fn text_of(content: &MessageContent) -> &str {
        match content {
            MessageContent::Text { text, .. } => text,
            _ => panic!("expected Text, got {:?}", content.content_type()),
        }
    }

    #[tokio::test]
    async fn test_command_routing() {
        let app = TestApp::new(make_router());
        app.send_message(1, "/start").await.unwrap();

        let msgs = app.sent_messages().await;
        assert!(!msgs.is_empty(), "should send a message");
        let (chat_id, content) = &msgs[0];
        assert_eq!(chat_id.0, 1);
        assert_eq!(text_of(content), "Welcome!");
    }

    #[tokio::test]
    async fn test_callback_routing() {
        let app = TestApp::new(make_router());
        app.send_message(1, "/start").await.unwrap();
        // Callback should not error
        let result = app.send_callback(1, "counter:0").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_callback_prefix_routing() {
        let app = TestApp::new(make_router());
        app.send_message(1, "/start").await.unwrap();
        // "counter:5" should match "counter" handler via prefix matching
        let result = app.send_callback(1, "counter:5").await;
        assert!(result.is_ok());

        // State should reflect the counter screen was navigated to
        let state = app.state(1).await.unwrap();
        assert_eq!(state.current_screen, ScreenId::from("counter"));
    }

    #[tokio::test]
    async fn test_member_joined() {
        let app = TestApp::new(make_router());
        app.simulate_member_joined(1).await.unwrap();

        let state = app.state(1).await.unwrap();
        assert_eq!(
            state.data.get("member_joined"),
            Some(&serde_json::json!(true))
        );
    }

    #[tokio::test]
    async fn test_member_left() {
        let app = TestApp::new(make_router());
        app.simulate_member_left(1).await.unwrap();

        let state = app.state(1).await.unwrap();
        assert_eq!(
            state.data.get("member_left"),
            Some(&serde_json::json!(true))
        );
    }

    #[tokio::test]
    async fn test_pre_checkout() {
        let app = TestApp::new(make_router());
        // approve_checkout calls answer_pre_checkout_query — should not error
        let result = app.simulate_pre_checkout(1, "XTR", 100, "order_123").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_successful_payment() {
        let app = TestApp::new(make_router());
        app.simulate_successful_payment(1, "USD", 999, "order_456")
            .await
            .unwrap();

        let state = app.state(1).await.unwrap();
        assert_eq!(state.data.get("paid"), Some(&serde_json::json!(true)));
    }

    #[tokio::test]
    async fn test_sticker_message() {
        let app = TestApp::new(make_router());
        let result = app.send_sticker(1, "sticker_abc").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_location_message() {
        let app = TestApp::new(make_router());
        let result = app.send_location(1, 55.7558, 37.6173).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_contact_message() {
        let app = TestApp::new(make_router());
        let result = app.send_contact(1, "+79001234567", "John").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_video_message() {
        let app = TestApp::new(make_router());
        let result = app.send_video(1, "video_abc", Some("hello")).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_voice_message() {
        let app = TestApp::new(make_router());
        let result = app.send_voice(1, "voice_abc", 5).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_state_persistence_across_updates() {
        let app = TestApp::new(make_router());

        app.send_message(1, "/start").await.unwrap();
        let state = app.state(1).await;
        assert!(state.is_some());

        app.send_callback(1, "counter:0").await.unwrap();
        let state = app.state(1).await.unwrap();
        assert_eq!(state.chat_id, ChatId(1));
    }

    // ─── Assertion helpers ───

    #[tokio::test]
    async fn test_assert_screen() {
        let app = TestApp::new(make_router());
        app.send_message(1, "/start").await.unwrap();
        app.assert_screen(1, "home").await;
    }

    #[tokio::test]
    async fn test_assert_sent_text() {
        let app = TestApp::new(make_router());
        app.send_message(1, "/start").await.unwrap();
        app.assert_sent_text("Welcome!").await;
    }

    #[tokio::test]
    async fn test_assert_sent_count() {
        let app = TestApp::new(make_router());
        app.send_message(1, "/start").await.unwrap();
        app.assert_sent_count(1).await;
    }

    #[tokio::test]
    async fn test_assert_no_messages() {
        let app = TestApp::new(make_router());
        app.assert_no_messages().await;
    }

    #[tokio::test]
    async fn test_current_screen() {
        let app = TestApp::new(make_router());
        app.send_message(1, "/start").await.unwrap();
        assert_eq!(app.current_screen(1).await, "home");
    }

    #[tokio::test]
    async fn test_fire_scheduled_callback() {
        let app = TestApp::new(make_router());
        // fire_scheduled_callback just delegates to send_callback
        app.send_message(1, "/start").await.unwrap();
        let result = app.fire_scheduled_callback(1, "counter:5").await;
        assert!(result.is_ok());
    }

    // ─── Router groups via TestApp ───

    #[tokio::test]
    async fn test_group_command_via_testapp() {
        let mut router = Router::new();
        router.group(RouterGroup::new().command("secret", |ctx: &mut Ctx| {
            Box::pin(async move {
                ctx.navigate(Screen::text("secret", "Secret page").build())
                    .await
            })
        }));
        router.command("start", |ctx: &mut Ctx| {
            Box::pin(async move { ctx.navigate(Screen::text("home", "Home").build()).await })
        });
        let app = TestApp::new(router);
        app.send_message(1, "/secret").await.unwrap();
        app.assert_screen(1, "secret").await;
        app.assert_sent_text("Secret page").await;
    }

    // ─── edit_last via TestApp ───

    #[tokio::test]
    async fn test_edit_last() {
        let mut router = Router::new();
        router.command("start", |ctx: &mut Ctx| {
            Box::pin(async move { ctx.navigate(Screen::text("home", "Hello").build()).await })
        });
        router.callback("update", |ctx: &mut Ctx| {
            Box::pin(async move {
                ctx.edit_last(Screen::text("home", "Updated!").build())
                    .await
            })
        });
        let app = TestApp::new(router);
        app.send_message(1, "/start").await.unwrap();
        app.send_callback(1, "update").await.unwrap();
        // edit should have happened
        let edits = app.edits().await;
        assert!(!edits.is_empty(), "should have at least one edit");
        assert!(edits.iter().any(|(_, _, text)| text == "Updated!"));
    }
}
