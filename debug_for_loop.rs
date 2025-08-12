use seen_lexer::{Lexer, KeywordManager, TokenType};
use std::sync::Arc;

fn main() {
    let mut keyword_manager = KeywordManager::new();
    keyword_manager.load_from_toml("en").unwrap();
    keyword_manager.switch_language("en").unwrap();
    
    let input = "for item in items { process(item) }";
    let mut lexer = Lexer::new(input.to_string(), Arc::new(keyword_manager));
    
    println!("Tokenizing: {}", input);
    println!();
    
    let mut tokens = Vec::new();
    let mut position = 0;
    loop {
        let token = lexer.next_token().unwrap();
        if token.token_type == TokenType::Eof {
            break;
        }
        println!("Position {:3}: {:?}", position, token.token_type);
        if position == 20 {
            println!("  ^ This is where the parser expects a colon but found: {:?}", token.token_type);
        }
        position += token.lexeme.len();
        tokens.push(token);
    }
}