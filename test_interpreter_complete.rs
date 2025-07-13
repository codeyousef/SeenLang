// Test the complete interpreter with all new features

use seen_interpreter::interpret_program;
use seen_lexer::{KeywordConfig, KeywordManager, Lexer};
use seen_parser::Parser;

fn main() {
    let source = r#"
    func main() {
        println("Testing interpreter features");
        
        // Test array literals
        val arr = [1, 2, 3, 4, 5];
        println("Array created");
        
        // Test array indexing
        val first = arr[0];
        val last = arr[4];
        println("First element:");
        println(first);
        println("Last element:");
        println(last);
        
        // Test for loop with range
        println("Numbers 0 to 2:");
        for i in 0..3 {
            println(i);
        }
        
        // Test for loop with array
        println("Array elements:");
        for element in arr {
            println(element);
        }
        
        println("All tests completed!");
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
        println!("\n=== Interpretation successful! ===");
        if let Some(value) = result.value {
            println!("Final result: {:?}", value);
        }
    } else {
        println!("\n=== Interpretation failed ===");
        for error in &result.errors {
            println!("Error: {}", error);
        }
    }
}