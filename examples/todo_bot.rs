//! To-do list bot. Demonstrates pagination, text input, and state persistence.
//!
//! Run:  BOT_TOKEN=123:xxx cargo run --example todo_bot

use blazegram::{handler, prelude::*};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default, Clone)]
struct TodoState {
    items: Vec<String>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let token = std::env::var("BOT_TOKEN").expect("BOT_TOKEN required");

    App::builder(&token)
        .session_file("todo_bot.session")
        .snapshot("todo_state.bin")
        .command(
            "start",
            handler!(ctx => {
                ctx.navigate(list_screen(&ctx.state::<TodoState>())).await
            }),
        )
        .callback(
            "list",
            handler!(ctx => {
                ctx.navigate(list_screen(&ctx.state::<TodoState>())).await
            }),
        )
        .callback(
            "add",
            handler!(ctx => {
                ctx.navigate(
                    Screen::text("add", "Type a new to-do item:")
                        .expect_text()
                            .placeholder("Buy milk...")
                            .validator(|s| {
                                if s.trim().is_empty() { Err("Cannot be empty.".into()) }
                                else { Ok(()) }
                            })
                        .build()
                ).await
            }),
        )
        .on_input(
            "add",
            handler!(ctx, text => {
                let mut st: TodoState = ctx.state();
                st.items.push(text.trim().to_string());
                ctx.set_state(&st);
                ctx.navigate(list_screen(&st)).await
            }),
        )
        .callback(
            "del",
            handler!(ctx => {
                let idx: usize = ctx.callback_param_as().unwrap_or(usize::MAX);
                let mut st: TodoState = ctx.state();
                if idx < st.items.len() {
                    st.items.remove(idx);
                    ctx.set_state(&st);
                }
                ctx.navigate(list_screen(&st)).await
            }),
        )
        .callback(
            "clear",
            handler!(ctx => {
                ctx.set_state(&TodoState::default());
                ctx.navigate(list_screen(&TodoState::default())).await
            }),
        )
        .run()
        .await;
}

fn list_screen(st: &TodoState) -> Screen {
    if st.items.is_empty() {
        return Screen::text("list", "<b>To-Do</b>\n\nNo items yet.")
            .keyboard(|kb| kb.button_row("+ Add", "add"))
            .build();
    }

    let lines: String = st
        .items
        .iter()
        .enumerate()
        .map(|(i, item)| format!("{}. {}", i + 1, blazegram::markup::escape(item)))
        .collect::<Vec<_>>()
        .join("\n");

    Screen::text(
        "list",
        format!("<b>To-Do</b> ({})\n\n{lines}", st.items.len()),
    )
    .keyboard(|kb| {
        let mut kb = kb;
        for (i, _) in st.items.iter().enumerate() {
            kb = kb.button(format!("\u{2716} {}", i + 1), format!("del:{i}"));
            if (i + 1) % 4 == 0 {
                kb = kb.row();
            }
        }
        kb.row()
            .button("+ Add", "add")
            .button("Clear all", "clear")
            .row()
    })
    .build()
}
