// Simple test to use the Rust bootstrap compiler directly
use seen_lexer::{Lexer, KeywordManager, KeywordConfig};
use seen_parser::Parser;
use seen_interpreter::Interpreter;

fn main() {
    let source = r#"
        fun main() {
            println("Hello from Seen Bootstrap Compiler!");
            val x = 42;
            val y = x + 8;
            println(y);
        }
    "#;
    
    println!("🚀 Testing Seen Bootstrap Compiler");
    
    // Load keyword configuration
    let specs_dir = std::path::PathBuf::from("/mnt/d/Projects/Rust/seenlang/specifications");
    let keyword_config = KeywordConfig::from_directory(&specs_dir)
        .expect("Failed to load keyword configuration");
    
    let keyword_manager = KeywordManager::new(keyword_config, "en".to_string())
        .expect("Failed to create KeywordManager");
    
    // Lexical analysis
    println!("📝 Lexical analysis...");
    let mut lexer = Lexer::new(source, &keyword_manager, "en".to_string());
    let tokens = match lexer.tokenize() {
        Ok(tokens) => {
            println!("✅ Lexing successful: {} tokens", tokens.len());
            tokens
        }
        Err(e) => {
            eprintln!("❌ Lexing failed: {:?}", e);
            return;
        }
    };
    
    // Syntax analysis  
    println!("🌳 Parsing...");
    let mut parser = Parser::new(tokens);
    let program = match parser.parse() {
        Ok(program) => {
            println!("✅ Parsing successful: {} declarations", program.declarations.len());
            program
        }
        Err(e) => {
            eprintln!("❌ Parsing failed: {:?}", e);
            return;
        }
    };
    
    // Interpretation
    println!("⚡ Interpreting...");
    let mut interpreter = Interpreter::new();
    match interpreter.interpret_program(&program) {
        Ok(_) => println!("✅ Interpretation successful!"),
        Err(e) => eprintln!("❌ Interpretation failed: {:?}", e),
    }
    
    println!("🎉 Bootstrap compiler test completed!");
}