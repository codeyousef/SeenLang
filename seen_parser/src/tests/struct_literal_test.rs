use crate::ast::*;
// Tests for struct literal parsing
use crate::parser::Parser;
use seen_lexer::token::{Location, Position, Token, TokenType};

#[cfg(test)]
mod struct_literal_tests {
    /// Helper to create a token with location
    fn make_token(token_type: TokenType, lexeme: &str, line: usize, col: usize) -> Token {
        let pos = Position::new(line, col);
        Token {
            token_type,
            lexeme: lexeme.to_string(),
            location: Location::new(pos, Position::new(line, col + lexeme.len())),
            language: "en".to_string(),
        }
    }

    /// Create tokens for: Point { x: 10, y: 20 }
    fn struct_literal_tokens() -> Vec<Token> {
        vec![
            make_token(TokenType::Identifier, "Point", 1, 1),
            make_token(TokenType::LeftBrace, "{", 1, 7),
            make_token(TokenType::Identifier, "x", 1, 9),
            make_token(TokenType::Colon, ":", 1, 10),
            make_token(TokenType::IntLiteral, "10", 1, 12),
            make_token(TokenType::Comma, ",", 1, 14),
            make_token(TokenType::Identifier, "y", 1, 16),
            make_token(TokenType::Colon, ":", 1, 17),
            make_token(TokenType::IntLiteral, "20", 1, 19),
            make_token(TokenType::RightBrace, "}", 1, 22),
            make_token(TokenType::EOF, "", 1, 23),
        ]
    }

    #[test]
    fn test_parse_struct_literal_basic() {
        let tokens = struct_literal_tokens();
        let mut parser = Parser::new(tokens);

        // Try to parse as an expression
        let result = parser.parse_expression();

        match result {
            Ok(expr) => {
                match expr {
                    Expression::StructLiteral(struct_lit) => {
                        assert_eq!(struct_lit.struct_name, "Point");
                        assert_eq!(struct_lit.fields.len(), 2);

                        // Check first field
                        assert_eq!(struct_lit.fields[0].field_name, "x");
                        // Check the value is 10

                        // Check second field
                        assert_eq!(struct_lit.fields[1].field_name, "y");
                        // Check the value is 20
                    }
                    _ => panic!("Expected StructLiteral expression, got {:?}", expr),
                }
            }
            Err(e) => panic!("Failed to parse struct literal: {:?}", e),
        }
    }

    #[test]
    fn test_parse_empty_struct_literal() {
        // Test: Empty {}
        let tokens = vec![
            make_token(TokenType::Identifier, "Empty", 1, 1),
            make_token(TokenType::LeftBrace, "{", 1, 7),
            make_token(TokenType::RightBrace, "}", 1, 8),
            make_token(TokenType::EOF, "", 1, 9),
        ];

        let mut parser = Parser::new(tokens);
        let result = parser.parse_expression();

        match result {
            Ok(Expression::StructLiteral(struct_lit)) => {
                assert_eq!(struct_lit.struct_name, "Empty");
                assert_eq!(struct_lit.fields.len(), 0);
            }
            Ok(other) => panic!("Expected StructLiteral, got {:?}", other),
            Err(e) => panic!("Failed to parse empty struct literal: {:?}", e),
        }
    }

    #[test]
    fn test_parse_nested_struct_literal() {
        // Test: Line { start: Point { x: 0, y: 0 }, end: Point { x: 10, y: 10 } }
        let tokens = vec![
            make_token(TokenType::Identifier, "Line", 1, 1),
            make_token(TokenType::LeftBrace, "{", 1, 6),
            make_token(TokenType::Identifier, "start", 1, 8),
            make_token(TokenType::Colon, ":", 1, 13),
            // Nested Point struct
            make_token(TokenType::Identifier, "Point", 1, 15),
            make_token(TokenType::LeftBrace, "{", 1, 21),
            make_token(TokenType::Identifier, "x", 1, 23),
            make_token(TokenType::Colon, ":", 1, 24),
            make_token(TokenType::IntLiteral, "0", 1, 26),
            make_token(TokenType::Comma, ",", 1, 27),
            make_token(TokenType::Identifier, "y", 1, 29),
            make_token(TokenType::Colon, ":", 1, 30),
            make_token(TokenType::IntLiteral, "0", 1, 32),
            make_token(TokenType::RightBrace, "}", 1, 33),
            make_token(TokenType::Comma, ",", 1, 34),
            // end field
            make_token(TokenType::Identifier, "end", 1, 36),
            make_token(TokenType::Colon, ":", 1, 39),
            // Another nested Point
            make_token(TokenType::Identifier, "Point", 1, 41),
            make_token(TokenType::LeftBrace, "{", 1, 47),
            make_token(TokenType::Identifier, "x", 1, 49),
            make_token(TokenType::Colon, ":", 1, 50),
            make_token(TokenType::IntLiteral, "10", 1, 52),
            make_token(TokenType::Comma, ",", 1, 54),
            make_token(TokenType::Identifier, "y", 1, 56),
            make_token(TokenType::Colon, ":", 1, 57),
            make_token(TokenType::IntLiteral, "10", 1, 59),
            make_token(TokenType::RightBrace, "}", 1, 61),
            make_token(TokenType::RightBrace, "}", 1, 63),
            make_token(TokenType::EOF, "", 1, 64),
        ];

        let mut parser = Parser::new(tokens);
        let result = parser.parse_expression();

        match result {
            Ok(Expression::StructLiteral(struct_lit)) => {
                assert_eq!(struct_lit.struct_name, "Line");
                assert_eq!(struct_lit.fields.len(), 2);
                assert_eq!(struct_lit.fields[0].field_name, "start");
                assert_eq!(struct_lit.fields[1].field_name, "end");

                // Verify nested structs
                match &*struct_lit.fields[0].value {
                    Expression::StructLiteral(nested) => {
                        assert_eq!(nested.struct_name, "Point");
                    }
                    _ => panic!("Expected nested struct literal"),
                }
            }
            Ok(other) => panic!("Expected StructLiteral, got {:?}", other),
            Err(e) => panic!("Failed to parse nested struct literal: {:?}", e),
        }
    }

