//! Main lexer implementation

use crate::{
    keyword_manager::{KeywordManager, KeywordType},
    token::{Token, TokenType, InterpolationPart, InterpolationKind},
    position::Position,
    error::{LexerError, LexerResult},
};
use std::sync::Arc;

pub struct Lexer {
    keyword_manager: Arc<KeywordManager>,
    input: String,
    position: usize,
    current_char: Option<char>,
    pos_tracker: Position,
}

impl Lexer {
    pub fn new(input: String, keyword_manager: Arc<KeywordManager>) -> Self {
        let mut lexer = Self {
            keyword_manager,
            input,
            position: 0,
            current_char: None,
            pos_tracker: Position::start(),
        };
        lexer.current_char = lexer.input.chars().next();
        lexer
    }
    
    pub fn next_token(&mut self) -> LexerResult<Token> {
        self.skip_whitespace();
        
        let start_pos = self.pos_tracker;
        
        match self.current_char {
            None => Ok(Token::new(TokenType::EOF, "".to_string(), start_pos)),
            
            Some('\n') => {
                self.advance();
                Ok(Token::new(TokenType::Newline, "\n".to_string(), start_pos))
            }
            
            Some(ch) if ch.is_ascii_digit() => self.read_number(),
            
            Some('"') => self.read_string_literal(),
            
            Some('\'') => self.read_char_literal(),
            
            Some(ch) if self.is_identifier_start(ch) => self.read_identifier(),
            
            // Mathematical operators
            Some('+') => {
                self.advance();
                Ok(Token::new(TokenType::Plus, "+".to_string(), start_pos))
            }
            Some('-') => {
                if self.peek() == Some('>') {
                    self.advance();
                    self.advance();
                    Ok(Token::new(TokenType::Arrow, "->".to_string(), start_pos))
                } else {
                    self.advance();
                    Ok(Token::new(TokenType::Minus, "-".to_string(), start_pos))
                }
            }
            Some('*') => {
                self.advance();
                Ok(Token::new(TokenType::Multiply, "*".to_string(), start_pos))
            }
            Some('/') => {
                self.advance();
                Ok(Token::new(TokenType::Divide, "/".to_string(), start_pos))
            }
            Some('%') => {
                self.advance();
                Ok(Token::new(TokenType::Modulo, "%".to_string(), start_pos))
            }
            
            // Comparison operators
            Some('=') => {
                if self.peek() == Some('=') {
                    self.advance();
                    self.advance();
                    Ok(Token::new(TokenType::Equal, "==".to_string(), start_pos))
                } else {
                    self.advance();
                    Ok(Token::new(TokenType::Assign, "=".to_string(), start_pos))
                }
            }
            Some('!') => {
                if self.peek() == Some('=') {
                    self.advance();
                    self.advance();
                    Ok(Token::new(TokenType::NotEqual, "!=".to_string(), start_pos))
                } else if self.peek() == Some('!') {
                    self.advance();
                    self.advance();
                    Ok(Token::new(TokenType::ForceUnwrap, "!!".to_string(), start_pos))
                } else {
                    return Err(LexerError::UnexpectedCharacter {
                        character: '!',
                        position: start_pos,
                    });
                }
            }
            Some('<') => {
                if self.peek() == Some('=') {
                    self.advance();
                    self.advance();
                    Ok(Token::new(TokenType::LessEqual, "<=".to_string(), start_pos))
                } else {
                    self.advance();
                    Ok(Token::new(TokenType::Less, "<".to_string(), start_pos))
                }
            }
            Some('>') => {
                if self.peek() == Some('=') {
                    self.advance();
                    self.advance();
                    Ok(Token::new(TokenType::GreaterEqual, ">=".to_string(), start_pos))
                } else {
                    self.advance();
                    Ok(Token::new(TokenType::Greater, ">".to_string(), start_pos))
                }
            }
            
            // Nullable operators
            Some('?') => {
                if self.peek() == Some('.') {
                    self.advance();
                    self.advance();
                    Ok(Token::new(TokenType::SafeNavigation, "?.".to_string(), start_pos))
                } else if self.peek() == Some(':') {
                    self.advance();
                    self.advance();
                    Ok(Token::new(TokenType::Elvis, "?:".to_string(), start_pos))
                } else {
                    self.advance();
                    Ok(Token::new(TokenType::Question, "?".to_string(), start_pos))
                }
            }
            
            // Range operators
            Some('.') => {
                if self.peek() == Some('.') {
                    self.advance();
                    self.advance();
                    if self.current_char == Some('<') {
                        self.advance();
                        Ok(Token::new(TokenType::ExclusiveRange, "..<".to_string(), start_pos))
                    } else {
                        Ok(Token::new(TokenType::InclusiveRange, "..".to_string(), start_pos))
                    }
                } else {
                    self.advance();
                    Ok(Token::new(TokenType::Dot, ".".to_string(), start_pos))
                }
            }
            
            // Punctuation
            Some('(') => {
                self.advance();
                Ok(Token::new(TokenType::LeftParen, "(".to_string(), start_pos))
            }
            Some(')') => {
                self.advance();
                Ok(Token::new(TokenType::RightParen, ")".to_string(), start_pos))
            }
            Some('{') => {
                self.advance();
                Ok(Token::new(TokenType::LeftBrace, "{".to_string(), start_pos))
            }
            Some('}') => {
                self.advance();
                Ok(Token::new(TokenType::RightBrace, "}".to_string(), start_pos))
            }
            Some('[') => {
                self.advance();
                Ok(Token::new(TokenType::LeftBracket, "[".to_string(), start_pos))
            }
            Some(']') => {
                self.advance();
                Ok(Token::new(TokenType::RightBracket, "]".to_string(), start_pos))
            }
            Some(',') => {
                self.advance();
                Ok(Token::new(TokenType::Comma, ",".to_string(), start_pos))
            }
            Some(';') => {
                self.advance();
                Ok(Token::new(TokenType::Semicolon, ";".to_string(), start_pos))
            }
            Some(':') => {
                self.advance();
                Ok(Token::new(TokenType::Colon, ":".to_string(), start_pos))
            }
            
            Some(ch) => Err(LexerError::UnexpectedCharacter {
                character: ch,
                position: start_pos,
            }),
        }
    }
    
