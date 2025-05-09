use std::iter::Peekable;
use std::str::Chars;
use unicode_xid::UnicodeXID;

use crate::token::{Token, TokenType, Position, Location};
use crate::keyword_config::KeywordManager;

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
    /// The source code being lexed
    source: &'a str,
    
    /// Character iterator over the source
    chars: Peekable<Chars<'a>>,
    
    /// Current position in the source
    position: Position,
    
    /// Current line being processed
    current_line: &'a str,
    
    /// Keyword manager for handling bilingual keywords
    keyword_manager: &'a KeywordManager,
    
    /// Has the lexer reached the end of input?
    is_at_end: bool,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str, keyword_manager: &'a KeywordManager) -> Self {
        // Split the source by line to make line tracking easier
        let first_line = source.lines().next().unwrap_or("");
        
        Self {
            source,
            chars: source.chars().peekable(),
            position: Position::new(1, 1), // 1-indexed line and column
            current_line: first_line,
            keyword_manager,
            is_at_end: source.is_empty(),
        }
    }
    
    /// Get the next token from the source
    pub fn next_token(&mut self) -> Result<Token, LexerError> {
        self.skip_whitespace();
        
        if self.is_at_end {
            return Ok(self.create_token(TokenType::EOF, ""));
        }
        
        let start_position = self.position;
        let c = self.advance();
        
        match c {
            // Delimiters
            '(' => Ok(self.create_token(TokenType::LeftParen, "(")),
            ')' => Ok(self.create_token(TokenType::RightParen, ")")),
            '{' => Ok(self.create_token(TokenType::LeftBrace, "{")),
            '}' => Ok(self.create_token(TokenType::RightBrace, "}")),
            '[' => Ok(self.create_token(TokenType::LeftBracket, "[")),
            ']' => Ok(self.create_token(TokenType::RightBracket, "]")),
            ';' => Ok(self.create_token(TokenType::Semicolon, ";")),
            ':' => Ok(self.create_token(TokenType::Colon, ":")),
            ',' => Ok(self.create_token(TokenType::Comma, ",")),
            '.' => Ok(self.create_token(TokenType::Dot, ".")),
            
            // Operators (single character)
            '+' => Ok(self.create_token(TokenType::Plus, "+")),
            '-' => {
                // Check for arrow "->"
                if self.match_char('>') {
                    Ok(self.create_token(TokenType::Arrow, "->"))
                } else {
                    Ok(self.create_token(TokenType::Minus, "-"))
                }
            },
            '*' => Ok(self.create_token(TokenType::Multiply, "*")),
            '/' => {
                // Handle comments
                if self.match_char('/') {
                    // Skip until end of line
                    while self.peek() != '\n' && !self.is_at_end {
                        self.advance();
                    }
                    // Recursively get the next token after the comment
                    self.next_token()
                } else {
                    Ok(self.create_token(TokenType::Divide, "/"))
                }
            },
            '%' => Ok(self.create_token(TokenType::Modulo, "%")),
            
            // Operators (potentially two characters)
            '=' => {
                if self.match_char('=') {
                    Ok(self.create_token(TokenType::Equal, "=="))
                } else {
                    Ok(self.create_token(TokenType::Assign, "="))
                }
            },
            '!' => {
                if self.match_char('=') {
                    Ok(self.create_token(TokenType::NotEqual, "!="))
                } else {
                    Ok(self.create_token(TokenType::Not, "!"))
                }
            },
            '<' => {
                if self.match_char('=') {
                    Ok(self.create_token(TokenType::LessEqual, "<="))
                } else {
                    Ok(self.create_token(TokenType::LessThan, "<"))
                }
            },
            '>' => {
                if self.match_char('=') {
                    Ok(self.create_token(TokenType::GreaterEqual, ">="))
                } else {
                    Ok(self.create_token(TokenType::GreaterThan, ">"))
                }
            },
            
            // Logical operators
            '&' => {
                if self.match_char('&') {
                    Ok(self.create_token(TokenType::And, "&&"))
                } else {
                    Err(LexerError::UnexpectedCharacter('&', start_position))
                }
            },
            '|' => {
                if self.match_char('|') {
                    Ok(self.create_token(TokenType::Or, "||"))
                } else {
                    Err(LexerError::UnexpectedCharacter('|', start_position))
                }
            },
            
            // String literals
            '"' => self.string(),
            
            // Number literals
            c if c.is_ascii_digit() => self.number(),
            
            // Identifiers and keywords
            c if is_identifier_start(c) => self.identifier_or_keyword(),
            
            // Arabic comments
            '#' => {
                if self.match_char('#') {
                    // Skip until end of line
                    while self.peek() != '\n' && !self.is_at_end {
                        self.advance();
                    }
                    // Recursively get the next token after the comment
                    self.next_token()
                } else {
                    Err(LexerError::UnexpectedCharacter('#', start_position))
                }
            },
            
            // Multi-line comments
            c if c == '/' && self.peek() == '*' => {
                self.advance(); // Consume '*'
                let mut level = 1;
                
                while level > 0 && !self.is_at_end {
                    let c = self.advance();
                    if c == '*' && self.peek() == '/' {
                        self.advance(); // Consume '/'
                        level -= 1;
                    } else if c == '/' && self.peek() == '*' {
                        self.advance(); // Consume '*'
                        level += 1;
                    }
                }
                
                if level > 0 {
                    Err(LexerError::UnterminatedString(start_position))
                } else {
                    // Recursively get the next token after the comment
                    self.next_token()
                }
            },
            
            // Unexpected characters
            _ => Err(LexerError::UnexpectedCharacter(c, start_position)),
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
    
    /// Create a token with the current location
    fn create_token(&self, token_type: TokenType, lexeme: &str) -> Token {
        let start = self.position;
        // Adjust end position based on lexeme length
        let end = Position::new(
            start.line,
            start.column + lexeme.chars().count() - 1,
        );
        
        Token::new(
            token_type,
            lexeme.to_string(),
            Location::new(start, end),
            self.keyword_manager.get_active_language().to_string(),
        )
    }
    
    /// Advance to the next character
    fn advance(&mut self) -> char {
        if let Some(c) = self.chars.next() {
            // Update position
            if c == '\n' {
                self.position.line += 1;
                self.position.column = 1;
                
                // Get the next line
                let line_index = self.position.line - 1; // 0-indexed
                self.current_line = self.source.lines().nth(line_index).unwrap_or("");
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
    
    /// Process a string literal
    fn string(&mut self) -> Result<Token, LexerError> {
        let start_position = self.position;
        let mut value = String::new();
        
        while self.peek() != '"' && !self.is_at_end {
            let c = self.advance();
            value.push(c);
        }
        
        if self.is_at_end {
            return Err(LexerError::UnterminatedString(start_position));
        }
        
        // Consume the closing "
        self.advance();
        
        // Create the token with the string value (not including the quotes)
        let token = self.create_token(TokenType::StringLiteral, &value);
        Ok(token)
    }
    
    /// Process a number literal
    fn number(&mut self) -> Result<Token, LexerError> {
        let start_position = self.position;
        let mut value = String::new();
        
        // Get integer part
        while self.peek().is_ascii_digit() && !self.is_at_end {
            value.push(self.advance());
        }
        
        // Look for fractional part
        let mut is_float = false;
        if self.peek() == '.' && self.chars.clone().nth(1).map_or(false, |c| c.is_ascii_digit()) {
            is_float = true;
            value.push(self.advance()); // Consume the '.'
            
            // Get fractional part
            while self.peek().is_ascii_digit() && !self.is_at_end {
                value.push(self.advance());
            }
        }
        
        // Validate the number
        if is_float {
            if value.parse::<f64>().is_err() {
                return Err(LexerError::InvalidNumber(start_position, value));
            }
            Ok(self.create_token(TokenType::FloatLiteral, &value))
        } else {
            if value.parse::<i64>().is_err() {
                return Err(LexerError::InvalidNumber(start_position, value));
            }
            Ok(self.create_token(TokenType::IntLiteral, &value))
        }
    }
    
    /// Process an identifier or keyword
    fn identifier_or_keyword(&mut self) -> Result<Token, LexerError> {
        let start_position = self.position;
        let mut value = String::new();
        
        // First character is already consumed and validated
        value.push(self.chars.clone().nth(0).unwrap_or('\0'));
        
        // Get the rest of the identifier
        while !self.is_at_end && is_identifier_continue(self.peek()) {
            value.push(self.advance());
        }
        
        // Check if this is a keyword in the active language
        let token_type = self.keyword_manager
            .get_token_type(&value)
            .unwrap_or(TokenType::Identifier);
        
        Ok(self.create_token(token_type, &value))
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
