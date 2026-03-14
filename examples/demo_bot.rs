//! Blazegram demo bot -- minimal example covering screens, callbacks, state, and input.
//!
//! Run:  BOT_TOKEN=123:xxx cargo run --example demo_bot

use blazegram::{handler, prelude::*};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let token = std::env::var("BOT_TOKEN").expect("BOT_TOKEN required");

    App::builder(&token)
        .session_file("demo_bot.session")
        .command("start", handler!(ctx => {
            ctx.navigate(main_menu(&ctx.user.first_name)).await
        }))
        .command("help", handler!(ctx => {
            ctx.navigate(
                Screen::text("help", "Commands:\n/start — main menu\n/help — this message")
                    .keyboard(|kb| kb.nav_back("menu"))
                    .build()
            ).await
        }))
        .callback("menu", handler!(ctx => {
            ctx.navigate(main_menu(&ctx.user.first_name)).await
        }))
        .callback("counter", handler!(ctx => {
            let n: i64 = ctx.get("counter").unwrap_or(0);
            ctx.navigate(counter_screen(n)).await
        }))
        .callback("inc", handler!(ctx => {
            let n: i64 = ctx.get("counter").unwrap_or(0) + 1;
            ctx.set("counter", &n);
            ctx.navigate(counter_screen(n)).await
        }))
        .callback("dec", handler!(ctx => {
            let n: i64 = ctx.get("counter").unwrap_or(0) - 1;
            ctx.set("counter", &n);
            ctx.navigate(counter_screen(n)).await
        }))
        .callback("ask_name", handler!(ctx => {
            ctx.navigate(
                Screen::text("ask_name", "What's your name?")
                    .expect_text()
                        .placeholder("Type your name")
                        .validator(|s| {
                            if s.len() < 2 { Err("Too short (min 2)".into()) }
                            else if s.len() > 50 { Err("Too long (max 50)".into()) }
                            else { Ok(()) }
                        })
                    .build()
            ).await
        }))
        .callback("about", handler!(ctx => {
            ctx.navigate(
                Screen::text("about", "\
                    <b>Blazegram v0.4.1</b>\n\n\
                    Pure Rust MTProto (grammers)\n\
                    Direct TCP to Telegram DC\n\
                    Zero HTTP, zero external processes\n\
                    Automatic chat cleanup\n\
                    Virtual Chat Differ")
                    .keyboard(|kb| kb.nav_back("menu"))
                    .build()
            ).await
        }))
        .on_input("ask_name", handler!(ctx, text => {
            ctx.navigate(
                Screen::text("greeted", format!("Hello, <b>{}</b>!", blazegram::markup::escape(&text)))
                    .keyboard(|kb| kb.nav_back("menu"))
                    .build()
            ).await
        }))
        .run()
        .await;
}

fn main_menu(name: &str) -> Screen {
    Screen::text(
        "menu",
        format!(
            "<b>Hello, {}!</b>\n\nPick an action:",
            blazegram::markup::escape(name)
        ),
    )
    .keyboard(|kb| {
        kb.button_row("Counter", "counter")
            .button_row("Text input", "ask_name")
            .button_row("About", "about")
    })
    .build()
}

fn counter_screen(n: i64) -> Screen {
    Screen::text(
        "counter",
        format!("<b>Counter</b>\n\nValue: <code>{n}</code>"),
    )
    .keyboard(|kb| {
        kb.button("-1", "dec")
            .button("+1", "inc")
            .row()
            .nav_back("menu")
    })
    .build()
}
