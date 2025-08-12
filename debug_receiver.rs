use std::sync::Arc;
use seen_lexer::{Lexer, KeywordManager};
use seen_parser::Parser;

fn main() {
    println!("Testing receiver syntax parsing:");
    
    let mut keyword_manager = KeywordManager::new();
    keyword_manager.load_from_toml("en").unwrap();
    keyword_manager.switch_language("en").unwrap();
    let lexer = Lexer::new(r#"
        fun (p: Person) getName(): String {
            return p.name
        }
    "#.to_string(), Arc::new(keyword_manager));
    let mut parser = Parser::new(lexer);
    
    match parser.parse_expression() {
        Ok(expr) => println!("✓ Parsed successfully: {:?}", expr),
        Err(e) => println!("✗ Parse error: {:?}", e),
    }
}