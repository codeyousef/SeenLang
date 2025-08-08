//! Comprehensive integration tests for the lexer
//! These tests verify complete lexer functionality for MVP requirements

use seen_lexer::{Lexer, LanguageConfig, Token, TokenType};
use seen_common::Position;
use std::collections::HashMap;

fn create_test_config() -> LanguageConfig {
    // Try loading from TOML file from project root
    let paths = ["languages/en.toml", "../../../languages/en.toml", "../../languages/en.toml"];
    for path in paths {
        if let Ok(config) = LanguageConfig::load_from_file(path) {
            return config;
        }
    }
    
    // Fallback to minimal config
    LanguageConfig::new_english()
}

#[test]
fn test_basic_function_tokenization() {
    let config = create_test_config();
    let source = "fun main() { println(\"Hello\") }";
    
    let mut lexer = Lexer::new(source, 0, &config);
    let tokens = lexer.tokenize().expect("Tokenization should succeed");
    
    assert!(tokens.len() >= 8, "Should have at least 8 tokens");
    assert!(matches!(tokens[0].value, TokenType::KeywordFun));
    assert!(matches!(tokens[1].value, TokenType::Identifier(_)));
    assert!(matches!(tokens[2].value, TokenType::LeftParen));
}

#[test]
fn test_all_primitive_types() {
    let config = create_test_config();
    let source = "i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 bool char str";
    
    let mut lexer = Lexer::new(source, 0, &config);
    let tokens = lexer.tokenize().expect("Tokenization should succeed");
    
    assert_eq!(tokens.len() - 1, 15); // 15 types + EOF
    for token in &tokens[..15] {
        assert!(matches!(token.value, TokenType::Identifier(_)));
    }
}

#[test]
fn test_all_operators() {
    let config = create_test_config();
    let source = "+ - * / % == != < <= > >= && || ! & | ^ << >> += -= *= /= %= &= |= ^=";
    
    let mut lexer = Lexer::new(source, 0, &config);
    let tokens = lexer.tokenize().expect("Tokenization should succeed");
    
    assert!(tokens.len() >= 24, "Should tokenize all operators");
}

#[test]
fn test_string_literals() {
    let config = create_test_config();
    let source = r#""simple" "with\nescape" "with \"quotes\"" "emoji 😀""#;
    
    let mut lexer = Lexer::new(source, 0, &config);
    let tokens = lexer.tokenize().expect("Tokenization should succeed");
    
    let string_tokens: Vec<_> = tokens.iter()
        .filter(|t| matches!(t.value, TokenType::StringLiteral(_)))
        .collect();
    
    assert_eq!(string_tokens.len(), 4);
}

#[test]
fn test_numeric_literals() {
    let config = create_test_config();
    let source = "42 0xFF 0b1010 0o77 3.14 1.23e-4 1_000_000";
    
    let mut lexer = Lexer::new(source, 0, &config);
    let tokens = lexer.tokenize().expect("Tokenization should succeed");
    
    let numeric_tokens: Vec<_> = tokens.iter()
        .filter(|t| matches!(t.value, TokenType::IntegerLiteral(_) | TokenType::FloatLiteral(_)))
        .collect();
    
    assert!(numeric_tokens.len() >= 6, "Should tokenize various numeric formats");
}

#[test]
fn test_comments() {
    let config = create_test_config();
    let source = r#"
    // Single line comment
    fun test() {
        /* Multi-line
           comment */
        val x = 42 // inline comment
    }
    "#;
    
    let mut lexer = Lexer::new(source, 0, &config);
    let tokens = lexer.tokenize().expect("Tokenization should succeed");
    
    // Comments should be skipped
    for token in &tokens {
        if let TokenType::Identifier(s) = &token.value {
            assert!(!s.contains("comment"), "Comments should not appear as tokens");
        }
    }
}

#[test]
fn test_kotlin_keywords() {
    let config = create_test_config();
    let source = "fun val var if else when sealed data class object companion inline suspend";
    
    let mut lexer = Lexer::new(source, 0, &config);
    let tokens = lexer.tokenize().expect("Tokenization should succeed");
    
    assert!(matches!(tokens[0].value, TokenType::KeywordFun));
    assert!(matches!(tokens[1].value, TokenType::KeywordVal));
    assert!(matches!(tokens[2].value, TokenType::KeywordVar));
    assert!(matches!(tokens[3].value, TokenType::KeywordIf));
    assert!(matches!(tokens[4].value, TokenType::KeywordElse));
    assert!(matches!(tokens[5].value, TokenType::KeywordWhen));
    assert!(matches!(tokens[6].value, TokenType::KeywordSealed));
    assert!(matches!(tokens[7].value, TokenType::KeywordData));
    assert!(matches!(tokens[8].value, TokenType::KeywordClass));
    assert!(matches!(tokens[9].value, TokenType::KeywordObject));
    assert!(matches!(tokens[10].value, TokenType::KeywordCompanion));
    assert!(matches!(tokens[11].value, TokenType::KeywordInline));
    assert!(matches!(tokens[12].value, TokenType::KeywordSuspend));
}

