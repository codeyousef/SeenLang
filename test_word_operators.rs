// Test file to verify word-based logical operators are working

use seen_lexer::{Lexer, TokenType, KeywordManager};
use std::sync::Arc;

fn main() {
    // Test the fundamental design principle: word-based logical operators
    let keyword_manager = Arc::new(KeywordManager::new());
    
    // Test "and" operator (Stefik & Siebert 2013 research)
    let mut lexer = Lexer::new("age >= 18 and hasPermission".to_string(), keyword_manager.clone());
    
    println!("Testing word-based logical operators (research-based design):");
    
    // Skip to the "and" token
    lexer.next_token().unwrap(); // age
    lexer.next_token().unwrap(); // >=
    lexer.next_token().unwrap(); // 18
    
    let and_token = lexer.next_token().unwrap();
    match and_token.token_type {
        TokenType::LogicalAnd => println!("✅ SUCCESS: 'and' correctly tokenized as LogicalAnd"),
        TokenType::Keyword(_) => println!("❌ FAIL: 'and' incorrectly tokenized as generic Keyword"),
        TokenType::PublicIdentifier(_) => println!("❌ FAIL: 'and' incorrectly tokenized as identifier"),
        other => println!("❌ FAIL: 'and' tokenized as {:?}", other),
    }
    
    // Test "or" operator
    let mut lexer2 = Lexer::new("not valid or expired".to_string(), keyword_manager.clone());
    
    let not_token = lexer2.next_token().unwrap();
    match not_token.token_type {
        TokenType::LogicalNot => println!("✅ SUCCESS: 'not' correctly tokenized as LogicalNot"),
        other => println!("❌ FAIL: 'not' tokenized as {:?}", other),
    }
    
    lexer2.next_token().unwrap(); // valid
    let or_token = lexer2.next_token().unwrap();
    match or_token.token_type {
        TokenType::LogicalOr => println!("✅ SUCCESS: 'or' correctly tokenized as LogicalOr"),
        other => println!("❌ FAIL: 'or' tokenized as {:?}", other),
    }
    
    println!("\nResearch basis: Stefik & Siebert (2013) found word-based operators");
    println!("significantly outperform C-style symbols (&&, ||, !) for novice programmers.");
}