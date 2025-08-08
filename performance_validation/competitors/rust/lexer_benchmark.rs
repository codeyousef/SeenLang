// Rust lexer benchmark implementation for fair comparison with Seen
// Uses a realistic tokenizer similar to what Seen's lexer would do

use std::collections::HashMap;
use std::fs;
use std::time::{Duration, Instant};
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub enum TokenType {
    // Keywords
    Func, Let, Mut, If, Else, While, For, Loop, Return, Break, Continue,
    Struct, Enum, Impl, Trait, Pub, Priv, Mod, Use, Import, Export,
    Match, When, Try, Catch, Finally, Async, Await, Const, Static,
    Type, Interface, Class, Extends, Implements, Abstract, Override,
    Virtual, Final,
    
    // Types
    I8, I16, I32, I64, U8, U16, U32, U64, F32, F64, Bool, Char, Str,
    String, Vec, HashMap, HashSet, Option, Result, Box, Rc, Arc,
    
    // Literals
    IntegerLiteral(String),
    FloatLiteral(String),
    StringLiteral(String),
    CharLiteral(String),
    BoolLiteral(bool),
    
    // Identifiers
    Identifier(String),
    
    // Operators
    Plus, Minus, Star, Slash, Percent, Equal, EqualEqual, NotEqual,
    Less, LessEqual, Greater, GreaterEqual, AndAnd, OrOr, Not,
    And, Or, Xor, LeftShift, RightShift, PlusEqual, MinusEqual,
    StarEqual, SlashEqual, PercentEqual, AndEqual, OrEqual,
    XorEqual, LeftShiftEqual, RightShiftEqual,
    
    // Punctuation
    LeftParen, RightParen, LeftBrace, RightBrace, LeftBracket,
    RightBracket, Semicolon, Comma, Dot, Arrow, FatArrow, Colon,
    DoubleColon, Question, At, Dollar, Hash,
    
    // Special
    Newline,
    Whitespace(String),
    Comment(String),
    Eof,
    Invalid(String),
}

#[derive(Debug, Clone)]
pub struct Token {
    pub token_type: TokenType,
    pub lexeme: String,
    pub line: usize,
    pub column: usize,
}

pub struct Lexer {
    input: Vec<char>,
    position: usize,
    line: usize,
    column: usize,
    keywords: HashMap<String, TokenType>,
}

