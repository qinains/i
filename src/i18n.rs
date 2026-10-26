//! Internationalization (i18n) module
//!
//! Registered locales share their language list with the browser interfaces.

use std::collections::HashMap;
use std::path::Path;

#[path = "locales_generated.rs"]
mod builtin;
const EMBEDDED_EN: &str = include_str!("../locales/en.json");

pub struct I18n {
    strings: HashMap<String, String>,
    lang: String,
}

impl I18n {
    /// Load translations for given language code
    pub fn load(lang: &str, locale_dir: &Path) -> Self {
        let resolved = resolve_locale(lang);

        let english = parse_translations(EMBEDDED_EN).expect("validated embedded English catalog");
        for locale in &resolved {
            let embedded = builtin::LOCALES.iter().find(|(code, _)| code == locale);
            let path = locale_dir.join(format!("{}.json", locale));
            if let Ok(content) = std::fs::read_to_string(&path)
                && let Ok(strings) = parse_translations(&content)
            {
                let mut complete = english.clone();
                if let Some((_, source)) = embedded {
                    complete.extend(parse_translations(source).expect("validated embedded catalog"));
                }
                complete.extend(strings);
                return Self { strings: complete, lang: locale.to_string() };
            }
            if let Some((_, source)) = embedded {
                let mut strings = english.clone();
                strings.extend(parse_translations(source).expect("validated embedded catalog"));
                return Self { strings, lang: locale.to_string() };
            }
        }
        Self { strings: english, lang: "en".to_string() }
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
    let language = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .into_iter()
        .filter_map(|name| std::env::var(name).ok())
        .find(|value| !value.trim().is_empty())
        .unwrap_or_default();
    normalize_locale(&language)
}

/// Resolve locale to fallback chain
fn resolve_locale(input: &str) -> Vec<String> {
    let normalized = normalize_locale(input);

    match normalized.as_str() {
        "en" => vec![normalized],
        "zh-TW" => vec![normalized, "zh-CN".to_string(), "en".to_string()],
        _ => vec![normalized, "en".to_string()],
    }
}

/// Normalize locale code
fn normalize_locale(input: &str) -> String {
    let cleaned = input.split(['.', '@']).next().unwrap_or("").replace('_', "-").to_lowercase();
    let parts: Vec<_> = cleaned.split('-').collect();
    if parts[0] == "zh" {
        return if parts.contains(&"hans") {
            "zh-CN"
        } else if parts.iter().any(|part| ["hant", "tw", "hk", "mo"].contains(part)) {
            "zh-TW"
        } else {
            "zh-CN"
        }
        .to_string();
    }
    builtin::LOCALES
        .iter()
        .find(|(code, _)| code.eq_ignore_ascii_case(&cleaned) || *code == parts[0])
        .map_or("en", |(code, _)| code)
        .to_string()
}

/// Parse JSON translation file
fn parse_translations(content: &str) -> Result<HashMap<String, String>, serde_json::Error> {
    serde_json::from_str(content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_locale() {
        assert_eq!(normalize_locale("zh_CN.UTF-8"), "zh-CN");
        assert_eq!(normalize_locale("zh_TW"), "zh-TW");
        assert_eq!(normalize_locale("zh-HK"), "zh-TW");
        assert_eq!(normalize_locale("en_US.UTF-8"), "en");
        assert_eq!(normalize_locale("ja"), "ja");
        assert_eq!(normalize_locale("pt_BR.UTF-8"), "pt");
        assert_eq!(normalize_locale("ar-SA"), "ar");
        assert_eq!(normalize_locale("zh-Hans-SG"), "zh-CN");
        assert_eq!(normalize_locale("zh-Hant-CN"), "zh-TW");
    }

    #[test]
    fn test_resolve_locale() {
        assert_eq!(resolve_locale("zh-CN"), vec!["zh-CN", "en"]);
        assert_eq!(resolve_locale("zh-TW"), vec!["zh-TW", "zh-CN", "en"]);
        assert_eq!(resolve_locale("en"), vec!["en"]);
    }

    #[test]
    fn all_registered_languages_work_without_a_locale_directory_in_the_project() {
        let registry: Vec<serde_json::Value> = serde_json::from_str(include_str!("../locales/languages.json")).unwrap();
        for metadata in registry {
            let code = metadata["code"].as_str().unwrap();
            let i18n = I18n::load(code, Path::new("__missing_locale_fixture__"));
            assert_eq!(i18n.lang(), code);
            assert!(i18n.has("setup.title"));
            assert_ne!(i18n.t("setup.title"), "setup.title");
        }
    }

    #[test]
    fn partial_or_invalid_project_overrides_keep_the_embedded_language() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("es.json");
        std::fs::write(&source, r#"{"welcome":"Hola personalizado"}"#).unwrap();
        let i18n = I18n::load("es-ES", directory.path());
        assert_eq!(i18n.lang(), "es");
        assert_eq!(i18n.t("welcome"), "Hola personalizado");
        assert_eq!(i18n.t("setup.title"), "Configuración");
        std::fs::write(&source, "[]").unwrap();
        assert_eq!(I18n::load("es", directory.path()).lang(), "es");
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
