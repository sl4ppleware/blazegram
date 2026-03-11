<div align="center">

<img src="logo.png" width="120">

# blazegram

**Declarative Telegram bot framework for Rust.**\
One screen at a time. Zero garbage in chat. Direct MTProto over persistent TCP.

[![Crates.io](https://img.shields.io/crates/v/blazegram.svg)](https://crates.io/crates/blazegram)
[![docs.rs](https://img.shields.io/docsrs/blazegram)](https://docs.rs/blazegram)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/slappleware/blazegram/blob/main/LICENSE)
[![Rust 1.75+](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)

</div>

---

Blazes skips the HTTP Bot API entirely. Instead of polling an HTTP server that polls Telegram,
it holds a **single persistent TCP socket** to Telegram's datacenter via [grammers](https://crates.io/crates/grammers-client) MTProto.
The result: lower latency, 2 GB file uploads, no middleman.

| | HTTP Bot API | blazegram (MTProto) |
|---|---|---|
| Transport | HTTPS poll / webhook | Persistent TCP to DC |
| Overhead per call | ~50 ms (HTTP + JSON) | ~5 ms (binary TL) |
| File limit | 50 MB multipart | 2 GB binary |
| Connection | new TCP per cycle | one socket, kept alive |
| External dep | Bot API server | none |

On top of that, blazegram introduces the **Screen** — a declarative description of what the user
should see right now. When you call `navigate()`, a **Virtual Chat Differ** computes the minimal
set of Telegram API calls (edit, delete, send) to get from the current state to the new one.
You never manage message IDs.

## 30-second example

```rust,no_run
use blazegram::prelude::*;

#[tokio::main]
async fn main() {
    App::builder("BOT_TOKEN")
        .command("start", |ctx| Box::pin(async move {
            ctx.navigate(
                Screen::text("home", "<b>Pick a side.</b>")
                    .keyboard(|kb| kb
                        .button("Light", "pick:light")
                        .button("Dark", "pick:dark"))
                    .build()
            ).await
        }))
        .callback("pick", |ctx| Box::pin(async move {
            let side = ctx.callback_param().unwrap_or_default();
            ctx.navigate(
                Screen::text("chosen", format!("You chose <b>{side}</b>."))
                    .keyboard(|kb| kb.button_row("Back", "menu"))
                    .build()
            ).await
        }))
        .run().await;
}
```

```bash
cargo add blazegram tokio --features tokio/full
BOT_TOKEN=123:ABC cargo run
```

First launch authenticates via MTProto and creates a `.session` file.
Subsequent starts reconnect in under 100 ms.

## The differ

Every `navigate()` call runs through the differ before touching the network:

```text
  callback (button press)     → edit messages in place      (1 API call)
  user sent text / command    → delete old, send new at end (2–3 calls)
  content identical           → nothing                     (0 calls)
```

```text
  [Settings] pressed:

   Before                      After
   ┌──────────────────┐       ┌──────────────────┐
   │ Main Menu        │  ─→  │ Settings         │
   │ [Settings] [Help]│       │ [Lang] [Back]    │
   └──────────────────┘       └──────────────────┘
   differ: EditText(msg_id=42)   ← one API call, not two
```

If the user typed something between screens, the old message is scrolled out of view —
editing it would be invisible. The differ detects this and switches to delete + send instead.
Active progressive streams are auto-cancelled before diffing, so no concurrent edits can race.

## Screens

```rust
# use blazegram::prelude::*;
Screen::text("id", "<b>Hello</b>").build();

Screen::text("menu", "Pick one:")
    .keyboard(|kb| kb
        .button_row("A", "pick:a")
        .button_row("B", "pick:b"))
    .build();

Screen::builder("gallery")
    .photo("https://example.com/pic.jpg")
        .caption("Nice shot")
        .keyboard(|kb| kb.button_row("Next", "next"))
        .done()
    .build();

// multi-message
Screen::builder("receipt")
    .text("Order confirmed.").done()
    .photo("https://example.com/qr.png").caption("QR code").done()
    .build();
```

## Navigation stack

```rust,ignore
ctx.push(Screen::text("detail", "Deep view").build()).await?;
// ... user clicks [Back]
ctx.pop(|prev_id| make_screen(prev_id)).await?;
```

Stack depth capped at 20. Oldest entry dropped silently.

## Forms

```rust,ignore
Form::builder("signup")
    .text_step("name", "name", "Your name?")
        .validator(|s| if s.len() < 2 { Err("Too short".into()) } else { Ok(()) })
        .done()
    .integer_step("age", "age", "Age?")
        .min(13).max(120).done()
    .choice_step("plan", "plan", "Pick a plan:", &["Free", "Pro"])
        .done()
    .confirm_step(|d| format!("Name: {}\nAge: {}\nPlan: {}", d["name"], d["age"], d["plan"]))
    .on_complete(|ctx, data| Box::pin(async move {
        ctx.navigate(Screen::text("done", "Welcome aboard.").build()).await
    }))
    .build()
```

Validation errors auto-delete and show a 3 s toast. Cancel/back buttons built-in.

## Progressive updates

Stream edits to one message. Auto-throttled to stay under Telegram rate limits.
If `navigate()` is called before `finalize()`, the stream is cancelled automatically.

```rust,ignore
let h = ctx.progressive(Screen::text("t", "Loading...").build()).await?;
h.update(Screen::text("t", "Loading... 40%").build()).await;
h.update(Screen::text("t", "Loading... 90%").build()).await;
h.finalize(Screen::text("t", "Done.").build()).await?;
```

## State

```rust,ignore
ctx.set("counter", &42);
let n: i32 = ctx.get("counter").unwrap_or(0);

#[derive(Serialize, Deserialize, Default)]
struct Profile { xp: u64 }
let p: Profile = ctx.state();
ctx.set_state(&Profile { xp: 100 });
```

| Backend | Setup |
|---|---|
| In-memory (default) | nothing |
| Memory + snapshot | `.snapshot("state.bin")` |
| SQLite | `.sqlite_store("bot.db")` |
| Redis | `.redis_store("redis://...")` + feature `redis` |

## Reply mode

For conversational bots that don't need chat cleanup:

```rust,ignore
ctx.reply(Screen::text("r", "thinking...").build()).await?;   // sends
ctx.reply(Screen::text("r", "thinking... ok").build()).await?; // edits
ctx.reply(Screen::text("r", "Here you go.").build()).await?;   // edits
```

User messages are **not** deleted. Previous replies are **not** deleted.
Next handler call starts a fresh message.

## Frozen / permanent messages

```rust,ignore
// frozen: survives navigate(), differ won't touch it
let sent = ctx.send_text("Pinned info").await?;
ctx.freeze_message(sent.message_id);

// permanent: never tracked, never deleted
ctx.send_permanent(Screen::text("p", "Receipt #123").build()).await?;
```

## Inline mode

```rust,ignore
.inline(|ctx| Box::pin(async move {
    ctx.answer_inline(vec![
        InlineResult::article("1", "Result")
            .description("Description")
            .text("Selected."),
    ], None, None, false).await
}))
```

## i18n

```text
locales/en.json  { "hi": "Hello, { $name }!" }
locales/de.json  { "hi": "Hallo, { $name }!" }
```

```rust,ignore
let text = ctx.t_with("hi", &[("name", "World")]);
```

Language auto-detected from `user.language_code`. Falls back to default.

## Template engine

```rust,ignore
let html = blazegram::template::render(
    "<b>{{ title }}</b>\n{% for x in items %}- {{ x }}\n{% endfor %}",
    &vars,
);
```

Supports `{{ var }}`, `{% if cond %}`, `{% for x in list %}`, `{% else %}`.

## Middleware

```rust,ignore
App::builder("TOKEN")
    .middleware(LoggingMiddleware)
    .middleware(AnalyticsMiddleware::new())
    .middleware(ThrottleMiddleware::new(5, Duration::from_secs(1)))
    // custom:
    .middleware(MyAuthMiddleware { admin_ids: vec![123] })
```

## Testing

```rust,ignore
use blazegram::mock::MockBotApi;
use blazegram::testing::TestApp;

#[tokio::test]
async fn test_start() {
    let app = TestApp::new();
    let reply = app.send_command("/start").await;
    assert!(reply.text.contains("Pick a side"));
}
```

No network. `MockBotApi` records every API call for assertions.

## All features

| | |
|---|---|
| **Screen system** | Declarative text / photo / video / document / multi-message screens |
| **Virtual Chat Differ** | Minimal edit / delete / send operations per transition |
| **Inline keyboards** | Buttons, grids, URLs, web apps, switch-inline, callback params |
| **Reply keyboards** | Bottom keyboard with resize, one-time, placeholder |
| **Navigation stack** | `push()` / `pop()`, max depth 20 |
| **Forms** | Text, integer, choice, photo steps; validation; cancel/back/confirm |
| **Pagination** | Auto-paged lists with prev/next buttons |
| **Progressive updates** | Throttled streaming edits; auto-cancel on navigate |
| **Reply mode** | Conversational send-then-edit, no cleanup |
| **Frozen messages** | Survive `navigate()` transitions |
| **Permanent messages** | Bypass differ completely |
| **Inline mode** | Query results with builder API, pagination, chosen result handler |
| **i18n** | JSON bundles, `{ $var }` interpolation, auto language detection |
| **Templates** | `{{ var }}`, `{% if %}`, `{% for %}` in any message text |
| **Middleware** | Logging, analytics, throttle built-in; custom via trait |
| **Metrics** | Update/error counters, latency histograms, `.summary()` |
| **State** | Memory, snapshot, SQLite, Redis backends |
| **File cache** | file_id caching to skip re-uploads |
| **Rate limiter** | Token bucket + automatic FLOOD_WAIT retry |
| **Entity fallback** | Auto plain-text retry when HTML entities fail |
| **Broadcast** | Concurrent multi-chat send with rate limiting |
| **Payments** | Invoices, pre-checkout, Stars API |
| **Forum topics** | Create, edit, close, reopen |
| **BotApi trait** | 73 async methods, fully mockable |
| **Testing** | `MockBotApi` + `TestApp`, zero network |

## Architecture

```text
   Your handlers        .command() / .callback() / .on_input()
        │
        ▼
      Ctx              navigate() / push() / pop() / reply()
        │
        ▼
     Differ            old tracked msgs + new Screen → minimal ops
        │
        ▼
    Executor           retry on FLOOD_WAIT, fallback on entity errors
        │
        ▼
     BotApi            73 async methods (trait, mockable)
        │
        ▼
    grammers           MTProto → Telegram DC (persistent TCP)
```

Per-chat mutex serializer guarantees sequential update processing.
No race conditions across concurrent users.

## Install

```toml
[dependencies]
blazegram = "0.3"
tokio = { version = "1", features = ["full"] }
```

Optional:

```toml
blazegram = { version = "0.3", features = ["redis"] }  # Redis state backend
tracing-subscriber = "0.3"                              # structured logging
```

## MSRV

1.75

## License

MIT
