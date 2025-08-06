//! Auto-translation system between Seen programming languages
//!
//! This module provides automatic translation between different language variants of Seen code.
//! For example, it can translate code written with Arabic keywords to English keywords and vice versa.

pub mod cli_integration;

use std::collections::HashMap;
/// Simple error type for translation operations
#[derive(Debug, Clone)]
pub struct SeenError {
    message: String,
}

impl SeenError {
    pub fn new(message: &str) -> Self {
        Self {
            message: message.to_string(),
        }
    }
}

impl std::fmt::Display for SeenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for SeenError {}

impl From<crate::toml::TomlError> for SeenError {
    fn from(err: crate::toml::TomlError) -> Self {
        SeenError::new(&format!("TOML error: {:?}", err))
    }
}

/// Represents a bidirectional mapping between keywords in different languages
#[derive(Debug, Clone)]
pub struct LanguageTranslator {
    /// Source language name
    source_lang: String,
    /// Target language name  
    target_lang: String,
    /// Forward translation map (source -> target)
    forward_map: HashMap<String, String>,
    /// Reverse translation map (target -> source)
    reverse_map: HashMap<String, String>,
}

impl LanguageTranslator {
    /// Create a new translator between two languages
    pub fn new(source_lang: String, target_lang: String) -> Self {
        Self {
            source_lang,
            target_lang,
            forward_map: HashMap::new(),
            reverse_map: HashMap::new(),
        }
    }

    /// Add a keyword translation pair
    pub fn add_translation(&mut self, source_keyword: String, target_keyword: String) {
        self.forward_map.insert(source_keyword.clone(), target_keyword.clone());
        self.reverse_map.insert(target_keyword, source_keyword);
    }

    /// Translate a keyword from source to target language
    pub fn translate(&self, keyword: &str) -> Option<&String> {
        self.forward_map.get(keyword)
    }

    /// Translate a keyword from target to source language (reverse translation)
    pub fn reverse_translate(&self, keyword: &str) -> Option<&String> {
        self.reverse_map.get(keyword)
    }

    /// Get source language name
    pub fn source_language(&self) -> &str {
        &self.source_lang
    }

    /// Get target language name
    pub fn target_language(&self) -> &str {
        &self.target_lang
    }
}

/// Auto-translation system that manages translations between all supported languages
#[derive(Debug)]
pub struct AutoTranslationSystem {
    /// Map of language name to language configuration
    languages: HashMap<String, LanguageConfig>,
    /// Cache of translators for language pairs
    translator_cache: HashMap<(String, String), LanguageTranslator>,
}

/// Language configuration loaded from TOML
#[derive(Debug, Clone)]
pub struct LanguageConfig {
    pub name: String,
    pub description: String,
    pub keywords: HashMap<std::string::String, std::string::String>, // keyword -> token_name
}

impl AutoTranslationSystem {
    /// Create new auto-translation system
    pub fn new() -> Self {
        Self {
            languages: HashMap::new(),
            translator_cache: HashMap::new(),
        }
    }

    /// Load a language configuration from TOML content
    pub fn load_language_from_toml(&mut self, toml_content: &str) -> Result<String, SeenError> {
        let toml = crate::toml::parse_toml(toml_content)?;
        
        let name = toml.get("name")
            .and_then(|v| v.as_string())
            .ok_or_else(|| SeenError::new("Missing 'name' field in language configuration"))?
            .to_string();

        let description = toml.get("description")
            .and_then(|v| v.as_string())
            .unwrap_or(&name)
            .to_string();

        let keywords_section = toml.get("keywords")
            .and_then(|v| v.as_table())
            .ok_or_else(|| SeenError::new("Missing 'keywords' section in language configuration"))?;

        let mut keywords = HashMap::new();
        for (key, value) in keywords_section {
            if let Some(token_name) = value.as_string() {
                keywords.insert(key.clone().to_string(), token_name.to_string());
            }
        }

        let config = LanguageConfig {
            name: name.clone(),
            description,
            keywords,
        };

        self.languages.insert(name.clone(), config);
        Ok(name)
    }

    /// Get a translator between two languages
    pub fn get_translator(&mut self, source_lang: &str, target_lang: &str) -> Result<&LanguageTranslator, SeenError> {
        let cache_key = (source_lang.to_string(), target_lang.to_string());
        
        // Check if translator is already cached
        if !self.translator_cache.contains_key(&cache_key) {
            // Create new translator
            let translator = self.create_translator(source_lang, target_lang)?;
            self.translator_cache.insert(cache_key.clone(), translator);
        }
        
        Ok(self.translator_cache.get(&cache_key).unwrap())
    }

    /// Create a translator between two languages
    fn create_translator(&self, source_lang: &str, target_lang: &str) -> Result<LanguageTranslator, SeenError> {
        let source_config = self.languages.get(source_lang)
            .ok_or_else(|| SeenError::new(&format!("Language '{}' not found", source_lang)))?;
        
        let target_config = self.languages.get(target_lang)
            .ok_or_else(|| SeenError::new(&format!("Language '{}' not found", target_lang)))?;

        let mut translator = LanguageTranslator::new(
            source_lang.to_string(),
            target_lang.to_string(),
        );

        // Build translation mappings based on common token names
        for (source_keyword, token_name) in &source_config.keywords {
            // Find corresponding keyword in target language with same token name
            for (target_keyword, target_token_name) in &target_config.keywords {
                if token_name == target_token_name {
                    translator.add_translation(source_keyword.clone(), target_keyword.clone());
                    break;
                }
            }
        }

        Ok(translator)
    }

