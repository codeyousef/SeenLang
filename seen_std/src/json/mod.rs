//! JSON parser and serializer for data interchange
//!
//! High-performance JSON implementation optimized for compiler use:
//! - Configuration data exchange
//! - API communication
//! - Build artifact serialization
//! - Language server protocol

use crate::string::String;
use crate::collections::{Vec, HashMap};

/// JSON parsing and serialization errors
#[derive(Debug, Clone, PartialEq)]
pub enum JsonError {
    /// Invalid JSON syntax
    InvalidSyntax(String),
    /// Unexpected end of input
    UnexpectedEof,
    /// Invalid number format
    InvalidNumber(String),
    /// Invalid string escape sequence
    InvalidEscape(String),
    /// Invalid Unicode escape
    InvalidUnicode(String),
    /// Maximum nesting depth exceeded
    MaxDepthExceeded,
    /// Invalid character in JSON
    InvalidCharacter(char),
}

impl std::fmt::Display for JsonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JsonError::InvalidSyntax(msg) => write!(f, "Invalid JSON syntax: {}", msg.as_str()),
            JsonError::UnexpectedEof => write!(f, "Unexpected end of JSON input"),
            JsonError::InvalidNumber(num) => write!(f, "Invalid number: {}", num.as_str()),
            JsonError::InvalidEscape(seq) => write!(f, "Invalid escape sequence: {}", seq.as_str()),
            JsonError::InvalidUnicode(seq) => write!(f, "Invalid Unicode escape: {}", seq.as_str()),
            JsonError::MaxDepthExceeded => write!(f, "Maximum nesting depth exceeded"),
            JsonError::InvalidCharacter(ch) => write!(f, "Invalid character: '{}'", ch),
        }
    }
}

impl std::error::Error for JsonError {}

/// Result type for JSON operations
pub type JsonResult<T> = Result<T, JsonError>;

/// JSON value representation
#[derive(Debug, Clone, PartialEq)]
pub enum JsonValue {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<JsonValue>),
    Object(HashMap<String, JsonValue>),
}

impl JsonValue {
    /// Returns true if the value is null
    pub fn is_null(&self) -> bool {
        matches!(self, JsonValue::Null)
    }
    
    /// Returns true if the value is a boolean
    pub fn is_bool(&self) -> bool {
        matches!(self, JsonValue::Bool(_))
    }
    
    /// Returns true if the value is a number
    pub fn is_number(&self) -> bool {
        matches!(self, JsonValue::Number(_))
    }
    
    /// Returns true if the value is a string
    pub fn is_string(&self) -> bool {
        matches!(self, JsonValue::String(_))
    }
    
    /// Returns true if the value is an array
    pub fn is_array(&self) -> bool {
        matches!(self, JsonValue::Array(_))
    }
    
    /// Returns true if the value is an object
    pub fn is_object(&self) -> bool {
        matches!(self, JsonValue::Object(_))
    }
    
    /// Converts to boolean if possible
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            JsonValue::Bool(b) => Some(*b),
            _ => None,
        }
    }
    
    /// Converts to number if possible
    pub fn as_number(&self) -> Option<f64> {
        match self {
            JsonValue::Number(n) => Some(*n),
            _ => None,
        }
    }
    
    /// Converts to string if possible
    pub fn as_str(&self) -> Option<&str> {
        match self {
            JsonValue::String(s) => Some(s.as_str()),
            _ => None,
        }
    }
    
    /// Converts to array if possible
    pub fn as_array(&self) -> Option<&Vec<JsonValue>> {
        match self {
            JsonValue::Array(a) => Some(a),
            _ => None,
        }
    }
    
    /// Converts to object if possible
    pub fn as_object(&self) -> Option<&HashMap<String, JsonValue>> {
        match self {
            JsonValue::Object(o) => Some(o),
            _ => None,
        }
    }
    
    /// Gets a value from an object by key
    pub fn get(&self, key: &str) -> Option<&JsonValue> {
        match self {
            JsonValue::Object(obj) => obj.get(key),
            _ => None,
        }
    }
    
    /// Gets a value from an array by index
    pub fn get_index(&self, index: usize) -> Option<&JsonValue> {
        match self {
            JsonValue::Array(arr) => arr.get(index),
            _ => None,
        }
    }
}

/// JSON parser with streaming support
pub struct JsonParser {
    input: String,
    chars: Vec<char>,
    position: usize,
    line: usize,
    column: usize,
    depth: usize,
    max_depth: usize,
}

