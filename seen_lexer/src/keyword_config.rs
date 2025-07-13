use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

use crate::token::TokenType;

/// Errors that can occur when working with keyword configurations
#[derive(Error, Debug)]
pub enum KeywordConfigError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("TOML parsing error: {0}")]
    TomlError(#[from] toml::de::Error),

    #[error("Schema validation error: {0}")]
    SchemaError(String),

    #[error("Language '{0}' not found in keyword configuration")]
    LanguageNotFound(String),

    #[error("Invalid keyword configuration: {0}")]
    InvalidConfig(String),

    #[error("Language file not found: {0}")]
    LanguageFileNotFound(PathBuf),
}

/// Represents a supported language in the keywords configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Language {
    pub code: String,
    pub name: String,
    pub direction: String,
}

/// Represents a single language keyword file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageKeywords {
    pub language: Language,
    pub keywords: HashMap<String, String>,
}

/// Represents the entire keyword mapping configuration
#[derive(Debug, Clone)]
pub struct KeywordConfig {
    /// Mapping from language code to language metadata
    pub languages: HashMap<String, Language>,

    /// Mapping from token name to its representation in each language
    /// E.g., "val" -> {"en": "val", "ar": "ثابت"}
    pub keyword_mappings: HashMap<String, HashMap<String, String>>,

    /// Base directory where language files are stored
    pub base_dir: PathBuf,
}

impl KeywordConfig {
    /// Load keyword configuration from separate language files in a directory
    pub fn from_directory<P: AsRef<Path>>(directory: P) -> Result<Self, KeywordConfigError> {
        let dir_path = directory.as_ref();
        let base_dir = dir_path.to_path_buf();

        // Look for language files in the directory
        let mut languages = HashMap::new();
        let mut keyword_mappings = HashMap::new();

        // Check for english.toml and arabic.toml
        let english_path = dir_path.join("english.toml");
        let arabic_path = dir_path.join("arabic.toml");

        // Load English keywords
        if english_path.exists() {
            let english_keywords: LanguageKeywords = Self::load_language_file(&english_path)?;
            languages.insert(
                english_keywords.language.code.clone(),
                english_keywords.language.clone(),
            );

            // Add keywords to the mappings
            for (token_name, keyword) in english_keywords.keywords {
                let lang_map = keyword_mappings
                    .entry(token_name)
                    .or_insert_with(HashMap::new);
                lang_map.insert(english_keywords.language.code.clone(), keyword);
            }
        } else {
            return Err(KeywordConfigError::LanguageFileNotFound(english_path));
        }

        // Load Arabic keywords
        if arabic_path.exists() {
            let arabic_keywords: LanguageKeywords = Self::load_language_file(&arabic_path)?;
            languages.insert(
                arabic_keywords.language.code.clone(),
                arabic_keywords.language.clone(),
            );

            // Add keywords to the mappings
            for (token_name, keyword) in arabic_keywords.keywords {
                let lang_map = keyword_mappings
                    .entry(token_name)
                    .or_insert_with(HashMap::new);
                lang_map.insert(arabic_keywords.language.code.clone(), keyword);
            }
        } else {
            return Err(KeywordConfigError::LanguageFileNotFound(arabic_path));
        }

        // Validate the configuration
        if languages.is_empty() {
            return Err(KeywordConfigError::InvalidConfig(
                "No languages defined in keyword configuration".to_string(),
            ));
        }

        if keyword_mappings.is_empty() {
            return Err(KeywordConfigError::InvalidConfig(
                "No keyword mappings defined".to_string(),
            ));
        }

        Ok(Self {
            languages,
            keyword_mappings,
            base_dir,
        })
    }

    /// Load a single language file
    fn load_language_file<P: AsRef<Path>>(path: P) -> Result<LanguageKeywords, KeywordConfigError> {
        let content = fs::read_to_string(&path)?;

        #[derive(Deserialize)]
        struct RawLanguageFile {
            language: Language,
            keywords: HashMap<String, String>,
        }

        let raw_file: RawLanguageFile = toml::from_str(&content)?;

        Ok(LanguageKeywords {
            language: raw_file.language,
            keywords: raw_file.keywords,
        })
    }

