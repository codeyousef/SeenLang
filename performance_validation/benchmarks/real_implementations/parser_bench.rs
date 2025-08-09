// Real Parser Benchmark - Rust Implementation
use std::env;
use std::fs;
use std::time::Instant;

#[derive(Debug, Clone)]
enum TokenType {
    Identifier,
    Number,
    String,
    Keyword,
    Operator,
    Delimiter,
    End,
}

#[derive(Debug, Clone)]
struct Token {
    token_type: TokenType,
    value: String,
}

#[derive(Debug)]
struct ASTNode {
    node_type: String,
    value: String,
    children: Vec<Box<ASTNode>>,
}

impl ASTNode {
    fn new(node_type: &str) -> Self {
        ASTNode {
            node_type: node_type.to_string(),
            value: String::new(),
            children: Vec::new(),
        }
    }
    
    fn new_with_value(node_type: &str, value: &str) -> Self {
        ASTNode {
            node_type: node_type.to_string(),
            value: value.to_string(),
            children: Vec::new(),
        }
    }
}

struct SimpleParser {
    input: Vec<char>,
    position: usize,
    current_token: Token,
    nodes_created: usize,
}

impl SimpleParser {
    fn new(input: &str) -> Self {
        let mut parser = SimpleParser {
            input: input.chars().collect(),
            position: 0,
            current_token: Token { token_type: TokenType::End, value: String::new() },
            nodes_created: 0,
        };
        parser.current_token = parser.next_token();
        parser
    }
    
    fn skip_whitespace(&mut self) {
        while self.position < self.input.len() && self.input[self.position].is_whitespace() {
            self.position += 1;
        }
    }
    
    fn next_token(&mut self) -> Token {
        self.skip_whitespace();
        
        if self.position >= self.input.len() {
            return Token { token_type: TokenType::End, value: String::new() };
        }
        
        let c = self.input[self.position];
        
        // Identifier or keyword
        if c.is_alphabetic() || c == '_' {
            let mut value = String::new();
            while self.position < self.input.len() && 
                  (self.input[self.position].is_alphanumeric() || self.input[self.position] == '_') {
                value.push(self.input[self.position]);
                self.position += 1;
            }
            
            let token_type = match value.as_str() {
                "fun" | "val" | "var" | "if" | "else" | "while" | "for" | 
                "return" | "class" | "interface" | "import" => TokenType::Keyword,
                _ => TokenType::Identifier,
            };
            
            return Token { token_type, value };
        }
        
        // Number
        if c.is_numeric() {
            let mut value = String::new();
            while self.position < self.input.len() && 
                  (self.input[self.position].is_numeric() || self.input[self.position] == '.') {
                value.push(self.input[self.position]);
                self.position += 1;
            }
            return Token { token_type: TokenType::Number, value };
        }
        
        // String
        if c == '"' || c == '\'' {
            let quote = c;
            self.position += 1;
            let mut value = String::new();
            while self.position < self.input.len() && self.input[self.position] != quote {
                if self.input[self.position] == '\\' && self.position + 1 < self.input.len() {
                    self.position += 1;
                }
                value.push(self.input[self.position]);
                self.position += 1;
            }
            if self.position < self.input.len() {
                self.position += 1; // Skip closing quote
            }
            return Token { token_type: TokenType::String, value };
        }
        
        // Operators and delimiters
        let operators = "+-*/%=<>!&|";
        let delimiters = "(){}[],;:.";
        
        if operators.contains(c) {
            let mut value = String::from(c);
            self.position += 1;
            
            // Check for two-character operators
            if self.position < self.input.len() {
                let next = self.input[self.position];
                if (c == '=' && next == '=') || (c == '!' && next == '=') ||
                   (c == '<' && next == '=') || (c == '>' && next == '=') ||
                   (c == '&' && next == '&') || (c == '|' && next == '|') {
                    value.push(next);
                    self.position += 1;
                }
            }
            return Token { token_type: TokenType::Operator, value };
        }
        
        if delimiters.contains(c) {
            self.position += 1;
            return Token { token_type: TokenType::Delimiter, value: String::from(c) };
        }
        
        // Unknown character, skip it
        self.position += 1;
        self.next_token()
    }
    
