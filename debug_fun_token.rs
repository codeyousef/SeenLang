use std::sync::Arc;
use seen_lexer::{Lexer, KeywordManager};

fn main() {
    println!("Testing lexer with 'fun' keyword:");
    
    let mut keyword_manager = KeywordManager::new();
    match keyword_manager.load_from_toml("en") {
        Ok(_) => println!("✓ Keywords loaded successfully"),
        Err(e) => {
            println!("✗ Failed to load keywords: {:?}", e);
            return;
        }
    }
    
    match keyword_manager.switch_language("en") {
        Ok(_) => println!("✓ Language switched to English"),
        Err(e) => {
            println!("✗ Failed to switch language: {:?}", e);
            return;
        }
    }
    
    let mut lexer = Lexer::new("fun greet".to_string(), Arc::new(keyword_manager));
    
    println!("\nTokens:");
    loop {
        match lexer.next_token() {
            Ok(token) => {
                println!("Token: {:?}", token);
                if matches!(token.token_type, seen_lexer::TokenType::EOF) {
                    break;
                }
            }
            Err(e) => {
                println!("Lexer error: {:?}", e);
                break;
            }
        }
    }
}