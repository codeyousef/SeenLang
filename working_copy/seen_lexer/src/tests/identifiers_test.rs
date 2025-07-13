//\! Tests for identifier tokenization and Unicode support

use super::*;
use crate::{Lexer, TokenType, KeywordManager};
use pretty_assertions::assert_eq;
use test_case::test_case;

#[test_case("x")]
#[test_case("myVar")]
#[test_case("_private")]
#[test_case("number123")]
#[test_case("camelCase")]
#[test_case("snake_case")]
#[test_case("CONSTANT")]
fn test_ascii_identifiers(input: &str) {
    let keyword_manager = KeywordManager::new_for_testing("english");
    let mut lexer = Lexer::new(input, &keyword_manager);
    let tokens = lexer.tokenize().unwrap();
    
    assert_eq!(tokens[0].token_type, TokenType::Identifier);
    assert_eq!(tokens[0].lexeme, input);
}

#[test]
fn test_arabic_identifiers() {
    let test_cases = vec![
        "متغير",
        "رقم",
        "نص",
        "قائمة",
        "دالة_مساعدة",
        "متغير123",
    ];
    
    let keyword_manager = KeywordManager::new_for_testing("english");
    
    for identifier in test_cases {
        let mut lexer = Lexer::new(identifier, &keyword_manager);
        let tokens = lexer.tokenize().unwrap();
        
        assert_eq!(tokens[0].token_type, TokenType::Identifier);
        assert_eq!(tokens[0].lexeme, identifier);
    }
}

#[test]
fn test_mixed_script_identifiers() {
    let test_cases = vec![
        "user_اسم",
        "data_بيانات",
        "mixed_مختلط_123",
    ];
    
    let keyword_manager = KeywordManager::new_for_testing("english");
    
    for identifier in test_cases {
        let mut lexer = Lexer::new(identifier, &keyword_manager);
        let tokens = lexer.tokenize().unwrap();
        
        assert_eq!(tokens[0].token_type, TokenType::Identifier);
        assert_eq!(tokens[0].lexeme, identifier);
    }
}

#[test]
fn test_invalid_identifier_start() {
    let invalid_starts = vec![
        "123abc",  // Starts with digit
        "-identifier",  // Starts with operator
        "@name",  // Starts with invalid character
    ];
    
    let keyword_manager = KeywordManager::new_for_testing("english");
    
    for input in invalid_starts {
        let mut lexer = Lexer::new(input, &keyword_manager);
        let tokens = lexer.tokenize().unwrap();
        
        // Should not tokenize as a single identifier
        assert_ne!(tokens[0].token_type, TokenType::Identifier);
    }
}

#[test]
fn test_unicode_normalization() {
    // Test that different Unicode representations of the same character
    // are handled correctly
    let keyword_manager = KeywordManager::new_for_testing("english");
    
    // These are different Unicode representations of "é"
    let e_acute_composed = "café";  // é as single character
    let e_acute_decomposed = "cafe\u{0301}";  // e + combining acute accent
    
    let mut lexer1 = Lexer::new(e_acute_composed, &keyword_manager);
    let tokens1 = lexer1.tokenize().unwrap();
    
    let mut lexer2 = Lexer::new(e_acute_decomposed, &keyword_manager);
    let tokens2 = lexer2.tokenize().unwrap();
    
    assert_eq!(tokens1[0].token_type, TokenType::Identifier);
    assert_eq!(tokens2[0].token_type, TokenType::Identifier);
    // Note: Exact equality depends on normalization strategy
}
