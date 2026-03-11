//! Simple template engine with auto-escaping.
//!
//! Syntax:
//! - `{{ var }}` — insert value with HTML-escape
//! - `{! var !}` — insert raw (no escape)
//! - `{% if key %}...{% else %}...{% endif %}` — conditional (truthy = non-empty)
//! - `{% for item in list %}...{% endfor %}` — loop (list = JSON array in vars)
//!
//! # Examples
//!
//! ```
//! use std::collections::HashMap;
//! use blazegram::template::render;
//!
//! let mut vars = HashMap::new();
//! vars.insert("name", String::from("Alice"));
//! assert_eq!(render("Hello {{ name }}!", &vars), "Hello Alice!");
//! ```

use std::collections::HashMap;

use crate::markup;

/// Render a template string with the given variables.
///
/// Missing variables silently expand to the empty string.
/// `{{ var }}` is HTML-escaped; `{! var !}` is inserted raw.
pub fn render(template: &str, vars: &HashMap<&str, String>) -> String {
    let tokens = tokenize(template);
    // Convert to owned keys internally so for-loops don't need string leaking
    let owned: HashMap<String, String> = vars
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect();
    eval_owned(&tokens, &owned)
}

// ─── Token representation ───

#[derive(Debug, Clone, PartialEq)]
enum Token {
    /// Literal text
    Text(String),
    /// `{{ var }}` — escaped interpolation
    VarEscaped(String),
    /// `{! var !}` — raw interpolation
    VarRaw(String),
    /// `{% if key %}`
    If(String),
    /// `{% else %}`
    Else,
    /// `{% endif %}`
    EndIf,
    /// `{% for item in list %}`
    For { item: String, list: String },
    /// `{% endfor %}`
    EndFor,
}

// ─── Tokenizer ───

fn tokenize(template: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut rest = template;

    while !rest.is_empty() {
        // Find the earliest tag opening
        let positions = [rest.find("{{"), rest.find("{!"), rest.find("{%")];
        let earliest = positions.iter().filter_map(|p| *p).min();

        match earliest {
            None => {
                // No more tags — the rest is literal text
                tokens.push(Token::Text(rest.to_string()));
                break;
            }
            Some(pos) => {
                // Push any text before this tag
                if pos > 0 {
                    tokens.push(Token::Text(rest[..pos].to_string()));
                }

                let tag_start = &rest[pos..];

                if tag_start.starts_with("{{") {
                    if let Some(end) = tag_start.find("}}") {
                        let inner = tag_start[2..end].trim();
                        tokens.push(Token::VarEscaped(inner.to_string()));
                        rest = &rest[pos + end + 2..];
                    } else {
                        // No closing — treat as text
                        tokens.push(Token::Text("{{".to_string()));
                        rest = &rest[pos + 2..];
                    }
                } else if tag_start.starts_with("{!") {
                    if let Some(end) = tag_start.find("!}") {
                        let inner = tag_start[2..end].trim();
                        tokens.push(Token::VarRaw(inner.to_string()));
                        rest = &rest[pos + end + 2..];
                    } else {
                        tokens.push(Token::Text("{!".to_string()));
                        rest = &rest[pos + 2..];
                    }
                } else if tag_start.starts_with("{%") {
                    if let Some(end) = tag_start.find("%}") {
                        let inner = tag_start[2..end].trim();
                        let tok = parse_block_tag(inner);
                        tokens.push(tok);
                        rest = &rest[pos + end + 2..];
                    } else {
                        tokens.push(Token::Text("{%".to_string()));
                        rest = &rest[pos + 2..];
                    }
                } else {
                    unreachable!("earliest matched a tag prefix but none of {{, {{!, {{% matched");
                }
            }
        }
    }

    tokens
}

fn parse_block_tag(inner: &str) -> Token {
    let parts: Vec<&str> = inner.split_whitespace().collect();
    match parts.first().copied() {
        Some("if") => {
            let key = parts.get(1).unwrap_or(&"").to_string();
            Token::If(key)
        }
        Some("else") => Token::Else,
        Some("endif") => Token::EndIf,
        Some("for") => {
            // {% for item in list %}
            let item = parts.get(1).unwrap_or(&"_").to_string();
            // parts[2] should be "in"
            let list = parts.get(3).unwrap_or(&"").to_string();
            Token::For { item, list }
        }
        Some("endfor") => Token::EndFor,
        _ => Token::Text(String::new()),
    }
}

// ─── Evaluator (owned keys — no string leaking) ───

