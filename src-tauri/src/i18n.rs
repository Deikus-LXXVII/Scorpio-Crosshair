//! The languages Scorpio speaks. English is written in the code itself; every
//! other language is a dictionary from the English text to its translation,
//! in `locales/<code>.json`. The settings window gets the chosen dictionary
//! whole; the few texts made here (the tray menu, error messages) look
//! themselves up. Nothing is kept parsed: a lookup reads the built-in JSON.

use std::sync::atomic::{AtomicUsize, Ordering};

/// Code and the language's own name, in the order the settings window lists them.
pub const LANGUAGES: [(&str, &str); 12] = [
    ("en", "English"),
    ("ru", "Русский"),
    ("uk", "Українська"),
    ("de", "Deutsch"),
    ("fr", "Français"),
    ("es", "Español"),
    ("pt-BR", "Português (Brasil)"),
    ("pl", "Polski"),
    ("tr", "Türkçe"),
    ("zh-CN", "简体中文"),
    ("ja", "日本語"),
    ("ko", "한국어"),
];

/// The setting that follows the system's language.
pub const AUTO: &str = "auto";

const DICTIONARIES: [(&str, &str); 11] = [
    ("ru", include_str!("../locales/ru.json")),
    ("uk", include_str!("../locales/uk.json")),
    ("de", include_str!("../locales/de.json")),
    ("fr", include_str!("../locales/fr.json")),
    ("es", include_str!("../locales/es.json")),
    ("pt-BR", include_str!("../locales/pt-BR.json")),
    ("pl", include_str!("../locales/pl.json")),
    ("tr", include_str!("../locales/tr.json")),
    ("zh-CN", include_str!("../locales/zh-CN.json")),
    ("ja", include_str!("../locales/ja.json")),
    ("ko", include_str!("../locales/ko.json")),
];

/// Index into `LANGUAGES` of the language in use.
static CURRENT: AtomicUsize = AtomicUsize::new(0);

pub fn is_setting(value: &str) -> bool {
    value == AUTO || LANGUAGES.iter().any(|(code, _)| *code == value)
}

/// The language a setting means: itself, or for "auto" the system's when
/// Scorpio speaks it, else English.
pub fn resolve(setting: &str) -> &'static str {
    let wanted = if setting == AUTO { system() } else { Some(setting.to_string()) };
    wanted.and_then(|tag| matching(&tag)).unwrap_or("en")
}

/// A language tag such as "ru-RU", "pt_BR.UTF-8" or "zh-Hant" to one of ours.
fn matching(tag: &str) -> Option<&'static str> {
    let tag = tag.split(['.', '@']).next()?.replace('_', "-");
    let lower = tag.to_ascii_lowercase();
    if lower.starts_with("zh") && ["-tw", "-hk", "-mo", "-hant"].iter().any(|t| lower.contains(t)) {
        return None;
    }
    let primary = tag.split('-').next()?.to_ascii_lowercase();
    LANGUAGES
        .iter()
        .map(|(code, _)| *code)
        .find(|code| code.eq_ignore_ascii_case(&tag))
        .or_else(|| LANGUAGES.iter().map(|(code, _)| *code).find(|code| code.split('-').next() == Some(primary.as_str())))
}

#[cfg(windows)]
fn system() -> Option<String> {
    // The display language of the user's Windows, as a language id.
    let full = unsafe { windows::Win32::Globalization::GetUserDefaultUILanguage() };
    let code = match full & 0x3ff {
        0x09 => "en",
        0x19 => "ru",
        0x22 => "uk",
        0x07 => "de",
        0x0c => "fr",
        0x0a => "es",
        0x16 => "pt",
        0x15 => "pl",
        0x1f => "tr",
        // Simplified Chinese only; Traditional readers get English.
        0x04 if matches!(full, 0x0804 | 0x1004) => "zh",
        0x11 => "ja",
        0x12 => "ko",
        _ => return None,
    };
    Some(code.into())
}