    /// Check if a language is supported
    pub fn is_language_supported(&self, language_code: &str) -> bool {
        self.languages.contains_key(language_code)
    }

    /// Get the list of language codes
    pub fn get_language_codes(&self) -> Vec<String> {
        self.languages.keys().cloned().collect()
    }

    /// Get the base directory where language files are stored
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }
}

/// Maps between language-specific keywords and internal token types
pub struct KeywordManager {
    /// The configuration loaded from language files
    config: KeywordConfig,

    /// Maps from language code + keyword to token type (for lexer)
    keyword_to_token: HashMap<String, TokenType>,

    /// Maps from token type to keyword in a specific language (for pretty printing)
    token_to_keyword: HashMap<TokenType, HashMap<String, String>>,

    /// The currently active language code
    active_language: String,
}

impl KeywordManager {
    pub fn new(config: KeywordConfig, active_language: String) -> Result<Self, KeywordConfigError> {
        if !config.is_language_supported(&active_language) {
            return Err(KeywordConfigError::LanguageNotFound(active_language));
        }

        let mut keyword_to_token = HashMap::new();
        let mut token_to_keyword = HashMap::new();

        // Build mappings in both directions
        for (token_name, lang_mappings) in &config.keyword_mappings {
            let token_type = match token_name.as_str() {
                "val" => TokenType::Val,
                "var" => TokenType::Var,
                "func" => TokenType::Func,
                "if" => TokenType::If,
                "else" => TokenType::Else,
                "while" => TokenType::While,
                "for" => TokenType::For,
                "return" => TokenType::Return,
                "true" => TokenType::True,
                "false" => TokenType::False,
                "null" => TokenType::Null,
                "println" => TokenType::Println,
                "when" => TokenType::When,
                "in" => TokenType::In,
                "loop" => TokenType::Loop,
                "break" => TokenType::Break,
                "continue" => TokenType::Continue,
                "struct" => TokenType::Struct,
                "enum" => TokenType::Enum,
                "unsafe" => TokenType::Unsafe,
                "ref" => TokenType::Ref,
                "own" => TokenType::Own,
                "async" => TokenType::Async,
                "await" => TokenType::Await,
                _ => continue, // Skip unknown token types
            };

            let mut token_keywords = HashMap::new();

            for (lang_code, keyword) in lang_mappings {
                // For keyword → token lookup (used by lexer)
                let key = format!("{}{}", lang_code, keyword);
                keyword_to_token.insert(key, token_type.clone());

                // For token → keyword lookup (used for pretty printing)
                token_keywords.insert(lang_code.clone(), keyword.clone());
            }

            token_to_keyword.insert(token_type, token_keywords);
        }

        Ok(Self {
            config,
            keyword_to_token,
            token_to_keyword,
            active_language,
        })
    }

    /// Check if a string is a keyword in the active language
    pub fn is_keyword(&self, text: &str) -> bool {
        let key = format!("{}{}", self.active_language, text);
        self.keyword_to_token.contains_key(&key)
    }

    /// Get the token type for a keyword in the active language
    pub fn get_token_type(&self, text: &str) -> Option<TokenType> {
        let key = format!("{}{}", self.active_language, text);
        self.keyword_to_token.get(&key).cloned()
    }

    /// Get the keyword for a token type in the active language
    pub fn get_keyword(&self, token_type: &TokenType) -> Option<&String> {
        self.token_to_keyword
            .get(token_type)
            .and_then(|langs| langs.get(&self.active_language))
    }

    /// Set the active language
    pub fn set_active_language(&mut self, language_code: String) -> Result<(), KeywordConfigError> {
        if !self.config.is_language_supported(&language_code) {
            return Err(KeywordConfigError::LanguageNotFound(language_code));
        }

        self.active_language = language_code;
        Ok(())
    }

    /// Get the active language
    pub fn get_active_language(&self) -> &str {
        &self.active_language
    }
}
