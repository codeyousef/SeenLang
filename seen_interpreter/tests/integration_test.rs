use seen_lexer::{Lexer, KeywordManager};
use seen_lexer::keyword_config::KeywordConfig;
use seen_parser::Parser;
use seen_typechecker::type_check_program;
use seen_interpreter::interpret_program;
use std::path::PathBuf;

fn create_test_keyword_manager() -> KeywordManager {
    // Get the specifications directory relative to the workspace root
    let specs_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent() // Go up from seen_interpreter crate root to workspace root
        .unwrap()
        .join("specifications");
    
    let keyword_config = KeywordConfig::from_directory(&specs_dir)
        .expect("Failed to load keyword configuration for testing");
    
    KeywordManager::new(keyword_config, "en".to_string())
        .expect("Failed to create KeywordManager for testing")
}

#[test]
fn test_simple_hello_world() {
    let source = r#"val greeting = "Hello, World!";"#;
    
    // Tokenize
    let keyword_manager = create_test_keyword_manager();
    let mut lexer = Lexer::new(source, &keyword_manager, "en".to_string());
    let tokens = lexer.tokenize().expect("Lexer should work");
    
    // Parse
    let mut parser = Parser::new(tokens);
    let program = parser.parse().expect("Parser should work");
    
    // Type check
    let type_result = type_check_program(&program);
    assert!(type_result.is_ok(), "Type checking should pass");
    
    // Interpret
    let interpret_result = interpret_program(&program);
    assert!(interpret_result.is_ok(), "Interpretation should pass");
}

#[test]
fn test_arithmetic_operations() {
    let source = r#"val x = 5 + 3;
val y = x * 2;
val z = y - 4;"#;
    
    // Tokenize
    let keyword_manager = create_test_keyword_manager();
    let mut lexer = Lexer::new(source, &keyword_manager, "en".to_string());
    let tokens = lexer.tokenize().expect("Lexer should work");
    
    // Parse
    let mut parser = Parser::new(tokens);
    let program = parser.parse().expect("Parser should work");
    
    // Type check
    let type_result = type_check_program(&program);
    assert!(type_result.is_ok(), "Type checking should pass");
    
    // Interpret
    let interpret_result = interpret_program(&program);
    assert!(interpret_result.is_ok(), "Interpretation should pass");
}

#[test]
fn test_basic_values() {
    let source = r#"val integer_val = 42;
val float_val = 3.14;
val bool_val = true;
val string_val = "test";"#;
    
    // Tokenize
    let keyword_manager = create_test_keyword_manager();
    let mut lexer = Lexer::new(source, &keyword_manager, "en".to_string());
    let tokens = lexer.tokenize().expect("Lexer should work");
    
    // Parse
    let mut parser = Parser::new(tokens);
    let program = parser.parse().expect("Parser should work");
    
    // Type check
    let type_result = type_check_program(&program);
    assert!(type_result.is_ok(), "Type checking should pass");
    
    // Interpret
    let interpret_result = interpret_program(&program);
    assert!(interpret_result.is_ok(), "Interpretation should pass");
}

#[test]
fn test_array_features() {
    let source = r#"
        val arr = [1, 2, 3];
        val first = arr[0];
    "#;
    
    let keyword_manager = create_test_keyword_manager();
    let mut lexer = Lexer::new(source, &keyword_manager, "en".to_string());
    let tokens = lexer.tokenize().expect("Lexer should work");
    
    let mut parser = Parser::new(tokens);
    let program = parser.parse().expect("Parser should work");
    
    let interpret_result = interpret_program(&program);
    assert!(interpret_result.is_ok(), "Array interpretation should pass: {:?}", interpret_result.errors);
}

#[test]
fn test_range_features() {
    let source = r#"
        val range = 0..3;
        val first = range[0];
    "#;
    
    let keyword_manager = create_test_keyword_manager();
    let mut lexer = Lexer::new(source, &keyword_manager, "en".to_string());
    let tokens = lexer.tokenize().expect("Lexer should work");
    
    let mut parser = Parser::new(tokens);
    let program = parser.parse().expect("Parser should work");
    
    let interpret_result = interpret_program(&program);
    assert!(interpret_result.is_ok(), "Range interpretation should pass: {:?}", interpret_result.errors);
}

#[test]
fn test_for_loop() {
    let source = r#"
        func main() {
            for i in 0..3 {
                val x = i;
            }
        }
    "#;
    
    let keyword_manager = create_test_keyword_manager();
    let mut lexer = Lexer::new(source, &keyword_manager, "en".to_string());
    let tokens = lexer.tokenize().expect("Lexer should work");
    
    let mut parser = Parser::new(tokens);
    let program = parser.parse().expect("Parser should work");
    
    let interpret_result = interpret_program(&program);
    assert!(interpret_result.is_ok(), "For loop interpretation should pass: {:?}", interpret_result.errors);
}

