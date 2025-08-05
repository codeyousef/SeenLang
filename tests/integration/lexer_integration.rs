//! Integration tests for the lexer

#[cfg(test)]
mod tests {
    use seen_lexer::{Lexer, LanguageConfig, TokenType};
    use std::collections::HashMap;
    
    fn create_test_config() -> LanguageConfig {
        let mut keywords = HashMap::new();
        keywords.insert("func".to_string(), "TokenFunc".to_string());
        keywords.insert("if".to_string(), "TokenIf".to_string());
        keywords.insert("let".to_string(), "TokenLet".to_string());
        
        let mut operators = HashMap::new();
        operators.insert("+".to_string(), "TokenPlus".to_string());
        operators.insert("=".to_string(), "TokenAssign".to_string());
        
        LanguageConfig {
            keywords,
            operators,
            name: "Test".to_string(),
            description: Some("Test configuration".to_string()),
        }
    }
    
    #[test]
    fn test_simple_program_tokenization() {
        let config = create_test_config();
        let source = r#"
            func main() {
                let x = 42;
            }
        "#;
        
        let mut lexer = Lexer::new(source, 0, &config);
        let tokens = lexer.tokenize().expect("Tokenization should succeed");
        
        // Should have: func, main, (, ), {, let, x, =, 42, ;, }, EOF
        assert!(tokens.len() >= 10);
        
        // Check for expected tokens
        let token_types: Vec<&TokenType> = tokens.iter().map(|t| &t.value).collect();
        
        assert!(matches!(token_types[0], TokenType::KeywordFunc));
        assert!(matches!(token_types[1], TokenType::Identifier(_)));
        assert!(matches!(token_types[2], TokenType::LeftParen));
        assert!(matches!(token_types[3], TokenType::RightParen));
        assert!(matches!(token_types[4], TokenType::LeftBrace));
        assert!(matches!(token_types[5], TokenType::KeywordLet));
    }
    
    #[test]
    fn test_error_recovery() {
        let config = create_test_config();
        let source = r#"
            func main() {
                let x = "unclosed string
                let y = 42;
            }
        "#;
        
        let mut lexer = Lexer::new(source, 0, &config);
        let result = lexer.tokenize();
        
        // Should have errors but still produce some tokens
        assert!(lexer.diagnostics().has_errors());
        assert!(result.is_err() || result.unwrap().len() > 0);
    }
    
    #[test]
    fn test_multilingual_keywords() {
        // Test that the lexer can handle different language configurations
        let mut keywords = HashMap::new();
        keywords.insert("دالة".to_string(), "TokenFunc".to_string()); // Arabic "function"
        
        let operators = HashMap::new();
        
        let arabic_config = LanguageConfig {
            keywords,
            operators,
            name: "Arabic".to_string(),
            description: Some("Arabic test configuration".to_string()),
        };
        
        let source = "دالة";
        let mut lexer = Lexer::new(source, 0, &arabic_config);
        let tokens = lexer.tokenize().expect("Tokenization should succeed");
        
        assert_eq!(tokens.len(), 2); // دالة + EOF
        assert!(matches!(tokens[0].value, TokenType::KeywordFunc));
    }
}