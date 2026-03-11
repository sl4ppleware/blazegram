//! Minimal ping-pong bot. Responds to /ping with MTProto round-trip latency.
//!
//! Run:  BOT_TOKEN=123:xxx cargo run --example ping_bot

use blazegram::{handler, prelude::*};
use std::time::Instant;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let token = std::env::var("BOT_TOKEN").expect("BOT_TOKEN required");

    App::builder(&token)
        .session_file("ping_bot.session")
        .command(
            "start",
            handler!(ctx => {
                ctx.navigate(
                    Screen::text("home", "Send /ping to measure MTProto round-trip.")
                        .build()
                ).await
            }),
        )
        .command(
            "ping",
            handler!(ctx => {
                let t0 = Instant::now();
                let sent = ctx.send_text("Pong!").await?;
                let ms = t0.elapsed().as_millis();
                ctx.bot().edit_message_text(
                    ctx.chat_id, sent.message_id,
                    format!("Pong! <code>{ms}ms</code>"),
                    ParseMode::Html,
                    Some(InlineKeyboard {
                        rows: vec![vec![InlineButton {
                            text: "Ping again".into(),
                            action: ButtonAction::Callback("ping_again".into()),
                        }]],
                    }),
                    false,
                ).await.ok();
                Ok(())
            }),
        )
        .callback(
            "ping_again",
            handler!(ctx => {
                let t0 = Instant::now();
                let sent = ctx.send_text("Pong!").await?;
                let ms = t0.elapsed().as_millis();
                ctx.bot().edit_message_text(
                    ctx.chat_id, sent.message_id,
                    format!("Pong! <code>{ms}ms</code>"),
                    ParseMode::Html,
                    Some(InlineKeyboard {
                        rows: vec![vec![InlineButton {
                            text: "Ping again".into(),
                            action: ButtonAction::Callback("ping_again".into()),
                        }]],
                    }),
                    false,
                ).await.ok();
                Ok(())
            }),
        )
        .run()
        .await;
}
