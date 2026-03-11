//! Markup processor: CleanBot-style markdown → HTML, plus HTML helpers.

/// Convert CleanBot markup to Telegram HTML.
///
/// ```text
/// *bold*         → <b>bold</b>
/// _italic_       → <i>italic</i>
/// __underline__  → <u>underline</u>
/// ~strike~       → <s>strike</s>
/// `code`         → <code>code</code>
/// ```block```    → <pre>block</pre>
/// [text](url)    → <a href="url">text</a>
/// ||spoiler||    → <tg-spoiler>spoiler</tg-spoiler>
/// ```
pub fn render(input: &str) -> String {
    let mut result = String::with_capacity(input.len() * 2);
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '*' => {
                let content = read_until(&mut chars, '*');
                result.push_str("<b>");
                result.push_str(&escape(&content));
                result.push_str("</b>");
            }
            '_' if chars.peek() == Some(&'_') => {
                chars.next();
                let content = read_until_double(&mut chars, '_');
                result.push_str("<u>");
                result.push_str(&escape(&content));
                result.push_str("</u>");
            }
            '_' => {
                let content = read_until(&mut chars, '_');
                result.push_str("<i>");
                result.push_str(&escape(&content));
                result.push_str("</i>");
            }
            '~' => {
                let content = read_until(&mut chars, '~');
                result.push_str("<s>");
                result.push_str(&escape(&content));
                result.push_str("</s>");
            }
            '`' if chars.peek() == Some(&'`') => {
                chars.next();
                if chars.peek() == Some(&'`') {
                    chars.next();
                    let block = read_until_triple(&mut chars, '`');
                    if let Some((lang, code)) = block.split_once('\n') {
                        let lang = lang.trim();
                        if !lang.is_empty() {
                            result.push_str(&format!(
                                "<pre><code class=\"language-{}\">{}</code></pre>",
                                escape(lang),
                                escape(code)
                            ));
                        } else {
                            result.push_str("<pre>");
                            result.push_str(&escape(code));
                            result.push_str("</pre>");
                        }
                    } else {
                        result.push_str("<pre>");
                        result.push_str(&escape(&block));
                        result.push_str("</pre>");
                    }
                } else {
                    // `` fallback
                    result.push_str("``");
                }
            }
            '`' => {
                let content = read_until(&mut chars, '`');
                result.push_str("<code>");
                result.push_str(&escape(&content));
                result.push_str("</code>");
            }
            '[' => {
                let text = read_until(&mut chars, ']');
                if chars.peek() == Some(&'(') {
                    chars.next();
                    let url = read_until(&mut chars, ')');
                    result.push_str(&format!(
                        "<a href=\"{}\">{}</a>",
                        escape_attr(&url),
                        escape(&text)
                    ));
                } else {
                    result.push('[');
                    result.push_str(&escape(&text));
                    result.push(']');
                }
            }
            '|' if chars.peek() == Some(&'|') => {
                chars.next();
                let content = read_until_double(&mut chars, '|');
                result.push_str("<tg-spoiler>");
                result.push_str(&escape(&content));
                result.push_str("</tg-spoiler>");
            }
            '<' => result.push_str("&lt;"),
            '>' => result.push_str("&gt;"),
            '&' => result.push_str("&amp;"),
            _ => result.push(ch),
        }
    }

    result
}

// ─── HTML Helpers ───

/// Escape.
pub fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Escape attr.
pub fn escape_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Bold.
pub fn bold(text: &str) -> String {
    format!("<b>{}</b>", escape(text))
}

/// Italic.
pub fn italic(text: &str) -> String {
    format!("<i>{}</i>", escape(text))
}

/// Underline.
pub fn underline(text: &str) -> String {
    format!("<u>{}</u>", escape(text))
}

/// Strike.
pub fn strike(text: &str) -> String {
    format!("<s>{}</s>", escape(text))
}

/// Code.
pub fn code(text: &str) -> String {
    format!("<code>{}</code>", escape(text))
}

/// Pre.
pub fn pre(text: &str) -> String {
    format!("<pre>{}</pre>", escape(text))
}

/// Pre lang.
pub fn pre_lang(lang: &str, text: &str) -> String {
    format!(
        "<pre><code class=\"language-{}\">{}</code></pre>",
        escape(lang),
        escape(text)
    )
}

/// Link.
pub fn link(text: &str, url: &str) -> String {
    format!("<a href=\"{}\">{}</a>", escape_attr(url), escape(text))
}

/// Spoiler.
pub fn spoiler(text: &str) -> String {
    format!("<tg-spoiler>{}</tg-spoiler>", escape(text))
}

/// Blockquote.
pub fn blockquote(text: &str) -> String {
    format!("<blockquote>{}</blockquote>", escape(text))
}

/// Mention.
pub fn mention(user_id: u64, text: &str) -> String {
    format!("<a href=\"tg://user?id={}\">{}</a>", user_id, escape(text))
}

// ─── Parser helpers ───

fn read_until(chars: &mut std::iter::Peekable<std::str::Chars>, delimiter: char) -> String {
    let mut result = String::new();
    for ch in chars.by_ref() {
        if ch == delimiter {
            break;
        }
        result.push(ch);
    }
    result
}

fn read_until_double(chars: &mut std::iter::Peekable<std::str::Chars>, delimiter: char) -> String {
    let mut result = String::new();
    while let Some(ch) = chars.next() {
        if ch == delimiter {
            if chars.peek() == Some(&delimiter) {
                chars.next();
                break;
            } else {
                result.push(ch);
            }
        } else {
            result.push(ch);
        }
    }
    result
}

fn read_until_triple(chars: &mut std::iter::Peekable<std::str::Chars>, delimiter: char) -> String {
    let mut result = String::new();
    let mut count = 0;
    for ch in chars.by_ref() {
        if ch == delimiter {
            count += 1;
            if count == 3 {
                break;
            }
        } else {
            for _ in 0..count {
                result.push(delimiter);
            }
            count = 0;
            result.push(ch);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bold() {
        assert_eq!(render("*bold*"), "<b>bold</b>");
    }

    #[test]
    fn test_italic() {
        assert_eq!(render("_italic_"), "<i>italic</i>");
    }

    #[test]
    fn test_code() {
        assert_eq!(render("`code`"), "<code>code</code>");
    }

    #[test]
    fn test_link() {
        assert_eq!(
            render("[click](https://example.com)"),
            r#"<a href="https://example.com">click</a>"#
        );
    }

    #[test]
    fn test_escape_user_input() {
        assert_eq!(
            render("hello <script>alert(1)</script>"),
            "hello &lt;script&gt;alert(1)&lt;/script&gt;"
        );
    }

    #[test]
    fn test_mixed() {
        assert_eq!(
            render("*bold* and _italic_ and `code`"),
            "<b>bold</b> and <i>italic</i> and <code>code</code>"
        );
    }
}
