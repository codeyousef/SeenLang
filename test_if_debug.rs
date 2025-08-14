use seen_parser::Parser;
use seen_lexer::{Lexer, KeywordManager, TokenType};
use std::sync::Arc;

fn main() {
    let input = "if condition { doSomething() }";
    
    let mut keyword_manager = KeywordManager::new();
    keyword_manager.load_from_toml("en").unwrap();
    keyword_manager.switch_language("en").unwrap();
    
    let mut lexer = Lexer::new(input.to_string(), Arc::new(keyword_manager));
    
    // Print tokens
    println!("Tokens:");
    loop {
        match lexer.next_token() {
            Ok(token) => {
                println!("  {:?}", token);
                if token.token_type == seen_lexer::TokenType::EOF {
                    break;
                }
            }
            Err(e) => {
                println!("  Error: {:?}", e);
                break;
            }
        }
    }
    
    // Now parse
    let mut keyword_manager = KeywordManager::new();
    keyword_manager.load_from_toml("en").unwrap();
    keyword_manager.switch_language("en").unwrap();
    let lexer = Lexer::new(input.to_string(), Arc::new(keyword_manager));
    let mut parser = Parser::new(lexer);
    
    match parser.parse_expression() {
        Ok(expr) => println!("Parsed: {:?}", expr),
        Err(e) => println!("Parse error: {:?}", e),
    }
}