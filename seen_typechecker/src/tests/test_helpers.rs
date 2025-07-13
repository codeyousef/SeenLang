//! Helper utilities for type checker tests

use crate::{Type, TypeCheckResult, TypeChecker, TypeError};
use seen_lexer::keyword_config::{KeywordConfig, KeywordManager};
use seen_lexer::lexer::Lexer;
use seen_parser::ast::Program;
use seen_parser::Parser;
use std::path::PathBuf;

/// Parse and type check a source string
pub fn type_check_source(source: &str) -> TypeCheckResult {
    let program = parse_source(source);
    crate::type_check_program(&program)
}

/// Parse a source string into an AST
pub fn parse_source(source: &str) -> Program {
    // Get the specifications directory
    let lang_files_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("specifications");

    let keyword_config = KeywordConfig::from_directory(&lang_files_dir)
        .expect("Failed to load keyword configuration for testing");

    let keyword_manager = KeywordManager::new(keyword_config, "en".to_string())
        .expect("Failed to create KeywordManager for testing");

    let mut lexer = Lexer::new(source, &keyword_manager, "en".to_string());
    let tokens = lexer.tokenize().expect("Failed to tokenize");
    let mut parser = Parser::new(tokens);
    parser.parse().expect("Failed to parse")
}

/// Assert that type checking succeeds with no errors
pub fn assert_type_check_ok(source: &str) -> TypeCheckResult {
    let result = type_check_source(source);
    assert!(result.is_ok(), "Expected no type errors, but got: {:?}", result.errors);
    result
}

/// Assert that type checking fails with errors
pub fn assert_type_check_error(source: &str) -> Vec<TypeError> {
    let result = type_check_source(source);
    assert!(result.is_err(), "Expected type errors, but type checking succeeded");
    result.errors
}

/// Assert that a variable has a specific type
pub fn assert_variable_type(result: &TypeCheckResult, var_name: &str, expected_type: Type) {
    let actual_type = result.get_variable_type(var_name)
        .expect(&format!("Variable '{}' not found in type check result", var_name));
    assert_eq!(actual_type, &expected_type,
               "Variable '{}' has type {:?}, expected {:?}", var_name, actual_type, expected_type);
}

/// Assert that a specific type error exists
pub fn assert_has_type_error<F>(errors: &[TypeError], predicate: F)
where
    F: Fn(&TypeError) -> bool,
{
    assert!(
        errors.iter().any(|e| predicate(e)),
        "Expected type error not found in: {:?}", errors
    );
}