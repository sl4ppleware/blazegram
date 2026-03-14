//! Conversation bot — demonstrates branching conversations.
//!
//! A simple onboarding flow that branches based on user role.

use std::sync::Arc;

use blazegram::{handler, prelude::*};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let onboarding = Conversation::builder("onboarding")
        .step(
            "name",
            |_data, _lang| Screen::text("conv_name", "👋 What's your name?").build(),
            None,
        )
        .step(
            "role",
            |_data, _lang| {
                Screen::text("conv_role", "Are you a *student* or *teacher*?")
                    .keyboard(|kb| kb.button("Student", "student").button("Teacher", "teacher"))
                    .build()
            },
            None,
        )
        .branch(
            "role",
            Arc::new(|data| match data.get("role").and_then(|v| v.as_str()) {
                Some("student") => "student_year".to_string(),
                _ => "teacher_subject".to_string(),
            }),
        )
        .step(
            "student_year",
            |_data, _lang| Screen::text("conv_year", "📚 What year are you in?").build(),
            None,
        )
        .step(
            "teacher_subject",
            |_data, _lang| Screen::text("conv_subject", "📖 What subject do you teach?").build(),
            None,
        )
        .end_at("student_year")
        .end_at("teacher_subject")
        .on_complete(Arc::new(|ctx, data| {
            Box::pin(async move {
                let name = data.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                let role = data.get("role").and_then(|v| v.as_str()).unwrap_or("?");
                let text = format!("✅ Welcome, {} ({}). All set!", name, role);
                ctx.navigate(Screen::text("done", text).build()).await
            })
        }))
        .build()
        .expect("valid conversation");

    App::builder(std::env::var("BOT_TOKEN").expect("BOT_TOKEN"))
        .conversation(onboarding)
        .command("start", handler!(ctx => {
            ctx.navigate(Screen::text("home", "Welcome! Type /onboard to begin.").build()).await
        }))
        .command("onboard", handler!(ctx => {
            // NOTE: In a real app you'd pass the conversations map.
            // Here we just set the state manually for demonstration.
            ctx.set("__conv_id", &"onboarding".to_string());
            ctx.set("__conv_step", &0usize);
            ctx.set("__conv_data", &std::collections::HashMap::<String, serde_json::Value>::new());
            ctx.navigate(Screen::text("conv_name", "👋 What's your name?").build()).await
        }))
        .run()
        .await;
}
