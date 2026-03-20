use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};

use crate::i18n::ft;

/// An inline keyboard attached below a message.
///
/// Build one with [`KeyboardBuilder`] (usually via the `.keyboard()` closure
/// on screen builders).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InlineKeyboard {
    /// Rows of buttons (outer = rows, inner = buttons in a row).
    pub rows: Vec<Vec<InlineButton>>,
}

impl Hash for InlineKeyboard {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.rows.len().hash(state);
        for row in &self.rows {
            row.len().hash(state);
            for btn in row {
                btn.hash(state);
            }
        }
    }
}

/// A single inline keyboard button.
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct InlineButton {
    /// Label displayed on the button.
    pub text: String,
    /// What happens when the button is pressed.
    pub action: ButtonAction,
}

/// What an [`InlineButton`] does when pressed.
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub enum ButtonAction {
    /// Send a callback query with this data string to the bot.
    Callback(String),
    /// Open a URL in the user’s browser.
    Url(String),
    /// Open a [Web App](https://core.telegram.org/bots/webapps).
    WebApp(String),
    /// Prompt the user to choose a chat and insert an inline query.
    SwitchInline {
        /// Pre-filled inline query text.
        query: String,
        /// If `true`, insert in the current chat instead of choosing.
        current_chat: bool,
    },
}

// ─── Builder ───

/// Fluent builder for constructing an [`InlineKeyboard`].
///
/// Buttons are added to the "current row". Call [`.row()`](Self::row) to
/// start a new row, or use [`.button_row()`](Self::button_row) for
/// single-button rows.
pub struct KeyboardBuilder {
    rows: Vec<Vec<InlineButton>>,
    current_row: Vec<InlineButton>,
    lang: String,
}

impl KeyboardBuilder {
    /// Create a new builder with default language (`"en"`).
    pub fn new() -> Self {
        Self {
            rows: Vec::new(),
            current_row: Vec::new(),
            lang: "en".into(),
        }
    }

    /// Create a builder with a specific language for framework labels.
    pub fn with_lang(lang: impl Into<String>) -> Self {
        Self {
            rows: Vec::new(),
            current_row: Vec::new(),
            lang: lang.into(),
        }
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
            action: ButtonAction::SwitchInline {
                query: query.into(),
                current_chat: false,
            },
        });
        self
    }

    /// Add switch-inline-current-chat button.
    pub fn switch_inline_current(
        mut self,
        text: impl Into<String>,
        query: impl Into<String>,
    ) -> Self {
        self.current_row.push(InlineButton {
            text: text.into(),
            action: ButtonAction::SwitchInline {
                query: query.into(),
                current_chat: true,
            },
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
        if columns == 0 {
            return self;
        }
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

    /// Consume the builder and produce an [`InlineKeyboard`].
    ///
    /// Any buttons remaining in the current row are flushed automatically.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_button_and_row() {
        let kb = KeyboardBuilder::new()
            .button("Click", "action:click")
            .row()
            .button("Back", "nav:back")
            .build();
        assert_eq!(kb.rows.len(), 2);
        assert_eq!(kb.rows[0].len(), 1);
        assert_eq!(kb.rows[0][0].text, "Click");
        assert_eq!(kb.rows[1][0].text, "Back");
    }

    #[test]
    fn builder_grid() {
        let items = vec![("a", "1"), ("b", "2"), ("c", "3"), ("d", "4"), ("e", "5")];
        let kb = KeyboardBuilder::new()
            .grid(items, 2, |(t, d)| (t.to_string(), d.to_string()))
            .build();
        assert_eq!(kb.rows.len(), 3); // [a,b], [c,d], [e]
        assert_eq!(kb.rows[0].len(), 2);
        assert_eq!(kb.rows[2].len(), 1);
    }

    #[test]
    fn builder_url_button() {
        let kb = KeyboardBuilder::new()
            .url("Google", "https://google.com")
            .build();
        assert_eq!(kb.rows[0][0].text, "Google");
        assert!(matches!(&kb.rows[0][0].action, ButtonAction::Url(u) if u == "https://google.com"));
    }

    #[test]
    fn builder_webapp_button() {
        let kb = KeyboardBuilder::new()
            .webapp("App", "https://app.example.com")
            .build();
        assert!(
            matches!(&kb.rows[0][0].action, ButtonAction::WebApp(u) if u == "https://app.example.com")
        );
    }

    #[test]
    fn empty_builder_builds_empty() {
        let kb = KeyboardBuilder::new().build();
        assert!(kb.rows.is_empty());
    }

    #[test]
    fn button_row_creates_single_button_row() {
        let kb = KeyboardBuilder::new()
            .button_row("Solo", "solo_data")
            .button_row("Another", "another_data")
            .build();
        assert_eq!(kb.rows.len(), 2);
        assert_eq!(kb.rows[0].len(), 1);
        assert_eq!(kb.rows[0][0].text, "Solo");
    }

    #[test]
    fn confirm_cancel_row() {
        let kb = KeyboardBuilder::new()
            .confirm_cancel("Yes", "confirm", "No", "cancel")
            .build();
        assert_eq!(kb.rows.len(), 1);
        assert_eq!(kb.rows[0].len(), 2);
        assert_eq!(kb.rows[0][0].text, "Yes");
        assert_eq!(kb.rows[0][1].text, "No");
    }

    #[test]
    fn switch_inline_button() {
        let kb = KeyboardBuilder::new()
            .switch_inline("Search", "query")
            .build();
        assert!(matches!(
            &kb.rows[0][0].action,
            ButtonAction::SwitchInline { query, current_chat: false } if query == "query"
        ));
    }

    #[test]
    fn switch_inline_current_button() {
        let kb = KeyboardBuilder::new()
            .switch_inline_current("Here", "inline")
            .build();
        assert!(matches!(
            &kb.rows[0][0].action,
            ButtonAction::SwitchInline {
                current_chat: true,
                ..
            }
        ));
    }

    #[test]
    fn pagination_single_page_no_buttons() {
        let kb = KeyboardBuilder::new().pagination(0, 1, "page").build();
        assert!(kb.rows.is_empty());
    }

    #[test]
    fn pagination_multi_page() {
        let kb = KeyboardBuilder::new().pagination(0, 3, "page").build();
        // First page: [1/3] [>]
        assert_eq!(kb.rows.len(), 1);
        assert_eq!(kb.rows[0].len(), 2); // counter + next
    }

    #[test]
    fn grid_zero_columns_no_panic() {
        let items = vec![("a", "1"), ("b", "2")];
        let kb = KeyboardBuilder::new()
            .grid(items, 0, |(t, d)| (t.to_string(), d.to_string()))
            .build();
        assert!(kb.rows.is_empty(), "grid(0) should return builder unchanged");
    }

    #[test]
    fn keyboard_hash_encodes_row_boundaries() {
        use std::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;

        // [[A,B],[C]] vs [[A],[B,C]]
        let kb1 = KeyboardBuilder::new()
            .button("A", "a").button("B", "b").row()
            .button("C", "c").build();
        let kb2 = KeyboardBuilder::new()
            .button("A", "a").row()
            .button("B", "b").button("C", "c").build();

        let hash = |kb: &InlineKeyboard| {
            let mut h = DefaultHasher::new();
            kb.hash(&mut h);
            h.finish()
        };
        assert_ne!(hash(&kb1), hash(&kb2), "different row layouts must produce different hashes");
    }
}
