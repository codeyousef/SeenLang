// Test if the lexer properly recognizes 'fun' as a keyword
use seen_lexer::{Lexer, KeywordManager, TokenType};
use std::sync::Arc;

fn main() {
    println!("Testing 'fun' keyword recognition...\n");
    
    let test_code = "fun add(x, y) { x + y }";
    println!("Code: {}\n", test_code);
    
    let keyword_manager = Arc::new(KeywordManager::new());
    let mut lexer = Lexer::new(test_code.to_string(), keyword_manager);
    
    println!("Tokens:");
    loop {
        match lexer.next_token() {
            Ok(token) => {
                println!("  {:?}", token);
                if token.token_type == TokenType::EOF {
                    break;
                }
            }
            Err(e) => {
                println!("  Error: {}", e);
                break;
            }
        }
    }
}