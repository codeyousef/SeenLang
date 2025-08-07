//! Simple test for basic reactive types with generics

use crate::parser::Parser;
use seen_lexer::{Lexer, LanguageConfig};
use crate::ast::*;

fn setup_parser(code: &str) -> Parser {
    let lang_config = LanguageConfig::new_english();
    let mut lexer = Lexer::new(code, 0, &lang_config);
    let tokens = lexer.tokenize().expect("Failed to tokenize test input");
    Parser::new(tokens)
}

#[cfg(test)]
mod simple_reactive_tests {
    use super::*;

    #[test]
    fn test_basic_generic_types() {
        let code = r#"
            fun createObservable(): Observable<String> {
                return Observable.empty()
            }
            
            fun processFlow(flow: Flow<User>): Flow<ProcessedUser> {
                return flow
            }
        "#;

        let mut parser = setup_parser(code);
        let program = parser.parse_program().expect("Failed to parse generic types");

        assert_eq!(program.items.len(), 2);
        
        // Test Observable<String> return type
        match &program.items[0].kind {
            ItemKind::Function(func) => {
                assert_eq!(func.name.value, "createObservable");
                
                if let Some(ref return_type) = func.return_type {
                    match &*return_type.kind {
                        TypeKind::Named { path, generic_args } => {
                            assert_eq!(path.segments[0].name.value, "Observable");
                            assert_eq!(generic_args.len(), 1);
                            
                            // Check String generic argument
                            match &*generic_args[0].kind {
                                TypeKind::Named { path: inner_path, .. } => {
                                    assert_eq!(inner_path.segments[0].name.value, "String");
                                }
                                _ => panic!("Expected String type argument"),
                            }
                        }
                        _ => panic!("Expected Named type Observable<String>, got {:?}", return_type.kind),
                    }
                } else {
                    panic!("Expected return type");
                }
            }
            _ => panic!("Expected function, got {:?}", program.items[0].kind),
        }
    }

    #[test]
    fn test_suspend_function_with_generic_return() {
        let code = r#"
            suspend fun fetchUsers(): Observable<List<User>> {
                return Observable.empty()
            }
        "#;

        let mut parser = setup_parser(code);
        let program = parser.parse_program().expect("Failed to parse suspend with generic return");

        assert_eq!(program.items.len(), 1);
        
        match &program.items[0].kind {
            ItemKind::Function(func) => {
                assert_eq!(func.name.value, "fetchUsers");
                
                // Should have suspend attribute
                assert_eq!(func.attributes.len(), 1);
                assert_eq!(func.attributes[0].name.value, "suspend");
                
                // Should have Observable<List<User>> return type
                if let Some(ref return_type) = func.return_type {
                    match &*return_type.kind {
                        TypeKind::Named { path, generic_args } => {
                            assert_eq!(path.segments[0].name.value, "Observable");
                            assert_eq!(generic_args.len(), 1);
                            
                            // Check List<User> generic argument
                            match &*generic_args[0].kind {
                                TypeKind::Named { path: list_path, generic_args: list_args } => {
                                    assert_eq!(list_path.segments[0].name.value, "List");
                                    assert_eq!(list_args.len(), 1);
                                    
                                    // Check User argument
                                    match &*list_args[0].kind {
                                        TypeKind::Named { path: user_path, .. } => {
                                            assert_eq!(user_path.segments[0].name.value, "User");
                                        }
                                        _ => panic!("Expected User type"),
                                    }
                                }
                                _ => panic!("Expected List<User> type argument"),
                            }
                        }
                        _ => panic!("Expected Observable<List<User>> type"),
                    }
                }
            }
            _ => panic!("Expected function"),
        }
    }

    #[test] 
    fn test_nullable_generic_types() {
        let code = r#"
            fun maybeObservable(): Observable<String>? {
                return null
            }
        "#;

        let mut parser = setup_parser(code);
        let program = parser.parse_program().expect("Failed to parse nullable generic types");

        assert_eq!(program.items.len(), 1);
        
        match &program.items[0].kind {
            ItemKind::Function(func) => {
                assert_eq!(func.name.value, "maybeObservable");
                
                if let Some(ref return_type) = func.return_type {
                    // Should be Nullable(Observable<String>)
                    match &*return_type.kind {
                        TypeKind::Nullable(inner) => {
                            match &*inner.kind {
                                TypeKind::Named { path, generic_args } => {
                                    assert_eq!(path.segments[0].name.value, "Observable");
                                    assert_eq!(generic_args.len(), 1);
                                }
                                _ => panic!("Expected Observable<String> inside nullable"),
                            }
                        }
                        _ => panic!("Expected nullable type"),
                    }
                }
            }
            _ => panic!("Expected function"),
        }
    }
}