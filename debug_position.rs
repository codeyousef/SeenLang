use std::sync::Arc;
use seen_lexer::{Lexer, KeywordManager, TokenType, InterpolationKind};

fn main() {
    // Create a simple test string with debug output
    let keyword_manager = KeywordManager::new();
    let input = r#""A{x}B""#;  // Simple case
    
    println!("Simple input: {}", input);
    for (i, c) in input.chars().enumerate() {
        println!("  Position {}: {:?}", i + 1, c);
    }
    
    let mut lexer = Lexer::new(input.to_string(), Arc::new(keyword_manager));
    
    match lexer.next_token() {
        Ok(token) => {
            match token.token_type {
                TokenType::InterpolatedString(parts) => {
                    println!("\nFound {} parts:", parts.len());
                    for (i, part) in parts.iter().enumerate() {
                        match &part.kind {
                            InterpolationKind::Text(text) => {
                                println!("Part {}: Text {:?} at line {}, column {}, offset {}", 
                                       i, text, part.position.line, part.position.column, part.position.offset);
                            }
                            InterpolationKind::Expression(expr) => {
                                println!("Part {}: Expression {:?} at line {}, column {}, offset {}", 
                                       i, expr, part.position.line, part.position.column, part.position.offset);
                            }
                            InterpolationKind::LiteralBrace => {
                                println!("Part {}: LiteralBrace at line {}, column {}, offset {}", 
                                       i, part.position.line, part.position.column, part.position.offset);
                            }
                        }
                    }
                }
                _ => println!("Not an interpolated string: {:?}", token.token_type),
            }
        }
        Err(e) => {
            println!("Error: {:?}", e);
        }
    }
}