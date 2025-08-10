use seen_lexer::{Lexer, LanguageConfig};
use seen_parser::{Parser};

fn main() {
    let content = "pub use verifier.{
    BootstrapVerifier,
    CompilerInfo,
    VerificationResult
};";
    
    println!("=== Lexing Input ===");
    println!("{}", content);
    println!("\n=== Tokens ===");
    
    let language_config = LanguageConfig::new_english();
    let mut lexer = Lexer::new(content, 0, &language_config);
    let tokens = lexer.tokenize();
    
    match tokens {
        Ok(token_list) => {
            for (i, token) in token_list.iter().enumerate() {
                println!("{}: {:?}", i, token);
            }
            
            println!("\n=== Parsing ===");
            let mut parser = Parser::new(token_list);
            match parser.parse_program() {
                Ok(program) => {
                    println!("✅ Parse successful!");
                    println!("Items: {}", program.items.len());
                }
                Err(e) => {
                    println!("❌ Parse failed: {:?}", e);
                    println!("\nDiagnostics:");
                    for diagnostic in parser.diagnostics().messages.iter() {
                        println!("  {}", diagnostic);
                    }
                }
            }
        }
        Err(e) => {
            println!("❌ Lexer failed: {:?}", e);
        }
    }
}