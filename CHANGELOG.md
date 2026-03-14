# Changelog

## [0.4.2] — 2026-03-14

### Added

- **State error handling**: `StateStore` trait methods now return `Result` instead of
  silently swallowing errors. `InMemoryStore`, `RedbStore`, and `RedisStore` all updated.
  `ChatSerializer` logs errors on load (falls back to fresh state) and save.
  New `HandlerError::State` variant for state-related failures.

- **`ctx.edit_last()`**: Edit the last bot message in place without running the differ.
  Lighter than `navigate()` — no diff, no delete, just one edit call.

- **Router groups**: `RouterGroup` allows organizing handlers into logical modules with
  per-group middleware. Groups are checked before the main router. Added `Router::group()`
  and `AppBuilder::group()`.

- **Scheduler**: `SchedulerHandle` for time-delayed actions — auto-delete messages and
  fire synthetic callbacks after a duration. `ctx.delete_later()` and
  `ctx.schedule_callback()` methods. `notify_temp()` now uses the scheduler when available.
  Scheduled callbacks are routed through the normal callback pipeline.

- **Conversation system**: Branching multi-step dialogues with `Conversation::builder()`.
  Supports sequential steps, conditional branching (`branch()`), unconditional jumps
  (`goto()`), end markers (`end_at()`), custom input validators per step, and cancel
  handling. Dispatched before forms in the update pipeline. `ctx.start_conversation()`.

- **TestApp assertions**: `assert_screen()`, `assert_sent_text()`, `assert_sent_count()`,
  `assert_no_messages()`, `assert_deleted()`, `current_screen()`, `fire_scheduled_callback()`.

- **Observability**: `process_update` tracing span now includes `user_id` field.

- **Examples**: 3 new examples:
  - `admin_bot.rs` — router groups with auth middleware
  - `scheduled_bot.rs` — scheduled messages & callbacks
  - `conversation_bot.rs` — branching conversations

- **Prelude**: Added `Conversation`, `ConversationBuilder`, `ConversationData`, `FormData`,
  `RouterGroup`, `SchedulerHandle` to prelude.

### Changed

- `StateStore::load()` returns `Result<Option<ChatState>, String>` (was `Option<ChatState>`).
- `StateStore::save()` returns `Result<(), String>` (was `()`).
- `StateStore::delete()` returns `Result<(), String>` (was `()`).
- `StateStore::all_chat_ids()` returns `Result<Vec<ChatId>, String>` (was `Vec<ChatId>`).
- `RedbStore`: all methods now propagate errors instead of `let _ =` or `.ok()`.
- `RedisStore`: all methods now propagate errors instead of silent drops.
- `ChatSerializer::serialize()`: handles load/save Results with proper tracing.
- `broadcast()`: handles `all_chat_ids()` Result.

## [0.4.1] — 2026-03-12

### Added

- **adapter**: 15 previously unimplemented BotApi methods now implemented:
  `send_media_group`, `send_invoice`, `create_invoice_link`, `answer_shipping_query`,
  `get_chat_member`, `get_chat`, `set_chat_permissions`, `set_chat_photo`,
  `unpin_all_chat_messages`, `create_chat_invite_link`, `revoke_chat_invite_link`.
  Forum convenience methods (`close_forum_topic`, `reopen_forum_topic`,
  `hide_general_forum_topic`, `unhide_general_forum_topic`) delegate to `edit_forum_topic`.
- **types**: `types.rs` (1 704 LOC) split into `types/` module: `mod.rs` (IDs, hasher,
  InlineQueryResult), `content.rs` (MessageContent, ContentType, FileSource),
  `chat.rs` (ChatState, UserInfo, TrackedMessage, CtxMode), `update.rs` (IncomingUpdate,
  UpdateKind, ReceivedMedia), `telegram.rs` (all remaining Telegram API types).
- **inline**: `InlineResultKind` unified — single type shared between `types::InlineResultKind`
  and `inline::InlineResult`. Removed duplicate enum and `From` mapping. Field names
  simplified (`photo_url` → `url`, `mime_type` → `mime`).
