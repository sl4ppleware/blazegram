use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};

use crate::i18n::ft;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InlineKeyboard {
    pub rows: Vec<Vec<InlineButton>>,
}

impl Hash for InlineKeyboard {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for row in &self.rows {
            for btn in row {
                btn.hash(state);
            }
        }
    }
}

#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct InlineButton {
    pub text: String,
    pub action: ButtonAction,
}

#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub enum ButtonAction {
    Callback(String),
    Url(String),
    WebApp(String),
    SwitchInline { query: String, current_chat: bool },
}

// ─── Builder ───

pub struct KeyboardBuilder {
    rows: Vec<Vec<InlineButton>>,
    current_row: Vec<InlineButton>,
    lang: String,
}

impl KeyboardBuilder {
    pub fn new() -> Self {
        Self { rows: Vec::new(), current_row: Vec::new(), lang: "en".into() }
    }

    /// Create a builder with a specific language for framework labels.
    pub fn with_lang(lang: impl Into<String>) -> Self {
        Self { rows: Vec::new(), current_row: Vec::new(), lang: lang.into() }
    }

    /// Add callback button to current row.
    pub fn button(mut self, text: impl Into<String>, callback: impl Into<String>) -> Self {
        self.current_row.push(InlineButton {
            text: text.into(),
            action: ButtonAction::Callback(callback.into()),
        });
        self
    }

    /// Add URL button to current row.
    pub fn url(mut self, text: impl Into<String>, url: impl Into<String>) -> Self {
        self.current_row.push(InlineButton {
            text: text.into(),
            action: ButtonAction::Url(url.into()),
        });
        self
    }

    /// Add WebApp button to current row.
    pub fn webapp(mut self, text: impl Into<String>, url: impl Into<String>) -> Self {
        self.current_row.push(InlineButton {
            text: text.into(),
            action: ButtonAction::WebApp(url.into()),
        });
        self
    }

    /// Add switch-inline button (opens inline query in another chat).
    pub fn switch_inline(mut self, text: impl Into<String>, query: impl Into<String>) -> Self {
        self.current_row.push(InlineButton {
            text: text.into(),
            action: ButtonAction::SwitchInline { query: query.into(), current_chat: false },
        });
        self
    }

    /// Add switch-inline-current-chat button.
    pub fn switch_inline_current(mut self, text: impl Into<String>, query: impl Into<String>) -> Self {
        self.current_row.push(InlineButton {
            text: text.into(),
            action: ButtonAction::SwitchInline { query: query.into(), current_chat: true },
        });
        self
    }

    /// End current row, start new one.
    pub fn row(mut self) -> Self {
        if !self.current_row.is_empty() {
            self.rows.push(std::mem::take(&mut self.current_row));
        }
        self
    }

    /// Single button on its own row.
    pub fn button_row(self, text: impl Into<String>, callback: impl Into<String>) -> Self {
        self.button(text, callback).row()
    }

    /// Build a grid of buttons from items.
    pub fn grid<I, F>(mut self, items: I, columns: usize, f: F) -> Self
    where
        I: IntoIterator,
        F: Fn(I::Item) -> (String, String),
    {
        let mut count = 0;
        for item in items {
            let (text, data) = f(item);
            self = self.button(text, data);
            count += 1;
            if count % columns == 0 {
                self = self.row();
            }
        }
        if count % columns != 0 {
            self = self.row();
        }
        self
    }

    /// Pagination row: ← [2/5] →
    ///
    /// Labels come from `bg-nav-prev` / `bg-nav-next` i18n keys.
    pub fn pagination(self, page: usize, total_pages: usize, prefix: &str) -> Self {
        if total_pages <= 1 {
            return self;
        }
        let lang = self.lang.clone();
        let mut b = self;
        if page > 0 {
            b = b.button(ft(&lang, "bg-nav-prev"), format!("{}:{}", prefix, page - 1));
        }
        b = b.button(format!("{}/{}", page + 1, total_pages), "_noop");
        if page < total_pages - 1 {
            b = b.button(ft(&lang, "bg-nav-next"), format!("{}:{}", prefix, page + 1));
        }
        b.row()
    }

    /// Back button row. Label comes from `bg-nav-back` i18n key.
    pub fn nav_back(self, callback: impl Into<String>) -> Self {
        let label = ft(&self.lang, "bg-nav-back");
        self.button_row(label, callback)
    }

    /// Confirm / Cancel row.
    pub fn confirm_cancel(
        self,
        confirm_text: impl Into<String>,
        confirm_cb: impl Into<String>,
        cancel_text: impl Into<String>,
        cancel_cb: impl Into<String>,
    ) -> Self {
        self.button(confirm_text, confirm_cb)
            .button(cancel_text, cancel_cb)
            .row()
    }

    pub fn build(mut self) -> InlineKeyboard {
        if !self.current_row.is_empty() {
            self.rows.push(self.current_row);
        }
        InlineKeyboard { rows: self.rows }
    }
}

impl Default for KeyboardBuilder {
    fn default() -> Self {
        Self::new()
    }
}
