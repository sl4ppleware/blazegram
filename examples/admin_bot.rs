//! Admin bot — demonstrates router groups with auth middleware.
//!
//! The admin group requires the user to be in the `ADMIN_IDS` list.
//! Public commands work for everyone.

use blazegram::{handler, prelude::*};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    const ADMIN_IDS: &[u64] = &[123456789]; // Replace with your Telegram user ID

    // Admin group with auth middleware
    let admin = RouterGroup::new()
        .middleware(AuthMiddleware::new(ADMIN_IDS.iter().copied()))
        .command(
            "ban",
            handler!(ctx => {
                ctx.navigate(Screen::text("ban", "🔨 User banned!").build()).await
            }),
        )
        .command(
            "stats",
            handler!(ctx => {
                let count = ctx.member_count().await.unwrap_or(0);
                let text = format!("📊 Members: {}", count);
                ctx.navigate(Screen::text("stats", text).build()).await
            }),
        );

    App::builder(std::env::var("BOT_TOKEN").expect("BOT_TOKEN"))
        .group(admin)
        .command(
            "start",
            handler!(ctx => {
                ctx.navigate(Screen::text("home", "Welcome! /help for commands").build()).await
            }),
        )
        .command(
            "help",
            handler!(ctx => {
                ctx.navigate(Screen::text("help",
                    "Public: /start /help\nAdmin: /ban /stats"
                ).build()).await
            }),
        )
        .run()
        .await;
}
