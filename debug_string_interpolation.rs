use seen_lexer::{Lexer, TokenType, KeywordManager, InterpolationKind};
use std::sync::Arc;

fn main() {
    let keyword_manager = KeywordManager::new();
    
    let mut lexer = Lexer::new(r#""Result: {a + b * 2}""#.to_string(), Arc::new(keyword_manager));
    
    let token = lexer.next_token().unwrap();
    match token.token_type {
        TokenType::InterpolatedString(parts) => {
            println!("Found {} parts:", parts.len());
            for (i, part) in parts.iter().enumerate() {
                match &part.kind {
                    InterpolationKind::Text(text) => println!("  Part {}: Text = '{}'", i, text),
                    InterpolationKind::Expression(expr) => println!("  Part {}: Expression = '{}'", i, expr),
                }
            }
        }
        other => println!("Unexpected token type: {:?}", other),
    }
}