//! Integration tests for the Seen parser

use seen_lexer::{KeywordConfig, KeywordManager, Lexer};
use seen_parser::{parse_program, Parser};
use std::path::PathBuf;

/// Helper function to create keyword manager for testing
fn create_test_keyword_manager(language: &str) -> KeywordManager {
    // Get the specifications directory relative to the parser crate
    let lang_files_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent() // Go up from seen_parser crate root to workspace root
        .unwrap()
        .join("specifications");

    let keyword_config = KeywordConfig::from_directory(&lang_files_dir)
        .expect("Failed to load keyword configuration for testing");

    let active_lang = match language {
        "english" | "en" => "en".to_string(),
        "arabic" | "ar" => "ar".to_string(),
        _ => "en".to_string(), // Default to English
    };

    KeywordManager::new(keyword_config, active_lang)
        .expect("Failed to create KeywordManager for testing")
}

#[test]
fn test_parse_hello_world_english() {
    let source = r#"
    func main() {
        val greeting = "Hello, World!";
        println(greeting);
    }
    "#;

    let keyword_manager = create_test_keyword_manager("english");
    let mut lexer = Lexer::new(source, &keyword_manager, "en".to_string());
    let tokens = lexer.tokenize().unwrap();

    let program = parse_program(tokens).unwrap();

    // Should have one function declaration
    assert_eq!(program.declarations.len(), 1);
}

#[test]
fn test_parse_hello_world_arabic() {
    let source = r#"
    دالة رئيسية() {
        ثابت تحية = "مرحباً، يا عالم!";
        اطبع(تحية);
    }
    "#;

    let keyword_manager = create_test_keyword_manager("arabic");
    let mut lexer = Lexer::new(source, &keyword_manager, "ar".to_string());
    let tokens = lexer.tokenize().unwrap();

    let program = parse_program(tokens).unwrap();

    // Should have one function declaration
    assert_eq!(program.declarations.len(), 1);
}

#[test]
fn test_parse_fibonacci_program() {
    let source = r#"
    func fibonacci(n: Int) -> Int {
        if n <= 1 {
            return n;
        }
        return fibonacci(n - 1) + fibonacci(n - 2);
    }
    
    func main() {
        var i = 0;
        while i < 10 {
            val result = fibonacci(i);
            println(result);
            i = i + 1;
        }
    }
    "#;

    let keyword_manager = create_test_keyword_manager("english");
    let mut lexer = Lexer::new(source, &keyword_manager, "en".to_string());
    let tokens = lexer.tokenize().unwrap();

    let program = parse_program(tokens).unwrap();

    // Should have two function declarations
    assert_eq!(program.declarations.len(), 2);
}

#[test]
fn test_parse_complex_expressions() {
    let source = r#"
    func calculate() {
        val a = 5 + 3 * 2;
        val b = (5 + 3) * 2;
        val c = -a + b;
        val d = !true || false && true;
        val e = arr[i * 2 + 1];
        val f = foo(bar(1, 2), baz(3));
    }
    "#;

    let keyword_manager = create_test_keyword_manager("english");
    let mut lexer = Lexer::new(source, &keyword_manager, "en".to_string());
    let tokens = lexer.tokenize().unwrap();

    let program = parse_program(tokens).unwrap();

    assert_eq!(program.declarations.len(), 1);
}

#[test]
fn test_parse_control_flow() {
    let source = r#"
    func testControlFlow(x: Int) -> Int {
        if x < 0 {
            return -x;
        } else if x == 0 {
            return 0;
        } else {
            var sum = 0;
            for i in 0..x {
                sum = sum + i;
            }
            return sum;
        }
    }
    "#;

    let keyword_manager = create_test_keyword_manager("english");
    let mut lexer = Lexer::new(source, &keyword_manager, "en".to_string());
    let tokens = lexer.tokenize().unwrap();

    let program = parse_program(tokens).unwrap();

    assert_eq!(program.declarations.len(), 1);
}

#[test]
fn test_parse_nested_structures() {
    let source = r#"
    func outer() {
        func inner() {
            func innermost() {
                return 42;
            }
            return innermost();
        }
        
        if true {
            while false {
                for x in [1, 2, 3] {
                    if x > 1 {
                        println(x);
                    }
                }
            }
        }
        
        return inner();
    }
    "#;

    let keyword_manager = create_test_keyword_manager("english");
    let mut lexer = Lexer::new(source, &keyword_manager, "en".to_string());
    let tokens = lexer.tokenize().unwrap();

    let program = parse_program(tokens).unwrap();

    assert_eq!(program.declarations.len(), 1);
}

#[test]
fn test_parse_various_types() {
    let source = r#"
    func testTypes() {
        val a: Int = 42;
        val b: Float = 3.14;
        val c: Bool = true;
        val d: String = "hello";
        val e: Array<Int> = [1, 2, 3];
        val f: Int? = null;
        val g: (Int, Int) -> Int = add;
        val h: Array<Array<Float>> = [[1.0, 2.0], [3.0, 4.0]];
    }
    "#;

    let keyword_manager = create_test_keyword_manager("english");
    let mut lexer = Lexer::new(source, &keyword_manager, "en".to_string());
    let tokens = lexer.tokenize().unwrap();

    let program = parse_program(tokens).unwrap();

    assert_eq!(program.declarations.len(), 1);
}

#[test]
fn test_parse_error_invalid_syntax() {
    let source = r#"
    func broken() {
        val x = ;  // Missing expression
    }
    "#;

    let keyword_manager = create_test_keyword_manager("english");
    let mut lexer = Lexer::new(source, &keyword_manager, "en".to_string());
    let tokens = lexer.tokenize().unwrap();

    let result = parse_program(tokens);
    assert!(result.is_err());
}

#[test]
fn test_parse_whitespace_handling() {
    // Test that parser handles various whitespace correctly
    let source = r#"func compact(){val x=1+2*3;return x;}"#;

    let keyword_manager = create_test_keyword_manager("english");
    let mut lexer = Lexer::new(source, &keyword_manager, "en".to_string());
    let tokens = lexer.tokenize().unwrap();

    let program = parse_program(tokens).unwrap();
    assert_eq!(program.declarations.len(), 1);
}

#[test]
fn test_parse_comments_between_tokens() {
    let source = r#"
    func /* this is a function */ main() {
        val x = 5 /* initialize x */ + /* add */ 10;
        // Single line comment
        return x; // return the result
    }
    "#;

    let keyword_manager = create_test_keyword_manager("english");
    let mut lexer = Lexer::new(source, &keyword_manager, "en".to_string());
    let tokens = lexer.tokenize().unwrap();

    let program = parse_program(tokens).unwrap();
    assert_eq!(program.declarations.len(), 1);
}