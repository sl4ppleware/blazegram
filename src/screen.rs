use crate::keyboard::{InlineKeyboard, KeyboardBuilder};
use crate::types::*;

/// A screen — the atomic unit of UI. What the user sees right now.
#[derive(Debug, Clone)]
pub struct Screen {
    pub id: ScreenId,
    pub messages: Vec<ScreenMessage>,
    pub input: Option<InputSpec>,
    pub typing_action: Option<ChatAction>,
    /// Protect content from forwarding/saving.
    pub protect_content: bool,
    /// Reply keyboard (bottom keyboard). None = don't change, Some(Remove) = remove.
    pub reply_keyboard: Option<ReplyKeyboardAction>,
    /// Reply to a specific message.
    pub reply_to: Option<MessageId>,
}

/// What to do with the reply (bottom) keyboard.
#[derive(Debug, Clone)]
pub enum ReplyKeyboardAction {
    /// Show a reply keyboard with these buttons.
    Show {
        rows: Vec<Vec<ReplyButton>>,
        resize: bool,
        one_time: bool,
        placeholder: Option<String>,
    },
    /// Remove the reply keyboard.
    Remove,
}

#[derive(Debug, Clone)]
pub struct ReplyButton {
    pub text: String,
    pub request_contact: bool,
    pub request_location: bool,
}

#[derive(Debug, Clone)]
pub struct ScreenMessage {
    pub content: MessageContent,
}

impl Screen {
    pub fn builder(id: impl Into<ScreenId>) -> ScreenBuilder {
        ScreenBuilder {
            id: id.into(),
            messages: Vec::new(),
            input: None,
            typing_action: None,
            protect_content: false,
            reply_keyboard: None,
            reply_to: None,
            lang: None,
        }
    }

    /// Shortcut: single text message screen.
    pub fn text(id: impl Into<ScreenId>, text: impl Into<String>) -> ScreenTextBuilder {
        Self::builder(id).text(text)
    }

    /// Quick text screen with auto-generated ID. Ideal for `ctx.reply()`.
    pub fn reply_text(text: impl Into<String>) -> Screen {
        Self::builder("__reply__").text(text).build()
    }
}

// ─── Screen Builder ───

pub struct ScreenBuilder {
    id: ScreenId,
    messages: Vec<ScreenMessage>,
    input: Option<InputSpec>,
    typing_action: Option<ChatAction>,
    protect_content: bool,
    reply_keyboard: Option<ReplyKeyboardAction>,
    reply_to: Option<MessageId>,
    /// Language code for framework-generated labels (pagination, nav_back, etc).
    lang: Option<String>,
}

impl ScreenBuilder {
    pub fn text(self, text: impl Into<String>) -> ScreenTextBuilder {
        ScreenTextBuilder {
            parent: self,
            text: text.into(),
            parse_mode: ParseMode::Html,
            keyboard: None,
            link_preview: LinkPreview::Disabled,
        }
    }

    pub fn markup(self, text: impl Into<String>) -> ScreenTextBuilder {
        let processed = crate::markup::render(&text.into());
        ScreenTextBuilder {
            parent: self,
            text: processed,
            parse_mode: ParseMode::Html,
            keyboard: None,
            link_preview: LinkPreview::Disabled,
        }
    }

    pub fn photo(self, source: impl Into<FileSource>) -> ScreenMediaBuilder {
        ScreenMediaBuilder {
            parent: self,
            media_type: ContentType::Photo,
            source: source.into(),
            caption: None,
            parse_mode: ParseMode::Html,
            keyboard: None,
            spoiler: false,
        }
    }

    pub fn video(self, source: impl Into<FileSource>) -> ScreenMediaBuilder {
        ScreenMediaBuilder {
            parent: self,
            media_type: ContentType::Video,
            source: source.into(),
            caption: None,
            parse_mode: ParseMode::Html,
            keyboard: None,
            spoiler: false,
        }
    }

