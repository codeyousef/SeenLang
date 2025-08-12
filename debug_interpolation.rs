use std::sync::Arc;
use seen_lexer::{Lexer, KeywordManager, TokenType, InterpolationKind};

fn main() {
    let mut keyword_manager = KeywordManager::new();
    let input = r#""Hello {name}!""#;
    let lexer = Lexer::new(input.to_string(), Arc::new(keyword_manager));
    
    let mut lexer = lexer;
    let token = lexer.next_token().unwrap();
    
    match token.token_type {
        TokenType::InterpolatedString(parts) => {
            println!("Found {} parts:", parts.len());
            for (i, part) in parts.iter().enumerate() {
                match &part.kind {
                    InterpolationKind::Text(text) => {
                        println!("Part {}: Text '{}' at line {}, column {}", 
                               i, text, part.position.line, part.position.column);
                    }
                    InterpolationKind::Expression(expr) => {
                        println!("Part {}: Expression '{}' at line {}, column {}", 
                               i, expr, part.position.line, part.position.column);
                    }
                    InterpolationKind::LiteralBrace => {
                        println!("Part {}: LiteralBrace at line {}, column {}", 
                               i, part.position.line, part.position.column);
                    }
                }
            }
        }
        _ => println!("Not an interpolated string: {:?}", token.token_type),
    }
}