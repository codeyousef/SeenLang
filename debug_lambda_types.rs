use std::sync::Arc;
use seen_lexer::{Lexer, KeywordManager};
use seen_parser::Parser;

fn main() {
    println!("Testing lambda with types parsing:");
    
    let mut keyword_manager = KeywordManager::new();
    keyword_manager.load_from_toml("en").unwrap();
    keyword_manager.switch_language("en").unwrap();
    
    // Test the exact failing case
    let input = "{ (x: Int, y: Int) -> Int in x + y }";
    println!("Input: {}", input);
    
    // Now try parsing
    let lexer = Lexer::new(input.to_string(), Arc::new(keyword_manager));
    let mut parser = Parser::new(lexer);
    
    match parser.parse_expression() {
        Ok(expr) => println!("✓ Parsed successfully: {:?}", expr),
        Err(e) => println!("✗ Parse error: {:?}", e),
    }
}