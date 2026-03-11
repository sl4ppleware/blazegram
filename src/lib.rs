#![doc = include_str!("../README.md")]
// #![warn(missing_docs)] // TODO: enable before 1.0
#![allow(clippy::too_many_arguments)]

// ─── Core types & errors ───

/// Core types: ChatId, MessageId, Screen identifiers, message content, parsed updates.
pub mod types;
/// Error types for API calls and handler logic.
pub mod error;

// ─── Screen system (the heart of Blazegram) ───

/// Screen builder — declarative UI for chat messages.
pub mod screen;
/// Inline keyboard builder with buttons, rows, grids, navigation.
pub mod keyboard;
/// Markup processor: `*bold*` `_italic_` → Telegram HTML, plus `escape()` helper.
pub mod markup;
/// Virtual Chat Differ — computes minimal API operations for screen transitions.
pub mod differ;
/// Diff operation executor — applies edit/delete/send with retry and fallback.
pub mod executor;

// ─── Runtime ───

/// BotApi trait — 60+ methods abstracting Telegram API calls.
pub mod bot_api;
/// grammers MTProto adapter implementing BotApi.
pub mod grammers_adapter;
/// Handler context — the single object handlers interact with.
pub mod ctx;
/// Command/callback/input router with prefix matching.
pub mod router;
/// Per-chat message tracking serializer (locks, state, tracked messages).
pub mod serializer;
/// State storage trait + in-memory store with snapshot support.
pub mod state;
/// App builder and main event loop.
pub mod app;

// ─── Features ───

/// Multi-step form wizards with validation.
pub mod form;
/// Paginated lists with auto-generated navigation buttons.
pub mod pagination;
/// Inline query results with builder API.
pub mod inline;
/// Progressive screen updates (streaming, progress bars). Auto-cancelled on navigate().
pub mod progressive;
/// Broadcast messages to multiple chats.
pub mod broadcast;
/// Template engine: `{{ var }}`, `{% if %}`, `{% for %}`.
pub mod template;
/// JSON-based i18n with `{ $var }` interpolation.
pub mod i18n;

// ─── Infrastructure ───

/// Middleware trait + built-in logging, analytics, throttle.
pub mod middleware;
/// Token-bucket rate limiter with automatic FLOOD_WAIT handling.
pub mod rate_limiter;
/// Prometheus-style counters and histograms.
pub mod metrics;
/// File ID cache — avoid re-uploading the same media.
pub mod file_cache;
/// SQLite-backed persistent state store.
pub mod sqlite_store;

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
