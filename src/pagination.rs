//! Pagination helper.

use serde::{Deserialize, Serialize};

use crate::screen::Screen;
use crate::types::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Configures how a list of items is split into pages.
pub struct Paginator<T> {
    /// The items on the current page.
    pub items: Vec<T>,
    /// Page size.
    pub page_size: usize,
    /// Current page.
    pub current_page: usize,
}

impl<T> Paginator<T> {
    /// Create a new paginator.
    pub fn new(items: Vec<T>, page_size: usize) -> Self {
        Self {
            items,
            page_size,
            current_page: 0,
        }
    }

    /// Total number of pages.
    pub fn total_pages(&self) -> usize {
        if self.items.is_empty() {
            return 1;
        }
        self.items.len().div_ceil(self.page_size)
    }

    /// Current items.
    pub fn current_items(&self) -> &[T] {
        let start = self.current_page * self.page_size;
        let end = (start + self.page_size).min(self.items.len());
        if start >= self.items.len() {
            return &[];
        }
        &self.items[start..end]
    }

    /// Set page.
    pub fn set_page(&mut self, page: usize) {
        self.current_page = page.min(self.total_pages().saturating_sub(1));
    }

    /// Has prev.
    pub fn has_prev(&self) -> bool {
        self.current_page > 0
    }

    /// Has next.
    pub fn has_next(&self) -> bool {
        self.current_page + 1 < self.total_pages()
    }
}

/// Build a paginated screen from a Paginator.
///
/// `item_formatter` returns (display_text, callback_data) for each item.
pub fn paginated_screen<T, F>(
    id: impl Into<ScreenId>,
    title: &str,
    paginator: &Paginator<T>,
    item_formatter: F,
    page_callback_prefix: &str,
    back_callback: &str,
) -> Screen
where
    F: Fn(usize, &T) -> (String, String),
{
    let items = paginator.current_items();

    // Pre-format everything into owned data so the keyboard closure is 'static
    let mut text = format!("<b>{}</b>\n\n", crate::markup::escape(title));
    let mut buttons: Vec<(String, String)> = Vec::with_capacity(items.len());
    for (i, item) in items.iter().enumerate() {
        let global_idx = paginator.current_page * paginator.page_size + i;
        let (display, data) = item_formatter(global_idx, item);
        text.push_str(&format!("{}. {}\n", global_idx + 1, display));
        buttons.push((display, data));
    }

    let page = paginator.current_page;
    let total = paginator.total_pages();
    let prefix = page_callback_prefix.to_string();
    let back = back_callback.to_string();

    Screen::builder(id)
        .text(text)
        .keyboard(move |kb| {
            let mut kb = kb;
            for (display, data) in &buttons {
                kb = kb.button_row(display.clone(), data.clone());
            }
            kb = kb.pagination(page, total, &prefix);
            kb.nav_back(back.clone())
        })
        .build()
}