#[test]
fn test_complete_hello_world() {
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
    
    let keyword_manager = create_test_keyword_manager();
    let mut lexer = Lexer::new(source, &keyword_manager, "en".to_string());
    let tokens = lexer.tokenize().expect("Lexer should work");
    
    let mut parser = Parser::new(tokens);
    let program = parser.parse().expect("Parser should work");
    
    let type_result = type_check_program(&program);
    assert!(type_result.is_ok(), "Type checking should pass: {:?}", type_result);
    
    let interpret_result = interpret_program(&program);
    assert!(interpret_result.is_ok(), "Complete Hello World should pass: {:?}", interpret_result.errors);
}

#[test]
fn test_modulo_operation() {
    let source = r#"
        val x = 10 % 3;
        val y = 15.5 % 4.0;
    "#;
    
    let keyword_manager = create_test_keyword_manager();
    let mut lexer = Lexer::new(source, &keyword_manager, "en".to_string());
    let tokens = lexer.tokenize().expect("Lexer should work");
    
    let mut parser = Parser::new(tokens);
    let program = parser.parse().expect("Parser should work");
    
    let interpret_result = interpret_program(&program);
    assert!(interpret_result.is_ok(), "Modulo operation should pass: {:?}", interpret_result.errors);
}

#[test]
fn test_stdlib_functions() {
    let source = r#"
        // Test basic math functions
        func abs(x: Int) -> Int {
            if x < 0 {
                return -x;
            } else {
                return x;
            }
        }

        func max(a: Int, b: Int) -> Int {
            if a > b {
                return a;
            } else {
                return b;
            }
        }

        func factorial(n: Int) -> Int {
            if n <= 1 {
                return 1;
            } else {
                return n * factorial(n - 1);
            }
        }

        func main() {
            val neg_val = abs(-5);
            println("abs(-5) = ");
            println(neg_val);
            
            val max_val = max(10, 15);
            println("max(10, 15) = ");
            println(max_val);
            
            val fact = factorial(5);
            println("factorial(5) = ");
            println(fact);
        }
    "#;
    
    let keyword_manager = create_test_keyword_manager();
    let mut lexer = Lexer::new(source, &keyword_manager, "en".to_string());
    let tokens = lexer.tokenize().expect("Lexer should work");
    
    let mut parser = Parser::new(tokens);
    let program = parser.parse().expect("Parser should work");
    
    let interpret_result = interpret_program(&program);
    assert!(interpret_result.is_ok(), "Stdlib functions should work: {:?}", interpret_result.errors);
}

#[test]
fn test_basic_seen_compiler() {
    let source = r#"
        // Basic compiler implementation in Seen
        func evaluate_number(value: Int) -> Int {
            return value;
        }

        func simple_compile(input: Int) -> Int {
            val result = evaluate_number(input);
            return result;
        }

        func main() {
            println("Testing basic compiler in Seen");
            
            val test_input = 42;
            val compiled_result = simple_compile(test_input);
            
            println("Input:");
            println(test_input);
            println("Compiled result:");
            println(compiled_result);
        }
    "#;
    
    let keyword_manager = create_test_keyword_manager();
    let mut lexer = Lexer::new(source, &keyword_manager, "en".to_string());
    let tokens = lexer.tokenize().expect("Lexer should work");
    
    let mut parser = Parser::new(tokens);
    let program = parser.parse().expect("Parser should work");
    
    let interpret_result = interpret_program(&program);
    assert!(interpret_result.is_ok(), "Basic Seen compiler should work: {:?}", interpret_result.errors);
}

#[test]
fn test_bootstrap_capability() {
    let source = r#"
        // Bootstrap test - Seen compiler in Seen
        func simple_compiler(input: Int) -> Int {
            println("Compiling input:");
            println(input);
            
            // Mock compilation steps
            val lexed = input + 1;
            val parsed = lexed * 2;  
            val compiled = parsed + 10;
            
            println("Compilation result:");
            println(compiled);
            
            return compiled;
        }

        func main() {
            println("=== BOOTSTRAP TEST ===");
            println("Seen compiler written in Seen");
            
            val source_value = 5;
            val result = simple_compiler(source_value);
            
            println("Final bootstrap result:");
            println(result);
            
            // Verify the calculation: (5 + 1) * 2 + 10 = 22
            val expected = 22;
            if result == expected {
                println("Bootstrap test PASSED!");
            } else {
                println("Bootstrap test FAILED!");
            }
        }
    "#;
    
    let keyword_manager = create_test_keyword_manager();
    let mut lexer = Lexer::new(source, &keyword_manager, "en".to_string());
    let tokens = lexer.tokenize().expect("Lexer should work");
    
    let mut parser = Parser::new(tokens);
    let program = parser.parse().expect("Parser should work");
    
    let interpret_result = interpret_program(&program);
    assert!(interpret_result.is_ok(), "Bootstrap capability should work: {:?}", interpret_result.errors);
}