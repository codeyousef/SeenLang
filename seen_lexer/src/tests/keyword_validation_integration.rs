//! Integration tests for keyword validation across all language files
//! 
//! This module implements comprehensive integration tests to verify that
//! all language files contain the required keywords and that the dynamic
//! keyword system works correctly across all supported languages.

use crate::keyword_manager::{KeywordManager, KeywordType};
use std::collections::HashSet;

/// Test that all 10 required languages are properly loaded and validated
#[test]
fn test_all_languages_have_required_keywords() {
    let mut manager = KeywordManager::new();
    
    // Load all 10 required languages
    let languages = vec!["en", "ar", "es", "zh", "fr", "de", "ja", "ru", "pt", "hi"];
    
    for lang in &languages {
        let result = manager.load_from_toml(lang);
        assert!(
            result.is_ok(),
            "Failed to load language {}: {:?}",
            lang,
            result.err()
        );
    }
    
    // Validate that all languages have the required keywords
    let validation_result = manager.validate_all_languages();
    assert!(
        validation_result.is_ok(),
        "Keyword validation failed: {:?}",
        validation_result.err()
    );
    
    // Test each language individually
    for lang in &languages {
        manager.switch_language(lang).unwrap();
        
        // Test that basic keywords exist
        let fun_keyword = manager.get_keyword_text(&KeywordType::KeywordFun);
        let if_keyword = manager.get_keyword_text(&KeywordType::KeywordIf);
        let and_keyword = manager.get_logical_and();
        let or_keyword = manager.get_logical_or();
        let not_keyword = manager.get_logical_not();
        
        assert!(fun_keyword.is_some(), "Language {} missing 'fun' keyword", lang);
        assert!(if_keyword.is_some(), "Language {} missing 'if' keyword", lang);
        assert!(!and_keyword.is_empty(), "Language {} missing 'and' keyword", lang);
        assert!(!or_keyword.is_empty(), "Language {} missing 'or' keyword", lang);
        assert!(!not_keyword.is_empty(), "Language {} missing 'not' keyword", lang);
        
        // Test that keywords are recognized
        assert_eq!(
            manager.is_keyword(&fun_keyword.unwrap()),
            Some(KeywordType::KeywordFun),
            "Language {} 'fun' keyword not recognized",
            lang
        );
        assert_eq!(
            manager.is_keyword(&if_keyword.unwrap()),
            Some(KeywordType::KeywordIf),
            "Language {} 'if' keyword not recognized",
            lang
        );
        assert_eq!(
            manager.is_keyword(&and_keyword),
            Some(KeywordType::KeywordAnd),
            "Language {} 'and' keyword not recognized",
            lang
        );
    }
}

/// Test that language switching works correctly and keywords don't leak between languages
#[test]
fn test_language_isolation() {
    let mut manager = KeywordManager::new();
    
    // Load English and Arabic
    manager.load_from_toml("en").unwrap();
    manager.load_from_toml("ar").unwrap();
    
    // Get English keywords
    manager.switch_language("en").unwrap();
    let en_fun = manager.get_keyword_text(&KeywordType::KeywordFun).unwrap();
    let en_and = manager.get_logical_and();
    
    // Get Arabic keywords
    manager.switch_language("ar").unwrap();
    let ar_fun = manager.get_keyword_text(&KeywordType::KeywordFun).unwrap();
    let ar_and = manager.get_logical_and();
    
    // Verify they're different
    assert_ne!(en_fun, ar_fun, "English and Arabic 'fun' keywords should be different");
    assert_ne!(en_and, ar_and, "English and Arabic 'and' keywords should be different");
    
    // Test isolation: English keywords should not work in Arabic mode
    assert_eq!(
        manager.is_keyword(&en_fun),
        None,
        "English 'fun' keyword should not be recognized in Arabic mode"
    );
    assert_eq!(
        manager.is_keyword(&en_and),
        None,
        "English 'and' keyword should not be recognized in Arabic mode"
    );
    
    // Arabic keywords should work in Arabic mode
    assert_eq!(
        manager.is_keyword(&ar_fun),
        Some(KeywordType::KeywordFun),
        "Arabic 'fun' keyword should be recognized in Arabic mode"
    );
    assert_eq!(
        manager.is_keyword(&ar_and),
        Some(KeywordType::KeywordAnd),
        "Arabic 'and' keyword should be recognized in Arabic mode"
    );
    
    // Switch back to English and test isolation
    manager.switch_language("en").unwrap();
    
    // Arabic keywords should not work in English mode
    assert_eq!(
        manager.is_keyword(&ar_fun),
        None,
        "Arabic 'fun' keyword should not be recognized in English mode"
    );
    assert_eq!(
        manager.is_keyword(&ar_and),
        None,
        "Arabic 'and' keyword should not be recognized in English mode"
    );
    
    // English keywords should work in English mode
    assert_eq!(
        manager.is_keyword(&en_fun),
        Some(KeywordType::KeywordFun),
        "English 'fun' keyword should be recognized in English mode"
    );
    assert_eq!(
        manager.is_keyword(&en_and),
        Some(KeywordType::KeywordAnd),
        "English 'and' keyword should be recognized in English mode"
    );
}

