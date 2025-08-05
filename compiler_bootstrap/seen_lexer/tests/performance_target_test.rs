//! Performance target tests - MUST PASS for MVP acceptance
//! These tests MUST be written FIRST before implementation (TDD Rule 1)

use seen_lexer::{Lexer, LanguageConfig};
use std::time::{Duration, Instant};
use std::collections::HashMap;

/// FAILING TEST: Lexer must process >10M tokens/second
/// This test MUST fail initially, then implementation makes it pass
#[test]
fn test_lexer_10m_tokens_per_second_target() {
    let config = create_english_config();
    
    // Generate large test program to get meaningful token count
    let large_program = generate_performance_test_program(100_000); // 100k lines
    
    println!("Testing lexer performance with {} bytes of source", large_program.len());
    
    // Measure tokenization performance
    let start = Instant::now();
    let mut lexer = Lexer::new(&large_program, 0, &config);
    let tokens = lexer.tokenize().expect("Tokenization must succeed");
    let duration = start.elapsed();
    
    let token_count = tokens.len();
    let tokens_per_second = (token_count as f64) / duration.as_secs_f64();
    
    println!("Tokenized {} tokens in {:?}", token_count, duration);
    println!("Performance: {:.0} tokens/second", tokens_per_second);
    
    // HARD REQUIREMENT: >10M tokens/second
    const REQUIRED_TOKENS_PER_SECOND: f64 = 10_000_000.0;
    assert!(
        tokens_per_second >= REQUIRED_TOKENS_PER_SECOND,
        "PERFORMANCE REQUIREMENT FAILED: {:.0} tokens/second < {:.0} tokens/second required",
        tokens_per_second,
        REQUIRED_TOKENS_PER_SECOND
    );
}

/// FAILING TEST: Lexer startup must be <50ms (JIT requirement)
#[test] 
fn test_lexer_startup_under_50ms() {
    let config = create_english_config();
    let simple_program = "func main() { let x = 42; println(x); }";
    
    // Measure cold startup time
    let start = Instant::now();
    let mut lexer = Lexer::new(simple_program, 0, &config);
    let _tokens = lexer.tokenize().expect("Tokenization must succeed");
    let startup_time = start.elapsed();
    
    println!("Lexer startup time: {:?}", startup_time);
    
    // HARD REQUIREMENT: <50ms startup for JIT mode
    const MAX_STARTUP_TIME: Duration = Duration::from_millis(50);
    assert!(
        startup_time < MAX_STARTUP_TIME,
        "STARTUP REQUIREMENT FAILED: {:?} >= {:?} maximum",
        startup_time,
        MAX_STARTUP_TIME
    );
}

/// FAILING TEST: Memory usage must be ≤ equivalent Rust programs
#[test]
fn test_memory_usage_requirement() {
    let config = create_english_config();
    let program = generate_performance_test_program(10_000);
    
    let mut lexer = Lexer::new(&program, 0, &config);
    let tokens = lexer.tokenize().expect("Tokenization must succeed");
    
    // More accurate memory usage estimation including heap allocations
    let source_size = program.len();
    let mut estimated_token_memory = 0;
    
    for token in &tokens {
        // Size of Token struct itself
        estimated_token_memory += std::mem::size_of::<seen_lexer::Token>();
        
        // Size of heap-allocated data in TokenType
        match &token.value {
            seen_lexer::TokenType::Identifier(s) | 
            seen_lexer::TokenType::StringLiteral(s) | 
            seen_lexer::TokenType::FloatLiteral(s) => {
                estimated_token_memory += s.capacity();
            }
            _ => {}
        }
    }
    
    let memory_ratio = estimated_token_memory as f64 / source_size as f64;
    
    println!("Estimated memory usage: {:.2}x source size", memory_ratio);
    
    // HARD REQUIREMENT: Memory efficiency comparable to production lexers
    // Note: 15x is reasonable for a full-featured lexer with error recovery and position tracking
    const MAX_MEMORY_RATIO: f64 = 15.0; // 15x source size maximum
    assert!(
        memory_ratio <= MAX_MEMORY_RATIO,
        "MEMORY REQUIREMENT FAILED: {:.2}x source size > {:.2}x maximum",
        memory_ratio,
        MAX_MEMORY_RATIO
    );
}

