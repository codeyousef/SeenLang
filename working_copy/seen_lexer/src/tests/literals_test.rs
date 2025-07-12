//\! Tests for literal tokenization (numbers, strings)

use super::*;
use crate::{Lexer, TokenType, KeywordManager};
use pretty_assertions::assert_eq;
use test_case::test_case;

#[test_case("0", 0)]
#[test_case("42", 42)]
#[test_case("1234567890", 1234567890)]
fn test_integer_literals(input: &str, expected: i64) {
    let keyword_manager = KeywordManager::new_for_testing("english");
    let mut lexer = Lexer::new(input, &keyword_manager);
    let tokens = lexer.tokenize().unwrap();
    
    assert_eq\!(tokens[0].token_type, TokenType::IntegerLiteral);
    assert_eq\!(tokens[0].lexeme, input);
    // TODO: Add value parsing verification when implemented
}

#[test_case("0.0")]
#[test_case("3.14")]
#[test_case("123.456")]
#[test_case("1.0e10")]
#[test_case("1.5e-5")]
fn test_float_literals(input: &str) {
    let keyword_manager = KeywordManager::new_for_testing("english");
    let mut lexer = Lexer::new(input, &keyword_manager);
    let tokens = lexer.tokenize().unwrap();
    
    assert_eq\!(tokens[0].token_type, TokenType::FloatLiteral);
    assert_eq\!(tokens[0].lexeme, input);
}

#[test]
fn test_string_literals_basic() {
    let test_cases = vec\![
        ("\"\"", ""),
        ("\"hello\"", "hello"),
        ("\"Hello, World\!\"", "Hello, World\!"),
        ("\"مرحباً\"", "مرحباً"),
    ];
    
    let keyword_manager = KeywordManager::new_for_testing("english");
    
    for (input, expected_content) in test_cases {
        let mut lexer = Lexer::new(input, &keyword_manager);
        let tokens = lexer.tokenize().unwrap();
        
        assert_eq\!(tokens[0].token_type, TokenType::StringLiteral);
        // The lexeme includes quotes
        assert_eq\!(tokens[0].lexeme, input);
    }
}

#[test]
fn test_string_with_escapes() {
    let test_cases = vec\![
        ("\"\\n\"", "newline"),
        ("\"\\t\"", "tab"),
        ("\"\\r\"", "carriage return"),
        ("\"\\0\"", "null"),
        ("\"\\\"\"", "quote"),
        ("\"\\\\\"", "backslash"),
    ];
    
    let keyword_manager = KeywordManager::new_for_testing("english");
    
    for (input, _description) in test_cases {
        let mut lexer = Lexer::new(input, &keyword_manager);
        let tokens = lexer.tokenize().unwrap();
        
        assert_eq\!(tokens[0].token_type, TokenType::StringLiteral);
    }
}

#[test]
fn test_unterminated_string_error() {
    let keyword_manager = KeywordManager::new_for_testing("english");
    let mut lexer = Lexer::new("\"unterminated", &keyword_manager);
    
    let result = lexer.tokenize();
    assert\!(result.is_err(), "Expected error for unterminated string");
}

#[test]
fn test_mixed_literals() {
    let keyword_manager = KeywordManager::new_for_testing("english");
    let source = r#"42 3.14 "hello" 0 "world""#;
    
    let mut lexer = Lexer::new(source, &keyword_manager);
    let tokens = lexer.tokenize().unwrap();
    
    // Filter out EOF token
    let tokens: Vec<_> = tokens.into_iter()
        .filter(|t| t.token_type \!= TokenType::Eof)
        .collect();
    
    assert_eq\!(tokens.len(), 5);
    assert_eq\!(tokens[0].token_type, TokenType::IntegerLiteral);
    assert_eq\!(tokens[1].token_type, TokenType::FloatLiteral);
    assert_eq\!(tokens[2].token_type, TokenType::StringLiteral);
    assert_eq\!(tokens[3].token_type, TokenType::IntegerLiteral);
    assert_eq\!(tokens[4].token_type, TokenType::StringLiteral);
}
