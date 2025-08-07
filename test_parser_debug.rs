use seen_lexer::{Lexer, LanguageConfig};
use std::path::Path;

fn main() {
    let code = r#"fun main() {
    println("Testing built-in functions")
}"#;
    
    let lang_path = Path::new("/home/yousef/Development/SeenLang/languages/en.toml");
    let lang_config = LanguageConfig::load_from_file(lang_path)
        .expect("Failed to load language config");
    
    let mut lexer = Lexer::new(code, 0, &lang_config);
    let tokens = lexer.tokenize().unwrap();
    
    println!("Tokens:");
    for token in &tokens {
        println!("  {:?}", token);
    }
}