    pub fn handle_unicode(&mut self) -> LexerResult<char> {
        match self.current_char {
            Some(ch) => {
                self.advance();
                Ok(ch)
            }
            None => Err(LexerError::UnexpectedCharacter {
                character: '\0',
                position: self.pos_tracker,
            }),
        }
    }
    
    pub fn classify_identifier(&self, text: &str) -> TokenType {
        // Capitalization-based visibility (Go's proven pattern)
        if let Some(first_char) = text.chars().next() {
            if first_char.is_uppercase() {
                TokenType::PublicIdentifier(text.to_string())
            } else {
                TokenType::PrivateIdentifier(text.to_string())
            }
        } else {
            TokenType::PrivateIdentifier(text.to_string())
        }
    }
    
    pub fn check_keyword(&self, text: &str) -> Option<TokenType> {
        // Use dynamic keyword lookup - NO HARDCODING!
        if let Some(keyword_type) = self.keyword_manager.is_keyword(text) {
            // Special handling for boolean literals
            match keyword_type {
                KeywordType::KeywordTrue => Some(TokenType::BoolLiteral(true)),
                KeywordType::KeywordFalse => Some(TokenType::BoolLiteral(false)),
                // All other keywords use the dynamic Keyword variant
                _ => Some(TokenType::Keyword(keyword_type))
            }
        } else {
            None
        }
    }
    
    fn advance(&mut self) {
        if let Some(ch) = self.current_char {
            self.pos_tracker.advance_char(ch);
            self.position += ch.len_utf8();
            
            // Get next character from the remaining string
            let remaining = &self.input[self.position..];
            self.current_char = remaining.chars().next();
        }
    }
    
