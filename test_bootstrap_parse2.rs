// Test if we can parse simple function definition
use std::fs;
use seen_lexer::{Lexer, KeywordManager};
use seen_parser::Parser;
use std::sync::Arc;

fn main() {
    println!("Testing if bootstrap compiler can parse function definition...\n");
    
    let content = fs::read_to_string("test_first_func.seen").unwrap();
    println!("Source:\n{}\n", content);
    
    let keyword_manager = Arc::new(KeywordManager::new());
    let lexer = Lexer::new(content, keyword_manager);
    let mut parser = Parser::new(lexer);
    
    match parser.parse_program() {
        Ok(program) => {
            println!("✓ Successfully parsed!");
            println!("Program has {} expressions", program.expressions.len());
            for (i, expr) in program.expressions.iter().enumerate() {
                println!("Expression {}: {:?}", i, expr);
            }
        }
        Err(e) => {
            println!("✗ Parse error: {}", e);
        }
    }
}