//! Inline mode support — declarative result builders and auto-pagination.
//!
//! Build inline query results with a fluent API, then use `InlineAnswer` to
//! auto-paginate them based on Telegram's offset mechanism.
//!
//! # Example
//!
//! ```ignore
//! let results = vec![
//!     InlineResult::article("1")
//!         .title("Hello")
//!         .description("A greeting")
//!         .screen(Screen::text("greet", "Hello, world!").build())
//!         .build(),
//!     InlineResult::photo("2", "https://example.com/pic.jpg")
//!         .title("Nice pic")
//!         .build(),
//! ];
//!
//! let answer = InlineAnswer::new(results).per_page(10).cache_time(60);
//! let (page, next_offset) = answer.paginate(&query.offset);
//! ```

use crate::screen::Screen;

// ─── InlineResult ───

/// A single inline query result.
#[derive(Debug, Clone)]
pub struct InlineResult {
    /// Unique identifier for this result (1–64 bytes).
    pub id: String,
    /// What kind of result this is.
    pub kind: InlineResultKind,
    /// Title shown in the result list.
    pub title: Option<String>,
    /// Description shown below the title.
    pub description: Option<String>,
    /// Thumbnail URL shown next to the result.
    pub thumbnail_url: Option<String>,
    /// Screen content (text + keyboard) for the message that will be sent
    /// when the user taps this result.
    pub screen: Option<Screen>,
}

/// The content type of an inline result.
#[derive(Debug, Clone)]
pub enum InlineResultKind {
    /// Text article (most common).
    Article,
    /// Photo result.
    Photo {
        /// Direct URL to the photo.
        url: String,
    },
    /// GIF animation.
    Gif {
        /// Direct URL to the GIF.
        url: String,
    },
    /// Video result.
    Video {
        /// Direct URL to the video.
        url: String,
        /// MIME type (e.g. `"video/mp4"`).
        mime: String,
    },
    /// Voice message.
    Voice {
        /// Direct URL to the OGG audio.
        url: String,
    },
    /// Document / file.
    Document {
        /// Direct URL to the document.
        url: String,
        /// MIME type.
        mime: String,
    },
}

impl InlineResult {
    /// Start building an article result.
    pub fn article(id: impl Into<String>) -> InlineResultBuilder {
        InlineResultBuilder {
            id: id.into(),
            kind: InlineResultKind::Article,
            title: None,
            description: None,
            thumbnail_url: None,
            screen: None,
        }
    }

    /// Start building a photo result.
    pub fn photo(id: impl Into<String>, url: impl Into<String>) -> InlineResultBuilder {
        InlineResultBuilder {
            id: id.into(),
            kind: InlineResultKind::Photo { url: url.into() },
            title: None,
            description: None,
            thumbnail_url: None,
            screen: None,
        }
    }

    /// Start building a GIF result.
    pub fn gif(id: impl Into<String>, url: impl Into<String>) -> InlineResultBuilder {
        InlineResultBuilder {
            id: id.into(),
            kind: InlineResultKind::Gif { url: url.into() },
            title: None,
            description: None,
            thumbnail_url: None,
            screen: None,
        }
    }

    /// Start building a video result.
    pub fn video(
        id: impl Into<String>,
        url: impl Into<String>,
        mime: impl Into<String>,
    ) -> InlineResultBuilder {
        InlineResultBuilder {
            id: id.into(),
            kind: InlineResultKind::Video {
                url: url.into(),
                mime: mime.into(),
            },
            title: None,
            description: None,
            thumbnail_url: None,
            screen: None,
        }
    }

    /// Start building a voice result.
    pub fn voice(id: impl Into<String>, url: impl Into<String>) -> InlineResultBuilder {
        InlineResultBuilder {
            id: id.into(),
            kind: InlineResultKind::Voice { url: url.into() },
            title: None,
            description: None,
            thumbnail_url: None,
            screen: None,
        }
    }