    pub fn document(self, source: impl Into<FileSource>) -> ScreenMediaBuilder {
        ScreenMediaBuilder {
            parent: self,
            media_type: ContentType::Document,
            source: source.into(),
            caption: None,
            parse_mode: ParseMode::Html,
            keyboard: None,
            spoiler: false,
        }
    }

    pub fn expect_text(self) -> ScreenInputBuilder {
        ScreenInputBuilder {
            parent: self,
            validator: None,
            placeholder: None,
        }
    }

    pub fn expect_photo(mut self) -> Self {
        self.input = Some(InputSpec::Photo);
        self
    }

    pub fn expect_choice(mut self, options: Vec<String>) -> Self {
        self.input = Some(InputSpec::Choice { options });
        self
    }

    /// Set the language code for framework-generated labels.
    /// Propagated to `KeyboardBuilder` in `.keyboard()` closures.
    pub fn lang(mut self, lang: impl Into<String>) -> Self {
        self.lang = Some(lang.into());
        self
    }

    pub fn typing(mut self) -> Self {
        self.typing_action = Some(ChatAction::Typing);
        self
    }

    pub fn protect_content(mut self) -> Self {
        self.protect_content = true;
        self
    }

    /// Set a reply (bottom) keyboard.
    pub fn reply_keyboard(mut self, rows: Vec<Vec<&str>>) -> Self {
        self.reply_keyboard = Some(ReplyKeyboardAction::Show {
            rows: rows.into_iter().map(|row| {
                row.into_iter().map(|t| ReplyButton {
                    text: t.to_string(),
                    request_contact: false,
                    request_location: false,
                }).collect()
            }).collect(),
            resize: true,
            one_time: false,
            placeholder: None,
        });
        self
    }

    /// Remove the reply keyboard.
    pub fn remove_reply_keyboard(mut self) -> Self {
        self.reply_keyboard = Some(ReplyKeyboardAction::Remove);
        self
    }

    /// Reply to a specific message when sending.
    pub fn reply_to(mut self, message_id: MessageId) -> Self {
        self.reply_to = Some(message_id);
        self
    }

    pub fn build(self) -> Screen {
        if self.messages.is_empty() {
            tracing::warn!(
                screen_id = %self.id,
                "Screen::build() called with no messages — this screen will be a no-op. \
                 Did you forget to call .text() or .photo()?"
            );
        }
        Screen {
            id: self.id,
            messages: self.messages,
            input: self.input,
            typing_action: self.typing_action,
            protect_content: self.protect_content,
            reply_keyboard: self.reply_keyboard,
            reply_to: self.reply_to,
        }
    }
}

// ─── Text sub-builder ───

pub struct ScreenTextBuilder {
    parent: ScreenBuilder,
    text: String,
    parse_mode: ParseMode,
    keyboard: Option<InlineKeyboard>,
    link_preview: LinkPreview,
}

impl ScreenTextBuilder {
    pub fn parse_mode(mut self, pm: ParseMode) -> Self {
        self.parse_mode = pm;
        self
    }

    pub fn link_preview(mut self, lp: LinkPreview) -> Self {
        self.link_preview = lp;
        self
    }

    pub fn keyboard(mut self, f: impl FnOnce(KeyboardBuilder) -> KeyboardBuilder) -> Self {
        let kb_builder = match &self.parent.lang {
            Some(lang) => KeyboardBuilder::with_lang(lang.clone()),
            None => KeyboardBuilder::new(),
        };
        let kb = f(kb_builder);
        self.keyboard = Some(kb.build());
        self
    }

    /// Finish this text message, return to ScreenBuilder.
    pub fn done(mut self) -> ScreenBuilder {
        self.parent.messages.push(ScreenMessage {
            content: MessageContent::Text {
                text: self.text,
                parse_mode: self.parse_mode,
                keyboard: self.keyboard,
                link_preview: self.link_preview,
            },
        });
        self.parent
    }

    pub fn build(self) -> Screen {
        self.done().build()
    }

    /// Finalize this message and return to ScreenBuilder for adding more.
    pub fn build_msg(self) -> ScreenBuilder {
        self.done()
    }

