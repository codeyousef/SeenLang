// Rust JSON parser benchmark implementation for fair comparison with Seen
// High-performance JSON parser using similar architecture to Seen implementation

use std::collections::HashMap;
use std::fs;
use std::time::{Duration, Instant};
use std::path::Path;
use std::str::Chars;
use std::iter::Peekable;

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
    pub fn is_valid(&self) -> bool {
        match self {
            JsonValue::Null => true,
            JsonValue::Bool(_) => true,
            JsonValue::Number(n) => n.is_finite(),
            JsonValue::String(s) => !s.is_empty(),
            JsonValue::Array(arr) => arr.iter().all(|v| v.is_valid()),
            JsonValue::Object(obj) => obj.values().all(|v| v.is_valid()),
        }
    }
    
    pub fn size(&self) -> usize {
        match self {
            JsonValue::Null => 1,
            JsonValue::Bool(_) => 1,
            JsonValue::Number(_) => 1,
            JsonValue::String(s) => s.len(),
            JsonValue::Array(arr) => arr.iter().map(|v| v.size()).sum(),
            JsonValue::Object(obj) => obj.values().map(|v| v.size()).sum::<usize>() + obj.len(),
        }
    }
}

pub struct JsonParser {
    chars: Peekable<Chars<'static>>,
    line: usize,
    column: usize,
    input: &'static str,
}

impl JsonParser {
    pub fn new(input: &'static str) -> Self {
        Self {
            chars: input.chars().peekable(),
            line: 1,
            column: 1,
            input,
        }
    }
    
    fn current_char(&mut self) -> Option<&char> {
        self.chars.peek()
    }
    
    fn advance(&mut self) -> Option<char> {
        match self.chars.next() {
            Some('\n') => {
                self.line += 1;
                self.column = 1;
                Some('\n')
            }
            Some(ch) => {
                self.column += 1;
                Some(ch)
            }
            None => None,
        }
    }
    
