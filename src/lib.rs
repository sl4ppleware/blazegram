#![doc = include_str!("../README.md")]
// All public items are documented as of 0.4.0.
#![warn(missing_docs)]
#![allow(clippy::too_many_arguments)]

// ─── Core types & errors ───

/// Error types for API calls and handler logic.
pub mod error;
/// Core types: ChatId, MessageId, Screen identifiers, message content, parsed updates.
pub mod types;

// ─── Screen system (the heart of Blazegram) ───

/// Virtual Chat Differ — computes minimal API operations for screen transitions.
pub mod differ;
/// Diff operation executor — applies edit/delete/send with retry and fallback.
pub mod executor;
/// Inline keyboard builder with buttons, rows, grids, navigation.
pub mod keyboard;
/// Markup processor: `*bold*` `_italic_` → Telegram HTML, plus `escape()` helper.
pub mod markup;
/// Screen builder — declarative UI for chat messages.
pub mod screen;

// ─── Runtime ───

/// grammers MTProto adapter implementing [`BotApi`](crate::bot_api::BotApi).
///
/// Split into sub-modules for send, media, admin, settings, forum, and stars operations.
pub mod adapter;
/// `BotApi` trait — 60+ async methods abstracting Telegram API calls.
pub mod bot_api;
/// Re-export for backward compatibility with pre-0.4 code that used
/// `blazegram::grammers_adapter::*`.
pub mod grammers_adapter {
    pub use crate::adapter::*;
}
/// App builder and main event loop.
pub mod app;
/// Handler context — the single object handlers interact with.
pub mod ctx;
/// File-backed session — MemorySession + JSON persistence. Zero SQLite.
pub mod file_session;
/// Command/callback/input router with prefix matching.
pub mod router;
/// Per-chat message tracking serializer (locks, state, tracked messages).
pub mod serializer;
/// State storage trait + in-memory store with snapshot support.
pub mod state;
/// Raw grammers Update → IncomingUpdate parser (isolates TL pattern matching).
pub(crate) mod update_parser;

// ─── Features ───

/// Broadcast messages to multiple chats.
pub mod broadcast;
/// Multi-step form wizards with validation.
pub mod form;
/// JSON-based i18n with `{ $var }` interpolation.
pub mod i18n;
/// Inline query results with builder API.
pub mod inline;
/// Paginated lists with auto-generated navigation buttons.
pub mod pagination;
/// Progressive screen updates (streaming, progress bars). Auto-cancelled on navigate().
pub mod progressive;
/// Template engine: `{{ var }}`, `{% if %}`, `{% for %}`.
pub mod template;

// ─── Infrastructure ───

/// File ID cache — avoid re-uploading the same media.
pub mod file_cache;
/// Prometheus-style counters and histograms.
pub mod metrics;
/// Middleware trait + built-in logging, analytics, throttle.
pub mod middleware;
/// Token-bucket rate limiter with automatic FLOOD_WAIT handling.
pub mod rate_limiter;
/// Pure-Rust persistent state store (redb). Zero C deps, no SQLite conflicts.
/// Requires the `redb` feature (enabled by default).
#[cfg(feature = "redb")]
pub mod redb_store;
/// Ergonomic handler macros (`handler!`, `form_handler!`).
#[macro_use]
pub mod macros;

// ─── Testing ───

/// Mock BotApi for unit tests without Telegram.
pub mod mock;
/// TestApp harness for integration-style tests.
pub mod testing;

// ─── Prelude ───

/// Re-exports everything you need: `use blazegram::prelude::*;`
pub mod prelude;

// ─── Optional stores ───

/// Redis-backed state store (requires `redis` feature).
#[cfg(feature = "redis")]
pub mod redis_store;
