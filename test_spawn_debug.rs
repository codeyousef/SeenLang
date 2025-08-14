use seen_parser::Parser;
use seen_lexer::{Lexer, KeywordManager, TokenType};
use std::sync::Arc;

fn main() {
    let input = "spawn { FetchUser(123) }";
    
    let mut keyword_manager = KeywordManager::new();
    keyword_manager.load_from_toml("en").unwrap();
    keyword_manager.switch_language("en").unwrap();
    
    println!("Testing: {}", input);
    
    let lexer = Lexer::new(input.to_string(), Arc::new(keyword_manager));
    let mut parser = Parser::new(lexer);
    
    match parser.parse_expression() {
        Ok(expr) => println!("Parsed: {:?}", expr),
        Err(e) => println!("Parse error: {:?}", e),
    }
}