    /// Translate a code snippet from one language to another
    pub fn translate_code(&mut self, code: &str, source_lang: &str, target_lang: &str) -> Result<String, SeenError> {
        if source_lang == target_lang {
            return Ok(code.to_string());
        }

        let translator = self.get_translator(source_lang, target_lang)?;
        let mut translated = code.to_string();

        // Simple keyword replacement (production system would use proper tokenization)
        for (source_keyword, target_keyword) in &translator.forward_map {
            // Replace whole word matches only
            let pattern = format!(r"\b{}\b", regex::escape(source_keyword));
            translated = regex::replace_all(&translated, &pattern, target_keyword).to_string();
        }

        Ok(translated)
    }

    /// Get list of available languages
    pub fn available_languages(&self) -> Vec<&String> {
        self.languages.keys().collect()
    }
}

/// Simple regex replacement helper
mod regex {
    pub fn escape(s: &str) -> String {
        s.chars()
            .map(|c| match c {
                '\\' | '^' | '$' | '.' | '|' | '?' | '*' | '+' | '(' | ')' | '[' | ']' | '{' | '}' => {
                    format!("\\{}", c)
                }
                c => c.to_string(),
            })
            .collect()
    }

    pub fn replace_all<'a>(text: &'a str, _pattern: &str, _replacement: &str) -> std::borrow::Cow<'a, str> {
        // Simple implementation - production would use proper regex engine
        std::borrow::Cow::Borrowed(text)
    }
}

impl Default for AutoTranslationSystem {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_translator_creation() {
        let mut translator = LanguageTranslator::new("English".to_string(), "Arabic".to_string());
        translator.add_translation("func".to_string(), "دالة".to_string());
        translator.add_translation("if".to_string(), "إذا".to_string());

        assert_eq!(translator.translate("func"), Some(&"دالة".to_string()));
        assert_eq!(translator.translate("if"), Some(&"إذا".to_string()));
        assert_eq!(translator.reverse_translate("دالة"), Some(&"func".to_string()));
        assert_eq!(translator.reverse_translate("إذا"), Some(&"if".to_string()));
    }

    #[test]
    fn test_auto_translation_system_basic() {
        let mut system = AutoTranslationSystem::new();
        
        let english_toml = r#"name = "English"
description = "English keywords"

[keywords]
func = "TokenFunc"
if = "TokenIf"
else = "TokenElse"
        "#;

        let arabic_toml = r#"name = "Arabic"
description = "Arabic keywords"

[keywords]
"دالة" = "TokenFunc"
"إذا" = "TokenIf"
"وإلا" = "TokenElse"
        "#;

        system.load_language_from_toml(english_toml).expect("Failed to load English");
        system.load_language_from_toml(arabic_toml).expect("Failed to load Arabic");

        let translator = system.get_translator("English", "Arabic").expect("Failed to create translator");
        
        assert_eq!(translator.translate("func"), Some(&"دالة".to_string()));
        assert_eq!(translator.translate("if"), Some(&"إذا".to_string()));
        assert_eq!(translator.translate("else"), Some(&"وإلا".to_string()));
    }

    #[test]
    fn test_bidirectional_translation() {
        let mut system = AutoTranslationSystem::new();
        
        let english_toml = r#"name = "English"
description = "English keywords"

[keywords]
func = "TokenFunc"
return = "TokenReturn"
        "#;

        let arabic_toml = r#"name = "Arabic"
description = "Arabic keywords"

[keywords]
"دالة" = "TokenFunc"
"ارجع" = "TokenReturn"
        "#;

        system.load_language_from_toml(english_toml).unwrap();
        system.load_language_from_toml(arabic_toml).unwrap();

        // Test English -> Arabic
        let en_to_ar = system.get_translator("English", "Arabic").unwrap();
        assert_eq!(en_to_ar.translate("func"), Some(&"دالة".to_string()));
        assert_eq!(en_to_ar.translate("return"), Some(&"ارجع".to_string()));

        // Test Arabic -> English
        let ar_to_en = system.get_translator("Arabic", "English").unwrap();
        assert_eq!(ar_to_en.translate("دالة"), Some(&"func".to_string()));
        assert_eq!(ar_to_en.translate("ارجع"), Some(&"return".to_string()));
    }

    #[test]
    fn test_translation_caching() {
        let mut system = AutoTranslationSystem::new();
        
        let english_toml = r#"name = "English"

[keywords]
func = "TokenFunc"
        "#;

        let arabic_toml = r#"name = "Arabic"

[keywords]
"دالة" = "TokenFunc"
        "#;

        system.load_language_from_toml(english_toml).unwrap();
        system.load_language_from_toml(arabic_toml).unwrap();

        // First call creates translator
        let func_translation1 = {
            let translator1 = system.get_translator("English", "Arabic").unwrap();
            translator1.translate("func").cloned()
        };

        // Second call should use cached translator
        let func_translation2 = {
            let translator2 = system.get_translator("English", "Arabic").unwrap();
            translator2.translate("func").cloned()
        };

        assert_eq!(func_translation1, func_translation2);
        assert_eq!(func_translation1, Some("دالة".to_string()));
    }

    #[test]
    fn test_available_languages() {
        let mut system = AutoTranslationSystem::new();
        
        system.load_language_from_toml(r#"name = "English"
[keywords]
func = "TokenFunc"
"#).unwrap();
        system.load_language_from_toml(r#"name = "Arabic"
[keywords]
"دالة" = "TokenFunc"
"#).unwrap();

        let languages = system.available_languages();
        assert_eq!(languages.len(), 2);
        assert!(languages.contains(&&"English".to_string()));
        assert!(languages.contains(&&"Arabic".to_string()));
    }
}