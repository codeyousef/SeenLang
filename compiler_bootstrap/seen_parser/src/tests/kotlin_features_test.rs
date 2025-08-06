//! Tests for Kotlin-inspired language features
//!
//! This module tests Step 11 implementation including:
//! - Extension functions with receiver types
//! - Data classes with auto-generated methods
//! - Null safety with nullable types
//! - Default and named parameters

use crate::parser::Parser;
use seen_lexer::{Lexer, LanguageConfig};

#[cfg(test)]
mod kotlin_features_tests {
    use super::*;

    fn setup_parser(input: &str) -> Parser {
        let lang_config = LanguageConfig::new_english();
        let mut lexer = Lexer::new(input, 0, &lang_config);
        let tokens = lexer.tokenize().expect("Failed to tokenize test input");
        Parser::new(tokens)
    }

    #[test]
    fn test_extension_function_parsing() {
        let code = r#"
            extension func String.isEmpty(): Bool {
                return self.length() == 0;
            }
            
            extension func Int.isEven(): Bool {
                return self % 2 == 0;
            }
        "#;
        
        let mut parser = setup_parser(code);
        let program = match parser.parse_program() {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Parse error: {:?}", e);
                eprintln!("Diagnostics: {:?}", parser.diagnostics());
                panic!("Failed to parse extension functions");
            }
        };
        
        println!("Parsed {} items", program.items.len());
        assert_eq!(program.items.len(), 2);
        
        // First extension function
        match &program.items[0].kind {
            crate::ast::ItemKind::ExtensionFunction(ext_func) => {
                assert_eq!(ext_func.receiver_type.to_string(), "String");
                assert_eq!(ext_func.function.name.value, "isEmpty");
                assert_eq!(ext_func.function.params.len(), 0); // self is implicit
            }
            _ => panic!("Expected extension function, got {:?}", program.items[0].kind),
        }
        