- **tests**: 41 new unit tests for previously untested core modules:
  - `ctx.rs`: state get/set/remove, callback parsing, freeze/unfreeze, lang, max_state_keys.
  - `executor.rs`: send, delete, edit text/keyboard, empty ops, multiple ops.
  - `screen.rs`: text, photo, video, document, multi-message, reply keyboard, input builders.
  - `form.rs`: builder API, all field parsers (text, integer, choice), cancel handler.
  Total test count: 109 → 150.

### Fixed

- **update_parser**: Parse all media types from MTProto (voice, video, video note,
  sticker, contact, location, live location). Previously these UpdateKind variants
  were defined but never produced — all media handlers were dead code.
- **update_parser**: Parse service messages for `ChatMemberJoined` (ChatAddUser,
  ChatJoinedByLink, ChatJoinedByRequest) and `ChatMemberLeft` (ChatDeleteUser).
- **update_parser**: Parse `SuccessfulPayment` from `MessageActionPaymentSentMe`.
- **update_parser**: Parse `PreCheckoutQuery` from raw `UpdateBotPrecheckoutQuery`.
- **rate_limiter**: `RateLimitedBotApi` now proxies all 73 BotApi trait methods.
  Previously 30 methods fell through to the trait default returning "not implemented",
  silently breaking the API when rate limiting was enabled.
- **mock**: `MockBotApi` now implements all non-convenience BotApi methods (12 added:
  send_media_group, stop_poll, send_invoice, restrict/promote/get_chat_member, get_chat,
  set_chat_permissions, unpin_all_chat_messages, create/export_chat_invite_link,
  set_chat_photo).
- **adapter**: `restrict_chat_member` — maps `ChatPermissions` to `ChatBannedRights`.
- **adapter**: `promote_chat_member` — maps `ChatPermissions` to `ChatAdminRights`.
- **testing**: `TestApp` expanded with simulation methods for voice, video, sticker,
  location, contact, member joined/left, pre-checkout, and successful payment.
- **testing**: 13 new integration tests covering command routing, callback routing with
  prefix matching, member events, payment flows, and all media input types.

## [0.4.0] — 2026-03-11

### Added

- `handler!` / `form_handler!` macros — no more `Box::pin(async move { … })` boilerplate.
- `FileSession` — JSON-based session storage replacing SQLite. Zero C dependencies in the tree.
- `redb` state backend (default feature) — pure-Rust ACID store replaces SQLite for chat state.
- `UpdateEnvelope` / `UpdateKind` — structured update parsing in a dedicated `update_parser` module.
- `PaymentContext` — extracted from `Ctx`.
- `Timeout` error variant in `HandlerError`.
- `delete_unrecognized(bool)` — control auto-deletion of unrecognized messages (default: `true`).
- `max_state_keys(usize)` — cap per-chat state keys (default: 1 000) with automatic eviction.
- `ctx.reply_message_id()` — accessor for the last `reply()` message.
- Safety limits: tracked messages capped at 100/chat, oldest evicted.
- Benchmarks for differ, screen building, content hashing, serialization.
- GitHub Actions CI (MSRV 1.85 + stable, test, clippy, doc, format).
- `#![warn(missing_docs)]` — all public items documented.

### Changed

- Edition 2024 (Rust 1.85+).
- Serialization: `bincode` → `postcard`.
- `grammers_adapter` (1 569 LOC) split into `adapter/` module tree.
- `reply()` messages are now tracked by the differ, so `navigate()` cleans them up.
- `unwrap()` → `expect()` in all production code paths.

### Removed

- `rusqlite` — no SQLite anywhere in the dependency tree.
- `bincode` — replaced by `postcard`.

## [0.3.1] — 2026-03-11

- README formatting fix.

## [0.3.0] — 2026-03-10

- Initial public release.

[0.4.2]: https://github.com/sl4ppleware/blazegram/compare/v0.4.1...v0.4.2
[0.4.1]: https://github.com/sl4ppleware/blazegram/compare/v0.4.0...v0.4.1
[0.4.0]: https://github.com/sl4ppleware/blazegram/compare/v0.3.1...v0.4.0
[0.3.1]: https://github.com/sl4ppleware/blazegram/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/sl4ppleware/blazegram/releases/tag/v0.3.0
