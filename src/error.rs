use thiserror::Error;

/// API-level errors from Telegram.
#[derive(Debug, Error)]
pub enum ApiError {
    #[error("message not found (already deleted?)")]
    MessageNotFound,

    #[error("message is not modified")]
    MessageNotModified,

    #[error("too many requests (retry after {retry_after}s)")]
    TooManyRequests { retry_after: u32 },

    #[error("chat not found")]
    ChatNotFound,

    #[error("bot was blocked by user")]
    BotBlocked,

    #[error("invalid HTML entities (ENTITY_BOUNDS_INVALID)")]
    EntityBoundsInvalid,

    #[error("forbidden: {0}")]
    Forbidden(String),

    #[error("network error: {0}")]
    Network(String),

    #[error("{0}")]
    Unknown(String),
}

/// Handler-level errors.
#[derive(Debug, Error)]
pub enum HandlerError {
    #[error("api: {0}")]
    Api(#[from] ApiError),

    #[error("user error: {0}")]
    User(String),

    #[error("{0}")]
    Internal(#[from] anyhow::Error),
}

pub type HandlerResult = Result<(), HandlerError>;

impl ApiError {
    /// Whether the error is transient and the operation could succeed on retry.
    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::TooManyRequests { .. } | Self::Network(_))
    }

    /// Whether the error means we should stop trying to contact this chat entirely
    /// (user blocked the bot, account deleted, etc).
    pub fn is_fatal_for_chat(&self) -> bool {
        matches!(self, Self::BotBlocked | Self::ChatNotFound)
    }
}

impl HandlerError {
    /// Whether the inner error is a chat-fatal condition (blocked, deleted, etc).
    pub fn is_fatal_for_chat(&self) -> bool {
        matches!(self, Self::Api(e) if e.is_fatal_for_chat())
    }
}
