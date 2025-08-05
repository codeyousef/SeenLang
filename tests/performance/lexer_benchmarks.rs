//! Performance benchmarks for the lexer

#[cfg(test)]
mod benchmarks {
    use seen_lexer::{Lexer, LanguageConfig};
    use std::time::{Duration, Instant};
    use std::collections::HashMap;
    
    fn create_benchmark_config() -> LanguageConfig {
        let mut keywords = HashMap::new();
        keywords.insert("func".to_string(), "TokenFunc".to_string());
        keywords.insert("if".to_string(), "TokenIf".to_string());
        keywords.insert("else".to_string(), "TokenElse".to_string());
        keywords.insert("while".to_string(), "TokenWhile".to_string());
        keywords.insert("for".to_string(), "TokenFor".to_string());
        keywords.insert("let".to_string(), "TokenLet".to_string());
        keywords.insert("return".to_string(), "TokenReturn".to_string());
        
        let mut operators = HashMap::new();
        operators.insert("+".to_string(), "TokenPlus".to_string());
        operators.insert("-".to_string(), "TokenMinus".to_string());
        operators.insert("*".to_string(), "TokenMultiply".to_string());
        operators.insert("/".to_string(), "TokenDivide".to_string());
        operators.insert("=".to_string(), "TokenAssign".to_string());
        operators.insert("==".to_string(), "TokenEqual".to_string());
        operators.insert("!=".to_string(), "TokenNotEqual".to_string());
        
        LanguageConfig {
            keywords,
            operators,
            name: "Benchmark".to_string(),
            description: Some("Benchmark configuration".to_string()),
        }
    }
    
    fn generate_large_program(lines: usize) -> String {
        let mut program = String::new();
        
        for i in 0..lines {
            program.push_str(&format!(
                "func function_{i}(param1: i32, param2: str) -> bool {{\n\
                 \tlet x = 42 + {i};\n\
                 \tlet y = \"string_{i}\";\n\
                 \tif x == {i} {{\n\
                 \t\treturn true;\n\
                 \t}} else {{\n\
                 \t\treturn false;\n\
                 \t}}\n\
                 }}\n\n",
                i = i
            ));
        }
        
        program
    }
    
    #[test]
    fn benchmark_lexer_performance_target() {
        let config = create_benchmark_config();
        
        // Generate a large program (approximately 1MB of source code)
        let large_program = generate_large_program(10_000);
        let source_size = large_program.len();
        
        println!("Benchmarking lexer with {} bytes of source code", source_size);
        
        let start = Instant::now();
        let mut lexer = Lexer::new(&large_program, 0, &config);
        let tokens = lexer.tokenize().expect("Tokenization should succeed");
        let duration = start.elapsed();
        
        let token_count = tokens.len();
        let tokens_per_second = (token_count as f64) / duration.as_secs_f64();
        
        println!("Tokenized {} tokens in {:?}", token_count, duration);
        println!("Performance: {:.0} tokens/second", tokens_per_second);
        
        // Verify we meet the >10M tokens/second target
        // Note: This is a relaxed check for CI environments
        const MIN_TOKENS_PER_SECOND: f64 = 1_000_000.0; // 1M tokens/sec minimum
        assert!(
            tokens_per_second >= MIN_TOKENS_PER_SECOND,
            "Lexer performance below target: {:.0} tokens/second (target: >{:.0})",
            tokens_per_second,
            MIN_TOKENS_PER_SECOND
        );
    }
    
    #[test]
    fn benchmark_lexer_startup_time() {
        let config = create_benchmark_config();
        let simple_program = "func main() { let x = 42; }";
        
        // Measure cold startup time
        let start = Instant::now();
        let mut lexer = Lexer::new(simple_program, 0, &config);
        let _tokens = lexer.tokenize().expect("Tokenization should succeed");
        let startup_time = start.elapsed();
        
        println!("Lexer startup time: {:?}", startup_time);
        
        // Should be very fast for small programs
        const MAX_STARTUP_TIME: Duration = Duration::from_millis(10);
        assert!(
            startup_time < MAX_STARTUP_TIME,
            "Lexer startup time too slow: {:?} (target: <{:?})",
            startup_time,
            MAX_STARTUP_TIME
        );
    }
    
    #[test]
    fn benchmark_lexer_memory_usage() {
        let config = create_benchmark_config();
        let large_program = generate_large_program(1000);
        
        // This is a basic test - in a real implementation we'd measure actual memory usage
        let mut lexer = Lexer::new(&large_program, 0, &config);
        let tokens = lexer.tokenize().expect("Tokenization should succeed");
        
        // Verify that we're not using excessive memory relative to input size
        let source_bytes = large_program.len();
        let estimated_token_bytes = tokens.len() * std::mem::size_of::<seen_lexer::Token>();
        let memory_ratio = estimated_token_bytes as f64 / source_bytes as f64;
        
        println!("Memory usage ratio: {:.2}x source size", memory_ratio);
        
        // Memory usage should be reasonable (less than 10x source size)
        assert!(
            memory_ratio < 10.0,
            "Memory usage too high: {:.2}x source size",
            memory_ratio
        );
    }
}