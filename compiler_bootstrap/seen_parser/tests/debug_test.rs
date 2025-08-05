//! Debug test to understand parsing behavior

use seen_parser::{Parser};
use seen_lexer::{Lexer, LanguageConfig, TokenType};
use std::collections::HashMap;

#[test]
fn debug_parser_detailed() {
    let config = create_english_config();
    
    let program = r#"
        func fibonacci(n: i32) -> i32 {
            if n <= 1 {
                return n;
            }
            return fibonacci(n - 1) + fibonacci(n - 2);
        }
        
        struct Point {
            x: f64,
            y: f64,
        }
        
        enum Color {
            Red,
            Green,
            Blue,
            RGB(u8, u8, u8),
        }
        
        func main() {
            let x = 42;
            let result = fibonacci(10);
            return 0;
        }
    "#;
    
    let mut lexer = Lexer::new(program, 0, &config);
    let tokens = lexer.tokenize().expect("Tokenization must succeed");
    
    println!("Generated {} tokens", tokens.len());
    
    // Find each major item
    let mut func_positions = Vec::new();
    let mut struct_positions = Vec::new();
    let mut enum_positions = Vec::new();
    
    for (i, token) in tokens.iter().enumerate() {
        match &token.value {
            TokenType::KeywordFunc => func_positions.push(i),
            TokenType::KeywordStruct => struct_positions.push(i),
            TokenType::KeywordEnum => enum_positions.push(i),
            _ => {}
        }
    }
    
    println!("Found {} func keywords at positions: {:?}", func_positions.len(), func_positions);
    println!("Found {} struct keywords at positions: {:?}", struct_positions.len(), struct_positions);
    println!("Found {} enum keywords at positions: {:?}", enum_positions.len(), enum_positions);
    
    let mut parser = Parser::new(tokens);
    let ast = parser.parse_program().expect("Parsing must succeed");
    
    println!("\nGenerated AST with {} items:", ast.items.len());
    for (i, item) in ast.items.iter().enumerate() {
        match &item.kind {
            seen_parser::ItemKind::Function(func) => {
                println!("  {}: Function '{}'", i, func.name.value);
            }
            seen_parser::ItemKind::Struct(struct_def) => {
                println!("  {}: Struct '{}'", i, struct_def.name.value);
            }
            seen_parser::ItemKind::Enum(enum_def) => {
                println!("  {}: Enum '{}'", i, enum_def.name.value);
            }
            _ => {
                println!("  {}: Other item", i);
            }
        }
    }
    
    if parser.diagnostics().has_errors() {
        println!("\nParser errors ({} total):", parser.diagnostics().error_count());
        for diagnostic in &parser.diagnostics().messages {
            if diagnostic.severity == seen_common::Severity::Error {
                println!("  {}", diagnostic.message);
            }
        }
    }
    
    // Expected: 2 func, 1 struct, 1 enum = 4 total items
    assert_eq!(func_positions.len(), 2, "Should find 2 func keywords");
    assert_eq!(struct_positions.len(), 1, "Should find 1 struct keyword");  
    assert_eq!(enum_positions.len(), 1, "Should find 1 enum keyword");
    assert_eq!(ast.items.len(), 4, "Should parse 4 items");
}

fn create_english_config() -> LanguageConfig {
    let mut keywords = HashMap::new();
    keywords.insert("func".to_string(), "TokenFunc".to_string());
    keywords.insert("let".to_string(), "TokenLet".to_string());
    keywords.insert("if".to_string(), "TokenIf".to_string());
    keywords.insert("else".to_string(), "TokenElse".to_string());
    keywords.insert("while".to_string(), "TokenWhile".to_string());
    keywords.insert("for".to_string(), "TokenFor".to_string());
    keywords.insert("return".to_string(), "TokenReturn".to_string());
    keywords.insert("struct".to_string(), "TokenStruct".to_string());
    keywords.insert("enum".to_string(), "TokenEnum".to_string());
    keywords.insert("true".to_string(), "TokenTrue".to_string());
    keywords.insert("false".to_string(), "TokenFalse".to_string());
    
    let mut operators = HashMap::new();
    operators.insert("+".to_string(), "TokenPlus".to_string());
    operators.insert("-".to_string(), "TokenMinus".to_string());
    operators.insert("*".to_string(), "TokenMultiply".to_string());
    operators.insert("/".to_string(), "TokenDivide".to_string());
    operators.insert("=".to_string(), "TokenAssign".to_string());
    operators.insert("==".to_string(), "TokenEqual".to_string());
    operators.insert("!=".to_string(), "TokenNotEqual".to_string());
    operators.insert("<".to_string(), "TokenLess".to_string());
    operators.insert("<=".to_string(), "TokenLessEqual".to_string());
    operators.insert(">".to_string(), "TokenGreater".to_string());
    operators.insert(">=".to_string(), "TokenGreaterEqual".to_string());
    operators.insert("&&".to_string(), "TokenLogicalAnd".to_string());
    operators.insert("||".to_string(), "TokenLogicalOr".to_string());
    
    LanguageConfig {
        keywords,
        operators,
        name: "English".to_string(),
        description: Some("English test configuration".to_string()),
    }
}