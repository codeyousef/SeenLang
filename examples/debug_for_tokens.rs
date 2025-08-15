use seen_lexer::{Lexer, KeywordManager, token::TokenType};
use std::sync::Arc;

fn main() {
    let mut keyword_manager = KeywordManager::new();
    keyword_manager.load_from_toml("en").unwrap();
    keyword_manager.switch_language("en").unwrap();
    let keyword_manager = Arc::new(keyword_manager);
    
    let input = "for item in items { item }";
    println!("Input: '{}'", input);
    println!("Length: {}", input.len());
    
    let mut lexer = Lexer::new(input.to_string(), keyword_manager);
    
    println!("\nTokens:");
    loop {
        let token = lexer.next_token();
        println!("  {:?}", token);
        
        if matches!(token.token_type, TokenType::EOF) {
            break;
        }
    }
}