impl JsonParser {
    /// Creates a new JSON parser
    pub fn new(input: &str) -> Self {
        let chars: Vec<char> = input.chars().collect();
        JsonParser {
            input: String::from(input),
            chars,
            position: 0,
            line: 1,
            column: 1,
            depth: 0,
            max_depth: 128, // Prevent stack overflow
        }
    }
    
    /// Parses JSON input into a value
    pub fn parse(&mut self) -> JsonResult<JsonValue> {
        self.skip_whitespace();
        let value = self.parse_value()?;
        self.skip_whitespace();
        
        if !self.is_at_end() {
            return Err(JsonError::InvalidSyntax(String::from("Extra characters after JSON")));
        }
        
        Ok(value)
    }
    
    /// Parses a JSON value
    fn parse_value(&mut self) -> JsonResult<JsonValue> {
        if self.depth >= self.max_depth {
            return Err(JsonError::MaxDepthExceeded);
        }
        
        self.skip_whitespace();
        
        if self.is_at_end() {
            return Err(JsonError::UnexpectedEof);
        }
        
        match self.peek() {
            'n' => self.parse_null(),
            't' | 'f' => self.parse_bool(),
            '"' => self.parse_string(),
            '[' => self.parse_array(),
            '{' => self.parse_object(),
            '-' | '0'..='9' => self.parse_number(),
            c => Err(JsonError::InvalidCharacter(c)),
        }
    }
    
    /// Parses a null value
    fn parse_null(&mut self) -> JsonResult<JsonValue> {
        if self.match_string("null") {
            Ok(JsonValue::Null)
        } else {
            Err(JsonError::InvalidSyntax(String::from("Invalid null")))
        }
    }
    
    /// Parses a boolean value
    fn parse_bool(&mut self) -> JsonResult<JsonValue> {
        if self.match_string("true") {
            Ok(JsonValue::Bool(true))
        } else if self.match_string("false") {
            Ok(JsonValue::Bool(false))
        } else {
            Err(JsonError::InvalidSyntax(String::from("Invalid boolean")))
        }
    }
    
    /// Parses a string value
    fn parse_string(&mut self) -> JsonResult<JsonValue> {
        self.advance(); // consume opening quote
        let mut value = String::new();
        
        while !self.is_at_end() && self.peek() != '"' {
            if self.peek() == '\\' {
                self.advance(); // consume backslash
                if self.is_at_end() {
                    return Err(JsonError::UnexpectedEof);
                }
                
                match self.advance() {
                    '"' => value.push('"'),
                    '\\' => value.push('\\'),
                    '/' => value.push('/'),
                    'b' => value.push('\u{0008}'),
                    'f' => value.push('\u{000C}'),
                    'n' => value.push('\n'),
                    'r' => value.push('\r'),
                    't' => value.push('\t'),
                    'u' => {
                        // Unicode escape sequence
                        let mut hex = String::new();
                        for _ in 0..4 {
                            if self.is_at_end() {
                                return Err(JsonError::UnexpectedEof);
                            }
                            let ch = self.advance();
                            if ch.is_ascii_hexdigit() {
                                hex.push(ch);
                            } else {
                                return Err(JsonError::InvalidUnicode(hex));
                            }
                        }
                        
                        match u32::from_str_radix(hex.as_str(), 16) {
                            Ok(code) => {
                                if let Some(unicode_char) = char::from_u32(code) {
                                    value.push(unicode_char);
                                } else {
                                    return Err(JsonError::InvalidUnicode(hex));
                                }
                            }
                            Err(_) => return Err(JsonError::InvalidUnicode(hex)),
                        }
                    }
                    c => return Err(JsonError::InvalidEscape(String::from(&format!("\\{}", c)))),
                }
            } else {
                let ch = self.advance();
                if (ch as u32) >= 0x20 || ch == '\t' {
                    value.push(ch);
                } else {
                    return Err(JsonError::InvalidCharacter(ch));
                }
            }
        }
        
        if self.is_at_end() {
            return Err(JsonError::UnexpectedEof);
        }
        
        self.advance(); // consume closing quote
        Ok(JsonValue::String(value))
    }
    
