use seen_parser::Parser;
use seen_lexer::{Lexer, LanguageConfig};

#[test]
fn test_kotlin_constructor_syntax() {
    let config = LanguageConfig::new_english();
    let test_code = r#"
        fun test() {
            BootstrapVerifier()
        }
    "#;
    
    let mut lexer = Lexer::new(test_code, 0, &config);
    let tokens = lexer.tokenize().expect("Failed to tokenize");
    
    println!("=== Tokens ===");
    for (i, token) in tokens.iter().enumerate() {
        println!("{}: {:?}", i, token);
    }
    
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().expect("Failed to parse");
    
    assert_eq!(program.items.len(), 1);
    
    if let seen_parser::ItemKind::Function(func) = &program.items[0].kind {
        assert_eq!(func.body.statements.len(), 1);
        
        if let seen_parser::StmtKind::Expr(expr) = &func.body.statements[0].kind {
            println!("Expression kind: {:?}", expr.kind);
            
            // The expression should be a Call with an Identifier as the function
            if let seen_parser::ExprKind::Call { function, args } = &*expr.kind {
                println!("Function: {:?}", function.kind);
                println!("Args: {:?}", args);
                
                // The function should be an Identifier (constructor call)
                if let seen_parser::ExprKind::Identifier(name) = &*function.kind {
                    assert_eq!(name.value, "BootstrapVerifier");
                    println!("✅ Constructor call parsed correctly!");
                } else {
                    panic!("Expected Identifier expression for function, got: {:?}", function.kind);
                }
            } else {
                panic!("Expected Call expression, got: {:?}", expr.kind);
            }
        } else {
            panic!("Expected expression statement");
        }
    } else {
        panic!("Expected function item");
    }
}

#[test]
fn test_static_method_call_syntax() {
    let config = LanguageConfig::new_english();
    let test_code = r#"
        fun test() {
            System.out.println("hello")
        }
    "#;
    
    let mut lexer = Lexer::new(test_code, 0, &config);
    let tokens = lexer.tokenize().expect("Failed to tokenize");
    
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().expect("Failed to parse");
    
    assert_eq!(program.items.len(), 1);
    
    if let seen_parser::ItemKind::Function(func) = &program.items[0].kind {
        if let seen_parser::StmtKind::Expr(expr) = &func.body.statements[0].kind {
            // Should be a method call: System.out.println()
            if let seen_parser::ExprKind::MethodCall { receiver, method, args } = &*expr.kind {
                // receiver should be System.out (a field access)
                if let seen_parser::ExprKind::FieldAccess { object, field } = &*receiver.kind {
                    if let seen_parser::ExprKind::Identifier(name) = &*object.kind {
                        assert_eq!(name.value, "System");
                        assert_eq!(field.value, "out");
                        assert_eq!(method.value, "println");
                        println!("✅ Static method call parsed correctly!");
                    } else {
                        panic!("Expected System identifier, got: {:?}", object.kind);
                    }
                } else {
                    panic!("Expected field access for System.out, got: {:?}", receiver.kind);
                }
            } else {
                panic!("Expected method call, got: {:?}", expr.kind);
            }
        }
    }
}