fn eval_owned(tokens: &[Token], vars: &HashMap<String, String>) -> String {
    let mut out = String::new();
    let mut i = 0;

    while i < tokens.len() {
        match &tokens[i] {
            Token::Text(s) => {
                out.push_str(s);
                i += 1;
            }
            Token::VarEscaped(name) => {
                if let Some(val) = vars.get(name) {
                    out.push_str(&markup::escape(val));
                }
                i += 1;
            }
            Token::VarRaw(name) => {
                if let Some(val) = vars.get(name) {
                    out.push_str(val);
                }
                i += 1;
            }
            Token::If(key) => {
                let (if_body, else_body, end_idx) = collect_if_block(tokens, i);
                let truthy = vars.get(key).map(|v| !v.is_empty()).unwrap_or(false);
                if truthy {
                    out.push_str(&eval_owned(&if_body, vars));
                } else {
                    out.push_str(&eval_owned(&else_body, vars));
                }
                i = end_idx + 1;
            }
            Token::For { item, list } => {
                let (body_tokens, end_idx) = collect_for_block(tokens, i);
                let items = vars
                    .get(list)
                    .map(|v| parse_json_array(v))
                    .unwrap_or_default();
                for element in &items {
                    let mut child_vars = vars.clone();
                    child_vars.insert(item.clone(), element.clone());
                    out.push_str(&eval_owned(&body_tokens, &child_vars));
                }
                i = end_idx + 1;
            }
            // Stray else / endif / endfor — skip
            Token::Else | Token::EndIf | Token::EndFor => {
                i += 1;
            }
        }
    }

    out
}

/// Collect tokens between `{% if %}` … `{% else %}` … `{% endif %}`.
/// Returns (if_body, else_body, index_of_endif).
fn collect_if_block(tokens: &[Token], start: usize) -> (Vec<Token>, Vec<Token>, usize) {
    let mut depth = 0;
    let mut if_body = Vec::new();
    let mut else_body = Vec::new();
    let mut in_else = false;
    let mut i = start + 1; // skip the opening If token

    while i < tokens.len() {
        match &tokens[i] {
            Token::If(_) => {
                depth += 1;
                if in_else {
                    else_body.push(tokens[i].clone());
                } else {
                    if_body.push(tokens[i].clone());
                }
            }
            Token::EndIf if depth == 0 => {
                return (if_body, else_body, i);
            }
            Token::EndIf => {
                depth -= 1;
                if in_else {
                    else_body.push(tokens[i].clone());
                } else {
                    if_body.push(tokens[i].clone());
                }
            }
            Token::Else if depth == 0 => {
                in_else = true;
            }
            other => {
                if in_else {
                    else_body.push(other.clone());
                } else {
                    if_body.push(other.clone());
                }
            }
        }
        i += 1;
    }

    // Unclosed if — return what we have
    (if_body, else_body, tokens.len().saturating_sub(1))
}

/// Collect tokens between `{% for %}` … `{% endfor %}`.
/// Returns (body, index_of_endfor).
fn collect_for_block(tokens: &[Token], start: usize) -> (Vec<Token>, usize) {
    let mut depth = 0;
    let mut body = Vec::new();
    let mut i = start + 1;

    while i < tokens.len() {
        match &tokens[i] {
            Token::For { .. } => {
                depth += 1;
                body.push(tokens[i].clone());
            }
            Token::EndFor if depth == 0 => {
                return (body, i);
            }
            Token::EndFor => {
                depth -= 1;
                body.push(tokens[i].clone());
            }
            other => {
                body.push(other.clone());
            }
        }
        i += 1;
    }

    (body, tokens.len().saturating_sub(1))
}

