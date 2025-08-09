// Rust Lexer Benchmark Implementation
use std::fs;
use std::time::Instant;
use std::env;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq)]
enum TokenType {
    Keyword,
    Identifier,
    Number,
    String,
    Operator,
    Punctuation,
    Comment,
    Whitespace,
    Eof,
}

#[derive(Debug, Clone)]
struct Token {
    token_type: TokenType,
    value: String,
    line: usize,
    column: usize,
}

struct Lexer {
    source: Vec<char>,
    pos: usize,
    line: usize,
    column: usize,
    keywords: HashSet<String>,
}

impl Lexer {
    fn new(source: String) -> Self {
        let mut keywords = HashSet::new();
        for kw in &[
            "fun", "val", "var", "if", "else", "when", "for", "while",
            "class", "interface", "object", "return", "break", "continue",
            "true", "false", "null", "this", "super", "import", "package",
        ] {
            keywords.insert(kw.to_string());
        }

        Lexer {
            source: source.chars().collect(),
            pos: 0,
            line: 1,
            column: 1,
            keywords,
        }
    }

    fn current(&self) -> Option<char> {
        if self.pos < self.source.len() {
            Some(self.source[self.pos])
        } else {
            None
        }
    }

    fn peek(&self, offset: usize) -> Option<char> {
        let next_pos = self.pos + offset;
        if next_pos < self.source.len() {
            Some(self.source[next_pos])
        } else {
            None
        }
    }

