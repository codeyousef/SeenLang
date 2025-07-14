// Test complete Hello World program with the interpreter

use seen_interpreter::interpret_program;
use seen_lexer::{KeywordConfig, KeywordManager, Lexer};
use seen_parser::Parser;
use std::path::PathBuf;

fn main() {
    let source = r#"
    func main() {
        println("Hello, World!");
        val greeting = "Welcome to Seen!";
        println(greeting);
        
        // Test basic arithmetic
        val x = 10 + 5;
        val y = x * 2;
        println("Result:");
        println(y);
        
        // Test arrays
        val numbers = [1, 2, 3];
        println("First number:");
        println(numbers[0]);
        
        // Test for loops
        println("Counting:");
        for i in 0..3 {
            println(i);
        }
    }
    "#;

    // Load keywords
    let specs_dir = PathBuf::from("specifications");
    let keyword_config = KeywordConfig::from_directory(&specs_dir).unwrap();
    let keyword_manager = KeywordManager::new(keyword_config, "en".to_string()).unwrap();

    // Lex
    let mut lexer = Lexer::new(source, &keyword_manager, "en".to_string());
    let tokens = lexer.tokenize().unwrap();

    // Parse
    let mut parser = Parser::new(tokens);
    let program = parser.parse().unwrap();

    // Interpret
    let result = interpret_program(&program);

    if result.is_ok() {
        println!("\n=== Hello World program executed successfully! ===");
        if let Some(value) = result.value {
            println!("Final result: {:?}", value);
        }
    } else {
        println!("\n=== Program execution failed ===");
        for error in &result.errors {
            println!("Error: {}", error);
        }
    }
}