    fn skip_whitespace(&mut self) {
        while let Some(&ch) = self.current_char() {
            if ch.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }
    
    pub fn parse(&mut self) -> Result<JsonValue, String> {
        let value = self.parse_value()?;
        self.skip_whitespace();
        
        if self.current_char().is_some() {
            return Err(format!("Unexpected content after JSON value at line {}, column {}", 
                              self.line, self.column));
        }
        
        Ok(value)
    }
    
    fn parse_value(&mut self) -> Result<JsonValue, String> {
        self.skip_whitespace();
        
        match self.current_char() {
            Some(&'"') => self.parse_string(),
            Some(&'[') => self.parse_array(),
            Some(&'{') => self.parse_object(),
            Some(&'t') | Some(&'f') => self.parse_boolean(),
            Some(&'n') => self.parse_null(),
            Some(&ch) if ch.is_ascii_digit() || ch == '-' => self.parse_number(),
            Some(&ch) => Err(format!("Unexpected character '{}' at line {}, column {}", 
                                   ch, self.line, self.column)),
            None => Err("Unexpected end of input".to_string()),
        }
    }
    
    fn parse_string(&mut self) -> Result<JsonValue, String> {
        self.advance(); // consume opening quote
        
        let mut value = String::new();
        
        while let Some(ch) = self.advance() {
            match ch {
                '"' => return Ok(JsonValue::String(value)),
                '\\' => {
                    match self.advance() {
                        Some('"') => value.push('"'),
                        Some('\\') => value.push('\\'),
                        Some('/') => value.push('/'),
                        Some('b') => value.push('\u{0008}'),
                        Some('f') => value.push('\u{000C}'),
                        Some('n') => value.push('\n'),
                        Some('r') => value.push('\r'),
                        Some('t') => value.push('\t'),
                        Some('u') => {
                            let hex = self.parse_unicode_escape()?;
                            value.push(hex);
                        }
                        Some(ch) => return Err(format!("Invalid escape sequence '\\{}' at line {}, column {}", 
                                                     ch, self.line, self.column)),
                        None => return Err("Unexpected end of input in string".to_string()),
                    }
                }
                ch => value.push(ch),
            }
        }
        
        Err("Unterminated string".to_string())
    }
    
    fn parse_unicode_escape(&mut self) -> Result<char, String> {
        let mut hex = String::new();
        
        for _ in 0..4 {
            match self.advance() {
                Some(ch) if ch.is_ascii_hexdigit() => hex.push(ch),
                Some(ch) => return Err(format!("Invalid hex digit '{}' in unicode escape at line {}, column {}", 
                                             ch, self.line, self.column)),
                None => return Err("Unexpected end of input in unicode escape".to_string()),
            }
        }
        
        let code_point = u32::from_str_radix(&hex, 16)
            .map_err(|_| "Invalid unicode escape sequence".to_string())?;
        
        char::from_u32(code_point)
            .ok_or("Invalid unicode code point".to_string())
    }
    
    fn parse_number(&mut self) -> Result<JsonValue, String> {
        let mut number = String::new();
        
        // Handle negative sign
        if let Some(&'-') = self.current_char() {
            number.push('-');
            self.advance();
        }
        
        // Parse integer part
        if let Some(&'0') = self.current_char() {
            number.push('0');
            self.advance();
        } else {
            let mut has_digits = false;
            while let Some(&ch) = self.current_char() {
                if ch.is_ascii_digit() {
                    number.push(ch);
                    self.advance();
                    has_digits = true;
                } else {
                    break;
                }
            }
            if !has_digits {
                return Err("Invalid number: missing digits".to_string());
            }
        }
        
        // Parse decimal part
        if let Some(&'.') = self.current_char() {
            number.push('.');
            self.advance();
            
            let mut has_digits = false;
            while let Some(&ch) = self.current_char() {
                if ch.is_ascii_digit() {
                    number.push(ch);
                    self.advance();
                    has_digits = true;
                } else {
                    break;
                }
            }
            
            if !has_digits {
                return Err("Invalid number: missing digits after decimal point".to_string());
            }
        }
        
        // Parse exponent part
        if let Some(&ch) = self.current_char() {
            if ch == 'e' || ch == 'E' {
                number.push('e');
                self.advance();
                
                if let Some(&sign) = self.current_char() {
                    if sign == '+' || sign == '-' {
                        number.push(sign);
                        self.advance();
                    }
                }
                
                let mut has_digits = false;
                while let Some(&ch) = self.current_char() {
                    if ch.is_ascii_digit() {
                        number.push(ch);
                        self.advance();
                        has_digits = true;
                    } else {
                        break;
                    }
                }
                
                if !has_digits {
                    return Err("Invalid number: missing digits in exponent".to_string());
                }
            }
        }
        
        let value: f64 = number.parse()
            .map_err(|_| format!("Invalid number format: '{}'", number))?;
        
        Ok(JsonValue::Number(value))
    }
    
    fn parse_boolean(&mut self) -> Result<JsonValue, String> {
        if self.match_keyword("true") {
            Ok(JsonValue::Bool(true))
        } else if self.match_keyword("false") {
            Ok(JsonValue::Bool(false))
        } else {
            Err(format!("Invalid boolean at line {}, column {}", self.line, self.column))
        }
    }
    
    fn parse_null(&mut self) -> Result<JsonValue, String> {
        if self.match_keyword("null") {
            Ok(JsonValue::Null)
        } else {
            Err(format!("Invalid null at line {}, column {}", self.line, self.column))
        }
    }
    
    fn match_keyword(&mut self, keyword: &str) -> bool {
        let mut temp_chars = self.chars.clone();
        
        for expected_char in keyword.chars() {
            match temp_chars.next() {
                Some(ch) if ch == expected_char => continue,
                _ => return false,
            }
        }
        
        // Check that the keyword is not part of a larger identifier
        if let Some(&ch) = temp_chars.peek() {
            if ch.is_alphanumeric() {
                return false;
            }
        }
        
        // Consume the keyword
        for _ in keyword.chars() {
            self.advance();
        }
        
        true
    }
    
    fn parse_array(&mut self) -> Result<JsonValue, String> {
        self.advance(); // consume '['
        self.skip_whitespace();
        
        let mut elements = Vec::new();
        
        // Handle empty array
        if let Some(&']') = self.current_char() {
            self.advance();
            return Ok(JsonValue::Array(elements));
        }
        
        loop {
            let value = self.parse_value()?;
            elements.push(value);
            
            self.skip_whitespace();
            
            match self.current_char() {
                Some(&',') => {
                    self.advance();
                    self.skip_whitespace();
                }
                Some(&']') => {
                    self.advance();
                    break;
                }
                Some(&ch) => return Err(format!("Expected ',' or ']' but found '{}' at line {}, column {}", 
                                              ch, self.line, self.column)),
                None => return Err("Unexpected end of input in array".to_string()),
            }
        }
        
        Ok(JsonValue::Array(elements))
    }
    
    fn parse_object(&mut self) -> Result<JsonValue, String> {
        self.advance(); // consume '{'
        self.skip_whitespace();
        
        let mut object = HashMap::new();
        
        // Handle empty object
        if let Some(&'}') = self.current_char() {
            self.advance();
            return Ok(JsonValue::Object(object));
        }
        
        loop {
            // Parse key (must be string)
            let key = match self.parse_string()? {
                JsonValue::String(s) => s,
                _ => return Err("Object key must be a string".to_string()),
            };
            
            self.skip_whitespace();
            
            // Expect colon
            match self.current_char() {
                Some(&':') => self.advance(),
                Some(&ch) => return Err(format!("Expected ':' after object key but found '{}' at line {}, column {}", 
                                              ch, self.line, self.column)),
                None => return Err("Unexpected end of input in object".to_string()),
            }
            
            self.skip_whitespace();
            
            // Parse value
            let value = self.parse_value()?;
            object.insert(key, value);
            
            self.skip_whitespace();
            
            match self.current_char() {
                Some(&',') => {
                    self.advance();
                    self.skip_whitespace();
                }
                Some(&'}') => {
                    self.advance();
                    break;
                }
                Some(&ch) => return Err(format!("Expected ',' or '}}' but found '{}' at line {}, column {}", 
                                              ch, self.line, self.column)),
                None => return Err("Unexpected end of input in object".to_string()),
            }
        }
        
        Ok(JsonValue::Object(object))
    }
}

// Benchmark functions
pub fn benchmark_json_parser_real_world() -> Result<(), Box<dyn std::error::Error>> {
    let test_files = vec![
        "../../test_data/json_files/twitter.json",
        "../../test_data/json_files/canada.json", 
        "../../test_data/json_files/citm_catalog.json",
        "../../test_data/json_files/large.json",
    ];
    
    let mut total_elements = 0u64;
    let mut total_bytes = 0u64;
    let mut total_time = Duration::new(0, 0);
    
    for file_path in test_files {
        if Path::new(file_path).exists() {
            let content = fs::read_to_string(file_path)?;
            let file_size = content.len();
            
            // We need to leak the string to get a 'static reference for the parser
            let static_content: &'static str = Box::leak(content.into_boxed_str());
            
            println!("Testing Rust JSON parser performance on {} ({} bytes)", file_path, file_size);
            
            // Run multiple iterations for statistical accuracy
            let iterations = 10;
            let mut file_elements = 0;
            let mut file_time = Duration::new(0, 0);
            
            for _ in 0..iterations {
                let mut parser = JsonParser::new(static_content);
                
                let start_time = Instant::now();
                let result = parser.parse();
                let elapsed = start_time.elapsed();
                
                match result {
                    Ok(json) => {
                        assert!(json.is_valid());
                        file_elements = json.size();
                        file_time += elapsed;
                    }
                    Err(error) => {
                        println!("❌ JSON parsing failed: {}", error);
                        return Err(error.into());
                    }
                }
            }
            
            let avg_time = file_time / iterations as u32;
            let bytes_per_second = if avg_time.as_secs_f64() > 0.0 {
                file_size as f64 / avg_time.as_secs_f64()
            } else {
                0.0
            };
            
            println!("  Elements: {}, Avg Time: {:?}, Bytes/sec: {:.0}", 
                     file_elements, avg_time, bytes_per_second);
            
            total_elements += file_elements as u64;
            total_bytes += file_size as u64;
            total_time += avg_time;
        } else {
            println!("Warning: Test file {} not found, skipping...", file_path);
        }
    }
    
    let overall_bytes_per_sec = if total_time.as_secs_f64() > 0.0 {
        total_bytes as f64 / total_time.as_secs_f64()
    } else {
        0.0
    };
    
    println!("Rust JSON Parser Overall Performance:");
    println!("  Total elements: {}", total_elements);
    println!("  Total bytes: {}", total_bytes);
    println!("  Total time: {:?}", total_time);
    println!("  Average bytes/second: {:.0}", overall_bytes_per_sec);
    println!("  Average MB/sec: {:.2}", overall_bytes_per_sec / (1024.0 * 1024.0));
    
    Ok(())
}

fn generate_deeply_nested_json(depth: usize) -> String {
    let mut json = String::new();
    
    for _ in 0..depth {
        json.push_str("{\"nested\":");
    }
    
    json.push_str("\"value\"");
    
    for _ in 0..depth {
        json.push('}');
    }
    
    json
}

fn generate_wide_json(count: usize) -> String {
    let mut json = String::from("{");
    
    for i in 0..count {
        if i > 0 {
            json.push(',');
        }
        json.push_str(&format!("\"key{}\": {}", i, i));
    }
    
    json.push('}');
    json
}

pub fn benchmark_json_parser_stress_test() -> Result<(), Box<dyn std::error::Error>> {
    println!("Running Rust JSON parser stress tests...");
    
    // Test deeply nested structures
    let deeply_nested = generate_deeply_nested_json(1000);
    let static_nested: &'static str = Box::leak(deeply_nested.into_boxed_str());
    
    let start = Instant::now();
    let mut parser = JsonParser::new(static_nested);
    let result = parser.parse()?;
    let elapsed = start.elapsed();
    
    assert!(result.is_valid());
    println!("  Deeply nested (1000 levels): {:?}", elapsed);
    
    // Test wide structures
    let wide_structure = generate_wide_json(10000);
    let static_wide: &'static str = Box::leak(wide_structure.into_boxed_str());
    
    let start = Instant::now();
    let mut parser = JsonParser::new(static_wide);
    let result = parser.parse()?;
    let elapsed = start.elapsed();
    
    assert!(result.is_valid());
    println!("  Wide structure (10000 keys): {:?}", elapsed);
    
    Ok(())
}

fn main() {
    println!("Running Rust JSON Parser Benchmarks...");
    
    if let Err(e) = benchmark_json_parser_real_world() {
        eprintln!("Error running real-world JSON parser benchmark: {}", e);
        std::process::exit(1);
    }
    
    if let Err(e) = benchmark_json_parser_stress_test() {
        eprintln!("Error running JSON parser stress test: {}", e);
        std::process::exit(1);
    }
    
    println!("Rust JSON parser benchmarks completed successfully!");
}