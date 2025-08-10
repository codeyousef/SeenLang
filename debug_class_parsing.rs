use seen_lexer::{Lexer, LanguageConfig};
use seen_parser::{Parser};

fn main() {
    let test_cases = vec![
        ("pub class", "pub class BootstrapManager { }"),
        ("static method", "fun test() { let x = BootstrapVerifier::new(); }"),
        ("method return type", "fun migrate(&mut self) -> MigrationResult { }"),
        ("complex struct init", "MigrationResult { success: true, message: \"test\".to_string(), verification: Some(result) }"),
        ("Vec generic", "let mut failed_checks = Vec::new();"),
    ];
    
    for (name, content) in test_cases {
        println!("=== Testing: {} ===", name);
        println!("Content: {}", content);
        
        let language_config = LanguageConfig::new_english();
        let mut lexer = Lexer::new(content, 0, &language_config);
        let tokens = lexer.tokenize();
        
        match tokens {
            Ok(token_list) => {
                println!("✅ Lexing successful ({} tokens)", token_list.len());
                
                let mut parser = Parser::new(token_list);
                match parser.parse_program() {
                    Ok(program) => {
                        println!("✅ Parsing successful ({} items)\n", program.items.len());
                    }
                    Err(e) => {
                        println!("❌ Parsing failed: {:?}", e);
                        println!("Diagnostics:");
                        for diagnostic in parser.diagnostics().messages.iter() {
                            println!("  {}", diagnostic);
                        }
                        println!();
                    }
                }
            }
            Err(e) => {
                println!("❌ Lexing failed: {:?}\n", e);
            }
        }
    }
}