    pub fn protect_content(mut self) -> Self {
        self.parent.protect_content = true;
        self
    }

    pub fn reply_keyboard(mut self, rows: Vec<Vec<&str>>) -> Self {
        self.parent = self.parent.reply_keyboard(rows);
        self
    }

    pub fn remove_reply_keyboard(mut self) -> Self {
        self.parent = self.parent.remove_reply_keyboard();
        self
    }

    /// Chain: add another text message.
    pub fn text(self, text: impl Into<String>) -> ScreenTextBuilder {
        self.done().text(text)
    }

    /// Chain: add photo message.
    pub fn photo(self, source: impl Into<FileSource>) -> ScreenMediaBuilder {
        self.done().photo(source)
    }

    /// Chain: expect text input.
    pub fn expect_text(self) -> ScreenInputBuilder {
        self.done().expect_text()
    }

    /// Chain: expect photo input.
    pub fn expect_photo(self) -> ScreenBuilder {
        self.done().expect_photo()
    }
}

// ─── Media sub-builder ───

pub struct ScreenMediaBuilder {
    parent: ScreenBuilder,
    media_type: ContentType,
    source: FileSource,
    caption: Option<String>,
    parse_mode: ParseMode,
    keyboard: Option<InlineKeyboard>,
    spoiler: bool,
}

impl ScreenMediaBuilder {
    pub fn caption(mut self, cap: impl Into<String>) -> Self {
        self.caption = Some(cap.into());
        self
    }

    pub fn spoiler(mut self) -> Self {
        self.spoiler = true;
        self
    }

    pub fn keyboard(mut self, f: impl FnOnce(KeyboardBuilder) -> KeyboardBuilder) -> Self {
        let kb_builder = match &self.parent.lang {
            Some(lang) => KeyboardBuilder::with_lang(lang.clone()),
            None => KeyboardBuilder::new(),
        };
        let kb = f(kb_builder);
        self.keyboard = Some(kb.build());
        self
    }

    pub fn done(mut self) -> ScreenBuilder {
        let content = match self.media_type {
            ContentType::Photo => MessageContent::Photo {
                source: self.source,
                caption: self.caption,
                parse_mode: self.parse_mode,
                keyboard: self.keyboard,
                spoiler: self.spoiler,
            },
            ContentType::Video => MessageContent::Video {
                source: self.source,
                caption: self.caption,
                parse_mode: self.parse_mode,
                keyboard: self.keyboard,
                spoiler: self.spoiler,
            },
            ContentType::Document => MessageContent::Document {
                source: self.source,
                caption: self.caption,
                parse_mode: self.parse_mode,
                keyboard: self.keyboard,
                filename: None,
            },
            _ => unreachable!("ScreenMediaBuilder only supports Photo/Video/Document"),
        };
        self.parent.messages.push(ScreenMessage { content });
        self.parent
    }

    pub fn build(self) -> Screen {
        self.done().build()
    }

    pub fn text(self, text: impl Into<String>) -> ScreenTextBuilder {
        self.done().text(text)
    }
}

// ─── Input sub-builder ───

pub struct ScreenInputBuilder {
    parent: ScreenBuilder,
    validator: Option<ValidatorFn>,
    placeholder: Option<String>,
}

impl ScreenInputBuilder {
    pub fn validator(
        mut self,
        f: impl Fn(&str) -> Result<(), String> + Send + Sync + 'static,
    ) -> Self {
        self.validator = Some(std::sync::Arc::new(f));
        self
    }

    pub fn placeholder(mut self, p: impl Into<String>) -> Self {
        self.placeholder = Some(p.into());
        self
    }

    pub fn build(mut self) -> Screen {
        self.parent.input = Some(InputSpec::Text {
            validator: self.validator,
            placeholder: self.placeholder,
        });
        self.parent.build()
    }

    pub fn done(mut self) -> ScreenBuilder {
        self.parent.input = Some(InputSpec::Text {
            validator: self.validator,
            placeholder: self.placeholder,
        });
        self.parent
    }
}
