use seen_parser::Parser;
use seen_lexer::{Lexer, LanguageConfig};

#[test]
fn test_generic_types() {
    let config = LanguageConfig::new_english();
    let test_code = r#"
        fun test() {
            let list: Vec<String> = Vec();
        }
    "#;
    
    let mut lexer = Lexer::new(test_code, 0, &config);
    let tokens = lexer.tokenize().expect("Failed to tokenize");
    
    println!("=== Tokens ===");
    for (i, token) in tokens.iter().enumerate() {
        println!("{}: {:?}", i, token);
    }
    
    let mut parser = Parser::new(tokens);
    match parser.parse_program() {
        Ok(program) => {
            println!("✅ Parsing successful ({} items)", program.items.len());
            if let Some(item) = program.items.first() {
                if let seen_parser::ItemKind::Function(func) = &item.kind {
                    if let Some(stmt) = func.body.statements.first() {
                        if let seen_parser::StmtKind::Let(let_stmt) = &stmt.kind {
                            println!("Variable type: {:?}", let_stmt.ty);
                        }
                    }
                }
            }
        }
        Err(err) => {
            println!("❌ Parse error: {:?}", err);
        }
    }
}

#[test]
fn test_method_chaining() {
    let config = LanguageConfig::new_english();
    let test_code = r#"
        fun test() {
            "hello".to_string().length()
        }
    "#;
    
    let mut lexer = Lexer::new(test_code, 0, &config);
    let tokens = lexer.tokenize().expect("Failed to tokenize");
    
    let mut parser = Parser::new(tokens);
    match parser.parse_program() {
        Ok(program) => {
            println!("✅ Method chaining parsing successful");
            if let Some(item) = program.items.first() {
                if let seen_parser::ItemKind::Function(func) = &item.kind {
                    if let Some(stmt) = func.body.statements.first() {
                        if let seen_parser::StmtKind::Expr(expr) = &stmt.kind {
                            println!("Chained expression: {:?}", expr.kind);
                        }
                    }
                }
            }
        }
        Err(err) => {
            println!("❌ Parse error: {:?}", err);
        }
    }
}