    #[test]
    fn test_struct_literal_in_assignment() {
        // Test: val point = Point { x: 10, y: 20 };
        let tokens = vec![
            make_token(TokenType::Val, "val", 1, 1),
            make_token(TokenType::Identifier, "point", 1, 5),
            make_token(TokenType::Assign, "=", 1, 11),
            make_token(TokenType::Identifier, "Point", 1, 13),
            make_token(TokenType::LeftBrace, "{", 1, 19),
            make_token(TokenType::Identifier, "x", 1, 21),
            make_token(TokenType::Colon, ":", 1, 22),
            make_token(TokenType::IntLiteral, "10", 1, 24),
            make_token(TokenType::Comma, ",", 1, 26),
            make_token(TokenType::Identifier, "y", 1, 28),
            make_token(TokenType::Colon, ":", 1, 29),
            make_token(TokenType::IntLiteral, "20", 1, 31),
            make_token(TokenType::RightBrace, "}", 1, 33),
            make_token(TokenType::Semicolon, ";", 1, 34),
            make_token(TokenType::EOF, "", 1, 35),
        ];

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
                            }
                            _ => panic!("Expected struct literal initializer"),
                        }
                    }
                    _ => panic!("Expected variable declaration"),
                }
            }
            Err(e) => panic!("Failed to parse struct literal assignment: {:?}", e),
        }
    }

    #[test]
    fn test_struct_literal_with_expressions() {
        // Test: Point { x: a + b, y: func() }
        let tokens = vec![
            make_token(TokenType::Identifier, "Point", 1, 1),
            make_token(TokenType::LeftBrace, "{", 1, 7),
            make_token(TokenType::Identifier, "x", 1, 9),
            make_token(TokenType::Colon, ":", 1, 10),
            make_token(TokenType::Identifier, "a", 1, 12),
            make_token(TokenType::Plus, "+", 1, 14),
            make_token(TokenType::Identifier, "b", 1, 16),
            make_token(TokenType::Comma, ",", 1, 17),
            make_token(TokenType::Identifier, "y", 1, 19),
            make_token(TokenType::Colon, ":", 1, 20),
            make_token(TokenType::Identifier, "func", 1, 22),
            make_token(TokenType::LeftParen, "(", 1, 26),
            make_token(TokenType::RightParen, ")", 1, 27),
            make_token(TokenType::RightBrace, "}", 1, 29),
            make_token(TokenType::EOF, "", 1, 30),
        ];

        let mut parser = Parser::new(tokens);
        let result = parser.parse_expression();

        match result {
            Ok(Expression::StructLiteral(struct_lit)) => {
                assert_eq!(struct_lit.struct_name, "Point");
                assert_eq!(struct_lit.fields.len(), 2);

                // Check that field values are expressions
                match &*struct_lit.fields[0].value {
                    Expression::Binary(_) => {} // Expected
                    _ => panic!("Expected binary expression for x field"),
                }

                match &*struct_lit.fields[1].value {
                    Expression::Call(_) => {} // Expected
                    _ => panic!("Expected call expression for y field"),
                }
            }
            Ok(other) => panic!("Expected StructLiteral, got {:?}", other),
            Err(e) => panic!("Failed to parse struct literal with expressions: {:?}", e),
        }
    }
}