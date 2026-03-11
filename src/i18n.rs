//! I18n — full localization system.
//!
//! Load `.ftl` files from a directory, one file per language. The file name
//! (without extension) is the language code. Keys use Fluent-like syntax;
//! `{ $var }` placeholders are substituted at runtime.
//!
//! # Directory layout
//!
//! ```text
//! locales/
//!   en.ftl      # English (default)
//!   ru.ftl      # Russian
//!   uk.ftl      # Ukrainian
//! ```
//!
//! # File format (`.ftl`)
//!
//! ```text
//! # Comment
//! greeting = Hello, { $name }!
//! main-menu =
//!     Welcome back.
//!     Choose an action:
//! btn-back = ← Back
//! btn-settings = ⚙️ Settings
//! err-not-a-number = Not a number.
//! ```
//!
//! # Usage in handlers
//!
//! ```rust,ignore
//! fn greet(ctx: &mut Ctx) -> BoxFut<'_> {
//!     Box::pin(async move {
//!         let text = ctx.t_with("greeting", &[("name", &ctx.user.first_name)]);
//!         ctx.navigate(
//!             Screen::text("home", text)
//!                 .keyboard(|kb| kb.button_row(ctx.t("btn-settings"), "settings"))
//!                 .build()
//!         ).await
//!     })
//! }
//! ```
//!
//! # Framework keys
//!
//! The framework uses these keys internally (for forms, pagination, etc.).
//! Override them in your `.ftl` files to localize framework UI.
//!
//! | Key                 | English default            |
//! |---------------------|----------------------------|
//! | `bg-nav-back`       | `←`                       |
//! | `bg-nav-prev`       | `←`                       |
//! | `bg-nav-next`       | `→`                       |
//! | `bg-form-cancel`    | `✕ Cancel`                |
//! | `bg-form-confirm`   | `✅ Confirm`               |
//! | `bg-form-review`    | `Review:\n\n{ $summary }` |
//! | `bg-err-nan`        | `Not a number.`            |
//! | `bg-err-min`        | `Minimum: { $min }`        |
//! | `bg-err-max`        | `Maximum: { $max }`        |
//! | `bg-err-choice`     | `Pick one of the options.` |
//! | `bg-err-photo`      | `Send a photo.`            |
//! | `bg-dismiss`        | `✖️`                      |

use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

static I18N: OnceLock<I18n> = OnceLock::new();

/// Multi-language translation store.
#[derive(Debug, Clone)]
pub struct I18n {
    bundles: HashMap<String, Bundle>,
    default_lang: String,
}

/// One language bundle (key → template string).
#[derive(Debug, Clone, Default)]
struct Bundle {
    messages: HashMap<String, String>,
}

// ──────────────────────────────────────────────────
// Built-in framework keys (English defaults)
// ──────────────────────────────────────────────────

fn framework_defaults() -> Vec<(&'static str, &'static str)> {
    vec![
        ("bg-nav-back",     "←"),
        ("bg-nav-prev",     "←"),
        ("bg-nav-next",     "→"),
        ("bg-form-cancel",  "✕ Cancel"),
        ("bg-form-confirm", "✅ Confirm"),
        ("bg-form-review",  "Review:\n\n{ $summary }"),
        ("bg-err-nan",      "Not a number."),
        ("bg-err-min",      "Minimum: { $min }"),
        ("bg-err-max",      "Maximum: { $max }"),
        ("bg-err-choice",   "Pick one of the options."),
        ("bg-err-photo",    "Send a photo."),
        ("bg-dismiss",      "✖️"),
    ]
}

// ──────────────────────────────────────────────────
// I18n impl
// ──────────────────────────────────────────────────

impl I18n {
    /// Create an empty I18n with only framework defaults.
    pub fn new(default_lang: &str) -> Self {
        let mut bundles = HashMap::new();
        let mut def = Bundle::default();
        for (k, v) in framework_defaults() {
            def.messages.insert(k.to_string(), v.to_string());
        }
        bundles.insert(default_lang.to_string(), def);
        Self {
            bundles,
            default_lang: default_lang.to_string(),
        }
    }

