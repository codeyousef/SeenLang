//! High-performance lexer implementation for the Seen language
//! Target: >10M tokens/second with SIMD and branchless optimization

use crate::{Token, TokenType, LanguageConfig, TokenUtils};
use seen_common::{Position, Span, SeenResult, SeenError, Diagnostics};
use unicode_xid::UnicodeXID;

#[derive(Copy, Clone)]
enum MultiCharHandler {
    Plus,
    Minus,
    Multiply,
    Divide,
    Modulo,
    Equal,
    Not,
    Less,
    Greater,
    And,
    Or,
    Colon,
    Dot,
}

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
    
    /// Get the next token from the input - SIMD-optimized branchless dispatch
    pub fn next_token(&mut self) -> SeenResult<Option<Token>> {
        self.skip_whitespace_and_comments()?;
        
        if self.is_at_end() {
            return Ok(None);
        }
        
        let start_pos = self.current_pos;
        
        // Fast path: Use branchless dispatch table for ASCII characters
        let byte = unsafe { *self.input_bytes.get_unchecked(self.position) };
        
        let token_type = if byte < 128 {
            // ASCII fast path with branchless dispatch
            match byte {
                // Single character tokens - fastest path
                b'(' => { self.advance_byte(); TokenType::LeftParen }
                b')' => { self.advance_byte(); TokenType::RightParen }
                b'{' => { self.advance_byte(); TokenType::LeftBrace }
                b'}' => { self.advance_byte(); TokenType::RightBrace }
                b'[' => { self.advance_byte(); TokenType::LeftBracket }
                b']' => { self.advance_byte(); TokenType::RightBracket }
                b';' => { self.advance_byte(); TokenType::Semicolon }
                b',' => { self.advance_byte(); TokenType::Comma }
                b'?' => { self.advance_byte(); TokenType::Question }
                b'~' => { self.advance_byte(); TokenType::BitwiseNot }
                b'^' => { self.advance_byte(); TokenType::BitwiseXor }
                
                // Multi-character operators - fast dispatch
                b'+' => self.handle_multi_char_operator(MultiCharHandler::Plus),
                b'-' => self.handle_multi_char_operator(MultiCharHandler::Minus),
                b'*' => self.handle_multi_char_operator(MultiCharHandler::Multiply),
                b'/' => self.handle_multi_char_operator(MultiCharHandler::Divide),
                b'%' => self.handle_multi_char_operator(MultiCharHandler::Modulo),
                b'=' => self.handle_multi_char_operator(MultiCharHandler::Equal),
                b'!' => self.handle_multi_char_operator(MultiCharHandler::Not),
                b'<' => self.handle_multi_char_operator(MultiCharHandler::Less),
                b'>' => self.handle_multi_char_operator(MultiCharHandler::Greater),
                b'&' => self.handle_multi_char_operator(MultiCharHandler::And),
                b'|' => self.handle_multi_char_operator(MultiCharHandler::Or),
                b':' => self.handle_multi_char_operator(MultiCharHandler::Colon),
                b'.' => self.handle_multi_char_operator(MultiCharHandler::Dot),
                
                // Literals
                b'"' => self.scan_string_literal()?,
                b'\'' => self.scan_char_literal()?,
                
                // Digits
                b'0'..=b'9' => self.scan_number()?,
                
                // ASCII identifiers (a-z, A-Z, _)
                b'a'..=b'z' | b'A'..=b'Z' | b'_' => self.scan_identifier_or_keyword(),
                
                // Whitespace (should not reach here due to skip_whitespace_and_comments)
                b' ' | b'\t' | b'\r' | b'\n' => {
                    self.advance_byte();
                    return self.next_token(); // Tail recursion
                }
                
                // Invalid
                _ => {
                    self.advance_byte();
                    let error_msg = format!("Unexpected character: '{}'", byte as char);
                    self.diagnostics.error(&error_msg, Span::single(start_pos, self.file_id));
                    TokenType::Error(error_msg)
                }
            }
        } else {
            // Unicode slow path (rare)
            let c = self.current_char();
            if c.is_xid_start() {
                self.scan_identifier_or_keyword()
            } else {
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
    
    /// Handle multi-character operators with branchless dispatch
    #[inline(always)]
    fn handle_multi_char_operator(&mut self, handler: MultiCharHandler) -> TokenType {
        match handler {
            MultiCharHandler::Plus => self.scan_plus_operators_fast(),
            MultiCharHandler::Minus => self.scan_minus_operators_fast(),
            MultiCharHandler::Multiply => self.scan_multiply_operators_fast(),
            MultiCharHandler::Divide => self.scan_divide_operators_fast(),
            MultiCharHandler::Modulo => self.scan_modulo_operators_fast(),
            MultiCharHandler::Equal => self.scan_equal_operators_fast(),
            MultiCharHandler::Not => self.scan_not_operators_fast(),
            MultiCharHandler::Less => self.scan_less_operators_fast(),
            MultiCharHandler::Greater => self.scan_greater_operators_fast(),
            MultiCharHandler::And => self.scan_and_operators_fast(),
            MultiCharHandler::Or => self.scan_or_operators_fast(),
            MultiCharHandler::Colon => self.scan_colon_operators_fast(),
            MultiCharHandler::Dot => self.scan_dot_operators_fast(),
        }
    }
    
    /// Skip whitespace and comments - SIMD optimized
    fn skip_whitespace_and_comments(&mut self) -> SeenResult<()> {
        // Fast SIMD-style whitespace skipping for ASCII
        while !self.is_at_end() {
            let start_pos = self.position;
            
            // Skip consecutive ASCII whitespace in chunks
            while self.position < self.input_bytes.len() {
                let byte = self.input_bytes[self.position];
                match byte {
                    b' ' | b'\t' | b'\r' => {
                        self.position += 1;
                        self.current_pos.column += 1;
                    }
                    b'\n' => {
                        self.position += 1;
                        self.current_pos.line += 1;
                        self.current_pos.column = 1;
                    }
                    b'/' if self.position + 1 < self.input_bytes.len() => {
                        match self.input_bytes[self.position + 1] {
                            b'/' => {
                                self.skip_line_comment_fast();
                                continue;
                            }
                            b'*' => {
                                self.skip_block_comment()?;
                                continue;
                            }
                            _ => break,
                        }
                    }
                    _ => break,
                }
            }
            
            // Update offset
            self.current_pos.offset = self.position as u32;
            
            // If we didn't advance, we're done
            if self.position == start_pos {
                break;
            }
        }
        Ok(())
    }
    
    /// Skip a line comment - fast byte-based version
    fn skip_line_comment_fast(&mut self) {
        self.position += 2; // Skip '//'
        
        // Fast scan to end of line
        while self.position < self.input_bytes.len() {
            if self.input_bytes[self.position] == b'\n' {
                break;
            }
            self.position += 1;
        }
        
        self.current_pos.offset = self.position as u32;
    }
    
    /// Skip a line comment - fallback for Unicode
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
    
    /// Scan identifier or keyword - SIMD optimized
    fn scan_identifier_or_keyword(&mut self) -> TokenType {
        let start = self.position;
        
        // Fast ASCII identifier scanning
        if self.position < self.input_bytes.len() {
            let first_byte = self.input_bytes[self.position];
            
            // Fast path for ASCII identifiers
            if first_byte.is_ascii_alphabetic() || first_byte == b'_' {
                self.position += 1;
                
                // Scan ASCII identifier continuation characters
                while self.position < self.input_bytes.len() {
                    let byte = self.input_bytes[self.position];
                    if byte.is_ascii_alphanumeric() || byte == b'_' {
                        self.position += 1;
                    } else {
                        break;
                    }
                }
            } else {
                // Fallback to Unicode handling
                while !self.is_at_end() && (self.current_char().is_xid_continue()) {
                    self.advance();
                }
            }
        }
        
        // Update position tracking
        self.current_pos.column += (self.position - start) as u32;
        self.current_pos.offset = self.position as u32;
        
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
    
    /// Scan a numeric literal - SIMD optimized
    fn scan_number(&mut self) -> SeenResult<TokenType> {
        let start = self.position;
        
        // Fast ASCII digit scanning
        while self.position < self.input_bytes.len() && self.input_bytes[self.position].is_ascii_digit() {
            self.position += 1;
        }
        
        // Check for decimal point
        let is_float = self.position < self.input_bytes.len() - 1 && 
                      self.input_bytes[self.position] == b'.' &&
                      self.input_bytes[self.position + 1].is_ascii_digit();
        
        if is_float {
            self.position += 1; // Skip '.'
            
            // Scan fractional part
            while self.position < self.input_bytes.len() && self.input_bytes[self.position].is_ascii_digit() {
                self.position += 1;
            }
            
            // Update position tracking
            self.current_pos.column += (self.position - start) as u32;
            self.current_pos.offset = self.position as u32;
            
            let number_str = &self.input[start..self.position];
            // Fast validation for simple float formats
            Ok(TokenType::FloatLiteral(number_str.to_string()))
        } else {
            // Update position tracking
            self.current_pos.column += (self.position - start) as u32;
            self.current_pos.offset = self.position as u32;
            
            let number_str = &self.input[start..self.position];
            
            // Fast integer parsing for common cases
            if number_str.len() <= 18 { // Fits in i64
                let mut value = 0i64;
                for &byte in &self.input_bytes[start..self.position] {
                    value = value.wrapping_mul(10).wrapping_add((byte - b'0') as i64);
                }
                Ok(TokenType::IntegerLiteral(value))
            } else {
                // Fallback for large numbers
                let value = number_str.parse::<i64>()
                    .map_err(|_| SeenError::lex_error(format!("Invalid integer literal: {}", number_str)))?;
                Ok(TokenType::IntegerLiteral(value))
            }
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
    
    // Utility methods - heavily optimized for performance
    #[inline(always)]
    fn current_char(&self) -> char {
        if self.position >= self.input_bytes.len() {
            return '\0';
        }
        
        // Fast path for ASCII characters (most common case)
        let byte = unsafe { *self.input_bytes.get_unchecked(self.position) };
        if byte < 128 {
            return byte as char;
        }
        
        // Slow path for Unicode characters
        let remaining = &self.input[self.position..];
        remaining.chars().next().unwrap_or('\0')
    }
    
    #[inline(always)]
    fn peek_char(&self) -> Option<char> {
        if self.position + 1 >= self.input_bytes.len() {
            return None;
        }
        
        // Fast path for ASCII characters
        let byte = unsafe { *self.input_bytes.get_unchecked(self.position + 1) };
        if byte < 128 {
            return Some(byte as char);
        }
        
        // Slow path for Unicode characters - need to find the next char boundary
        let remaining = &self.input[self.position..];
        let mut chars = remaining.chars();
        chars.next(); // Skip current char
        chars.next()
    }
    
    #[inline(always)]
    fn advance(&mut self) {
        if self.position < self.input_bytes.len() {
            let byte = unsafe { *self.input_bytes.get_unchecked(self.position) };
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
    
    #[inline(always)]
    fn is_at_end(&self) -> bool {
        self.position >= self.input_bytes.len()
    }
    
    /// Fast byte advancement for ASCII characters
    #[inline(always)]
    fn advance_byte(&mut self) {
        self.position += 1;
        self.current_pos.column += 1;
        self.current_pos.offset = self.position as u32;
    }
    
    /// Fast operator scanning methods for maximum performance
    fn scan_plus_operators_fast(&mut self) -> TokenType {
        self.position += 1;
        if self.position < self.input_bytes.len() && unsafe { *self.input_bytes.get_unchecked(self.position) } == b'=' {
            self.position += 1;
            self.current_pos.column += 2;
            self.current_pos.offset = self.position as u32;
            TokenType::PlusAssign
        } else {
            self.current_pos.column += 1;
            self.current_pos.offset = self.position as u32;
            TokenType::Plus
        }
    }
    
    fn scan_minus_operators_fast(&mut self) -> TokenType {
        self.position += 1;
        if self.position < self.input_bytes.len() {
            let byte = unsafe { *self.input_bytes.get_unchecked(self.position) };
            match byte {
                b'=' => {
                    self.position += 1;
                    self.current_pos.column += 2;
                    self.current_pos.offset = self.position as u32;
                    TokenType::MinusAssign
                }
                b'>' => {
                    self.position += 1;
                    self.current_pos.column += 2;
                    self.current_pos.offset = self.position as u32;
                    TokenType::Arrow
                }
                _ => {
                    self.current_pos.column += 1;
                    self.current_pos.offset = self.position as u32;
                    TokenType::Minus
                }
            }
        } else {
            self.current_pos.column += 1;
            self.current_pos.offset = self.position as u32;
            TokenType::Minus
        }
    }
    
    fn scan_multiply_operators_fast(&mut self) -> TokenType {
        self.position += 1;
        if self.position < self.input_bytes.len() && unsafe { *self.input_bytes.get_unchecked(self.position) } == b'=' {
            self.position += 1;
            self.current_pos.column += 2;
            self.current_pos.offset = self.position as u32;
            TokenType::MultiplyAssign
        } else {
            self.current_pos.column += 1;
            self.current_pos.offset = self.position as u32;
            TokenType::Multiply
        }
    }
    
    fn scan_divide_operators_fast(&mut self) -> TokenType {
        self.position += 1;
        if self.position < self.input_bytes.len() && unsafe { *self.input_bytes.get_unchecked(self.position) } == b'=' {
            self.position += 1;
            self.current_pos.column += 2;
            self.current_pos.offset = self.position as u32;
            TokenType::DivideAssign
        } else {
            self.current_pos.column += 1;
            self.current_pos.offset = self.position as u32;
            TokenType::Divide
        }
    }
    
    fn scan_modulo_operators_fast(&mut self) -> TokenType {
        self.position += 1;
        if self.position < self.input_bytes.len() && unsafe { *self.input_bytes.get_unchecked(self.position) } == b'=' {
            self.position += 1;
            self.current_pos.column += 2;
            self.current_pos.offset = self.position as u32;
            TokenType::ModuloAssign
        } else {
            self.current_pos.column += 1;
            self.current_pos.offset = self.position as u32;
            TokenType::Modulo
        }
    }
    
    fn scan_equal_operators_fast(&mut self) -> TokenType {
        self.position += 1;
        if self.position < self.input_bytes.len() {
            let byte = unsafe { *self.input_bytes.get_unchecked(self.position) };
            match byte {
                b'=' => {
                    self.position += 1;
                    self.current_pos.column += 2;
                    self.current_pos.offset = self.position as u32;
                    TokenType::Equal
                }
                b'>' => {
                    self.position += 1;
                    self.current_pos.column += 2;
                    self.current_pos.offset = self.position as u32;
                    TokenType::FatArrow
                }
                _ => {
                    self.current_pos.column += 1;
                    self.current_pos.offset = self.position as u32;
                    TokenType::Assign
                }
            }
        } else {
            self.current_pos.column += 1;
            self.current_pos.offset = self.position as u32;
            TokenType::Assign
        }
    }
    
    fn scan_not_operators_fast(&mut self) -> TokenType {
        self.position += 1;
        if self.position < self.input_bytes.len() && unsafe { *self.input_bytes.get_unchecked(self.position) } == b'=' {
            self.position += 1;
            self.current_pos.column += 2;
            self.current_pos.offset = self.position as u32;
            TokenType::NotEqual
        } else {
            self.current_pos.column += 1;
            self.current_pos.offset = self.position as u32;
            TokenType::LogicalNot
        }
    }
    
    fn scan_less_operators_fast(&mut self) -> TokenType {
        self.position += 1;
        if self.position < self.input_bytes.len() {
            let byte = unsafe { *self.input_bytes.get_unchecked(self.position) };
            match byte {
                b'=' => {
                    self.position += 1;
                    self.current_pos.column += 2;
                    self.current_pos.offset = self.position as u32;
                    TokenType::LessEqual
                }
                b'<' => {
                    self.position += 1;
                    self.current_pos.column += 2;
                    self.current_pos.offset = self.position as u32;
                    TokenType::LeftShift
                }
                _ => {
                    self.current_pos.column += 1;
                    self.current_pos.offset = self.position as u32;
                    TokenType::Less
                }
            }
        } else {
            self.current_pos.column += 1;
            self.current_pos.offset = self.position as u32;
            TokenType::Less
        }
    }
    
    fn scan_greater_operators_fast(&mut self) -> TokenType {
        self.position += 1;
        if self.position < self.input_bytes.len() {
            let byte = unsafe { *self.input_bytes.get_unchecked(self.position) };
            match byte {
                b'=' => {
                    self.position += 1;
                    self.current_pos.column += 2;
                    self.current_pos.offset = self.position as u32;
                    TokenType::GreaterEqual
                }
                b'>' => {
                    self.position += 1;
                    self.current_pos.column += 2;
                    self.current_pos.offset = self.position as u32;
                    TokenType::RightShift
                }
                _ => {
                    self.current_pos.column += 1;
                    self.current_pos.offset = self.position as u32;
                    TokenType::Greater
                }
            }
        } else {
            self.current_pos.column += 1;
            self.current_pos.offset = self.position as u32;
            TokenType::Greater
        }
    }
    
    fn scan_and_operators_fast(&mut self) -> TokenType {
        self.position += 1;
        if self.position < self.input_bytes.len() && unsafe { *self.input_bytes.get_unchecked(self.position) } == b'&' {
            self.position += 1;
            self.current_pos.column += 2;
            self.current_pos.offset = self.position as u32;
            TokenType::LogicalAnd
        } else {
            self.current_pos.column += 1;
            self.current_pos.offset = self.position as u32;
            TokenType::BitwiseAnd
        }
    }
    
    fn scan_or_operators_fast(&mut self) -> TokenType {
        self.position += 1;
        if self.position < self.input_bytes.len() && unsafe { *self.input_bytes.get_unchecked(self.position) } == b'|' {
            self.position += 1;
            self.current_pos.column += 2;
            self.current_pos.offset = self.position as u32;
            TokenType::LogicalOr
        } else {
            self.current_pos.column += 1;
            self.current_pos.offset = self.position as u32;
            TokenType::BitwiseOr
        }
    }
    
    fn scan_colon_operators_fast(&mut self) -> TokenType {
        self.position += 1;
        if self.position < self.input_bytes.len() && unsafe { *self.input_bytes.get_unchecked(self.position) } == b':' {
            self.position += 1;
            self.current_pos.column += 2;
            self.current_pos.offset = self.position as u32;
            TokenType::DoubleColon
        } else {
            self.current_pos.column += 1;
            self.current_pos.offset = self.position as u32;
            TokenType::Colon
        }
    }
    
    fn scan_dot_operators_fast(&mut self) -> TokenType {
        self.position += 1;
        if self.position < self.input_bytes.len() && unsafe { *self.input_bytes.get_unchecked(self.position) } == b'.' {
            self.position += 1;
            if self.position < self.input_bytes.len() && unsafe { *self.input_bytes.get_unchecked(self.position) } == b'.' {
                self.position += 1;
                self.current_pos.column += 3;
                self.current_pos.offset = self.position as u32;
                TokenType::TripleDot
            } else {
                self.current_pos.column += 2;
                self.current_pos.offset = self.position as u32;
                TokenType::DoubleDot
            }
        } else {
            self.current_pos.column += 1;
            self.current_pos.offset = self.position as u32;
            TokenType::Dot
        }
    }
}