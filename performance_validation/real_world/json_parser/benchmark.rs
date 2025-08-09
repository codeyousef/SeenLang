// JSON Parser Benchmark - Rust Implementation
use std::collections::HashMap;
use std::time::Instant;
use std::env;

#[derive(Debug, Clone)]
enum JsonValue {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<JsonValue>),
    Object(HashMap<String, JsonValue>),
}

struct JsonParser {
    input: Vec<char>,
    pos: usize,
}

impl JsonParser {
    fn new(input: &str) -> Self {
        JsonParser {
            input: input.chars().collect(),
            pos: 0,
        }
    }
    
    fn skip_whitespace(&mut self) {
        while self.pos < self.input.len() && self.input[self.pos].is_whitespace() {
            self.pos += 1;
        }
    }
    
    fn peek(&self) -> Option<char> {
        if self.pos < self.input.len() {
            Some(self.input[self.pos])
        } else {
            None
        }
    }
    
    fn consume(&mut self) -> Result<char, String> {
        if self.pos >= self.input.len() {
            return Err("Unexpected end of input".to_string());
        }
        let c = self.input[self.pos];
        self.pos += 1;
        Ok(c)
    }
    
    fn parse_string(&mut self) -> Result<String, String> {
        self.consume()?; // Skip opening quote
        let mut result = String::new();
        
        while self.pos < self.input.len() {
            let c = self.consume()?;
            if c == '"' {
                return Ok(result);
            } else if c == '\\' {
                let next = self.consume()?;
                match next {
                    '"' => result.push('"'),
                    '\\' => result.push('\\'),
                    '/' => result.push('/'),
                    'b' => result.push('\u{0008}'),
                    'f' => result.push('\u{000C}'),
                    'n' => result.push('\n'),
                    'r' => result.push('\r'),
                    't' => result.push('\t'),
                    'u' => {
                        // Unicode escape
                        let mut hex = String::new();
                        for _ in 0..4 {
                            hex.push(self.consume()?);
                        }
                        if let Ok(codepoint) = u32::from_str_radix(&hex, 16) {
                            if let Some(c) = char::from_u32(codepoint) {
                                result.push(c);
                            } else {
                                result.push('?');
                            }
                        } else {
                            result.push('?');
                        }
                    }
                    _ => result.push(next),
                }
            } else {
                result.push(c);
            }
        }
        
        Err("Unterminated string".to_string())
    }
    
    fn parse_number(&mut self) -> Result<f64, String> {
        let start = self.pos;
        
        if self.peek() == Some('-') {
            self.consume()?;
        }
        
        // Integer part
        if self.peek() == Some('0') {
            self.consume()?;
        } else {
            while self.peek().map_or(false, |c| c.is_ascii_digit()) {
                self.consume()?;
            }
        }
        
        // Fractional part
        if self.peek() == Some('.') {
            self.consume()?;
            while self.peek().map_or(false, |c| c.is_ascii_digit()) {
                self.consume()?;
            }
        }
        
        // Exponent part
        if self.peek() == Some('e') || self.peek() == Some('E') {
            self.consume()?;
            if self.peek() == Some('+') || self.peek() == Some('-') {
                self.consume()?;
            }
            while self.peek().map_or(false, |c| c.is_ascii_digit()) {
                self.consume()?;
            }
        }
        
        let num_str: String = self.input[start..self.pos].iter().collect();
        num_str.parse().map_err(|e| format!("Invalid number: {}", e))
    }
    
    fn parse_array(&mut self) -> Result<Vec<JsonValue>, String> {
        self.consume()?; // Skip '['
        let mut arr = Vec::new();
        
        self.skip_whitespace();
        if self.peek() == Some(']') {
            self.consume()?;
            return Ok(arr);
        }
        
        loop {
            self.skip_whitespace();
            arr.push(self.parse_value()?);
            self.skip_whitespace();
            
            match self.peek() {
                Some(',') => {
                    self.consume()?;
                }
                Some(']') => {
                    self.consume()?;
                    break;
                }
                _ => return Err("Expected ',' or ']' in array".to_string()),
            }
        }
        
        Ok(arr)
    }
    