    /// Load all `.ftl` files from a directory.
    ///
    /// Each file name (minus `.ftl`) becomes a language code.
    /// Framework default keys are injected into every language bundle
    /// *unless* the user’s file already defines them.
    pub fn load(locales_dir: impl AsRef<Path>, default_lang: &str) -> Result<Self, I18nError> {
        let dir = locales_dir.as_ref();
        if !dir.is_dir() {
            return Err(I18nError::NotADirectory(dir.display().to_string()));
        }

        let mut bundles: HashMap<String, Bundle> = HashMap::new();

        // Read every .ftl file
        let entries = std::fs::read_dir(dir)
            .map_err(|e| I18nError::Io(e, dir.display().to_string()))?;

        for entry in entries {
            let entry = entry.map_err(|e| I18nError::Io(e, dir.display().to_string()))?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("ftl") {
                continue;
            }
            let lang = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();

            let content = std::fs::read_to_string(&path)
                .map_err(|e| I18nError::Io(e, path.display().to_string()))?;

            let messages = parse_ftl(&content);
            bundles.insert(lang, Bundle { messages });
        }

        // Ensure default lang bundle exists
        bundles
            .entry(default_lang.to_string())
            .or_default();

        // Inject framework defaults into every bundle (don’t overwrite user keys)
        let fw = framework_defaults();
        for bundle in bundles.values_mut() {
            for (k, v) in &fw {
                bundle
                    .messages
                    .entry(k.to_string())
                    .or_insert_with(|| v.to_string());
            }
        }

        Ok(Self {
            bundles,
            default_lang: default_lang.to_string(),
        })
    }

    /// Look up a simple message (no variables).
    pub fn t(&self, lang: &str, key: &str) -> String {
        self.get_raw(lang, key)
    }

    /// Look up a message with `{ $var }` substitutions.
    pub fn t_with(&self, lang: &str, key: &str, args: &[(&str, &str)]) -> String {
        let raw = self.get_raw(lang, key);
        substitute(&raw, args)
    }

    /// Default language code.
    pub fn default_lang(&self) -> &str {
        &self.default_lang
    }

    /// List all loaded language codes.
    pub fn languages(&self) -> Vec<&str> {
        self.bundles.keys().map(|s| s.as_str()).collect()
    }

    /// Add a single translation key for a language.
    pub fn add(&mut self, lang: &str, key: &str, value: &str) {
        self.bundles
            .entry(lang.to_string())
            .or_default()
            .messages
            .insert(key.to_string(), value.to_string());
    }

    // Look up key: try exact lang, then default lang, then return key itself.
    fn get_raw(&self, lang: &str, key: &str) -> String {
        if let Some(bundle) = self.bundles.get(lang) {
            if let Some(msg) = bundle.messages.get(key) {
                return msg.clone();
            }
        }
        if let Some(bundle) = self.bundles.get(&self.default_lang) {
            if let Some(msg) = bundle.messages.get(key) {
                return msg.clone();
            }
        }
        // Fallback: return the key so missing translations are visible
        key.to_string()
    }
}

impl Default for I18n {
    fn default() -> Self {
        Self::new("en")
    }
}

// ──────────────────────────────────────────────────
// Global accessor
// ──────────────────────────────────────────────────

/// Set the global I18n instance. Call once at startup.
pub fn set_i18n(i: I18n) {
    let _ = I18N.set(i);
}

/// Get the global I18n (framework defaults if nothing was loaded).
pub fn i18n() -> &'static I18n {
    I18N.get_or_init(I18n::default)
}

/// Shortcut: look up a framework key for a given language.
///
/// Used internally by keyboard/form code that doesn’t have a `Ctx`.
pub fn ft(lang: &str, key: &str) -> String {
    i18n().t(lang, key)
}

/// Shortcut: look up a framework key with args.
pub fn ft_with(lang: &str, key: &str, args: &[(&str, &str)]) -> String {
    i18n().t_with(lang, key, args)
}

// ──────────────────────────────────────────────────
// .ftl parser
// ──────────────────────────────────────────────────

/// Parse a `.ftl` file into key→value pairs.
///
/// Supported syntax:
/// - `key = value` (single line)
/// - `key =\n    line1\n    line2` (multiline, 4-space or tab indent)
/// - `# comment`
/// - blank lines ignored
/// - `{ $var }` kept as-is (substituted at runtime)
fn parse_ftl(input: &str) -> HashMap<String, String> {
    let mut messages = HashMap::new();
    let mut current_key: Option<String> = None;
    let mut current_value = String::new();

    for line in input.lines() {
        let trimmed = line.trim();

        // Comment or blank
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Continuation line (starts with whitespace)
        if (line.starts_with(' ') || line.starts_with('\t')) && current_key.is_some() {
            if !current_value.is_empty() {
                current_value.push('\n');
            }
            current_value.push_str(trimmed);
            continue;
        }

        // Flush previous key
        if let Some(key) = current_key.take() {
            messages.insert(key, current_value.clone());
            current_value.clear();
        }

        // New key = value
        if let Some(eq_pos) = trimmed.find('=') {
            let key = trimmed[..eq_pos].trim().to_string();
            let val = trimmed[eq_pos + 1..].trim().to_string();
            current_key = Some(key);
            current_value = val;
        }
    }

    // Flush last key
    if let Some(key) = current_key {
        messages.insert(key, current_value);
    }

    messages
}