    /// Parses a number value
    fn parse_number(&mut self) -> JsonResult<JsonValue> {
        let mut number = String::new();
        
        // Handle negative sign
        if self.peek() == '-' {
            number.push(self.advance());
        }
        
        // Parse integer part
        if self.peek() == '0' {
            number.push(self.advance());
        } else if self.peek().is_ascii_digit() {
            while !self.is_at_end() && self.peek().is_ascii_digit() {
                number.push(self.advance());
            }
        } else {
            return Err(JsonError::InvalidNumber(number));
        }
        
        // Parse fractional part
        if !self.is_at_end() && self.peek() == '.' {
            number.push(self.advance());
            if self.is_at_end() || !self.peek().is_ascii_digit() {
                return Err(JsonError::InvalidNumber(number));
            }
            while !self.is_at_end() && self.peek().is_ascii_digit() {
                number.push(self.advance());
            }
        }
        
        // Parse exponent part
        if !self.is_at_end() && (self.peek() == 'e' || self.peek() == 'E') {
            number.push(self.advance());
            if !self.is_at_end() && (self.peek() == '+' || self.peek() == '-') {
                number.push(self.advance());
            }
            if self.is_at_end() || !self.peek().is_ascii_digit() {
                return Err(JsonError::InvalidNumber(number));
            }
            while !self.is_at_end() && self.peek().is_ascii_digit() {
                number.push(self.advance());
            }
        }
        
        match number.as_str().parse::<f64>() {
            Ok(n) => Ok(JsonValue::Number(n)),
            Err(_) => Err(JsonError::InvalidNumber(number)),
        }
    }
    
    /// Parses an array value
    fn parse_array(&mut self) -> JsonResult<JsonValue> {
        self.depth += 1;
        self.advance(); // consume '['
        let mut array = Vec::new();
        
        self.skip_whitespace();
        
        if self.peek() == ']' {
            self.advance(); // consume ']'
            self.depth -= 1;
            return Ok(JsonValue::Array(array));
        }
        
        loop {
            let value = self.parse_value()?;
            array.push(value);
            
            self.skip_whitespace();
            
            if self.peek() == ']' {
                self.advance(); // consume ']'
                break;
            } else if self.peek() == ',' {
                self.advance(); // consume ','
                self.skip_whitespace();
            } else {
                return Err(JsonError::InvalidSyntax(String::from("Expected ',' or ']' in array")));
            }
        }
        
        self.depth -= 1;
        Ok(JsonValue::Array(array))
    }
    
    /// Parses an object value
    fn parse_object(&mut self) -> JsonResult<JsonValue> {
        self.depth += 1;
        self.advance(); // consume '{'
        let mut object = HashMap::new();
        
        self.skip_whitespace();
        
        if self.peek() == '}' {
            self.advance(); // consume '}'
            self.depth -= 1;
            return Ok(JsonValue::Object(object));
        }
        
        loop {
            self.skip_whitespace();
            
            // Parse key (must be a string)
            if self.peek() != '"' {
                return Err(JsonError::InvalidSyntax(String::from("Expected string key in object")));
            }
            
            let key_value = self.parse_string()?;
            let key = match key_value {
                JsonValue::String(k) => k,
                _ => unreachable!(),
            };
            
            self.skip_whitespace();
            
            // Expect colon
            if self.peek() != ':' {
                return Err(JsonError::InvalidSyntax(String::from("Expected ':' after object key")));
            }
            self.advance(); // consume ':'
            
            // Parse value
            let value = self.parse_value()?;
            
            // Check for duplicate keys (JSON spec allows, but it's good practice to detect)
            object.insert(key, value);
            
            self.skip_whitespace();
            
            if self.peek() == '}' {
                self.advance(); // consume '}'
                break;
            } else if self.peek() == ',' {
                self.advance(); // consume ','
            } else {
                return Err(JsonError::InvalidSyntax(String::from("Expected ',' or '}' in object")));
            }
        }
        
        self.depth -= 1;
        Ok(JsonValue::Object(object))
    }
    
    /// Helper functions
    fn is_at_end(&self) -> bool {
        self.position >= self.chars.len()
    }
    
    fn peek(&self) -> char {
        if self.is_at_end() {
            '\0'
        } else {
            self.chars[self.position]
        }
    }
    
    fn advance(&mut self) -> char {
        let ch = self.peek();
        if ch == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        self.position += 1;
        ch
    }
    
    fn match_string(&mut self, s: &str) -> bool {
        let remaining = &self.input.as_str()[self.position..];
        if remaining.starts_with(s) {
            self.position += s.len();
            self.column += s.len();
            true
        } else {
            false
        }
    }
    
    fn skip_whitespace(&mut self) {
        while !self.is_at_end() {
            match self.peek() {
                ' ' | '\t' | '\n' | '\r' => {
                    self.advance();
                }
                _ => break,
            }
        }
    }
}