/// Minimal JSON array parser — extracts strings and numbers from a `[…]` array.
/// Handles: `["a", "b"]`, `[1, 2, 3]`, and mixed.
fn parse_json_array(s: &str) -> Vec<String> {
    let s = s.trim();
    if !s.starts_with('[') || !s.ends_with(']') {
        return Vec::new();
    }
    let inner = &s[1..s.len() - 1];
    if inner.trim().is_empty() {
        return Vec::new();
    }

    let mut items = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    let mut escape_next = false;
    let mut depth = 0;

    for ch in inner.chars() {
        if escape_next {
            current.push(ch);
            escape_next = false;
            continue;
        }
        match ch {
            '\\' if in_string => {
                escape_next = true;
            }
            '"' => {
                in_string = !in_string;
            }
            '[' if !in_string => {
                depth += 1;
                current.push(ch);
            }
            ']' if !in_string => {
                depth -= 1;
                current.push(ch);
            }
            ',' if !in_string && depth == 0 => {
                items.push(current.trim().to_string());
                current = String::new();
            }
            _ => {
                current.push(ch);
            }
        }
    }

    let last = current.trim().to_string();
    if !last.is_empty() {
        items.push(last);
    }

    items
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars<'a>(pairs: &[(&'a str, &str)]) -> HashMap<&'a str, String> {
        pairs.iter().map(|(k, v)| (*k, v.to_string())).collect()
    }

    #[test]
    fn test_escaped_var() {
        let v = vars(&[("name", "<b>Alice</b>")]);
        assert_eq!(
            render("Hello {{ name }}!", &v),
            "Hello &lt;b&gt;Alice&lt;/b&gt;!"
        );
    }

    #[test]
    fn test_raw_var() {
        let v = vars(&[("html", "<b>bold</b>")]);
        assert_eq!(render("Got: {! html !}", &v), "Got: <b>bold</b>");
    }

    #[test]
    fn test_missing_var_is_empty() {
        let v: HashMap<&str, String> = HashMap::new();
        assert_eq!(render("Hello {{ name }}!", &v), "Hello !");
    }

    #[test]
    fn test_if_truthy() {
        let v = vars(&[("show", "yes")]);
        assert_eq!(render("{% if show %}visible{% endif %}", &v), "visible");
    }

    #[test]
    fn test_if_falsy_missing() {
        let v: HashMap<&str, String> = HashMap::new();
        assert_eq!(render("{% if show %}visible{% endif %}", &v), "");
    }

    #[test]
    fn test_if_falsy_empty() {
        let v = vars(&[("show", "")]);
        assert_eq!(render("{% if show %}visible{% endif %}", &v), "");
    }

    #[test]
    fn test_if_else() {
        let v = vars(&[("admin", "true")]);
        assert_eq!(
            render("{% if admin %}Admin{% else %}User{% endif %}", &v),
            "Admin"
        );

        let v2: HashMap<&str, String> = HashMap::new();
        assert_eq!(
            render("{% if admin %}Admin{% else %}User{% endif %}", &v2),
            "User"
        );
    }

    #[test]
    fn test_for_loop() {
        let v = vars(&[("items", r#"["apple", "banana", "cherry"]"#)]);
        assert_eq!(
            render("{% for item in items %}{{ item }}, {% endfor %}", &v),
            "apple, banana, cherry, "
        );
    }

    #[test]
    fn test_for_loop_empty() {
        let v = vars(&[("items", "[]")]);
        assert_eq!(
            render("{% for item in items %}{{ item }}{% endfor %}", &v),
            ""
        );
    }

    #[test]
    fn test_for_loop_missing_list() {
        let v: HashMap<&str, String> = HashMap::new();
        assert_eq!(
            render("{% for item in items %}{{ item }}{% endfor %}", &v),
            ""
        );
    }

    #[test]
    fn test_nested_if_in_for() {
        let v = vars(&[("users", r#"["Alice", "Bob"]"#), ("greet", "yes")]);
        let tpl = "{% for u in users %}{% if greet %}Hi {{ u }}! {% endif %}{% endfor %}";
        assert_eq!(render(tpl, &v), "Hi Alice! Hi Bob! ");
    }

    #[test]
    fn test_nested_if() {
        let v = vars(&[("a", "1"), ("b", "2")]);
        let tpl = "{% if a %}A{% if b %}B{% endif %}{% endif %}";
        assert_eq!(render(tpl, &v), "AB");
    }

    #[test]
    fn test_for_with_numbers() {
        let v = vars(&[("nums", "[1, 2, 3]")]);
        assert_eq!(
            render("{% for n in nums %}{{ n }} {% endfor %}", &v),
            "1 2 3 "
        );
    }

    #[test]
    fn test_for_escapes_items() {
        let v = vars(&[("items", r#"["<script>", "safe"]"#)]);
        assert_eq!(
            render("{% for x in items %}{{ x }}|{% endfor %}", &v),
            "&lt;script&gt;|safe|"
        );
    }

    #[test]
    fn test_plain_text_passthrough() {
        let v: HashMap<&str, String> = HashMap::new();
        assert_eq!(render("just text", &v), "just text");
    }

    #[test]
    fn test_mixed() {
        let v = vars(&[
            ("title", "Page"),
            ("show_footer", "1"),
            ("items", r#"["A", "B"]"#),
        ]);
        let tpl = "<h1>{{ title }}</h1>{% for i in items %}<li>{{ i }}</li>{% endfor %}{% if show_footer %}<footer/>{% endif %}";
        assert_eq!(
            render(tpl, &v),
            "<h1>Page</h1><li>A</li><li>B</li><footer/>"
        );
    }

    #[test]
    fn test_parse_json_array_strings() {
        assert_eq!(parse_json_array(r#"["a", "b", "c"]"#), vec!["a", "b", "c"]);
    }

    #[test]
    fn test_parse_json_array_numbers() {
        assert_eq!(parse_json_array("[1, 2, 3]"), vec!["1", "2", "3"]);
    }

    #[test]
    fn test_parse_json_array_empty() {
        assert_eq!(parse_json_array("[]"), Vec::<String>::new());
    }

    #[test]
    fn test_parse_json_array_not_array() {
        assert_eq!(parse_json_array("hello"), Vec::<String>::new());
    }
}
