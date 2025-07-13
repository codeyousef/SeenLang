//\! Tests for operator and delimiter tokenization

use super::*;
use crate::{KeywordManager, Lexer, TokenType};
use pretty_assertions::assert_eq;
use test_case::test_case;

#[test_case("+", TokenType::Plus)]
#[test_case("-", TokenType::Minus)]
#[test_case("*", TokenType::Star)]
#[test_case("/", TokenType::Slash)]
#[test_case("%", TokenType::Percent)]
#[test_case("=", TokenType::Equal)]
#[test_case("\!", TokenType::Bang)]
#[test_case("<", TokenType::Less)]
#[test_case(">", TokenType::Greater)]
#[test_case("&", TokenType::Ampersand)]
#[test_case("|", TokenType::Pipe)]
fn test_single_char_operators(input: &str, expected: TokenType) {
    let keyword_manager = KeywordManager::new_for_testing("english");
    let mut lexer = Lexer::new(input, &keyword_manager);
    let tokens = lexer.tokenize().unwrap();

    assert_eq!(tokens[0].token_type, expected);
    assert_eq!(tokens[0].lexeme, input);
}

#[test_case("==", TokenType::EqualEqual)]
#[test_case("\!=", TokenType::BangEqual)]
#[test_case("<=", TokenType::LessEqual)]
#[test_case(">=", TokenType::GreaterEqual)]
#[test_case("&&", TokenType::AmpersandAmpersand)]
#[test_case("||", TokenType::PipePipe)]
#[test_case("->", TokenType::Arrow)]
#[test_case("::", TokenType::ColonColon)]
fn test_multi_char_operators(input: &str, expected: TokenType) {
    let keyword_manager = KeywordManager::new_for_testing("english");
    let mut lexer = Lexer::new(input, &keyword_manager);
    let tokens = lexer.tokenize().unwrap();

    assert_eq!(tokens[0].token_type, expected);
    assert_eq!(tokens[0].lexeme, input);
}

#[test]
fn test_operator_sequences() {
    let keyword_manager = KeywordManager::new_for_testing("english");
    let source = "a + b - c * d / e";

    let mut lexer = Lexer::new(source, &keyword_manager);
    let tokens = lexer.tokenize().unwrap();

    // Find operator tokens
    let operators: Vec<_> = tokens.iter()
        .filter(|t| matches\!(t.token_type,
                              TokenType::Plus | TokenType::Minus | TokenType::Star | TokenType::Slash))
        .collect();

    assert_eq!(operators.len(), 4);
    assert_eq!(operators[0].token_type, TokenType::Plus);
    assert_eq!(operators[1].token_type, TokenType::Minus);
    assert_eq!(operators[2].token_type, TokenType::Star);
    assert_eq!(operators[3].token_type, TokenType::Slash);
}

#[test_case("(", TokenType::LeftParen)]
#[test_case(")", TokenType::RightParen)]
#[test_case("[", TokenType::LeftBracket)]
#[test_case("]", TokenType::RightBracket)]
#[test_case("{", TokenType::LeftBrace)]
#[test_case("}", TokenType::RightBrace)]
#[test_case(",", TokenType::Comma)]
#[test_case(".", TokenType::Dot)]
#[test_case(":", TokenType::Colon)]
#[test_case(";", TokenType::Semicolon)]
fn test_delimiters(input: &str, expected: TokenType) {
    let keyword_manager = KeywordManager::new_for_testing("english");
    let mut lexer = Lexer::new(input, &keyword_manager);
    let tokens = lexer.tokenize().unwrap();

    assert_eq!(tokens[0].token_type, expected);
    assert_eq!(tokens[0].lexeme, input);
}

#[test]
fn test_complex_expression() {
    let keyword_manager = KeywordManager::new_for_testing("english");
    let source = "result = (a + b) * c >= d && e \!= f";

    let mut lexer = Lexer::new(source, &keyword_manager);
    let tokens = lexer.tokenize().unwrap();

    // Verify we have all expected token types
    assert!(tokens.iter().any(|t| t.token_type == TokenType::Equal));
    assert!(tokens.iter().any(|t| t.token_type == TokenType::LeftParen));
    assert!(tokens.iter().any(|t| t.token_type == TokenType::Plus));
    assert!(tokens.iter().any(|t| t.token_type == TokenType::RightParen));
    assert!(tokens.iter().any(|t| t.token_type == TokenType::Star));
    assert!(tokens.iter().any(|t| t.token_type == TokenType::GreaterEqual));
    assert!(tokens.iter().any(|t| t.token_type == TokenType::AmpersandAmpersand));
    assert!(tokens.iter().any(|t| t.token_type == TokenType::BangEqual));
}
