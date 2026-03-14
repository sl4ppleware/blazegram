//! Blazegram showcase bot — exercises every framework feature.
//!
//! BOT_TOKEN=... cargo run --example showcase_bot

use blazegram::{form_handler, handler, prelude::*};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct UserData {
    counter: i64,
    notes: Vec<String>,
    lang: String,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let token = std::env::var("BOT_TOKEN").expect("BOT_TOKEN required");

    // ── i18n ──
    let mut i18n = I18n::new("en");
    i18n.add("en", "welcome", "Welcome, { $name }!");
    i18n.add("en", "pick", "Pick a section:");
    i18n.add("de", "welcome", "Willkommen, { $name }!");
    i18n.add("de", "pick", "W\u{00e4}hle einen Bereich:");
    // framework built-in key overrides (optional -- defaults are fine)
    i18n.add("en", "bg-nav-back", "\u{2190} Back");
    i18n.add("en", "bg-form-cancel", "Cancel");
    i18n.add("en", "bg-form-confirm", "Confirm");

    // ── Analytics middleware ──
    let analytics = AnalyticsMiddleware::new();
    let analytics_ref = analytics.clone();

    // ── Form ──
    let feedback_form = Form::builder("feedback")
        .text_step("name", "name", "What's your name?")
        .validator(|s| {
            if s.len() < 2 {
                Err("Min 2 chars".into())
            } else if s.len() > 50 {
                Err("Max 50 chars".into())
            } else {
                Ok(())
            }
        })
        .done()
        .integer_step("rating", "rating", "Rate 1-5:")
        .min(1)
        .max(5)
        .done()
        .choice_step(
            "topic",
            "topic",
            "What about?",
            vec![("UX", "ux"), ("Bugs", "bugs"), ("Features", "features")],
        )
        .confirm_step(|data| {
            format!(
                "Name: {}\nRating: {}\nTopic: {}",
                data.get("name").and_then(|v| v.as_str()).unwrap_or("?"),
                data.get("rating").and_then(|v| v.as_i64()).unwrap_or(0),
                data.get("topic").and_then(|v| v.as_str()).unwrap_or("?"),
            )
        })
        .on_complete(form_handler!(ctx, data => {
            let summary = format!(
                "Feedback received!\n\nName: <b>{}</b>\nRating: {}\nTopic: {}",
                blazegram::markup::escape(
                    data.get("name").and_then(|v| v.as_str()).unwrap_or("?")
                ),
                data.get("rating").and_then(|v| v.as_i64()).unwrap_or(0),
                data.get("topic").and_then(|v| v.as_str()).unwrap_or("?"),
            );
            ctx.navigate(Screen::text("feedback_done", summary)
                .keyboard(|kb| kb.button_row("Menu", "menu"))
                .build()
            ).await
        }))
        .on_cancel(handler!(ctx => {
            ctx.navigate(main_menu(ctx)).await
        }))
        .build()
        .unwrap();

    // ── App ──
    App::builder(&token)
        .session_file("showcase.session")
        .i18n(i18n)
        .middleware(LoggingMiddleware)
        .middleware(analytics)
        .form(feedback_form)
        .snapshot("showcase_state.bin")
        .snapshot_interval(std::time::Duration::from_secs(60))

        // === COMMANDS ===
        .command("start", handler!(ctx => {
            // Deep link support
            if let Some(payload) = ctx.deep_link() {
                return ctx.navigate(
                    Screen::text("deeplink", format!("Deep link payload: <code>{}</code>", blazegram::markup::escape(payload)))
                        .keyboard(|kb| kb.button_row("Menu", "menu"))
                        .build()
                ).await;
            }
            ctx.navigate(main_menu(ctx)).await
        }))
        .command("help", handler!(ctx => {
            ctx.navigate(
                Screen::text("help", HELP_TEXT)
                    .keyboard(|kb| kb.button_row("Menu", "menu"))
                    .build()
            ).await
        }))
        .command("stats", {
            let a = analytics_ref.clone();
            move |ctx| { let a = a.clone(); Box::pin(async move { stats_handler(ctx, &a).await }) }
        })
        .command("dice", handler!(ctx => {
            ctx.send_dice(DiceEmoji::Dice).await?;
            Ok(())
        }))
        .command("typing", handler!(ctx => {
            ctx.typing().await?;
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            ctx.reply(Screen::text("typed", "Done typing!").build()).await
        }))

        // === CALLBACKS: Menu & Navigation ===
        .callback("menu", handler!(ctx => {
            ctx.navigate(main_menu(ctx)).await
        }))
        .callback("show_stats", {
            let a = analytics_ref.clone();
            move |ctx| { let a = a.clone(); Box::pin(async move { stats_handler(ctx, &a).await }) }
        })

        // -- Push/Pop navigation stack --
        .callback("nav_demo", handler!(ctx => {
            ctx.push(Screen::text("nav_l1", "Level 1 (pushed)")
                .keyboard(|kb| kb.button_row("Go deeper", "nav_l2").nav_back("nav_pop"))
                .build()
            ).await
        }))
        .callback("nav_l2", handler!(ctx => {
            ctx.push(Screen::text("nav_l2", "Level 2 (pushed again)")
                .keyboard(|kb| kb.button_row("Go deeper", "nav_l3").nav_back("nav_pop"))
                .build()
            ).await
        }))
        .callback("nav_l3", handler!(ctx => {
            ctx.push(Screen::text("nav_l3", "Level 3 — the bottom")
                .keyboard(|kb| kb.nav_back("nav_pop"))
                .build()
            ).await
        }))
        .callback("nav_pop", handler!(ctx => {
            ctx.pop(|prev_id| {
                Screen::text(prev_id.clone(), format!("Popped back to <code>{}</code>", prev_id))
                    .keyboard(|kb| kb.button_row("Go deeper", "nav_l2").nav_back("nav_pop").button_row("Menu", "menu"))
                    .build()
            }).await
        }))

        // -- Counter (typed state) --
        .callback("counter", handler!(ctx => {
            let d: UserData = ctx.state();
            ctx.navigate(counter_screen(d.counter)).await
        }))
        .callback("inc", handler!(ctx => {
            let mut d: UserData = ctx.state();
            d.counter += 1;
            ctx.set_state(&d);
            ctx.navigate(counter_screen(d.counter)).await
        }))
        .callback("dec", handler!(ctx => {
            let mut d: UserData = ctx.state();
            d.counter -= 1;
            ctx.set_state(&d);
            ctx.navigate(counter_screen(d.counter)).await
        }))
        .callback("reset", handler!(ctx => {
            let mut d: UserData = ctx.state();
            d.counter = 0;
            ctx.set_state(&d);
            ctx.toast("Reset!").await?;
            ctx.navigate(counter_screen(0)).await
        }))

        // -- Differ demo (edit vs delete+send) --
        .callback("diff_main", handler!(ctx => {
            ctx.navigate(Screen::text("diff_main", 
                "<b>Differ Demo</b>\n\nPress a button = edit in place (no flicker).\nType something = delete old + send new at bottom.")
                .keyboard(|kb| kb
                    .button_row("Edit me (A)", "diff_a")
                    .button_row("Edit me (B)", "diff_b")
                    .nav_back("menu")
                ).build()
            ).await
        }))
        .callback("diff_a", handler!(ctx => {
            ctx.navigate(Screen::text("diff_main", "State A — edited in place!")
                .keyboard(|kb| kb.button_row("Switch to B", "diff_b").button_row("Switch to A", "diff_a").nav_back("menu"))
                .build()
            ).await
        }))
        .callback("diff_b", handler!(ctx => {
            ctx.navigate(Screen::text("diff_main", "State B — still in place!")
                .keyboard(|kb| kb.button_row("Switch to A", "diff_a").button_row("Switch to B", "diff_b").nav_back("menu"))
                .build()
            ).await
        }))

        // -- Markup --
        .callback("markup", handler!(ctx => {
            // markup::render() converts custom markdown to HTML
            let text = blazegram::markup::render("*Bold* _italic_ ~strike~ `code` __underline__");
            let full = format!(
                "{}\n\n{}\n\n{}",
                text,
                blazegram::markup::link("GitHub", "https://github.com"),
                blazegram::markup::spoiler("spoiler text"),
            );
            ctx.navigate(Screen::text("markup", full)
                .keyboard(|kb| kb.nav_back("menu"))
                .build()
            ).await
        }))

        // -- Template engine --
        .callback("template", handler!(ctx => {
            let tpl = "<b>{{ title }}</b>\n\n{% for item in items %}* {{ item }}\n{% endfor %}\n{% if show_footer %}\n<i>{{ footer }}</i>{% endif %}";
            let mut vars = std::collections::HashMap::new();
            vars.insert("title", "Shopping List".to_string());
            vars.insert("items", serde_json::json!(["Milk", "Eggs", "Bread", "Butter"]).to_string());
            vars.insert("show_footer", "true".to_string());
            vars.insert("footer", "Don't forget the cheese!".to_string());
            let text = blazegram::template::render(tpl, &vars);
            ctx.navigate(Screen::text("template", text)
                .keyboard(|kb| kb.nav_back("menu"))
                .build()
            ).await
        }))

        // -- Pagination --
        .callback("paginate", handler!(ctx => {
            let items: Vec<String> = (1..=47).map(|i| format!("Item #{}", i)).collect();
            let pag = Paginator::new(items, 8);
            ctx.set("pager", &pag);
            ctx.navigate(make_page(&pag)).await
        }))
        .callback("page", handler!(ctx => {
            let page: usize = ctx.callback_param_as().unwrap_or(0);
            let mut pag: Paginator<String> = ctx.get("pager").unwrap();
            pag.set_page(page);
            ctx.set("pager", &pag);
            ctx.navigate(make_page(&pag)).await
        }))

        // -- Progressive (streaming) --
        .callback("progressive", handler!(ctx => {
            let handle = ctx.progressive(
                Screen::text("prog", "Loading...").build()
            ).await?;
            for i in 1..=5 {
                tokio::time::sleep(std::time::Duration::from_millis(800)).await;
                let dots = ".".repeat(i);
                handle.update(Screen::text("prog", format!("Loading{} ({}%)", dots, i * 20)).build()).await;
            }
            handle.finalize(
                Screen::text("prog", "<b>Done!</b> Progressive update complete.")
                    .keyboard(|kb| kb.nav_back("menu"))
                    .build()
            ).await?;
            Ok(())
        }))

        // -- Reply mode (conversation) --
        .callback("reply_demo", handler!(ctx => {
            ctx.reply(Screen::reply_text("Thinking...")).await?;
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            ctx.reply(Screen::reply_text("Still thinking...")).await?;
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            ctx.reply(Screen::text("reply_done", "<b>Reply mode demo!</b>\n\nThis message was sent then edited twice. User messages are preserved.")
                .keyboard(|kb| kb.nav_back("menu"))
                .build()
            ).await
        }))

        // -- Freeze message --
        .callback("freeze_demo", handler!(ctx => {
            let sent = ctx.send_text("This message is FROZEN. Navigate won't delete it.").await?;
            ctx.freeze_message(sent.message_id);
            ctx.navigate(Screen::text("freeze_info", "Frozen message above will survive navigations.\nPress Menu — it stays!")
                .keyboard(|kb| kb.button_row("Unfreeze & clean", format!("unfreeze:{}", sent.message_id.0)).nav_back("menu"))
                .build()
            ).await
        }))
        .callback("unfreeze", handler!(ctx => {
            let mid: i32 = ctx.callback_param_as().unwrap_or(0);
            ctx.unfreeze_message(MessageId(mid));
            let _ = ctx.delete_now(MessageId(mid)).await;
            ctx.toast("Unfrozen & deleted").await?;
            ctx.navigate(main_menu(ctx)).await
        }))

        // -- Permanent message --
        .callback("permanent", handler!(ctx => {
            ctx.send_permanent(
                Screen::text("perm", "This is a PERMANENT message. It survives navigate().").build()
            ).await?;
            ctx.navigate(Screen::text("perm_info", "Permanent message sent above.")
                .keyboard(|kb| kb.nav_back("menu"))
                .build()
            ).await
        }))

        // -- Keyboard variety --
        .callback("keyboards", handler!(ctx => {
            ctx.navigate(Screen::text("keyboards", "<b>Keyboard Types</b>\n\nInline buttons below. Reply keyboard at bottom.")
                .keyboard(|kb| kb
                    .button("A", "kb_noop").button("B", "kb_noop").button("C", "kb_noop").row()
                    .url("GitHub", "https://github.com")
                    .switch_inline("Share inline", "test query")
                    .row()
                    .grid(1..=6, 3, |i| (format!("#{}", i), format!("kb_noop:{}", i)))
                    .confirm_cancel("OK", "kb_confirm", "Cancel", "menu")
                )
                .reply_keyboard(vec![vec!["Reply A", "Reply B"], vec!["Reply C"]])
                .build()
            ).await
        }))
        .callback("kb_noop", handler!(ctx => {
            ctx.toast(format!("Pressed: {}", ctx.callback_data().unwrap_or("?"))).await
        }))
        .callback("kb_confirm", handler!(ctx => {
            ctx.alert("Confirmed!").await?;
            ctx.navigate(Screen::text("kb_done", "Keyboard demo done.")
                .remove_reply_keyboard()
                .keyboard(|kb| kb.nav_back("menu"))
                .build()
            ).await
        }))

        // -- Notify & temp notify --
        .callback("notify", handler!(ctx => {
            ctx.notify("This notification will be deleted on next navigate()").await?;
            ctx.notify_temp("This auto-deletes in 3 seconds", std::time::Duration::from_secs(3)).await?;
            Ok(())
        }))

        // -- Protect content --
        .callback("protect", handler!(ctx => {
            ctx.navigate(Screen::text("protected", "This message has <b>protect_content</b> = true.\nTry to forward it — you can't!")
                .protect_content()
                .keyboard(|kb| kb.nav_back("menu"))
                .build()
            ).await
        }))

        // -- Link preview --
        .callback("linkpreview", handler!(ctx => {
            ctx.navigate(Screen::text("linkpreview", "Link preview enabled:\nhttps://github.com/nickel-lang/grammers")
                .link_preview(LinkPreview::Enabled)
                .keyboard(|kb| kb.nav_back("menu"))
                .build()
            ).await
        }))

        // -- Multi-message screen --
        .callback("multimsg", handler!(ctx => {
            ctx.navigate(
                Screen::builder("multimsg")
                    .text("<b>Message 1</b>\nThis is the first message.")
                        .done()
                    .text("<b>Message 2</b>\nSecond message in same screen.")
                        .done()
                    .text("<b>Message 3</b>\nThird. All managed by the differ.")
                        .keyboard(|kb| kb.nav_back("menu"))
                        .done()
                    .build()
            ).await
        }))

        // -- Photo screen --
        .callback("photo", handler!(ctx => {
            ctx.navigate(
                Screen::builder("photo")
                    .photo("https://picsum.photos/400/300")
                        .caption("Random photo from picsum.photos")
                        .keyboard(|kb| kb.button_row("Another", "photo").nav_back("menu"))
                        .done()
                    .build()
            ).await
        }))

        // -- Dice --
        .callback("dice", handler!(ctx => {
            ctx.send_dice(DiceEmoji::Dice).await?;
            Ok(())
        }))
        .callback("darts", handler!(ctx => {
            ctx.send_dice(DiceEmoji::Darts).await?;
            Ok(())
        }))
        .callback("slots", handler!(ctx => {
            ctx.send_dice(DiceEmoji::SlotMachine).await?;
            Ok(())
        }))
        .callback("fun", handler!(ctx => {
            ctx.navigate(Screen::text("fun", "<b>Fun stuff</b>")
                .keyboard(|kb| kb
                    .button("Dice", "dice").button("Darts", "darts").button("Slots", "slots").row()
                    .nav_back("menu")
                ).build()
            ).await
        }))

        // -- Form start --
        .callback("form", handler!(ctx => {
            ctx.set("__form_id", &"feedback".to_string());
            ctx.set("__form_step", &0usize);
            ctx.set("__form_data", &std::collections::HashMap::<String, serde_json::Value>::new());
            ctx.navigate(
                Screen::builder("__form__name")
                    .text("What's your name?")
                    .keyboard(|kb| kb.button_row("Cancel", "__form_cancel"))
                    .build()
            ).await
        }))

        // -- Reaction --
        .callback("react", handler!(ctx => {
            if let Some(mid) = ctx.message_id() {
                let _ = ctx.react(mid, "\u{1f525}").await;
            }
            ctx.toast("Reacted!").await
        }))

        // === INPUT HANDLERS ===
        .callback("ask_input", handler!(ctx => {
            ctx.navigate(
                Screen::builder("awaiting_text")
                    .text("Send me any text message:")
                    .expect_text()
                        .placeholder("Type something...")
                        .validator(|s| if s.is_empty() { Err("Can't be empty".into()) } else { Ok(()) })
                    .build()
            ).await
        }))
        .on_input("awaiting_text", handler!(ctx, text => {
            ctx.navigate(Screen::text("got_text",
                format!("You said: <code>{}</code>\nLength: {}", blazegram::markup::escape(&text), text.len())
            ).keyboard(|kb| kb.button_row("Again", "ask_input").nav_back("menu")).build()).await
        }))

        .callback("ask_media", handler!(ctx => {
            ctx.navigate(
                Screen::builder("awaiting_media")
                    .text("Send me a photo, document, voice, video, or sticker:")
                    .expect_photo() // also triggers media_inputs
                    .build()
            ).await
        }))
        .on_media_input("awaiting_media", handler!(ctx, media => {
            ctx.navigate(Screen::text("got_media",
                format!("Got <b>{:?}</b>\n\nfile_id: <code>{}</code>",
                    media.file_type, &media.file_id[..20.min(media.file_id.len())])
            ).keyboard(|kb| kb.button_row("Again", "ask_media").nav_back("menu")).build()).await
        }))

        // === INLINE MODE ===
        .on_inline(handler!(ctx, query, offset => {
            let all: Vec<InlineQueryResult> = (0..35).map(|i| {
                let text = if query.is_empty() {
                    format!("Result #{}", i + 1)
                } else {
                    format!("{}  #{}", query, i + 1)
                };
                InlineQueryResult {
                    id: i.to_string(),
                    kind: InlineResultKind::Article,
                    title: Some(text.clone()),
                    description: Some(format!("Tap to send result {}", i + 1)),
                    thumb_url: None,
                    message_text: Some(format!("<b>{}</b>\n\nSent via Blazegram showcase bot", blazegram::markup::escape(&text))),
                    parse_mode: ParseMode::Html,
                    keyboard: Some(blazegram::keyboard::InlineKeyboard {
                        rows: vec![vec![blazegram::keyboard::InlineButton {
                            text: "Edit me".into(),
                            action: ButtonAction::Callback("inline_edit".into()),
                        }]],
                    }),
                }
            }).collect();

            let answer = blazegram::inline::InlineAnswer::new(
                all.iter().map(|r| {
                    blazegram::inline::InlineResult::article(&r.id)
                        .title(r.title.as_deref().unwrap_or(""))
                        .description(r.description.as_deref().unwrap_or(""))
                        .build()
                }).collect()
            ).per_page(10).cache_time(5).personal();

            let (page_results, next_off) = answer.paginate(&offset);
            let _ = page_results; // we send our own results with keyboards

            let page: usize = if offset.is_empty() { 0 } else { offset.parse().unwrap_or(0) };
            let start = page * 10;
            let end = (start + 10).min(all.len());
            let page_slice = &all[start..end];

            ctx.answer_inline(
                page_slice.to_vec(),
                if next_off.is_empty() { None } else { Some(next_off) },
                Some(5),
                true,
            ).await
        }))

        .on_chosen_inline(handler!(ctx => {
            tracing::info!(result_id = ?ctx.chosen_inline_result_id(), "chosen inline result");
            Ok(())
        }))

        .callback("inline_edit", handler!(ctx => {
            // Works in both private and inline mode
            ctx.navigate(Screen::text("inline_edited", "<b>Edited via callback!</b>\nThis works in inline mode too.")
                .keyboard(|kb| kb.button_row("Edit again", "inline_edit"))
                .build()
            ).await
        }))

        // === MESSAGE EDITED ===
        .on_message_edited(handler!(ctx, text => {
            ctx.reply(Screen::reply_text(
                format!("You edited a message to: <code>{}</code>", blazegram::markup::escape(&text))
            )).await
        }))

        // === ANY TEXT (catch-all) ===
        .on_any_text(handler!(ctx, text => {
            // Only fires when no screen-specific on_input matches
            ctx.reply(Screen::reply_text(
                format!("Echo: {}", blazegram::markup::escape(&text))
            )).await
        }))

        // === UNRECOGNIZED ===
        .on_unrecognized(handler!(ctx => {
            ctx.notify_temp("Unknown input — try /start", std::time::Duration::from_secs(3)).await
        }))

        // === MEMBER EVENTS ===
        .on_member_joined(handler!(ctx => {
            let name = &ctx.user.first_name;
            ctx.send_text(format!("Welcome, {}!", blazegram::markup::escape(name))).await?;
            Ok(())
        }))
        .on_member_left(handler!(ctx => {
            let name = &ctx.user.first_name;
            ctx.send_text(format!("Goodbye, {}.", blazegram::markup::escape(name))).await?;
            Ok(())
        }))

        .run()
        .await;
}