    fn parse(&mut self) -> Box<ASTNode> {
        let mut root = Box::new(ASTNode::new("Program"));
        self.nodes_created += 1;
        
        while !matches!(self.current_token.token_type, TokenType::End) {
            if let Some(stmt) = self.parse_statement() {
                root.children.push(stmt);
            }
        }
        
        root
    }
    
    fn parse_statement(&mut self) -> Option<Box<ASTNode>> {
        match &self.current_token.token_type {
            TokenType::Keyword => {
                match self.current_token.value.as_str() {
                    "fun" => Some(self.parse_function_declaration()),
                    "val" | "var" => Some(self.parse_variable_declaration()),
                    "if" => Some(self.parse_if_statement()),
                    "while" => Some(self.parse_while_statement()),
                    "for" => Some(self.parse_for_statement()),
                    "return" => Some(self.parse_return_statement()),
                    "class" => Some(self.parse_class_declaration()),
                    _ => self.parse_expression(),
                }
            }
            _ => self.parse_expression(),
        }
    }
    
    fn parse_function_declaration(&mut self) -> Box<ASTNode> {
        let mut node = Box::new(ASTNode::new("FunctionDecl"));
        self.nodes_created += 1;
        
        self.current_token = self.next_token(); // Skip 'fun'
        if matches!(self.current_token.token_type, TokenType::Identifier) {
            node.value = self.current_token.value.clone();
            self.current_token = self.next_token();
        }
        
        // Parse parameters
        if matches!(self.current_token.token_type, TokenType::Delimiter) && self.current_token.value == "(" {
            self.current_token = self.next_token();
            while !(matches!(self.current_token.token_type, TokenType::Delimiter) && self.current_token.value == ")") {
                if matches!(self.current_token.token_type, TokenType::Identifier) {
                    let param = Box::new(ASTNode::new_with_value("Parameter", &self.current_token.value));
                    self.nodes_created += 1;
                    node.children.push(param);
                }
                self.current_token = self.next_token();
            }
            self.current_token = self.next_token(); // Skip ')'
        }
        
        // Parse body
        if matches!(self.current_token.token_type, TokenType::Delimiter) && self.current_token.value == "{" {
            node.children.push(self.parse_block());
        }
        
        node
    }
    
    fn parse_variable_declaration(&mut self) -> Box<ASTNode> {
        let mut node = Box::new(ASTNode::new_with_value("VarDecl", &self.current_token.value));
        self.nodes_created += 1;
        
        self.current_token = self.next_token(); // Skip 'val' or 'var'
        if matches!(self.current_token.token_type, TokenType::Identifier) {
            let id = Box::new(ASTNode::new_with_value("Identifier", &self.current_token.value));
            self.nodes_created += 1;
            node.children.push(id);
            self.current_token = self.next_token();
        }
        
        if matches!(self.current_token.token_type, TokenType::Operator) && self.current_token.value == "=" {
            self.current_token = self.next_token();
            if let Some(expr) = self.parse_expression() {
                node.children.push(expr);
            }
        }
        
        node
    }
    
    fn parse_if_statement(&mut self) -> Box<ASTNode> {
        let mut node = Box::new(ASTNode::new("IfStatement"));
        self.nodes_created += 1;
        
        self.current_token = self.next_token(); // Skip 'if'
        if matches!(self.current_token.token_type, TokenType::Delimiter) && self.current_token.value == "(" {
            self.current_token = self.next_token();
            if let Some(expr) = self.parse_expression() {
                node.children.push(expr);
            }
            if matches!(self.current_token.token_type, TokenType::Delimiter) && self.current_token.value == ")" {
                self.current_token = self.next_token();
            }
        }
        
        if let Some(stmt) = self.parse_statement() {
            node.children.push(stmt);
        }
        
        if matches!(self.current_token.token_type, TokenType::Keyword) && self.current_token.value == "else" {
            self.current_token = self.next_token();
            if let Some(stmt) = self.parse_statement() {
                node.children.push(stmt);
            }
        }
        
        node
    }
    
