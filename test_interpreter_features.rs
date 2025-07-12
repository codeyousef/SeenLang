// Test the interpreter with new features

use seen_lexer::{Lexer, KeywordConfig, KeywordManager};
use seen_parser::Parser;
use seen_interpreter::interpret_program;

fn main() {
    let source = r#"
    func main() {
        // Test array literals
        val arr = [1, 2, 3];
        
        // Test array indexing
        val first = arr[0];
        
        // Test for loop with range
        var sum = 0;
        for i in 0..3 {
            sum = sum + arr[i];
        }
        
        println(sum);
    }
    "#;
    
    // Load keywords
    let keyword_config = KeywordConfig::from_directory("specifications").unwrap();
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
        println!("Interpretation successful!");
        if let Some(value) = result.value {
            println!("Result: {:?}", value);
        }
    } else {
        println!("Interpretation failed:");
        for error in &result.errors {
            println!("  - {:?}", error);
        }
    }
}