    fn advance(&mut self) {
        if self.pos < self.source.len() {
            if self.source[self.pos] == '\n' {
                self.line += 1;
                self.column = 1;
            } else {
                self.column += 1;
            }
            self.pos += 1;
        }
    }

    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.current() {
            if ch.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    fn scan_identifier(&mut self) -> Token {
        let start = self.pos;
        let start_col = self.column;

        while let Some(ch) = self.current() {
            if ch.is_alphanumeric() || ch == '_' {
                self.advance();
            } else {
                break;
            }
        }

        let value: String = self.source[start..self.pos].iter().collect();
        let token_type = if self.keywords.contains(&value) {
            TokenType::Keyword
        } else {
            TokenType::Identifier
        };

        Token {
            token_type,
            value,
            line: self.line,
            column: start_col,
        }
    }

    fn scan_number(&mut self) -> Token {
        let start = self.pos;
        let start_col = self.column;

        while let Some(ch) = self.current() {
            if ch.is_numeric() {
                self.advance();
            } else {
                break;
            }
        }

        if self.current() == Some('.') && self.peek(1).map_or(false, |c| c.is_numeric()) {
            self.advance(); // consume '.'
            while let Some(ch) = self.current() {
                if ch.is_numeric() {
                    self.advance();
                } else {
                    break;
                }
            }
        }

        let value: String = self.source[start..self.pos].iter().collect();
        Token {
            token_type: TokenType::Number,
            value,
            line: self.line,
            column: start_col,
        }
    }

    fn scan_string(&mut self) -> Token {
        let start = self.pos;
        let start_col = self.column;
        let quote = self.current().unwrap();
        self.advance(); // consume opening quote

        while let Some(ch) = self.current() {
            if ch == quote {
                self.advance(); // consume closing quote
                break;
            } else if ch == '\\' {
                self.advance(); // skip escape char
                if self.current().is_some() {
                    self.advance(); // skip escaped char
                }
            } else {
                self.advance();
            }
        }

        let value: String = self.source[start..self.pos].iter().collect();
        Token {
            token_type: TokenType::String,
            value,
            line: self.line,
            column: start_col,
        }
    }

    fn scan_comment(&mut self) -> Token {
        let start = self.pos;
        let start_col = self.column;

        if self.current() == Some('/') && self.peek(1) == Some('/') {
            // Single-line comment
            while let Some(ch) = self.current() {
                if ch == '\n' {
                    break;
                }
                self.advance();
            }
        } else if self.current() == Some('/') && self.peek(1) == Some('*') {
            // Multi-line comment
            self.advance(); // consume /
            self.advance(); // consume *
            while self.current().is_some() {
                if self.current() == Some('*') && self.peek(1) == Some('/') {
                    self.advance(); // consume *
                    self.advance(); // consume /
                    break;
                }
                self.advance();
            }
        }

        let value: String = self.source[start..self.pos].iter().collect();
        Token {
            token_type: TokenType::Comment,
            value,
            line: self.line,
            column: start_col,
        }
    }

    fn tokenize(&mut self) -> Vec<Token> {
        let mut tokens = Vec::new();

        while self.pos < self.source.len() {
            self.skip_whitespace();

            if self.pos >= self.source.len() {
                break;
            }

            if let Some(ch) = self.current() {
                if ch.is_alphabetic() || ch == '_' {
                    tokens.push(self.scan_identifier());
                } else if ch.is_numeric() {
                    tokens.push(self.scan_number());
                } else if ch == '"' || ch == '\'' {
                    tokens.push(self.scan_string());
                } else if ch == '/' && (self.peek(1) == Some('/') || self.peek(1) == Some('*')) {
                    tokens.push(self.scan_comment());
                } else {
                    // Operators and punctuation
                    let start_col = self.column;
                    let mut op = String::from(ch);
                    self.advance();

                    // Check for multi-character operators
                    if (ch == '=' || ch == '!' || ch == '<' || ch == '>') 
                        && self.current() == Some('=') {
                        op.push(self.current().unwrap());
                        self.advance();
                    } else if (ch == '&' && self.current() == Some('&')) 
                           || (ch == '|' && self.current() == Some('|')) {
                        op.push(self.current().unwrap());
                        self.advance();
                    }

                    tokens.push(Token {
                        token_type: TokenType::Operator,
                        value: op,
                        line: self.line,
                        column: start_col,
                    });
                }
            }
        }

        tokens.push(Token {
            token_type: TokenType::Eof,
            value: String::new(),
            line: self.line,
            column: self.column,
        });

        tokens
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    
    if args.len() < 2 {
        eprintln!("Usage: {} <input_file> [iterations]", args[0]);
        std::process::exit(1);
    }
    
    let filename = &args[1];
    let iterations = if args.len() > 2 {
        args[2].parse().unwrap_or(1)
    } else {
        1
    };
    
    // Read file
    let source = fs::read_to_string(filename)
        .expect(&format!("Error: Cannot read file {}", filename));
    
    // Warmup
    for _ in 0..5 {
        let mut warmup_lexer = Lexer::new(source.clone());
        let _ = warmup_lexer.tokenize();
    }
    
    // Benchmark
    let mut times = Vec::new();
    let mut total_tokens = 0;
    
    for _ in 0..iterations {
        let start = Instant::now();
        
        let mut lexer = Lexer::new(source.clone());
        let tokens = lexer.tokenize();
        
        let duration = start.elapsed();
        
        times.push(duration.as_secs_f64());
        total_tokens = tokens.len();
    }
    
    // Output results in JSON format
    println!("{{");
    println!("  \"language\": \"rust\",");
    println!("  \"benchmark\": \"lexer\",");
    println!("  \"iterations\": {},", iterations);
    println!("  \"tokens_processed\": {},", total_tokens);
    print!("  \"times\": [");
    
    for (i, time) in times.iter().enumerate() {
        print!("{}", time);
        if i < times.len() - 1 {
            print!(", ");
        }
    }
    
    println!("],");
    
    // Calculate statistics
    let sum: f64 = times.iter().sum();
    let avg = sum / times.len() as f64;
    
    let tokens_per_sec = total_tokens as f64 / avg;
    
    println!("  \"average_time\": {},", avg);
    println!("  \"tokens_per_second\": {}", tokens_per_sec);
    println!("}}");
}