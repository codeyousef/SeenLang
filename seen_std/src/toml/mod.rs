//! TOML parser for configuration files
//!
//! High-performance TOML parser optimized for compiler configuration:
//! - Seen.toml project configuration
//! - Build system settings
//! - Dependencies and targets
//! - Environment settings

use crate::string::String;
use crate::collections::{Vec, HashMap};

/// TOML parsing and serialization errors
#[derive(Debug, Clone, PartialEq)]
pub enum TomlError {
    /// Invalid TOML syntax
    InvalidSyntax(String),
    /// Unexpected end of input
    UnexpectedEof,
    /// Invalid key format
    InvalidKey(String),
    /// Invalid value format
    InvalidValue(String),
    /// Duplicate key
    DuplicateKey(String),
    /// Invalid table name
    InvalidTable(String),
    /// Type conversion error
    TypeError(String),
}

impl std::fmt::Display for TomlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TomlError::InvalidSyntax(msg) => write!(f, "Invalid TOML syntax: {}", msg.as_str()),
            TomlError::UnexpectedEof => write!(f, "Unexpected end of input"),
            TomlError::InvalidKey(key) => write!(f, "Invalid key: {}", key.as_str()),
            TomlError::InvalidValue(val) => write!(f, "Invalid value: {}", val.as_str()),
            TomlError::DuplicateKey(key) => write!(f, "Duplicate key: {}", key.as_str()),
            TomlError::InvalidTable(table) => write!(f, "Invalid table: {}", table.as_str()),
            TomlError::TypeError(msg) => write!(f, "Type error: {}", msg.as_str()),
        }
    }
}

impl std::error::Error for TomlError {}

/// Result type for TOML operations
pub type TomlResult<T> = Result<T, TomlError>;

/// TOML value representation
#[derive(Debug, Clone, PartialEq)]
pub enum TomlValue {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Array(Vec<TomlValue>),
    Table(HashMap<String, TomlValue>),
}

impl TomlValue {
    /// Returns true if the value is a string
    pub fn is_string(&self) -> bool {
        matches!(self, TomlValue::String(_))
    }
    
    /// Returns true if the value is an integer
    pub fn is_integer(&self) -> bool {
        matches!(self, TomlValue::Integer(_))
    }
    
    /// Returns true if the value is a float
    pub fn is_float(&self) -> bool {
        matches!(self, TomlValue::Float(_))
    }
    
    /// Returns true if the value is a boolean
    pub fn is_boolean(&self) -> bool {
        matches!(self, TomlValue::Boolean(_))
    }
    
    /// Returns true if the value is an array
    pub fn is_array(&self) -> bool {
        matches!(self, TomlValue::Array(_))
    }
    
    /// Returns true if the value is a table
    pub fn is_table(&self) -> bool {
        matches!(self, TomlValue::Table(_))
    }
    
    /// Converts to string if possible
    pub fn as_string(&self) -> Option<&str> {
        match self {
            TomlValue::String(s) => Some(s.as_str()),
            _ => None,
        }
    }
    
    /// Converts to integer if possible
    pub fn as_integer(&self) -> Option<i64> {
        match self {
            TomlValue::Integer(i) => Some(*i),
            _ => None,
        }
    }
    
    /// Converts to float if possible
    pub fn as_float(&self) -> Option<f64> {
        match self {
            TomlValue::Float(f) => Some(*f),
            TomlValue::Integer(i) => Some(*i as f64),
            _ => None,
        }
    }
    
    /// Converts to boolean if possible
    pub fn as_boolean(&self) -> Option<bool> {
        match self {
            TomlValue::Boolean(b) => Some(*b),
            _ => None,
        }
    }
    
    /// Converts to array if possible
    pub fn as_array(&self) -> Option<&Vec<TomlValue>> {
        match self {
            TomlValue::Array(a) => Some(a),
            _ => None,
        }
    }
    
    /// Converts to table if possible
    pub fn as_table(&self) -> Option<&HashMap<String, TomlValue>> {
        match self {
            TomlValue::Table(t) => Some(t),
            _ => None,
        }
    }
}

/// TOML document parser and serializer
pub struct TomlParser {
    input: String,
    position: usize,
    line: usize,
    column: usize,
}

impl TomlParser {
    /// Creates a new TOML parser
    pub fn new(input: &str) -> Self {
        TomlParser {
            input: String::from(input),
            position: 0,
            line: 1,
            column: 1,
        }
    }
    