// === SCREENS ===

fn main_menu(ctx: &Ctx) -> Screen {
    let name = blazegram::markup::escape(&ctx.user.first_name);
    let welcome = ctx.t_with("welcome", &[("name", &name)]);
    Screen::text(
        "menu",
        format!(
            "<b>{}</b>\n\n{}\n\n<i>Blazegram v0.4.0 showcase</i>",
            welcome,
            ctx.t("pick")
        ),
    )
    .keyboard(|kb| {
        kb
            // Row 1: core
            .button("Counter", "counter")
            .button("Navigation", "nav_demo")
            .row()
            // Row 2: content
            .button("Differ", "diff_main")
            .button("Markup", "markup")
            .row()
            .button("Template", "template")
            .button("Pagination", "paginate")
            .row()
            // Row 3: modes
            .button("Progressive", "progressive")
            .button("Reply", "reply_demo")
            .row()
            // Row 4: features
            .button("Freeze msg", "freeze_demo")
            .button("Permanent", "permanent")
            .row()
            .button("Keyboards", "keyboards")
            .button("Multi-msg", "multimsg")
            .row()
            .button("Photo", "photo")
            .button("Link preview", "linkpreview")
            .row()
            .button("Protect", "protect")
            .button("Notify", "notify")
            .row()
            // Row 5: interactive
            .button("Fun (dice)", "fun")
            .button("React", "react")
            .row()
            // Row 6: input
            .button("Text input", "ask_input")
            .button("Media input", "ask_media")
            .row()
            // Row 7: form
            .button_row("Form wizard", "form")
            // Row 8: stats
            .button_row("/stats", "show_stats")
    })
    .build()
}

