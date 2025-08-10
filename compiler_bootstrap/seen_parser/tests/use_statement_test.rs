use seen_parser::Parser;
use seen_lexer::{Lexer, LanguageConfig};

#[test]
fn test_public_use_statement() {
    let config = LanguageConfig::new_english();
    let test_code = r#"
        public use verifier.{
            BootstrapVerifier,
            CompilerInfo,
            VerificationResult
        };
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
            println!("✅ Public use statement parsing successful ({} items)", program.items.len());
            assert_eq!(program.items.len(), 1);
            
            if let seen_parser::ItemKind::Import(import) = &program.items[0].kind {
                // Check visibility
                assert_eq!(import.visibility, seen_parser::Visibility::Public);
                println!("✓ Visibility is public");
                
                // Check the import path
                println!("✓ Use statement parsed as Import successfully");
            } else {
                panic!("Expected Import item, got: {:?}", program.items[0].kind);
            }
        }
        Err(err) => {
            println!("❌ Parse error: {:?}", err);
            panic!("Parse failed: {:?}", err);
        }
    }
}

#[test]
fn test_public_class_simple() {
    let config = LanguageConfig::new_english();
    let test_code = r#"
        public class BootstrapManager {
            public fun new() -> BootstrapManager {
                return BootstrapManager();
            }
        }
    "#;
    
    let mut lexer = Lexer::new(test_code, 0, &config);
    let tokens = lexer.tokenize().expect("Failed to tokenize");
    
    println!("=== Tokens for Class Test ===");
    for (i, token) in tokens.iter().take(10).enumerate() {
        println!("{}: {:?}", i, token);
    }
    
    let mut parser = Parser::new(tokens);
    match parser.parse_program() {
        Ok(program) => {
            println!("✅ Public class parsing successful ({} items)", program.items.len());
            assert_eq!(program.items.len(), 1);
            
            if let seen_parser::ItemKind::Class(class) = &program.items[0].kind {
                assert_eq!(class.name.value, "BootstrapManager");
                assert_eq!(class.visibility, seen_parser::Visibility::Public);
                println!("✓ Public class parsed successfully");
            } else {
                panic!("Expected Class item, got: {:?}", program.items[0].kind);
            }
        }
        Err(err) => {
            println!("❌ Parse error: {:?}", err);
            panic!("Parse failed: {:?}", err);
        }
    }
}