        // Second extension function  
        match &program.items[1].kind {
            crate::ast::ItemKind::ExtensionFunction(ext_func) => {
                assert_eq!(ext_func.receiver_type.to_string(), "Int");
                assert_eq!(ext_func.function.name.value, "isEven");
            }
            _ => panic!("Expected extension function, got {:?}", program.items[1].kind),
        }
    }

    #[test]
    fn test_data_class_parsing() {
        let code = r#"
            data class Person(
                val name: String,
                val age: Int,
                var email: String = "noemail@example.com"
            );
            
            data class Point(val x: Float, val y: Float);
        "#;
        
        let mut parser = setup_parser(code);
        let program = parser.parse_program().expect("Failed to parse data classes");
        
        assert_eq!(program.items.len(), 2);
        
        // First data class
        match &program.items[0].kind {
            crate::ast::ItemKind::DataClass(data_class) => {
                assert_eq!(data_class.name.value, "Person");
                assert_eq!(data_class.fields.len(), 3);
                
                // Check fields
                assert_eq!(data_class.fields[0].name.value, "name");
                assert!(!data_class.fields[0].is_mutable); // val = immutable
                assert_eq!(data_class.fields[0].default_value, None);
                
                assert_eq!(data_class.fields[1].name.value, "age");
                assert!(!data_class.fields[1].is_mutable);
                
                assert_eq!(data_class.fields[2].name.value, "email");
                assert!(data_class.fields[2].is_mutable); // var = mutable
                assert!(data_class.fields[2].default_value.is_some());
            }
            _ => panic!("Expected data class, got {:?}", program.items[0].kind),
        }
    }

    #[test]
    fn test_nullable_types_parsing() {
        let code = r#"
            func maybeNull(input: String?): Int? {
                if input == null {
                    return null;
                } else {
                    return input.length();
                }
            }
            
            func nonNullExample(name: String): String {
                return "Hello, " + name;
            }
        "#;
        
        let mut parser = setup_parser(code);
        let program = parser.parse_program().expect("Failed to parse nullable types");
        
        assert_eq!(program.items.len(), 2);
        
        match &program.items[0].kind {
            crate::ast::ItemKind::Function(func) => {
                assert_eq!(func.name.value, "maybeNull");
                
                // Check parameter type is nullable String
                assert_eq!(func.params.len(), 1);
                match &*func.params[0].ty.kind {
                    crate::ast::TypeKind::Nullable(inner_type) => {
                        match &*inner_type.kind {
                            crate::ast::TypeKind::Named { path, generic_args: _ } => {
                                assert_eq!(path.segments[0].name.value, "String");
                            }
                            _ => panic!("Expected Path type inside Nullable"),
                        }
                    }
                    _ => panic!("Expected nullable type for parameter"),
                }
                
                // Check return type is nullable Int
                match &*func.return_type.as_ref().expect("Function should have return type").kind {
                    crate::ast::TypeKind::Nullable(inner_type) => {
                        match &*inner_type.kind {
                            crate::ast::TypeKind::Named { path, generic_args: _ } => {
                                assert_eq!(path.segments[0].name.value, "Int");
                            }
                            _ => panic!("Expected Path type inside Nullable"),
                        }
                    }
                    _ => panic!("Expected nullable return type"),
                }
            }
            _ => panic!("Expected function, got {:?}", program.items[0].kind),
        }
    }

    #[test]
    fn test_default_and_named_parameters() {
        let code = r#"
            func createUser(
                name: String,
                age: Int = 25,
                email: String = "noemail@example.com",
                isActive: Bool = true
            ): User {
                return User { name: name, age: age, email: email, isActive: isActive };
            }
            
            func main() {
                let user1 = createUser("John");
                let user2 = createUser("Jane", age: 30);
                let user3 = createUser("Bob", email: "bob@test.com", age: 35);
            }
        "#;
        
        let mut parser = setup_parser(code);
        let program = parser.parse_program().expect("Failed to parse default parameters");
        
        // Check function definition has default values
        match &program.items[0].kind {
            crate::ast::ItemKind::Function(func) => {
                assert_eq!(func.name.value, "createUser");
                assert_eq!(func.params.len(), 4);
                
                // First parameter has no default
                assert!(func.params[0].default_value.is_none());
                assert_eq!(func.params[0].name.value, "name");
                
                // Other parameters have defaults
                assert!(func.params[1].default_value.is_some());
                assert!(func.params[2].default_value.is_some());
                assert!(func.params[3].default_value.is_some());
            }
            _ => panic!("Expected function"),
        }
        
        // Check function calls with named arguments
        match &program.items[1].kind {
            crate::ast::ItemKind::Function(main_func) => {
                assert_eq!(main_func.name.value, "main");
                
                // Check function body contains calls with named args
                let statements = &main_func.body.statements;
                assert_eq!(statements.len(), 3);
                
                // user2 call should have named parameter
                if let crate::ast::StmtKind::Let(let_stmt) = &statements[1].kind {
                    if let Some(ref initializer) = let_stmt.initializer {
                        if let crate::ast::ExprKind::Call { function: _, args } = &*initializer.kind {
                        assert_eq!(args.len(), 2); // "Jane" + age: 30
                        
                        match &*args[1].kind {
                            crate::ast::ExprKind::NamedArg { name, value } => {
                                assert_eq!(name.value, "age");
                                match &*value.kind {
                                    crate::ast::ExprKind::Literal(lit) => {
                                        match &lit.kind {
                                            crate::ast::LiteralKind::Integer(30) => {},
                                            _ => panic!("Expected integer 30"),
                                        }
                                    }
                                    _ => panic!("Expected literal"),
                                }
                            }
                            _ => panic!("Expected named argument"),
                        }
                        }
                    }
                }
            }
            _ => panic!("Expected main function"),
        }
    }

    #[test]
    fn test_pattern_matching_with_guards() {
        let code = r#"
            func classify(value: Int): String {
                return match value {
                    n if n < 0 => "negative",
                    0 => "zero", 
                    n if n > 0 && n <= 10 => "small positive",
                    n if n > 10 => "large positive",
                    _ => "impossible"
                };
            }
        "#;
        
        let mut parser = setup_parser(code);
        let program = parser.parse_program().expect("Failed to parse pattern matching with guards");
        
        match &program.items[0].kind {
            crate::ast::ItemKind::Function(func) => {
                assert_eq!(func.name.value, "classify");
                
                // Check return statement has match expression
                if let crate::ast::StmtKind::Return(Some(expr)) = &func.body.statements[0].kind {
                    match &*expr.kind {
                        crate::ast::ExprKind::Match { scrutinee: _, arms } => {
                            assert_eq!(arms.len(), 5);
                            
                            // First arm has guard
                            assert!(arms[0].guard.is_some());
                            match &arms[0].pattern.kind {
                                crate::ast::PatternKind::Identifier(name) => {
                                    assert_eq!(name.value, "n");
                                }
                                _ => panic!("Expected identifier pattern"),
                            }
                            
                            // Second arm is literal without guard
                            assert!(arms[1].guard.is_none());
                            match &arms[1].pattern.kind {
                                crate::ast::PatternKind::Literal(lit) => {
                                    match &lit.kind {
                                        crate::ast::LiteralKind::Integer(0) => {},
                                        _ => panic!("Expected integer 0"),
                                    }
                                }
                                _ => panic!("Expected literal pattern"),
                            }
                            
                            // Last arm is wildcard
                            match &arms[4].pattern.kind {
                                crate::ast::PatternKind::Wildcard => {},
                                _ => panic!("Expected wildcard pattern"),
                            }
                        }
                        _ => panic!("Expected match expression"),
                    }
                }
            }
            _ => panic!("Expected function"),
        }
    }

    #[test]
    fn test_closure_expressions() {
        let code = r#"
            func main() {
                let numbers = [1, 2, 3, 4, 5];
                
                let doubled = numbers.map(|x| x * 2);
                let evens = numbers.filter(|x| x % 2 == 0);
                
                let sum = numbers.fold(0, |acc, x| acc + x);
                
                let complex_closure = |a: Int, b: Int| -> Int {
                    let result = a * b;
                    return result + 1;
                };
            }
        "#;
        
        let mut parser = setup_parser(code);
        let program = parser.parse_program().expect("Failed to parse closures");
        
        match &program.items[0].kind {
            crate::ast::ItemKind::Function(main_func) => {
                let statements = &main_func.body.statements;
                
                // Check doubled assignment has closure
                if let crate::ast::StmtKind::Let(let_stmt) = &statements[1].kind {
                    // Check pattern is identifier "doubled"
                    match &let_stmt.pattern.kind {
                        crate::ast::PatternKind::Identifier(name) => {
                            assert_eq!(name.value, "doubled");
                        }
                        _ => panic!("Expected identifier pattern"),
                    }
                    
                    if let Some(ref initializer) = let_stmt.initializer {
                        if let crate::ast::ExprKind::MethodCall { receiver: _, method, args } = &*initializer.kind {
                            assert_eq!(method.value, "map");
                            assert_eq!(args.len(), 1);
                            
                            match &*args[0].kind {
                                crate::ast::ExprKind::Closure(closure) => {
                                    assert_eq!(closure.params.len(), 1);
                                    assert_eq!(closure.params[0].name.value, "x");
                                    assert!(closure.return_type.is_none()); // Inferred return type
                                    
                                    match &closure.body {
                                        crate::ast::ClosureBody::Expression(expr) => {
                                            match &*expr.kind {
                                                crate::ast::ExprKind::Binary { op, left: _, right: _ } => {
                                                    match op {
                                                        crate::ast::BinaryOp::Mul => {},
                                                        _ => panic!("Expected multiplication operator"),
                                                    }
                                                }
                                                _ => panic!("Expected binary expression"),
                                            }
                                        }
                                        _ => panic!("Expected expression body"),
                                    }
                                }
                                _ => panic!("Expected closure expression"),
                            }
                        }
                    }
                }
            }
            _ => panic!("Expected main function"),
        }
    }
}