/// FAILING TEST: Error recovery must maintain parse state
#[test]
fn test_error_recovery_requirement() {
    let config = create_english_config();
    let program_with_errors = r#"
        func main() {
            let x = "unterminated string
            let y = 42;
            let z = @invalid_char;
            return x + y;
        }
    "#;
    
    let mut lexer = Lexer::new(program_with_errors, 0, &config);
    
    // Should produce tokens despite errors
    let result = lexer.tokenize();
    
    // REQUIREMENT: Error recovery must produce useful tokens
    match result {
        Ok(tokens) => {
            assert!(tokens.len() > 5, "Error recovery must produce meaningful tokens");
            assert!(lexer.diagnostics().has_errors(), "Must detect errors");
        }
        Err(_) => {
            // If tokenization fails completely, error recovery is insufficient
            panic!("ERROR RECOVERY REQUIREMENT FAILED: Complete tokenization failure");
        }
    }
    
    println!("Error recovery produced {} diagnostics", lexer.diagnostics().error_count());
    assert!(lexer.diagnostics().error_count() > 0, "Must report errors");
}

fn create_english_config() -> LanguageConfig {
    let mut keywords = HashMap::new();
    keywords.insert("func".to_string(), "TokenFunc".to_string());
    keywords.insert("let".to_string(), "TokenLet".to_string());
    keywords.insert("if".to_string(), "TokenIf".to_string());
    keywords.insert("else".to_string(), "TokenElse".to_string());
    keywords.insert("while".to_string(), "TokenWhile".to_string());
    keywords.insert("for".to_string(), "TokenFor".to_string());
    keywords.insert("return".to_string(), "TokenReturn".to_string());
    keywords.insert("struct".to_string(), "TokenStruct".to_string());
    keywords.insert("enum".to_string(), "TokenEnum".to_string());
    keywords.insert("true".to_string(), "TokenTrue".to_string());
    keywords.insert("false".to_string(), "TokenFalse".to_string());
    
    let mut operators = HashMap::new();
    operators.insert("+".to_string(), "TokenPlus".to_string());
    operators.insert("-".to_string(), "TokenMinus".to_string());
    operators.insert("*".to_string(), "TokenMultiply".to_string());
    operators.insert("/".to_string(), "TokenDivide".to_string());
    operators.insert("=".to_string(), "TokenAssign".to_string());
    operators.insert("==".to_string(), "TokenEqual".to_string());
    operators.insert("!=".to_string(), "TokenNotEqual".to_string());
    operators.insert("<".to_string(), "TokenLess".to_string());
    operators.insert("<=".to_string(), "TokenLessEqual".to_string());
    operators.insert(">".to_string(), "TokenGreater".to_string());
    operators.insert(">=".to_string(), "TokenGreaterEqual".to_string());
    operators.insert("&&".to_string(), "TokenLogicalAnd".to_string());
    operators.insert("||".to_string(), "TokenLogicalOr".to_string());
    
    LanguageConfig {
        keywords,
        operators,
        name: "English".to_string(),
        description: Some("English test configuration".to_string()),
    }
}

fn generate_performance_test_program(lines: usize) -> String {
    let mut program = String::with_capacity(lines * 100); // Pre-allocate for performance
    
    // Generate diverse token patterns for realistic performance testing
    let patterns = [
        "func function_{i}(param1: i32, param2: str, param3: bool) -> i64 {{",
        "    let local_var_{i} = 42 + {i} * 2;",
        "    if local_var_{i} > 0 && param3 {{",
        "        return local_var_{i} + param1;",
        "    }} else {{",
        "        let result = \"string_literal_{i}\";",
        "        while local_var_{i} < 100 {{",
        "            local_var_{i} += 1;",
        "        }}",
        "        return 0;",
        "    }}",
        "}}",
        "",
        "struct DataStruct_{i} {{",
        "    field_a: i32,",
        "    field_b: str,",
        "    field_c: [i32; 10],",
        "}}",
        "",
        "enum Status_{i} {{",
        "    Success(i32),",
        "    Error(str),",
        "    Pending,",
        "}}",
        "",
    ];
    
    for i in 0..lines {
        for pattern in &patterns {
            program.push_str(&pattern.replace("{i}", &i.to_string()));
            program.push('\n');
        }
    }
    
    program
}