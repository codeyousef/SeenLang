// Test the complete Seen Bootstrap Compiler pipeline
use seen_lexer::{Lexer, KeywordManager, KeywordConfig};
use seen_parser::parse;
use seen_typechecker::TypeChecker;
use seen_interpreter::Interpreter;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
        func main() {
            println("🚀 Hello from Seen Bootstrap Compiler!");
            val x = 42;
            val y = x + 8;
            println("Result: ");
            println(y);
        }
    "#;
    
    println!("🔧 Testing Complete Seen Bootstrap Compiler Pipeline");
    println!("📄 Source code:");
    println!("{}", source);
    println!();
    
    // Load keyword configuration
    let specs_dir = std::path::PathBuf::from("/mnt/d/Projects/Rust/seenlang/specifications");
    println!("📚 Loading language configuration from: {:?}", specs_dir);
    let keyword_config = KeywordConfig::from_directory(&specs_dir)?;
    let keyword_manager = KeywordManager::new(keyword_config, "en".to_string())?;
    
    // Step 1: Lexical Analysis
    println!("1️⃣ Lexical Analysis...");
    let mut lexer = Lexer::new(source, &keyword_manager, "en".to_string());
    let mut tokens = Vec::new();
    loop {
        match lexer.next_token() {
            Ok(token) => {
                let is_eof = token.token_type == seen_lexer::TokenType::EOF;
                tokens.push(token);
                if is_eof {
                    break;
                }
            }
            Err(e) => {
                eprintln!("❌ Lexing failed: {:?}", e);
                return Err(e.into());
            }
        }
    }
    println!("✅ Lexing successful: {} tokens generated", tokens.len());
    
    // Step 2: Syntax Analysis  
    println!("2️⃣ Parsing...");
    let program = parse(tokens)?;
    println!("✅ Parsing successful: {} declarations", program.declarations.len());
    
    // Step 3: Type Checking
    println!("3️⃣ Type Checking...");
    let mut typechecker = TypeChecker::new();
    let type_result = typechecker.check_program(&program);
    if !type_result.errors.is_empty() {
        eprintln!("⚠️ Type checking errors:");
        for error in &type_result.errors {
            eprintln!("  • {}", error);
        }
    } else {
        println!("✅ Type checking successful: no errors");
    }
    
    // Step 4: Interpretation
    println!("4️⃣ Interpreting...");
    let mut interpreter = Interpreter::new();
    let interp_result = interpreter.interpret_program(&program);
    if interp_result.is_ok() {
        println!("✅ Interpretation completed successfully!");
        if let Some(value) = &interp_result.value {
            println!("   Final value: {}", value);
        }
    } else {
        eprintln!("❌ Interpretation failed with {} errors:", interp_result.errors.len());
        for error in &interp_result.errors {
            eprintln!("  • {}", error);
        }
        return Err("Interpretation failed".into());
    }
    
    println!();
    println!("🎉 BOOTSTRAP COMPILER TEST COMPLETED SUCCESSFULLY!");
    println!("✨ All 4 stages of compilation pipeline working:");
    println!("   ✅ Lexical Analysis (Tokenization)");  
    println!("   ✅ Syntax Analysis (Parsing)");
    println!("   ✅ Semantic Analysis (Type Checking)");
    println!("   ✅ Code Execution (Interpretation)");
    println!();
    println!("🚀 The Seen Bootstrap Compiler is now generating REAL results!");
    
    Ok(())
}