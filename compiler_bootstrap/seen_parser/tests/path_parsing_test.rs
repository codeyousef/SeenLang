use seen_parser::Parser;
use seen_lexer::{Lexer, LanguageConfig};

#[test]
fn test_static_method_call_parsing() {
    let config = LanguageConfig::new_english();
    let test_code = r#"
        fun test() {
            BootstrapVerifier::new()
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
            
            // The expression should be a Call with a Path as the function
            if let seen_parser::ExprKind::Call { function, args } = &*expr.kind {
                println!("Function: {:?}", function.kind);
                println!("Args: {:?}", args);
                
                // The function should be a Path
                if let seen_parser::ExprKind::Path(path) = &*function.kind {
                    assert_eq!(path.segments.len(), 2);
                    assert_eq!(path.segments[0].name.value, "BootstrapVerifier");
                    assert_eq!(path.segments[1].name.value, "new");
                    println!("✅ Static method call parsed correctly!");
                } else {
                    panic!("Expected Path expression for function, got: {:?}", function.kind);
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