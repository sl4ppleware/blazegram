//! Quiz game bot. Demonstrates state, timed screens, and score tracking.
//!
//! Run:  BOT_TOKEN=123:xxx cargo run --example quiz_bot

use blazegram::prelude::*;

const QUESTIONS: &[(&str, &[&str], usize)] = &[
    ("What does MTProto stand for?",
     &["Mobile Transport Protocol", "Meta Transfer Protocol", "Message Type Protocol"],
     0),
    ("Which crate provides Rust MTProto?",
     &["teloxide", "grammers", "frankenstein"],
     1),
    ("Maximum file size via MTProto?",
     &["50 MB", "500 MB", "2 GB"],
     2),
    ("What does the Differ minimize?",
     &["Memory usage", "API calls", "Binary size"],
     1),
    ("Blazegram's UI primitive is called?",
     &["View", "Screen", "Widget"],
     1),
];

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let token = std::env::var("BOT_TOKEN").expect("BOT_TOKEN required");

    App::builder(&token)
        .session_file("quiz_bot.session")
        .command("start", |ctx| Box::pin(async move {
            ctx.set("q", &0usize);
            ctx.set("score", &0usize);
            ctx.navigate(question_screen(0, 0)).await
        }))
        .callback("ans", |ctx| Box::pin(async move {
            let picked: usize = ctx.callback_param_as().unwrap_or(0);
            let q: usize = ctx.get("q").unwrap_or(0);
            let mut score: usize = ctx.get("score").unwrap_or(0);

            let (_, _, correct) = QUESTIONS[q];
            if picked == correct { score += 1; }
            ctx.set("score", &score);

            let next = q + 1;
            if next < QUESTIONS.len() {
                ctx.set("q", &next);
                ctx.navigate(question_screen(next, score)).await
            } else {
                let total = QUESTIONS.len();
                let pct = (score * 100) / total;
                let grade = match pct {
                    80..=100 => "Excellent!",
                    60..=79 => "Good job.",
                    40..=59 => "Not bad.",
                    _ => "Try again?",
                };
                ctx.navigate(
                    Screen::text("result", format!(
                        "<b>Quiz complete!</b>\n\nScore: {score}/{total} ({pct}%)\n{grade}"
                    ))
                    .keyboard(|kb| kb.button_row("Play again", "restart"))
                    .build()
                ).await
            }
        }))
        .callback("restart", |ctx| Box::pin(async move {
            ctx.set("q", &0usize);
            ctx.set("score", &0usize);
            ctx.navigate(question_screen(0, 0)).await
        }))
        .run()
        .await;
}

fn question_screen(idx: usize, score: usize) -> Screen {
    let (question, options, _) = QUESTIONS[idx];
    let num = idx + 1;
    let total = QUESTIONS.len();

    Screen::text("quiz", format!(
        "<b>Question {num}/{total}</b>  (score: {score})\n\n{question}"
    ))
    .keyboard(|kb| {
        let mut kb = kb;
        for (i, opt) in options.iter().enumerate() {
            kb = kb.button_row(*opt, &format!("ans:{i}"));
        }
        kb
    })
    .build()
}
