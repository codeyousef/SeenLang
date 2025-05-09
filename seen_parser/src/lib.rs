//! The parser for the Seen programming language
//! 
//! This crate contains the parser that transforms tokens from the lexer into an Abstract Syntax Tree (AST).
//! The parser handles both English and Arabic keywords through the language-neutral token types.

pub mod ast;
pub mod parser;

pub use parser::{Parser, ParserError};

/// Parse a sequence of tokens into an AST
pub fn parse(tokens: Vec<seen_lexer::token::Token>) -> Result<ast::Program, parser::ParserError> {
    let mut parser = parser::Parser::new(tokens);
    parser.parse()
}

#[cfg(test)]
mod tests {
    use super::*;
    use seen_lexer::token::{Token, TokenType, Position, Location};

    #[test]
    fn test_basic_parsing() {
        // Create a simple token stream for a Hello World program
        let tokens = vec![
            Token {
                token_type: TokenType::Func,
                lexeme: "func".to_string(),
                location: Location::from_positions(1, 1, 1, 5),
                language: "en".to_string(),
            },
            Token {
                token_type: TokenType::Identifier,
                lexeme: "main".to_string(),
                location: Location::from_positions(1, 6, 1, 10),
                language: "en".to_string(),
            },
            Token {
                token_type: TokenType::LeftParen,
                lexeme: "(".to_string(),
                location: Location::from_positions(1, 10, 1, 11),
                language: "en".to_string(),
            },
            Token {
                token_type: TokenType::RightParen,
                lexeme: ")".to_string(),
                location: Location::from_positions(1, 11, 1, 12),
                language: "en".to_string(),
            },
            Token {
                token_type: TokenType::LeftBrace,
                lexeme: "{".to_string(),
                location: Location::from_positions(1, 13, 1, 14),
                language: "en".to_string(),
            },
            Token {
                token_type: TokenType::Println,
                lexeme: "println".to_string(),
                location: Location::from_positions(2, 5, 2, 12),
                language: "en".to_string(),
            },
            Token {
                token_type: TokenType::LeftParen,
                lexeme: "(".to_string(),
                location: Location::from_positions(2, 12, 2, 13),
                language: "en".to_string(),
            },
            Token {
                token_type: TokenType::StringLiteral,
                lexeme: "Hello, World!".to_string(),
                location: Location::from_positions(2, 13, 2, 27),
                language: "en".to_string(),
            },
            Token {
                token_type: TokenType::RightParen,
                lexeme: ")".to_string(),
                location: Location::from_positions(2, 27, 2, 28),
                language: "en".to_string(),
            },
            Token {
                token_type: TokenType::Semicolon,
                lexeme: ";".to_string(),
                location: Location::from_positions(2, 28, 2, 29),
                language: "en".to_string(),
            },
            Token {
                token_type: TokenType::RightBrace,
                lexeme: "}".to_string(),
                location: Location::from_positions(3, 1, 3, 2),
                language: "en".to_string(),
            },
            Token {
                token_type: TokenType::EOF,
                lexeme: "".to_string(),
                location: Location::from_positions(3, 2, 3, 2),
                language: "en".to_string(),
            },
        ];

        // Parse the tokens
        let result = parse(tokens);
        
        // Check that parsing succeeded
        assert!(result.is_ok());
        
        // Basic validation of the AST structure
        let program = result.unwrap();
        assert_eq!(program.declarations.len(), 1);
        
        if let ast::Declaration::Function(func) = &program.declarations[0] {
            assert_eq!(func.name, "main");
            assert_eq!(func.parameters.len(), 0);
            assert!(func.return_type.is_none());
            assert_eq!(func.body.statements.len(), 1);
            
            if let ast::Statement::Print(print_stmt) = &func.body.statements[0] {
                if let ast::Expression::Literal(ast::LiteralExpression::String(string_lit)) = &*print_stmt.expression {
                    assert_eq!(string_lit.value, "Hello, World!");
                } else {
                    panic!("Expected string literal");
                }
            } else {
                panic!("Expected print statement");
            }
        } else {
            panic!("Expected function declaration");
        }
    }
}
