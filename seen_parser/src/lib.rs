// The parser for the Seen programming language
//
// This crate contains the parser that transforms tokens from the lexer into an Abstract Syntax Tree (AST).
// The parser handles both English and Arabic keywords through the language-neutral token types.

pub mod ast;
pub mod parser;

pub use parser::{Parser, ParserError};
pub use parse as parse_program;

/// Parse a sequence of tokens into an AST
pub fn parse(tokens: Vec<seen_lexer::token::Token>) -> Result<ast::Program, parser::ParserError> {
    let mut parser = parser::Parser::new(tokens);
    parser.parse()
}

#[cfg(test)]
mod tests {
    use super::*; // Make items from parent module (lib.rs) accessible
    use seen_lexer::lexer::Lexer;
    use seen_lexer::token::{Token, TokenType, Location};
    use seen_lexer::keyword_config::{KeywordConfig, KeywordManager};
    use std::path::PathBuf;

    // Helper function to tokenize source and filter out non-essential tokens for parser tests
    fn tokenize_and_filter(source: &str) -> Vec<Token> {
        // Define the path to the language configuration files directory
        let lang_files_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent() // Go up from seen_parser crate root to workspace root (/usr/src/app)
            .unwrap()
            .join("specifications"); // Language files are in /usr/src/app/specifications/

        let keyword_config = KeywordConfig::from_directory(&lang_files_dir)
            .unwrap_or_else(|e| panic!("Failed to load keyword configuration from {:?}: {:?}", lang_files_dir, e));
        
        let active_lang = "en".to_string();
        let keyword_manager = KeywordManager::new(keyword_config, active_lang.clone())
            .unwrap_or_else(|e| panic!("Failed to create KeywordManager: {:?}", e));

        let mut lexer = Lexer::new(source, &keyword_manager, active_lang);
        let mut tokens = Vec::new();
        loop {
            match lexer.next_token() {
                Ok(token) => {
                    let is_eof = token.token_type == TokenType::EOF;
                    // Lexer is assumed to handle/skip whitespace and comments internally.
                    // All tokens from the lexer are passed to the parser.
                    tokens.push(token);
                    if is_eof { // Ensure EOF is the last token and stop
                        break;
                    }
                }
                Err(e) => panic!("Lexing failed during test setup: {:?}", e),
            }
        }
        tokens
    }

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
        let result = super::parse(tokens);
        
        // Check that parsing succeeded
        assert!(result.is_ok(), "test_basic_parsing failed: {:?}", result.err());
        
        // Basic validation of the AST structure
        let program = result.unwrap();
        assert_eq!(program.declarations.len(), 1);
        
