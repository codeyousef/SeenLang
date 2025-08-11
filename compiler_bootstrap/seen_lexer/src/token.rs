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
    
    // String interpolation tokens
    StringInterpolationStart(String),  // The string part before first {
    StringInterpolationMiddle(String), // String parts between } and {
    StringInterpolationEnd(String),    // The string part after last }
    InterpolationExpression(String),   // The expression inside { }
    LeftBraceInterpolation,           // { in interpolation context
    RightBraceInterpolation,          // } in interpolation context
    
    // Identifiers
    Identifier(String),
    
    // Keywords (language-agnostic representation)
    KeywordFun,
    KeywordIf,
    KeywordElse,
    KeywordWhile,
    KeywordFor,
    KeywordIn,
    KeywordReturn,
    KeywordLet,
    KeywordVar,
    KeywordMut,  // For 'borrow mut' syntax
    KeywordTrue,
    KeywordFalse,
    KeywordStruct,
    KeywordEnum,
    KeywordImpl,
    KeywordTrait,
    KeywordUse,
    KeywordImport,
    KeywordModule,
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
    KeywordTry,
    KeywordCatch,
    KeywordFinally,
    KeywordThrow,
    KeywordClass,
    KeywordInline,
    KeywordReified,
    KeywordCrossinline,
    KeywordNoinline,
    KeywordBy,
    KeywordData,
    KeywordSealed,
    KeywordObject,
    KeywordInterface,
    KeywordOpen,
    KeywordFinal,
    KeywordAbstract,
    KeywordOverride,
    KeywordLateinit,
    KeywordCompanion,
    KeywordOperator,
    KeywordInfix,
    KeywordTailrec,
    KeywordWhen,       // when (for pattern matching)
    KeywordNull,       // null
    KeywordAnd,        // and (word operator)
    KeywordOr,         // or (word operator)
    KeywordNot,        // not (word operator)
    KeywordMove,       // move (ownership transfer)
    KeywordBorrow,     // borrow (explicit immutable borrow)
    KeywordInout,      // inout (in-place modification)
    
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
    QuestionDot,       // ?. (safe call operator)
    Elvis,             // ?: (Elvis operator)
    BangBang,          // !! (not-null assertion)
    Dot,               // .
    DotDot,            // .. (range operator)
    DotDotLess,        // ..< (exclusive range)
    DoubleDot,         // .. (alias for compatibility)
    TripleDot,         // ...
    DoubleColon,       // ::
    At,                // @
    Underscore,        // _ (wildcard)
    LeftAngle,         // < (for generics)
    RightAngle,        // > (for generics)
    
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
            
            // String interpolation tokens
            TokenType::StringInterpolationStart(s) => write!(f, "\"{}{{", s),
            TokenType::StringInterpolationMiddle(s) => write!(f, "}}{}{{", s),
            TokenType::StringInterpolationEnd(s) => write!(f, "}}{}\"", s),
            TokenType::InterpolationExpression(expr) => write!(f, "{{{}}}", expr),
            TokenType::LeftBraceInterpolation => write!(f, "{{"),
            TokenType::RightBraceInterpolation => write!(f, "}}"),
            _ => {
                // For keywords and operators, display the enum variant name
                // Language-specific strings should come from LanguageConfig
                let token_str = match self {
                    TokenType::KeywordFun => "KeywordFun",
                    TokenType::KeywordIf => "KeywordIf",
                    TokenType::KeywordElse => "KeywordElse",
                    TokenType::KeywordWhile => "KeywordWhile",
                    TokenType::KeywordFor => "KeywordFor",
                    TokenType::KeywordIn => "KeywordIn",
                    TokenType::KeywordReturn => "KeywordReturn",
                    TokenType::KeywordLet => "KeywordLet",
                    TokenType::KeywordVar => "KeywordVar",
                    TokenType::KeywordMut => "KeywordMut",
                    TokenType::KeywordTrue => "KeywordTrue",
                    TokenType::KeywordFalse => "KeywordFalse",
                    TokenType::KeywordStruct => "KeywordStruct",
                    TokenType::KeywordEnum => "KeywordEnum",
                    TokenType::KeywordImpl => "KeywordImpl",
                    TokenType::KeywordTrait => "KeywordTrait",
                    TokenType::KeywordUse => "KeywordUse",
                    TokenType::KeywordImport => "KeywordImport",
                    TokenType::KeywordModule => "KeywordModule",
                    TokenType::KeywordStatic => "KeywordStatic",
                    TokenType::KeywordConst => "KeywordConst",
                    TokenType::KeywordType => "KeywordType",
                    TokenType::KeywordMatch => "KeywordMatch",
                    TokenType::KeywordBreak => "KeywordBreak",
                    TokenType::KeywordContinue => "KeywordContinue",
                    TokenType::KeywordIs => "KeywordIs",
                    TokenType::KeywordAs => "KeywordAs",
                    TokenType::KeywordSuspend => "KeywordSuspend",
                    TokenType::KeywordAwait => "KeywordAwait",
                    TokenType::KeywordLaunch => "KeywordLaunch",
                    TokenType::KeywordFlow => "KeywordFlow",
                    TokenType::KeywordTry => "KeywordTry",
                    TokenType::KeywordCatch => "KeywordCatch",
                    TokenType::KeywordFinally => "KeywordFinally",
                    TokenType::KeywordThrow => "KeywordThrow",
                    TokenType::KeywordClass => "KeywordClass",
                    TokenType::KeywordInline => "KeywordInline",
                    TokenType::KeywordReified => "KeywordReified",
                    TokenType::KeywordCrossinline => "KeywordCrossinline",
                    TokenType::KeywordNoinline => "KeywordNoinline",
                    TokenType::KeywordBy => "KeywordBy",
                    TokenType::KeywordData => "KeywordData",
                    TokenType::KeywordSealed => "KeywordSealed",
                    TokenType::KeywordObject => "KeywordObject",
                    TokenType::KeywordInterface => "KeywordInterface",
                    TokenType::KeywordOpen => "KeywordOpen",
                    TokenType::KeywordFinal => "KeywordFinal",
                    TokenType::KeywordAbstract => "KeywordAbstract",
                    TokenType::KeywordOverride => "KeywordOverride",
                    TokenType::KeywordLateinit => "KeywordLateinit",
                    TokenType::KeywordCompanion => "KeywordCompanion",
                    TokenType::KeywordOperator => "KeywordOperator",
                    TokenType::KeywordInfix => "KeywordInfix",
                    TokenType::KeywordTailrec => "KeywordTailrec",
                    TokenType::KeywordWhen => "KeywordWhen",
                    TokenType::KeywordNull => "KeywordNull",
                    TokenType::KeywordAnd => "KeywordAnd",
                    TokenType::KeywordOr => "KeywordOr",
                    TokenType::KeywordNot => "KeywordNot",
                    TokenType::KeywordMove => "KeywordMove",
                    TokenType::KeywordBorrow => "KeywordBorrow",
                    TokenType::KeywordInout => "KeywordInout",
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
                    TokenType::QuestionDot => "?.",
                    TokenType::Elvis => "?:",
                    TokenType::BangBang => "!!",
                    TokenType::Dot => ".",
                    TokenType::DotDot => "..",
                    TokenType::DotDotLess => "..<",
                    TokenType::DoubleDot => "..",
                    TokenType::TripleDot => "...",
                    TokenType::DoubleColon => "::",
                    TokenType::At => "@",
                    TokenType::Underscore => "_",
                    TokenType::LeftAngle => "<",
                    TokenType::RightAngle => ">",
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
            TokenType::KeywordFun | TokenType::KeywordIf | TokenType::KeywordElse |
            TokenType::KeywordWhile | TokenType::KeywordFor | TokenType::KeywordIn |
            TokenType::KeywordReturn | TokenType::KeywordLet | TokenType::KeywordVar |
            TokenType::KeywordMut | TokenType::KeywordTrue | TokenType::KeywordFalse | TokenType::KeywordStruct |
            TokenType::KeywordEnum | TokenType::KeywordImpl | TokenType::KeywordTrait |
            TokenType::KeywordUse | TokenType::KeywordImport | TokenType::KeywordModule |
            TokenType::KeywordStatic | TokenType::KeywordConst |
            TokenType::KeywordType | TokenType::KeywordMatch | TokenType::KeywordBreak |
            TokenType::KeywordContinue | TokenType::KeywordIs | TokenType::KeywordAs |
            TokenType::KeywordSuspend | TokenType::KeywordAwait | TokenType::KeywordLaunch |
            TokenType::KeywordFlow | TokenType::KeywordTry | TokenType::KeywordCatch |
            TokenType::KeywordFinally | TokenType::KeywordThrow | TokenType::KeywordClass |
            TokenType::KeywordInline | TokenType::KeywordReified | TokenType::KeywordCrossinline |
            TokenType::KeywordNoinline | TokenType::KeywordBy | TokenType::KeywordData |
            TokenType::KeywordSealed | TokenType::KeywordObject | TokenType::KeywordInterface |
            TokenType::KeywordOpen | TokenType::KeywordFinal | TokenType::KeywordAbstract |
            TokenType::KeywordOverride | TokenType::KeywordLateinit | TokenType::KeywordCompanion |
            TokenType::KeywordOperator | TokenType::KeywordInfix | TokenType::KeywordTailrec |
            TokenType::KeywordWhen | TokenType::KeywordNull |
            TokenType::KeywordAnd | TokenType::KeywordOr | TokenType::KeywordNot |
            TokenType::KeywordMove | TokenType::KeywordBorrow | TokenType::KeywordInout
        )
    }
    
    fn is_operator(&self) -> bool {
        matches!(self.value,
            TokenType::Plus | TokenType::Minus | TokenType::Multiply | TokenType::Divide |
            TokenType::Modulo | TokenType::Assign | TokenType::Equal | TokenType::NotEqual |
            TokenType::Less | TokenType::LessEqual | TokenType::Greater | TokenType::GreaterEqual |
            TokenType::BitwiseAnd | TokenType::BitwiseOr | TokenType::BitwiseXor |
            TokenType::BitwiseNot | TokenType::LeftShift | TokenType::RightShift |
            TokenType::PlusAssign | TokenType::MinusAssign | TokenType::MultiplyAssign |
            TokenType::DivideAssign | TokenType::ModuloAssign | TokenType::Arrow |
            TokenType::FatArrow | TokenType::Question | TokenType::QuestionDot | 
            TokenType::Elvis | TokenType::BangBang | TokenType::Dot | TokenType::DotDot |
            TokenType::DotDotLess | TokenType::DoubleDot | TokenType::TripleDot | 
            TokenType::DoubleColon | TokenType::Underscore | TokenType::LeftAngle | 
            TokenType::RightAngle
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