#[cfg(not(windows))]
fn system() -> Option<String> {
    // The first of these that is set decides, as for other programs.
    ["LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .filter_map(|name| std::env::var(name).ok())
        .find(|value| !value.is_empty())
        .and_then(|value| value.split(':').find_map(matching))
        .map(String::from)
}

/// Uses this language from now on.
pub fn set(code: &str) {
    let index = LANGUAGES.iter().position(|(c, _)| *c == code).unwrap_or(0);
    CURRENT.store(index, Ordering::Relaxed);
}

pub fn current() -> &'static str {
    LANGUAGES[CURRENT.load(Ordering::Relaxed)].0
}

fn dictionary(code: &str) -> Option<&'static str> {
    DICTIONARIES.iter().find(|(c, _)| *c == code).map(|(_, text)| *text)
}

/// The whole dictionary of a language, for the settings window; null for English.
pub fn texts(code: &str) -> serde_json::Value {
    dictionary(code).and_then(|text| serde_json::from_str(text).ok()).unwrap_or(serde_json::Value::Null)
}

/// One text in the language in use; `{name}` marks are filled from `values`.
pub fn tr(english: &str, values: &[(&str, &str)]) -> String {
    let found = dictionary(current())
        .and_then(|text| serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(text).ok())
        .and_then(|mut map| match map.remove(english) {
            Some(serde_json::Value::String(text)) => Some(text),
            _ => None,
        });
    let text = found.unwrap_or_else(|| english.to_string());
    // In one pass, so a value is never filled again.
    let mut out = String::new();
    let mut rest = text.as_str();
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        match after.find('}').map(|close| (&after[..close], close)).and_then(|(name, close)| values.iter().find(|(n, _)| *n == name).map(|(_, v)| (*v, close))) {
            Some((value, close)) => {
                out.push_str(value);
                rest = &after[close + 1..];
            }
            None => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn tags_match_our_languages() {
        assert_eq!(matching("ru-RU"), Some("ru"));
        assert_eq!(matching("ru_RU.UTF-8"), Some("ru"));
        assert_eq!(matching("pt_BR.UTF-8"), Some("pt-BR"));
        assert_eq!(matching("pt-PT"), Some("pt-BR"));
        assert_eq!(matching("zh-Hans-CN"), Some("zh-CN"));
        assert_eq!(matching("de@euro"), Some("de"));
        assert_eq!(matching("zh-TW"), None);
        assert_eq!(matching("zh-Hant-HK"), None);
        assert_eq!(matching("C"), None);
        assert_eq!(matching("nl-NL"), None);
        assert_eq!(resolve("ru"), "ru");
        assert_eq!(resolve("xx"), "en");
        assert_eq!(tr("{a} and {b}", &[("a", "{b}"), ("b", "2")]), "{b} and 2", "a value is never filled again");
    }

    /// Every source file of the settings window and of the app itself.
    fn sources() -> Vec<(String, String)> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut files = Vec::new();
        let mut folders = vec![root.join("src"), root.join("../ui")];
        while let Some(folder) = folders.pop() {
            for entry in std::fs::read_dir(&folder).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if path.file_name() != Some("fonts".as_ref()) && path.file_name() != Some("brand".as_ref()) {
                        folders.push(path);
                    }
                } else if matches!(path.extension().and_then(|e| e.to_str()), Some("rs" | "js" | "html")) && !path.ends_with("i18n.rs") {
                    files.push((path.display().to_string(), std::fs::read_to_string(&path).unwrap()));
                }
            }
        }
        files
    }

    /// Arguments that are not literals but are allowed: the style names (added
    /// below), the page's own marked texts, and the functions' own parameters.
    const NOT_LITERAL: [&str; 4] = ["name )", "node.dataset.", "english", "count"];

    /// The texts the code translates: every `t( '…' )`, `tr("…")` and
    /// `tn( n, '…', '…' )` (its "other" form), every `data-i18n` in the page,
    /// and the crosshair style names. Any other call fails the test, so no
    /// text can slip past it.
    fn english_texts() -> BTreeSet<String> {
        let mut found = BTreeSet::new();
        for (file, source) in sources() {
            for needle in ["data-i18n=\"", "data-i18n-label=\""] {
                for (start, _) in source.match_indices(needle) {
                    let (text, _) = literal(&source[start + needle.len() - 1..]).unwrap();
                    found.insert(text);
                }
            }
            for name in ["t(", "tn(", "tr("] {
                for (start, _) in source.match_indices(name) {
                    if source[..start].chars().last().is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '.') {
                        continue;
                    }
                    let rest = source[start + name.len()..].trim_start();
                    if NOT_LITERAL.iter().any(|allowed| rest.starts_with(allowed)) {
                        continue;
                    }
                    let rest = if name == "tn(" {
                        // The count, then the "one" form, then the key.
                        let quote = rest.find(['\'', '"', '`']).unwrap_or_else(|| panic!("{file}: tn( without texts: {}", &rest[..40.min(rest.len())]));
                        let (_, used) = literal(&rest[quote..]).unwrap_or_else(|| panic!("{file}: tn( one form is not a plain text"));
                        rest[quote + used..].trim_start_matches([',', ' ', '\n', '\t'])
                    } else {
                        rest
                    };
                    let (text, _) = literal(rest).unwrap_or_else(|| panic!("{file}: {name} takes a plain text, found: {}", &rest[..60.min(rest.len())]));
                    found.insert(text);
                }
            }
        }
        for (_, name, _) in crate::core::crosshair::STYLES {
            found.insert(name.to_string());
        }
        found
    }

    /// A JS or Rust string literal at the start of `text`, unescaped, and the
    /// bytes it took; None for anything else (a variable, a template with `${`).
    fn literal(text: &str) -> Option<(String, usize)> {
        let quote = text.chars().next().filter(|c| matches!(c, '\'' | '"' | '`'))?;
        let mut out = String::new();
        let mut chars = text.char_indices().skip(1);
        while let Some((_, c)) = chars.next() {
            match c {
                '\\' => match chars.next()?.1 {
                    'n' => out.push('\n'),
                    other => out.push(other),
                },
                c if c == quote => {
                    let used = chars.next().map_or(text.len(), |(i, _)| i);
                    return (!out.contains("${")).then_some((out, used));
                }
                c => out.push(c),
            }
        }
        None
    }

    fn marks(text: &str) -> BTreeSet<&str> {
        text.match_indices('{').filter_map(|(i, _)| text[i + 1..].find('}').map(|end| &text[i + 1..i + 1 + end])).collect()
    }

    #[test]
    fn every_language_translates_every_text_and_nothing_else() {
        let english = english_texts();
        assert!(english.len() > 100, "found only {} texts to translate", english.len());
        for (code, text) in DICTIONARIES {
            let map: serde_json::Map<String, serde_json::Value> = serde_json::from_str(text).unwrap_or_else(|e| panic!("{code}.json: {e}"));
            let keys: BTreeSet<String> = map.keys().cloned().collect();
            let missing: Vec<_> = english.difference(&keys).collect();
            let extra: Vec<_> = keys.difference(&english).collect();
            assert!(missing.is_empty(), "{code}.json lacks {missing:#?}");
            assert!(extra.is_empty(), "{code}.json has texts the code no longer uses: {extra:#?}");
            for (key, value) in &map {
                let forms: Vec<&str> = match value {
                    serde_json::Value::String(s) => vec![s],
                    serde_json::Value::Object(o) => o.values().filter_map(|v| v.as_str()).collect(),
                    _ => panic!("{code}.json: {key:?} is neither text nor plural forms"),
                };
                for form in forms {
                    assert!(!form.trim().is_empty(), "{code}.json: {key:?} is empty");
                    assert_eq!(marks(form), marks(key), "{code}.json: {key:?} must keep the same {{marks}}");
                }
            }
        }
    }
}
