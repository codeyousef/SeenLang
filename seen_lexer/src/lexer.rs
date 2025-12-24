//! Main lexer implementation

use crate::{
    error::{LexerError, LexerResult},
    keyword_manager::{KeywordManager, KeywordType},
    position::Position,
    token::{InterpolationKind, InterpolationPart, Token, TokenType},
};
use std::sync::Arc;
use unicode_normalization::UnicodeNormalization;

/// Controls how identifier visibility is determined.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisibilityPolicy {
    /// Visibility follows capitalization (upper-case identifiers are public).
    Caps,
    /// Visibility requires explicit keywords (e.g. `pub`); identifiers default to private.
    Explicit,
}

/// Lexer configuration options.
#[derive(Debug, Clone, Copy)]
pub struct LexerConfig {
    pub visibility_policy: VisibilityPolicy,
}

impl Default for LexerConfig {
    fn default() -> Self {
        Self {
            visibility_policy: VisibilityPolicy::Caps,
        }
    }
}

pub struct Lexer {
    keyword_manager: Arc<KeywordManager>,
    input: String,
    position: usize,
    current_char: Option<char>,
    pos_tracker: Position,
    config: LexerConfig,
}

impl Lexer {
    pub fn new(input: String, keyword_manager: Arc<KeywordManager>) -> Self {
        Self::with_config(input, keyword_manager, LexerConfig::default())
    }

    pub fn with_config(
        input: String,
        keyword_manager: Arc<KeywordManager>,
        config: LexerConfig,
    ) -> Self {
        let mut lexer = Self {
            keyword_manager,
            input,
            position: 0,
            current_char: None,
            pos_tracker: Position::start(),
            config,
        };
        lexer.current_char = lexer.input.chars().next();
        lexer
    }

    /// Get the keyword manager used by this lexer
    pub fn keyword_manager(&self) -> Arc<KeywordManager> {
        self.keyword_manager.clone()
    }

