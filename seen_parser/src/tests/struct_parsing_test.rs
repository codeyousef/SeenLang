// Struct parsing tests - TDD RED phase (these should fail initially)
use crate::parser::Parser;
use crate::ast::*;
use seen_lexer::token::{Token, TokenType, Location};

#[cfg(test)]
mod struct_parsing_tests {
    use super::*;

    fn create_test_tokens(source: &str) -> Vec<Token> {
        use seen_lexer::lexer::Lexer;
        use seen_lexer::keyword_config::{KeywordConfig, KeywordManager};
        use std::path::PathBuf;
        
        // Get the specifications directory relative to the parser crate
        let lang_files_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent() // Go up from seen_parser crate root to workspace root
            .unwrap()
            .join("specifications");
        
        let keyword_config = KeywordConfig::from_directory(&lang_files_dir)
            .expect("Failed to load keyword configuration for testing");
        
        let keyword_manager = KeywordManager::new(keyword_config, "en".to_string())
            .expect("Failed to create KeywordManager for testing");
            
        let mut lexer = Lexer::new(source, &keyword_manager, "en".to_string());
        lexer.tokenize().expect("Failed to tokenize")
    }

    #[test]
    fn test_parse_simple_struct() {
        // Test: "struct Point { x: int, y: int }"
        // EXPECTED: Should fail initially (RED) - no struct parsing implemented
        let source = "struct Point { x: int, y: int }";
        let tokens = create_test_tokens(source);
        let mut parser = Parser::new(tokens);
        
        let result = parser.parse();
        
        // This should parse to a Program with one StructDeclaration
        match result {
            Ok(program) => {
                assert_eq!(program.declarations.len(), 1);
                match &program.declarations[0] {
                    Declaration::Struct(struct_decl) => {
                        assert_eq!(struct_decl.name, "Point");
                        assert_eq!(struct_decl.fields.len(), 2);
                        
                        // Check first field
                        assert_eq!(struct_decl.fields[0].name, "x");
                        assert_eq!(struct_decl.fields[0].field_type, Type::Simple("int".to_string()));
                        
                        // Check second field
                        assert_eq!(struct_decl.fields[1].name, "y");
                        assert_eq!(struct_decl.fields[1].field_type, Type::Simple("int".to_string()));
                    }
                    _ => panic!("Expected struct declaration"),
                }
            }
            Err(e) => panic!("Expected successful parse, got error: {:?}", e),
        }
    }

    #[test]
    fn test_parse_empty_struct() {
        // Test: "struct Empty {}"
        // EXPECTED: Should fail initially (RED)
        let source = "struct Empty {}";
        let tokens = create_test_tokens(source);
        let mut parser = Parser::new(tokens);
        
        let result = parser.parse();
        
        match result {
            Ok(program) => {
                assert_eq!(program.declarations.len(), 1);
                match &program.declarations[0] {
                    Declaration::Struct(struct_decl) => {
                        assert_eq!(struct_decl.name, "Empty");
                        assert_eq!(struct_decl.fields.len(), 0);
                    }
                    _ => panic!("Expected struct declaration"),
                }
            }
            Err(e) => panic!("Expected successful parse, got error: {:?}", e),
        }
    }

    #[test]
    fn test_parse_mixed_type_struct() {
        // Test: "struct Mixed { name: string, age: int, active: bool }"
        // EXPECTED: Should fail initially (RED)
        let source = "struct Mixed { name: string, age: int, active: bool }";
        let tokens = create_test_tokens(source);
        let mut parser = Parser::new(tokens);
        
        let result = parser.parse();
        
        match result {
            Ok(program) => {
                assert_eq!(program.declarations.len(), 1);
                match &program.declarations[0] {
                    Declaration::Struct(struct_decl) => {
                        assert_eq!(struct_decl.name, "Mixed");
                        assert_eq!(struct_decl.fields.len(), 3);
                        
                        // Verify field types
                        assert_eq!(struct_decl.fields[0].field_type, Type::Simple("string".to_string()));
                        assert_eq!(struct_decl.fields[1].field_type, Type::Simple("int".to_string()));
                        assert_eq!(struct_decl.fields[2].field_type, Type::Simple("bool".to_string()));
                    }
                    _ => panic!("Expected struct declaration"),
                }
            }
            Err(e) => panic!("Expected successful parse, got error: {:?}", e),
        }
    }