    /// Start building a document result.
    pub fn document(
        id: impl Into<String>,
        url: impl Into<String>,
        mime: impl Into<String>,
    ) -> InlineResultBuilder {
        InlineResultBuilder {
            id: id.into(),
            kind: InlineResultKind::Document {
                url: url.into(),
                mime: mime.into(),
            },
            title: None,
            description: None,
            thumbnail_url: None,
            screen: None,
        }
    }
}

// ─── InlineResultBuilder ───

/// Fluent builder for [`InlineResult`].
pub struct InlineResultBuilder {
    id: String,
    kind: InlineResultKind,
    title: Option<String>,
    description: Option<String>,
    thumbnail_url: Option<String>,
    screen: Option<Screen>,
}

impl InlineResultBuilder {
    /// Set the title displayed in the result list.
    pub fn title(mut self, t: impl Into<String>) -> Self {
        self.title = Some(t.into());
        self
    }

    /// Set the description displayed below the title.
    pub fn description(mut self, d: impl Into<String>) -> Self {
        self.description = Some(d.into());
        self
    }

    /// Set the thumbnail URL.
    pub fn thumb(mut self, url: impl Into<String>) -> Self {
        self.thumbnail_url = Some(url.into());
        self
    }

    /// Set the screen that will be sent when this result is chosen.
    ///
    /// The first message in the screen is used as the inline result’s content
    /// (text + inline keyboard).
    pub fn screen(mut self, s: Screen) -> Self {
        self.screen = Some(s);
        self
    }

    /// Build the [`InlineResult`].
    pub fn build(self) -> InlineResult {
        InlineResult {
            id: self.id,
            kind: self.kind,
            title: self.title,
            description: self.description,
            thumbnail_url: self.thumbnail_url,
            screen: self.screen,
        }
    }
}

// ─── InlineAnswer ───

/// An answer to an inline query, with automatic offset-based pagination.
///
/// Telegram inline queries support pagination via the `offset` field: the bot
/// sends a page of results plus a `next_offset` string; when the user scrolls
/// down, Telegram re-sends the query with that offset.
///
/// `InlineAnswer` handles this for you — just provide all results and call
/// [`paginate`](Self::paginate) with the raw offset from the query.
pub struct InlineAnswer {
    /// All results (the full set, before pagination).
    pub results: Vec<InlineResult>,
    /// Max results per page. Telegram allows up to 50.
    pub per_page: usize,
    /// How long (seconds) Telegram should cache results. 0 = no caching.
    pub cache_time: i32,
    /// Whether results are specific to the querying user.
    pub is_personal: bool,
    /// If set, a button is shown above results that switches to PM.
    pub switch_pm_text: Option<String>,
    /// Deep-link parameter sent with the PM /start command.
    pub switch_pm_parameter: Option<String>,
}

impl InlineAnswer {
    /// Create a new answer with the given results.
    ///
    /// Defaults: 20 results per page, 300s cache, not personal.
    pub fn new(results: Vec<InlineResult>) -> Self {
        Self {
            results,
            per_page: 20,
            cache_time: 300,
            is_personal: false,
            switch_pm_text: None,
            switch_pm_parameter: None,
        }
    }

    /// Set the number of results per page (max 50).
    pub fn per_page(mut self, n: usize) -> Self {
        self.per_page = n.clamp(1, 50);
        self
    }

    /// Set the cache time in seconds.
    pub fn cache_time(mut self, secs: i32) -> Self {
        self.cache_time = secs;
        self
    }

    /// Mark results as personal (different per user).
    pub fn personal(mut self) -> Self {
        self.is_personal = true;
        self
    }

    /// Set a "switch to PM" button.
    pub fn switch_pm(mut self, text: impl Into<String>, parameter: impl Into<String>) -> Self {
        self.switch_pm_text = Some(text.into());
        self.switch_pm_parameter = Some(parameter.into());
        self
    }

