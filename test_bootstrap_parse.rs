// Test if we can parse the Seen compiler source files
use std::fs;
use seen_lexer::{Lexer, KeywordManager};
use seen_parser::Parser;
use std::sync::Arc;

fn main() {
    println!("Testing if bootstrap compiler can parse Seen compiler source...\n");
    
    // Test parsing a simple Seen compiler source file
    let test_files = vec![
        "test_bootstrap_simple_only.seen",
        "test_multi_statement.seen",
        "test_class.seen",
        "compiler_seen/simple_test.seen",
        "compiler_seen/src/lexer/token.seen",
    ];
    
    let keyword_manager = Arc::new(KeywordManager::new());
    
    for file_path in test_files {
        println!("Parsing {}...", file_path);
        
        match fs::read_to_string(file_path) {
            Ok(content) => {
                let lexer = Lexer::new(content, keyword_manager.clone());
                let mut parser = Parser::new(lexer);
                
                match parser.parse_program() {
                    Ok(_program) => {
                        println!("  ✓ Successfully parsed!");
                    }
                    Err(e) => {
                        println!("  ✗ Parse error: {}", e);
                    }
                }
            }
            Err(e) => {
                println!("  ✗ Could not read file: {}", e);
            }
        }
    }
}