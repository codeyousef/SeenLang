//! Token definitions for the Seen language lexer

use crate::position::Position;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Token {
    pub token_type: TokenType,
    pub lexeme: String,
    pub position: Position,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TokenType {
    // Literals
    IntegerLiteral(i64),
    UIntegerLiteral(u64),
    FloatLiteral(f64),
    StringLiteral(String),
    CharLiteral(char),
    BoolLiteral(bool),
    InterpolatedString(Vec<InterpolationPart>),
    
    // Identifiers (visibility based on capitalization)
    PublicIdentifier(String),
    PrivateIdentifier(String),
    
    // Keywords (dynamically loaded from TOML)
    // Single variant for all keywords - no hardcoding!
    Keyword(crate::keyword_manager::KeywordType),
    
    // Logical operators (research-based)
    LogicalAnd, LogicalOr, LogicalNot,
    
    // Memory management (Vale-style)
    Move, Borrow, Inout,
    
    // Mathematical operators
    Plus, Minus, Multiply, Divide, Modulo,
    
    // Assignment operators
    Assign,             // =
    
    // Comparison operators
    Equal, NotEqual, Less, Greater, LessEqual, GreaterEqual,
    
    // Nullable operators
    SafeNavigation,     // ?.
    Elvis,              // ?:
    ForceUnwrap,        // !!
    Question,           // ?
    
    // Range operators
    InclusiveRange,     // ..
    ExclusiveRange,     // ..<
    
    // String interpolation tokens
    InterpolationStart, // {
    InterpolationEnd,   // }
    LiteralBrace,       // {{ or }}
    
    // Punctuation
    LeftParen, RightParen,
    LeftBrace, RightBrace,
    LeftBracket, RightBracket,
    Comma, Colon,
    Dot,                // .
    Arrow,              // ->
    
    // Special tokens
    Newline, EOF,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InterpolationPart {
    pub kind: InterpolationKind,
    pub content: String,
    pub position: Position,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum InterpolationKind {
    Text(String),
    Expression(String),
    LiteralBrace,
}

impl Token {
    pub fn new(token_type: TokenType, lexeme: String, position: Position) -> Self {
        Self {
            token_type,
            lexeme,
            position,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_token_creation() {
        let pos = Position::new(1, 1, 0);
        let token = Token::new(TokenType::Plus, "+".to_string(), pos);
        
        assert_eq!(token.token_type, TokenType::Plus);
        assert_eq!(token.lexeme, "+");
        assert_eq!(token.position.line, 1);
    }
}