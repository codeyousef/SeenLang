use std::iter::Peekable;
use std::str::Chars;
use unicode_xid::UnicodeXID;

use crate::keyword_config::KeywordManager;
use crate::token::{Location, Position, Token, TokenType};

/// Lexical analysis errors
#[derive(Debug, thiserror::Error)]
pub enum LexerError {
    #[error("Unexpected character: '{0}' at {1}")]
    UnexpectedCharacter(char, Position),

    #[error("Unterminated string literal at {0}")]
    UnterminatedString(Position),

    #[error("Invalid number literal at {0}: {1}")]
    InvalidNumber(Position, String),

    #[error("Invalid identifier at {0}: {1}")]
    InvalidIdentifier(Position, String),
}

/// The lexer for the Seen programming language
pub struct Lexer<'a> {
    /// Character iterator over the source
    chars: Peekable<Chars<'a>>,

    /// Current position in the source, pointing to the START of the next character to be processed.
    position: Position,

    /// Keyword manager for handling bilingual keywords
    keyword_manager: &'a KeywordManager,

    /// Has the lexer reached the end of input?
    is_at_end: bool,

    /// Language of the source code
    language: String,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str, keyword_manager: &'a KeywordManager, language: String) -> Self {
        Self {
            chars: source.chars().peekable(),
            position: Position::new(1, 1), // 1-indexed line and column
            keyword_manager,
            is_at_end: source.is_empty(),
            language,
        }
    }

    /// Get the next token from the source
    pub fn next_token(&mut self) -> Result<Token, LexerError> {
        self.skip_whitespace();

        if self.is_at_end {
            return Ok(self.create_token(TokenType::EOF, "", self.position));
        }

        let token_start_pos = self.position; // Mark position before advancing
        let c = self.advance(); // Consumes char at token_start_pos, self.position now points to char *after* c

        match c {
            // Delimiters
            '(' => Ok(self.create_token(TokenType::LeftParen, "(", token_start_pos)),
            ')' => Ok(self.create_token(TokenType::RightParen, ")", token_start_pos)),
            '{' => Ok(self.create_token(TokenType::LeftBrace, "{", token_start_pos)),
            '}' => Ok(self.create_token(TokenType::RightBrace, "}", token_start_pos)),
            '[' => Ok(self.create_token(TokenType::LeftBracket, "[", token_start_pos)),
            ']' => Ok(self.create_token(TokenType::RightBracket, "]", token_start_pos)),
            ';' => Ok(self.create_token(TokenType::Semicolon, ";", token_start_pos)),
            ':' => Ok(self.create_token(TokenType::Colon, ":", token_start_pos)),
            ',' => Ok(self.create_token(TokenType::Comma, ",", token_start_pos)),
            '.' => Ok(self.create_token(TokenType::Dot, ".", token_start_pos)),

            // Operators (single character)
            '+' => Ok(self.create_token(TokenType::Plus, "+", token_start_pos)),
            '-' => {
                if self.match_char('>') {
                    Ok(self.create_token(TokenType::Arrow, "->", token_start_pos))
                } else {
                    Ok(self.create_token(TokenType::Minus, "-", token_start_pos))
                }
            }
            '*' => Ok(self.create_token(TokenType::Multiply, "*", token_start_pos)),
            '/' => {
                if self.match_char('/') {
                    while self.peek() != '\n' && !self.is_at_end {
                        self.advance();
                    }
                    self.next_token()
                } else {
                    Ok(self.create_token(TokenType::Divide, "/", token_start_pos))
                }
            }
            '%' => Ok(self.create_token(TokenType::Modulo, "%", token_start_pos)),

            // Operators (potentially two characters)
            '=' => {
                if self.match_char('=') {
                    Ok(self.create_token(TokenType::Equal, "==", token_start_pos))
                } else {
                    Ok(self.create_token(TokenType::Assign, "=", token_start_pos))
                }
            }
            '!' => match self.peek() {
                '=' => {
                    self.advance(); // Consume '='
                    Ok(self.create_token(TokenType::NotEqual, "!=", token_start_pos))
                }
                _ => Ok(self.create_token(TokenType::Not, "!", token_start_pos)),
            },
            '<' => match self.peek() {
                '=' => {
                    self.advance(); // Consume '='
                    Ok(self.create_token(TokenType::LessEqual, "<=", token_start_pos))
                }
                _ => Ok(self.create_token(TokenType::LessThan, "<", token_start_pos)),
            },
            '>' => match self.peek() {
                '=' => {
                    self.advance(); // Consume '='
                    Ok(self.create_token(TokenType::GreaterEqual, ">=", token_start_pos))
                }
                _ => Ok(self.create_token(TokenType::GreaterThan, ">", token_start_pos)),
            },

            // String literals
            '"' => self.string(token_start_pos),

            // Numbers
            c if c.is_ascii_digit() => self.number(c, token_start_pos),

            // Identifiers and keywords
            c if is_identifier_start(c) => self.identifier_or_keyword(c, token_start_pos),

            _ => Err(LexerError::UnexpectedCharacter(c, token_start_pos)),
        }
    }

    /// Get all tokens from the source
    pub fn tokenize(&mut self) -> Result<Vec<Token>, LexerError> {
        let mut tokens = Vec::new();

        loop {
            let token = self.next_token()?;
            let is_eof = token.token_type == TokenType::EOF;

            tokens.push(token);

            if is_eof {
                break;
            }
        }

        Ok(tokens)
    }

    // Helper methods

    /// Create a token with a specific start position
    fn create_token(&self, token_type: TokenType, lexeme: &str, start_pos: Position) -> Token {
        let end_pos = self.position; // Current lexer position is the end of this token

        Token {
            token_type,
            lexeme: lexeme.to_string(),
            location: Location {
                start: start_pos,
                end: end_pos,
            },
            language: self.language.clone(),
        }
    }

    /// Advance to the next character
    fn advance(&mut self) -> char {
        if let Some(c) = self.chars.next() {
            // Update position
            if c == '\n' {
                self.position.line += 1;
                self.position.column = 1;
            } else {
                self.position.column += 1;
            }

            // Check if we've reached the end
            self.is_at_end = self.chars.peek().is_none();

            c
        } else {
            self.is_at_end = true;
            '\0' // Null character at end of input
        }
    }

    /// Peek at the next character without advancing
    fn peek(&mut self) -> char {
        self.chars.peek().copied().unwrap_or('\0')
    }

    /// Check if the next character matches expected, and if so, consume it
    fn match_char(&mut self, expected: char) -> bool {
        if self.is_at_end || self.peek() != expected {
            false
        } else {
            self.advance();
            true
        }
    }

    /// Skip whitespace characters
    fn skip_whitespace(&mut self) {
        while !self.is_at_end {
            match self.peek() {
                ' ' | '\t' | '\r' | '\n' => {
                    self.advance();
                }
                _ => break,
            }
        }
    }

    /// Process a string literal, `start_pos` is the position of the opening quote.
    fn string(&mut self, start_pos: Position) -> Result<Token, LexerError> {
        let mut value = String::new();

        while self.peek() != '"' && !self.is_at_end {
            // Handle escape sequences if necessary (not implemented in this snippet)
            value.push(self.advance());
        }

        if self.is_at_end { // Unterminated string
            return Err(LexerError::UnterminatedString(start_pos));
        }

        self.advance(); // Consume the closing "

        Ok(self.create_token(TokenType::StringLiteral, &value, start_pos))
    }

    /// Process a number literal, `first_digit` is the initial digit char, `start_pos` is its position.
    fn number(&mut self, first_digit: char, start_pos: Position) -> Result<Token, LexerError> {
        let mut value = String::new();
        value.push(first_digit);

        // Get integer part
        while self.peek().is_ascii_digit() && !self.is_at_end {
            value.push(self.advance());
        }

        let mut is_float = false;
        if self.peek() == '.' {
            // Create a clone of the char iterator to peek ahead for a digit after '.'
            let mut next_chars = self.chars.clone();
            if next_chars.nth(1).map_or(false, |ch| ch.is_ascii_digit()) { // Check char after '.'
                is_float = true;
                value.push(self.advance()); // Consume the '.'

                while self.peek().is_ascii_digit() && !self.is_at_end {
                    value.push(self.advance());
                }
            }
        }

        if is_float {
            if value.parse::<f64>().is_err() {
                return Err(LexerError::InvalidNumber(start_pos, value));
            }
            Ok(self.create_token(TokenType::FloatLiteral, &value, start_pos))
        } else {
            if value.parse::<i64>().is_err() {
                return Err(LexerError::InvalidNumber(start_pos, value));
            }
            Ok(self.create_token(TokenType::IntLiteral, &value, start_pos))
        }
    }

    /// Process an identifier or keyword, `first_char` is its first char, `start_pos` is its position.
    fn identifier_or_keyword(&mut self, first_char: char, start_pos: Position) -> Result<Token, LexerError> {
        let mut value = String::new();
        value.push(first_char);

        while !self.is_at_end && is_identifier_continue(self.peek()) {
            value.push(self.advance());
        }

        let token_type = self.keyword_manager
            .get_token_type(&value)
            .unwrap_or(TokenType::Identifier);

        Ok(self.create_token(token_type, &value, start_pos))
    }
}

/// Check if a character can start an identifier
fn is_identifier_start(c: char) -> bool {
    // Allow ASCII letters, underscore, and Arabic characters
    UnicodeXID::is_xid_start(c)
}

/// Check if a character can continue an identifier
fn is_identifier_continue(c: char) -> bool {
    // Allow ASCII letters, digits, underscore, and Arabic characters
    UnicodeXID::is_xid_continue(c)
}
