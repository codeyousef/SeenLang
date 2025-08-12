use seen_lexer::{Lexer, KeywordManager};
use seen_parser::Parser;
use std::sync::Arc;

fn main() {
    let mut keyword_manager = KeywordManager::new();
    keyword_manager.load_from_toml("en").unwrap();
    keyword_manager.switch_language("en").unwrap();
    
    let input = "{ 42 }";
    println!("Parsing: {}", input);
    
    let lexer = Lexer::new(input.to_string(), Arc::new(keyword_manager));
    let mut parser = Parser::new(lexer);
    
    match parser.parse_expression() {
        Ok(expr) => {
            // Print the variant name
            println!("Got expression variant: {:?}", expr);
        }
        Err(e) => {
            println!("Parse error: {:?}", e);
        }
    }
}