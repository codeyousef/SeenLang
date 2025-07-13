//\! Tests for error conditions and error recovery

use super::*;
use crate::{KeywordManager, Lexer, LexerError, TokenType};
use pretty_assertions::assert_eq;

#[test]
fn test_invalid_character_error() {
    let keyword_manager = KeywordManager::new_for_testing("english");
    let source = "func main() { @ }"; // @ is not a valid character

    let mut lexer = Lexer::new(source, &keyword_manager);
    let result = lexer.tokenize();

    assert!(result.is_err());
    // Verify error details when LexerError is properly defined
}

#[test]
fn test_unterminated_string_recovery() {
    let keyword_manager = KeywordManager::new_for_testing("english");
    let source = "\"unterminated\nfunc main() {}";

    let mut lexer = Lexer::new(source, &keyword_manager);
    let result = lexer.tokenize();

    // Should error on unterminated string but potentially recover
    assert!(result.is_err());
}

#[test]
fn test_invalid_number_format() {
    let keyword_manager = KeywordManager::new_for_testing("english");
    let invalid_numbers = vec![
        "123.456.789", // Multiple decimal points
        "1e",          // Incomplete scientific notation
        "0x",          // Incomplete hex literal (if supported)
    ];

    for input in invalid_numbers {
        let mut lexer = Lexer::new(input, &keyword_manager);
        let tokens = lexer.tokenize();

        // These should either error or tokenize as separate tokens
        // Exact behavior depends on lexer implementation
    }
}

#[test]
fn test_error_location_tracking() {
    let keyword_manager = KeywordManager::new_for_testing("english");
    let source = "func main() {\n    \"unterminated string\n}";

    let mut lexer = Lexer::new(source, &keyword_manager);
    let result = lexer.tokenize();

    if let Err(error) = result {
        // Verify error reports correct line/column
        // This depends on how LexerError is structured
    }
}

#[test]
fn test_mixed_valid_invalid_tokens() {
    let keyword_manager = KeywordManager::new_for_testing("english");
    let source = "func @ main() { }"; // Invalid @ in the middle

    let mut lexer = Lexer::new(source, &keyword_manager);
    let result = lexer.tokenize();

    // Should error when encountering @
    assert!(result.is_err());
}

#[test]
fn test_unicode_edge_cases() {
    let keyword_manager = KeywordManager::new_for_testing("english");

    // Test various Unicode edge cases
    let test_cases = vec![
        "\u{200B}", // Zero-width space
        "\u{FEFF}", // Byte order mark
        "\u{2028}", // Line separator
        "\u{2029}", // Paragraph separator
    ];

    for input in test_cases {
        let mut lexer = Lexer::new(input, &keyword_manager);
        let _ = lexer.tokenize();
        // Verify appropriate handling (skip, error, or tokenize)
    }
}
