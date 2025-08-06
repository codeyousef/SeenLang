//! Token definitions for the Seen language

use seen_common::{Span, Spanned};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Token types in the Seen language
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TokenType {
    // Literals
    IntegerLiteral(i64),
    FloatLiteral(String), // Store as string to avoid f64 Hash/Eq issues
    StringLiteral(String),
    BooleanLiteral(bool),
    CharLiteral(char),
    
    // Identifiers
    Identifier(String),
    
    // Keywords (language-agnostic representation)
    KeywordFunc,
    KeywordIf,
    KeywordElse,
    KeywordWhile,
    KeywordFor,
    KeywordIn,
    KeywordReturn,
    KeywordLet,
    KeywordMut,
    KeywordVal,
    KeywordTrue,
    KeywordFalse,
    KeywordStruct,
    KeywordEnum,
    KeywordImpl,
    KeywordTrait,
    KeywordImport,
    KeywordModule,
    KeywordPub,
    KeywordPriv,
    KeywordStatic,
    KeywordConst,
    KeywordType,
    KeywordMatch,
    KeywordBreak,
    KeywordContinue,
    KeywordIs,
    KeywordAs,
    KeywordSuspend,
    KeywordAwait,
    KeywordLaunch,
    KeywordFlow,
    
    // Operators
    Plus,              // +
    Minus,             // -
    Multiply,          // *
    Divide,            // /
    Modulo,            // %
    Assign,            // =
    Equal,             // ==
    NotEqual,          // !=
    Less,              // <
    LessEqual,         // <=
    Greater,           // >
    GreaterEqual,      // >=
    LogicalAnd,        // &&
    LogicalOr,         // ||
    LogicalNot,        // !
    BitwiseAnd,        // &
    BitwiseOr,         // |
    BitwiseXor,        // ^
    BitwiseNot,        // ~
    LeftShift,         // <<
    RightShift,        // >>
    PlusAssign,        // +=
    MinusAssign,       // -=
    MultiplyAssign,    // *=
    DivideAssign,      // /=
    ModuloAssign,      // %=
    Arrow,             // ->
    FatArrow,          // =>
    Question,          // ?
    Dot,               // .
    DoubleDot,         // ..
    TripleDot,         // ...
    DoubleColon,       // ::
    
    // Delimiters
    LeftParen,         // (
    RightParen,        // )
    LeftBrace,         // {
    RightBrace,        // }
    LeftBracket,       // [
    RightBracket,      // ]
    Semicolon,         // ;
    Comma,             // ,
    Colon,             // :
    
    // Special
    Newline,
    Whitespace,
    Comment(String),
    EndOfFile,
    
    // Error token for error recovery
    Error(String),
}

impl fmt::Display for TokenType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenType::IntegerLiteral(n) => write!(f, "{}", n),
            TokenType::FloatLiteral(n) => write!(f, "{}", n),
            TokenType::StringLiteral(s) => write!(f, "\"{}\"", s),
            TokenType::BooleanLiteral(b) => write!(f, "{}", b),
            TokenType::CharLiteral(c) => write!(f, "'{}'", c),
            TokenType::Identifier(name) => write!(f, "{}", name),
            TokenType::Comment(text) => write!(f, "// {}", text),
            TokenType::Error(msg) => write!(f, "ERROR: {}", msg),
            _ => {
                let token_str = match self {
                    TokenType::KeywordFunc => "func",
                    TokenType::KeywordIf => "if",
                    TokenType::KeywordElse => "else",
                    TokenType::KeywordWhile => "while",
                    TokenType::KeywordFor => "for",
                    TokenType::KeywordIn => "in",
                    TokenType::KeywordReturn => "return",
                    TokenType::KeywordLet => "let",
                    TokenType::KeywordMut => "mut",
                    TokenType::KeywordVal => "val",
                    TokenType::KeywordTrue => "true",
                    TokenType::KeywordFalse => "false",
                    TokenType::KeywordStruct => "struct",
                    TokenType::KeywordEnum => "enum",
                    TokenType::KeywordImpl => "impl",
                    TokenType::KeywordTrait => "trait",
                    TokenType::KeywordImport => "import",
                    TokenType::KeywordModule => "module",
                    TokenType::KeywordPub => "pub",
                    TokenType::KeywordPriv => "priv",
                    TokenType::KeywordStatic => "static",
                    TokenType::KeywordConst => "const",
                    TokenType::KeywordType => "type",
                    TokenType::KeywordMatch => "match",
                    TokenType::KeywordBreak => "break",
                    TokenType::KeywordContinue => "continue",
                    TokenType::Plus => "+",
                    TokenType::Minus => "-",
                    TokenType::Multiply => "*",
                    TokenType::Divide => "/",
                    TokenType::Modulo => "%",
                    TokenType::Assign => "=",
                    TokenType::Equal => "==",
                    TokenType::NotEqual => "!=",
                    TokenType::Less => "<",
                    TokenType::LessEqual => "<=",
                    TokenType::Greater => ">",
                    TokenType::GreaterEqual => ">=",
                    TokenType::LogicalAnd => "&&",
                    TokenType::LogicalOr => "||",
                    TokenType::LogicalNot => "!",
                    TokenType::BitwiseAnd => "&",
                    TokenType::BitwiseOr => "|",
                    TokenType::BitwiseXor => "^",
                    TokenType::BitwiseNot => "~",
                    TokenType::LeftShift => "<<",
                    TokenType::RightShift => ">>",
                    TokenType::PlusAssign => "+=",
                    TokenType::MinusAssign => "-=",
                    TokenType::MultiplyAssign => "*=",
                    TokenType::DivideAssign => "/=",
                    TokenType::ModuloAssign => "%=",
                    TokenType::Arrow => "->",
                    TokenType::FatArrow => "=>",
                    TokenType::Question => "?",
                    TokenType::Dot => ".",
                    TokenType::DoubleDot => "..",
                    TokenType::TripleDot => "...",
                    TokenType::DoubleColon => "::",
                    TokenType::LeftParen => "(",
                    TokenType::RightParen => ")",
                    TokenType::LeftBrace => "{",
                    TokenType::RightBrace => "}",
                    TokenType::LeftBracket => "[",
                    TokenType::RightBracket => "]",
                    TokenType::Semicolon => ";",
                    TokenType::Comma => ",",
                    TokenType::Colon => ":",
                    TokenType::Newline => "\\n",
                    TokenType::Whitespace => " ",
                    TokenType::EndOfFile => "EOF",
                    _ => unreachable!(),
                };
                write!(f, "{}", token_str)
            }
        }
    }
}