    /// Paginate results based on the raw offset string from the inline query.
    ///
    /// Returns a tuple of:
    /// - The results for this page (references into `self.results`).
    /// - The `next_offset` string to include in the answer. Empty string means
    ///   no more pages.
    ///
    /// The offset is a simple stringified page index ("0", "1", "2", ...).
    /// An empty offset string is treated as page 0.
    pub fn paginate(&self, raw_offset: &str) -> (Vec<&InlineResult>, String) {
        let page: usize = if raw_offset.is_empty() {
            0
        } else {
            raw_offset.parse().unwrap_or(0)
        };

        let start = page * self.per_page;

        if start >= self.results.len() {
            // Past the end — return empty with no next offset.
            return (Vec::new(), String::new());
        }

        let end = (start + self.per_page).min(self.results.len());
        let page_results: Vec<&InlineResult> = self.results[start..end].iter().collect();

        let next_offset = if end < self.results.len() {
            (page + 1).to_string()
        } else {
            String::new() // no more pages
        };

        (page_results, next_offset)
    }
}

impl From<InlineResult> for crate::types::InlineQueryResult {
    fn from(r: InlineResult) -> Self {
        use crate::types::InlineResultKind as TlKind;
        let (message_text, parse_mode, keyboard) = match r.screen {
            Some(screen) => {
                let msg = screen.messages.into_iter().next();
                match msg.map(|m| m.content) {
                    Some(crate::types::MessageContent::Text { text, parse_mode, keyboard, .. }) => {
                        (Some(text), parse_mode, keyboard)
                    }
                    _ => (None, crate::types::ParseMode::Html, None),
                }
            }
            None => (None, crate::types::ParseMode::Html, None),
        };
        crate::types::InlineQueryResult {
            id: r.id,
            kind: match r.kind {
                InlineResultKind::Article => TlKind::Article,
                InlineResultKind::Photo { url } => TlKind::Photo { photo_url: url, width: None, height: None },
                InlineResultKind::Gif { url } => TlKind::Gif { gif_url: url },
                InlineResultKind::Video { url, mime } => TlKind::Video { video_url: url, mime_type: mime },
                InlineResultKind::Voice { url } => TlKind::Voice { voice_url: url },
                InlineResultKind::Document { url, mime } => TlKind::Document { document_url: url, mime_type: mime },
            },
            title: r.title,
            description: r.description,
            thumb_url: r.thumbnail_url,
            message_text,
            parse_mode,
            keyboard,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_results(n: usize) -> Vec<InlineResult> {
        (0..n)
            .map(|i| {
                InlineResult::article(i.to_string())
                    .title(format!("Result {}", i))
                    .build()
            })
            .collect()
    }

    #[test]
    fn paginate_first_page() {
        let answer = InlineAnswer::new(make_results(25)).per_page(10);
        let (page, next) = answer.paginate("");
        assert_eq!(page.len(), 10);
        assert_eq!(page[0].id, "0");
        assert_eq!(page[9].id, "9");
        assert_eq!(next, "1");
    }

    #[test]
    fn paginate_middle_page() {
        let answer = InlineAnswer::new(make_results(25)).per_page(10);
        let (page, next) = answer.paginate("1");
        assert_eq!(page.len(), 10);
        assert_eq!(page[0].id, "10");
        assert_eq!(page[9].id, "19");
        assert_eq!(next, "2");
    }

    #[test]
    fn paginate_last_page() {
        let answer = InlineAnswer::new(make_results(25)).per_page(10);
        let (page, next) = answer.paginate("2");
        assert_eq!(page.len(), 5);
        assert_eq!(page[0].id, "20");
        assert_eq!(page[4].id, "24");
        assert!(next.is_empty(), "no more pages");
    }

    #[test]
    fn paginate_past_end() {
        let answer = InlineAnswer::new(make_results(25)).per_page(10);
        let (page, next) = answer.paginate("100");
        assert!(page.is_empty());
        assert!(next.is_empty());
    }

    #[test]
    fn paginate_empty_results() {
        let answer = InlineAnswer::new(vec![]).per_page(10);
        let (page, next) = answer.paginate("");
        assert!(page.is_empty());
        assert!(next.is_empty());
    }

    #[test]
    fn paginate_exact_fit() {
        let answer = InlineAnswer::new(make_results(20)).per_page(10);
        let (page1, next1) = answer.paginate("");
        assert_eq!(page1.len(), 10);
        assert_eq!(next1, "1");

        let (page2, next2) = answer.paginate("1");
        assert_eq!(page2.len(), 10);
        assert!(next2.is_empty(), "exactly 20 items in 2 pages of 10");
    }

    #[test]
    fn paginate_single_page() {
        let answer = InlineAnswer::new(make_results(5)).per_page(10);
        let (page, next) = answer.paginate("");
        assert_eq!(page.len(), 5);
        assert!(next.is_empty());
    }

    #[test]
    fn paginate_invalid_offset() {
        let answer = InlineAnswer::new(make_results(25)).per_page(10);
        let (page, next) = answer.paginate("not_a_number");
        // Invalid offset falls back to page 0.
        assert_eq!(page.len(), 10);
        assert_eq!(page[0].id, "0");
        assert_eq!(next, "1");
    }

    #[test]
    fn per_page_clamped() {
        let answer = InlineAnswer::new(make_results(100)).per_page(999);
        assert_eq!(answer.per_page, 50);

        let answer = InlineAnswer::new(make_results(100)).per_page(0);
        assert_eq!(answer.per_page, 1);
    }

    #[test]
    fn builder_article() {
        let result = InlineResult::article("abc")
            .title("Hello")
            .description("World")
            .thumb("https://example.com/thumb.jpg")
            .build();

        assert_eq!(result.id, "abc");
        assert_eq!(result.title.as_deref(), Some("Hello"));
        assert_eq!(result.description.as_deref(), Some("World"));
        assert_eq!(
            result.thumbnail_url.as_deref(),
            Some("https://example.com/thumb.jpg")
        );
        assert!(matches!(result.kind, InlineResultKind::Article));
    }

    #[test]
    fn builder_photo() {
        let result = InlineResult::photo("p1", "https://example.com/photo.jpg")
            .title("Photo")
            .build();

        assert_eq!(result.id, "p1");
        assert!(
            matches!(result.kind, InlineResultKind::Photo { ref url } if url == "https://example.com/photo.jpg")
        );
    }

    #[test]
    fn builder_gif() {
        let result = InlineResult::gif("g1", "https://example.com/cat.gif").build();

        assert!(
            matches!(result.kind, InlineResultKind::Gif { ref url } if url == "https://example.com/cat.gif")
        );
    }

    #[test]
    fn builder_with_screen() {
        let screen = Screen::text("test", "Hello from inline!").build();
        let result = InlineResult::article("s1")
            .title("With Screen")
            .screen(screen)
            .build();

        assert!(result.screen.is_some());
    }

    #[test]
    fn inline_answer_defaults() {
        let answer = InlineAnswer::new(vec![]);
        assert_eq!(answer.per_page, 20);
        assert_eq!(answer.cache_time, 300);
        assert!(!answer.is_personal);
        assert!(answer.switch_pm_text.is_none());
    }

    #[test]
    fn inline_answer_personal_and_switch_pm() {
        let answer = InlineAnswer::new(vec![])
            .personal()
            .switch_pm("Start bot", "inline_ref");

        assert!(answer.is_personal);
        assert_eq!(answer.switch_pm_text.as_deref(), Some("Start bot"));
        assert_eq!(answer.switch_pm_parameter.as_deref(), Some("inline_ref"));
    }
}
