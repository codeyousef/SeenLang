use serde::{Deserialize, Serialize};
use std::fmt;

/// Represents a source position in the code
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

impl Position {
    pub fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }
}

impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
    }
}

/// Represents a location in the source code with start and end positions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Location {
    pub start: Position,
    pub end: Position,
}

impl Location {
    pub fn new(start: Position, end: Position) -> Self {
        Self { start, end }
    }

    pub fn from_positions(start_line: usize, start_column: usize, end_line: usize, end_column: usize) -> Self {
        Self {
            start: Position::new(start_line, start_column),
            end: Position::new(end_line, end_column),
        }
    }
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}", self.start, self.end)
    }
}

/// The language-neutral token type for Seen language
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum TokenType {
    // Keywords
    Val,      // val (EN) / ثابت (AR)
    Var,      // var (EN) / متغير (AR)
    Func,     // func (EN) / دالة (AR)
    If,       // if (EN) / إذا (AR)
    Else,     // else (EN) / وإلا (AR)
    While,    // while (EN) / طالما (AR)
    For,      // for (EN) / لكل (AR)
    Return,   // return (EN) / إرجاع (AR)
    True,     // true (EN) / صحيح (AR)
    False,    // false (EN) / خطأ (AR)
    Null,     // null (EN) / فارغ (AR)
    Println,  // println (EN) / اطبع (AR)
    When,     // when (EN) / عندما (AR)
    In,       // in (EN) / في (AR)
    Loop,     // loop (EN) / حلقة (AR)
    Break,    // break (EN) / اخرج (AR)
    Continue, // continue (EN) / استمر (AR)
    Struct,   // struct (EN) / هيكل (AR)
    Enum,     // enum (EN) / تعداد (AR)
    Unsafe,   // unsafe (EN) / غير_آمن (AR)
    Ref,      // ref (EN) / مرجع (AR)
    Own,      // own (EN) / ملك (AR)
    Async,    // async (EN) / غير_متزامن (AR)
    Await,    // await (EN) / انتظر (AR)

    // Literals
    IntLiteral,
    FloatLiteral,
    StringLiteral,

    // Identifiers
    Identifier,

    // Operators
    Plus,       // +
    Minus,      // -
    Multiply,   // *
    Divide,     // /
    Modulo,     // %
    Assign,     // =
    Equal,      // ==
    NotEqual,   // !=
    LessThan,   // <
    GreaterThan, // >
    LessEqual,  // <=
    GreaterEqual, // >=
    And,        // &&
    Or,         // ||
    Not,        // !

    // Delimiters
    LeftParen,  // (
    RightParen, // )
    LeftBrace,  // {
    RightBrace, // }
    LeftBracket, // [
    RightBracket, // ]
    Semicolon,  // ;
    Colon,      // :
    Comma,      // ,
    Dot,        // .
    Arrow,      // ->

    // Special
    EOF,
    Error,
}

/// Represents a token in the Seen language
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Token {
    /// The canonical type of this token
    pub token_type: TokenType,

    /// The actual text as it appeared in the source
    pub lexeme: String,

    /// Location in the source code
    pub location: Location,

    /// The language the token was written in (e.g., "en", "ar")
    pub language: String,
}

impl Token {
    pub fn new(token_type: TokenType, lexeme: String, location: Location, language: String) -> Self {
        Self {
            token_type,
            lexeme,
            location,
            language,
        }
    }
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}({})", self.token_type, self.lexeme)
    }
}
