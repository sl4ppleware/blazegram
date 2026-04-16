# Changelog

## [0.4.2] — 2026-04-16

### Added

- **BotApi: 130+ new methods** — full Telegram Bot API 9.6 coverage:
  - Convenience media senders (`send_photo`, `send_audio`, `send_document`, `send_video`,
    `send_animation`, `send_voice`, `send_video_note`)
  - Paid media, live location, checklists, message drafts
  - Stories (`post_story`, `edit_story`, `delete_story`)
  - Emoji status, user/chat verification
  - Business account management (name, username, bio, photos, gifts, star balance, transfers)
  - Gift operations (send, premium subscription, convert, upgrade, transfer)
  - Sticker set management (get, create, add, replace, delete, set emoji/keywords/mask/title/thumbnail)
  - Game operations (send, set score, get high scores)
  - Inline message operations (`answer_web_app_query`, `save_prepared_inline_message`)
  - Managed bot tokens, prepared keyboard, passport data errors
  - Chat member tags, sender chat ban/unban, subscription invite links
  - Default admin rights, profile photo management
  - General forum topic operations, `log_out`, `close`
- **Types**: `PaidMediaInput`, `ChecklistItem`, `LiveLocationOptions`, `StoryContent`,
  `StoryAreas`, `InputSticker`, `MaskPosition`, `PreparedInlineMessage`, `SentWebAppMessage`,
  `GameHighScore`, `PassportElementError`, `AcceptedGiftTypes`, `OwnedGift`, `Gift`,
  `BusinessConnection`, and more.
- **MockBotApi**: stubs for all new methods.
- **Router groups**: `RouterGroup` for organizing handlers into modules with per-group middleware.
- **Scheduler**: `SchedulerHandle` for delayed actions — auto-delete messages and fire
  synthetic callbacks. `ctx.delete_later()`, `ctx.schedule_callback()`, `notify_temp()`.
- **Conversation system**: branching multi-step dialogues with `Conversation::builder()`.
  Conditional branching, unconditional jumps, end markers, custom validators, cancel handling.
- **`ctx.edit_last()`**: edit last bot message without running the differ.
- **State error handling**: `StateStore` methods return `Result`. New `HandlerError::State`.
- **TestApp**: `assert_screen()`, `assert_sent_text()`, `assert_sent_count()`,
  `assert_no_messages()`, `fire_scheduled_callback()`, media/payment/member event simulation.
- **Examples**: `admin_bot.rs`, `scheduled_bot.rs`, `conversation_bot.rs`.

### Fixed

- **rate_limiter**: replace 10ms busy-loop with `Notify`-based wake in `GlobalLimiter`.
  Remove dead `Semaphore` (created but never used for backpressure).
- **rate_limiter**: unify `effective_rps` into single `Arc<AtomicU32>` shared between
  `GlobalLimiter` and `RateLimiterMetrics` — fixes metrics desync race.
- **rate_limiter**: add `gc_idle_buckets()` — evict per-chat token buckets idle >10 min.
  Previously `DashMap<i64, ChatBucket>` grew unbounded (memory leak).
- **serializer**: fix potential deadlock in `gc()` — `DashMap::retain` holds shard lock
  conflicting with concurrent `serialize()` calls. Now two-phase: collect then `remove_if`.
- **app**: fix double push to `pending_user_messages` when form/conversation cancels via
  /command and falls through to router. Form/conv now push only for messages they handle.
- **executor**: chunk `deleteMessages` into batches of 100 (Telegram API limit).
- **differ**: use `HashSet<MessageId>` for `frozen_messages` lookup (was O(n) per message).
- **ctx**: `navigate_group` now tracks sent message in `active_bot_messages` when sending
  new (no trigger). Previously lost, causing stale messages on subsequent navigate.
- **ctx**: `reply()` now updates `TrackedMessage` hash after edit, preventing unnecessary
  edits on next `navigate()`.
- **broadcast**: `add_dismiss_button` now supports Video, Animation, Document (was Text/Photo only).
- **update_parser**: parse all media types (voice, video note, sticker, contact, location),
  service messages (member joined/left), payment messages.
- **rate_limiter**: proxy all BotApi methods (was 30 missing, returning "not implemented").

### Changed

- `cargo fmt` + `cargo clippy` clean.
- `StateStore` methods return `Result` (breaking change from 0.4.1 internals).
- Markup: simplified `read_until_double` — flattened nested if/else.
- Conversation handlers: removed redundant else blocks.

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