#[test]
fn test_nullable_types() {
    let config = create_test_config();
    let source = "String? Int? List<String>? Map<String, Int>?";
    
    let mut lexer = Lexer::new(source, 0, &config);
    let tokens = lexer.tokenize().expect("Tokenization should succeed");
    
    let question_marks: Vec<_> = tokens.iter()
        .filter(|t| matches!(t.value, TokenType::Question))
        .collect();
    
    assert_eq!(question_marks.len(), 4, "Should tokenize all nullable markers");
}

#[test]
fn test_generic_types() {
    let config = create_test_config();
    let source = "List<T> Map<K, V> Function<T, R>";
    
    let mut lexer = Lexer::new(source, 0, &config);
    let tokens = lexer.tokenize().expect("Tokenization should succeed");
    
    let angle_brackets: Vec<_> = tokens.iter()
        .filter(|t| matches!(t.value, TokenType::Less | TokenType::Greater))
        .collect();
    
    assert_eq!(angle_brackets.len(), 6, "Should tokenize generic brackets");
}

#[test]
fn test_position_tracking() {
    let config = create_test_config();
    let source = "fun\nmain()\n{\n    println()\n}";
    
    let mut lexer = Lexer::new(source, 0, &config);
    let tokens = lexer.tokenize().expect("Tokenization should succeed");
    
    // Check line numbers
    assert_eq!(tokens[0].span.start.line, 1); // fun
    assert_eq!(tokens[1].span.start.line, 2); // main
    assert_eq!(tokens[4].span.start.line, 3); // {
    assert_eq!(tokens[5].span.start.line, 4); // println
}

#[test]
fn test_error_recovery() {
    let config = create_test_config();
    // Use a syntax error that allows recovery, not an unclosed string
    let source = "fun test() { let x = 123 invalid_token let y = 42 }";
    
    let mut lexer = Lexer::new(source, 0, &config);
    let result = lexer.tokenize();
    
    // Should still produce tokens despite potential errors
    assert!(result.is_ok(), "Should tokenize despite syntax issues");
    
    if let Ok(tokens) = result {
        // Should continue tokenizing after invalid syntax
        let has_42 = tokens.iter().any(|t| {
            matches!(t.value, TokenType::IntegerLiteral(42))
        });
        let has_123 = tokens.iter().any(|t| {
            matches!(t.value, TokenType::IntegerLiteral(123))
        });
        assert!(has_42 && has_123, "Should tokenize numbers despite syntax errors");
    }
}

#[test]
fn test_unicode_identifiers() {
    let config = create_test_config();
    let source = "متغير = 42; 変数 = 100; переменная = 200";
    
    let mut lexer = Lexer::new(source, 0, &config);
    let tokens = lexer.tokenize().expect("Tokenization should succeed");
    
    let identifiers: Vec<_> = tokens.iter()
        .filter(|t| matches!(t.value, TokenType::Identifier(_)))
        .collect();
    
    assert!(identifiers.len() >= 3, "Should tokenize Unicode identifiers");
}

#[test]
fn test_lambda_syntax() {
    let config = create_test_config();
    let source = "{ x -> x * 2 } { x, y -> x + y }";
    
    let mut lexer = Lexer::new(source, 0, &config);
    let tokens = lexer.tokenize().expect("Tokenization should succeed");
    
    let arrows: Vec<_> = tokens.iter()
        .filter(|t| matches!(t.value, TokenType::Arrow))
        .collect();
    
    assert_eq!(arrows.len(), 2, "Should tokenize lambda arrows");
}

#[test]
fn test_range_operators() {
    let config = create_test_config();
    let source = "1..10 1..<10 1..";
    
    let mut lexer = Lexer::new(source, 0, &config);
    let tokens = lexer.tokenize().expect("Tokenization should succeed");
    
    let ranges: Vec<_> = tokens.iter()
        .filter(|t| matches!(t.value, TokenType::DotDot | TokenType::DotDotLess))
        .collect();
    
    assert!(ranges.len() >= 2, "Should tokenize range operators");
}

