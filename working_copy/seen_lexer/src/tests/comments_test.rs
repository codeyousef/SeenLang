//\! Tests for comment handling

use super::*;
use crate::{Lexer, TokenType, KeywordManager};
use pretty_assertions::assert_eq;

#[test]
fn test_single_line_comment_english() {
    let keyword_manager = KeywordManager::new_for_testing("english");
    let source = "// This is a comment\nfunc main() {}";
    
    let mut lexer = Lexer::new(source, &keyword_manager);
    let tokens = lexer.tokenize().unwrap();
    
    // Comment should be skipped
    assert_eq\!(tokens[0].token_type, TokenType::Func);
    assert_eq\!(tokens[0].line, 2); // Should be on line 2
}

#[test]
fn test_single_line_comment_arabic() {
    let keyword_manager = KeywordManager::new_for_testing("arabic");
    let source = "## هذا تعليق\nدالة رئيسية() {}";
    
    let mut lexer = Lexer::new(source, &keyword_manager);
    let tokens = lexer.tokenize().unwrap();
    
    // Comment should be skipped
    assert_eq\!(tokens[0].token_type, TokenType::Func);
    assert_eq\!(tokens[0].line, 2); // Should be on line 2
}

#[test]
fn test_multi_line_comment() {
    let keyword_manager = KeywordManager::new_for_testing("english");
    let source = "/* This is a\nmulti-line\ncomment */\nfunc main() {}";
    
    let mut lexer = Lexer::new(source, &keyword_manager);
    let tokens = lexer.tokenize().unwrap();
    
    // Comment should be skipped
    assert_eq\!(tokens[0].token_type, TokenType::Func);
    assert_eq\!(tokens[0].line, 4); // Should be on line 4
}

#[test]
fn test_nested_comments() {
    let keyword_manager = KeywordManager::new_for_testing("english");
    let source = "/* outer /* inner */ still in outer */ func";
    
    let mut lexer = Lexer::new(source, &keyword_manager);
    let tokens = lexer.tokenize().unwrap();
    
    // All comments should be skipped
    assert_eq\!(tokens[0].token_type, TokenType::Func);
}

#[test]
fn test_unterminated_comment_error() {
    let keyword_manager = KeywordManager::new_for_testing("english");
    let source = "/* This comment never ends";
    
    let mut lexer = Lexer::new(source, &keyword_manager);
    let result = lexer.tokenize();
    
    assert\!(result.is_err(), "Expected error for unterminated comment");
}

#[test]
fn test_comment_at_end_of_line() {
    let keyword_manager = KeywordManager::new_for_testing("english");
    let source = "func main() {} // End of line comment";
    
    let mut lexer = Lexer::new(source, &keyword_manager);
    let tokens = lexer.tokenize().unwrap();
    
    // Should not include comment
    let non_eof_tokens: Vec<_> = tokens.iter()
        .filter(|t| t.token_type \!= TokenType::Eof)
        .collect();
    
    assert_eq\!(non_eof_tokens.last().unwrap().token_type, TokenType::RightBrace);
}

#[test]
fn test_mixed_comment_styles() {
    let keyword_manager = KeywordManager::new_for_testing("english");
    let source = r#"
    // Single line comment
    func main() {
        /* Multi-line comment */
        val x = 42; // Another single line
        /* Another
           multi-line
           comment */
    }
    "#;
    
    let mut lexer = Lexer::new(source, &keyword_manager);
    let tokens = lexer.tokenize().unwrap();
    
    // All comments should be skipped, verify structure
    assert\!(tokens.iter().any(|t| t.token_type == TokenType::Func));
    assert\!(tokens.iter().any(|t| t.token_type == TokenType::Val));
    assert\!(tokens.iter().any(|t| t.token_type == TokenType::IntegerLiteral && t.lexeme == "42"));
}
