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