/// A token with source location information
pub type Token = Spanned<TokenType>;

/// Token utility trait
pub trait TokenUtils {
    fn new(token_type: TokenType, span: Span) -> Self;
    fn is_keyword(&self) -> bool;
    fn is_operator(&self) -> bool;
    fn is_literal(&self) -> bool;
    fn is_delimiter(&self) -> bool;
    fn is_whitespace(&self) -> bool;
    fn is_error(&self) -> bool;
}

impl TokenUtils for Token {
    fn new(token_type: TokenType, span: Span) -> Self {
        Spanned::new(token_type, span)
    }
    
    fn is_keyword(&self) -> bool {
        matches!(self.value, 
            TokenType::KeywordFunc | TokenType::KeywordIf | TokenType::KeywordElse |
            TokenType::KeywordWhile | TokenType::KeywordFor | TokenType::KeywordIn |
            TokenType::KeywordReturn | TokenType::KeywordLet | TokenType::KeywordMut |
            TokenType::KeywordVal |
            TokenType::KeywordTrue | TokenType::KeywordFalse | TokenType::KeywordStruct |
            TokenType::KeywordEnum | TokenType::KeywordImpl | TokenType::KeywordTrait |
            TokenType::KeywordImport | TokenType::KeywordModule | TokenType::KeywordPub |
            TokenType::KeywordPriv | TokenType::KeywordStatic | TokenType::KeywordConst |
            TokenType::KeywordType | TokenType::KeywordMatch | TokenType::KeywordBreak |
            TokenType::KeywordContinue
        )
    }
    
    fn is_operator(&self) -> bool {
        matches!(self.value,
            TokenType::Plus | TokenType::Minus | TokenType::Multiply | TokenType::Divide |
            TokenType::Modulo | TokenType::Assign | TokenType::Equal | TokenType::NotEqual |
            TokenType::Less | TokenType::LessEqual | TokenType::Greater | TokenType::GreaterEqual |
            TokenType::LogicalAnd | TokenType::LogicalOr | TokenType::LogicalNot |
            TokenType::BitwiseAnd | TokenType::BitwiseOr | TokenType::BitwiseXor |
            TokenType::BitwiseNot | TokenType::LeftShift | TokenType::RightShift |
            TokenType::PlusAssign | TokenType::MinusAssign | TokenType::MultiplyAssign |
            TokenType::DivideAssign | TokenType::ModuloAssign | TokenType::Arrow |
            TokenType::FatArrow | TokenType::Question | TokenType::Dot | TokenType::DoubleDot |
            TokenType::TripleDot | TokenType::DoubleColon
        )
    }
    
    fn is_literal(&self) -> bool {
        matches!(self.value,
            TokenType::IntegerLiteral(_) | TokenType::FloatLiteral(_) |
            TokenType::StringLiteral(_) | TokenType::BooleanLiteral(_) |
            TokenType::CharLiteral(_)
        )
    }
    
    fn is_delimiter(&self) -> bool {
        matches!(self.value,
            TokenType::LeftParen | TokenType::RightParen | TokenType::LeftBrace |
            TokenType::RightBrace | TokenType::LeftBracket | TokenType::RightBracket |
            TokenType::Semicolon | TokenType::Comma | TokenType::Colon
        )
    }
    
    fn is_whitespace(&self) -> bool {
        matches!(self.value, TokenType::Whitespace | TokenType::Newline)
    }
    
    fn is_error(&self) -> bool {
        matches!(self.value, TokenType::Error(_))
    }
}