//! Debug error recovery behavior

use seen_parser::{Parser};
use seen_lexer::{Lexer, LanguageConfig, TokenType};
use std::collections::HashMap;

#[test]
fn debug_error_recovery() {
    let config = create_english_config();
    
    let program_with_errors = r#"
        fun valid_function() {
            let x = 42;
        }
        
        fun invalid_syntax   // Missing parentheses and body
        
        fun another_valid_function() {
            return 100;
        }
        
        struct ValidStruct {
            value: i32,
        }
    "#;
    
    let mut lexer = Lexer::new(program_with_errors, 0, &config);
    let tokens = lexer.tokenize().expect("Tokenization should handle errors");
    
    println!("Generated {} tokens", tokens.len());
    
    // Find each major item
    let mut func_positions = Vec::new();
    let mut struct_positions = Vec::new();
    
    for (i, token) in tokens.iter().enumerate() {
        match &token.value {
            TokenType::KeywordFun => func_positions.push(i),
            TokenType::KeywordStruct => struct_positions.push(i),
            _ => {}
        }
    }
    
    println!("Found {} fun keywords at positions: {:?}", func_positions.len(), func_positions);
    println!("Found {} struct keywords at positions: {:?}", struct_positions.len(), struct_positions);
    
    let mut parser = Parser::new(tokens);
    let result = parser.parse_program();
    
    match result {
        Ok(ast) => {
            println!("\nGenerated AST with {} items:", ast.items.len());
            for (i, item) in ast.items.iter().enumerate() {
                match &item.kind {
                    seen_parser::ItemKind::Function(func) => {
                        println!("  {}: Function '{}'", i, func.name.value);
                    }
                    seen_parser::ItemKind::Struct(struct_def) => {
                        println!("  {}: Struct '{}'", i, struct_def.name.value);
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
            
            // Should have produced at least 2 items (valid functions/structs)
            assert!(ast.items.len() >= 2);
            assert!(parser.diagnostics().has_errors());
        }
        Err(e) => {
            println!("Parsing failed completely: {:?}", e);
            panic!("Error recovery should produce AST even with errors");
        }
    }
}

fn create_english_config() -> LanguageConfig {
    let mut keywords = HashMap::new();
    keywords.insert("fun".to_string(), "TokenFun".to_string());
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