    fn parse_while_statement(&mut self) -> Box<ASTNode> {
        let mut node = Box::new(ASTNode::new("WhileStatement"));
        self.nodes_created += 1;
        
        self.current_token = self.next_token(); // Skip 'while'
        if matches!(self.current_token.token_type, TokenType::Delimiter) && self.current_token.value == "(" {
            self.current_token = self.next_token();
            if let Some(expr) = self.parse_expression() {
                node.children.push(expr);
            }
            if matches!(self.current_token.token_type, TokenType::Delimiter) && self.current_token.value == ")" {
                self.current_token = self.next_token();
            }
        }
        
        if let Some(stmt) = self.parse_statement() {
            node.children.push(stmt);
        }
        
        node
    }
    
    fn parse_for_statement(&mut self) -> Box<ASTNode> {
        let mut node = Box::new(ASTNode::new("ForStatement"));
        self.nodes_created += 1;
        
        self.current_token = self.next_token(); // Skip 'for'
        if matches!(self.current_token.token_type, TokenType::Delimiter) && self.current_token.value == "(" {
            self.current_token = self.next_token();
            while !(matches!(self.current_token.token_type, TokenType::Delimiter) && self.current_token.value == ")") {
                self.current_token = self.next_token();
            }
            self.current_token = self.next_token(); // Skip ')'
        }
        
        if let Some(stmt) = self.parse_statement() {
            node.children.push(stmt);
        }
        
        node
    }
    
    fn parse_return_statement(&mut self) -> Box<ASTNode> {
        let mut node = Box::new(ASTNode::new("ReturnStatement"));
        self.nodes_created += 1;
        
        self.current_token = self.next_token(); // Skip 'return'
        if !(matches!(self.current_token.token_type, TokenType::Delimiter) && self.current_token.value == ";") {
            if let Some(expr) = self.parse_expression() {
                node.children.push(expr);
            }
        }
        
        node
    }
    
    fn parse_class_declaration(&mut self) -> Box<ASTNode> {
        let mut node = Box::new(ASTNode::new("ClassDecl"));
        self.nodes_created += 1;
        
        self.current_token = self.next_token(); // Skip 'class'
        if matches!(self.current_token.token_type, TokenType::Identifier) {
            node.value = self.current_token.value.clone();
            self.current_token = self.next_token();
        }
        
        if matches!(self.current_token.token_type, TokenType::Delimiter) && self.current_token.value == "{" {
            node.children.push(self.parse_block());
        }
        
        node
    }
    
    fn parse_block(&mut self) -> Box<ASTNode> {
        let mut node = Box::new(ASTNode::new("Block"));
        self.nodes_created += 1;
        
        if matches!(self.current_token.token_type, TokenType::Delimiter) && self.current_token.value == "{" {
            self.current_token = self.next_token();
            while !(matches!(self.current_token.token_type, TokenType::Delimiter) && self.current_token.value == "}") {
                if let Some(stmt) = self.parse_statement() {
                    node.children.push(stmt);
                }
                if matches!(self.current_token.token_type, TokenType::Delimiter) && self.current_token.value == ";" {
                    self.current_token = self.next_token();
                }
                if matches!(self.current_token.token_type, TokenType::End) {
                    break;
                }
            }
            if matches!(self.current_token.token_type, TokenType::Delimiter) && self.current_token.value == "}" {
                self.current_token = self.next_token(); // Skip '}'
            }
        }
        
        node
    }
    
    fn parse_expression(&mut self) -> Option<Box<ASTNode>> {
        self.parse_binary_expression(0)
    }
    
    fn parse_binary_expression(&mut self, min_precedence: i32) -> Option<Box<ASTNode>> {
        let mut left = self.parse_primary_expression()?;
        
        while matches!(self.current_token.token_type, TokenType::Operator) {
            let op = self.current_token.value.clone();
            let precedence = self.get_operator_precedence(&op);
            
            if precedence < min_precedence {
                break;
            }
            
            self.current_token = self.next_token();
            let right = self.parse_binary_expression(precedence + 1)?;
            
            let mut bin_op = Box::new(ASTNode::new_with_value("BinaryOp", &op));
            self.nodes_created += 1;
            bin_op.children.push(left);
            bin_op.children.push(right);
            left = bin_op;
        }
        
        Some(left)
    }
    
