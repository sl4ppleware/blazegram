<div align="center">

<img src="logo.png" width="120">

# blazegram

**Declarative Telegram bot framework for Rust.**\
One screen at a time. Zero garbage in chat. Direct MTProto over persistent TCP.

[![Crates.io](https://img.shields.io/crates/v/blazegram.svg)](https://crates.io/crates/blazegram)
[![docs.rs](https://img.shields.io/docsrs/blazegram)](https://docs.rs/blazegram)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/sl4ppleware/blazegram/blob/main/LICENSE)
[![Rust 1.85+](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org)

</div>

---

## Why blazegram?

| | HTTP Bot API | **blazegram** |
|---|---|---|
| **Latency** | ~50 ms per call (2 hops) | **~5 ms** (direct MTProto) |
| **File uploads** | 50 MB limit | **2 GB** |
| **Connection** | New HTTP per call | **Persistent TCP socket** |
| **Message management** | Manual IDs everywhere | **Automatic diffing** |
| **Chat cleanup** | You delete manually | **Auto-managed** |

blazegram holds a **single persistent TCP socket** to Telegram's datacenter via
[grammers](https://crates.io/crates/grammers-client) MTProto — no webhook server,
no middleman, no HTTP overhead.

On top of that, it introduces the **Screen** abstraction: declare what the user should see,
and a **Virtual Chat Differ** computes the minimal set of API calls to get there.

## Quick start

```toml
[dependencies]
blazegram = "0.4"
tokio = { version = "1", features = ["full"] }
```

```rust,no_run
use blazegram::{handler, prelude::*};

#[tokio::main]
async fn main() {
    App::builder("BOT_TOKEN")
        .command("start", handler!(ctx => {
            ctx.navigate(
                Screen::text("home", "<b>Pick a side.</b>")
                    .keyboard(|kb| kb
                        .button("Light", "pick:light")
                        .button("Dark", "pick:dark"))
                    .build()
            ).await
        }))
        .callback("pick", handler!(ctx => {
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

First launch authenticates via MTProto and creates a `.session` file.
Subsequent starts reconnect in under 100 ms.

---

## Core concepts

### Screens & the Differ

A **Screen** is a declarative snapshot of what the user should see. Call `navigate()` and the
differ handles everything:

```text
  callback (button press)     → edit in place       (1 API call)
  user sent text / command    → delete old + send   (2–3 calls)
  content identical           → nothing             (0 calls)
```

No message IDs. No "should I edit or re-send?" logic. No stale buttons lingering in chat.

```rust
# use blazegram::prelude::*;
// text + keyboard
Screen::text("menu", "Pick one:")
    .keyboard(|kb| kb
        .button_row("A", "pick:a")
        .button_row("B", "pick:b"))
    .build();

// photo with caption
Screen::builder("gallery")
    .photo("https://example.com/pic.jpg")
        .caption("Nice shot")
        .keyboard(|kb| kb.button_row("Next", "next"))
        .done()
    .build();

// multi-message screen
Screen::builder("receipt")
    .text("Order confirmed.").done()
    .photo("https://example.com/qr.png").caption("QR code").done()
    .build();
```

### Navigation stack

`push()` / `pop()` give you a navigation stack (capped at 20 levels) —
back buttons work out of the box:

```rust,ignore
ctx.push(detail_screen).await?;           // push new screen
ctx.pop(|prev_id| make_screen(prev_id)).await?;  // pop back
```

---

## Features

### 🔲 Keyboards & Grids

Fluent keyboard builder with automatic row management:

```rust,ignore
.keyboard(|kb| kb
    .button("A", "a").button("B", "b").button("C", "c").row()  // 3 buttons in one row
    .button_row("Full width", "full")                            // single-button row
    .grid(items, 3, |item| (item.name, item.id))                 // auto-grid from iterator
    .pagination(page, total, "page")                             // ← [2/5] →
    .nav_back("menu")                                            // localized back button
    .confirm_cancel("OK", "ok", "Cancel", "cancel")              // confirm/cancel pair
)
```

### 📄 Pagination

One-liner paginated lists with navigation buttons:

```rust,ignore
let paginator = Paginator::new(items, 5);           // 5 per page
let screen = paginated_screen(
    "items",              // screen ID
    "Your items",         // title
    &paginator,           // paginator
    |i, item| (item.name.clone(), format!("view:{i}")),  // formatter
    "page",               // callback prefix for ←/→
    "menu",               // back callback
);
ctx.navigate(screen).await?;
```

`← [2/5] →` buttons auto-generated. Handles empty lists. Labels localized via i18n.

### 📝 Multi-step forms

Declarative form wizard with validation, type coercion, and auto-generated keyboards:

```rust,ignore
Form::builder("signup")
    .text_step("name", "name", "Your name?")
        .validator(|s| if s.len() < 2 { Err("Too short".into()) } else { Ok(()) })
        .done()
    .integer_step("age", "age", "Age?").min(13).max(120).done()
    .choice_step("plan", "plan", "Pick a plan:", vec![("Free", "free"), ("Pro", "pro")])
    .confirm_step(|d| format!("Name: {}\nAge: {}", d["name"], d["age"]))
    .on_complete(form_handler!(ctx, data => {
        ctx.navigate(Screen::text("done", "Welcome aboard.").build()).await
    }))
    .build()
    .unwrap()
```

Bad input auto-deleted, error shown as 3 s toast, cancel button built-in.

### ⚡ Progressive updates (streaming)

Stream edits to a single message, auto-throttled to respect Telegram rate limits.
Perfect for LLM streaming, progress bars, live dashboards:

```rust,ignore
let h = ctx.progressive(Screen::text("t", "Loading...").build()).await?;
h.update(Screen::text("t", "Loading... 40%").build()).await;
h.update(Screen::text("t", "Loading... 80%").build()).await;
h.finalize(Screen::text("t", "Done ✅").build()).await?;
```

If `navigate()` is called before `finalize()`, the stream is cancelled automatically — no races.

### 💬 Reply mode

For conversational bots (LLM wrappers, support bots) that don't need chat cleanup:

```rust,ignore
ctx.reply(Screen::text("r", "thinking...").build()).await?;    // sends new message
ctx.reply(Screen::text("r", "thinking... ok").build()).await?;  // edits same message
ctx.reply(Screen::text("r", "Here you go.").build()).await?;    // edits same message
// next handler call → fresh message
```

User messages are **not** deleted. Combine with `freeze_message()` to keep important messages
across `navigate()` transitions.

### 💾 State management

Typed per-chat state with zero boilerplate:

```rust,ignore
// key-value
ctx.set("counter", &42);
let n: i32 = ctx.get("counter").unwrap_or(0);

// or full typed state
#[derive(Serialize, Deserialize, Default)]
struct Profile { xp: u64, level: u32 }
let p: Profile = ctx.state();
ctx.set_state(&Profile { xp: 100, level: 2 });
```

**4 backends**, same API:

| Backend | Setup | Persistence |
|---------|-------|-------------|
| In-memory | default | none |
| Memory + snapshot | `.snapshot("state.bin")` | periodic flush to disk |
| **redb** | `.redb_store("bot.redb")` | pure Rust, ACID, zero C deps |
| Redis | `.redis_store("redis://...")` | multi-instance, feature `redis` |

### 🌍 i18n

FTL-based with automatic user language detection:

```rust,ignore
// locales/en.ftl: greeting = Hello, { $name }!
// locales/ru.ftl: greeting = Привет, { $name }!

let text = ctx.t_with("greeting", &[("name", "World")]);
// → "Hello, World!" or "Привет, World!" depending on user.language_code
```

Framework labels (back, next, cancel) are auto-localized.

### 📡 Broadcast

Mass-message all users with built-in rate limiting and optional dismiss button:

```rust,ignore
let screen = Screen::text("update", "🎉 New feature!").build();
let result = broadcast(&bot, &store, screen, BroadcastOptions::default().hideable()).await;
// result.sent = 1523, result.blocked = 12, result.failed = 0
```

### 🔌 Inline mode

Declarative result builders with auto-pagination:

```rust,ignore
.on_inline(handler!(ctx, query, offset => {
    let results = search(&query).iter().map(|r|
        InlineResult::article(&r.id)
            .title(&r.title)
            .description(&r.summary)
            .screen(Screen::text("r", &r.body).build())
            .build()
    ).collect();
    let answer = InlineAnswer::new(results).per_page(20).cache_time(60);
    let (page, next) = answer.paginate(&offset);
    ctx.answer_inline(page.into_iter().map(|r| r.clone().into()).collect(), Some(next), Some(60), false).await
}))
```

### 🛡️ Middleware

Composable middleware chain — auth, throttle, logging, analytics:

```rust,ignore
App::builder("TOKEN")
    .middleware(LoggingMiddleware)
    .middleware(ThrottleMiddleware::new(5, Duration::from_secs(1)))
    .middleware(AuthMiddleware::new(vec![UserId(123456)]))
    .run().await;
```

### 🧪 Testing

Full test harness with `MockBotApi` — no network, no tokens:

```rust,ignore
use blazegram::{handler, prelude::*};
use blazegram::testing::TestApp;

fn make_router() -> Router {
    let mut r = Router::new();
    r.command("start", handler!(ctx => {
        ctx.navigate(Screen::text("home", "Pick a side.").build()).await
    }));
    r
}

#[tokio::test]
async fn test_start() {
    let app = TestApp::new(make_router());
    app.send_message(100, "/start").await.unwrap();
    let msgs = app.sent_messages();
    assert!(msgs.last().unwrap().text.contains("Pick a side"));
}

#[tokio::test]
async fn test_callback() {
    let app = TestApp::new(make_router());
    app.send_message(100, "/start").await.unwrap();
    app.send_callback(100, "pick:dark").await.unwrap();
}
```

Simulate any update type: text, callbacks, photos, voice, stickers, locations,
payments, member joins/leaves.

### 💳 Payments (Stars & Fiat)

```rust,ignore
// Send invoice (Telegram Stars)
ctx.send_invoice(Invoice {
    title: "Premium".into(),
    description: "Unlock premium features".into(),
    payload: "premium_1".into(),
    currency: "XTR".into(),
    prices: vec![("Premium".into(), 100)],
    provider_token: None,  // None = Stars
    ..Default::default()
}).await?;

// Handle checkout
.on_pre_checkout(handler!(ctx => { ctx.approve_checkout().await }))
.on_successful_payment(handler!(ctx => {
    ctx.navigate(Screen::text("ty", "Thanks for your purchase! 🎉").build()).await
}))
```

---

## Good to know

**Unrecognized messages are deleted by default** to keep the chat clean.
Disable with `.delete_unrecognized(false)`.

**Rate limiting** is adaptive: global (30 rps), per-chat (1 rps private, 20/min groups),
with automatic FLOOD_WAIT retry. `answer_callback_query` bypasses the limiter.

**Entity fallback**: if HTML formatting fails, the executor automatically retries as plain text.

**`handler!` macro** eliminates `Box::pin(async move { ... })` boilerplate:

```rust,ignore
handler!(ctx => { ... })                  // commands, callbacks
handler!(ctx, text => { ... })            // on_input
form_handler!(ctx, data => { ... })       // form completion
```

## Architecture

```text
   Handlers       .command() / .callback() / .on_input()
       │
       ▼
     Ctx           navigate() / push() / pop() / reply()
       │
       ▼
    Differ          old msgs + new Screen → minimal ops
       │
       ▼
   Executor         FLOOD_WAIT retry, entity fallback
       │
       ▼
    BotApi          70+ async methods (trait, mockable)
       │
       ▼
   grammers         MTProto → Telegram DC (persistent TCP)
```

Per-chat mutex guarantees sequential update processing. No race conditions.

## License

MIT
