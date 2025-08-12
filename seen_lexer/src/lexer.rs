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
                    return Err(LexerError::UnexpectedCharacter {
                        character: '=',
                        position: start_pos,
                    });
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
                    return Err(LexerError::UnexpectedCharacter {
                        character: '.',
                        position: start_pos,
                    });
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
    
    pub fn tokenize_string_interpolation(&mut self) -> LexerResult<Vec<InterpolationPart>> {
        // Implementation will follow TDD methodology
        todo!("Implementation follows TDD - tests first")
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
        // Use dynamic keyword lookup instead of hardcoded keywords
        if let Some(keyword_type) = self.keyword_manager.is_keyword(text) {
            // Convert KeywordType to TokenType
            Some(match keyword_type {
                KeywordType::KeywordFun => TokenType::Fun,
                KeywordType::KeywordIf => TokenType::If,
                KeywordType::KeywordElse => TokenType::Else,
                KeywordType::KeywordWhile => TokenType::While,
                KeywordType::KeywordFor => TokenType::For,
                KeywordType::KeywordIn => TokenType::In,
                KeywordType::KeywordMatch => TokenType::Match,
                KeywordType::KeywordBreak => TokenType::Break,
                KeywordType::KeywordContinue => TokenType::Continue,
                KeywordType::KeywordReturn => TokenType::Return,
                KeywordType::KeywordWhen => TokenType::When,
                KeywordType::KeywordLet => TokenType::Let,
                KeywordType::KeywordMut => TokenType::Mut,
                KeywordType::KeywordConst => TokenType::Const,
                KeywordType::KeywordStatic => TokenType::Static,
                KeywordType::KeywordVal => TokenType::Val,
                KeywordType::KeywordVar => TokenType::Var,
                KeywordType::KeywordStruct => TokenType::Struct,
                KeywordType::KeywordEnum => TokenType::Enum,
                KeywordType::KeywordTrait => TokenType::Trait,
                KeywordType::KeywordImpl => TokenType::Impl,
                KeywordType::KeywordType => TokenType::Type,
                KeywordType::KeywordClass => TokenType::Class,
                KeywordType::KeywordData => TokenType::Data,
                KeywordType::KeywordSealed => TokenType::Sealed,
                KeywordType::KeywordObject => TokenType::Object,
                KeywordType::KeywordInterface => TokenType::Interface,
                KeywordType::KeywordModule => TokenType::Module,
                KeywordType::KeywordImport => TokenType::Import,
                KeywordType::KeywordUse => TokenType::Use,
                KeywordType::KeywordTrue => TokenType::True,
                KeywordType::KeywordFalse => TokenType::False,
                KeywordType::KeywordNull => TokenType::Null,
                KeywordType::KeywordIs => TokenType::Is,
                KeywordType::KeywordAs => TokenType::As,
                KeywordType::KeywordBy => TokenType::By,
                KeywordType::KeywordSuspend => TokenType::Suspend,
                KeywordType::KeywordAwait => TokenType::Await,
                KeywordType::KeywordLaunch => TokenType::Launch,
                KeywordType::KeywordFlow => TokenType::Flow,
                KeywordType::KeywordTry => TokenType::Try,
                KeywordType::KeywordCatch => TokenType::Catch,
                KeywordType::KeywordFinally => TokenType::Finally,
                KeywordType::KeywordThrow => TokenType::Throw,
                KeywordType::KeywordInline => TokenType::Inline,
                KeywordType::KeywordReified => TokenType::Reified,
                KeywordType::KeywordCrossinline => TokenType::Crossinline,
                KeywordType::KeywordNoinline => TokenType::Noinline,
                KeywordType::KeywordOperator => TokenType::Operator,
                KeywordType::KeywordInfix => TokenType::Infix,
                KeywordType::KeywordTailrec => TokenType::Tailrec,
                KeywordType::KeywordOpen => TokenType::Open,
                KeywordType::KeywordFinal => TokenType::Final,
                KeywordType::KeywordAbstract => TokenType::Abstract,
                KeywordType::KeywordOverride => TokenType::Override,
                KeywordType::KeywordLateinit => TokenType::Lateinit,
                KeywordType::KeywordCompanion => TokenType::Companion,
                KeywordType::KeywordAnd => TokenType::LogicalAnd,
                KeywordType::KeywordOr => TokenType::LogicalOr,
                KeywordType::KeywordNot => TokenType::LogicalNot,
                KeywordType::KeywordMove => TokenType::Move,
                KeywordType::KeywordBorrow => TokenType::Borrow,
                KeywordType::KeywordInout => TokenType::Inout,
            })
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
        ch.is_alphabetic() || ch == '_' || ch.is_numeric() == false && ch.is_alphanumeric()
    }
    
    fn is_identifier_continue(&self, ch: char) -> bool {
        ch.is_alphanumeric() || ch == '_'
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
        let mut string_value = String::new();
        let mut lexeme = String::new();
        
        // Skip opening quote
        lexeme.push('"');
        self.advance();
        
        while let Some(ch) = self.current_char {
            if ch == '"' {
                lexeme.push('"');
                self.advance();
                return Ok(Token::new(TokenType::StringLiteral(string_value), lexeme, start_pos));
            } else if ch == '\\' {
                lexeme.push('\\');
                self.advance();
                
                match self.current_char {
                    Some('n') => {
                        string_value.push('\n');
                        lexeme.push('n');
                        self.advance();
                    }
                    Some('t') => {
                        string_value.push('\t');
                        lexeme.push('t');
                        self.advance();
                    }
                    Some('r') => {
                        string_value.push('\r');
                        lexeme.push('r');
                        self.advance();
                    }
                    Some('\\') => {
                        string_value.push('\\');
                        lexeme.push('\\');
                        self.advance();
                    }
                    Some('"') => {
                        string_value.push('"');
                        lexeme.push('"');
                        self.advance();
                    }
                    Some('u') => {
                        lexeme.push('u');
                        self.advance();
                        let unicode_char = self.read_unicode_escape()?;
                        string_value.push(unicode_char);
                    }
                    Some(escape_char) => {
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
                string_value.push(ch);
                lexeme.push(ch);
                self.advance();
            }
        }
        
        Err(LexerError::UnterminatedString {
            position: start_pos,
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
        
        assert_eq!(lexer.check_keyword(&fun_keyword), Some(TokenType::Fun));
        assert_eq!(lexer.check_keyword(&if_keyword), Some(TokenType::If));
        assert_eq!(lexer.check_keyword(&and_keyword), Some(TokenType::LogicalAnd));
        assert_eq!(lexer.check_keyword(&or_keyword), Some(TokenType::LogicalOr));
        assert_eq!(lexer.check_keyword(&not_keyword), Some(TokenType::LogicalNot));
        
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
        
        assert_eq!(lexer_en.check_keyword(&en_fun_keyword), Some(TokenType::Fun));
        assert_eq!(lexer_en.check_keyword(ar_fun_keyword), None); // Arabic should not work in English mode
        
        // Test Arabic
        keyword_manager.switch_language("ar").unwrap();
        let lexer_ar = Lexer::new("test".to_string(), Arc::new(keyword_manager.clone()));
        
        let ar_fun_keyword_dynamic = keyword_manager.get_keyword_text(&KeywordType::KeywordFun).unwrap();
        
        assert_eq!(lexer_ar.check_keyword(&ar_fun_keyword_dynamic), Some(TokenType::Fun));
        assert_eq!(lexer_ar.check_keyword(&en_fun_keyword), None); // English should not work in Arabic mode
    }
    
    // Additional tests will be implemented following TDD methodology
}