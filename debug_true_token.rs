use seen_lexer::{Lexer, KeywordManager, TokenType};
use std::sync::Arc;

fn main() {
    let mut keyword_manager = KeywordManager::new();
    keyword_manager.load_from_toml("en").unwrap();
    keyword_manager.switch_language("en").unwrap();
    
    let mut lexer = Lexer::new("true".to_string(), Arc::new(keyword_manager));
    
    let token = lexer.next_token().unwrap();
    println!("Token for 'true': {:?}", token.token_type);
    
    match token.token_type {
        TokenType::Keyword(k) => println!("  It's a keyword: {:?}", k),
        TokenType::PrivateIdentifier(s) => println!("  It's a private identifier: {}", s),
        TokenType::PublicIdentifier(s) => println!("  It's a public identifier: {}", s),
        _ => println!("  It's something else"),
    }
}