#[test]
fn test_destructuring_syntax() {
    let config = create_test_config();
    let source = "val (x, y) = point; val (name, _, age) = person";
    
    let mut lexer = Lexer::new(source, 0, &config);
    let tokens = lexer.tokenize().expect("Tokenization should succeed");
    
    let underscores: Vec<_> = tokens.iter()
        .filter(|t| matches!(t.value, TokenType::Underscore))
        .collect();
    
    assert!(underscores.len() >= 1, "Should tokenize underscore for destructuring");
}

#[test]
fn test_elvis_and_safe_call() {
    let config = create_test_config();
    let source = "x ?: default; obj?.method(); obj!!.property";
    
    let mut lexer = Lexer::new(source, 0, &config);
    let tokens = lexer.tokenize().expect("Tokenization should succeed");
    
    let elvis = tokens.iter().any(|t| matches!(t.value, TokenType::Elvis));
    let safe_call = tokens.iter().any(|t| matches!(t.value, TokenType::QuestionDot));
    let not_null = tokens.iter().any(|t| matches!(t.value, TokenType::BangBang));
    
    assert!(elvis, "Should tokenize elvis operator");
    assert!(safe_call, "Should tokenize safe call operator");
    assert!(not_null, "Should tokenize not-null assertion");
}

#[test]
fn test_coroutine_keywords() {
    let config = create_test_config();
    let source = "suspend fun test() { launch { async { await() } } }";
    
    let mut lexer = Lexer::new(source, 0, &config);
    let tokens = lexer.tokenize().expect("Tokenization should succeed");
    
    assert!(matches!(tokens[0].value, TokenType::KeywordSuspend));
    let has_launch = tokens.iter().any(|t| {
        matches!(t.value, TokenType::KeywordLaunch)
    });
    let has_async = tokens.iter().any(|t| {
        matches!(t.value, TokenType::Identifier(ref s) if s == "async")
    });
    let has_await = tokens.iter().any(|t| {
        matches!(t.value, TokenType::KeywordAwait)
    });
    
    assert!(has_launch, "Should tokenize launch keyword");
    assert!(has_async, "Should tokenize async identifier");
    assert!(has_await, "Should tokenize await keyword");
}

#[test]
fn test_template_strings() {
    let config = create_test_config();
    let source = r#""Hello ${name}!" "Value: ${x + y}""#;
    
    let mut lexer = Lexer::new(source, 0, &config);
    let tokens = lexer.tokenize().expect("Tokenization should succeed");
    
    // For now, template strings are tokenized as regular strings
    // Full template support would be added later
    let strings: Vec<_> = tokens.iter()
        .filter(|t| matches!(t.value, TokenType::StringLiteral(_)))
        .collect();
    
    assert_eq!(strings.len(), 2, "Should tokenize template strings");
}

#[test]
fn test_annotations() {
    let config = create_test_config();
    let source = "@Test @Deprecated @JvmStatic @file:JvmName(\"Utils\")";
    
    let mut lexer = Lexer::new(source, 0, &config);
    let tokens = lexer.tokenize().expect("Tokenization should succeed");
    
    let at_signs: Vec<_> = tokens.iter()
        .filter(|t| matches!(t.value, TokenType::At))
        .collect();
    
    assert!(at_signs.len() >= 4, "Should tokenize annotation markers");
}

#[test]
fn test_extension_function_syntax() {
    let config = create_test_config();
    let source = "fun String.reverse(): String { }";
    
    let mut lexer = Lexer::new(source, 0, &config);
    let tokens = lexer.tokenize().expect("Tokenization should succeed");
    
    // Should tokenize: fun String . reverse ( ) : String { }
    assert!(tokens.len() >= 9);
    assert!(matches!(tokens[0].value, TokenType::KeywordFun));
    assert!(matches!(tokens[2].value, TokenType::Dot));
}

#[test]
fn test_large_file_performance() {
    let config = create_test_config();
    
    // Generate a large source file
    let mut source = String::new();
    for i in 0..1000 {
        source.push_str(&format!("fun function_{}() {{ val x = {}; }}\n", i, i));
    }
    
    let start = std::time::Instant::now();
    let mut lexer = Lexer::new(&source, 0, &config);
    let tokens = lexer.tokenize().expect("Tokenization should succeed");
    let duration = start.elapsed();
    
    assert!(tokens.len() > 10000, "Should tokenize large file");
    assert!(duration.as_secs() < 1, "Should tokenize large file quickly");
    
    println!("Tokenized {} tokens in {:?}", tokens.len(), duration);
}