//! Scheduled bot — demonstrates scheduled messages & callbacks.
//!
//! - `/remind` sends a message that auto-deletes after 10 seconds.
//! - `/timer` schedules a callback that fires after 5 seconds.

use blazegram::{handler, prelude::*};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    App::builder(std::env::var("BOT_TOKEN").expect("BOT_TOKEN"))
        .command("start", handler!(ctx => {
            ctx.navigate(Screen::text("home", "🕐 /remind or /timer").build()).await
        }))
        .command("remind", handler!(ctx => {
            ctx.notify_temp(
                "⏰ This message will disappear in 10 seconds!",
                std::time::Duration::from_secs(10),
            ).await?;
            Ok(())
        }))
        .command("timer", handler!(ctx => {
            ctx.navigate(Screen::text("waiting", "⏳ Timer set! Callback in 5 seconds...").build()).await?;
            ctx.schedule_callback("timer:fired", std::time::Duration::from_secs(5));
            Ok(())
        }))
        .callback("timer:fired", handler!(ctx => {
            ctx.navigate(Screen::text("fired", "🎉 Timer fired!").build()).await
        }))
        .run()
        .await;
}
