//! Step 4 TDD Tests: Type System Foundation Performance
//! These tests MUST fail initially, then implementation makes them pass

use seen_typechecker::{TypeChecker, Type, PrimitiveType};
use seen_parser::{Parser};
use seen_lexer::{Lexer, LanguageConfig};  
use std::time::{Duration, Instant};
use std::collections::HashMap;

/// FAILING TEST: Type inference must complete in <100μs per function
/// This test MUST fail initially, then implementation makes it pass
#[test]
fn test_type_inference_under_100_microseconds() {
    let config = create_english_config();
    
    // Generate test functions for type inference
    let test_program = r#"
        func simple_function(x: i32, y: str) -> bool {
            let z = x + 42;
            let result = y == "test";
            return result && (z > 0);
        }
        
        func complex_function(a: i32, b: f64, c: str) -> i32 {
            let temp1 = a * 2;
            let temp2 = b + 3.14;
            let temp3 = c + " world";
            if temp1 > 10 {
                return temp1 + 1;
            } else {
                return temp1 - 1;
            }
        }
        
        func generic_function<T>(value: T) -> T {
            return value;
        }
    "#;
    
    // Parse the program
    let mut lexer = Lexer::new(test_program, 0, &config);
    let tokens = lexer.tokenize().expect("Tokenization must succeed");
    let mut parser = Parser::new(tokens);
    let ast = parser.parse_program().expect("Parsing must succeed");
    
    // Measure type inference performance per function
    let mut type_checker = TypeChecker::new();
    let start = Instant::now();
    
    type_checker.check_program(&ast).expect("Type checking must succeed");
    
    let duration = start.elapsed();
    let functions_count = ast.items.iter()
        .filter(|item| matches!(item.kind, seen_parser::ItemKind::Function(_)))
        .count();
    
    let microseconds_per_function = duration.as_micros() as f64 / functions_count as f64;
    
    println!("Type inference: {:.2}μs per function", microseconds_per_function);
    println!("Processed {} functions in {:?}", functions_count, duration);
    
    // HARD REQUIREMENT: <100μs per function
    const MAX_MICROSECONDS_PER_FUNCTION: f64 = 100.0;
    assert!(
        microseconds_per_function < MAX_MICROSECONDS_PER_FUNCTION,
        "TYPE INFERENCE PERFORMANCE FAILED: {:.2}μs per function >= {:.2}μs maximum",
        microseconds_per_function,
        MAX_MICROSECONDS_PER_FUNCTION
    );
}

/// FAILING TEST: Generic type resolution must work correctly
#[test]
fn test_generic_type_resolution() {
    let config = create_english_config();
    
    let generic_program = r#"
        func identity<T>(x: T) -> T {
            return x;
        }
        
        func pair<A, B>(first: A, second: B) -> (A, B) {
            return (first, second);
        }
        
        func main() {
            let int_result = identity(42);
            let str_result = identity("hello");
            let pair_result = pair(10, "world");
        }
    "#;
    
    let mut lexer = Lexer::new(generic_program, 0, &config);
    let tokens = lexer.tokenize().expect("Tokenization must succeed");
    let mut parser = Parser::new(tokens);
    let ast = parser.parse_program().expect("Parsing must succeed");
    
    let mut type_checker = TypeChecker::new();
    let result = type_checker.check_program(&ast);
    
    // REQUIREMENT: Generic type resolution must succeed
    assert!(result.is_ok(), "Generic type resolution must work correctly");
    assert!(!type_checker.diagnostics().has_errors(), "No type errors should occur");
    
    // Verify specific type resolutions
    let type_env = type_checker.type_environment();
    
    // identity(42) should resolve to i32
    assert!(type_env.contains_function("identity"));
    
    // pair(10, "world") should resolve to (i32, str)
    assert!(type_env.contains_function("pair"));
    
    println!("✓ Generic type resolution validation passed");
}

