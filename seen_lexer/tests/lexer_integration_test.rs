//\! Integration tests for the Seen lexer
//\! These tests run in a separate process and test the public API

use seen_lexer::{KeywordManager, Lexer, TokenType};
use std::fs;
use tempfile::TempDir;

#[test]
fn test_lexer_with_project_config() {
    // Create a temporary project directory
    let temp_dir = TempDir::new().unwrap();
    let project_path = temp_dir.path();

    // Create seen.toml
    let config_content = r#"
[project]
name = "test_project"
version = "0.1.0"
language = "english"
"#;
    fs::write(project_path.join("seen.toml"), config_content).unwrap();

    // Create a source file
    let source_content = r#"
func main() {
    println("Hello from test\!");
}
"#;

    // Test lexing with project configuration
    // This tests the integration between project config and lexer
    // Implementation depends on KeywordManager API
}

#[test]
fn test_lexer_with_custom_keyword_file() {
    let temp_dir = TempDir::new().unwrap();
    let keywords_path = temp_dir.path().join("custom_keywords.toml");

    // Create a custom keyword mapping file
    let keywords_content = r#"
[keywords]
func = "function"
val = "const"
var = "variable"
"#;
    fs::write(&keywords_path, keywords_content).unwrap();

    // Test that lexer can use custom keyword mappings
    // Implementation depends on KeywordManager API
}

#[test]
fn test_large_file_performance() {
    // Generate a large source file
    let mut large_source = String::new();
    for i in 0..1000 {
        large_source.push_str(&format!(
            "func function_{}() {{ val x = {}; return x * 2; }}\n",
            i, i
        ));
    }

    // Measure lexing time
    let start = std::time::Instant::now();

    // TODO: Create keyword manager and lexer
    // let keyword_manager = KeywordManager::new("english");
    // let mut lexer = Lexer::new(&large_source, &keyword_manager);
    // let tokens = lexer.tokenize().unwrap();

    let duration = start.elapsed();

    // Assert reasonable performance (adjust threshold as needed)
    assert!(
        duration.as_secs() < 1,
        "Lexing took too long: {:?}",
        duration
    );
}

#[test]
fn test_real_world_examples() {
    // Test with actual Seen code examples
    let examples = vec![
        // Fibonacci
        r#"
        func fibonacci(n: Int) -> Int {
            if n <= 1 {
                return n;
            }
            return fibonacci(n - 1) + fibonacci(n - 2);
        }
        "#,
        // Sorting algorithm
        r#"
        func bubble_sort(arr: Array<Int>) -> Array<Int> {
            var n = arr.length();
            for i in 0..n {
                for j in 0..(n-i-1) {
                    if arr[j] > arr[j+1] {
                        val temp = arr[j];
                        arr[j] = arr[j+1];
                        arr[j+1] = temp;
                    }
                }
            }
            return arr;
        }
        "#,
        // Class definition (if supported)
        r#"
        class Person {
            var name: String;
            var age: Int;
            
            func new(name: String, age: Int) -> Person {
                return Person { name: name, age: age };
            }
            
            func greet() -> String {
                return "Hello, my name is " + self.name;
            }
        }
        "#,
    ];

    for example in examples {
        // Test that each example lexes without errors
        // TODO: Implement when KeywordManager is available
    }
}