    fn parse_object(&mut self) -> Result<HashMap<String, JsonValue>, String> {
        self.consume()?; // Skip '{'
        let mut obj = HashMap::new();
        
        self.skip_whitespace();
        if self.peek() == Some('}') {
            self.consume()?;
            return Ok(obj);
        }
        
        loop {
            self.skip_whitespace();
            
            if self.peek() != Some('"') {
                return Err("Expected string key in object".to_string());
            }
            
            let key = self.parse_string()?;
            self.skip_whitespace();
            
            if self.consume()? != ':' {
                return Err("Expected ':' after object key".to_string());
            }
            
            self.skip_whitespace();
            let value = self.parse_value()?;
            obj.insert(key, value);
            self.skip_whitespace();
            
            match self.peek() {
                Some(',') => {
                    self.consume()?;
                }
                Some('}') => {
                    self.consume()?;
                    break;
                }
                _ => return Err("Expected ',' or '}' in object".to_string()),
            }
        }
        
        Ok(obj)
    }
    
    fn parse_value(&mut self) -> Result<JsonValue, String> {
        self.skip_whitespace();
        
        match self.peek() {
            Some('"') => Ok(JsonValue::String(self.parse_string()?)),
            Some('{') => Ok(JsonValue::Object(self.parse_object()?)),
            Some('[') => Ok(JsonValue::Array(self.parse_array()?)),
            Some('t') => {
                if self.input[self.pos..].starts_with(&['t', 'r', 'u', 'e']) {
                    self.pos += 4;
                    Ok(JsonValue::Bool(true))
                } else {
                    Err("Invalid value".to_string())
                }
            }
            Some('f') => {
                if self.input[self.pos..].starts_with(&['f', 'a', 'l', 's', 'e']) {
                    self.pos += 5;
                    Ok(JsonValue::Bool(false))
                } else {
                    Err("Invalid value".to_string())
                }
            }
            Some('n') => {
                if self.input[self.pos..].starts_with(&['n', 'u', 'l', 'l']) {
                    self.pos += 4;
                    Ok(JsonValue::Null)
                } else {
                    Err("Invalid value".to_string())
                }
            }
            Some(c) if c == '-' || c.is_ascii_digit() => {
                Ok(JsonValue::Number(self.parse_number()?))
            }
            _ => Err("Unexpected character in JSON".to_string()),
        }
    }
    
    fn parse(&mut self) -> Result<JsonValue, String> {
        let result = self.parse_value()?;
        self.skip_whitespace();
        if self.pos < self.input.len() {
            return Err("Unexpected characters after JSON value".to_string());
        }
        Ok(result)
    }
}

// Generate test JSON
fn generate_test_json(depth: i32, breadth: i32) -> String {
    if depth <= 0 {
        return "\"leaf\"".to_string();
    }
    
    let mut result = String::from("{");
    for i in 0..breadth {
        if i > 0 {
            result.push(',');
        }
        result.push_str(&format!("\"field{}\":", i));
        
        if i % 3 == 0 {
            result.push('[');
            for j in 0..3 {
                if j > 0 {
                    result.push(',');
                }
                result.push_str(&generate_test_json(depth - 1, breadth));
            }
            result.push(']');
        } else if i % 3 == 1 {
            result.push_str(&generate_test_json(depth - 1, breadth));
        } else {
            result.push_str(&format!("{}", i as f64 * 123.456));
        }
    }
    result.push('}');
    result
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let iterations = if args.len() > 1 {
        args[1].parse().unwrap_or(100)
    } else {
        100
    };
    
    // Generate complex test data
    let test_json = generate_test_json(5, 5);
    println!("Test JSON size: {} bytes", test_json.len());
    
    // Warmup
    for _ in 0..10 {
        let mut parser = JsonParser::new(&test_json);
        let _ = parser.parse();
    }
    
    // Benchmark
    let mut times = Vec::new();
    
    for _ in 0..iterations {
        let start = Instant::now();
        
        let mut parser = JsonParser::new(&test_json);
        let _ = parser.parse().unwrap();
        
        let duration = start.elapsed();
        times.push(duration.as_secs_f64());
    }
    
    // Calculate statistics
    let sum: f64 = times.iter().sum();
    let avg = sum / times.len() as f64;
    
    // Output results in JSON format
    println!("{{");
    println!("  \"language\": \"rust\",");
    println!("  \"benchmark\": \"json_parser\",");
    println!("  \"iterations\": {},", iterations);
    println!("  \"json_size\": {},", test_json.len());
    print!("  \"times\": [");
    for (i, time) in times.iter().enumerate() {
        print!("{}", time);
        if i < times.len() - 1 {
            print!(", ");
        }
    }
    println!("],");
    println!("  \"average_time\": {},", avg);
    println!("  \"throughput_mb_per_sec\": {}", test_json.len() as f64 / avg / 1048576.0);
    println!("}}")
}