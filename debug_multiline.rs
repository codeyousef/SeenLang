use std::sync::Arc;
use seen_lexer::{Lexer, KeywordManager, TokenType, InterpolationKind};

fn main() {
    let keyword_manager = KeywordManager::new();
    let input = r#""Line 1
{interpolation}
Line 3""#;
    
    println!("Input string:");
    for (i, c) in input.chars().enumerate() {
        let line = if i < 7 { 1 } else if i < 23 { 2 } else { 3 };
        let col = if i < 7 { i + 1 } else if i < 23 { i - 6 } else { i - 22 };
        println!("  Char pos {}: {:?} (expected line {}, col {})", i + 1, c, line, col);
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