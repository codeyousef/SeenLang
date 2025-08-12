// Simple test to verify LSP can analyze a Seen file
use std::fs;
use seen_lexer::{Lexer, KeywordManager};
use seen_parser::Parser;
use seen_typechecker::TypeChecker;
use seen_memory_manager::MemoryManager;
use std::sync::Arc;

fn main() {
    let content = fs::read_to_string("test_lsp_simple.seen").unwrap();
    println!("Testing LSP analysis on test_lsp_simple.seen:\n");
    
    // Lexical analysis
    let keyword_manager = Arc::new(KeywordManager::new());
    let mut lexer = Lexer::new(content.clone(), keyword_manager);
    
    // Parse the tokens
    let mut parser = Parser::new(lexer);
    match parser.parse_program() {
        Ok(program) => {
            println!("✓ Parsing succeeded");
            
            // Type check the program
            let mut type_checker = TypeChecker::new();
            let type_result = type_checker.check_program(&program);
            
            if type_result.get_errors().is_empty() {
                println!("✓ Type checking succeeded");
            } else {
                println!("✗ Type errors found:");
                for error in type_result.get_errors() {
                    println!("  - {}", error);
                }
            }
            
            // Memory safety analysis
            let mut memory_manager = MemoryManager::new();
            let memory_result = memory_manager.analyze_program(&program);
            
            if memory_result.get_errors().is_empty() {
                println!("✓ Memory analysis succeeded");
            } else {
                println!("✗ Memory safety issues:");
                for error in memory_result.get_errors() {
                    println!("  - {}", error);
                }
            }
        }
        Err(e) => {
            println!("✗ Parse error: {}", e);
        }
    }
}