impl Lexer {
    pub fn new(input: &str) -> Self {
        let mut keywords = HashMap::new();
        
        // Keywords
        keywords.insert("func".to_string(), TokenType::Func);
        keywords.insert("let".to_string(), TokenType::Let);
        keywords.insert("mut".to_string(), TokenType::Mut);
        keywords.insert("if".to_string(), TokenType::If);
        keywords.insert("else".to_string(), TokenType::Else);
        keywords.insert("while".to_string(), TokenType::While);
        keywords.insert("for".to_string(), TokenType::For);
        keywords.insert("loop".to_string(), TokenType::Loop);
        keywords.insert("return".to_string(), TokenType::Return);
        keywords.insert("break".to_string(), TokenType::Break);
        keywords.insert("continue".to_string(), TokenType::Continue);
        keywords.insert("struct".to_string(), TokenType::Struct);
        keywords.insert("enum".to_string(), TokenType::Enum);
        keywords.insert("impl".to_string(), TokenType::Impl);
        keywords.insert("trait".to_string(), TokenType::Trait);
        keywords.insert("pub".to_string(), TokenType::Pub);
        keywords.insert("priv".to_string(), TokenType::Priv);
        keywords.insert("mod".to_string(), TokenType::Mod);
        keywords.insert("use".to_string(), TokenType::Use);
        keywords.insert("import".to_string(), TokenType::Import);
        keywords.insert("export".to_string(), TokenType::Export);
        keywords.insert("match".to_string(), TokenType::Match);
        keywords.insert("when".to_string(), TokenType::When);
        keywords.insert("try".to_string(), TokenType::Try);
        keywords.insert("catch".to_string(), TokenType::Catch);
        keywords.insert("finally".to_string(), TokenType::Finally);
        keywords.insert("async".to_string(), TokenType::Async);
        keywords.insert("await".to_string(), TokenType::Await);
        keywords.insert("const".to_string(), TokenType::Const);
        keywords.insert("static".to_string(), TokenType::Static);
        keywords.insert("type".to_string(), TokenType::Type);
        keywords.insert("interface".to_string(), TokenType::Interface);
        keywords.insert("class".to_string(), TokenType::Class);
        keywords.insert("extends".to_string(), TokenType::Extends);
        keywords.insert("implements".to_string(), TokenType::Implements);
        keywords.insert("abstract".to_string(), TokenType::Abstract);
        keywords.insert("override".to_string(), TokenType::Override);
        keywords.insert("virtual".to_string(), TokenType::Virtual);
        keywords.insert("final".to_string(), TokenType::Final);
        
        // Types
        keywords.insert("i8".to_string(), TokenType::I8);
        keywords.insert("i16".to_string(), TokenType::I16);
        keywords.insert("i32".to_string(), TokenType::I32);
        keywords.insert("i64".to_string(), TokenType::I64);
        keywords.insert("u8".to_string(), TokenType::U8);
        keywords.insert("u16".to_string(), TokenType::U16);
        keywords.insert("u32".to_string(), TokenType::U32);
        keywords.insert("u64".to_string(), TokenType::U64);
        keywords.insert("f32".to_string(), TokenType::F32);
        keywords.insert("f64".to_string(), TokenType::F64);
        keywords.insert("bool".to_string(), TokenType::Bool);
        keywords.insert("char".to_string(), TokenType::Char);
        keywords.insert("str".to_string(), TokenType::Str);
        keywords.insert("String".to_string(), TokenType::String);
        keywords.insert("Vec".to_string(), TokenType::Vec);
        keywords.insert("HashMap".to_string(), TokenType::HashMap);
        keywords.insert("HashSet".to_string(), TokenType::HashSet);
        keywords.insert("Option".to_string(), TokenType::Option);
        keywords.insert("Result".to_string(), TokenType::Result);
        keywords.insert("Box".to_string(), TokenType::Box);
        keywords.insert("Rc".to_string(), TokenType::Rc);
        keywords.insert("Arc".to_string(), TokenType::Arc);
        
        // Boolean literals
        keywords.insert("true".to_string(), TokenType::BoolLiteral(true));
        keywords.insert("false".to_string(), TokenType::BoolLiteral(false));
        
        Self {
            input: input.chars().collect(),
            position: 0,
            line: 1,
            column: 1,
            keywords,
        }
    }
    
    pub fn tokenize(&mut self) -> Vec<Token> {
        let mut tokens = Vec::new();
        
        while !self.is_at_end() {
            self.skip_whitespace();
            if self.is_at_end() {
                break;
            }
            
            let token = self.scan_token();
            tokens.push(token);
        }
        
        tokens.push(Token {
            token_type: TokenType::Eof,
            lexeme: "".to_string(),
            line: self.line,
            column: self.column,
        });
        
        tokens
    }
    
    fn is_at_end(&self) -> bool {
        self.position >= self.input.len()
    }
    
    fn current_char(&self) -> Option<char> {
        if self.is_at_end() {
            None
        } else {
            Some(self.input[self.position])
        }
    }
    
    fn peek_char(&self) -> Option<char> {
        if self.position + 1 >= self.input.len() {
            None
        } else {
            Some(self.input[self.position + 1])
        }
    }
    