    /// Return the lexer's configuration.
    pub fn config(&self) -> &LexerConfig {
        &self.config
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

            Some('"') => {
                // Check for triple-quoted string
                if self.peek() == Some('"') && self.peek_ahead(2) == Some('"') {
                    self.read_multiline_string_literal()
                } else {
                    self.read_string_literal()
                }
            }

            Some('\'') => self.read_char_literal(),

            Some(ch) if self.is_return_type_label_start(ch) && self.peek() == Some(':') => {
                self.read_return_type_label()
            }

            Some(ch) if self.is_identifier_start(ch) => self.read_identifier(),

            // Mathematical and assignment operators
            Some('+') => {
                self.advance();
                if self.current_char == Some('=') {
                    self.advance();
                    Ok(Token::new(
                        TokenType::PlusAssign,
                        "+=".to_string(),
                        start_pos,
                    ))
                } else {
                    Ok(Token::new(TokenType::Plus, "+".to_string(), start_pos))
                }
            }
            Some('-') => {
                self.advance();
                if self.current_char == Some('>') {
                    self.advance();
                    Ok(Token::new(TokenType::Arrow, "->".to_string(), start_pos))
                } else if self.current_char == Some('=') {
                    self.advance();
                    Ok(Token::new(
                        TokenType::MinusAssign,
                        "-=".to_string(),
                        start_pos,
                    ))
                } else {
                    Ok(Token::new(TokenType::Minus, "-".to_string(), start_pos))
                }
            }
            Some('*') => {
                self.advance();
                if self.current_char == Some('=') {
                    self.advance();
                    Ok(Token::new(
                        TokenType::MultiplyAssign,
                        "*=".to_string(),
                        start_pos,
                    ))
                } else {
                    Ok(Token::new(TokenType::Multiply, "*".to_string(), start_pos))
                }
            }
            Some('/') => {
                self.advance();
                if self.current_char == Some('/') {
                    // Single-line comment
                    self.advance();
                    let comment = self.read_single_line_comment();
                    Ok(Token::new(
                        TokenType::SingleLineComment(comment.clone()),
                        comment,
                        start_pos,
                    ))
                } else if self.current_char == Some('*') {
                    // Multi-line comment
                    self.advance();
                    let comment = self.read_multi_line_comment()?;
                    if comment.starts_with("/**") {
                        Ok(Token::new(
                            TokenType::DocComment(comment.clone()),
                            comment,
                            start_pos,
                        ))
                    } else {
                        Ok(Token::new(
                            TokenType::MultiLineComment(comment.clone()),
                            comment,
                            start_pos,
                        ))
                    }
                } else if self.current_char == Some('=') {
                    self.advance();
                    Ok(Token::new(
                        TokenType::DivideAssign,
                        "/=".to_string(),
                        start_pos,
                    ))
                } else {
                    Ok(Token::new(TokenType::Divide, "/".to_string(), start_pos))
                }
            }
            Some('%') => {
                self.advance();
                if self.current_char == Some('=') {
                    self.advance();
                    Ok(Token::new(
                        TokenType::ModuloAssign,
                        "%=".to_string(),
                        start_pos,
                    ))
                } else {
                    Ok(Token::new(TokenType::Modulo, "%".to_string(), start_pos))
                }
            }

            // Comparison and assignment operators
            Some('=') => {
                self.advance();
                if self.current_char == Some('=') {
                    self.advance();
                    Ok(Token::new(TokenType::Equal, "==".to_string(), start_pos))
                } else if self.current_char == Some('>') {
                    self.advance();
                    Ok(Token::new(TokenType::FatArrow, "=>".to_string(), start_pos))
                } else {
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
                    Ok(Token::new(
                        TokenType::ForceUnwrap,
                        "!!".to_string(),
                        start_pos,
                    ))
                } else {
                    self.advance();
                    Ok(Token::new(
                        TokenType::LogicalNot,
                        "!".to_string(),
                        start_pos,
                    ))
                }
            }
            Some('<') => {
                self.advance();
                if self.current_char == Some('=') {
                    self.advance();
                    Ok(Token::new(
                        TokenType::LessEqual,
                        "<=".to_string(),
                        start_pos,
                    ))
                } else if self.current_char == Some('<') {
                    self.advance();
                    Ok(Token::new(
                        TokenType::LeftShift,
                        "<<".to_string(),
                        start_pos,
                    ))
                } else {
                    Ok(Token::new(TokenType::Less, "<".to_string(), start_pos))
                }
            }
            Some('>') => {
                self.advance();
                if self.current_char == Some('=') {
                    self.advance();
                    Ok(Token::new(
                        TokenType::GreaterEqual,
                        ">=".to_string(),
                        start_pos,
                    ))
                } else if self.current_char == Some('>') {
                    self.advance();
                    Ok(Token::new(
                        TokenType::RightShift,
                        ">>".to_string(),
                        start_pos,
                    ))
                } else {
                    Ok(Token::new(TokenType::Greater, ">".to_string(), start_pos))
                }
            }

            // Nullable operators
            Some('?') => {
                if self.peek() == Some('.') {
                    self.advance();
                    self.advance();
                    Ok(Token::new(
                        TokenType::SafeNavigation,
                        "?.".to_string(),
                        start_pos,
                    ))
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
                        Ok(Token::new(
                            TokenType::ExclusiveRange,
                            "..<".to_string(),
                            start_pos,
                        ))
                    } else {
                        Ok(Token::new(
                            TokenType::InclusiveRange,
                            "..".to_string(),
                            start_pos,
                        ))
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
                Ok(Token::new(
                    TokenType::RightParen,
                    ")".to_string(),
                    start_pos,
                ))
            }
            Some('{') => {
                self.advance();
                Ok(Token::new(TokenType::LeftBrace, "{".to_string(), start_pos))
            }
            Some('}') => {
                self.advance();
                Ok(Token::new(
                    TokenType::RightBrace,
                    "}".to_string(),
                    start_pos,
                ))
            }
            Some('[') => {
                self.advance();
                Ok(Token::new(
                    TokenType::LeftBracket,
                    "[".to_string(),
                    start_pos,
                ))
            }
            Some(']') => {
                self.advance();
                Ok(Token::new(
                    TokenType::RightBracket,
                    "]".to_string(),
                    start_pos,
                ))
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
                if self.current_char == Some(':') {
                    self.advance();
                    Ok(Token::new(
                        TokenType::DoubleColon,
                        "::".to_string(),
                        start_pos,
                    ))
                } else {
                    Ok(Token::new(TokenType::Colon, ":".to_string(), start_pos))
                }
            }

            // Bitwise operators
            Some('&') => {
                self.advance();
                Ok(Token::new(
                    TokenType::BitwiseAnd,
                    "&".to_string(),
                    start_pos,
                ))
            }
            Some('|') => {
                self.advance();
                Ok(Token::new(TokenType::BitwiseOr, "|".to_string(), start_pos))
            }
            Some('^') => {
                self.advance();
                Ok(Token::new(
                    TokenType::BitwiseXor,
                    "^".to_string(),
                    start_pos,
                ))
            }
            Some('~') => {
                self.advance();
                Ok(Token::new(
                    TokenType::BitwiseNot,
                    "~".to_string(),
                    start_pos,
                ))
            }

            // Special characters
            Some('_') => {
                self.advance();
                Ok(Token::new(
                    TokenType::Underscore,
                    "_".to_string(),
                    start_pos,
                ))
            }
            Some('@') => {
                self.advance();
                Ok(Token::new(TokenType::At, "@".to_string(), start_pos))
            }
            Some('#') => {
                self.advance();
                Ok(Token::new(TokenType::Hash, "#".to_string(), start_pos))
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
        match self.config.visibility_policy {
            VisibilityPolicy::Caps => {
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
            VisibilityPolicy::Explicit => TokenType::PrivateIdentifier(text.to_string()),
        }
    }

    pub fn check_keyword(&self, text: &str) -> Option<TokenType> {
        // Use dynamic keyword lookup - NO HARDCODING!
        if let Some(keyword_type) = self.keyword_manager.is_keyword(text) {
            // Special handling for boolean literals
            match keyword_type {
                KeywordType::KeywordTrue => Some(TokenType::BoolLiteral(true)),
                KeywordType::KeywordFalse => Some(TokenType::BoolLiteral(false)),
                // RESEARCH-BASED: Word-based logical operators (Stefik & Siebert 2013)
                KeywordType::KeywordAnd => Some(TokenType::LogicalAnd),
                KeywordType::KeywordOr => Some(TokenType::LogicalOr),
                KeywordType::KeywordNot => Some(TokenType::LogicalNot),
                // VALE-STYLE: Memory management operators as first-class tokens
                KeywordType::KeywordMove => Some(TokenType::Move),
                KeywordType::KeywordBorrow => Some(TokenType::Borrow),
                KeywordType::KeywordInout => Some(TokenType::Inout),
                // All other keywords use the dynamic Keyword variant
                _ => Some(TokenType::Keyword(keyword_type)),
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

    fn peek_ahead(&self, offset: usize) -> Option<char> {
        let mut pos = self.position;
        let mut chars_ahead = 0;

        // Skip current character first
        if let Some(current_char) = self.current_char {
            pos += current_char.len_utf8();
            chars_ahead += 1;
        }

        // Skip additional characters until we reach the desired offset
        while chars_ahead <= offset && pos < self.input.len() {
            let remaining = &self.input[pos..];
            if let Some(ch) = remaining.chars().next() {
                if chars_ahead == offset {
                    return Some(ch);
                }
                pos += ch.len_utf8();
                chars_ahead += 1;
            } else {
                break;
            }
        }

        None
    }

    fn is_basic_identifier_start(ch: char) -> bool {
        ch == '_' || ch.is_ascii_alphabetic()
    }

    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.current_char {
            if ch.is_whitespace() && ch != '\n' {
                self.advance();
            } else if ch == '/' && self.peek() == Some('/') {
                // Skip line comment
                self.skip_line_comment();
            } else if ch == '/' && self.peek() == Some('*') {
                // Skip block comment
                self.skip_block_comment();
            } else {
                break;
            }
        }
    }

    fn skip_line_comment(&mut self) {
        // Skip //
        self.advance();
        self.advance();

        // Skip until end of line or file
        while let Some(ch) = self.current_char {
            if ch == '\n' {
                break;
            }
            self.advance();
        }
    }

    fn skip_block_comment(&mut self) {
        // Skip /*
        self.advance();
        self.advance();

        // Skip until */
        while let Some(ch) = self.current_char {
            if ch == '*' && self.peek() == Some('/') {
                self.advance(); // skip *
                self.advance(); // skip /
                break;
            }
            self.advance();
        }
    }

    fn is_identifier_start(&self, ch: char) -> bool {
        // Unicode-aware identifier start:
        // - Letters (including Unicode letters)
        // - Underscore
        // - Unicode characters that aren't operators, punctuation, or whitespace
        ch.is_alphabetic()
            || ch == '_'
            || (!ch.is_ascii()
                && !ch.is_numeric()
                && !ch.is_whitespace()
                && !self.is_operator_char(ch)
                && !self.is_punctuation_char(ch))
    }

    fn is_identifier_continue(&self, ch: char) -> bool {
        // Unicode-aware identifier continuation:
        // - Letters and digits (including Unicode)
        // - Underscore
        // - Unicode marks (combining characters)
        ch.is_alphanumeric()
            || ch == '_'
            || (!ch.is_ascii()
                && !ch.is_whitespace()
                && !self.is_operator_char(ch)
                && !self.is_punctuation_char(ch))
    }

    fn return_type_label(&self) -> String {
        // English defaults to "r:", Arabic locale switches to the locale-specific label ("\u0646:").
        let lang = self.keyword_manager.get_current_language();
        if lang == "ar" {
            "\u{646}:".to_string()
        } else {
            "r:".to_string()
        }
    }

    fn is_return_type_label_start(&self, ch: char) -> bool {
        self.return_type_label().chars().next().map_or(false, |expected| ch == expected)
    }

    fn read_return_type_label(&mut self) -> LexerResult<Token> {
        let start_pos = self.pos_tracker;
        let label = self.return_type_label();

        // Consume the leading marker character (e.g., 'r' or 'ن')
        self.advance();

        // Expect the ':' component of the label
        if self.current_char != Some(':') {
            return Err(LexerError::UnexpectedCharacter {
                character: self.current_char.unwrap_or('\0'),
                position: start_pos,
            });
        }

        // Consume ':'
        self.advance();

        // Require whitespace after the label to avoid "r:i32" ambiguity
        if self.current_char.map_or(true, |ch| !ch.is_whitespace()) {
            return Err(LexerError::MissingSpaceAfterReturnLabel {
                position: start_pos,
                label,
            });
        }

        Ok(Token::new(TokenType::ReturnTypeLabel, self.return_type_label(), start_pos))
    }

    fn read_single_line_comment(&mut self) -> String {
        let mut comment = String::from("//");

        while let Some(ch) = self.current_char {
            if ch == '\n' {
                break;
            }
            comment.push(ch);
            self.advance();
        }

        comment
    }

    fn read_multi_line_comment(&mut self) -> LexerResult<String> {
        let mut comment = String::from("/*");
        let start_pos = self.pos_tracker;
        let is_doc = self.current_char == Some('*');

        if is_doc {
            comment.push('*');
            self.advance();
        }

        let mut last_was_star = false;

        while let Some(ch) = self.current_char {
            comment.push(ch);

            if last_was_star && ch == '/' {
                self.advance();
                return Ok(comment);
            }

            last_was_star = ch == '*';
            self.advance();
        }

        Err(LexerError::UnterminatedComment {
            position: start_pos,
        })
    }

    fn is_operator_char(&self, ch: char) -> bool {
        // Check if character is an operator that we handle explicitly
        matches!(
            ch,
            '+' | '-' | '*' | '/' | '%' | '=' | '!' | '<' | '>' | '?' | '.' | ':'
        )
    }

    fn is_punctuation_char(&self, ch: char) -> bool {
        // Check if character is punctuation that we handle explicitly
        matches!(
            ch,
            '(' | ')' | '{' | '}' | '[' | ']' | ',' | ';' | '"' | '\'' | '\\'
        )
    }

    fn read_number(&mut self) -> LexerResult<Token> {
        let start_pos = self.pos_tracker;
        let mut number_str = String::new();
        let mut is_float = false;
        let mut is_unsigned = false;

        // Hexadecimal literal support (0xFF or 0XFF, optional 'u' suffix)
        if self.current_char == Some('0') {
            if let Some(peek_char) = self.peek() {
                if matches!(peek_char, 'x' | 'X') {
                    let mut lexeme = String::from("0");
                    lexeme.push(peek_char);
                    self.advance(); // consume '0'
                    self.advance(); // consume 'x' or 'X'

                    let mut digits = String::new();
                    while let Some(ch) = self.current_char {
                        if ch.is_ascii_hexdigit() {
                            digits.push(ch);
                            lexeme.push(ch);
                            self.advance();
                        } else {
                            break;
                        }
                    }

                    if digits.is_empty() {
                        return Err(LexerError::InvalidNumber {
                            position: start_pos,
                            message: "Expected hexadecimal digits after 0x prefix".to_string(),
                        });
                    }

                    let mut is_unsigned_hex = false;
                    if self.current_char == Some('u') {
                        is_unsigned_hex = true;
                        lexeme.push('u');
                        self.advance();
                    }

                    if is_unsigned_hex {
                        let value = u64::from_str_radix(&digits, 16).map_err(|_| {
                            LexerError::InvalidNumber {
                                position: start_pos,
                                message: "Invalid unsigned hexadecimal literal".to_string(),
                            }
                        })?;
                        return Ok(Token::new(
                            TokenType::UIntegerLiteral(value),
                            lexeme,
                            start_pos,
                        ));
                    } else {
                        let value = i64::from_str_radix(&digits, 16).map_err(|_| {
                            LexerError::InvalidNumber {
                                position: start_pos,
                                message: "Invalid hexadecimal literal".to_string(),
                            }
                        })?;
                        return Ok(Token::new(
                            TokenType::IntegerLiteral(value),
                            lexeme,
                            start_pos,
                        ));
                    }
                }
            }
        }

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

        // Check for scientific notation (e or E)
        if let Some('e') | Some('E') = self.current_char {
            is_float = true;
            number_str.push(self.current_char.unwrap());
            self.advance();

            // Optional +/- sign
            if let Some('+') | Some('-') = self.current_char {
                number_str.push(self.current_char.unwrap());
                self.advance();
            }

            // Exponent digits (required)
            let exp_start = number_str.len();
            while let Some(ch) = self.current_char {
                if ch.is_ascii_digit() {
                    number_str.push(ch);
                    self.advance();
                } else {
                    break;
                }
            }

            // Validate we got at least one exponent digit
            if number_str.len() == exp_start {
                return Err(LexerError::InvalidNumber {
                    position: start_pos,
                    message: "Expected digits after exponent indicator (e/E)".to_string(),
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
            let value: f64 = number_str.parse().map_err(|_| LexerError::InvalidNumber {
                position: start_pos,
                message: "Invalid float format".to_string(),
            })?;
            Ok(Token::new(
                TokenType::FloatLiteral(value),
                number_str,
                start_pos,
            ))
        } else if is_unsigned {
            let number_part = &number_str[..number_str.len() - 1]; // Remove 'u' suffix
            let value: u64 = number_part.parse().map_err(|_| LexerError::InvalidNumber {
                position: start_pos,
                message: "Invalid unsigned integer format".to_string(),
            })?;
            Ok(Token::new(
                TokenType::UIntegerLiteral(value),
                number_str,
                start_pos,
            ))
        } else {
            let value: i64 = number_str.parse().map_err(|_| LexerError::InvalidNumber {
                position: start_pos,
                message: "Invalid integer format".to_string(),
            })?;
            Ok(Token::new(
                TokenType::IntegerLiteral(value),
                number_str,
                start_pos,
            ))
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
                    // Add any remaining text only if there is actual text content
                    if !current_text.is_empty() {
                        let normalized_text = current_text.nfc().collect::<String>();
                        // Adjust position for text positioning
                        let mut final_text_pos = text_start_pos;
                        if current_text.starts_with('\n') && current_text.len() > 1 {
                            // Text starts with newline - position should be after the newline for meaningful content
                            final_text_pos.line += 1;
                            final_text_pos.column = 1;
                        } else if !current_text.starts_with('\n')
                            && text_start_pos.column > 1
                            && !parts.is_empty()
                        {
                            // Single-line text after interpolation - adjust to closing brace position
                            final_text_pos.column -= 1;
                        }

                        parts.push(InterpolationPart {
                            kind: InterpolationKind::Text(normalized_text.clone()),
                            content: normalized_text,
                            position: final_text_pos,
                        });
                    }
                    return Ok(Token::new(
                        TokenType::InterpolatedString(parts),
                        lexeme,
                        start_pos,
                    ));
                } else {
                    let normalized_text = current_text.nfc().collect::<String>();
                    return Ok(Token::new(
                        TokenType::StringLiteral(normalized_text),
                        lexeme,
                        start_pos,
                    ));
                }
            } else if ch == '{' {
                // Peek ahead to determine if this should start interpolation
                let brace_pos = self.pos_tracker; // Position of the '{'
                match self.peek() {
                    Some('{') => {
                        // Escaped opening brace `{{`
                        self.advance(); // consume first '{'
                        self.advance(); // consume second '{'
                        current_text.push('{');
                        lexeme.push_str("{{");
                    }
                    Some(next) if Self::is_basic_identifier_start(next) => {
                        self.advance(); // consume '{'
                        has_interpolation = true;

                        // Save current text part if any
                        if !current_text.is_empty() {
                            let normalized_text = current_text.nfc().collect::<String>();
                            // Adjust position for text positioning
                            let mut final_text_pos = text_start_pos;
                            if current_text.starts_with('\n') && current_text.len() > 1 {
                                final_text_pos.line += 1;
                                final_text_pos.column = 1;
                            } else if !current_text.starts_with('\n')
                                && text_start_pos.column > 1
                                && !parts.is_empty()
                            {
                                final_text_pos.column -= 1;
                            }

                            parts.push(InterpolationPart {
                                kind: InterpolationKind::Text(normalized_text.clone()),
                                content: normalized_text,
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
                        text_start_pos = self.pos_tracker;

                        lexeme.push_str(&format!("{{...}}"));
                    }
                    _ => {
                        // Treat as literal '{' when it is not followed by an identifier start
                        self.advance();
                        current_text.push('{');
                        lexeme.push('{');
                    }
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
            match ch {
                '{' => {
                    brace_depth += 1;
                    expr.push(ch);
                    self.advance();
                }
                '}' => {
                    brace_depth -= 1;
                    if brace_depth == 0 {
                        // End of interpolation - advance past the closing '}'
                        self.advance();
                        return Ok(expr);
                    } else {
                        expr.push(ch);
                        self.advance();
                    }
                }
                '"' => {
                    // Handle strings within interpolation
                    expr.push(ch);
                    self.advance();
                    self.read_string_in_interpolation(&mut expr)?;
                }
                '\\' => {
                    // Allow escaping quotes/backslashes/braces inside interpolation expressions so authors
                    // can embed them without confusing the outer string literal.
                    self.advance();
                    if let Some(next) = self.current_char {
                        match next {
                            '"' | '\\' | '{' | '}' => {
                                expr.push(next);
                                self.advance();
                            }
                            _ => {
                                // Preserve unknown escapes verbatim (the sub-parser will diagnose them if needed)
                                expr.push('\\');
                                expr.push(next);
                                self.advance();
                            }
                        }
                    } else {
                        // Trailing backslash - treat it literally
                        expr.push('\\');
                    }
                }
                _ => {
                    expr.push(ch);
                    self.advance();
                }
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

        let normalized_char_string = char_value.to_string().nfc().collect::<String>();
        let mut normalized_chars = normalized_char_string.chars();
        let normalized_char = normalized_chars.next().unwrap_or(char_value);
        if normalized_chars.next().is_some() {
            return Err(LexerError::InvalidUnicodeEscape {
                position: start_pos,
            });
        }

        // Expect closing quote
        if self.current_char == Some('\'') {
            lexeme.push('\'');
            self.advance();
            Ok(Token::new(
                TokenType::CharLiteral(normalized_char),
                lexeme,
                start_pos,
            ))
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
        let code_point =
            u32::from_str_radix(&hex_digits, 16).map_err(|_| LexerError::InvalidUnicodeEscape {
                position: self.pos_tracker,
            })?;

        // Convert to char
        char::from_u32(code_point).ok_or(LexerError::InvalidUnicodeEscape {
            position: self.pos_tracker,
        })
    }

    fn read_multiline_string_literal(&mut self) -> LexerResult<Token> {
        let start_pos = self.pos_tracker;
        let mut parts = Vec::new();
        let mut current_text = String::new();
        let has_interpolation = false;
        let mut lexeme = String::new();

        // Skip opening triple quotes
        lexeme.push_str("\"\"\"");
        self.advance(); // first "
        self.advance(); // second "
        self.advance(); // third "

        let text_start_pos = self.pos_tracker; // Position after opening quotes

        while let Some(ch) = self.current_char {
            // Check for closing triple quotes
            if ch == '"' && self.peek() == Some('"') && self.peek_ahead(2) == Some('"') {
                // End of multiline string
                lexeme.push_str("\"\"\"");
                self.advance(); // first "
                self.advance(); // second "
                self.advance(); // third "

                if has_interpolation {
                    // Add any remaining text only if there is actual text content
                    if !current_text.is_empty() {
                        let normalized_text = current_text.nfc().collect::<String>();
                        parts.push(InterpolationPart {
                            kind: InterpolationKind::Text(normalized_text.clone()),
                            content: normalized_text,
                            position: text_start_pos,
                        });
                    }
                    return Ok(Token::new(
                        TokenType::InterpolatedString(parts),
                        lexeme,
                        start_pos,
                    ));
                } else {
                    let normalized_text = current_text.nfc().collect::<String>();
                    return Ok(Token::new(
                        TokenType::StringLiteral(normalized_text),
                        lexeme,
                        start_pos,
                    ));
                }
            } else if ch == '{' {
                // Treat '{' as a literal character inside triple-quoted strings
                current_text.push('{');
                lexeme.push('{');
                self.advance();
            } else if ch == '}' && self.peek() == Some('}') {
                // Escaped closing brace
                current_text.push('}');
                lexeme.push_str("}}");
                self.advance();
                self.advance();
            } else if ch == '\\' {
                // Handle escape sequences
                lexeme.push('\\');
                self.advance();

                if let Some(escaped_ch) = self.current_char {
                    match escaped_ch {
                        'n' => current_text.push('\n'),
                        't' => current_text.push('\t'),
                        'r' => current_text.push('\r'),
                        '\\' => current_text.push('\\'),
                        '"' => current_text.push('"'),
                        '{' => current_text.push('{'),
                        '}' => current_text.push('}'),
                        _ => {
                            current_text.push('\\');
                            current_text.push(escaped_ch);
                        }
                    }
                    lexeme.push(escaped_ch);
                    self.advance();
                } else {
                    return Err(LexerError::UnexpectedCharacter {
                        character: '\\',
                        position: self.pos_tracker,
                    });
                }
            } else {
                // Regular character, including newlines
                current_text.push(ch);
                lexeme.push(ch);
                self.advance();
            }
        }

        // Reached end of input without closing triple quotes
        Err(LexerError::UnterminatedString {
            position: start_pos,
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

        // Normalize identifier to NFC
        let normalized = identifier.nfc().collect::<String>();

        // Check if it's a keyword
        if let Some(token_type) = self.check_keyword(&normalized) {
            Ok(Token::new(token_type, normalized, start_pos))
        } else {
            // Classify as public or private identifier based on capitalization
            let token_type = self.classify_identifier(&normalized);
            Ok(Token::new(token_type, normalized, start_pos))
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
        let fun_keyword = keyword_manager
            .get_keyword_text(&KeywordType::KeywordFun)
            .unwrap();
        let if_keyword = keyword_manager
            .get_keyword_text(&KeywordType::KeywordIf)
            .unwrap();
        let and_keyword = keyword_manager.get_logical_and();
        let or_keyword = keyword_manager.get_logical_or();
        let not_keyword = keyword_manager.get_logical_not();

        assert_eq!(
            lexer.check_keyword(&fun_keyword),
            Some(TokenType::Keyword(KeywordType::KeywordFun))
        );
        assert_eq!(
            lexer.check_keyword(&if_keyword),
            Some(TokenType::Keyword(KeywordType::KeywordIf))
        );
        // FIXED: Logical operators now convert to dedicated token types
        assert_eq!(
            lexer.check_keyword(&and_keyword),
            Some(TokenType::LogicalAnd)
        );
        assert_eq!(lexer.check_keyword(&or_keyword), Some(TokenType::LogicalOr));
        assert_eq!(
            lexer.check_keyword(&not_keyword),
            Some(TokenType::LogicalNot)
        );

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

        let en_fun_keyword = keyword_manager
            .get_keyword_text(&KeywordType::KeywordFun)
            .unwrap();
        let ar_fun_keyword = "دالة"; // This will be loaded from Arabic TOML

        assert_eq!(
            lexer_en.check_keyword(&en_fun_keyword),
            Some(TokenType::Keyword(KeywordType::KeywordFun))
        );
        assert_eq!(lexer_en.check_keyword(ar_fun_keyword), None); // Arabic should not work in English mode

        // Test Arabic
        keyword_manager.switch_language("ar").unwrap();
        let lexer_ar = Lexer::new("test".to_string(), Arc::new(keyword_manager.clone()));

        let ar_fun_keyword_dynamic = keyword_manager
            .get_keyword_text(&KeywordType::KeywordFun)
            .unwrap();

        assert_eq!(
            lexer_ar.check_keyword(&ar_fun_keyword_dynamic),
            Some(TokenType::Keyword(KeywordType::KeywordFun))
        );
        assert_eq!(lexer_ar.check_keyword(&en_fun_keyword), None); // English should not work in Arabic mode
    }

    #[test]
    fn test_keyword_when_loaded_from_toml() {
        let mut keyword_manager = KeywordManager::new();
        keyword_manager.load_from_toml("fr").unwrap();
        keyword_manager.switch_language("fr").unwrap();

        let french_when = keyword_manager
            .get_keyword_text(&KeywordType::KeywordWhen)
            .expect("fr language should define 'when'");

        let mut lexer = Lexer::new(french_when.clone(), Arc::new(keyword_manager));
        let token = lexer.next_token().unwrap();
        assert_eq!(
            token.token_type,
            TokenType::Keyword(KeywordType::KeywordWhen),
            "expected '{}' to map to KeywordWhen",
            french_when
        );
    }

    #[test]
    fn test_word_based_operators() {
        // RESEARCH-BASED: Test Stefik & Siebert (2013) word-based logical operators
        let keyword_manager = Arc::new(KeywordManager::new());

        // Test "and" operator
        let mut lexer = Lexer::new(
            "age >= 18 and hasPermission".to_string(),
            keyword_manager.clone(),
        );

        // Skip to the "and" token
        lexer.next_token().unwrap(); // age
        lexer.next_token().unwrap(); // >=
        lexer.next_token().unwrap(); // 18

        let and_token = lexer.next_token().unwrap();
        assert_eq!(
            and_token.token_type,
            TokenType::LogicalAnd,
            "Word 'and' should tokenize as LogicalAnd"
        );
        assert_eq!(and_token.lexeme, "and");

        // Test "or" operator
        let mut lexer2 = Lexer::new("not valid or expired".to_string(), keyword_manager.clone());

        let not_token = lexer2.next_token().unwrap();
        assert_eq!(
            not_token.token_type,
            TokenType::LogicalNot,
            "Word 'not' should tokenize as LogicalNot"
        );
        assert_eq!(not_token.lexeme, "not");

        lexer2.next_token().unwrap(); // valid
        let or_token = lexer2.next_token().unwrap();
        assert_eq!(
            or_token.token_type,
            TokenType::LogicalOr,
            "Word 'or' should tokenize as LogicalOr"
        );
        assert_eq!(or_token.lexeme, "or");

        // Test that boolean literals still work
        let mut lexer3 = Lexer::new("true and false or not true".to_string(), keyword_manager);

        let true_token = lexer3.next_token().unwrap();
        assert_eq!(true_token.token_type, TokenType::BoolLiteral(true));

        let and_token2 = lexer3.next_token().unwrap();
        assert_eq!(and_token2.token_type, TokenType::LogicalAnd);

        let false_token = lexer3.next_token().unwrap();
        assert_eq!(false_token.token_type, TokenType::BoolLiteral(false));

        let or_token2 = lexer3.next_token().unwrap();
        assert_eq!(or_token2.token_type, TokenType::LogicalOr);

        let not_token2 = lexer3.next_token().unwrap();
        assert_eq!(not_token2.token_type, TokenType::LogicalNot);

        let true_token2 = lexer3.next_token().unwrap();
        assert_eq!(true_token2.token_type, TokenType::BoolLiteral(true));
    }

    #[test]
    fn test_memory_management_operators() {
        // VALE-STYLE: Test memory management keywords converted to dedicated tokens
        let keyword_manager = Arc::new(KeywordManager::new());

        // Test "move" operator
        let mut lexer = Lexer::new("move data".to_string(), keyword_manager.clone());
        let move_token = lexer.next_token().unwrap();
        assert_eq!(
            move_token.token_type,
            TokenType::Move,
            "Word 'move' should tokenize as Move"
        );
        assert_eq!(move_token.lexeme, "move");

        // Test "borrow" operator
        let mut lexer2 = Lexer::new("borrow mut data".to_string(), keyword_manager.clone());
        let borrow_token = lexer2.next_token().unwrap();
        assert_eq!(
            borrow_token.token_type,
            TokenType::Borrow,
            "Word 'borrow' should tokenize as Borrow"
        );
        assert_eq!(borrow_token.lexeme, "borrow");

        // Test "inout" operator
        let mut lexer3 = Lexer::new("fun modify(inout data: Data)".to_string(), keyword_manager);
        lexer3.next_token().unwrap(); // fun
        lexer3.next_token().unwrap(); // modify
        lexer3.next_token().unwrap(); // (

        let inout_token = lexer3.next_token().unwrap();
        assert_eq!(
            inout_token.token_type,
            TokenType::Inout,
            "Word 'inout' should tokenize as Inout"
        );
        assert_eq!(inout_token.lexeme, "inout");
    }

    // Additional tests will be implemented following TDD methodology
}
