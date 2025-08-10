use seen_lexer::{Lexer, LanguageConfig};
use seen_parser::{Parser};

fn main() {
    let content = "pub class BootstrapManager { }";
    
    println!("=== Testing: {} ===", content);
    
    let language_config = LanguageConfig::new_english();
    let mut lexer = Lexer::new(content, 0, &language_config);
    let tokens = lexer.tokenize();
    
    match tokens {
        Ok(token_list) => {
            println!("=== Tokens ===");
            for (i, token) in token_list.iter().enumerate() {
                println!("{}: {:?}", i, token);
            }
            
            println!("\n=== Parsing ===");
            let mut parser = Parser::new(token_list);
            match parser.parse_program() {
                Ok(program) => {
                    println!("✅ Parsing successful ({} items)", program.items.len());
                    for (i, item) in program.items.iter().enumerate() {
                        println!("Item {}: {:?}", i, item.kind);
                    }
                }
                Err(e) => {
                    println!("❌ Parsing failed: {:?}", e);
                    println!("\nDiagnostics:");
                    for diagnostic in parser.diagnostics().messages.iter() {
                        println!("  {}", diagnostic);
                    }
                }
            }
        }
        Err(e) => {
            println!("❌ Lexing failed: {:?}", e);
        }
    }
}