    fn peek(&self) -> Option<char> {
        if let Some(current_char) = self.current_char {
            let current_len = current_char.len_utf8();
            let next_position = self.position + current_len;
            
            if next_position < self.input.len() {
                let remaining = &self.input[next_position..];
                remaining.chars().next()
            } else {
                None
            }
        } else {
            None
        }
    }
    
    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.current_char {
            if ch.is_whitespace() && ch != '\n' {
                self.advance();
            } else {
                break;
            }
        }
    }
    
    fn is_identifier_start(&self, ch: char) -> bool {
        // Unicode-aware identifier start:
        // - Letters (including Unicode letters)
        // - Underscore
        // - Unicode characters that aren't operators, punctuation, or whitespace
        ch.is_alphabetic() || ch == '_' || 
        (!ch.is_ascii() && !ch.is_numeric() && !ch.is_whitespace() && 
         !self.is_operator_char(ch) && !self.is_punctuation_char(ch))
    }
    
    fn is_identifier_continue(&self, ch: char) -> bool {
        // Unicode-aware identifier continuation:
        // - Letters and digits (including Unicode)
        // - Underscore
        // - Unicode marks (combining characters)
        ch.is_alphanumeric() || ch == '_' || 
        (!ch.is_ascii() && !ch.is_whitespace() && 
         !self.is_operator_char(ch) && !self.is_punctuation_char(ch))
    }
    
    fn is_operator_char(&self, ch: char) -> bool {
        // Check if character is an operator that we handle explicitly
        matches!(ch, '+' | '-' | '*' | '/' | '%' | '=' | '!' | '<' | '>' | '?' | '.' | ':')
    }
    
    fn is_punctuation_char(&self, ch: char) -> bool {
        // Check if character is punctuation that we handle explicitly
        matches!(ch, '(' | ')' | '{' | '}' | '[' | ']' | ',' | ';' | '"' | '\'' | '\\')
    }
    
    fn read_number(&mut self) -> LexerResult<Token> {
        let start_pos = self.pos_tracker;
        let mut number_str = String::new();
        let mut is_float = false;
        let mut is_unsigned = false;
        
        // Read integer part
        while let Some(ch) = self.current_char {
            if ch.is_ascii_digit() {
                number_str.push(ch);
                self.advance();
            } else {
                break;
            }
        }
        
        // Check for decimal point
        if self.current_char == Some('.') && self.peek().map_or(false, |ch| ch.is_ascii_digit()) {
            is_float = true;
            number_str.push('.');
            self.advance();
            
            // Read fractional part
            while let Some(ch) = self.current_char {
                if ch.is_ascii_digit() {
                    number_str.push(ch);
                    self.advance();
                } else {
                    break;
                }
            }
            
            // Check for additional decimal points (invalid)
            if self.current_char == Some('.') {
                return Err(LexerError::InvalidNumber {
                    position: start_pos,
                    message: "Number contains multiple decimal points".to_string(),
                });
            }
        }
        
        // Check for unsigned suffix
        if self.current_char == Some('u') && !is_float {
            is_unsigned = true;
            number_str.push('u');
            self.advance();
        }
        
        // Parse the number
        if is_float {
            let value: f64 = number_str.parse()
                .map_err(|_| LexerError::InvalidNumber {
                    position: start_pos,
                    message: "Invalid float format".to_string(),
                })?;
            Ok(Token::new(TokenType::FloatLiteral(value), number_str, start_pos))
        } else if is_unsigned {
            let number_part = &number_str[..number_str.len() - 1]; // Remove 'u' suffix
            let value: u64 = number_part.parse()
                .map_err(|_| LexerError::InvalidNumber {
                    position: start_pos,
                    message: "Invalid unsigned integer format".to_string(),
                })?;
            Ok(Token::new(TokenType::UIntegerLiteral(value), number_str, start_pos))
        } else {
            let value: i64 = number_str.parse()
                .map_err(|_| LexerError::InvalidNumber {
                    position: start_pos,
                    message: "Invalid integer format".to_string(),
                })?;
            Ok(Token::new(TokenType::IntegerLiteral(value), number_str, start_pos))
        }
    }
    
    fn read_string_literal(&mut self) -> LexerResult<Token> {
        let start_pos = self.pos_tracker;
        let mut parts = Vec::new();
        let mut current_text = String::new();
        let mut has_interpolation = false;
        let mut lexeme = String::new();
        
        // Skip opening quote
        lexeme.push('"');
        self.advance();
        let mut text_start_pos = self.pos_tracker; // Position after opening quote
        
        while let Some(ch) = self.current_char {
            if ch == '"' {
                // End of string
                lexeme.push('"');
                self.advance();
                
                if has_interpolation {
                    // Add any remaining text if there is any, or if we need to maintain proper structure
                    // (e.g., when string ends immediately after an interpolation)
                    if !current_text.is_empty() || parts.is_empty() || 
                       (parts.len() % 2 == 0) { // Odd number of parts means we ended on text, even means we ended on expression
                        
                        // Adjust position for text positioning
                        let mut final_text_pos = text_start_pos;
                        if current_text.starts_with('\n') && current_text.len() > 1 {
                            // Text starts with newline - position should be after the newline for meaningful content
                            final_text_pos.line += 1;
                            final_text_pos.column = 1;
                        } else if !current_text.starts_with('\n') && text_start_pos.column > 1 && !parts.is_empty() {
                            // Single-line text after interpolation - adjust to closing brace position
                            final_text_pos.column -= 1;
                        }
                        
                        parts.push(InterpolationPart {
                            kind: InterpolationKind::Text(current_text.clone()),
                            content: current_text,
                            position: final_text_pos,
                        });
                    }
                    return Ok(Token::new(TokenType::InterpolatedString(parts), lexeme, start_pos));
                } else {
                    return Ok(Token::new(TokenType::StringLiteral(current_text), lexeme, start_pos));
                }
            } else if ch == '{' {
                // Check for escaped brace or interpolation
                let brace_pos = self.pos_tracker; // Position of the '{'
                self.advance();
                if self.current_char == Some('{') {
                    // Escaped opening brace
                    current_text.push('{');
                    lexeme.push_str("{{");
                    self.advance();
                } else {
                    // Start of interpolation
                    has_interpolation = true;
                    
                    // Save current text part if any
                    if !current_text.is_empty() {
                        // Adjust position for text positioning
                        let mut final_text_pos = text_start_pos;
                        if current_text.starts_with('\n') && current_text.len() > 1 {
                            // Text starts with newline - position should be after the newline for meaningful content
                            final_text_pos.line += 1;
                            final_text_pos.column = 1;
                        } else if !current_text.starts_with('\n') && text_start_pos.column > 1 && !parts.is_empty() {
                            // Single-line text after interpolation - adjust to closing brace position
                            final_text_pos.column -= 1;
                        }
                        
                        parts.push(InterpolationPart {
                            kind: InterpolationKind::Text(current_text.clone()),
                            content: current_text.clone(),
                            position: final_text_pos,
                        });
                        current_text.clear();
                    }
                    
                    // Read the interpolated expression
                    let expr = self.read_interpolation_expression()?;
                    
                    if expr.is_empty() {
                        return Err(LexerError::InvalidInterpolation {
                            position: brace_pos,
                            message: "Empty interpolation expression".to_string(),
                        });
                    }
                    
                    parts.push(InterpolationPart {
                        kind: InterpolationKind::Expression(expr.clone()),
                        content: expr,
                        position: brace_pos,
                    });
                    
                    // Update text start position for next text part
                    // Position should be where the text content begins (after closing brace)
                    text_start_pos = self.pos_tracker;
                    
                    lexeme.push_str(&format!("{{...}}"));
                }
            } else if ch == '}' {
                // Check for escaped closing brace
                self.advance();
                if self.current_char == Some('}') {
                    // Escaped closing brace
                    current_text.push('}');
                    lexeme.push_str("}}");
                    self.advance();
                } else {
                    // Single closing brace in string
                    current_text.push('}');
                    lexeme.push('}');
                }
            } else if ch == '\\' {
                lexeme.push('\\');
                self.advance();
                
                match self.current_char {
                    Some('n') => {
                        current_text.push('\n');
                        lexeme.push('n');
                        self.advance();
                    }
                    Some('t') => {
                        current_text.push('\t');
                        lexeme.push('t');
                        self.advance();
                    }
                    Some('r') => {
                        current_text.push('\r');
                        lexeme.push('r');
                        self.advance();
                    }
                    Some('\\') => {
                        current_text.push('\\');
                        lexeme.push('\\');
                        self.advance();
                    }
                    Some('"') => {
                        current_text.push('"');
                        lexeme.push('"');
                        self.advance();
                    }
                    Some('u') => {
                        lexeme.push('u');
                        self.advance();
                        let unicode_char = self.read_unicode_escape()?;
                        current_text.push(unicode_char);
                    }
                    Some(_escape_char) => {
                        return Err(LexerError::InvalidUnicodeEscape {
                            position: self.pos_tracker,
                        });
                    }
                    None => {
                        return Err(LexerError::UnterminatedString {
                            position: start_pos,
                        });
                    }
                }
            } else {
                current_text.push(ch);
                lexeme.push(ch);
                self.advance();
            }
        }
        
        Err(LexerError::UnterminatedString {
            position: start_pos,
        })
    }
    
    fn read_interpolation_expression(&mut self) -> LexerResult<String> {
        let mut expr = String::new();
        let mut brace_depth = 1; // We're already inside one '{'
        
        while let Some(ch) = self.current_char {
            if ch == '{' {
                brace_depth += 1;
                expr.push(ch);
                self.advance();
            } else if ch == '}' {
                brace_depth -= 1;
                if brace_depth == 0 {
                    // End of interpolation - advance past the closing '}'
                    self.advance();
                    return Ok(expr);
                } else {
                    expr.push(ch);
                    self.advance();
                }
            } else if ch == '"' {
                // Handle strings within interpolation
                expr.push(ch);
                self.advance();
                self.read_string_in_interpolation(&mut expr)?;
            } else {
                expr.push(ch);
                self.advance();
            }
        }
        
        Err(LexerError::UnterminatedString {
            position: self.pos_tracker,
        })
    }
    
    fn read_string_in_interpolation(&mut self, expr: &mut String) -> LexerResult<()> {
        // Read a string literal within an interpolation expression
        while let Some(ch) = self.current_char {
            if ch == '"' {
                expr.push(ch);
                self.advance();
                return Ok(());
            } else if ch == '\\' {
                expr.push(ch);
                self.advance();
                if let Some(next) = self.current_char {
                    expr.push(next);
                    self.advance();
                }
            } else {
                expr.push(ch);
                self.advance();
            }
        }
        
        Err(LexerError::UnterminatedString {
            position: self.pos_tracker,
        })
    }
    
    fn read_char_literal(&mut self) -> LexerResult<Token> {
        let start_pos = self.pos_tracker;
        let mut lexeme = String::new();
        
        // Skip opening quote
        lexeme.push('\'');
        self.advance();
        
        let char_value = match self.current_char {
            Some('\\') => {
                lexeme.push('\\');
                self.advance();
                
                match self.current_char {
                    Some('n') => {
                        lexeme.push('n');
                        self.advance();
                        '\n'
                    }
                    Some('t') => {
                        lexeme.push('t');
                        self.advance();
                        '\t'
                    }
                    Some('r') => {
                        lexeme.push('r');
                        self.advance();
                        '\r'
                    }
                    Some('\\') => {
                        lexeme.push('\\');
                        self.advance();
                        '\\'
                    }
                    Some('\'') => {
                        lexeme.push('\'');
                        self.advance();
                        '\''
                    }
                    Some('u') => {
                        lexeme.push('u');
                        self.advance();
                        self.read_unicode_escape()?
                    }
                    _ => {
                        return Err(LexerError::InvalidUnicodeEscape {
                            position: self.pos_tracker,
                        });
                    }
                }
            }
            Some(ch) => {
                lexeme.push(ch);
                self.advance();
                ch
            }
            None => {
                return Err(LexerError::UnterminatedString {
                    position: start_pos,
                });
            }
        };
        
        // Expect closing quote
        if self.current_char == Some('\'') {
            lexeme.push('\'');
            self.advance();
            Ok(Token::new(TokenType::CharLiteral(char_value), lexeme, start_pos))
        } else {
            Err(LexerError::UnterminatedString {
                position: start_pos,
            })
        }
    }
    
    fn read_unicode_escape(&mut self) -> LexerResult<char> {
        // Expect {
        if self.current_char != Some('{') {
            return Err(LexerError::InvalidUnicodeEscape {
                position: self.pos_tracker,
            });
        }
        self.advance();
        
        let mut hex_digits = String::new();
        
        // Read hex digits
        while let Some(ch) = self.current_char {
            if ch == '}' {
                break;
            } else if ch.is_ascii_hexdigit() {
                hex_digits.push(ch);
                self.advance();
            } else {
                return Err(LexerError::InvalidUnicodeEscape {
                    position: self.pos_tracker,
                });
            }
        }
        
        // Expect }
        if self.current_char != Some('}') {
            return Err(LexerError::InvalidUnicodeEscape {
                position: self.pos_tracker,
            });
        }
        self.advance();
        
        // Parse hex value
        let code_point = u32::from_str_radix(&hex_digits, 16)
            .map_err(|_| LexerError::InvalidUnicodeEscape {
                position: self.pos_tracker,
            })?;
        
        // Convert to char
        char::from_u32(code_point)
            .ok_or(LexerError::InvalidUnicodeEscape {
                position: self.pos_tracker,
            })
    }
    
    fn read_identifier(&mut self) -> LexerResult<Token> {
        let start_pos = self.pos_tracker;
        let mut identifier = String::new();
        
        // Read identifier characters
        while let Some(ch) = self.current_char {
            if self.is_identifier_continue(ch) {
                identifier.push(ch);
                self.advance();
            } else {
                break;
            }
        }
        
        // Check if it's a keyword
        if let Some(token_type) = self.check_keyword(&identifier) {
            Ok(Token::new(token_type, identifier, start_pos))
        } else {
            // Classify as public or private identifier based on capitalization
            let token_type = self.classify_identifier(&identifier);
            Ok(Token::new(token_type, identifier, start_pos))
        }
    }
    
    /// Get the keyword text for a specific keyword type in the current language
    pub fn get_keyword_text(&self, keyword_type: &KeywordType) -> Option<String> {
        self.keyword_manager.get_keyword_text(keyword_type)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_lexer_creation() {
        let keyword_manager = Arc::new(KeywordManager::new());
        let lexer = Lexer::new("test".to_string(), keyword_manager);
        
        assert_eq!(lexer.input, "test");
        assert_eq!(lexer.position, 0);
        assert_eq!(lexer.current_char, Some('t'));
    }
    
    #[test]
    fn test_dynamic_keyword_lookup() {
        let mut keyword_manager = KeywordManager::new();
        keyword_manager.load_from_toml("en").unwrap();
        keyword_manager.switch_language("en").unwrap();
        
        let lexer = Lexer::new("test".to_string(), Arc::new(keyword_manager.clone()));
        
        // Test English keywords using dynamic lookup
        let fun_keyword = keyword_manager.get_keyword_text(&KeywordType::KeywordFun).unwrap();
        let if_keyword = keyword_manager.get_keyword_text(&KeywordType::KeywordIf).unwrap();
        let and_keyword = keyword_manager.get_logical_and();
        let or_keyword = keyword_manager.get_logical_or();
        let not_keyword = keyword_manager.get_logical_not();
        
        assert_eq!(lexer.check_keyword(&fun_keyword), Some(TokenType::Keyword(KeywordType::KeywordFun)));
        assert_eq!(lexer.check_keyword(&if_keyword), Some(TokenType::Keyword(KeywordType::KeywordIf)));
        assert_eq!(lexer.check_keyword(&and_keyword), Some(TokenType::Keyword(KeywordType::KeywordAnd)));
        assert_eq!(lexer.check_keyword(&or_keyword), Some(TokenType::Keyword(KeywordType::KeywordOr)));
        assert_eq!(lexer.check_keyword(&not_keyword), Some(TokenType::Keyword(KeywordType::KeywordNot)));
        
        // Test non-keywords
        assert_eq!(lexer.check_keyword("variable_name"), None);
        assert_eq!(lexer.check_keyword("123"), None);
    }
    
    #[test]
    fn test_multilingual_keyword_lookup() {
        let mut keyword_manager = KeywordManager::new();
        keyword_manager.load_from_toml("en").unwrap();
        keyword_manager.load_from_toml("ar").unwrap();
        
        // Test English
        keyword_manager.switch_language("en").unwrap();
        let lexer_en = Lexer::new("test".to_string(), Arc::new(keyword_manager.clone()));
        
        let en_fun_keyword = keyword_manager.get_keyword_text(&KeywordType::KeywordFun).unwrap();
        let ar_fun_keyword = "دالة"; // This will be loaded from Arabic TOML
        
        assert_eq!(lexer_en.check_keyword(&en_fun_keyword), Some(TokenType::Keyword(KeywordType::KeywordFun)));
        assert_eq!(lexer_en.check_keyword(ar_fun_keyword), None); // Arabic should not work in English mode
        
        // Test Arabic
        keyword_manager.switch_language("ar").unwrap();
        let lexer_ar = Lexer::new("test".to_string(), Arc::new(keyword_manager.clone()));
        
        let ar_fun_keyword_dynamic = keyword_manager.get_keyword_text(&KeywordType::KeywordFun).unwrap();
        
        assert_eq!(lexer_ar.check_keyword(&ar_fun_keyword_dynamic), Some(TokenType::Keyword(KeywordType::KeywordFun)));
        assert_eq!(lexer_ar.check_keyword(&en_fun_keyword), None); // English should not work in Arabic mode
    }
    
    // Additional tests will be implemented following TDD methodology
}