/// Replace `{ $var }` placeholders in a template string.
fn substitute(template: &str, args: &[(&str, &str)]) -> String {
    let mut result = template.to_string();
    for (key, val) in args {
        // Match both `{ $key }` and `{$key}`
        result = result.replace(&format!("{{ ${} }}", key), val);
        result = result.replace(&format!("{{${}}}", key), val);
    }
    result
}

// ──────────────────────────────────────────────────
// Errors
// ──────────────────────────────────────────────────

#[derive(Debug)]
pub enum I18nError {
    NotADirectory(String),
    Io(std::io::Error, String),
}

impl std::fmt::Display for I18nError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotADirectory(p) => write!(f, "locales path is not a directory: {}", p),
            Self::Io(e, p) => write!(f, "failed to read '{}': {}", p, e),
        }
    }
}

impl std::error::Error for I18nError {}

// ──────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_parse_ftl_single_line() {
        let input = r#"
# Comment
greeting = Hello!
btn-back = ← Back
"#;
        let m = parse_ftl(input);
        assert_eq!(m.get("greeting").unwrap(), "Hello!");
        assert_eq!(m.get("btn-back").unwrap(), "← Back");
    }

    #[test]
    fn test_parse_ftl_multiline() {
        let input = "menu =\n    Line one.\n    Line two.";
        let m = parse_ftl(input);
        assert_eq!(m.get("menu").unwrap(), "Line one.\nLine two.");
    }

    #[test]
    fn test_parse_ftl_with_vars() {
        let input = "hello = Hi, { $name }! Age: { $age }.";
        let m = parse_ftl(input);
        assert_eq!(
            m.get("hello").unwrap(),
            "Hi, { $name }! Age: { $age }."
        );
    }

    #[test]
    fn test_substitute() {
        assert_eq!(
            substitute("Hello, { $name }!", &[("name", "Alice")]),
            "Hello, Alice!"
        );
        assert_eq!(
            substitute("Min: { $min }, Max: { $max }", &[("min", "1"), ("max", "10")]),
            "Min: 1, Max: 10"
        );
    }

    #[test]
    fn test_substitute_compact() {
        assert_eq!(
            substitute("Hi {$name}!", &[("name", "Bob")]),
            "Hi Bob!"
        );
    }

    #[test]
    fn test_i18n_default_has_framework_keys() {
        let i = I18n::default();
        assert_eq!(i.t("en", "bg-nav-back"), "←");
        assert_eq!(i.t("en", "bg-form-cancel"), "✕ Cancel");
    }

    #[test]
    fn test_i18n_fallback_to_default_lang() {
        let i = I18n::new("en");
        // Requesting "fr" which doesn’t exist → falls back to "en"
        assert_eq!(i.t("fr", "bg-nav-back"), "←");
    }

    #[test]
    fn test_i18n_missing_key_returns_key() {
        let i = I18n::default();
        assert_eq!(i.t("en", "no-such-key"), "no-such-key");
    }

    #[test]
    fn test_i18n_t_with() {
        let i = I18n::default();
        let result = i.t_with("en", "bg-err-min", &[("min", "5")]);
        assert_eq!(result, "Minimum: 5");
    }

    #[test]
    fn test_load_from_dir() {
        let dir = tempdir();
        write_file(&dir, "en.ftl", "hello = Hello, { $name }!\nbtn-ok = OK");
        write_file(&dir, "de.ftl", "hello = Hallo, { $name }!\nbtn-ok = OK");

        let i = I18n::load(&dir, "en").unwrap();
        assert_eq!(
            i.t_with("en", "hello", &[("name", "World")]),
            "Hello, World!"
        );
        assert_eq!(
            i.t_with("de", "hello", &[("name", "Welt")]),
            "Hallo, Welt!"
        );
        assert_eq!(i.t("de", "btn-ok"), "OK");
        // Framework keys injected
        assert_eq!(i.t("de", "bg-nav-back"), "←");
    }

    #[test]
    fn test_user_overrides_framework_key() {
        let dir = tempdir();
        write_file(&dir, "de.ftl", "bg-nav-back = \u{2190} Back");
        write_file(&dir, "en.ftl", "");

        let i = I18n::load(&dir, "en").unwrap();
        assert_eq!(i.t("de", "bg-nav-back"), "← Back");
        assert_eq!(i.t("en", "bg-nav-back"), "←"); // default
    }

    // ─── test helpers ───

    fn tempdir() -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("blazegram_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn write_file(dir: &Path, name: &str, content: &str) {
        let mut f = std::fs::File::create(dir.join(name)).unwrap();
        f.write_all(content.as_bytes()).unwrap();
    }
}
