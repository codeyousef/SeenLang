//! Core lexer tests following TDD methodology
//! 
//! These tests define the expected behavior for basic tokenization of all token types,
//! Unicode character handling, position tracking, and error reporting.

use crate::{
    Lexer, Token, TokenType, KeywordManager, Position, LexerError, LexerResult,
    InterpolationPart, InterpolationKind
};
use std::sync::Arc;

/// Test helper to create a lexer with English keywords loaded
fn create_test_lexer(input: &str) -> Lexer {
    let mut keyword_manager = KeywordManager::new();
    keyword_manager.load_from_toml("en").unwrap();
    keyword_manager.switch_language("en").unwrap();
    
    Lexer::new(input.to_string(), Arc::new(keyword_manager))
}

/// Test helper to create a lexer with specific language keywords
fn create_test_lexer_with_language(input: &str, language: &str) -> Lexer {
    let mut keyword_manager = KeywordManager::new();
    keyword_manager.load_from_toml(language).unwrap();
    keyword_manager.switch_language(language).unwrap();
    
    Lexer::new(input.to_string(), Arc::new(keyword_manager))
}

#[cfg(test)]
mod basic_tokenization_tests {
    use super::*;

    #[test]
    fn test_tokenize_integer_literals() {
        let mut lexer = create_test_lexer("42 0 123456789");
        
        let token1 = lexer.next_token().unwrap();
        assert_eq!(token1.token_type, TokenType::IntegerLiteral(42));
        assert_eq!(token1.lexeme, "42");
        assert_eq!(token1.position.line, 1);
        assert_eq!(token1.position.column, 1);
        
        let token2 = lexer.next_token().unwrap();
        assert_eq!(token2.token_type, TokenType::IntegerLiteral(0));
        assert_eq!(token2.lexeme, "0");
        
        let token3 = lexer.next_token().unwrap();
        assert_eq!(token3.token_type, TokenType::IntegerLiteral(123456789));
        assert_eq!(token3.lexeme, "123456789");
    }

    #[test]
    fn test_tokenize_unsigned_integer_literals() {
        let mut lexer = create_test_lexer("42u 0u 123u");
        
        let token1 = lexer.next_token().unwrap();
        assert_eq!(token1.token_type, TokenType::UIntegerLiteral(42));
        assert_eq!(token1.lexeme, "42u");
        
        let token2 = lexer.next_token().unwrap();
        assert_eq!(token2.token_type, TokenType::UIntegerLiteral(0));
        assert_eq!(token2.lexeme, "0u");
        
        let token3 = lexer.next_token().unwrap();
        assert_eq!(token3.token_type, TokenType::UIntegerLiteral(123));
        assert_eq!(token3.lexeme, "123u");
    }

    #[test]
    fn test_tokenize_float_literals() {
        let mut lexer = create_test_lexer("3.14 0.0 123.456");
        
        let token1 = lexer.next_token().unwrap();
        assert_eq!(token1.token_type, TokenType::FloatLiteral(3.14));
        assert_eq!(token1.lexeme, "3.14");
        
        let token2 = lexer.next_token().unwrap();
        assert_eq!(token2.token_type, TokenType::FloatLiteral(0.0));
        assert_eq!(token2.lexeme, "0.0");
        
        let token3 = lexer.next_token().unwrap();
        assert_eq!(token3.token_type, TokenType::FloatLiteral(123.456));
        assert_eq!(token3.lexeme, "123.456");
    }