    fn advance(&mut self) -> Option<char> {
        if self.is_at_end() {
            None
        } else {
            let ch = self.input[self.position];
            self.position += 1;
            
            if ch == '\n' {
                self.line += 1;
                self.column = 1;
            } else {
                self.column += 1;
            }
            
            Some(ch)
        }
    }
    
    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.current_char() {
            match ch {
                ' ' | '\t' | '\r' => {
                    self.advance();
                }
                '\n' => {
                    self.advance();
                }
                _ => break,
            }
        }
    }
    
    fn scan_token(&mut self) -> Token {
        let start_line = self.line;
        let start_column = self.column;
        
        if let Some(ch) = self.advance() {
            let token_type = match ch {
                // Single character tokens
                '(' => TokenType::LeftParen,
                ')' => TokenType::RightParen,
                '{' => TokenType::LeftBrace,
                '}' => TokenType::RightBrace,
                '[' => TokenType::LeftBracket,
                ']' => TokenType::RightBracket,
                ';' => TokenType::Semicolon,
                ',' => TokenType::Comma,
                '.' => TokenType::Dot,
                ':' => {
                    if self.current_char() == Some(':') {
                        self.advance();
                        TokenType::DoubleColon
                    } else {
                        TokenType::Colon
                    }
                }
                '?' => TokenType::Question,
                '@' => TokenType::At,
                '$' => TokenType::Dollar,
                '#' => TokenType::Hash,
                
                // Operators
                '+' => {
                    if self.current_char() == Some('=') {
                        self.advance();
                        TokenType::PlusEqual
                    } else {
                        TokenType::Plus
                    }
                }
                '-' => {
                    if self.current_char() == Some('=') {
                        self.advance();
                        TokenType::MinusEqual
                    } else if self.current_char() == Some('>') {
                        self.advance();
                        TokenType::Arrow
                    } else {
                        TokenType::Minus
                    }
                }
                '*' => {
                    if self.current_char() == Some('=') {
                        self.advance();
                        TokenType::StarEqual
                    } else {
                        TokenType::Star
                    }
                }
                '/' => {
                    if self.current_char() == Some('=') {
                        self.advance();
                        TokenType::SlashEqual
                    } else if self.current_char() == Some('/') {
                        // Line comment
                        return self.scan_line_comment(start_line, start_column);
                    } else if self.current_char() == Some('*') {
                        // Block comment
                        return self.scan_block_comment(start_line, start_column);
                    } else {
                        TokenType::Slash
                    }
                }
                '%' => {
                    if self.current_char() == Some('=') {
                        self.advance();
                        TokenType::PercentEqual
                    } else {
                        TokenType::Percent
                    }
                }
                '=' => {
                    if self.current_char() == Some('=') {
                        self.advance();
                        TokenType::EqualEqual
                    } else if self.current_char() == Some('>') {
                        self.advance();
                        TokenType::FatArrow
                    } else {
                        TokenType::Equal
                    }
                }
                '!' => {
                    if self.current_char() == Some('=') {
                        self.advance();
                        TokenType::NotEqual
                    } else {
                        TokenType::Not
                    }
                }
                '<' => {
                    if self.current_char() == Some('=') {
                        self.advance();
                        TokenType::LessEqual
                    } else if self.current_char() == Some('<') {
                        self.advance();
                        if self.current_char() == Some('=') {
                            self.advance();
                            TokenType::LeftShiftEqual
                        } else {
                            TokenType::LeftShift
                        }
                    } else {
                        TokenType::Less
                    }
                }
                '>' => {
                    if self.current_char() == Some('=') {
                        self.advance();
                        TokenType::GreaterEqual
                    } else if self.current_char() == Some('>') {
                        self.advance();
                        if self.current_char() == Some('=') {
                            self.advance();
                            TokenType::RightShiftEqual
                        } else {
                            TokenType::RightShift
                        }
                    } else {
                        TokenType::Greater
                    }
                }
                '&' => {
                    if self.current_char() == Some('&') {
                        self.advance();
                        TokenType::AndAnd
                    } else if self.current_char() == Some('=') {
                        self.advance();
                        TokenType::AndEqual
                    } else {
                        TokenType::And
                    }
                }
                '|' => {
                    if self.current_char() == Some('|') {
                        self.advance();
                        TokenType::OrOr
                    } else if self.current_char() == Some('=') {
                        self.advance();
                        TokenType::OrEqual
                    } else {
                        TokenType::Or
                    }
                }
                '^' => {
                    if self.current_char() == Some('=') {
                        self.advance();
                        TokenType::XorEqual
                    } else {
                        TokenType::Xor
                    }
                }
                
                // String literals
                '"' => return self.scan_string_literal(start_line, start_column),
                '\'' => return self.scan_char_literal(start_line, start_column),
                
                // Numbers
                '0'..='9' => return self.scan_number(ch, start_line, start_column),
                
                // Identifiers and keywords
                'a'..='z' | 'A'..='Z' | '_' => return self.scan_identifier(ch, start_line, start_column),
                
                // Unicode characters - handle them as identifiers if they're valid identifier starts
                _ if ch.is_alphabetic() || ch == '_' => return self.scan_identifier(ch, start_line, start_column),
                
                // Invalid character
                _ => TokenType::Invalid(ch.to_string()),
            };
            
            Token {
                token_type,
                lexeme: ch.to_string(),
                line: start_line,
                column: start_column,
            }
        } else {
            Token {
                token_type: TokenType::Eof,
                lexeme: "".to_string(),
                line: start_line,
                column: start_column,
            }
        }
    }
    
    fn scan_line_comment(&mut self, start_line: usize, start_column: usize) -> Token {
        self.advance(); // consume the second '/'
        
        let mut comment = String::from("//");
        
        while let Some(ch) = self.current_char() {
            if ch == '\n' {
                break;
            }
            comment.push(ch);
            self.advance();
        }
        
        Token {
            token_type: TokenType::Comment(comment.clone()),
            lexeme: comment,
            line: start_line,
            column: start_column,
        }
    }
    
    fn scan_block_comment(&mut self, start_line: usize, start_column: usize) -> Token {
        self.advance(); // consume the '*'
        
        let mut comment = String::from("/*");
        let mut depth = 1;
        
        while depth > 0 && !self.is_at_end() {
            if let Some(ch) = self.advance() {
                comment.push(ch);
                
                if ch == '*' && self.current_char() == Some('/') {
                    comment.push('/');
                    self.advance();
                    depth -= 1;
                } else if ch == '/' && self.current_char() == Some('*') {
                    comment.push('*');
                    self.advance();
                    depth += 1;
                }
            }
        }
        
        Token {
            token_type: TokenType::Comment(comment.clone()),
            lexeme: comment,
            line: start_line,
            column: start_column,
        }
    }
    
    fn scan_string_literal(&mut self, start_line: usize, start_column: usize) -> Token {
        let mut value = String::new();
        let mut lexeme = String::from("\"");
        
        while let Some(ch) = self.current_char() {
            if ch == '"' {
                lexeme.push(ch);
                self.advance();
                break;
            } else if ch == '\\' {
                lexeme.push(ch);
                self.advance();
                if let Some(escaped) = self.current_char() {
                    lexeme.push(escaped);
                    value.push(match escaped {
                        'n' => '\n',
                        't' => '\t',
                        'r' => '\r',
                        '\\' => '\\',
                        '"' => '"',
                        '\'' => '\'',
                        _ => escaped,
                    });
                    self.advance();
                }
            } else {
                lexeme.push(ch);
                value.push(ch);
                self.advance();
            }
        }
        
        Token {
            token_type: TokenType::StringLiteral(value),
            lexeme,
            line: start_line,
            column: start_column,
        }
    }
    
    fn scan_char_literal(&mut self, start_line: usize, start_column: usize) -> Token {
        let mut value = String::new();
        let mut lexeme = String::from("'");
        
        if let Some(ch) = self.current_char() {
            lexeme.push(ch);
            if ch == '\\' {
                self.advance();
                if let Some(escaped) = self.current_char() {
                    lexeme.push(escaped);
                    value.push(match escaped {
                        'n' => '\n',
                        't' => '\t',
                        'r' => '\r',
                        '\\' => '\\',
                        '"' => '"',
                        '\'' => '\'',
                        _ => escaped,
                    });
                    self.advance();
                }
            } else {
                value.push(ch);
                self.advance();
            }
        }
        
        if self.current_char() == Some('\'') {
            lexeme.push('\'');
            self.advance();
        }
        
        Token {
            token_type: TokenType::CharLiteral(value),
            lexeme,
            line: start_line,
            column: start_column,
        }
    }
    
    fn scan_number(&mut self, first_digit: char, start_line: usize, start_column: usize) -> Token {
        let mut lexeme = String::from(first_digit);
        let mut is_float = false;
        
        // Consume remaining digits
        while let Some(ch) = self.current_char() {
            if ch.is_ascii_digit() {
                lexeme.push(ch);
                self.advance();
            } else if ch == '.' && !is_float && self.peek_char().map_or(false, |c| c.is_ascii_digit()) {
                is_float = true;
                lexeme.push(ch);
                self.advance();
            } else {
                break;
            }
        }
        
        let token_type = if is_float {
            TokenType::FloatLiteral(lexeme.clone())
        } else {
            TokenType::IntegerLiteral(lexeme.clone())
        };
        
        Token {
            token_type,
            lexeme,
            line: start_line,
            column: start_column,
        }
    }
    
    fn scan_identifier(&mut self, first_char: char, start_line: usize, start_column: usize) -> Token {
        let mut lexeme = String::from(first_char);
        
        while let Some(ch) = self.current_char() {
            if ch.is_alphanumeric() || ch == '_' {
                lexeme.push(ch);
                self.advance();
            } else {
                break;
            }
        }
        
        let token_type = self.keywords.get(&lexeme)
            .cloned()
            .unwrap_or_else(|| TokenType::Identifier(lexeme.clone()));
        
        Token {
            token_type,
            lexeme,
            line: start_line,
            column: start_column,
        }
    }
}

