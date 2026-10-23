//! Internationalization (i18n) module
//!
//! Supports: en, zh-CN, zh-TW, zh-HK
//! Fallback chain: requested → base → en → embedded

use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

// Embedded English fallback (compiled into binary)
const EMBEDDED_EN: &str = include_str!("../locales/en.json");

pub struct I18n {
    strings: HashMap<String, String>,
    lang: String,
}

impl I18n {
    /// Load translations for given language code
    pub fn load(lang: &str, locale_dir: &Path) -> Self {
        let resolved = resolve_locale(lang);

        // Try loading from files in order
        for locale in &resolved {
            let path = locale_dir.join(format!("{}.json", locale));
            if let Ok(content) = std::fs::read_to_string(&path)
                && let Ok(strings) = parse_translations(&content)
            {
                return Self { strings, lang: locale.to_string() };
            }
        }

        // Fallback to embedded English
        match parse_translations(EMBEDDED_EN) {
            Ok(strings) => Self { strings, lang: "en".to_string() },
            Err(_) => Self { strings: HashMap::new(), lang: "en".to_string() },
        }
    }

    /// Translate a key, optionally with format arguments
    pub fn t(&self, key: &str) -> String {
        self.strings.get(key).cloned().unwrap_or_else(|| key.to_string())
    }

    /// Translate with format arguments
    pub fn tf(&self, key: &str, args: &[&str]) -> String {
        let mut result = self.t(key);
        for (i, arg) in args.iter().enumerate() {
            result = result.replacen("{}", arg, 1);
            result = result.replace(&format!("{{{}}}", i), arg);
        }
        result
    }

    /// Get current language code
    pub fn lang(&self) -> &str {
        &self.lang
    }

    /// Check if a translation exists
    pub fn has(&self, key: &str) -> bool {
        self.strings.contains_key(key)
    }
}

/// Detect system language from environment
pub fn detect_system_language() -> String {
    // Check environment variables (Unix)
    let env_lang = std::env::var("LC_ALL")
        .or_else(|_| std::env::var("LC_MESSAGES"))
        .or_else(|_| std::env::var("LANG"))
        .unwrap_or_else(|_| String::new());

    if !env_lang.is_empty() {
        // Parse zh_CN.UTF-8 → zh-CN
        let parsed = env_lang.split('.').next().unwrap_or("").replace('_', "-").to_lowercase();

        if !parsed.is_empty() && parsed != "c" {
            return parsed;
        }
    }

    // Default to English
    "en".to_string()
}

/// Resolve locale to fallback chain
fn resolve_locale(input: &str) -> Vec<String> {
    let normalized = normalize_locale(input);

    match normalized.as_str() {
        "zh-CN" | "zh" => vec!["zh-CN".to_string(), "en".to_string()],
        "zh-TW" | "zh-HK" => vec![normalized.clone(), "zh-TW".to_string(), "zh-CN".to_string(), "en".to_string()],
        _ => vec![normalized, "en".to_string()],
    }
}

/// Normalize locale code
fn normalize_locale(input: &str) -> String {
    let cleaned = input.split('.').next().unwrap_or("").replace('_', "-").to_lowercase();

    // Special cases
    match cleaned.as_str() {
        "zh" | "zh-cn" => "zh-CN".to_string(),
        "zh-tw" | "zh-hk" | "zh-hant" => "zh-TW".to_string(),
        _ => cleaned,
    }
}

/// Parse JSON translation file
fn parse_translations(content: &str) -> Result<HashMap<String, String>, serde_json::Error> {
    let value: Value = serde_json::from_str(content)?;
    let mut strings = HashMap::new();

    if let Some(obj) = value.as_object() {
        for (key, value) in obj {
            if let Some(str_val) = value.as_str() {
                strings.insert(key.clone(), str_val.to_string());
            }
        }
    }

    Ok(strings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_locale() {
        assert_eq!(normalize_locale("zh_CN.UTF-8"), "zh-CN");
        assert_eq!(normalize_locale("zh_TW"), "zh-TW");
        assert_eq!(normalize_locale("zh-HK"), "zh-TW");
        assert_eq!(normalize_locale("en_US.UTF-8"), "en-us");
        assert_eq!(normalize_locale("ja"), "ja");
    }

    #[test]
    fn test_resolve_locale() {
        assert_eq!(resolve_locale("zh-CN"), vec!["zh-CN", "en"]);
        assert_eq!(resolve_locale("zh-TW"), vec!["zh-TW", "zh-TW", "zh-CN", "en"]);
        assert_eq!(resolve_locale("en"), vec!["en", "en"]);
    }

    #[test]
    fn test_i18n_load() {
        let i18n = I18n::load("en", Path::new("locales"));
        assert_eq!(i18n.lang(), "en");
        assert!(i18n.has("welcome"));
        assert_eq!(i18n.t("welcome"), "Welcome to i!");
    }

    #[test]
    fn test_i18n_format() {
        let i18n = I18n::load("en", Path::new("locales"));
        let result = i18n.tf("install.prompt", &["/usr/local/bin"]);
        assert_eq!(result, "Install i to /usr/local/bin?");
    }

    #[test]
    fn format_keeps_distinct_workspace_and_model_arguments() {
        let i18n = I18n::load("en", Path::new("locales"));
        assert_eq!(
            i18n.tf("status.workspace", &["/tmp/project", "fixture-model"]),
            "workspace: /tmp/project | model: fixture-model"
        );
    }
}
