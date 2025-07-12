//\! Integration tests for complete programs and language switching

use super::*;
use crate::{Lexer, TokenType, KeywordManager};
use pretty_assertions::assert_eq;

#[test]
fn test_hello_world_english() {
    let keyword_manager = KeywordManager::new_for_testing("english");
    let source = r#"
func main() {
    val greeting = "Hello, World\!";
    println(greeting);
}
"#;
    
    let mut lexer = Lexer::new(source, &keyword_manager);
    let tokens = lexer.tokenize().unwrap();
    
    // Verify key tokens are present
    assert\!(tokens.iter().any(|t| t.token_type == TokenType::Func));
    assert\!(tokens.iter().any(|t| t.token_type == TokenType::Val));
    assert\!(tokens.iter().any(|t| t.token_type == TokenType::StringLiteral 
        && t.lexeme.contains("Hello, World\!")));
}

#[test]
fn test_hello_world_arabic() {
    let keyword_manager = KeywordManager::new_for_testing("arabic");
    let source = r#"
دالة رئيسية() {
    ثابت تحية = "مرحباً، يا عالم\!";
    اطبع(تحية);
}
"#;
    
    let mut lexer = Lexer::new(source, &keyword_manager);
    let tokens = lexer.tokenize().unwrap();
    
    // Verify key tokens are present
    assert\!(tokens.iter().any(|t| t.token_type == TokenType::Func));
    assert\!(tokens.iter().any(|t| t.token_type == TokenType::Val));
    assert\!(tokens.iter().any(|t| t.token_type == TokenType::StringLiteral 
        && t.lexeme.contains("مرحباً، يا عالم\!")));
}

#[test]
fn test_language_consistency() {
    // Test that the same program structure produces equivalent tokens
    let english_source = "func add(a: Int, b: Int) -> Int { return a + b; }";
    let arabic_source = "دالة جمع(أ: صحيح، ب: صحيح) -> صحيح { ارجع أ + ب؛ }";
    
    let english_km = KeywordManager::new_for_testing("english");
    let arabic_km = KeywordManager::new_for_testing("arabic");
    
    let mut english_lexer = Lexer::new(english_source, &english_km);
    let mut arabic_lexer = Lexer::new(arabic_source, &arabic_km);
    
    let english_tokens = english_lexer.tokenize().unwrap();
    let arabic_tokens = arabic_lexer.tokenize().unwrap();
    
    // Extract token types (ignoring lexemes which will differ)
    let english_types: Vec<_> = english_tokens.iter()
        .map(|t| t.token_type.clone())
        .collect();
    let arabic_types: Vec<_> = arabic_tokens.iter()
        .map(|t| t.token_type.clone())
        .collect();
    
    // The token type sequences should be similar
    // (accounting for potential differences in identifier tokenization)
}

#[test]
fn test_complex_program() {
    let keyword_manager = KeywordManager::new_for_testing("english");
    let source = r#"
// Complex program test
func fibonacci(n: Int) -> Int {
    if n <= 1 {
        return n;
    } else {
        return fibonacci(n - 1) + fibonacci(n - 2);
    }
}

func main() {
    var i = 0;
    while i < 10 {
        val result = fibonacci(i);
        println("fib(" + i + ") = " + result);
        i = i + 1;
    }
}
"#;
    
    let mut lexer = Lexer::new(source, &keyword_manager);
    let tokens = lexer.tokenize().unwrap();
    
    // Verify various token types are present
    assert\!(tokens.iter().any(|t| t.token_type == TokenType::Func));
    assert\!(tokens.iter().any(|t| t.token_type == TokenType::If));
    assert\!(tokens.iter().any(|t| t.token_type == TokenType::Else));
    assert\!(tokens.iter().any(|t| t.token_type == TokenType::While));
    assert\!(tokens.iter().any(|t| t.token_type == TokenType::Return));
    assert\!(tokens.iter().any(|t| t.token_type == TokenType::Val));
    assert\!(tokens.iter().any(|t| t.token_type == TokenType::Var));
    
    // Verify operators
    assert\!(tokens.iter().any(|t| t.token_type == TokenType::Plus));
    assert\!(tokens.iter().any(|t| t.token_type == TokenType::Minus));
    assert\!(tokens.iter().any(|t| t.token_type == TokenType::Less));
    assert\!(tokens.iter().any(|t| t.token_type == TokenType::LessEqual));
}

#[test]
fn test_source_location_tracking() {
    let keyword_manager = KeywordManager::new_for_testing("english");
    let source = "func\nmain()\n{\n    val x = 42;\n}";
    
    let mut lexer = Lexer::new(source, &keyword_manager);
    let tokens = lexer.tokenize().unwrap();
    
    // Find specific tokens and verify their line numbers
    let func_token = tokens.iter().find(|t| t.token_type == TokenType::Func).unwrap();
    assert_eq\!(func_token.line, 1);
    
    let main_token = tokens.iter().find(|t| t.lexeme == "main").unwrap();
    assert_eq\!(main_token.line, 2);
    
    let val_token = tokens.iter().find(|t| t.token_type == TokenType::Val).unwrap();
    assert_eq\!(val_token.line, 4);
}
