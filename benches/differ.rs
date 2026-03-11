//! Benchmarks for the Virtual Chat Differ, router dispatch, and serialization.

use std::hint::black_box;
use std::time::Instant;

use blazegram::differ::Differ;
use blazegram::router::Router;
use blazegram::screen::Screen;
use blazegram::types::*;

fn bench(name: &str, iterations: u32, f: impl Fn()) {
    // Warmup
    for _ in 0..iterations / 10 {
        f();
    }
    let start = Instant::now();
    for _ in 0..iterations {
        f();
    }
    let elapsed = start.elapsed();
    let per_iter = elapsed / iterations;
    println!("{name}: {per_iter:?}/iter ({iterations} iters, {elapsed:?} total)");
}

fn main() {
    println!("=== Blazegram Benchmarks ===");
    println!();

    // ── Differ: identical screens (no-op) ──
    bench("differ_identical", 100_000, || {
        let old = vec![TrackedMessage {
            message_id: MessageId(1),
            content_type: ContentType::Text,
            content_hash: 12345,
            text_hash: 12345,
            caption_hash: 0,
            file_hash: 0,
            keyboard_hash: 0,
        }];
        let new_screen = Screen::text("home", "<b>Hello</b>").build();
        let ops = Differ::diff(&old, &new_screen, &[]);
        black_box(ops);
    });

    // ── Differ: text change (edit) ──
    bench("differ_text_edit", 100_000, || {
        let old = vec![TrackedMessage {
            message_id: MessageId(1),
            content_type: ContentType::Text,
            content_hash: 11111,
            text_hash: 11111,
            caption_hash: 0,
            file_hash: 0,
            keyboard_hash: 0,
        }];
        let new_screen = Screen::text("home", "<b>Updated</b>").build();
        let ops = Differ::diff(&old, &new_screen, &[]);
        black_box(ops);
    });

    // ── Differ: from empty (send all) ──
    bench("differ_from_empty", 100_000, || {
        let old: Vec<TrackedMessage> = vec![];
        let new_screen = Screen::text("home", "<b>Hello</b>")
            .keyboard(|kb| kb.button("A", "a").button("B", "b"))
            .build();
        let ops = Differ::diff(&old, &new_screen, &[]);
        black_box(ops);
    });

    // ── Screen building ──
    bench("screen_build", 100_000, || {
        let screen = Screen::text("menu", "Pick one:")
            .keyboard(|kb| {
                kb.button_row("Option A", "pick:a")
                    .button_row("Option B", "pick:b")
                    .button_row("Option C", "pick:c")
            })
            .build();
        black_box(screen);
    });

    // ── Content hashing ──
    bench("content_hash", 1_000_000, || {
        let content = MessageContent::Text {
            text: "<b>Hello, World!</b> This is a test message with some content.".into(),
            parse_mode: ParseMode::Html,
            keyboard: None,
            link_preview: LinkPreview::Disabled,
        };
        let hash = content.content_hash();
        black_box(hash);
    });

    // ── Router: command lookup ──
    bench("router_command_lookup", 100_000, || {
        let mut router = Router::new();
        for cmd in &[
            "start",
            "help",
            "settings",
            "about",
            "stats",
            "admin",
            "broadcast",
        ] {
            let cmd = cmd.to_string();
            router.command(&cmd, |_ctx| Box::pin(async { Ok(()) }));
        }
        black_box(&router);
    });

    // ── Serialization: ChatState postcard roundtrip ──
    bench("chatstate_serialize", 100_000, || {
        let user = UserInfo {
            id: UserId(12345),
            first_name: "Test".into(),
            last_name: Some("User".into()),
            username: Some("testuser".into()),
            language_code: Some("en".into()),
        };
        let mut state = ChatState::new(ChatId(12345), user);
        state.data.insert("key".into(), serde_json::json!("value"));
        state.data.insert("counter".into(), serde_json::json!(42));
        let bytes = serde_json::to_vec(&state).unwrap();
        let _: ChatState = serde_json::from_slice(&bytes).unwrap();
        black_box(bytes);
    });

    println!("\nDone.");
}
