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

Blazes skips the HTTP Bot API entirely. It holds a **single persistent TCP socket**
to Telegram's datacenter via [grammers](https://crates.io/crates/grammers-client) MTProto —
~5 ms per call instead of ~50 ms, 2 GB file uploads instead of 50 MB, no middleman server.

On top of that, blazegram introduces the **Screen** — a declarative description of what the user
should see right now. When you call `navigate()`, a **Virtual Chat Differ** computes the minimal
set of Telegram API calls (edit, delete, send) to transition from current state to the new one.
You never manage message IDs.

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

## The differ

Every `navigate()` call runs through the differ before touching the network:

```text
  callback (button press)     → edit in place       (1 API call)
  user sent text / command    → delete old + send   (2–3 calls)
  content identical           → nothing             (0 calls)
```

If the user typed something between screens, the old bot message is scrolled out of view —
editing it would be invisible. The differ detects this and switches to delete + send.
Active progressive streams are auto-cancelled before diffing, so no concurrent edits race.

## Screens

```rust
# use blazegram::prelude::*;
// simple text
Screen::text("id", "<b>Hello</b>").build();

// text + keyboard
Screen::text("menu", "Pick one:")
    .keyboard(|kb| kb
        .button_row("A", "pick:a")
        .button_row("B", "pick:b"))
    .build();

// photo with caption + keyboard
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

`push()` / `pop()` give you a navigation stack (capped at 20 levels):

```rust,ignore
ctx.push(screen).await?;
ctx.pop(|prev_id| make_screen(prev_id)).await?;
```

## Forms

```rust,ignore
Form::builder("signup")
    .text_step("name", "name", "Your name?")
        .validator(|s| if s.len() < 2 { Err("Too short".into()) } else { Ok(()) })
        .done()
    .integer_step("age", "age", "Age?").min(13).max(120).done()
    .choice_step("plan", "plan", "Pick a plan:", &[("Free", "free"), ("Pro", "pro")])
    .confirm_step(|d| format!("Name: {}\nAge: {}", d["name"], d["age"]))
    .on_complete(form_handler!(ctx, data => {
        ctx.navigate(Screen::text("done", "Welcome aboard.").build()).await
    }))
    .build()
```

Validation errors auto-delete the bad input and show a 3 s toast. Cancel/back buttons are built-in.

## Progressive updates

Stream edits to one message, auto-throttled to respect Telegram rate limits.
If `navigate()` is called before `finalize()`, the stream is cancelled automatically.

```rust,ignore
let h = ctx.progressive(Screen::text("t", "Loading...").build()).await?;
h.update(Screen::text("t", "Loading... 40%").build()).await;
h.finalize(Screen::text("t", "Done.").build()).await?;
```

## Reply mode

For conversational bots that don't need chat cleanup:

```rust,ignore
ctx.reply(Screen::text("r", "thinking...").build()).await?;    // sends
ctx.reply(Screen::text("r", "thinking... ok").build()).await?;  // edits
ctx.reply(Screen::text("r", "Here you go.").build()).await?;    // edits
```

User messages are **not** deleted. Next handler call starts a fresh message.

## State

```rust,ignore
ctx.set("counter", &42);
let n: i32 = ctx.get("counter").unwrap_or(0);

// or typed:
#[derive(Serialize, Deserialize, Default)]
struct Profile { xp: u64 }
let p: Profile = ctx.state();
ctx.set_state(&Profile { xp: 100 });
```

Backends: **in-memory** (default), **memory + snapshot** (`.snapshot("state.bin")`),
**redb** (`.redb_store("bot.redb")`, pure Rust, ACID, default feature),
**Redis** (`.redis_store("redis://...")`, feature `redis`).

Per-chat state is capped at 1 000 keys by default (configurable via `.max_state_keys()`).
Tracked bot messages are capped at 100 per chat. Oldest entries are evicted automatically.

## Frozen & permanent messages

```rust,ignore
// frozen: survives navigate(), differ won't touch it
let sent = ctx.send_text("Pinned info").await?;
ctx.freeze_message(sent.message_id);

// permanent: never tracked, never deleted
ctx.send_permanent(Screen::text("p", "Receipt #123").build()).await?;
```

## Inline mode, i18n, templates

```rust,ignore
// inline
.on_inline(handler!(ctx, query, offset => {
    ctx.answer_inline(vec![
        InlineResult::article("1", "Result").description("Desc").text("Selected."),
    ], None, None, false).await
}))

// i18n — auto-detected from user.language_code
// locales/en.json: { "hi": "Hello, { $name }!" }
let text = ctx.t_with("hi", &[("name", "World")]);

// templates
let html = blazegram::template::render(
    "<b>{{ title }}</b>\n{% for x in items %}- {{ x }}\n{% endfor %}",
    &vars,
);
```

## Middleware & testing

```rust,ignore
App::builder("TOKEN")
    .middleware(LoggingMiddleware)
    .middleware(ThrottleMiddleware::new(5, Duration::from_secs(1)))
    .middleware(MyAuthMiddleware { admin_ids: vec![123] })
    .run().await;
```

```rust,ignore
use blazegram::testing::TestApp;

#[tokio::test]
async fn test_start() {
    let app = TestApp::new();
    let reply = app.send_command("/start").await;
    assert!(reply.text.contains("Pick a side"));
}
```

No network. `MockBotApi` records every API call for assertions.

## `handler!` macro

Eliminates `Box::pin(async move { ... })` boilerplate:

```rust,ignore
handler!(ctx => { ... })                  // commands, callbacks
handler!(ctx, text => { ... })            // on_input
form_handler!(ctx, data => { ... })       // form completion
```

## Good to know

**Unrecognized messages are deleted by default** to keep the chat clean.
Disable with `.delete_unrecognized(false)` or register `.on_unrecognized(handler!(...))`.

**`reply()` messages are tracked by the differ.** Switching from `reply()` to `navigate()`
cleans up old replies. Freeze them if you want them to persist.

**Rate limiting** is adaptive: global (30 rps), per-chat (1 rps private, 20/min groups),
with automatic FLOOD_WAIT retry and exponential backoff. `answer_callback_query` bypasses the limiter.

**Entity fallback**: if HTML entities fail (`ENTITY_BOUNDS_INVALID`), the executor
automatically retries as plain text.

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
    BotApi          60+ async methods (trait, mockable)
       │
       ▼
   grammers         MTProto → Telegram DC (persistent TCP)
```

Per-chat mutex guarantees sequential update processing. No race conditions.

## License

MIT
