//! High-performance lexer implementation for the Seen language
//! Target: >10M tokens/second

use crate::{Token, TokenType, LanguageConfig, TokenUtils};
use seen_common::{Position, Span, SeenResult, SeenError, Diagnostics};
use unicode_xid::UnicodeXID;
// Performance optimization imports (will be used in optimized implementation)
// use memchr::{memchr, memchr2};
// use smallvec::SmallVec;

/// High-performance lexer for the Seen language
pub struct Lexer<'a> {
    input: &'a str,
    input_bytes: &'a [u8],
    position: usize,
    current_pos: Position,
    file_id: u32,
    language_config: &'a LanguageConfig,
    diagnostics: Diagnostics,
}

impl<'a> Lexer<'a> {
    /// Create a new lexer instance
    pub fn new(input: &'a str, file_id: u32, language_config: &'a LanguageConfig) -> Self {
        Self {
            input,
            input_bytes: input.as_bytes(),
            position: 0,
            current_pos: Position::start(),
            file_id,
            language_config,
            diagnostics: Diagnostics::new(),
        }
    }
    
    /// Get the collected diagnostics
    pub fn diagnostics(&self) -> &Diagnostics {
        &self.diagnostics
    }
    
    /// Tokenize the entire input into a vector of tokens
    pub fn tokenize(&mut self) -> SeenResult<Vec<Token>> {
        let mut tokens = Vec::new();
        
        while !self.is_at_end() {
            if let Some(token) = self.next_token()? {
                // Skip whitespace tokens in the final output (but preserve them for formatters)
                if !matches!(token.value, TokenType::Whitespace) {
                    tokens.push(token);
                }
            }
        }
        
        // Add EOF token
        let eof_span = Span::single(self.current_pos, self.file_id);
        tokens.push(TokenUtils::new(TokenType::EndOfFile, eof_span));
        
        Ok(tokens)
    }
    
    /// Get the next token from the input
    pub fn next_token(&mut self) -> SeenResult<Option<Token>> {
        self.skip_whitespace_and_comments()?;
        
        if self.is_at_end() {
            return Ok(None);
        }
        
        let start_pos = self.current_pos;
        let _start_offset = self.position;
        
        let token_type = match self.current_char() {
            // Single-character tokens
            '(' => { self.advance(); TokenType::LeftParen }
            ')' => { self.advance(); TokenType::RightParen }
            '{' => { self.advance(); TokenType::LeftBrace }
            '}' => { self.advance(); TokenType::RightBrace }
            '[' => { self.advance(); TokenType::LeftBracket }
            ']' => { self.advance(); TokenType::RightBracket }
            ';' => { self.advance(); TokenType::Semicolon }
            ',' => { self.advance(); TokenType::Comma }
            '?' => { self.advance(); TokenType::Question }
            '~' => { self.advance(); TokenType::BitwiseNot }
            
            // Multi-character operators
            '+' => self.scan_plus_operators(),
            '-' => self.scan_minus_operators(),
            '*' => self.scan_multiply_operators(),
            '/' => self.scan_divide_operators(),
            '%' => self.scan_modulo_operators(),
            '=' => self.scan_equal_operators(),
            '!' => self.scan_not_operators(),
            '<' => self.scan_less_operators(),
            '>' => self.scan_greater_operators(),
            '&' => self.scan_and_operators(),
            '|' => self.scan_or_operators(),
            '^' => self.scan_xor_operators(),
            ':' => self.scan_colon_operators(),
            '.' => self.scan_dot_operators(),
            
            // String literals
            '"' => self.scan_string_literal()?,
            '\'' => self.scan_char_literal()?,
            
            // Numeric literals
            c if c.is_ascii_digit() => self.scan_number()?,
            
            // Identifiers and keywords
            c if c.is_xid_start() => self.scan_identifier_or_keyword(),
            
            // Handle unexpected characters
            c => {
                self.advance();
                let error_msg = format!("Unexpected character: '{}'", c);
                self.diagnostics.error(&error_msg, Span::single(start_pos, self.file_id));
                TokenType::Error(error_msg)
            }
        };
        
        let end_pos = self.current_pos;
        let span = Span::new(start_pos, end_pos, self.file_id);
        
        Ok(Some(TokenUtils::new(token_type, span)))
    }
    