        if let ast::Declaration::Function(func) = &program.declarations[0] {
            assert_eq!(func.name, "main");
            assert_eq!(func.parameters.len(), 0);
            assert!(func.return_type.is_none());
            assert_eq!(func.body.statements.len(), 1);
            
            if let ast::Statement::Print(print_stmt) = &func.body.statements[0] {
                assert_eq!(print_stmt.arguments.len(), 1);
                if let ast::Expression::Literal(ast::LiteralExpression::String(string_lit)) = &print_stmt.arguments[0] {
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

    #[test]
    fn test_variable_declarations() {
        // Case 1: val x = 10;
        let tokens_val = vec![
            Token {
                token_type: TokenType::Val,
                lexeme: "val".to_string(),
                location: Location::from_positions(1, 1, 1, 4),
                language: "en".to_string(),
            },
            Token {
                token_type: TokenType::Identifier,
                lexeme: "x".to_string(),
                location: Location::from_positions(1, 5, 1, 6),
                language: "en".to_string(),
            },
            Token {
                token_type: TokenType::Assign,
                lexeme: "=".to_string(),
                location: Location::from_positions(1, 7, 1, 8),
                language: "en".to_string(),
            },
            Token {
                token_type: TokenType::IntLiteral,
                lexeme: "10".to_string(),
                location: Location::from_positions(1, 9, 1, 11),
                language: "en".to_string(),
            },
            Token {
                token_type: TokenType::Semicolon,
                lexeme: ";".to_string(),
                location: Location::from_positions(1, 11, 1, 12),
                language: "en".to_string(),
            },
            Token {
                token_type: TokenType::EOF,
                lexeme: "".to_string(),
                location: Location::from_positions(1, 12, 1, 12),
                language: "en".to_string(),
            },
        ];

        let result_val = super::parse(tokens_val);
        assert!(result_val.is_ok(), "Failed to parse 'val x = 10;': {:?}", result_val.err());
        let program_val = result_val.unwrap();
        assert_eq!(program_val.declarations.len(), 1);

        if let ast::Declaration::Variable(var_decl) = &program_val.declarations[0] {
            assert_eq!(var_decl.is_mutable, false);
            assert_eq!(var_decl.name, "x");
            assert!(var_decl.var_type.is_none());
            if let ast::Expression::Literal(ast::LiteralExpression::Number(num_lit)) = &*var_decl.initializer {
                assert_eq!(num_lit.value, "10");
            } else {
                panic!("Expected number literal initializer for 'val x'");
            }
        } else {
            panic!("Expected variable declaration for 'val x = 10;'");
        }

        // Case 2: var y: String = "hello";
        let tokens_var_typed = vec![
            Token {
                token_type: TokenType::Var,
                lexeme: "var".to_string(),
                location: Location::from_positions(1, 1, 1, 4),
                language: "en".to_string(),
            },
            Token {
                token_type: TokenType::Identifier,
                lexeme: "y".to_string(),
                location: Location::from_positions(1, 5, 1, 6),
                language: "en".to_string(),
            },
            Token {
                token_type: TokenType::Colon,
                lexeme: ":".to_string(),
                location: Location::from_positions(1, 6, 1, 7),
                language: "en".to_string(),
            },
            Token {
                token_type: TokenType::Identifier, // Assuming 'String' is treated as an identifier for type
                lexeme: "String".to_string(),
                location: Location::from_positions(1, 8, 1, 14),
                language: "en".to_string(),
            },
            Token {
                token_type: TokenType::Assign,
                lexeme: "=".to_string(),
                location: Location::from_positions(1, 15, 1, 16),
                language: "en".to_string(),
            },
            Token {
                token_type: TokenType::StringLiteral,
                lexeme: "hello".to_string(),
                location: Location::from_positions(1, 17, 1, 24),
                language: "en".to_string(),
            },
            Token {
                token_type: TokenType::Semicolon,
                lexeme: ";".to_string(),
                location: Location::from_positions(1, 24, 1, 25),
                language: "en".to_string(),
            },
            Token {
                token_type: TokenType::EOF,
                lexeme: "".to_string(),
                location: Location::from_positions(1, 25, 1, 25),
                language: "en".to_string(),
            },
        ];

        let result_var_typed = super::parse(tokens_var_typed);
        assert!(result_var_typed.is_ok(), "Failed to parse 'var y: String = \"hello\";': {:?}", result_var_typed.err());
        let program_var_typed = result_var_typed.unwrap();
        assert_eq!(program_var_typed.declarations.len(), 1);

        if let ast::Declaration::Variable(var_decl) = &program_var_typed.declarations[0] {
            assert_eq!(var_decl.is_mutable, true);
            assert_eq!(var_decl.name, "y");
            assert!(var_decl.var_type.is_some());
            if let Some(ast::Type::Simple(type_name)) = &var_decl.var_type {
                assert_eq!(type_name, "String");
            } else {
                panic!("Expected Simple type 'String' for 'var y'");
            }
            if let ast::Expression::Literal(ast::LiteralExpression::String(str_lit)) = &*var_decl.initializer {
                assert_eq!(str_lit.value, "hello");
            } else {
                panic!("Expected string literal initializer for 'var y'");
            }
        } else {
            panic!("Expected variable declaration for 'var y: String = \"hello\";'");
        }
    }

    #[test]
    fn test_if_statements() {
        // Case 1: func testIf() { if (true) { println("then"); } }
        let tokens_if_in_func = vec![
            Token { token_type: TokenType::Func, lexeme: "func".to_string(), location: Location::from_positions(1,1,1,5), language: "en".to_string() },
            Token { token_type: TokenType::Identifier, lexeme: "testIf".to_string(), location: Location::from_positions(1,6,1,12), language: "en".to_string() },
            Token { token_type: TokenType::LeftParen, lexeme: "(".to_string(), location: Location::from_positions(1,12,1,13), language: "en".to_string() },
            Token { token_type: TokenType::RightParen, lexeme: ")".to_string(), location: Location::from_positions(1,13,1,14), language: "en".to_string() },
            Token { token_type: TokenType::LeftBrace, lexeme: "{".to_string(), location: Location::from_positions(1,15,1,16), language: "en".to_string() }, // Func body opens
            // Actual if statement tokens from tokens_if_only, adjusting line/col if needed (here, assuming same line for simplicity)
            Token { token_type: TokenType::If, lexeme: "if".to_string(), location: Location::from_positions(2, 1, 2, 3), language: "en".to_string() },
            Token { token_type: TokenType::LeftParen, lexeme: "(".to_string(), location: Location::from_positions(2, 4, 2, 5), language: "en".to_string() },
            Token { token_type: TokenType::True, lexeme: "true".to_string(), location: Location::from_positions(2, 5, 2, 9), language: "en".to_string() },
            Token { token_type: TokenType::RightParen, lexeme: ")".to_string(), location: Location::from_positions(2, 9, 2, 10), language: "en".to_string() },
            Token { token_type: TokenType::LeftBrace, lexeme: "{".to_string(), location: Location::from_positions(2, 11, 2, 12), language: "en".to_string() },
            Token { token_type: TokenType::Println, lexeme: "println".to_string(), location: Location::from_positions(2, 13, 2, 20), language: "en".to_string() },
            Token { token_type: TokenType::LeftParen, lexeme: "(".to_string(), location: Location::from_positions(2, 20, 2, 21), language: "en".to_string() },
            Token { token_type: TokenType::StringLiteral, lexeme: "then".to_string(), location: Location::from_positions(2, 21, 2, 27), language: "en".to_string() },
            Token { token_type: TokenType::RightParen, lexeme: ")".to_string(), location: Location::from_positions(2, 27, 2, 28), language: "en".to_string() },
            Token { token_type: TokenType::Semicolon, lexeme: ";".to_string(), location: Location::from_positions(2, 28, 2, 29), language: "en".to_string() },
            Token { token_type: TokenType::RightBrace, lexeme: "}".to_string(), location: Location::from_positions(2, 30, 2, 31), language: "en".to_string() },
            Token { token_type: TokenType::RightBrace, lexeme: "}".to_string(), location: Location::from_positions(3,1,3,2), language: "en".to_string() }, // Func body closes
            Token { token_type: TokenType::EOF, lexeme: "".to_string(), location: Location::from_positions(3,2,3,2), language: "en".to_string() },
        ];

        let result_if_in_func = super::parse(tokens_if_in_func);
        assert!(result_if_in_func.is_ok(), "Failed to parse if statement in function: {:?}", result_if_in_func.err());
        let program_if_in_func = result_if_in_func.unwrap();
        assert_eq!(program_if_in_func.declarations.len(), 1);

        if let ast::Declaration::Function(func_decl) = &program_if_in_func.declarations[0] {
            assert_eq!(func_decl.body.statements.len(), 1);
            if let ast::Statement::If(if_stmt) = &func_decl.body.statements[0] {
                if let ast::Expression::Literal(ast::LiteralExpression::Boolean(bool_lit)) = &*if_stmt.condition {
                    assert_eq!(bool_lit.value, true);
                } else {
                    panic!("Expected boolean literal condition");
                }
                assert!(if_stmt.else_branch.is_none());
                // Further checks for then_branch (should be a Block with a Println)
                if let ast::Statement::Block(then_block) = &*if_stmt.then_branch {
                    assert_eq!(then_block.statements.len(), 1);
                    if let ast::Statement::Print(print_stmt) = &then_block.statements[0] {
                         assert_eq!(print_stmt.arguments.len(), 1);
                         if let ast::Expression::Literal(ast::LiteralExpression::String(str_lit)) = &print_stmt.arguments[0] {
                            assert_eq!(str_lit.value, "then");
                        } else {
                            panic!("Expected string literal in println in then_branch");
                        }
                    } else {
                         panic!("Expected Println statement in then_branch block");
                    }
                } else {
                    panic!("Expected Block statement for then_branch");
                }
            } else {
                panic!("Expected If statement in function body");
            }
        } else {
            panic!("Expected Function declaration");
        }

        // Case 2: func testIfElse() { if (false) { println("then"); } else { println("else"); } }
        let tokens_if_else_in_func = vec![
            Token { token_type: TokenType::Func, lexeme: "func".to_string(), location: Location::from_positions(1,1,1,5), language: "en".to_string() },
            Token { token_type: TokenType::Identifier, lexeme: "testIfElse".to_string(), location: Location::from_positions(1,6,1,16), language: "en".to_string() },
            Token { token_type: TokenType::LeftParen, lexeme: "(".to_string(), location: Location::from_positions(1,16,1,17), language: "en".to_string() },
            Token { token_type: TokenType::RightParen, lexeme: ")".to_string(), location: Location::from_positions(1,17,1,18), language: "en".to_string() },
            Token { token_type: TokenType::LeftBrace, lexeme: "{".to_string(), location: Location::from_positions(1,19,1,20), language: "en".to_string() }, // Func body opens
            
            Token { token_type: TokenType::If, lexeme: "if".to_string(), location: Location::from_positions(2, 1, 2, 3), language: "en".to_string() },
            Token { token_type: TokenType::LeftParen, lexeme: "(".to_string(), location: Location::from_positions(2, 4, 2, 5), language: "en".to_string() },
            Token { token_type: TokenType::False, lexeme: "false".to_string(), location: Location::from_positions(2, 5, 2, 10), language: "en".to_string() },
            Token { token_type: TokenType::RightParen, lexeme: ")".to_string(), location: Location::from_positions(2, 10, 2, 11), language: "en".to_string() },
            Token { token_type: TokenType::LeftBrace, lexeme: "{".to_string(), location: Location::from_positions(2, 12, 2, 13), language: "en".to_string() }, // Then branch
            Token { token_type: TokenType::Println, lexeme: "println".to_string(), location: Location::from_positions(2, 14, 2, 21), language: "en".to_string() },
            Token { token_type: TokenType::LeftParen, lexeme: "(".to_string(), location: Location::from_positions(2, 21, 2, 22), language: "en".to_string() },
            Token { token_type: TokenType::StringLiteral, lexeme: "then".to_string(), location: Location::from_positions(2, 22, 2, 28), language: "en".to_string() },
            Token { token_type: TokenType::RightParen, lexeme: ")".to_string(), location: Location::from_positions(2, 28, 2, 29), language: "en".to_string() },
            Token { token_type: TokenType::Semicolon, lexeme: ";".to_string(), location: Location::from_positions(2, 29, 2, 30), language: "en".to_string() },
            Token { token_type: TokenType::RightBrace, lexeme: "}".to_string(), location: Location::from_positions(2, 31, 2, 32), language: "en".to_string() }, // End then branch
            Token { token_type: TokenType::Else, lexeme: "else".to_string(), location: Location::from_positions(2, 33, 2, 37), language: "en".to_string() },
            Token { token_type: TokenType::LeftBrace, lexeme: "{".to_string(), location: Location::from_positions(2, 38, 2, 39), language: "en".to_string() }, // Else branch
            Token { token_type: TokenType::Println, lexeme: "println".to_string(), location: Location::from_positions(2, 40, 2, 47), language: "en".to_string() },
            Token { token_type: TokenType::LeftParen, lexeme: "(".to_string(), location: Location::from_positions(2, 47, 2, 48), language: "en".to_string() },
            Token { token_type: TokenType::StringLiteral, lexeme: "else".to_string(), location: Location::from_positions(2, 48, 2, 54), language: "en".to_string() },
            Token { token_type: TokenType::RightParen, lexeme: ")".to_string(), location: Location::from_positions(2, 54, 2, 55), language: "en".to_string() },
            Token { token_type: TokenType::Semicolon, lexeme: ";".to_string(), location: Location::from_positions(2, 55, 2, 56), language: "en".to_string() },
            Token { token_type: TokenType::RightBrace, lexeme: "}".to_string(), location: Location::from_positions(2, 57, 2, 58), language: "en".to_string() }, // End else branch
            
            Token { token_type: TokenType::RightBrace, lexeme: "}".to_string(), location: Location::from_positions(3,1,3,2), language: "en".to_string() }, // Func body closes
            Token { token_type: TokenType::EOF, lexeme: "".to_string(), location: Location::from_positions(3,2,3,2), language: "en".to_string() },
        ];

        let result_if_else_in_func = super::parse(tokens_if_else_in_func);
        assert!(result_if_else_in_func.is_ok(), "Failed to parse if-else statement in function: {:?}", result_if_else_in_func.err());
        let program_if_else_in_func = result_if_else_in_func.unwrap();
        assert_eq!(program_if_else_in_func.declarations.len(), 1);

        if let ast::Declaration::Function(func_decl) = &program_if_else_in_func.declarations[0] {
            assert_eq!(func_decl.body.statements.len(), 1);
            if let ast::Statement::If(if_stmt) = &func_decl.body.statements[0] {
                if let ast::Expression::Literal(ast::LiteralExpression::Boolean(bool_lit)) = &*if_stmt.condition {
                    assert_eq!(bool_lit.value, false);
                } else {
                    panic!("Expected boolean literal condition for if-else");
                }
                // Check then_branch
                if let ast::Statement::Block(then_block) = &*if_stmt.then_branch {
                    assert_eq!(then_block.statements.len(), 1);
                    // ... detailed check for then_block's println("then") ...
                } else {
                    panic!("Expected Block statement for then_branch in if-else");
                }
                // Check else_branch
                assert!(if_stmt.else_branch.is_some());
                if let Some(else_branch_stmt) = &if_stmt.else_branch {
                    if let ast::Statement::Block(else_block) = &**else_branch_stmt {
                        assert_eq!(else_block.statements.len(), 1);
                        if let ast::Statement::Print(print_stmt) = &else_block.statements[0] {
                            assert_eq!(print_stmt.arguments.len(), 1);
                            if let ast::Expression::Literal(ast::LiteralExpression::String(str_lit)) = &print_stmt.arguments[0] {
                                assert_eq!(str_lit.value, "else");
                            } else {
                                panic!("Expected string literal in println in else_branch");
                            }
                        } else {
                            panic!("Expected Println statement in else_branch block");
                        }
                    } else {
                        panic!("Expected Block statement for else_branch");
                    }
                } else {
                    panic!("else_branch was None unexpectedly");
                }
            } else {
                panic!("Expected If statement in function body for if-else");
            }
        } else {
            panic!("Expected Function declaration for if-else test");
        }
    }

    #[test]
    fn test_while_statements() {
        // Test case: func testWhile() { while (true) { println("loop"); } }
        let tokens_while_in_func = vec![
            Token { token_type: TokenType::Func, lexeme: "func".to_string(), location: Location::from_positions(1,1,1,5), language: "en".to_string() },
            Token { token_type: TokenType::Identifier, lexeme: "testWhile".to_string(), location: Location::from_positions(1,6,1,15), language: "en".to_string() },
            Token { token_type: TokenType::LeftParen, lexeme: "(".to_string(), location: Location::from_positions(1,15,1,16), language: "en".to_string() },
            Token { token_type: TokenType::RightParen, lexeme: ")".to_string(), location: Location::from_positions(1,16,1,17), language: "en".to_string() },
            Token { token_type: TokenType::LeftBrace, lexeme: "{".to_string(), location: Location::from_positions(1,18,1,19), language: "en".to_string() }, // Func body opens
            
            Token { token_type: TokenType::While, lexeme: "while".to_string(), location: Location::from_positions(2, 5, 2, 10), language: "en".to_string() },
            Token { token_type: TokenType::LeftParen, lexeme: "(".to_string(), location: Location::from_positions(2, 11, 2, 12), language: "en".to_string() },
            Token { token_type: TokenType::True, lexeme: "true".to_string(), location: Location::from_positions(2, 12, 2, 16), language: "en".to_string() },
            Token { token_type: TokenType::RightParen, lexeme: ")".to_string(), location: Location::from_positions(2, 16, 2, 17), language: "en".to_string() },
            Token { token_type: TokenType::LeftBrace, lexeme: "{".to_string(), location: Location::from_positions(2, 18, 2, 19), language: "en".to_string() }, // While body opens
            Token { token_type: TokenType::Println, lexeme: "println".to_string(), location: Location::from_positions(2, 20, 2, 27), language: "en".to_string() },
            Token { token_type: TokenType::LeftParen, lexeme: "(".to_string(), location: Location::from_positions(2, 27, 2, 28), language: "en".to_string() },
            Token { token_type: TokenType::StringLiteral, lexeme: "loop".to_string(), location: Location::from_positions(2, 28, 2, 34), language: "en".to_string() },
            Token { token_type: TokenType::RightParen, lexeme: ")".to_string(), location: Location::from_positions(2, 34, 2, 35), language: "en".to_string() },
            Token { token_type: TokenType::Semicolon, lexeme: ";".to_string(), location: Location::from_positions(2, 35, 2, 36), language: "en".to_string() },
            Token { token_type: TokenType::RightBrace, lexeme: "}".to_string(), location: Location::from_positions(2, 37, 2, 38), language: "en".to_string() }, // While body closes
            
            Token { token_type: TokenType::RightBrace, lexeme: "}".to_string(), location: Location::from_positions(3,1,3,2), language: "en".to_string() }, // Func body closes
            Token { token_type: TokenType::EOF, lexeme: "".to_string(), location: Location::from_positions(3,2,3,2), language: "en".to_string() },
        ];

        let result_while = super::parse(tokens_while_in_func);
        assert!(result_while.is_ok(), "Failed to parse while statement in function: {:?}", result_while.err());
        let program_while = result_while.unwrap();
        assert_eq!(program_while.declarations.len(), 1);

        if let ast::Declaration::Function(func_decl) = &program_while.declarations[0] {
            assert_eq!(func_decl.body.statements.len(), 1);
            if let ast::Statement::While(while_stmt) = &func_decl.body.statements[0] {
                // Check condition
                if let ast::Expression::Literal(ast::LiteralExpression::Boolean(bool_lit)) = &*while_stmt.condition {
                    assert_eq!(bool_lit.value, true);
                } else {
                    panic!("Expected boolean literal condition for while loop");
                }
                // Check body (should be a Block with a Println)
                if let ast::Statement::Block(body_block) = &*while_stmt.body {
                    assert_eq!(body_block.statements.len(), 1);
                    if let ast::Statement::Print(print_stmt) = &body_block.statements[0] {
                         assert_eq!(print_stmt.arguments.len(), 1);
                         if let ast::Expression::Literal(ast::LiteralExpression::String(str_lit)) = &print_stmt.arguments[0] {
                            assert_eq!(str_lit.value, "loop");
                        } else {
                            panic!("Expected string literal in println in while loop body");
                        }
                    } else {
                         panic!("Expected Println statement in while loop body block");
                    }
                } else {
                    panic!("Expected Block statement for while loop body");
                }
            } else {
                panic!("Expected While statement in function body");
            }
        } else {
            panic!("Expected Function declaration for while test");
        }
    }

    #[test]
    fn test_return_statements() {
        // Case 1: func testReturnVoid() { return; }
        let tokens_return_void = vec![
            Token { token_type: TokenType::Func, lexeme: "func".to_string(), location: Location::from_positions(1,1,1,5), language: "en".to_string() },
            Token { token_type: TokenType::Identifier, lexeme: "testReturnVoid".to_string(), location: Location::from_positions(1,6,1,20), language: "en".to_string() },
            Token { token_type: TokenType::LeftParen, lexeme: "(".to_string(), location: Location::from_positions(1,20,1,21), language: "en".to_string() },
            Token { token_type: TokenType::RightParen, lexeme: ")".to_string(), location: Location::from_positions(1,21,1,22), language: "en".to_string() },
            Token { token_type: TokenType::LeftBrace, lexeme: "{".to_string(), location: Location::from_positions(1,23,1,24), language: "en".to_string() },
            Token { token_type: TokenType::Return, lexeme: "return".to_string(), location: Location::from_positions(1,25,1,31), language: "en".to_string() },
            Token { token_type: TokenType::Semicolon, lexeme: ";".to_string(), location: Location::from_positions(1,31,1,32), language: "en".to_string() },
            Token { token_type: TokenType::RightBrace, lexeme: "}".to_string(), location: Location::from_positions(1,33,1,34), language: "en".to_string() },
            Token { token_type: TokenType::EOF, lexeme: "".to_string(), location: Location::from_positions(1,34,1,34), language: "en".to_string() },
        ];

        let result_return_void = super::parse(tokens_return_void);
        assert!(result_return_void.is_ok(), "Failed to parse 'return;' in function: {:?}", result_return_void.err());
        let program_return_void = result_return_void.unwrap();
        assert_eq!(program_return_void.declarations.len(), 1);

        if let ast::Declaration::Function(func_decl) = &program_return_void.declarations[0] {
            assert_eq!(func_decl.body.statements.len(), 1);
            if let ast::Statement::Return(return_stmt) = &func_decl.body.statements[0] {
                assert!(return_stmt.value.is_none(), "Expected no value for 'return;' statement");
            } else {
                panic!("Expected Return statement in function body for void return");
            }
        } else {
            panic!("Expected Function declaration for void return test");
        }

        // Case 2: func testReturnWithValue() { return 42; }
        let tokens_return_with_value = vec![
            Token { token_type: TokenType::Func, lexeme: "func".to_string(), location: Location::from_positions(1,1,1,5), language: "en".to_string() },
            Token { token_type: TokenType::Identifier, lexeme: "testReturnWithValue".to_string(), location: Location::from_positions(1,6,1,25), language: "en".to_string() },
            Token { token_type: TokenType::LeftParen, lexeme: "(".to_string(), location: Location::from_positions(1,25,1,26), language: "en".to_string() },
            Token { token_type: TokenType::RightParen, lexeme: ")".to_string(), location: Location::from_positions(1,26,1,27), language: "en".to_string() },
            Token { token_type: TokenType::LeftBrace, lexeme: "{".to_string(), location: Location::from_positions(1,28,1,29), language: "en".to_string() },
            Token { token_type: TokenType::Return, lexeme: "return".to_string(), location: Location::from_positions(1,30,1,36), language: "en".to_string() },
            Token { token_type: TokenType::IntLiteral, lexeme: "42".to_string(), location: Location::from_positions(1,37,1,39), language: "en".to_string() },
            Token { token_type: TokenType::Semicolon, lexeme: ";".to_string(), location: Location::from_positions(1,39,1,40), language: "en".to_string() },
            Token { token_type: TokenType::RightBrace, lexeme: "}".to_string(), location: Location::from_positions(1,41,1,42), language: "en".to_string() },
            Token { token_type: TokenType::EOF, lexeme: "".to_string(), location: Location::from_positions(1,42,1,42), language: "en".to_string() },
        ];

        let result_return_with_value = super::parse(tokens_return_with_value);
        assert!(result_return_with_value.is_ok(), "Failed to parse 'return 42;' in function: {:?}", result_return_with_value.err());
        let program_return_with_value = result_return_with_value.unwrap();
        assert_eq!(program_return_with_value.declarations.len(), 1);

        if let ast::Declaration::Function(func_decl) = &program_return_with_value.declarations[0] {
            assert_eq!(func_decl.body.statements.len(), 1);
            if let ast::Statement::Return(return_stmt) = &func_decl.body.statements[0] {
                assert!(return_stmt.value.is_some(), "Expected a value for 'return 42;' statement");
                if let Some(expr) = &return_stmt.value {
                    if let ast::Expression::Literal(ast::LiteralExpression::Number(num_lit)) = &**expr {
                        assert_eq!(num_lit.value, "42");
                    } else {
                        panic!("Expected number literal for return value");
                    }
                } else {
                    panic!("Value is None, but should be Some"); // Should be caught by is_some()
                }
            } else {
                panic!("Expected Return statement in function body for return with value");
            }
        } else {
            panic!("Expected Function declaration for return with value test");
        }
    }

    #[test]
    fn test_block_statements() {
        // Test case: func testBlock() { { val x: int = 1; println(x); } }
        let tokens_block_in_func = vec![
            Token { token_type: TokenType::Func, lexeme: "func".to_string(), location: Location::from_positions(1,1,1,5), language: "en".to_string() },
            Token { token_type: TokenType::Identifier, lexeme: "testBlock".to_string(), location: Location::from_positions(1,6,1,15), language: "en".to_string() },
            Token { token_type: TokenType::LeftParen, lexeme: "(".to_string(), location: Location::from_positions(1,15,1,16), language: "en".to_string() },
            Token { token_type: TokenType::RightParen, lexeme: ")".to_string(), location: Location::from_positions(1,16,1,17), language: "en".to_string() },
            Token { token_type: TokenType::LeftBrace, lexeme: "{".to_string(), location: Location::from_positions(1,18,1,19), language: "en".to_string() }, // Func body opens
            
            // Start of the actual block statement
            Token { token_type: TokenType::LeftBrace, lexeme: "{".to_string(), location: Location::from_positions(2, 5, 2, 6), language: "en".to_string() }, // Block opens
            // val x: int = 1;
            Token { token_type: TokenType::Val, lexeme: "val".to_string(), location: Location::from_positions(2,7,2,10), language: "en".to_string() },
            Token { token_type: TokenType::Identifier, lexeme: "x".to_string(), location: Location::from_positions(2,11,2,12), language: "en".to_string() },
            Token { token_type: TokenType::Colon, lexeme: ":".to_string(), location: Location::from_positions(2,12,2,13), language: "en".to_string() },
            Token { token_type: TokenType::Identifier, lexeme: "int".to_string(), location: Location::from_positions(2,14,2,17), language: "en".to_string() }, // Type 'int'
            Token { token_type: TokenType::Assign, lexeme: "=".to_string(), location: Location::from_positions(2,18,2,19), language: "en".to_string() },
            Token { token_type: TokenType::IntLiteral, lexeme: "1".to_string(), location: Location::from_positions(2,20,2,21), language: "en".to_string() },
            Token { token_type: TokenType::Semicolon, lexeme: ";".to_string(), location: Location::from_positions(2,21,2,22), language: "en".to_string() },
            // println(x);
            Token { token_type: TokenType::Println, lexeme: "println".to_string(), location: Location::from_positions(2,23,2,30), language: "en".to_string() },
            Token { token_type: TokenType::LeftParen, lexeme: "(".to_string(), location: Location::from_positions(2,30,2,31), language: "en".to_string() },
            Token { token_type: TokenType::Identifier, lexeme: "x".to_string(), location: Location::from_positions(2,31,2,32), language: "en".to_string() },
            Token { token_type: TokenType::RightParen, lexeme: ")".to_string(), location: Location::from_positions(2,32,2,33), language: "en".to_string() },
            Token { token_type: TokenType::Semicolon, lexeme: ";".to_string(), location: Location::from_positions(2,33,2,34), language: "en".to_string() },
            Token { token_type: TokenType::RightBrace, lexeme: "}".to_string(), location: Location::from_positions(2,35,2,36), language: "en".to_string() }, // Block closes
            
            Token { token_type: TokenType::RightBrace, lexeme: "}".to_string(), location: Location::from_positions(3,1,3,2), language: "en".to_string() }, // Func body closes
            Token { token_type: TokenType::EOF, lexeme: "".to_string(), location: Location::from_positions(3,2,3,2), language: "en".to_string() },
        ];

        let result_block = super::parse(tokens_block_in_func);
        assert!(result_block.is_ok(), "Failed to parse block statement in function: {:?}", result_block.err());
        let program_block = result_block.unwrap();
        assert_eq!(program_block.declarations.len(), 1);

        if let ast::Declaration::Function(func_decl) = &program_block.declarations[0] {
            assert_eq!(func_decl.name, "testBlock");
            // Now expecting 3 statements: var x = 0; var y = 0; x = y;
            assert_eq!(func_decl.body.statements.len(), 1, "Function body should contain one statement (the block statement)");
            
            // func_decl.body.statements[0] is the ast::Statement::Block for { val x: int = 1; println(x); }
            if let ast::Statement::Block(the_block_in_function_body) = &func_decl.body.statements[0] {
                assert_eq!(the_block_in_function_body.statements.len(), 2, "The block in function body should contain two statements");

                // Check 1st statement in the_block_in_function_body: val x: int = 1;
                if let ast::Statement::DeclarationStatement(decl_stmt) = &the_block_in_function_body.statements[0] {
                    if let ast::Declaration::Variable(var_decl) = decl_stmt {
                        assert_eq!(var_decl.name, "x");
                        assert!(!var_decl.is_mutable);
                        if let Some(ast::Type::Simple(type_name)) = &var_decl.var_type {
                            assert_eq!(type_name, "int");
                        } else {
                            panic!("Expected Simple type 'int' for variable x");
                        }
                        if let ast::Expression::Literal(ast::LiteralExpression::Number(num_lit)) = &*var_decl.initializer {
                            assert_eq!(num_lit.value, "1");
                        } else {
                            panic!("Expected number literal 1 for initializer");
                        }
                    } else {
                        panic!("Expected VariableDeclaration within DeclarationStatement as the first statement in the block");
                    }
                } else {
                    panic!("Expected DeclarationStatement as first statement in block, got {:?}", the_block_in_function_body.statements[0]);
                }

                // Check 2nd statement in the_block_in_function_body: println(x);
                if let ast::Statement::Print(print_stmt) = &the_block_in_function_body.statements[1] {
                    assert_eq!(print_stmt.arguments.len(), 1);
                    if let ast::Expression::Identifier(ident_expr) = &print_stmt.arguments[0] {
                        assert_eq!(ident_expr.name, "x");
                    } else {
                        panic!("Expected identifier 'x' as argument to println");
                    }
                } else {
                    panic!("Expected PrintStatement as second statement in block, got {:?}", the_block_in_function_body.statements[1]);
                }
            } else {
                panic!("Expected function body's first statement to be a block statement, got {:?}", func_decl.body.statements[0]);
            }
        } else {
            panic!("Expected Function declaration for block test");
        }
    }

    #[test]
    fn test_expression_statements() {
        // Test case 1: Assignment expression statement
        let source_assignment = "func testAssignment() { var x: int = 0; var y: int = 0; x = y; }";
        let tokens_assignment = tokenize_and_filter(source_assignment);

        let result_assignment = super::parse(tokens_assignment);
        assert!(result_assignment.is_ok(), "Failed to parse assignment test: {:?}", result_assignment.err());
        let program_assignment = result_assignment.unwrap();
        assert_eq!(program_assignment.declarations.len(), 1);

        if let ast::Declaration::Function(func_decl) = &program_assignment.declarations[0] {
            assert_eq!(func_decl.name, "testAssignment");
            // Now expecting 3 statements: var x = 0; var y = 0; x = y;
            assert_eq!(func_decl.body.statements.len(), 3, "Function body should have three statements");

            // Check the third statement is the assignment x = y;
            if let ast::Statement::Expression(expr_stmt) = &func_decl.body.statements[2] { // Check 3rd statement
                if let ast::Expression::Assignment(assign_expr) = &*expr_stmt.expression { // Dereference Box
                    assert_eq!(assign_expr.name, "x");
                    
                    if let ast::Expression::Identifier(value_ident) = &*assign_expr.value { // Dereference Box for value
                        assert_eq!(value_ident.name, "y");
                    } else {
                        panic!("Expected identifier 'y' as assignment value");
                    }
                } else {
                    panic!("Expected AssignmentExpression within ExpressionStatement");
                }
            } else {
                panic!("Expected ExpressionStatement as the third statement in function body, got {:?}", func_decl.body.statements[2]);
            }
        } else {
            panic!("Expected FunctionDeclaration for assignment test");
        }

        // Test case 2: Function call expression statement
        // func testCall() { foo(); }
        let tokens_call = tokenize_and_filter(
            "func testCall() { foo(); }"
        );
        let program_call = super::parse(tokens_call).unwrap_or_else(|e| panic!("Failed to parse call test: {:?}", e));
        assert_eq!(program_call.declarations.len(), 1);

        if let ast::Declaration::Function(func_decl) = &program_call.declarations[0] {
            assert_eq!(func_decl.name, "testCall");
            assert_eq!(func_decl.body.statements.len(), 1, "Function body should have one expression statement for call");

            if let ast::Statement::Expression(expr_stmt_call) = &func_decl.body.statements[0] {
                if let ast::Expression::Call(call_expr) = &*expr_stmt_call.expression {
                    assert_eq!(call_expr.callee, "foo");
                    assert!(call_expr.arguments.is_empty(), "Call to foo should have no arguments");
                } else {
                    panic!("Expected CallExpression within ExpressionStatement");
                }
            } else {
                panic!("Expected ExpressionStatement for call in function body, got {:?}", func_decl.body.statements[0]);
            }
        } else {
            panic!("Expected FunctionDeclaration for call test");
        }
    }

}


