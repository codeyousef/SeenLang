// Debug exact parse error
use std::fs;
use seen_lexer::{Lexer, KeywordManager};
use seen_parser::Parser;
use std::sync::Arc;

fn main() {
    let content = fs::read_to_string("compiler_seen/simple_test.seen").unwrap();
    
    // Show exactly what's at position 169
    println!("Character at position 169: {:?}", content.chars().nth(169));
    println!("Characters 160-180: {:?}", &content[160.min(content.len())..180.min(content.len())]);
    
    // Get surrounding context
    let lines: Vec<&str> = content.lines().collect();
    println!("\nLine 5: {:?}", lines.get(4));
    println!("Line 6: {:?}", lines.get(5));
    
    // Try parsing
    let keyword_manager = Arc::new(KeywordManager::new());
    let lexer = Lexer::new(content, keyword_manager);
    let mut parser = Parser::new(lexer);
    
    match parser.parse_program() {
        Ok(_) => println!("\nParsing succeeded!"),
        Err(e) => {
            println!("\nParse error: {}", e);
            println!("Error details: {:?}", e);
        }
    }
}