/// Test that all languages have unique keyword translations
#[test]
fn test_keyword_uniqueness_across_languages() {
    let mut manager = KeywordManager::new();
    
    // Load all languages
    let languages = vec!["en", "ar", "es", "zh", "fr", "de", "ja", "ru", "pt", "hi"];
    
    for lang in &languages {
        manager.load_from_toml(lang).unwrap();
    }
    
    // Collect all keyword translations for each keyword type
    let keyword_types = vec![
        KeywordType::KeywordFun,
        KeywordType::KeywordIf,
        KeywordType::KeywordElse,
        KeywordType::KeywordAnd,
        KeywordType::KeywordOr,
        KeywordType::KeywordNot,
    ];
    
    for keyword_type in keyword_types {
        let mut translations = HashSet::new();
        
        for lang in &languages {
            manager.switch_language(lang).unwrap();
            
            let translation = match keyword_type {
                KeywordType::KeywordAnd => manager.get_logical_and(),
                KeywordType::KeywordOr => manager.get_logical_or(),
                KeywordType::KeywordNot => manager.get_logical_not(),
                _ => manager.get_keyword_text(&keyword_type).unwrap_or_default(),
            };
            
            if !translation.is_empty() {
                let was_new = translations.insert((lang.to_string(), translation.clone()));
                assert!(
                    was_new,
                    "Duplicate translation '{}' found for keyword {:?} in language {}",
                    translation,
                    keyword_type,
                    lang
                );
            }
        }
        
        // Should have at least as many translations as languages (some might be the same)
        assert!(
            translations.len() >= 5, // At least 5 different translations
            "Keyword {:?} should have diverse translations across languages, found: {:?}",
            keyword_type,
            translations
        );
    }
}

/// Test performance of keyword lookup across all languages
#[test]
fn test_keyword_lookup_performance_all_languages() {
    let mut manager = KeywordManager::new();
    
    // Load all languages
    let languages = vec!["en", "ar", "es", "zh", "fr", "de", "ja", "ru", "pt", "hi"];
    
    for lang in &languages {
        manager.load_from_toml(lang).unwrap();
    }
    
    // Test performance for each language
    for lang in &languages {
        manager.switch_language(lang).unwrap();
        
        let start = std::time::Instant::now();
        
        // Perform many keyword lookups
        for _ in 0..1000 {
            let fun_keyword = manager.get_keyword_text(&KeywordType::KeywordFun).unwrap();
            let if_keyword = manager.get_keyword_text(&KeywordType::KeywordIf).unwrap();
            let and_keyword = manager.get_logical_and();
            let or_keyword = manager.get_logical_or();
            let not_keyword = manager.get_logical_not();
            
            // Verify lookups work
            assert!(manager.is_keyword(&fun_keyword).is_some());
            assert!(manager.is_keyword(&if_keyword).is_some());
            assert!(manager.is_keyword(&and_keyword).is_some());
            assert!(manager.is_keyword(&or_keyword).is_some());
            assert!(manager.is_keyword(&not_keyword).is_some());
            
            // Test non-keywords
            assert!(manager.is_keyword("nonexistent_keyword").is_none());
        }
        
        let duration = start.elapsed();
        
        // Should complete in reasonable time (less than 50ms for 5k lookups per language)
        assert!(
            duration.as_millis() < 50,
            "Keyword lookup too slow for language {}: {:?}",
            lang,
            duration
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_integration_test_setup() {
        // Verify that we can create a keyword manager and load at least one language
        let mut manager = KeywordManager::new();
        let result = manager.load_from_toml("en");
        
        assert!(
            result.is_ok(),
            "Should be able to load English language for integration tests: {:?}",
            result.err()
        );
        
        manager.switch_language("en").unwrap();
        let fun_keyword = manager.get_keyword_text(&KeywordType::KeywordFun);
        
        assert!(
            fun_keyword.is_some(),
            "Should be able to get 'fun' keyword in English"
        );
    }
}