//! Prelude — import everything you need with `use blazegram::prelude::*`.

pub use crate::app::App;
pub use crate::bot_api::{BotApi, SendOptions};
pub use crate::broadcast::{BroadcastOptions, BroadcastResult, broadcast, broadcast_text};
pub use crate::ctx::{Ctx, PaymentContext};
pub use crate::error::{ApiError, HandlerError, HandlerResult};
pub use crate::file_cache::FileIdCache;
pub use crate::form::Form;
pub use crate::i18n::I18n;
pub use crate::inline::{InlineAnswer, InlineResult, InlineResultBuilder};
pub use crate::keyboard::{ButtonAction, InlineButton, InlineKeyboard, KeyboardBuilder};
pub use crate::markup;
pub use crate::metrics::{self, Metrics, metrics};
pub use crate::middleware::{
    AnalyticsMiddleware, AuthMiddleware, LoggingMiddleware, Middleware, ThrottleMiddleware,
};
pub use crate::mock::MockBotApi;
pub use crate::pagination::{Paginator, paginated_screen};
pub use crate::progressive::ProgressiveHandle;
pub use crate::rate_limiter::RateLimitedBotApi;
#[cfg(feature = "redb")]
pub use crate::redb_store::RedbStore;
pub use crate::screen::{ReplyButton, ReplyKeyboardAction, Screen, ScreenBuilder};
pub use crate::state::{InMemoryStore, StateStore};
pub use crate::template;
pub use crate::testing::TestApp;
pub use crate::types::*;

#[cfg(feature = "redis")]
pub use crate::redis_store::RedisStore;
