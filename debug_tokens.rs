// Debug tokenization of Seen compiler source
use std::fs;
use seen_lexer::{Lexer, KeywordManager, TokenType};
use std::sync::Arc;

fn main() {
    let content = fs::read_to_string("compiler_seen/simple_test.seen").unwrap();
    println!("First 200 chars of file:\n{}\n", &content[..content.len().min(200)]);
    
    let keyword_manager = Arc::new(KeywordManager::new());
    let mut lexer = Lexer::new(content, keyword_manager);
    
    println!("First 10 tokens:");
    for i in 0..10 {
        match lexer.next_token() {
            Ok(token) => {
                println!("  {}: {:?}", i, token);
                if token.token_type == TokenType::EOF {
                    break;
                }
            }
            Err(e) => {
                println!("  Error: {}", e);
                break;
            }
        }
    }
}