    /// Skip whitespace and comments
    fn skip_whitespace_and_comments(&mut self) -> SeenResult<()> {
        while !self.is_at_end() {
            match self.current_char() {
                ' ' | '\t' | '\r' => {
                    self.advance();
                }
                '\n' => {
                    self.advance_line();
                }
                '/' if self.peek_char() == Some('/') => {
                    self.skip_line_comment();
                }
                '/' if self.peek_char() == Some('*') => {
                    self.skip_block_comment()?;
                }
                _ => break,
            }
        }
        Ok(())
    }
    
    /// Skip a line comment
    fn skip_line_comment(&mut self) {
        while !self.is_at_end() && self.current_char() != '\n' {
            self.advance();
        }
    }
    
    /// Skip a block comment
    fn skip_block_comment(&mut self) -> SeenResult<()> {
        self.advance(); // Skip '/'
        self.advance(); // Skip '*'
        
        let mut nesting_level = 1;
        
        while !self.is_at_end() && nesting_level > 0 {
            if self.current_char() == '/' && self.peek_char() == Some('*') {
                self.advance();
                self.advance();
                nesting_level += 1;
            } else if self.current_char() == '*' && self.peek_char() == Some('/') {
                self.advance();
                self.advance();
                nesting_level -= 1;
            } else if self.current_char() == '\n' {
                self.advance_line();
            } else {
                self.advance();
            }
        }
        
        if nesting_level > 0 {
            // Error recovery: continue despite unterminated comment
            self.diagnostics.error("Unterminated block comment", Span::single(self.current_pos, self.file_id));
        }
        
        Ok(())
    }
    
    /// Scan identifier or keyword
    fn scan_identifier_or_keyword(&mut self) -> TokenType {
        let start = self.position;
        
        while !self.is_at_end() && (self.current_char().is_xid_continue()) {
            self.advance();
        }
        
        let identifier = &self.input[start..self.position];
        
        // Check if it's a keyword in the current language
        if let Some(keyword_token) = self.language_config.keyword_to_token(identifier) {
            keyword_token
        } else {
            // Check for boolean literals (language-independent)
            match identifier {
                "true" => TokenType::BooleanLiteral(true),
                "false" => TokenType::BooleanLiteral(false),
                _ => TokenType::Identifier(identifier.to_string()),
            }
        }
    }
    
    /// Scan a string literal
    fn scan_string_literal(&mut self) -> SeenResult<TokenType> {
        self.advance(); // Skip opening quote
        
        let mut value = String::new();
        
        while !self.is_at_end() && self.current_char() != '"' {
            if self.current_char() == '\\' {
                self.advance();
                if self.is_at_end() {
                    // Error recovery: treat as escaped character and continue
                    self.diagnostics.error("Unterminated string literal in escape sequence", Span::single(self.current_pos, self.file_id));
                    break;
                }
                
                match self.current_char() {
                    'n' => value.push('\n'),
                    't' => value.push('\t'),
                    'r' => value.push('\r'),
                    '\\' => value.push('\\'),
                    '"' => value.push('"'),
                    '\'' => value.push('\''),
                    '0' => value.push('\0'),
                    c => {
                        self.diagnostics.warning(
                            &format!("Unknown escape sequence: \\{}", c),
                            Span::single(self.current_pos, self.file_id)
                        );
                        value.push(c);
                    }
                }
            } else if self.current_char() == '\n' {
                self.advance_line();
                value.push('\n');
            } else {
                value.push(self.current_char());
            }
            self.advance();
        }
        
        if self.is_at_end() {
            // Error recovery: treat as unterminated string but continue
            self.diagnostics.error("Unterminated string literal", Span::single(self.current_pos, self.file_id));
            return Ok(TokenType::StringLiteral(value));
        }
        
        self.advance(); // Skip closing quote
        Ok(TokenType::StringLiteral(value))
    }
    
