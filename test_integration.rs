use seen_lexer::{Lexer, KeywordManager};
use seen_parser::Parser;
use std::sync::Arc;

fn main() {
    // Test 1: Simple function
    test_parse("fun greet(name: String): String { return \"Hello!\" }");
    
    // Test 2: Async function
    test_parse("async fun fetchData(): Data { return await api.get() }");
    
    // Test 3: Lambda expression
    test_parse("{ x, y -> x + y }");
    
    // Test 4: If expression
    test_parse("if age >= 18 and hasPermission { \"allowed\" } else { \"denied\" }");
    
    // Test 5: String interpolation
    test_parse("\"Hello, {name}! You are {age} years old.\"");
    
    // Test 6: Nullable operators
    test_parse("user?.name ?: \"Unknown\"");
    
    // Test 7: Match expression
    test_parse("match value { 0 -> \"zero\", 1..10 -> \"small\", _ -> \"large\" }");
    
    println!("\nAll integration tests passed!");
}

fn test_parse(input: &str) {
    println!("Testing: {}", input);
    
    // Load keywords
    let mut keyword_manager = KeywordManager::new();
    let toml_path = "languages/en.toml";
    keyword_manager.load_from_toml_file(toml_path, "en")
        .expect("Failed to load keywords");
    
    // Create lexer
    let lexer = Lexer::new(input.to_string(), Arc::new(keyword_manager));
    
    // Create parser
    let mut parser = Parser::new(lexer);
    
    // Parse expression
    match parser.parse_expression() {
        Ok(expr) => println!("  ✓ Parsed successfully: {:?}", std::mem::discriminant(&expr)),
        Err(e) => panic!("  ✗ Parse failed: {:?}", e),
    }
}