    /// Parses TOML input into a table
    pub fn parse(&mut self) -> TomlResult<HashMap<String, TomlValue>> {
        let mut table = HashMap::new();
        let current_table = &mut table;
        let _table_stack: Vec<HashMap<String, TomlValue>> = Vec::new();
        
        while !self.is_at_end() {
            self.skip_whitespace_and_comments();
            
            if self.is_at_end() {
                break;
            }
            
            // Check for table header
            if self.peek() == '[' {
                let (_table_name, _is_array) = self.parse_table_header()?;
                // For MVP, ignore array tables and nested tables
                // Just treat all as top-level keys
                
                // Skip any trailing whitespace and newline after table header
                self.skip_whitespace();
                if !self.is_at_end() && (self.peek() == '\n' || self.peek() == '\r') {
                    self.advance();
                }
                continue;
            }
            
            // Parse key-value pair
            let (key, value) = self.parse_key_value_pair()?;
            
            if current_table.contains_key(&key) {
                return Err(TomlError::DuplicateKey(key));
            }
            
            current_table.insert(key, value);
            
            self.skip_whitespace();
            
            // Skip inline comment if present
            if !self.is_at_end() && self.peek() == '#' {
                self.skip_comment();
            }
            
            // Skip any trailing whitespace
            self.skip_whitespace();
            
            // Expect newline or end of file
            if !self.is_at_end() && self.peek() != '\n' && self.peek() != '\r' {
                return Err(TomlError::InvalidSyntax(String::from("Expected newline after key-value pair")));
            }
            
            if !self.is_at_end() {
                self.advance(); // consume newline
            }
        }
        
        Ok(table)
    }
    
    /// Parses a key-value pair
    fn parse_key_value_pair(&mut self) -> TomlResult<(String, TomlValue)> {
        let key = self.parse_key()?;
        
        self.skip_whitespace();
        
        if self.is_at_end() {
            return Err(TomlError::UnexpectedEof);
        }
        
        if self.peek() != '=' {
            return Err(TomlError::InvalidSyntax(String::from("Expected '=' after key")));
        }
        self.advance(); // consume '='
        
        self.skip_whitespace();
        
        if self.is_at_end() {
            return Err(TomlError::UnexpectedEof);
        }
        
        let value = self.parse_value()?;
        
        Ok((key, value))
    }
    
    /// Parses a key
    fn parse_key(&mut self) -> TomlResult<String> {
        let mut key = String::new();
        
        if self.peek() == '"' {
            // Quoted key
            self.advance(); // consume opening quote
            while !self.is_at_end() && self.peek() != '"' {
                key.push(self.advance());
            }
            if self.is_at_end() {
                return Err(TomlError::UnexpectedEof);
            }
            self.advance(); // consume closing quote
        } else {
            // Bare key
            while !self.is_at_end() && self.is_key_char(self.peek()) {
                key.push(self.advance());
            }
        }
        
        if key.is_empty() {
            return Err(TomlError::InvalidKey(String::from("Empty key")));
        }
        
        Ok(key)
    }
    
    /// Parses a value
    fn parse_value(&mut self) -> TomlResult<TomlValue> {
        match self.peek() {
            '"' => self.parse_string(),
            '[' => self.parse_array(),
            '{' => self.parse_inline_table(),
            't' | 'f' => self.parse_boolean(),
            c if c.is_ascii_digit() || c == '-' || c == '+' => self.parse_number(),
            _ => Err(TomlError::InvalidValue(String::from("Unrecognized value type"))),
        }
    }
    
    /// Parses a string value
    fn parse_string(&mut self) -> TomlResult<TomlValue> {
        self.advance(); // consume opening quote
        let mut value = String::new();
        
        while !self.is_at_end() && self.peek() != '"' {
            if self.peek() == '\\' {
                self.advance(); // consume backslash
                if self.is_at_end() {
                    return Err(TomlError::UnexpectedEof);
                }
                match self.advance() {
                    'n' => value.push('\n'),
                    't' => value.push('\t'),
                    'r' => value.push('\r'),
                    '\\' => value.push('\\'),
                    '"' => value.push('"'),
                    c => {
                        value.push('\\');
                        value.push(c);
                    }
                }
            } else {
                value.push(self.advance());
            }
        }
        
        if self.is_at_end() {
            return Err(TomlError::UnexpectedEof);
        }
        
        self.advance(); // consume closing quote
        Ok(TomlValue::String(value))
    }
    
    /// Parses a number (integer or float)
    fn parse_number(&mut self) -> TomlResult<TomlValue> {
        let mut number = String::new();
        let mut is_float = false;
        
        // Handle sign
        if self.peek() == '-' || self.peek() == '+' {
            number.push(self.advance());
        }
        
        // Parse digits and decimal point
        while !self.is_at_end() && (self.peek().is_ascii_digit() || self.peek() == '.') {
            if self.peek() == '.' {
                if is_float {
                    break; // Second decimal point
                }
                is_float = true;
            }
            number.push(self.advance());
        }
        
        if number.is_empty() || number.as_str() == "+" || number.as_str() == "-" {
            return Err(TomlError::InvalidValue(String::from("Invalid number")));
        }
        
        if is_float {
            match number.as_str().parse::<f64>() {
                Ok(f) => Ok(TomlValue::Float(f)),
                Err(_) => Err(TomlError::InvalidValue(String::from("Invalid float"))),
            }
        } else {
            match number.as_str().parse::<i64>() {
                Ok(i) => Ok(TomlValue::Integer(i)),
                Err(_) => Err(TomlError::InvalidValue(String::from("Invalid integer"))),
            }
        }
    }
    