fn counter_screen(count: i64) -> Screen {
    Screen::text(
        "counter",
        format!("<b>Counter</b>\n\nValue: <code>{}</code>", count),
    )
    .keyboard(|kb| {
        kb.button("-1", "dec")
            .button("Reset", "reset")
            .button("+1", "inc")
            .row()
            .nav_back("menu")
    })
    .build()
}

fn make_page(pag: &Paginator<String>) -> Screen {
    paginated_screen(
        "paginated",
        "Items",
        pag,
        |_idx, item| (item.clone(), format!("kb_noop:{}", item)),
        "page",
        "menu",
    )
}

const HELP_TEXT: &str = "\
<b>Showcase Bot Commands</b>\n\
/start — Main menu\n\
/start &lt;payload&gt; — Deep link demo\n\
/help — This message\n\
/stats — Analytics & metrics\n\
/dice — Roll a dice\n\
/typing — Typing indicator\n\
\n\
<b>Features covered:</b>\n\
• Screen navigation & Virtual Chat Differ\n\
• Push/pop navigation stack\n\
• Typed state (counter)\n\
• Custom markup (bold/italic/code/links/spoiler)\n\
• Template engine (for/if/vars)\n\
• Pagination (47 items, 8 per page)\n\
• Progressive updates (streaming)\n\
• Reply mode (send-once, edit-on-repeat)\n\
• Frozen & permanent messages\n\
• Inline keyboards (buttons, grid, url, switch_inline)\n\
• Reply keyboards (bottom)\n\
• Toast & alert popups\n\
• Notify & notify_temp\n\
• Protect content\n\
• Link preview\n\
• Multi-message screens\n\
• Photo screens\n\
• Dice/darts/slots\n\
• Reactions\n\
• Form wizard (text+int+choice+confirm)\n\
• Text & media input handlers\n\
• Inline mode (35 results, paginated)\n\
• Chosen inline result\n\
• Message edited handler\n\
• Any-text catch-all\n\
• Unrecognized input handler\n\
• Member joined/left\n\
• i18n (en/ru)\n\
• Middleware (logging + analytics)\n\
• Metrics\n\
• State snapshots\n\
• Deep links\n\
• Typing indicator";

async fn stats_handler(ctx: &mut Ctx, analytics: &AnalyticsMiddleware) -> Result<(), HandlerError> {
    let (total, msgs, cbs, users) = analytics.stats();
    let m = metrics();
    ctx.navigate(
        Screen::text(
            "stats",
            format!(
                "<b>Analytics</b>\n\nUpdates: {}\nMessages: {}\nCallbacks: {}\nUnique users: {}\n\n<b>Metrics</b>\n<pre>{}</pre>",
                total, msgs, cbs, users, blazegram::markup::escape(&m.summary())
            ),
        )
        .keyboard(|kb| kb.button_row("Menu", "menu"))
        .build(),
    )
    .await
}