    fn parse_primary_expression(&mut self) -> Option<Box<ASTNode>> {
        match &self.current_token.token_type {
            TokenType::Number => {
                let node = Box::new(ASTNode::new_with_value("Number", &self.current_token.value));
                self.nodes_created += 1;
                self.current_token = self.next_token();
                Some(node)
            }
            TokenType::String => {
                let node = Box::new(ASTNode::new_with_value("String", &self.current_token.value));
                self.nodes_created += 1;
                self.current_token = self.next_token();
                Some(node)
            }
            TokenType::Identifier => {
                let node = Box::new(ASTNode::new_with_value("Identifier", &self.current_token.value));
                self.nodes_created += 1;
                self.current_token = self.next_token();
                
                // Check for function call
                if matches!(self.current_token.token_type, TokenType::Delimiter) && self.current_token.value == "(" {
                    let mut call = Box::new(ASTNode::new("FunctionCall"));
                    self.nodes_created += 1;
                    call.children.push(node);
                    
                    self.current_token = self.next_token();
                    while !(matches!(self.current_token.token_type, TokenType::Delimiter) && self.current_token.value == ")") {
                        if let Some(expr) = self.parse_expression() {
                            call.children.push(expr);
                        }
                        if matches!(self.current_token.token_type, TokenType::Delimiter) && self.current_token.value == "," {
                            self.current_token = self.next_token();
                        }
                        if matches!(self.current_token.token_type, TokenType::End) {
                            break;
                        }
                    }
                    if matches!(self.current_token.token_type, TokenType::Delimiter) && self.current_token.value == ")" {
                        self.current_token = self.next_token(); // Skip ')'
                    }
                    Some(call)
                } else {
                    Some(node)
                }
            }
            TokenType::Delimiter if self.current_token.value == "(" => {
                self.current_token = self.next_token();
                let expr = self.parse_expression();
                if matches!(self.current_token.token_type, TokenType::Delimiter) && self.current_token.value == ")" {
                    self.current_token = self.next_token();
                }
                expr
            }
            _ => {
                // Skip unknown tokens
                self.current_token = self.next_token();
                None
            }
        }
    }
    
    fn get_operator_precedence(&self, op: &str) -> i32 {
        match op {
            "=" => 1,
            "||" => 2,
            "&&" => 3,
            "==" | "!=" => 4,
            "<" | ">" | "<=" | ">=" => 5,
            "+" | "-" => 6,
            "*" | "/" | "%" => 7,
            _ => 0,
        }
    }
    
    fn get_node_count(&self) -> usize {
        self.nodes_created
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <input_file> [iterations]", args[0]);
        std::process::exit(1);
    }
    
    // Read input file
    let input = fs::read_to_string(&args[1])
        .expect(&format!("Error: Cannot read file {}", args[1]));
    
    let iterations = if args.len() > 2 {
        args[2].parse().unwrap_or(30)
    } else {
        30
    };
    
    // Warm-up
    for _ in 0..5 {
        let mut parser = SimpleParser::new(&input);
        parser.parse();
    }
    
    // Benchmark
    let mut times = Vec::new();
    let mut total_nodes = 0;
    
    for _ in 0..iterations {
        let start = Instant::now();
        
        let mut parser = SimpleParser::new(&input);
        let _ast = parser.parse();
        
        let elapsed = start.elapsed().as_secs_f64();
        times.push(elapsed);
        total_nodes = parser.get_node_count();
    }
    
    // Calculate statistics
    let sum: f64 = times.iter().sum();
    let mean = sum / times.len() as f64;
    
    // Output JSON results
    println!("{{");
    println!("  \"language\": \"rust\",");
    println!("  \"benchmark\": \"parser\",");
    println!("  \"iterations\": {},", iterations);
    println!("  \"nodes_processed\": {},", total_nodes);
    print!("  \"times\": [");
    for (i, t) in times.iter().enumerate() {
        if i > 0 { print!(", "); }
        print!("{}", t);
    }
    println!("],");
    println!("  \"average_time\": {},", mean);
    println!("  \"nodes_per_second\": {}", total_nodes as f64 / mean);
    println!("}}");
}