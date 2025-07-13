//! Helper utilities for parser tests

use crate::{Parser, ParserError};
use crate::ast::*;
use seen_lexer::lexer::Lexer;
use seen_lexer::token::{Token, TokenType, Location};
use seen_lexer::keyword_config::{KeywordConfig, KeywordManager};

pub type ParseResult<T> = Result<T, ParserError>;

/// Create a parser from a token stream
pub fn create_parser(tokens: Vec<Token>) -> Parser {
    Parser::new(tokens)
}

/// Parse a source string and return the AST
pub fn parse_source(source: &str) -> ParseResult<Program> {
    use std::path::PathBuf;
    
    // Get the specifications directory relative to the parser crate
    let lang_files_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent() // Go up from seen_parser crate root to workspace root
        .unwrap()
        .join("specifications");
    
    let keyword_config = KeywordConfig::from_directory(&lang_files_dir)
        .expect("Failed to load keyword configuration for testing");
    
    let keyword_manager = KeywordManager::new(keyword_config, "en".to_string())
        .expect("Failed to create KeywordManager for testing");
        
    let mut lexer = Lexer::new(source, &keyword_manager, "en".to_string());
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    parser.parse()
}

/// Parse an expression from source (Note: Parser doesn't have this method yet)
pub fn parse_expression_from_source(_source: &str) -> ParseResult<Expression> {
    // For now, this is not available until we implement expression parsing
    todo!("Expression parsing not yet implemented in parser")
}

/// Parse a statement from source (Note: Parser doesn't have this method yet)  
pub fn parse_statement_from_source(_source: &str) -> ParseResult<Statement> {
    // For now, this is not available until we implement statement parsing
    todo!("Statement parsing not yet implemented in parser")
}

/// Assert that parsing fails with a specific error
pub fn assert_parse_error(source: &str) {
    let result = parse_source(source);
    assert!(result.is_err(), "Expected parse error for: {}", source);
}

/// Assert successful parse and return AST
pub fn assert_parse_ok(source: &str) -> Program {
    match parse_source(source) {
        Ok(program) => program,
        Err(e) => panic!("Parse failed for '{}': {:?}", source, e),
    }
}

/// Create a simple token for testing
pub fn token(token_type: TokenType, lexeme: &str) -> Token {
    Token {
        token_type,
        lexeme: lexeme.to_string(),
        location: Location::from_positions(1, 1, 1, 1),
        language: "en".to_string(),
    }
}

/// Create an EOF token
pub fn eof() -> Token {
    token(TokenType::EOF, "")
}