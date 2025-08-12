use std::sync::Arc;
use seen_lexer::{Lexer, KeywordManager};
use seen_parser::Parser;

fn main() {
    println!("Testing match with guard parsing:");
    
    let mut keyword_manager = KeywordManager::new();
    keyword_manager.load_from_toml("en").unwrap();
    keyword_manager.switch_language("en").unwrap();
    
    // Test just the pattern part that's failing
    let input = r#"
        match response {
            Ok(data) if data.length > 0 -> processData(data)
            Ok(_) -> "empty"
            Err(e) -> handleError(e)  
        }
    "#;
    println!("Input: {}", input);
    
    let lexer = Lexer::new(input.to_string(), Arc::new(keyword_manager));
    let mut parser = Parser::new(lexer);
    
    match parser.parse_expression() {
        Ok(expr) => println!("✓ Parsed successfully: {:?}", expr),
        Err(e) => println!("✗ Parse error: {:?}", e),
    }
}