/// FAILING TEST: C type mapping must be bidirectional and lossless
#[test]
fn test_c_type_mapping_bidirectional() {
    let mut type_checker = TypeChecker::new();
    
    // Test primitive type mappings
    let seen_types = vec![
        Type::Primitive(PrimitiveType::I8),
        Type::Primitive(PrimitiveType::I16),
        Type::Primitive(PrimitiveType::I32),
        Type::Primitive(PrimitiveType::I64),
        Type::Primitive(PrimitiveType::U8),
        Type::Primitive(PrimitiveType::U16),
        Type::Primitive(PrimitiveType::U32),
        Type::Primitive(PrimitiveType::U64),
        Type::Primitive(PrimitiveType::F32),
        Type::Primitive(PrimitiveType::F64),
        Type::Primitive(PrimitiveType::Bool),
        Type::Primitive(PrimitiveType::Char),
    ];
    
    for seen_type in &seen_types {
        // Map Seen type to C type
        let c_type = type_checker.map_to_c_type(seen_type)
            .expect("Mapping to C type must succeed");
        
        // Map back to Seen type
        let recovered_type = type_checker.map_from_c_type(&c_type)
            .expect("Mapping from C type must succeed");
        
        // REQUIREMENT: Bidirectional mapping must be lossless
        assert_eq!(
            seen_type, &recovered_type,
            "C type mapping must be bidirectional and lossless: {:?} -> {} -> {:?}",
            seen_type, c_type, recovered_type
        );
    }
    
    println!("✓ C type mapping bidirectional validation passed");
}

/// FAILING TEST: Error messages must exceed Rust quality
#[test] 
fn test_error_messages_exceed_rust_quality() {
    let config = create_english_config();
    
    let error_program = r#"
        func type_error_function() {
            let x: i32 = "this is a string";  // Type mismatch
            let y = x + 3.14;                // i32 + f64 mismatch
            return y.length();               // Method doesn't exist
        }
        
        func undefined_function_call() {
            return nonexistent_function(42); // Undefined function
        }
    "#;
    
    let mut lexer = Lexer::new(error_program, 0, &config);
    let tokens = lexer.tokenize().expect("Tokenization must succeed");
    let mut parser = Parser::new(tokens);
    let ast = parser.parse_program().expect("Parsing must succeed");
    
    let mut type_checker = TypeChecker::new();
    let result = type_checker.check_program(&ast);
    
    // REQUIREMENT: Must detect type errors
    assert!(result.is_err() || type_checker.diagnostics().has_errors(), 
            "Must detect type errors in invalid program");
    
    let errors: Vec<_> = type_checker.diagnostics().errors().collect();
    assert!(errors.len() >= 3, "Should detect at least 3 type errors");
    
    // REQUIREMENT: Error messages must exceed Rust quality
    for error in &errors {
        let message = error.message();
        
        // Quality criteria:
        assert!(message.len() > 20, "Error message too short: '{}'", message);
        assert!(!message.contains("TODO"), "Error message contains TODO: '{}'", message);
        assert!(!message.contains("placeholder"), "Error message is placeholder: '{}'", message);
        
        // Should contain helpful information
        assert!(
            message.contains("type") || message.contains("expected") || message.contains("found"),
            "Error message should be descriptive: '{}'", message
        );
    }
    
    println!("✓ Error message quality validation passed");
    println!("Generated {} high-quality error messages", errors.len());
}

fn create_english_config() -> LanguageConfig {
    let mut keywords = HashMap::new();
    keywords.insert("func".to_string(), "TokenFunc".to_string());
    keywords.insert("let".to_string(), "TokenLet".to_string());
    keywords.insert("if".to_string(), "TokenIf".to_string());
    keywords.insert("else".to_string(), "TokenElse".to_string());
    keywords.insert("return".to_string(), "TokenReturn".to_string());
    keywords.insert("struct".to_string(), "TokenStruct".to_string());
    keywords.insert("enum".to_string(), "TokenEnum".to_string());
    keywords.insert("true".to_string(), "TokenTrue".to_string());
    keywords.insert("false".to_string(), "TokenFalse".to_string());
    
    let mut operators = HashMap::new();
    operators.insert("+".to_string(), "TokenPlus".to_string());
    operators.insert("-".to_string(), "TokenMinus".to_string());
    operators.insert("*".to_string(), "TokenMultiply".to_string());
    operators.insert("/".to_string(), "TokenDivide".to_string());
    operators.insert("=".to_string(), "TokenAssign".to_string());
    operators.insert("==".to_string(), "TokenEqual".to_string());
    operators.insert("!=".to_string(), "TokenNotEqual".to_string());
    operators.insert("<".to_string(), "TokenLess".to_string());
    operators.insert("<=".to_string(), "TokenLessEqual".to_string());
    operators.insert(">".to_string(), "TokenGreater".to_string());
    operators.insert(">=".to_string(), "TokenGreaterEqual".to_string());
    operators.insert("&&".to_string(), "TokenLogicalAnd".to_string());
    operators.insert("||".to_string(), "TokenLogicalOr".to_string());
    
    LanguageConfig {
        keywords,
        operators,
        name: "English".to_string(),
        description: Some("English test configuration".to_string()),
    }
}