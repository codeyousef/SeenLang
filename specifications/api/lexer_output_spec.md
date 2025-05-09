# Lexer Output Specification

This document defines the API for the token stream output by the Seen language lexer.

## Overview

The lexer produces a stream of `Token` objects that capture:
1. The canonical token type (language-agnostic)
2. The original lexeme as it appeared in source code
3. Source location information
4. The language in which the token was written (based on the active language setting)

## Token Structure

Each token in the token stream has the following structure:

```rust
pub struct Token {
    // The canonical type of this token (language-agnostic)
    pub token_type: TokenType,
    
    // The actual text as it appeared in the source
    pub lexeme: String,
    
    // Location in the source code
    pub location: Location,
    
    // The language the token was written in (e.g., "en", "ar")
    pub language: String,
}
```

## TokenType Enumeration

The `TokenType` enum defines all possible token categories in a language-agnostic way:

```rust
pub enum TokenType {
    // Keywords (these map to different textual representations in different languages)
    Val,    // val (EN) / ثابت (AR)
    Var,    // var (EN) / متغير (AR)
    Func,   // func (EN) / دالة (AR)
    If,     // if (EN) / إذا (AR)
    Else,   // else (EN) / وإلا (AR)
    While,  // while (EN) / طالما (AR)
    For,    // for (EN) / لكل (AR)
    Return, // return (EN) / إرجاع (AR)
    True,   // true (EN) / صحيح (AR)
    False,  // false (EN) / خطأ (AR)
    Null,   // null (EN) / فارغ (AR)
    Println, // println (EN) / اطبع (AR)
    
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
```

## Source Location

Each token includes detailed source location information to support error reporting:

```rust
pub struct Location {
    pub start: Position,
    pub end: Position,
}

pub struct Position {
    pub line: usize,
    pub column: usize,
}
```

## Lexer API

The main lexer API includes:

```rust
// Create a new lexer for the given source string and keyword manager
pub fn new(source: &str, keyword_manager: &KeywordManager) -> Lexer;

// Get the next token from the source
pub fn next_token(&mut self) -> Result<Token, LexerError>;

// Get all tokens from the source
pub fn tokenize(&mut self) -> Result<Vec<Token>, LexerError>;
```

## Error Handling

Lexer errors are represented by the `LexerError` enum:

```rust
pub enum LexerError {
    UnexpectedCharacter(char, Position),
    UnterminatedString(Position),
    InvalidNumber(Position, String),
    InvalidIdentifier(Position, String),
}
```

## Bilingual Support

The lexer uses a `KeywordManager` to map between language-specific keywords and internal token types:

1. When lexing a source file, the active language is determined from the project configuration.
2. For each potential keyword, the lexer checks if it matches a keyword in the active language.
3. If it does, the lexer produces a token with the corresponding language-agnostic token type.
4. The original lexeme and its language are preserved for accurate error reporting.

## Usage Example

```rust
// Load keyword configuration
let keyword_config = KeywordConfig::from_file("keywords.toml").unwrap();

// Create keyword manager with English as the active language
let keyword_manager = KeywordManager::new(keyword_config, "en".to_string()).unwrap();

// Create a lexer for some source code
let source = "func main() { println(\"Hello, World!\"); }";
let mut lexer = Lexer::new(source, &keyword_manager);

// Tokenize the source
let tokens = lexer.tokenize().unwrap();

// Process the tokens
for token in tokens {
    println!("{:?}: {}", token.token_type, token.lexeme);
}
```

This will produce language-agnostic tokens that can be fed into the parser, regardless of whether the source code was written with English or Arabic keywords.
