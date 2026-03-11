use thiserror::Error;

/// API-level errors returned by Telegram.
///
/// The executor retries [`TooManyRequests`](Self::TooManyRequests) automatically
/// after the specified cooldown. [`Network`](Self::Network) errors are also
/// considered retryable.
#[derive(Debug, Error)]
pub enum ApiError {
    /// The target message was already deleted or doesn’t exist.
    #[error("message not found (already deleted?)")]
    MessageNotFound,

    /// `editMessage*` was called but nothing actually changed.
    #[error("message is not modified")]
    MessageNotModified,

    /// Rate-limited by Telegram — retry after `retry_after` seconds.
    #[error("too many requests (retry after {retry_after}s)")]
    TooManyRequests {
        /// How many seconds to wait before retrying.
        retry_after: u32,
    },

    /// The chat does not exist or was deleted.
    #[error("chat not found")]
    ChatNotFound,

    /// The user has blocked the bot.
    #[error("bot was blocked by user")]
    BotBlocked,

    /// HTML entity offsets are out of bounds (usually a Telegram-side bug).
    #[error("invalid HTML entities (ENTITY_BOUNDS_INVALID)")]
    EntityBoundsInvalid,

    /// A "forbidden" response with a descriptive message.
    #[error("forbidden: {0}")]
    Forbidden(String),

    /// A network-level error (timeout, DNS, connection reset, etc.).
    #[error("network error: {0}")]
    Network(String),

    /// Any other Telegram error not covered above.
    #[error("{0}")]
    Unknown(String),
}

/// Handler-level errors that can occur during update processing.
///
/// Wraps [`ApiError`] and adds application-specific variants.
#[derive(Debug, Error)]
pub enum HandlerError {
    /// An API call to Telegram failed.
    #[error("api: {0}")]
    Api(#[from] ApiError),

    /// A user-defined error (validation failure, business logic, etc.).
    #[error("user error: {0}")]
    User(String),

    /// The handler did not complete within the configured timeout.
    #[error("handler timed out after {0:?}")]
    Timeout(std::time::Duration),

    /// An internal / unexpected error (wraps [`anyhow::Error`]).
    #[error("{0}")]
    Internal(#[from] anyhow::Error),
}

/// Convenience alias: `Result<(), HandlerError>`.
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