    /// Scan a character literal
    fn scan_char_literal(&mut self) -> SeenResult<TokenType> {
        self.advance(); // Skip opening quote
        
        if self.is_at_end() {
            // Error recovery: return null character for unterminated literal
            self.diagnostics.error("Unterminated character literal", Span::single(self.current_pos, self.file_id));
            return Ok(TokenType::CharLiteral('\0'));
        }
        
        let mut ch = self.current_char();
        self.advance();
        
        if ch == '\\' {
            if self.is_at_end() {
                // Error recovery: use the backslash character
                self.diagnostics.error("Unterminated character literal in escape sequence", Span::single(self.current_pos, self.file_id));
                ch = '\\';
                return Ok(TokenType::CharLiteral(ch));
            }
            
            ch = match self.current_char() {
                'n' => '\n',
                't' => '\t',
                'r' => '\r',
                '\\' => '\\',
                '"' => '"',
                '\'' => '\'',
                '0' => '\0',
                c => c,
            };
            self.advance();
        }
        
        if self.is_at_end() || self.current_char() != '\'' {
            // Error recovery: treat as valid character but report error
            self.diagnostics.error("Unterminated character literal", Span::single(self.current_pos, self.file_id));
            return Ok(TokenType::CharLiteral(ch));
        }
        
        self.advance(); // Skip closing quote
        Ok(TokenType::CharLiteral(ch))
    }
    
    /// Scan a numeric literal
    fn scan_number(&mut self) -> SeenResult<TokenType> {
        let start = self.position;
        
        // Scan integer part
        while !self.is_at_end() && self.current_char().is_ascii_digit() {
            self.advance();
        }
        
        // Check for decimal point
        if !self.is_at_end() && self.current_char() == '.' && 
           self.peek_char().map_or(false, |c| c.is_ascii_digit()) {
            self.advance(); // Skip '.'
            
            // Scan fractional part
            while !self.is_at_end() && self.current_char().is_ascii_digit() {
                self.advance();
            }
            
            let number_str = &self.input[start..self.position];
            // Validate it's a valid float but store as string
            number_str.parse::<f64>()
                .map_err(|_| SeenError::lex_error(format!("Invalid float literal: {}", number_str)))?;
            
            Ok(TokenType::FloatLiteral(number_str.to_string()))
        } else {
            let number_str = &self.input[start..self.position];
            let value = number_str.parse::<i64>()
                .map_err(|_| SeenError::lex_error(format!("Invalid integer literal: {}", number_str)))?;
            
            Ok(TokenType::IntegerLiteral(value))
        }
    }
    
    // Operator scanning methods
    fn scan_plus_operators(&mut self) -> TokenType {
        self.advance();
        if !self.is_at_end() && self.current_char() == '=' {
            self.advance();
            TokenType::PlusAssign
        } else {
            TokenType::Plus
        }
    }
    
    fn scan_minus_operators(&mut self) -> TokenType {
        self.advance();
        if !self.is_at_end() {
            match self.current_char() {
                '=' => { self.advance(); TokenType::MinusAssign }
                '>' => { self.advance(); TokenType::Arrow }
                _ => TokenType::Minus
            }
        } else {
            TokenType::Minus
        }
    }
    
    fn scan_multiply_operators(&mut self) -> TokenType {
        self.advance();
        if !self.is_at_end() && self.current_char() == '=' {
            self.advance();
            TokenType::MultiplyAssign
        } else {
            TokenType::Multiply
        }
    }
    
    fn scan_divide_operators(&mut self) -> TokenType {
        self.advance();
        if !self.is_at_end() && self.current_char() == '=' {
            self.advance();
            TokenType::DivideAssign
        } else {
            TokenType::Divide
        }
    }
    
    fn scan_modulo_operators(&mut self) -> TokenType {
        self.advance();
        if !self.is_at_end() && self.current_char() == '=' {
            self.advance();
            TokenType::ModuloAssign
        } else {
            TokenType::Modulo
        }
    }
    
    fn scan_equal_operators(&mut self) -> TokenType {
        self.advance();
        if !self.is_at_end() {
            match self.current_char() {
                '=' => { self.advance(); TokenType::Equal }
                '>' => { self.advance(); TokenType::FatArrow }
                _ => TokenType::Assign
            }
        } else {
            TokenType::Assign
        }
    }
    
    fn scan_not_operators(&mut self) -> TokenType {
        self.advance();
        if !self.is_at_end() && self.current_char() == '=' {
            self.advance();
            TokenType::NotEqual
        } else {
            TokenType::LogicalNot
        }
    }
    