/// JSON serializer
pub struct JsonSerializer {
    output: String,
    indent: usize,
    pretty: bool,
}

impl JsonSerializer {
    /// Creates a new JSON serializer
    pub fn new() -> Self {
        JsonSerializer {
            output: String::new(),
            indent: 0,
            pretty: false,
        }
    }
    
    /// Creates a pretty-printing JSON serializer
    pub fn pretty() -> Self {
        JsonSerializer {
            output: String::new(),
            indent: 0,
            pretty: true,
        }
    }
    
    /// Serializes a JSON value to string
    pub fn serialize(&mut self, value: &JsonValue) -> String {
        self.output.clear();
        self.serialize_value(value);
        self.output.clone()
    }
    
    /// Serializes a JSON value
    fn serialize_value(&mut self, value: &JsonValue) {
        match value {
            JsonValue::Null => self.output.push_str("null"),
            JsonValue::Bool(b) => {
                if *b {
                    self.output.push_str("true");
                } else {
                    self.output.push_str("false");
                }
            }
            JsonValue::Number(n) => {
                if n.fract() == 0.0 && *n >= i64::MIN as f64 && *n <= i64::MAX as f64 {
                    self.output.push_str(&format!("{}", *n as i64));
                } else {
                    self.output.push_str(&format!("{}", n));
                }
            }
            JsonValue::String(s) => self.serialize_string(s),
            JsonValue::Array(a) => self.serialize_array(a),
            JsonValue::Object(o) => self.serialize_object(o),
        }
    }
    
    /// Serializes a string value with proper escaping
    fn serialize_string(&mut self, s: &str) {
        self.output.push('"');
        for ch in s.chars() {
            match ch {
                '"' => self.output.push_str("\\\""),
                '\\' => self.output.push_str("\\\\"),
                '\u{0008}' => self.output.push_str("\\b"),
                '\u{000C}' => self.output.push_str("\\f"),
                '\n' => self.output.push_str("\\n"),
                '\r' => self.output.push_str("\\r"),
                '\t' => self.output.push_str("\\t"),
                c if (c as u32) < 0x20 => {
                    self.output.push_str(&format!("\\u{:04x}", c as u32));
                }
                c => self.output.push(c),
            }
        }
        self.output.push('"');
    }
    
    /// Serializes an array
    fn serialize_array(&mut self, array: &Vec<JsonValue>) {
        self.output.push('[');
        
        if self.pretty && !array.is_empty() {
            self.indent += 1;
        }
        
        for (i, value) in array.iter().enumerate() {
            if i > 0 {
                self.output.push(',');
            }
            
            if self.pretty {
                self.output.push('\n');
                for _ in 0..self.indent {
                    self.output.push_str("  ");
                }
            }
            
            self.serialize_value(value);
        }
        
        if self.pretty && !array.is_empty() {
            self.indent -= 1;
            self.output.push('\n');
            for _ in 0..self.indent {
                self.output.push_str("  ");
            }
        }
        
        self.output.push(']');
    }
    
    /// Serializes an object
    fn serialize_object(&mut self, object: &HashMap<String, JsonValue>) {
        self.output.push('{');
        
        if self.pretty && object.len() != 0 {
            self.indent += 1;
        }
        
        let mut first = true;
        for (key, value) in object.iter() {
            if !first {
                self.output.push(',');
            }
            first = false;
            
            if self.pretty {
                self.output.push('\n');
                for _ in 0..self.indent {
                    self.output.push_str("  ");
                }
            }
            
            self.serialize_string(key.as_str());
            self.output.push(':');
            
            if self.pretty {
                self.output.push(' ');
            }
            
            self.serialize_value(value);
        }
        
        if self.pretty && object.len() != 0 {
            self.indent -= 1;
            self.output.push('\n');
            for _ in 0..self.indent {
                self.output.push_str("  ");
            }
        }
        
        self.output.push('}');
    }
}

/// Convenience functions for JSON operations
pub fn parse_json(input: &str) -> JsonResult<JsonValue> {
    let mut parser = JsonParser::new(input);
    parser.parse()
}

/// Serialize JSON value to string
pub fn to_json_string(value: &JsonValue) -> String {
    let mut serializer = JsonSerializer::new();
    serializer.serialize(value)
}

/// Serialize JSON value to pretty-printed string
pub fn to_json_pretty(value: &JsonValue) -> String {
    let mut serializer = JsonSerializer::pretty();
    serializer.serialize(value)
}

#[cfg(test)]
mod tests;