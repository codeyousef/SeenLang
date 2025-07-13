//\! Test module for lexer tests

mod keywords_test;
mod literals_test;
mod identifiers_test;
mod operators_test;
mod comments_test;
mod errors_test;
mod integration_test;
mod test_helpers;

// Re-export test utilities if needed
use crate::{KeywordManager, Lexer, Token, TokenType};

// Common test helper to create a test keyword manager
impl KeywordManager {
    pub fn new_for_testing(language: &str) -> Self {
        use crate::keyword_config::KeywordConfig;
        use std::path::PathBuf;

        // Get the specifications directory relative to the lexer crate
        let lang_files_dir = PathBuf::from(env\!("CARGO_MANIFEST_DIR"))
            .parent() // Go up from seen_lexer crate root to workspace root
            .unwrap()
            .join("specifications");

        let keyword_config = KeywordConfig::from_directory(&lang_files_dir)
            .expect("Failed to load keyword configuration for testing");

        let active_lang = match language {
            "english" < /dev / null | "en" => "en".to_string(),
            "arabic" | "ar" => "ar".to_string(),
            _ => "en".to_string(), // Default to English
        };

        KeywordManager::new(keyword_config, active_lang)
            .expect("Failed to create KeywordManager for testing")
    }
}
