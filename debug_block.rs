use seen_lexer::{Lexer, KeywordManager, TokenType};
use seen_parser::{Parser, Expression};
use std::sync::Arc;

fn main() {
    let mut keyword_manager = KeywordManager::new();
    keyword_manager.load_from_toml("en").unwrap();
    keyword_manager.switch_language("en").unwrap();
    
    let input = "{ let x = 10; x + 5 }";
    let lexer = Lexer::new(input.to_string(), Arc::new(keyword_manager));
    let mut parser = Parser::new(lexer);
    
    match parser.parse_expression() {
        Ok(expr) => {
            match expr {
                Expression::Block { expressions, .. } => {
                    println!("Got block with {} expressions", expressions.len());
                }
                _ => {
                    println!("Got expression type: {:?}", std::mem::discriminant(&expr));
                }
            }
        }
        Err(e) => {
            println!("Parse error: {:?}", e);
        }
    }
}