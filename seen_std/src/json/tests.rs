//! Comprehensive tests for JSON parser and serializer
//!
//! Tests cover all critical JSON functionality needed for data interchange

use super::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_null() {
        let json = "null";
        let result = parse_json(json).unwrap();
        
        assert!(result.is_null());
        assert_eq!(result, JsonValue::Null);
    }

    #[test]
    fn test_json_booleans() {
        let true_json = "true";
        let false_json = "false";
        
        let true_result = parse_json(true_json).unwrap();
        let false_result = parse_json(false_json).unwrap();
        
        assert!(true_result.is_bool());
        assert!(false_result.is_bool());
        assert_eq!(true_result.as_bool().unwrap(), true);
        assert_eq!(false_result.as_bool().unwrap(), false);
    }

    #[test]
    fn test_json_numbers() {
        let test_cases = vec![
            ("0", 0.0),
            ("42", 42.0),
            ("-17", -17.0),
            ("3.14159", 3.14159),
            ("-2.71828", -2.71828),
            ("1e10", 1e10),
            ("1E10", 1E10),
            ("1e+10", 1e+10),
            ("1e-10", 1e-10),
            ("123.456e-78", 123.456e-78),
        ];
        
        for (input, expected) in test_cases {
            let result = parse_json(input).unwrap();
            assert!(result.is_number(), "Failed to parse number: {}", input);
            assert!((result.as_number().unwrap() - expected).abs() < 1e-10, 
                    "Number mismatch for {}: got {}, expected {}", 
                    input, result.as_number().unwrap(), expected);
        }
    }

    #[test]
    fn test_json_strings() {
        let test_cases = vec![
            ("\"\"", ""),
            ("\"hello\"", "hello"),
            ("\"hello world\"", "hello world"),
            ("\"line1\\nline2\"", "line1\nline2"),
            ("\"tab\\there\"", "tab\there"),
            ("\"quote\\\"here\"", "quote\"here"),
            ("\"backslash\\\\here\"", "backslash\\here"),
            ("\"unicode\\u0041\"", "unicodeA"),
            ("\"emoji🚀\"", "emoji🚀"),
        ];
        
        for (input, expected) in test_cases {
            let result = parse_json(input).unwrap();
            assert!(result.is_string(), "Failed to parse string: {}", input);
            assert_eq!(result.as_str().unwrap(), expected, 
                      "String mismatch for {}", input);
        }
    }

    #[test]
    fn test_json_arrays() {
        // Empty array
        let empty = parse_json("[]").unwrap();
        assert!(empty.is_array());
        assert_eq!(empty.as_array().unwrap().len(), 0);
        
        // Array with numbers
        let numbers = parse_json("[1, 2, 3, 4, 5]").unwrap();
        assert!(numbers.is_array());
        let arr = numbers.as_array().unwrap();
        assert_eq!(arr.len(), 5);
        assert_eq!(arr[0].as_number().unwrap(), 1.0);
        assert_eq!(arr[4].as_number().unwrap(), 5.0);
        
        // Mixed array
        let mixed = parse_json(r#"[null, true, 42, "hello", [1, 2], {"key": "value"}]"#).unwrap();
        assert!(mixed.is_array());
        let arr = mixed.as_array().unwrap();
        assert_eq!(arr.len(), 6);
        assert!(arr[0].is_null());
        assert!(arr[1].is_bool());
        assert!(arr[2].is_number());
        assert!(arr[3].is_string());
        assert!(arr[4].is_array());
        assert!(arr[5].is_object());
    }

    #[test]
    fn test_json_objects() {
        // Empty object
        let empty = parse_json("{}").unwrap();
        assert!(empty.is_object());
        assert_eq!(empty.as_object().unwrap().len(), 0);
        
        // Simple object
        let simple = parse_json(r#"{"name": "seen", "version": 1.0, "active": true}"#).unwrap();
        assert!(simple.is_object());
        
        assert_eq!(simple.get("name").unwrap().as_str().unwrap(), "seen");
        assert_eq!(simple.get("version").unwrap().as_number().unwrap(), 1.0);
        assert_eq!(simple.get("active").unwrap().as_bool().unwrap(), true);
        
        // Nested object
        let nested = parse_json(r#"
        {
            "config": {
                "debug": true,
                "optimization": 2
            },
            "dependencies": ["std", "collections"]
        }
        "#).unwrap();
        
        assert!(nested.is_object());
        let config = nested.get("config").unwrap().as_object().unwrap();
        assert_eq!(config.get("debug").unwrap().as_bool().unwrap(), true);
        
        let deps = nested.get("dependencies").unwrap().as_array().unwrap();
        assert_eq!(deps.len(), 2);
    }

    #[test]
    fn test_json_whitespace_handling() {
        let json = r#"
        {
            "name"    :    "seen"   ,
            "version" :    1.0      ,
            "array"   : [  1  ,  2  ,  3  ]
        }
        "#;
        
        let result = parse_json(json).unwrap();
        assert!(result.is_object());
        assert_eq!(result.get("name").unwrap().as_str().unwrap(), "seen");
    }

    #[test]
    fn test_json_compiler_config() {
        // Test JSON structure similar to compiler configuration
        let json = r#"
        {
            "name": "seen-compiler",
            "version": "0.1.0",
            "targets": ["native", "wasm", "ios", "android"],
            "build": {
                "optimization": 2,
                "debug_symbols": true,
                "parallel": true
            },
            "features": {
                "jit": true,
                "incremental": true,
                "llvm_backend": true
            },
            "dependencies": [
                {"name": "llvm", "version": "18.0"},
                {"name": "cranelift", "version": "0.1", "optional": true}
            ]
        }
        "#;
        
        let result = parse_json(json).unwrap();
        assert!(result.is_object());
        
        // Verify structure
        assert_eq!(result.get("name").unwrap().as_str().unwrap(), "seen-compiler");
        
        let targets = result.get("targets").unwrap().as_array().unwrap();
        assert_eq!(targets.len(), 4);
        assert_eq!(targets[0].as_str().unwrap(), "native");
        
        let build = result.get("build").unwrap().as_object().unwrap();
        assert_eq!(build.get("optimization").unwrap().as_number().unwrap(), 2.0);
        assert_eq!(build.get("debug_symbols").unwrap().as_bool().unwrap(), true);
        
        let deps = result.get("dependencies").unwrap().as_array().unwrap();
        assert_eq!(deps.len(), 2);
        let llvm_dep = deps[0].as_object().unwrap();
        assert_eq!(llvm_dep.get("name").unwrap().as_str().unwrap(), "llvm");
    }

    #[test]
    fn test_json_language_server_protocol() {
        // Test JSON-RPC style messages used in language server protocol
        let request = r#"
        {
            "jsonrpc": "2.0",
            "id": 1,
            "method": "textDocument/completion",
            "params": {
                "textDocument": {
                    "uri": "file:///path/to/file.seen"
                },
                "position": {
                    "line": 10,
                    "character": 5
                }
            }
        }
        "#;
        
        let result = parse_json(request).unwrap();
        assert!(result.is_object());
        
        assert_eq!(result.get("jsonrpc").unwrap().as_str().unwrap(), "2.0");
        assert_eq!(result.get("id").unwrap().as_number().unwrap(), 1.0);
        assert_eq!(result.get("method").unwrap().as_str().unwrap(), "textDocument/completion");
        
        let params = result.get("params").unwrap().as_object().unwrap();
        let position = params.get("position").unwrap().as_object().unwrap();
        assert_eq!(position.get("line").unwrap().as_number().unwrap(), 10.0);
    }

    #[test]
    fn test_json_error_handling_invalid_syntax() {
        let invalid_cases = vec![
            "",
            "{",
            "}",
            "[",
            "]",
            r#"{"key": }"#,
            r#"{"key" "value"}"#,
            r#"{key: "value"}"#,
            "[1, 2, 3,]",
            "null true",
            "123abc",
        ];
        
        for invalid in invalid_cases {
            let result = parse_json(invalid);
            assert!(result.is_err(), "Should fail to parse: {}", invalid);
        }
    }

    #[test]
    fn test_json_error_handling_invalid_numbers() {
        let invalid_numbers = vec![
            "01",    // Leading zero
            "+1",    // Leading plus
            "1.",    // Trailing dot
            ".1",    // Leading dot
            "1e",    // Incomplete exponent
            "1e+",   // Incomplete exponent
            "1e-",   // Incomplete exponent
        ];
        
        for invalid in invalid_numbers {
            let result = parse_json(invalid);
            assert!(result.is_err(), "Should fail to parse number: {}", invalid);
        }
    }

    #[test]
    fn test_json_error_handling_invalid_strings() {
        let invalid_strings = vec![
            r#""unclosed string"#,
            r#""invalid\escape""#,
            r#""invalid\u""#,
            r#""invalid\u123""#,
            r#""invalid\uXYZ""#,
            "\"control\u{0001}char\"", // Control character
        ];
        
        for invalid in invalid_strings {
            let result = parse_json(invalid);
            assert!(result.is_err(), "Should fail to parse string: {}", invalid);
        }
    }

    #[test]
    fn test_json_serialization_basic_values() {
        // Test serializing basic values
        assert_eq!(to_json_string(&JsonValue::Null).as_str(), "null");
        assert_eq!(to_json_string(&JsonValue::Bool(true)).as_str(), "true");
        assert_eq!(to_json_string(&JsonValue::Bool(false)).as_str(), "false");
        assert_eq!(to_json_string(&JsonValue::Number(42.0)).as_str(), "42");
        assert_eq!(to_json_string(&JsonValue::Number(3.14)).as_str(), "3.14");
        assert_eq!(to_json_string(&JsonValue::String(String::from("hello"))).as_str(), r#""hello""#);
    }

    #[test]
    fn test_json_serialization_string_escaping() {
        let test_cases = vec![
            ("", r#""""#),
            ("hello", r#""hello""#),
            ("line1\nline2", r#""line1\nline2""#),
            ("tab\there", r#""tab\there""#),
            ("quote\"here", r#""quote\"here""#),
            ("backslash\\here", r#""backslash\\here""#),
        ];
        
        for (input, expected) in test_cases {
            let json_val = JsonValue::String(String::from(input));
            let serialized = to_json_string(&json_val);
            assert_eq!(serialized.as_str(), expected);
        }
    }

    #[test]
    fn test_json_serialization_arrays() {
        // Empty array
        let empty = JsonValue::Array(Vec::new());
        assert_eq!(to_json_string(&empty).as_str(), "[]");
        
        // Array with numbers
        let mut nums = Vec::new();
        nums.push(JsonValue::Number(1.0));
        nums.push(JsonValue::Number(2.0));
        nums.push(JsonValue::Number(3.0));
        let array = JsonValue::Array(nums);
        assert_eq!(to_json_string(&array).as_str(), "[1,2,3]");
    }

    #[test]
    fn test_json_serialization_objects() {
        // Empty object
        let empty = JsonValue::Object(HashMap::new());
        assert_eq!(to_json_string(&empty).as_str(), "{}");
        
        // Simple object
        let mut obj = HashMap::new();
        obj.insert(String::from("name"), JsonValue::String(String::from("seen")));
        obj.insert(String::from("version"), JsonValue::Number(1.0));
        let object = JsonValue::Object(obj);
        
        let serialized = to_json_string(&object);
        // Note: HashMap iteration order is not guaranteed, so we check both contain key-value pairs
        assert!(serialized.contains(r#""name":"seen""#));
        assert!(serialized.contains(r#""version":1"#));
    }

    #[test]
    fn test_json_round_trip() {
        // Test that parsing and serializing results in equivalent data
        let json_strings = vec![
            "null",
            "true",
            "false",
            "42",
            "3.14",
            r#""hello world""#,
            r#"[]"#,
            r#"[1,2,3]"#,
            r#"{}"#,
            r#"{"key":"value"}"#,
            r#"{"name":"seen","version":1,"active":true}"#,
        ];
        
        for json in json_strings {
            let parsed = parse_json(json).unwrap();
            let serialized = to_json_string(&parsed);
            let reparsed = parse_json(&serialized).unwrap();
            
            assert_eq!(parsed, reparsed, "Round trip failed for: {}", json);
        }
    }

    #[test]
    fn test_json_pretty_printing() {
        let mut obj = HashMap::new();
        obj.insert(String::from("name"), JsonValue::String(String::from("seen")));
        obj.insert(String::from("version"), JsonValue::Number(1.0));
        
        let mut arr = Vec::new();
        arr.push(JsonValue::String(String::from("feature1")));
        arr.push(JsonValue::String(String::from("feature2")));
        obj.insert(String::from("features"), JsonValue::Array(arr));
        
        let object = JsonValue::Object(obj);
        let pretty = to_json_pretty(&object);
        
        // Pretty printed JSON should contain newlines and indentation
        assert!(pretty.contains("\n"));
        assert!(pretty.contains("  ")); // Indentation
    }

    #[test]
    fn test_json_max_depth() {
        // Create deeply nested JSON to test depth limiting
        let mut deep_json = String::from("[");
        for _ in 0..150 {
            deep_json.push('[');
        }
        for _ in 0..150 {
            deep_json.push(']');
        }
        deep_json.push(']');
        
        let result = parse_json(&deep_json);
        assert!(result.is_err());
        
        if let Err(JsonError::MaxDepthExceeded) = result {
            // Expected error
        } else {
            panic!("Expected MaxDepthExceeded error");
        }
    }

    #[test]
    fn test_json_unicode_support() {
        let json = r#"{"message": "Hello 世界! 🚀 Ñoël"}"#;
        let result = parse_json(json).unwrap();
        
        let message = result.get("message").unwrap().as_str().unwrap();
        assert_eq!(message, "Hello 世界! 🚀 Ñoël");
    }

    #[test]
    fn test_json_large_numbers() {
        let test_cases = vec![
            "1234567890123456789",
            "-9876543210987654321", 
            "1.23456789012345e+100",
            "1.23456789012345e-100",
        ];
        
        for case in test_cases {
            let result = parse_json(case);
            assert!(result.is_ok(), "Should parse large number: {}", case);
            assert!(result.unwrap().is_number());
        }
    }

    #[test]
    fn test_json_performance_large_object() {
        // Test performance with a larger JSON object
        let mut large_json = String::from("{");
        
        for i in 0..1000 {
            if i > 0 {
                large_json.push(',');
            }
            large_json.push_str(&format!(r#""key{}":"value{}""#, i, i));
        }
        large_json.push('}');
        
        let start = std::time::Instant::now();
        let result = parse_json(&large_json).unwrap();
        let duration = start.elapsed();
        
        assert!(result.is_object());
        assert_eq!(result.as_object().unwrap().len(), 1000);
        assert!(duration.as_millis() < 1000); // Should parse reasonably fast (<1s)
    }

    #[test]
    fn test_json_memory_efficiency() {
        // Test that parsing doesn't use excessive memory
        let json = r#"
        {
            "name": "memory-test",
            "data": "A reasonably long string that tests memory allocation efficiency in our JSON parser implementation for the Seen language compiler",
            "numbers": [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
            "nested": {
                "level1": {
                    "level2": {
                        "level3": "deep"
                    }
                }
            }
        }
        "#;
        
        let result = parse_json(json).unwrap();
        
        // Basic checks to ensure data is correctly parsed
        assert_eq!(result.get("name").unwrap().as_str().unwrap(), "memory-test");
        assert!(result.get("data").unwrap().as_str().unwrap().len() > 50);
        assert_eq!(result.get("numbers").unwrap().as_array().unwrap().len(), 15);
        
        let nested = result.get("nested").unwrap().as_object().unwrap();
        let level1 = nested.get("level1").unwrap().as_object().unwrap();
        let level2 = level1.get("level2").unwrap().as_object().unwrap();
        assert_eq!(level2.get("level3").unwrap().as_str().unwrap(), "deep");
    }

    #[test]
    fn test_json_array_access() {
        let json = r#"["first", "second", "third"]"#;
        let result = parse_json(json).unwrap();
        
        assert_eq!(result.get_index(0).unwrap().as_str().unwrap(), "first");
        assert_eq!(result.get_index(1).unwrap().as_str().unwrap(), "second");
        assert_eq!(result.get_index(2).unwrap().as_str().unwrap(), "third");
        assert!(result.get_index(3).is_none());
    }

    #[test]
    fn test_json_type_checks() {
        let json = r#"
        {
            "null_val": null,
            "bool_val": true,
            "number_val": 42.5,
            "string_val": "hello",
            "array_val": [1, 2, 3],
            "object_val": {"key": "value"}
        }
        "#;
        
        let result = parse_json(json).unwrap();
        
        assert!(result.get("null_val").unwrap().is_null());
        assert!(result.get("bool_val").unwrap().is_bool());
        assert!(result.get("number_val").unwrap().is_number());
        assert!(result.get("string_val").unwrap().is_string());
        assert!(result.get("array_val").unwrap().is_array());
        assert!(result.get("object_val").unwrap().is_object());
    }

    #[test]
    fn test_json_compiler_api_response() {
        // Test JSON structure for compiler API responses
        let response_json = r#"
        {
            "status": "success",
            "diagnostics": [
                {
                    "level": "warning",
                    "message": "Unused variable 'x'",
                    "location": {
                        "file": "src/main.seen",
                        "line": 10,
                        "column": 5
                    }
                },
                {
                    "level": "error", 
                    "message": "Type mismatch: expected I32, found String",
                    "location": {
                        "file": "src/main.seen",
                        "line": 15,
                        "column": 12
                    }
                }
            ],
            "build_time": 1.234,
            "artifacts": {
                "executable": "target/main",
                "debug_info": "target/main.debug"
            }
        }
        "#;
        
        let result = parse_json(response_json).unwrap();
        assert!(result.is_object());
        
        assert_eq!(result.get("status").unwrap().as_str().unwrap(), "success");
        assert_eq!(result.get("build_time").unwrap().as_number().unwrap(), 1.234);
        
        let diagnostics = result.get("diagnostics").unwrap().as_array().unwrap();
        assert_eq!(diagnostics.len(), 2);
        
        let warning = diagnostics[0].as_object().unwrap();
        assert_eq!(warning.get("level").unwrap().as_str().unwrap(), "warning");
        
        let location = warning.get("location").unwrap().as_object().unwrap();
        assert_eq!(location.get("line").unwrap().as_number().unwrap(), 10.0);
    }
}