    #[test]
    fn test_tokenize_string_literals() {
        let mut lexer = create_test_lexer(r#""hello" "world" "with spaces" """#);
        
        let token1 = lexer.next_token().unwrap();
        assert_eq!(token1.token_type, TokenType::StringLiteral("hello".to_string()));
        assert_eq!(token1.lexeme, r#""hello""#);
        
        let token2 = lexer.next_token().unwrap();
        assert_eq!(token2.token_type, TokenType::StringLiteral("world".to_string()));
        assert_eq!(token2.lexeme, r#""world""#);
        
        let token3 = lexer.next_token().unwrap();
        assert_eq!(token3.token_type, TokenType::StringLiteral("with spaces".to_string()));
        assert_eq!(token3.lexeme, r#""with spaces""#);
        
        let token4 = lexer.next_token().unwrap();
        assert_eq!(token4.token_type, TokenType::StringLiteral("".to_string()));
        assert_eq!(token4.lexeme, r#""""#);
    }

    #[test]
    fn test_tokenize_char_literals() {
        let mut lexer = create_test_lexer("'A' 'z' '1' ' '");
        
        let token1 = lexer.next_token().unwrap();
        assert_eq!(token1.token_type, TokenType::CharLiteral('A'));
        assert_eq!(token1.lexeme, "'A'");
        
        let token2 = lexer.next_token().unwrap();
        assert_eq!(token2.token_type, TokenType::CharLiteral('z'));
        assert_eq!(token2.lexeme, "'z'");
        
        let token3 = lexer.next_token().unwrap();
        assert_eq!(token3.token_type, TokenType::CharLiteral('1'));
        assert_eq!(token3.lexeme, "'1'");
        
        let token4 = lexer.next_token().unwrap();
        assert_eq!(token4.token_type, TokenType::CharLiteral(' '));
        assert_eq!(token4.lexeme, "' '");
    }

    #[test]
    fn test_tokenize_boolean_literals() {
        let mut lexer = create_test_lexer("true false");
        
        let token1 = lexer.next_token().unwrap();
        assert_eq!(token1.token_type, TokenType::True);
        assert_eq!(token1.lexeme, "true");
        
        let token2 = lexer.next_token().unwrap();
        assert_eq!(token2.token_type, TokenType::False);
        assert_eq!(token2.lexeme, "false");
    }

    #[test]
    fn test_tokenize_identifiers_with_visibility() {
        let mut lexer = create_test_lexer("PublicName privateName _private CONSTANT");
        
        // Public identifier (starts with capital)
        let token1 = lexer.next_token().unwrap();
        assert_eq!(token1.token_type, TokenType::PublicIdentifier("PublicName".to_string()));
        assert_eq!(token1.lexeme, "PublicName");
        
        // Private identifier (starts with lowercase)
        let token2 = lexer.next_token().unwrap();
        assert_eq!(token2.token_type, TokenType::PrivateIdentifier("privateName".to_string()));
        assert_eq!(token2.lexeme, "privateName");
        
        // Private identifier (starts with underscore)
        let token3 = lexer.next_token().unwrap();
        assert_eq!(token3.token_type, TokenType::PrivateIdentifier("_private".to_string()));
        assert_eq!(token3.lexeme, "_private");
        
        // Public identifier (all caps)
        let token4 = lexer.next_token().unwrap();
        assert_eq!(token4.token_type, TokenType::PublicIdentifier("CONSTANT".to_string()));
        assert_eq!(token4.lexeme, "CONSTANT");
    }

    #[test]
    fn test_tokenize_mathematical_operators() {
        let mut lexer = create_test_lexer("+ - * / % == != < > <= >=");
        
        let expected_tokens = vec![
            TokenType::Plus,
            TokenType::Minus,
            TokenType::Multiply,
            TokenType::Divide,
            TokenType::Modulo,
            TokenType::Equal,
            TokenType::NotEqual,
            TokenType::Less,
            TokenType::Greater,
            TokenType::LessEqual,
            TokenType::GreaterEqual,
        ];
        
        for expected_token in expected_tokens {
            let token = lexer.next_token().unwrap();
            assert_eq!(token.token_type, expected_token);
        }
    }

    #[test]
    fn test_tokenize_punctuation() {
        let mut lexer = create_test_lexer("( ) { } [ ] , ; : ->");
        
        let expected_tokens = vec![
            TokenType::LeftParen,
            TokenType::RightParen,
            TokenType::LeftBrace,
            TokenType::RightBrace,
            TokenType::LeftBracket,
            TokenType::RightBracket,
            TokenType::Comma,
            TokenType::Semicolon,
            TokenType::Colon,
            TokenType::Arrow,
        ];
        
        for expected_token in expected_tokens {
            let token = lexer.next_token().unwrap();
            assert_eq!(token.token_type, expected_token);
        }
    }

    #[test]
    fn test_tokenize_keywords_with_dynamic_loading() {
        let mut lexer = create_test_lexer("fun if else and or not move borrow inout is");
        
        let expected_tokens = vec![
            TokenType::Fun,
            TokenType::If,
            TokenType::Else,
            TokenType::LogicalAnd,
            TokenType::LogicalOr,
            TokenType::LogicalNot,
            TokenType::Move,
            TokenType::Borrow,
            TokenType::Inout,
            TokenType::Is,
        ];
        
        for expected_token in expected_tokens {
            let token = lexer.next_token().unwrap();
            assert_eq!(token.token_type, expected_token);
        }
    }

    #[test]
    fn test_tokenize_eof() {
        let mut lexer = create_test_lexer("");
        
        let token = lexer.next_token().unwrap();
        assert_eq!(token.token_type, TokenType::EOF);
        assert_eq!(token.lexeme, "");
    }

    #[test]
    fn test_tokenize_newlines() {
        let mut lexer = create_test_lexer("line1\nline2\n\nline4");
        
        let token1 = lexer.next_token().unwrap();
        assert_eq!(token1.token_type, TokenType::PrivateIdentifier("line1".to_string()));
        
        let token2 = lexer.next_token().unwrap();
        assert_eq!(token2.token_type, TokenType::Newline);
        assert_eq!(token2.position.line, 1);
        
        let token3 = lexer.next_token().unwrap();
        assert_eq!(token3.token_type, TokenType::PrivateIdentifier("line2".to_string()));
        assert_eq!(token3.position.line, 2);
        
        let token4 = lexer.next_token().unwrap();
        assert_eq!(token4.token_type, TokenType::Newline);
        
        let token5 = lexer.next_token().unwrap();
        assert_eq!(token5.token_type, TokenType::Newline);
        
        let token6 = lexer.next_token().unwrap();
        assert_eq!(token6.token_type, TokenType::PrivateIdentifier("line4".to_string()));
        assert_eq!(token6.position.line, 4);
    }
}

#[cfg(test)]
mod unicode_handling_tests {
    use super::*;

    #[test]
    fn test_unicode_identifiers() {
        let mut lexer = create_test_lexer("变量名 Переменная مُتَغَيِّر");
        
        // Chinese identifier (starts with non-ASCII, should be private)
        let token1 = lexer.next_token().unwrap();
        assert_eq!(token1.token_type, TokenType::PrivateIdentifier("变量名".to_string()));
        assert_eq!(token1.lexeme, "变量名");
        
        // Russian identifier (starts with capital, should be public)
        let token2 = lexer.next_token().unwrap();
        assert_eq!(token2.token_type, TokenType::PublicIdentifier("Переменная".to_string()));
        assert_eq!(token2.lexeme, "Переменная");
        
        // Arabic identifier (starts with non-ASCII, should be private)
        let token3 = lexer.next_token().unwrap();
        assert_eq!(token3.token_type, TokenType::PrivateIdentifier("مُتَغَيِّر".to_string()));
        assert_eq!(token3.lexeme, "مُتَغَيِّر");
    }

    #[test]
    fn test_unicode_string_literals() {
        let mut lexer = create_test_lexer(r#""Hello, 世界!" "مرحبا بالعالم" "🚀 Rocket""#);
        
        let token1 = lexer.next_token().unwrap();
        assert_eq!(token1.token_type, TokenType::StringLiteral("Hello, 世界!".to_string()));
        
        let token2 = lexer.next_token().unwrap();
        assert_eq!(token2.token_type, TokenType::StringLiteral("مرحبا بالعالم".to_string()));
        
        let token3 = lexer.next_token().unwrap();
        assert_eq!(token3.token_type, TokenType::StringLiteral("🚀 Rocket".to_string()));
    }

    #[test]
    fn test_unicode_char_literals() {
        let mut lexer = create_test_lexer("'世' 'م' '🚀'");
        
        let token1 = lexer.next_token().unwrap();
        assert_eq!(token1.token_type, TokenType::CharLiteral('世'));
        
        let token2 = lexer.next_token().unwrap();
        assert_eq!(token2.token_type, TokenType::CharLiteral('م'));
        
        let token3 = lexer.next_token().unwrap();
        assert_eq!(token3.token_type, TokenType::CharLiteral('🚀'));
    }

    #[test]
    fn test_unicode_escape_sequences() {
        let mut lexer = create_test_lexer(r#""\u{1F680}" "\u{4E16}" "\u{0645}""#);
        
        let token1 = lexer.next_token().unwrap();
        assert_eq!(token1.token_type, TokenType::StringLiteral("🚀".to_string()));
        
        let token2 = lexer.next_token().unwrap();
        assert_eq!(token2.token_type, TokenType::StringLiteral("世".to_string()));
        
        let token3 = lexer.next_token().unwrap();
        assert_eq!(token3.token_type, TokenType::StringLiteral("م".to_string()));
    }

    #[test]
    fn test_handle_unicode_method() {
        let mut lexer = create_test_lexer("🚀");
        
        let unicode_char = lexer.handle_unicode().unwrap();
        assert_eq!(unicode_char, '🚀');
    }

    #[test]
    fn test_unicode_position_tracking() {
        let mut lexer = create_test_lexer("🚀\n世界");
        
        let token1 = lexer.next_token().unwrap();
        assert_eq!(token1.token_type, TokenType::PrivateIdentifier("🚀".to_string()));
        assert_eq!(token1.position.line, 1);
        assert_eq!(token1.position.column, 1);
        
        let token2 = lexer.next_token().unwrap();
        assert_eq!(token2.token_type, TokenType::Newline);
        assert_eq!(token2.position.line, 1);
        
        let token3 = lexer.next_token().unwrap();
        assert_eq!(token3.token_type, TokenType::PrivateIdentifier("世界".to_string()));
        assert_eq!(token3.position.line, 2);
        assert_eq!(token3.position.column, 1);
    }
}

#[cfg(test)]
mod position_tracking_tests {
    use super::*;

    #[test]
    fn test_position_tracking_single_line() {
        let mut lexer = create_test_lexer("fun main() { }");
        
        let token1 = lexer.next_token().unwrap();
        assert_eq!(token1.position, Position::new(1, 1, 0));
        
        let token2 = lexer.next_token().unwrap();
        assert_eq!(token2.position, Position::new(1, 5, 4));
        
        let token3 = lexer.next_token().unwrap();
        assert_eq!(token3.position, Position::new(1, 9, 8));
    }

    #[test]
    fn test_position_tracking_multiple_lines() {
        let mut lexer = create_test_lexer("fun\nmain\n()");
        
        let token1 = lexer.next_token().unwrap();
        assert_eq!(token1.position.line, 1);
        assert_eq!(token1.position.column, 1);
        
        let token2 = lexer.next_token().unwrap(); // newline
        assert_eq!(token2.position.line, 1);
        
        let token3 = lexer.next_token().unwrap(); // main
        assert_eq!(token3.position.line, 2);
        assert_eq!(token3.position.column, 1);
        
        let token4 = lexer.next_token().unwrap(); // newline
        assert_eq!(token4.position.line, 2);
        
        let token5 = lexer.next_token().unwrap(); // (
        assert_eq!(token5.position.line, 3);
        assert_eq!(token5.position.column, 1);
    }

    #[test]
    fn test_position_tracking_with_unicode() {
        let mut lexer = create_test_lexer("🚀 世界");
        
        let token1 = lexer.next_token().unwrap();
        assert_eq!(token1.position.line, 1);
        assert_eq!(token1.position.column, 1);
        
        let token2 = lexer.next_token().unwrap();
        assert_eq!(token2.position.line, 1);
        assert_eq!(token2.position.column, 3); // After emoji and space
    }
}

#[cfg(test)]
mod error_reporting_tests {
    use super::*;

    #[test]
    fn test_unexpected_character_error() {
        let mut lexer = create_test_lexer("@");
        
        let result = lexer.next_token();
        assert!(result.is_err());
        
        if let Err(LexerError::UnexpectedCharacter { character, position }) = result {
            assert_eq!(character, '@');
            assert_eq!(position.line, 1);
            assert_eq!(position.column, 1);
        } else {
            panic!("Expected UnexpectedCharacter error");
        }
    }

    #[test]
    fn test_unterminated_string_error() {
        let mut lexer = create_test_lexer(r#""unterminated string"#);
        
        let result = lexer.next_token();
        assert!(result.is_err());
        
        if let Err(LexerError::UnterminatedString { position }) = result {
            assert_eq!(position.line, 1);
            assert_eq!(position.column, 1);
        } else {
            panic!("Expected UnterminatedString error");
        }
    }

    #[test]
    fn test_invalid_number_format_error() {
        let mut lexer = create_test_lexer("123.456.789");
        
        let result = lexer.next_token();
        assert!(result.is_err());
        
        if let Err(LexerError::InvalidNumber { position, message }) = result {
            assert_eq!(position.line, 1);
            assert_eq!(position.column, 1);
            assert!(message.contains("multiple decimal points"));
        } else {
            panic!("Expected InvalidNumber error");
        }
    }

    #[test]
    fn test_invalid_unicode_escape_error() {
        let mut lexer = create_test_lexer(r#""\u{GGGG}""#);
        
        let result = lexer.next_token();
        assert!(result.is_err());
        
        if let Err(LexerError::InvalidUnicodeEscape { position }) = result {
            assert_eq!(position.line, 1);
        } else {
            panic!("Expected InvalidUnicodeEscape error");
        }
    }

    #[test]
    fn test_error_position_accuracy() {
        let mut lexer = create_test_lexer("fun main() {\n    @\n}");
        
        // Skip tokens until we hit the error
        lexer.next_token().unwrap(); // fun
        lexer.next_token().unwrap(); // main
        lexer.next_token().unwrap(); // (
        lexer.next_token().unwrap(); // )
        lexer.next_token().unwrap(); // {
        lexer.next_token().unwrap(); // newline
        
        let result = lexer.next_token();
        assert!(result.is_err());
        
        if let Err(LexerError::UnexpectedCharacter { character, position }) = result {
            assert_eq!(character, '@');
            assert_eq!(position.line, 2);
            assert_eq!(position.column, 5); // After 4 spaces
        } else {
            panic!("Expected UnexpectedCharacter error at correct position");
        }
    }
}

#[cfg(test)]
mod dynamic_keyword_integration_tests {
    use super::*;

    #[test]
    fn test_english_keywords() {
        let mut lexer = create_test_lexer_with_language("fun if else and or not", "en");
        
        let expected_tokens = vec![
            TokenType::Fun,
            TokenType::If,
            TokenType::Else,
            TokenType::LogicalAnd,
            TokenType::LogicalOr,
            TokenType::LogicalNot,
        ];
        
        for expected_token in expected_tokens {
            let token = lexer.next_token().unwrap();
            assert_eq!(token.token_type, expected_token);
        }
    }

    #[test]
    fn test_arabic_keywords() {
        // This test assumes Arabic TOML file exists with proper translations
        let mut lexer = create_test_lexer_with_language("دالة إذا وإلا و أو ليس", "ar");
        
        let expected_tokens = vec![
            TokenType::Fun,
            TokenType::If,
            TokenType::Else,
            TokenType::LogicalAnd,
            TokenType::LogicalOr,
            TokenType::LogicalNot,
        ];
        
        for expected_token in expected_tokens {
            let token = lexer.next_token().unwrap();
            assert_eq!(token.token_type, expected_token);
        }
    }

    #[test]
    fn test_keyword_manager_integration() {
        let mut keyword_manager = KeywordManager::new();
        keyword_manager.load_from_toml("en").unwrap();
        keyword_manager.switch_language("en").unwrap();
        
        let lexer = Lexer::new("fun".to_string(), Arc::new(keyword_manager.clone()));
        
        // Test that lexer uses keyword manager correctly
        let fun_keyword = keyword_manager.get_keyword_text(&crate::keyword_manager::KeywordType::KeywordFun).unwrap();
        assert_eq!(lexer.check_keyword(&fun_keyword), Some(TokenType::Fun));
        
        // Test non-keyword
        assert_eq!(lexer.check_keyword("not_a_keyword"), None);
    }

    #[test]
    fn test_no_hardcoded_keywords() {
        // This test ensures that the lexer doesn't contain any hardcoded keywords
        // by testing that it fails when no keyword manager is loaded
        let keyword_manager = KeywordManager::new(); // No languages loaded
        let lexer = Lexer::new("fun if else".to_string(), Arc::new(keyword_manager));
        
        // These should be treated as identifiers, not keywords
        assert_eq!(lexer.check_keyword("fun"), None);
        assert_eq!(lexer.check_keyword("if"), None);
        assert_eq!(lexer.check_keyword("else"), None);
    }

    #[test]
    fn test_classify_identifier_method() {
        let lexer = create_test_lexer("test");
        
        // Test public identifier (starts with capital)
        let public_type = lexer.classify_identifier("PublicName");
        assert_eq!(public_type, TokenType::PublicIdentifier("PublicName".to_string()));
        
        // Test private identifier (starts with lowercase)
        let private_type = lexer.classify_identifier("privateName");
        assert_eq!(private_type, TokenType::PrivateIdentifier("privateName".to_string()));
        
        // Test private identifier (starts with underscore)
        let underscore_type = lexer.classify_identifier("_private");
        assert_eq!(underscore_type, TokenType::PrivateIdentifier("_private".to_string()));
    }
}

#[cfg(test)]
mod whitespace_handling_tests {
    use super::*;

    #[test]
    fn test_skip_whitespace() {
        let mut lexer = create_test_lexer("   \t  fun   \t  main  \n  ");
        
        let token1 = lexer.next_token().unwrap();
        assert_eq!(token1.token_type, TokenType::Fun);
        assert_eq!(token1.position.column, 7); // After whitespace
        
        let token2 = lexer.next_token().unwrap();
        assert_eq!(token2.token_type, TokenType::PrivateIdentifier("main".to_string()));
        
        let token3 = lexer.next_token().unwrap();
        assert_eq!(token3.token_type, TokenType::Newline);
        
        let token4 = lexer.next_token().unwrap();
        assert_eq!(token4.token_type, TokenType::EOF);
    }

    #[test]
    fn test_preserve_newlines() {
        let mut lexer = create_test_lexer("line1\n\nline2");
        
        let token1 = lexer.next_token().unwrap();
        assert_eq!(token1.token_type, TokenType::PrivateIdentifier("line1".to_string()));
        
        let token2 = lexer.next_token().unwrap();
        assert_eq!(token2.token_type, TokenType::Newline);
        
        let token3 = lexer.next_token().unwrap();
        assert_eq!(token3.token_type, TokenType::Newline);
        
        let token4 = lexer.next_token().unwrap();
        assert_eq!(token4.token_type, TokenType::PrivateIdentifier("line2".to_string()));
    }
}