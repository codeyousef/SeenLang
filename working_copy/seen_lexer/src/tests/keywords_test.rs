//\! Tests for keyword tokenization in both English and Arabic

use super::*;
use crate::{KeywordManager, Lexer, Token, TokenType};
use pretty_assertions::assert_eq;

#[test]
fn test_english_func_keyword() {
    let keyword_manager = create_test_keyword_manager("english");
    let mut lexer = Lexer::new("func", &keyword_manager);
    let tokens = lexer.tokenize().unwrap();

    assert_eq!(tokens.len(), 2); // Token + EOF
    assert_eq!(tokens[0].token_type, TokenType::Func);
    assert_eq!(tokens[0].lexeme, "func");
    assert_eq!(tokens[0].language, "english");
}

#[test]
fn test_arabic_func_keyword() {
    let keyword_manager = create_test_keyword_manager("arabic");
    let mut lexer = Lexer::new("دالة", &keyword_manager);
    let tokens = lexer.tokenize().unwrap();

    assert_eq!(tokens.len(), 2); // Token + EOF
    assert_eq!(tokens[0].token_type, TokenType::Func);
    assert_eq!(tokens[0].lexeme, "دالة");
    assert_eq!(tokens[0].language, "arabic");
}

#[test]
fn test_english_all_keywords() {
    let test_cases = vec![
        ("func", TokenType::Func),
        ("if", TokenType::If),
        ("else", TokenType::Else),
        ("while", TokenType::While),
        ("for", TokenType::For),
        ("return", TokenType::Return),
        ("val", TokenType::Val),
        ("var", TokenType::Var),
        ("true", TokenType::True),
        ("false", TokenType::False),
    ];

    let keyword_manager = create_test_keyword_manager("english");

    for (keyword, expected_type) in test_cases {
        let mut lexer = Lexer::new(keyword, &keyword_manager);
        let tokens = lexer.tokenize().unwrap();

        assert_eq!(tokens[0].token_type, expected_type,
                   "Failed for keyword: {}", keyword);
        assert_eq!(tokens[0].lexeme, keyword);
    }
}

#[test]
fn test_arabic_all_keywords() {
    let test_cases = vec![
        ("دالة", TokenType::Func),
        ("إذا", TokenType::If),
        ("وإلا", TokenType::Else),
        ("بينما", TokenType::While),
        ("لكل", TokenType::For),
        ("ارجع", TokenType::Return),
        ("ثابت", TokenType::Val),
        ("متغير", TokenType::Var),
        ("صحيح", TokenType::True),
        ("خطأ", TokenType::False),
    ];

    let keyword_manager = create_test_keyword_manager("arabic");

    for (keyword, expected_type) in test_cases {
        let mut lexer = Lexer::new(keyword, &keyword_manager);
        let tokens = lexer.tokenize().unwrap();

        assert_eq!(tokens[0].token_type, expected_type,
                   "Failed for keyword: {}", keyword);
        assert_eq!(tokens[0].lexeme, keyword);
    }
}

#[test]
fn test_keyword_vs_identifier_disambiguation() {
    let keyword_manager = create_test_keyword_manager("english");

    // "func" is a keyword in English
    let mut lexer = Lexer::new("func", &keyword_manager);
    let tokens = lexer.tokenize().unwrap();
    assert_eq!(tokens[0].token_type, TokenType::Func);

    // "دالة" is just an identifier in English mode
    let mut lexer = Lexer::new("دالة", &keyword_manager);
    let tokens = lexer.tokenize().unwrap();
    assert_eq!(tokens[0].token_type, TokenType::Identifier);
    assert_eq!(tokens[0].lexeme, "دالة");
}

#[test]
fn test_complete_function_english() {
    let keyword_manager = create_test_keyword_manager("english");
    let source = "func add(a: Int, b: Int) -> Int { return a + b; }";

    let mut lexer = Lexer::new(source, &keyword_manager);
    let tokens = lexer.tokenize().unwrap();

    // Verify key tokens
    assert!(tokens.iter().any(< /dev / null | t | t.token_type == TokenType::Func));
    assert!(tokens.iter().any(|t| t.token_type == TokenType::Return));
    assert!(tokens.iter().any(|t| t.token_type == TokenType::Identifier && t.lexeme == "add"));
}

#[test]
fn test_complete_function_arabic() {
    let keyword_manager = create_test_keyword_manager("arabic");
    let source = "دالة جمع(أ: صحيح، ب: صحيح) -> صحيح { ارجع أ + ب؛ }";

    let mut lexer = Lexer::new(source, &keyword_manager);
    let tokens = lexer.tokenize().unwrap();

    // Verify key tokens
    assert!(tokens.iter().any(|t| t.token_type == TokenType::Func));
    assert!(tokens.iter().any(|t| t.token_type == TokenType::Return));
    assert!(tokens.iter().any(|t| t.token_type == TokenType::Identifier && t.lexeme == "جمع"));
}

// Helper function to create test keyword manager
fn create_test_keyword_manager(language: &str) -> KeywordManager {
    KeywordManager::new_for_testing(language)
}
