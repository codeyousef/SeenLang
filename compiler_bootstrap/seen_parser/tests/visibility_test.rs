use seen_parser::Parser;
use seen_lexer::{Lexer, LanguageConfig};

#[test]
fn test_kotlin_visibility_syntax() {
    let config = LanguageConfig::new_english();
    let test_code = r#"
        public class PublicClass {
            public fun publicMethod() {}
        }
        
        private class PrivateClass {
            private fun privateMethod() {}
        }
        
        internal class InternalClass {
            internal fun internalMethod() {}
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
            println!("✅ Visibility parsing successful ({} items)", program.items.len());
            
            // Check that we have 3 class declarations
            assert_eq!(program.items.len(), 3);
            
            // Check public class
            if let seen_parser::ItemKind::Class(class) = &program.items[0].kind {
                assert_eq!(class.name.value, "PublicClass");
                assert_eq!(class.visibility, seen_parser::Visibility::Public);
                println!("✓ Public class parsed correctly");
            } else {
                panic!("Expected Class item, got: {:?}", program.items[0].kind);
            }
            
            // Check private class  
            if let seen_parser::ItemKind::Class(class) = &program.items[1].kind {
                assert_eq!(class.name.value, "PrivateClass");
                assert_eq!(class.visibility, seen_parser::Visibility::Private);
                println!("✓ Private class parsed correctly");
            } else {
                panic!("Expected Class item, got: {:?}", program.items[1].kind);
            }
            
            // Check internal class
            if let seen_parser::ItemKind::Class(class) = &program.items[2].kind {
                assert_eq!(class.name.value, "InternalClass");
                assert_eq!(class.visibility, seen_parser::Visibility::Internal);
                println!("✓ Internal class parsed correctly");
            } else {
                panic!("Expected Class item, got: {:?}", program.items[2].kind);
            }
            
        }
        Err(err) => {
            println!("❌ Parse error: {:?}", err);
            panic!("Parse failed: {:?}", err);
        }
    }
}