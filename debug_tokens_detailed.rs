// Debug tokenization around the error position
use std::fs;
use seen_lexer::{Lexer, KeywordManager, TokenType};
use std::sync::Arc;

fn main() {
    let content = fs::read_to_string("compiler_seen/simple_test.seen").unwrap();
    
    let keyword_manager = Arc::new(KeywordManager::new());
    let mut lexer = Lexer::new(content, keyword_manager);
    
    println!("Tokens around position 169:");
    let mut count = 0;
    let mut found_error_area = false;
    
    loop {
        match lexer.next_token() {
            Ok(token) => {
                // Show tokens around offset 169
                if token.position.offset >= 150 && token.position.offset <= 200 {
                    println!("  Token at offset {}: {:?}", token.position.offset, token);
                    found_error_area = true;
                } else if found_error_area {
                    // Show a few more after the error area
                    println!("  Token at offset {}: {:?}", token.position.offset, token);
                    count += 1;
                    if count > 5 {
                        break;
                    }
                }
                
                if token.token_type == TokenType::EOF {
                    break;
                }
            }
            Err(e) => {
                println!("  Lexer error: {}", e);
                break;
            }
        }
    }
}