    fn scan_less_operators(&mut self) -> TokenType {
        self.advance();
        if !self.is_at_end() {
            match self.current_char() {
                '=' => { self.advance(); TokenType::LessEqual }
                '<' => { self.advance(); TokenType::LeftShift }
                _ => TokenType::Less
            }
        } else {
            TokenType::Less
        }
    }
    
    fn scan_greater_operators(&mut self) -> TokenType {
        self.advance();
        if !self.is_at_end() {
            match self.current_char() {
                '=' => { self.advance(); TokenType::GreaterEqual }
                '>' => { self.advance(); TokenType::RightShift }
                _ => TokenType::Greater
            }
        } else {
            TokenType::Greater
        }
    }
    
    fn scan_and_operators(&mut self) -> TokenType {
        self.advance();
        if !self.is_at_end() && self.current_char() == '&' {
            self.advance();
            TokenType::LogicalAnd
        } else {
            TokenType::BitwiseAnd
        }
    }
    
    fn scan_or_operators(&mut self) -> TokenType {
        self.advance();
        if !self.is_at_end() && self.current_char() == '|' {
            self.advance();
            TokenType::LogicalOr
        } else {
            TokenType::BitwiseOr
        }
    }
    
    fn scan_xor_operators(&mut self) -> TokenType {
        self.advance();
        TokenType::BitwiseXor
    }
    
    fn scan_colon_operators(&mut self) -> TokenType {
        self.advance();
        if !self.is_at_end() && self.current_char() == ':' {
            self.advance();
            TokenType::DoubleColon
        } else {
            TokenType::Colon
        }
    }
    
    fn scan_dot_operators(&mut self) -> TokenType {
        self.advance();
        if !self.is_at_end() && self.current_char() == '.' {
            self.advance();
            if !self.is_at_end() && self.current_char() == '.' {
                self.advance();
                TokenType::TripleDot
            } else {
                TokenType::DoubleDot
            }
        } else {
            TokenType::Dot
        }
    }
    
    // Utility methods - optimized for performance
    fn current_char(&self) -> char {
        if self.position >= self.input_bytes.len() {
            return '\0';
        }
        
        // Fast path for ASCII characters
        let byte = self.input_bytes[self.position];
        if byte < 128 {
            return byte as char;
        }
        
        // Slow path for Unicode characters
        let remaining = &self.input[self.position..];
        remaining.chars().next().unwrap_or('\0')
    }
    
    fn peek_char(&self) -> Option<char> {
        if self.position + 1 >= self.input_bytes.len() {
            return None;
        }
        
        // Fast path for ASCII characters
        let byte = self.input_bytes[self.position + 1];
        if byte < 128 {
            return Some(byte as char);
        }
        
        // Slow path for Unicode characters - need to find the next char boundary
        let remaining = &self.input[self.position..];
        let mut chars = remaining.chars();
        chars.next(); // Skip current char
        chars.next()
    }
    
    fn advance(&mut self) {
        if !self.is_at_end() {
            let byte = self.input_bytes[self.position];
            if byte < 128 {
                // Fast path for ASCII
                self.position += 1;
            } else {
                // Slow path for Unicode - find next char boundary
                let remaining = &self.input[self.position..];
                if let Some(ch) = remaining.chars().next() {
                    self.position += ch.len_utf8();
                } else {
                    self.position += 1;
                }
            }
            self.current_pos.column += 1;
            self.current_pos.offset = self.position as u32;
        }
    }
    
    fn advance_line(&mut self) {
        if !self.is_at_end() {
            let byte = self.input_bytes[self.position];
            if byte < 128 {
                // Fast path for ASCII
                self.position += 1;
            } else {
                // Slow path for Unicode
                let remaining = &self.input[self.position..];
                if let Some(ch) = remaining.chars().next() {
                    self.position += ch.len_utf8();
                } else {
                    self.position += 1;
                }
            }
            self.current_pos.line += 1;
            self.current_pos.column = 1;
            self.current_pos.offset = self.position as u32;
        }
    }
    
    fn is_at_end(&self) -> bool {
        self.position >= self.input_bytes.len()
    }
}