// Benchmark functions
pub fn benchmark_lexer_real_world() -> Result<(), Box<dyn std::error::Error>> {
    let test_files = vec![
        "../../test_data/large_codebases/large_codebase.seen",
        "../../test_data/large_codebases/minified_code.seen", 
        "../../test_data/large_codebases/sparse_code.seen",
        "../../test_data/large_codebases/unicode_heavy.seen",
    ];
    
    let mut total_tokens = 0u64;
    let mut total_time = Duration::new(0, 0);
    
    for file_path in test_files {
        if Path::new(file_path).exists() {
            let content = fs::read_to_string(file_path)?;
            let file_size = content.len();
            
            println!("Testing Rust lexer performance on {} ({} bytes)", file_path, file_size);
            
            // Run multiple iterations for statistical accuracy
            let iterations = 10;
            let mut file_tokens = 0;
            let mut file_time = Duration::new(0, 0);
            
            for _ in 0..iterations {
                let mut lexer = Lexer::new(&content);
                
                let start_time = Instant::now();
                let tokens = lexer.tokenize();
                let elapsed = start_time.elapsed();
                
                file_tokens = tokens.len();
                file_time += elapsed;
            }
            
            let avg_time = file_time / iterations as u32;
            let tokens_per_second = if avg_time.as_secs_f64() > 0.0 {
                file_tokens as f64 / avg_time.as_secs_f64()
            } else {
                0.0
            };
            
            println!("  Tokens: {}, Avg Time: {:?}, Tokens/sec: {:.0}", 
                     file_tokens, avg_time, tokens_per_second);
            
            total_tokens += file_tokens as u64;
            total_time += avg_time;
        } else {
            println!("Warning: Test file {} not found, skipping...", file_path);
        }
    }
    
    let overall_tokens_per_sec = if total_time.as_secs_f64() > 0.0 {
        total_tokens as f64 / total_time.as_secs_f64()
    } else {
        0.0
    };
    
    println!("Rust Lexer Overall Performance:");
    println!("  Total tokens: {}", total_tokens);
    println!("  Total time: {:?}", total_time);
    println!("  Average tokens/second: {:.0}", overall_tokens_per_sec);
    
    // Check if it meets the 14M tokens/sec claim
    if overall_tokens_per_sec >= 14_000_000.0 {
        println!("✅ RUST BASELINE: Achieved {:.1}M tokens/sec", overall_tokens_per_sec / 1_000_000.0);
    } else {
        println!("❌ RUST BASELINE: Achieved {:.1}M tokens/sec (target: 14M)", overall_tokens_per_sec / 1_000_000.0);
    }
    
    Ok(())
}

fn main() {
    if let Err(e) = benchmark_lexer_real_world() {
        eprintln!("Error running Rust lexer benchmark: {}", e);
        std::process::exit(1);
    }
}