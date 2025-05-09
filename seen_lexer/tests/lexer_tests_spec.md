# Lexer Tests Specification

This document outlines the test cases required to validate the behavior of the Seen language lexer, with a particular focus on bilingual keyword handling.

## Test Categories

The test suite should cover the following categories:

1. Basic Tokenization
2. Bilingual Keyword Handling
3. Literals (Numbers, Strings)
4. Identifiers and Unicode Support
5. Operators and Delimiters
6. Comments
7. Error Conditions
8. Integration with Project Configuration

## 1. Basic Tokenization Tests

### 1.1 Single Token Tests
- Test each token type in isolation
- Verify correct token type and lexeme
- Verify correct source location information

### 1.2 Mixed Token Tests
- Test sequence of different token types
- Verify correct order and content of token stream
- Verify handling of whitespace and token boundaries

### 1.3 Complete Statement Tests
- Test tokenization of complete language constructs
- Verify all tokens in standard programming patterns like declarations, control flow, etc.

## 2. Bilingual Keyword Handling Tests

### 2.1 English Keywords
- Test all English keywords individually and in combination
- Verify mapping to correct token types
- Verify original lexeme preservation

### 2.2 Arabic Keywords
- Test all Arabic keywords individually and in combination
- Verify mapping to correct token types
- Verify original lexeme preservation

### 2.3 Language Selection
- Test changing the active language through the keyword manager
- Verify same token types produced for semantically equivalent programs in different languages

### 2.4 Mixed Language (if supported)
- Test mixed English/Arabic keywords in the same file (if allowed by configuration)
- Verify error generation if mixed keywords are not allowed by configuration

## 3. Literal Tests

### 3.1 Integer Literals
- Test various integer literals (0, positive, negative, large numbers)
- Verify correct token type and value

### 3.2 Float Literals
- Test various floating-point literals (whole numbers, fractions, scientific notation)
- Verify correct token type and value

### 3.3 String Literals
- Test empty strings, simple strings, strings with escape sequences
- Test strings with Unicode characters, including Arabic
- Verify correct token type and value
- Verify handling of unterminated strings (error case)

## 4. Identifier and Unicode Support Tests

### 4.1 Basic Identifiers
- Test ASCII identifiers with various combinations of letters, digits, and underscores
- Verify correct token type and value

### 4.2 Unicode Identifiers
- Test identifiers with Arabic characters
- Test identifiers with mixed scripts
- Verify correct token type and value
- Verify correct handling of RTL text

### 4.3 Keyword vs. Identifier Disambiguation
- Test cases where an identifier in one language is a keyword in another
- Verify correct behavior based on the active language setting

## 5. Operator and Delimiter Tests

### 5.1 Single-Character Operators
- Test all single-character operators (+, -, *, /, etc.)
- Verify correct token type and value

### 5.2 Multi-Character Operators
- Test all multi-character operators (==, !=, <=, >=, &&, ||, etc.)
- Verify correct token type and value
- Verify correct handling of operator boundaries

### 5.3 Delimiters
- Test all delimiter tokens (parentheses, braces, brackets, etc.)
- Verify correct token type and value

## 6. Comment Tests

### 6.1 Single-Line Comments
- Test English-style single-line comments (//)
- Test Arabic-style single-line comments (##)
- Verify comments are correctly skipped

### 6.2 Multi-Line Comments
- Test single and multi-line block comments (/* ... */)
- Test nested block comments
- Verify comments are correctly skipped
- Verify handling of unterminated comments (error case)

## 7. Error Condition Tests

### 7.1 Invalid Characters
- Test handling of invalid characters in source code
- Verify appropriate error generation

### 7.2 Malformed Tokens
- Test malformed numbers, strings, etc.
- Verify appropriate error generation

### 7.3 Error Recovery
- Test lexer's ability to recover from errors and continue tokenizing
- Verify error tokens don't prevent processing of subsequent valid tokens

## 8. Integration Tests

### 8.1 Project Configuration Integration
- Test loading language settings from project configuration
- Verify lexer's behavior changes appropriately based on configuration

### 8.2 Keywords File Integration
- Test loading different keyword mapping files
- Verify lexer's behavior changes appropriately based on mappings

## Test Implementation Notes

Each test should verify:
1. The correct token type is produced
2. The original lexeme is preserved
3. Source location information is accurate
4. The language field is set correctly

Test files should be organized by category and each test should have a clear and descriptive name indicating what it's testing.

## Example Test Case: Arabic Hello World

```rust
#[test]
fn test_arabic_hello_world() {
    let keyword_manager = create_test_keyword_manager("ar");
    
    let source = r#"
    دالة رئيسية() {
        اطبع("مرحبا، يا عالم!");
        إرجاع 0;
    }
    "#;
    
    let mut lexer = Lexer::new(source, &keyword_manager);
    let tokens = lexer.tokenize().unwrap();
    
    // Verify expected token sequence
    assert_eq!(tokens[0].token_type, TokenType::Func);
    assert_eq!(tokens[0].lexeme, "دالة");
    assert_eq!(tokens[0].language, "ar");
    
    // Verify Println token
    let println_token = tokens.iter().find(|t| t.token_type == TokenType::Println).unwrap();
    assert_eq!(println_token.lexeme, "اطبع");
    
    // Verify string content
    let string_token = tokens.iter().find(|t| t.token_type == TokenType::StringLiteral).unwrap();
    assert_eq!(string_token.lexeme, "مرحبا، يا عالم!");
    
    // Verify Return token
    let return_token = tokens.iter().find(|t| t.token_type == TokenType::Return).unwrap();
    assert_eq!(return_token.lexeme, "إرجاع");
}
```

This specification serves as a guide for implementing comprehensive tests for the Seen language lexer, ensuring it correctly handles all aspects of lexical analysis with a focus on the bilingual keyword system.