    #[test]
    #[ignore = "Requires Arabic language mode in lexer"]
    fn test_parse_arabic_struct() {
        // Test: "هيكل نقطة { x: int, y: int }"
        // EXPECTED: Should fail initially (RED)
        let source = "هيكل نقطة { x: int, y: int }";
        let tokens = create_test_tokens(source);
        let mut parser = Parser::new(tokens);
        
        let result = parser.parse();
        
        match result {
            Ok(program) => {
                assert_eq!(program.declarations.len(), 1);
                match &program.declarations[0] {
                    Declaration::Struct(struct_decl) => {
                        assert_eq!(struct_decl.name, "نقطة");
                        assert_eq!(struct_decl.fields.len(), 2);
                    }
                    _ => panic!("Expected struct declaration"),
                }
            }
            Err(e) => panic!("Expected successful parse, got error: {:?}", e),
        }
    }

    #[test]
    fn test_parse_array_field_struct() {
        // Test: "struct Data { items: [int], names: [string] }"
        // EXPECTED: Should fail initially (RED)
        let source = "struct Data { items: [int], names: [string] }";
        let tokens = create_test_tokens(source);
        let mut parser = Parser::new(tokens);
        
        let result = parser.parse();
        
        match result {
            Ok(program) => {
                assert_eq!(program.declarations.len(), 1);
                match &program.declarations[0] {
                    Declaration::Struct(struct_decl) => {
                        assert_eq!(struct_decl.name, "Data");
                        assert_eq!(struct_decl.fields.len(), 2);
                        
                        // Check array types
                        assert_eq!(struct_decl.fields[0].field_type, Type::Array(Box::new(Type::Simple("int".to_string()))));
                        assert_eq!(struct_decl.fields[1].field_type, Type::Array(Box::new(Type::Simple("string".to_string()))));
                    }
                    _ => panic!("Expected struct declaration"),
                }
            }
            Err(e) => panic!("Expected successful parse, got error: {:?}", e),
        }
    }

    #[test]
    fn test_parse_struct_with_syntax_error() {
        // Test: "struct Bad { x: int y: int }" (missing comma)
        // EXPECTED: Should return parse error
        let source = "struct Bad { x: int y: int }";
        let tokens = create_test_tokens(source);
        let mut parser = Parser::new(tokens);
        
        let result = parser.parse();
        
        // This should fail with a parse error
        assert!(result.is_err(), "Expected parse error for missing comma");
    }

    #[test]
    fn test_parse_struct_with_duplicate_fields() {
        // Test: "struct Bad { x: int, x: string }"
        // EXPECTED: Should return parse error
        let source = "struct Bad { x: int, x: string }";
        let tokens = create_test_tokens(source);
        let mut parser = Parser::new(tokens);
        
        let result = parser.parse();
        
        // This should fail with a parse error for duplicate fields
        assert!(result.is_err(), "Expected parse error for duplicate fields");
    }

    #[test]
    fn test_struct_instantiation_parsing() {
        // Test: "val point = Point { x: 10, y: 20 };"
        // EXPECTED: Should fail initially (RED) - no struct literal parsing
        let source = "val point = Point { x: 10, y: 20 };";
        let tokens = create_test_tokens(source);
        let mut parser = Parser::new(tokens);
        
        let result = parser.parse();
        
        match result {
            Ok(program) => {
                assert_eq!(program.declarations.len(), 1);
                match &program.declarations[0] {
                    Declaration::Variable(var_decl) => {
                        assert_eq!(var_decl.name, "point");
                        match var_decl.initializer.as_ref() {
                            Expression::StructLiteral(struct_lit) => {
                                assert_eq!(struct_lit.struct_name, "Point");
                                assert_eq!(struct_lit.fields.len(), 2);
                            }
                            _ => panic!("Expected struct literal expression"),
                        }
                    }
                    _ => panic!("Expected variable declaration"),
                }
            }
            Err(e) => panic!("Expected successful parse, got error: {:?}", e),
        }
    }

    #[test]
    fn test_struct_field_access_parsing() {
        // Test: "val x = point.x;"
        // EXPECTED: Should fail initially (RED) - no field access parsing
        let source = "val x = point.x;";
        let tokens = create_test_tokens(source);
        let mut parser = Parser::new(tokens);
        
        let result = parser.parse();
        
        match result {
            Ok(program) => {
                assert_eq!(program.declarations.len(), 1);
                match &program.declarations[0] {
                    Declaration::Variable(var_decl) => {
                        assert_eq!(var_decl.name, "x");
                        match var_decl.initializer.as_ref() {
                            Expression::FieldAccess(field_access) => {
                                match field_access.object.as_ref() {
                                    Expression::Identifier(ident) => {
                                        assert_eq!(ident.name, "point");
                                    }
                                    _ => panic!("Expected identifier"),
                                }
                                assert_eq!(field_access.field, "x");
                            }
                            _ => panic!("Expected field access expression"),
                        }
                    }
                    _ => panic!("Expected variable declaration"),
                }
            }
            Err(e) => panic!("Expected successful parse, got error: {:?}", e),
        }
    }
}