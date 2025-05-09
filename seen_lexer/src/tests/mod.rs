#[cfg(test)]
mod lexer_tests {
    use std::path::Path;
    use crate::{KeywordConfig, KeywordManager, Lexer, TokenType};

    // Helper function to create a test keyword manager
    fn create_test_keyword_manager(active_language: &str) -> KeywordManager {
        use std::collections::HashMap;
        use std::path::PathBuf;
        
        // Create a mock config directly instead of parsing TOML
        let mut languages = HashMap::new();
        let mut keyword_mappings = HashMap::new();
        
        // English language
        let english = Language {
            code: "en".to_string(),
            name: "English".to_string(),
            direction: "ltr".to_string(),
        };
        languages.insert(english.code.clone(), english);
        
        // Arabic language
        let arabic = Language {
            code: "ar".to_string(),
            name: "Arabic".to_string(),
            direction: "rtl".to_string(),
        };
        languages.insert(arabic.code.clone(), arabic);
        
        // Add keyword mappings
        let keyword_pairs = [
            ("val", ("val", "ثابت")),
            ("var", ("var", "متغير")),
            ("func", ("func", "دالة")),
            ("if", ("if", "إذا")),
            ("else", ("else", "وإلا")),
            ("while", ("while", "طالما")),
            ("for", ("for", "لكل")),
            ("return", ("return", "إرجاع")),
            ("true", ("true", "صحيح")),
            ("false", ("false", "خطأ")),
            ("null", ("null", "فارغ")),
            ("println", ("println", "اطبع")),
            ("when", ("when", "عندما")),
            ("in", ("in", "في")),
            ("loop", ("loop", "حلقة")),
            ("break", ("break", "اخرج")),
            ("continue", ("continue", "استمر")),
            ("struct", ("struct", "هيكل")),
            ("enum", ("enum", "تعداد")),
            ("unsafe", ("unsafe", "غير_آمن")),
            ("ref", ("ref", "مرجع")),
            ("own", ("own", "ملك")),
            ("async", ("async", "غير_متزامن")),
            ("await", ("await", "انتظر")),
        ];
        
        for (token_name, (en_keyword, ar_keyword)) in keyword_pairs {
            let mut lang_map = HashMap::new();
            lang_map.insert("en".to_string(), en_keyword.to_string());
            lang_map.insert("ar".to_string(), ar_keyword.to_string());
            keyword_mappings.insert(token_name.to_string(), lang_map);
        }
        
        let config = KeywordConfig {
            languages,
            keyword_mappings,
            base_dir: PathBuf::from("/usr/src/app/specifications"),
        };
        
        KeywordManager::new(config, active_language.to_string()).unwrap()
    }

    #[test]
    fn test_lexer_english_keywords() {
        let keyword_manager = create_test_keyword_manager("en");
        
        // Test English source code
        let source = r#"
        func main() {
            val x = 10;
            var y = 20;
            
            if (x < y) {
                println("x is less than y");
            } else {
                println("x is not less than y");
            }
            
            return 0;
        }
        "#;
        
        let mut lexer = Lexer::new(source, &keyword_manager);
        let tokens = lexer.tokenize().unwrap();
        
        // Verify some key tokens
        let token_types: Vec<TokenType> = tokens.iter().map(|t| t.token_type.clone()).collect();
        
        assert!(token_types.contains(&TokenType::Func));
        assert!(token_types.contains(&TokenType::Val));
        assert!(token_types.contains(&TokenType::Var));
        assert!(token_types.contains(&TokenType::If));
        assert!(token_types.contains(&TokenType::Else));
        assert!(token_types.contains(&TokenType::Return));
        assert!(token_types.contains(&TokenType::Println));
    }
    
    #[test]
    fn test_lexer_arabic_keywords() {
        let keyword_manager = create_test_keyword_manager("ar");
        
        // Test Arabic source code
        let source = r#"
        دالة رئيسية() {
            ثابت س = 10;
            متغير ص = 20;
            
            إذا (س < ص) {
                اطبع("س أقل من ص");
            } وإلا {
                اطبع("س ليست أقل من ص");
            }
            
            إرجاع 0;
        }
        "#;
        
        let mut lexer = Lexer::new(source, &keyword_manager);
        let tokens = lexer.tokenize().unwrap();
        
        // Verify some key tokens
        let token_types: Vec<TokenType> = tokens.iter().map(|t| t.token_type.clone()).collect();
        
        assert!(token_types.contains(&TokenType::Func));
        assert!(token_types.contains(&TokenType::Val));
        assert!(token_types.contains(&TokenType::Var));
        assert!(token_types.contains(&TokenType::If));
        assert!(token_types.contains(&TokenType::Else));
        assert!(token_types.contains(&TokenType::Return));
        assert!(token_types.contains(&TokenType::Println));
    }

    #[test]
    fn test_lexer_numbers_and_operators() {
        let keyword_manager = create_test_keyword_manager("en");
        
        let source = "10 + 20.5 * (30 - 5) / 2 <= 100";
        
        let mut lexer = Lexer::new(source, &keyword_manager);
        let tokens = lexer.tokenize().unwrap();
        
        // Verify token types
        let token_types: Vec<TokenType> = tokens.iter().map(|t| t.token_type.clone()).collect();
        
        assert!(token_types.contains(&TokenType::IntLiteral));
        assert!(token_types.contains(&TokenType::FloatLiteral));
        assert!(token_types.contains(&TokenType::Plus));
        assert!(token_types.contains(&TokenType::Multiply));
        assert!(token_types.contains(&TokenType::Minus));
        assert!(token_types.contains(&TokenType::Divide));
        assert!(token_types.contains(&TokenType::LessEqual));
        assert!(token_types.contains(&TokenType::LeftParen));
        assert!(token_types.contains(&TokenType::RightParen));
    }
    
    #[test]
    fn test_lexer_strings_and_identifiers() {
        let keyword_manager = create_test_keyword_manager("en");
        
        let source = r#"
        func greet(name: String) {
            println("Hello, " + name + "!");
        }
        "#;
        
        let mut lexer = Lexer::new(source, &keyword_manager);
        let tokens = lexer.tokenize().unwrap();
        
        // Find identifiers
        let identifiers: Vec<String> = tokens.iter()
            .filter(|t| t.token_type == TokenType::Identifier)
            .map(|t| t.lexeme.clone())
            .collect();
        
        assert!(identifiers.contains(&"greet".to_string()));
        assert!(identifiers.contains(&"name".to_string()));
        assert!(identifiers.contains(&"String".to_string()));
        
        // Find string literals
        let strings: Vec<String> = tokens.iter()
            .filter(|t| t.token_type == TokenType::StringLiteral)
            .map(|t| t.lexeme.clone())
            .collect();
        
        assert!(strings.contains(&"Hello, ".to_string()));
        assert!(strings.contains(&"!".to_string()));
    }
}