    /// Parses a boolean value
    fn parse_boolean(&mut self) -> TomlResult<TomlValue> {
        if self.match_string("true") {
            Ok(TomlValue::Boolean(true))
        } else if self.match_string("false") {
            Ok(TomlValue::Boolean(false))
        } else {
            Err(TomlError::InvalidValue(String::from("Invalid boolean")))
        }
    }
    
    /// Parses an array
    fn parse_array(&mut self) -> TomlResult<TomlValue> {
        self.advance(); // consume '['
        let mut array = Vec::new();
        
        self.skip_whitespace();
        
        if self.is_at_end() {
            return Err(TomlError::UnexpectedEof);
        }
        
        if self.peek() == ']' {
            self.advance(); // consume ']'
            return Ok(TomlValue::Array(array));
        }
        
        loop {
            self.skip_whitespace();
            
            if self.is_at_end() {
                return Err(TomlError::UnexpectedEof);
            }
            
            let value = self.parse_value()?;
            array.push(value);
            
            self.skip_whitespace();
            
            if self.is_at_end() {
                return Err(TomlError::UnexpectedEof);
            }
            
            if self.peek() == ']' {
                self.advance(); // consume ']'
                break;
            }
            
            if self.peek() != ',' {
                return Err(TomlError::InvalidSyntax(String::from("Expected ',' or ']' in array")));
            }
            self.advance(); // consume ','
        }
        
        Ok(TomlValue::Array(array))
    }
    
    /// Parses an inline table
    fn parse_inline_table(&mut self) -> TomlResult<TomlValue> {
        self.advance(); // consume '{'
        let mut table = HashMap::new();
        
        self.skip_whitespace();
        
        if self.peek() == '}' {
            self.advance(); // consume '}'
            return Ok(TomlValue::Table(table));
        }
        
        loop {
            self.skip_whitespace();
            let (key, value) = self.parse_key_value_pair()?;
            
            if table.contains_key(&key) {
                return Err(TomlError::DuplicateKey(key));
            }
            
            table.insert(key, value);
            
            self.skip_whitespace();
            
            if self.peek() == '}' {
                self.advance(); // consume '}'
                break;
            }
            
            if self.peek() != ',' {
                return Err(TomlError::InvalidSyntax(String::from("Expected ',' or '}' in inline table")));
            }
            self.advance(); // consume ','
        }
        
        Ok(TomlValue::Table(table))
    }
    
    /// Parses table header
    fn parse_table_header(&mut self) -> TomlResult<(String, bool)> {
        self.advance(); // consume '['
        
        let is_array = if self.peek() == '[' {
            self.advance(); // consume second '['
            true
        } else {
            false
        };
        
        let mut name = String::new();
        
        while !self.is_at_end() && self.peek() != ']' {
            name.push(self.advance());
        }
        
        if self.is_at_end() {
            return Err(TomlError::UnexpectedEof);
        }
        
        self.advance(); // consume first ']'
        
        if is_array {
            if self.peek() != ']' {
                return Err(TomlError::InvalidSyntax(String::from("Expected second ']' for array table")));
            }
            self.advance(); // consume second ']'
        }
        
        Ok((String::from(name.trim().as_str()), is_array))
    }
    
    /// Helper functions
    fn is_at_end(&self) -> bool {
        self.position >= self.input.len()
    }
    
    fn peek(&self) -> char {
        if self.is_at_end() {
            '\0'
        } else {
            self.input.chars().nth(self.position).unwrap_or('\0')
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
        while !self.is_at_end() && (self.peek() == ' ' || self.peek() == '\t') {
            self.advance();
        }
    }
    
    fn skip_comment(&mut self) {
        if self.peek() == '#' {
            while !self.is_at_end() && self.peek() != '\n' {
                self.advance();
            }
        }
    }
    
    fn skip_whitespace_and_comments(&mut self) {
        while !self.is_at_end() {
            let ch = self.peek();
            if ch == ' ' || ch == '\t' || ch == '\n' || ch == '\r' {
                self.advance();
            } else if ch == '#' {
                // Skip comment
                while !self.is_at_end() && self.peek() != '\n' {
                    self.advance();
                }
            } else {
                break;
            }
        }
    }
    
    fn is_key_char(&self, ch: char) -> bool {
        ch.is_alphanumeric() || ch == '_' || ch == '-'
    }
}

/// Convenience functions for parsing TOML
pub fn parse_toml(input: &str) -> TomlResult<HashMap<String, TomlValue>> {
    let mut parser = TomlParser::new(input);
    parser.parse()
}

/// Convenience function to get a string value from a table
pub fn get_string<'a>(table: &'a HashMap<String, TomlValue>, key: &str) -> Option<&'a str> {
    table.get(key)?.as_string()
}

/// Convenience function to get an integer value from a table
pub fn get_integer(table: &HashMap<String, TomlValue>, key: &str) -> Option<i64> {
    table.get(key)?.as_integer()
}

/// Convenience function to get a boolean value from a table
pub fn get_boolean(table: &HashMap<String, TomlValue>, key: &str) -> Option<bool> {
    table.get(key)?.as_boolean()
}

#[cfg(test)]
mod tests;