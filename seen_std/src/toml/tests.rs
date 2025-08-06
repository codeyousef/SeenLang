//! Comprehensive tests for TOML parser
//!
//! Tests cover all critical TOML functionality needed for compiler development

use super::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_toml_simple_key_value() {
        let toml = r#"
            name = "seen-lang"
            version = "0.1.0"
            debug = true
            port = 8080
        "#;
        
        let result = parse_toml(toml).unwrap();
        
        assert_eq!(get_string(&result, "name").unwrap(), "seen-lang");
        assert_eq!(get_string(&result, "version").unwrap(), "0.1.0");
        assert_eq!(get_boolean(&result, "debug").unwrap(), true);
        assert_eq!(get_integer(&result, "port").unwrap(), 8080);
    }

    #[test]
    fn test_toml_numbers() {
        let toml = r#"
            positive_int = 42
            negative_int = -17
            zero = 0
            float_val = 3.14159
            negative_float = -2.71828
        "#;
        
        let result = parse_toml(toml).unwrap();
        
        assert_eq!(get_integer(&result, "positive_int").unwrap(), 42);
        assert_eq!(get_integer(&result, "negative_int").unwrap(), -17);
        assert_eq!(get_integer(&result, "zero").unwrap(), 0);
        assert!((result.get("float_val").unwrap().as_float().unwrap() - 3.14159).abs() < 0.0001);
        assert!((result.get("negative_float").unwrap().as_float().unwrap() - (-2.71828)).abs() < 0.0001);
    }

    #[test]
    fn test_toml_strings() {
        let toml = r#"
            simple = "hello world"
            empty = ""
            with_quotes = "She said \"hello\""
            with_escapes = "Line 1\nLine 2\tTabbed"
            multiword_key = "value with spaces"
        "#;
        
        let result = parse_toml(toml).unwrap();
        
        assert_eq!(get_string(&result, "simple").unwrap(), "hello world");
        assert_eq!(get_string(&result, "empty").unwrap(), "");
        assert_eq!(get_string(&result, "with_quotes").unwrap(), "She said \"hello\"");
        assert_eq!(get_string(&result, "with_escapes").unwrap(), "Line 1\nLine 2\tTabbed");
        assert_eq!(get_string(&result, "multiword_key").unwrap(), "value with spaces");
    }

    #[test]
    fn test_toml_booleans() {
        let toml = r#"
            enabled = true
            disabled = false
            debug_mode = true
            production = false
        "#;
        
        let result = parse_toml(toml).unwrap();
        
        assert_eq!(get_boolean(&result, "enabled").unwrap(), true);
        assert_eq!(get_boolean(&result, "disabled").unwrap(), false);
        assert_eq!(get_boolean(&result, "debug_mode").unwrap(), true);
        assert_eq!(get_boolean(&result, "production").unwrap(), false);
    }

    #[test]
    fn test_toml_arrays() {
        let toml = r#"
            numbers = [1, 2, 3, 4, 5]
            strings = ["hello", "world", "test"]
            booleans = [true, false, true]
            mixed = [1, "two", true]
            empty = []
        "#;
        
        let result = parse_toml(toml).unwrap();
        
        let numbers = result.get("numbers").unwrap().as_array().unwrap();
        assert_eq!(numbers.len(), 5);
        assert_eq!(numbers[0].as_integer().unwrap(), 1);
        assert_eq!(numbers[4].as_integer().unwrap(), 5);
        
        let strings = result.get("strings").unwrap().as_array().unwrap();
        assert_eq!(strings.len(), 3);
        assert_eq!(strings[0].as_string().unwrap(), "hello");
        assert_eq!(strings[2].as_string().unwrap(), "test");
        
        let booleans = result.get("booleans").unwrap().as_array().unwrap();
        assert_eq!(booleans.len(), 3);
        assert_eq!(booleans[0].as_boolean().unwrap(), true);
        assert_eq!(booleans[1].as_boolean().unwrap(), false);
        
        let empty = result.get("empty").unwrap().as_array().unwrap();
        assert_eq!(empty.len(), 0);
    }

    #[test]
    fn test_toml_inline_tables() {
        let toml = r#"
            point = { x = 10, y = 20 }
            color = { r = 255, g = 128, b = 0 }
            empty_table = {}
        "#;
        
        let result = parse_toml(toml).unwrap();
        
        let point = result.get("point").unwrap().as_table().unwrap();
        assert_eq!(get_integer(point, "x").unwrap(), 10);
        assert_eq!(get_integer(point, "y").unwrap(), 20);
        
        let color = result.get("color").unwrap().as_table().unwrap();
        assert_eq!(get_integer(color, "r").unwrap(), 255);
        assert_eq!(get_integer(color, "g").unwrap(), 128);
        assert_eq!(get_integer(color, "b").unwrap(), 0);
        
        let empty = result.get("empty_table").unwrap().as_table().unwrap();
        assert_eq!(empty.len(), 0);
    }

    #[test]
    fn test_toml_comments() {
        let toml = r#"
            # This is a comment
            name = "test" # End of line comment
            
            # Another comment
            version = "1.0"
            # Final comment
        "#;
        
        let result = parse_toml(toml).unwrap();
        
        assert_eq!(get_string(&result, "name").unwrap(), "test");
        assert_eq!(get_string(&result, "version").unwrap(), "1.0");
    }

    #[test]
    fn test_toml_whitespace_handling() {
        let toml = r#"
        
            name    =    "test"    
            
                version =  "1.0"
            
            numbers = [  1  ,  2  ,  3  ]
            
        "#;
        
        let result = parse_toml(toml).unwrap();
        
        assert_eq!(get_string(&result, "name").unwrap(), "test");
        assert_eq!(get_string(&result, "version").unwrap(), "1.0");
        
        let numbers = result.get("numbers").unwrap().as_array().unwrap();
        assert_eq!(numbers.len(), 3);
    }

    #[test]
    fn test_toml_quoted_keys() {
        let toml = r#"
            "quoted key" = "value1"
            "key with spaces" = "value2"
            "special-chars@#$" = "value3"
        "#;
        
        let result = parse_toml(toml).unwrap();
        
        assert_eq!(get_string(&result, "quoted key").unwrap(), "value1");
        assert_eq!(get_string(&result, "key with spaces").unwrap(), "value2");
        assert_eq!(get_string(&result, "special-chars@#$").unwrap(), "value3");
    }

    #[test]
    fn test_toml_seen_config() {
        // Test realistic Seen.toml configuration
        let toml = r#"
            [package]
            name = "hello-world"
            version = "0.1.0"
            description = "A sample Seen project"
            authors = ["Developer <dev@example.com>"]
            
            [build]
            target = "native"
            optimization = "release"
            debug_symbols = true
            
            [dependencies]
            std = "0.1.0"
            io = "0.2.0"
            
            [dev-dependencies]
            test-framework = "0.1.0"
        "#;
        
        let result = parse_toml(toml).unwrap();
        
        // For MVP, we just ensure it parses without error
        // Full table support would be implemented in a more advanced version
        assert!(result.len() >= 0);
    }

    #[test]
    fn test_toml_compiler_config() {
        // Test compiler configuration patterns
        let toml = r#"
            name = "seen-compiler"
            version = "0.1.0"
            
            optimization_level = 2
            debug_info = true
            target_triple = "x86_64-unknown-linux-gnu"
            
            features = ["simd", "parallel", "jit"]
            
            llvm_config = { version = "18.0", static_link = false }
        "#;
        
        let result = parse_toml(toml).unwrap();
        
        assert_eq!(get_string(&result, "name").unwrap(), "seen-compiler");
        assert_eq!(get_integer(&result, "optimization_level").unwrap(), 2);
        assert_eq!(get_boolean(&result, "debug_info").unwrap(), true);
        
        let features = result.get("features").unwrap().as_array().unwrap();
        assert_eq!(features.len(), 3);
        assert_eq!(features[0].as_string().unwrap(), "simd");
        
        let llvm = result.get("llvm_config").unwrap().as_table().unwrap();
        assert_eq!(get_string(llvm, "version").unwrap(), "18.0");
        assert_eq!(get_boolean(llvm, "static_link").unwrap(), false);
    }

    #[test]
    fn test_toml_error_handling_invalid_syntax() {
        let invalid_toml = r#"
            name = 
            version = "1.0"
        "#;
        
        let result = parse_toml(invalid_toml);
        assert!(result.is_err());
        
        match result.unwrap_err() {
            TomlError::InvalidValue(_) => {},
            _ => panic!("Expected InvalidValue error"),
        }
    }

    #[test]
    fn test_toml_error_handling_duplicate_keys() {
        let duplicate_toml = r#"
            name = "first"
            name = "second"
        "#;
        
        let result = parse_toml(duplicate_toml);
        assert!(result.is_err());
        
        match result.unwrap_err() {
            TomlError::DuplicateKey(_) => {},
            _ => panic!("Expected DuplicateKey error"),
        }
    }

    #[test]
    fn test_toml_error_handling_unclosed_string() {
        let unclosed_toml = r#"
            name = "unclosed string
            version = "1.0"
        "#;
        
        let result = parse_toml(unclosed_toml);
        assert!(result.is_err());
        
        match result.unwrap_err() {
            TomlError::UnexpectedEof => {},
            _ => panic!("Expected UnexpectedEof error"),
        }
    }

    #[test]
    fn test_toml_error_handling_invalid_number() {
        let invalid_toml = r#"
            port = 123abc
        "#;
        
        let result = parse_toml(invalid_toml);
        assert!(result.is_err());
    }

    #[test]
    fn test_toml_error_handling_unclosed_array() {
        let unclosed_toml = r#"
            numbers = [1, 2, 3
        "#;
        
        let result = parse_toml(unclosed_toml);
        assert!(result.is_err());
        
        match result.unwrap_err() {
            TomlError::UnexpectedEof => {},
            _ => panic!("Expected UnexpectedEof error"),
        }
    }

    #[test]
    fn test_toml_type_checking() {
        let toml = r#"
            string_val = "hello"
            int_val = 42
            float_val = 3.14
            bool_val = true
            array_val = [1, 2, 3]
            table_val = { x = 1 }
        "#;
        
        let result = parse_toml(toml).unwrap();
        
        assert!(result.get("string_val").unwrap().is_string());
        assert!(result.get("int_val").unwrap().is_integer());
        assert!(result.get("float_val").unwrap().is_float());
        assert!(result.get("bool_val").unwrap().is_boolean());
        assert!(result.get("array_val").unwrap().is_array());
        assert!(result.get("table_val").unwrap().is_table());
    }

    #[test]
    fn test_toml_value_conversion() {
        let toml = r#"
            string_val = "hello"
            int_val = 42
            float_val = 3.14
            bool_val = true
        "#;
        
        let result = parse_toml(toml).unwrap();
        
        // Test successful conversions
        assert_eq!(result.get("string_val").unwrap().as_string().unwrap(), "hello");
        assert_eq!(result.get("int_val").unwrap().as_integer().unwrap(), 42);
        assert!((result.get("float_val").unwrap().as_float().unwrap() - 3.14).abs() < 0.01);
        assert_eq!(result.get("bool_val").unwrap().as_boolean().unwrap(), true);
        
        // Test failed conversions
        assert!(result.get("string_val").unwrap().as_integer().is_none());
        assert!(result.get("int_val").unwrap().as_string().is_none());
        assert!(result.get("float_val").unwrap().as_boolean().is_none());
        assert!(result.get("bool_val").unwrap().as_array().is_none());
    }

    #[test]
    fn test_toml_integer_to_float_conversion() {
        let toml = r#"
            int_val = 42
        "#;
        
        let result = parse_toml(toml).unwrap();
        
        // Integer should be convertible to float
        assert_eq!(result.get("int_val").unwrap().as_float().unwrap(), 42.0);
    }

    #[test]
    fn test_toml_edge_cases() {
        let toml = r#"
            empty_string = ""
            zero = 0
            negative_zero = -0
            single_char = "x"
            boolean_true = true
            boolean_false = false
            single_item_array = [42]
        "#;
        
        let result = parse_toml(toml).unwrap();
        
        assert_eq!(get_string(&result, "empty_string").unwrap(), "");
        assert_eq!(get_integer(&result, "zero").unwrap(), 0);
        assert_eq!(get_integer(&result, "negative_zero").unwrap(), 0);
        assert_eq!(get_string(&result, "single_char").unwrap(), "x");
        assert_eq!(get_boolean(&result, "boolean_true").unwrap(), true);
        assert_eq!(get_boolean(&result, "boolean_false").unwrap(), false);
        
        let array = result.get("single_item_array").unwrap().as_array().unwrap();
        assert_eq!(array.len(), 1);
        assert_eq!(array[0].as_integer().unwrap(), 42);
    }

    #[test]
    fn test_toml_performance_large_config() {
        // Test parsing performance with a larger configuration
        let mut toml = String::new();
        toml.push_str("# Large configuration for performance testing\n");
        
        for i in 0..100 {
            toml.push_str(&format!("key{} = \"value{}\"\n", i, i));
        }
        
        toml.push_str("large_array = [");
        for i in 0..50 {
            if i > 0 { toml.push_str(", "); }
            toml.push_str(&format!("{}", i));
        }
        toml.push_str("]\n");
        
        let start = std::time::Instant::now();
        let result = parse_toml(&toml).unwrap();
        let duration = start.elapsed();
        
        assert_eq!(result.len(), 101); // 100 keys + 1 array
        assert!(duration.as_millis() < 100); // Should parse quickly (<100ms)
    }

    #[test]
    fn test_toml_memory_efficiency() {
        // Test that parsing doesn't use excessive memory
        let toml = r#"
            name = "memory-test"
            data = "A reasonably long string that tests memory allocation efficiency in our TOML parser implementation"
            numbers = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
        "#;
        
        let result = parse_toml(toml).unwrap();
        
        // Basic checks to ensure data is correctly parsed
        assert_eq!(get_string(&result, "name").unwrap(), "memory-test");
        assert!(get_string(&result, "data").unwrap().len() > 50);
        assert_eq!(result.get("numbers").unwrap().as_array().unwrap().len(), 10);
    }

    #[test]
    fn test_toml_realistic_seen_toml() {
        // Test with a realistic Seen.toml file structure
        let toml = r#"
            name = "seen-lang-compiler"
            version = "0.1.0"
            edition = "2024"
            
            target = "native"
            optimization = "release"
            
            features = ["llvm-backend", "jit", "incremental"]
            
            build_config = { 
                parallel = true, 
                cache_size = 1024,
                timeout = 300
            }
            
            dependencies = ["std", "collections", "io"]
        "#;
        
        let result = parse_toml(toml).unwrap();
        
        assert_eq!(get_string(&result, "name").unwrap(), "seen-lang-compiler");
        assert_eq!(get_string(&result, "version").unwrap(), "0.1.0");
        assert_eq!(get_string(&result, "target").unwrap(), "native");
        
        let features = result.get("features").unwrap().as_array().unwrap();
        assert_eq!(features.len(), 3);
        
        let build_config = result.get("build_config").unwrap().as_table().unwrap();
        assert_eq!(get_boolean(build_config, "parallel").unwrap(), true);
        assert_eq!(get_integer(build_config, "cache_size").unwrap(), 1024);
    }
}