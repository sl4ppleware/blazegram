# Changelog

## [0.4.1] — 2026-03-12

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

### Added

- **adapter**: `restrict_chat_member` — maps `ChatPermissions` to `ChatBannedRights`.
- **adapter**: `promote_chat_member` — maps `ChatPermissions` to `ChatAdminRights`.
- **testing**: `TestApp` expanded with simulation methods for voice, video, sticker,
  location, contact, member joined/left, pre-checkout, and successful payment.
- **testing**: 13 new integration tests covering command routing, callback routing with
  prefix matching, member events, payment flows, and all media input types.
  Total test count: 96 → 109.

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

[0.4.0]: https://github.com/sl4ppleware/blazegram/compare/v0.3.1...v0.4.0
[0.3.1]: https://github.com/sl4ppleware/blazegram/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/sl4ppleware/blazegram/releases/tag/v0.3.0
