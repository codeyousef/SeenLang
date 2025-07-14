use crate::ast::*;
use seen_lexer::token::{Location, Position, Token, TokenType};

/// Errors that can occur during parsing
#[derive(Debug, thiserror::Error)]
pub enum ParserError {
    #[error("Unexpected token: expected {expected:?}, got {got:?} at {position}")]
    UnexpectedToken {
        expected: String,
        got: String,
        position: Position,
    },

    #[error("Expected expression at {position}")]
    ExpectedExpression { position: Position },

    #[error("Expected identifier at {position}")]
    ExpectedIdentifier { position: Position },

    #[error("Expected statement at {position}")]
    ExpectedStatement { position: Position },

    #[error("Expected type at {position}")]
    ExpectedType { position: Position },
}

/// Parser for the Seen language
pub struct Parser {
    /// Tokens to parse
    tokens: Vec<Token>,
    /// Current position in the token stream
    current: usize,
}

impl Parser {
    /// Create a new parser with the given tokens
    pub fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            current: 0,
        }
    }

    /// Parse the tokens into a Program AST
    pub fn parse(&mut self) -> Result<Program, ParserError> {
        let start_pos = self.peek_position();
        let mut declarations = Vec::new();

        while !self.is_at_end() {
            declarations.push(self.declaration()?);
        }

        let end_pos = if self.current > 0 && !self.tokens.is_empty() {
            self.previous().location.end
        } else {
            start_pos
        };

        Ok(Program {
            declarations,
            location: Location::new(start_pos, end_pos),
        })
    }

    /// Parse a single expression from tokens
    pub fn parse_expression(&mut self) -> Result<Expression, ParserError> {
        self.expression()
    }

    /// Parse a single statement from tokens
    pub fn parse_statement(&mut self) -> Result<Statement, ParserError> {
        self.statement()
    }

    /// Parse a declaration
    fn declaration(&mut self) -> Result<Declaration, ParserError> {
        if self.match_tokens(&[TokenType::Func]) {
            // Function declaration
            self.function_declaration().map(Declaration::Function)
        } else if self.match_tokens(&[TokenType::Val, TokenType::Var]) {
            // Variable declaration
            self.variable_declaration().map(Declaration::Variable)
        } else if self.match_tokens(&[TokenType::Struct]) {
            // Struct declaration
            self.struct_declaration().map(Declaration::Struct)
        } else if self.match_tokens(&[TokenType::Enum]) {
            // Enum declaration
            self.enum_declaration().map(Declaration::Enum)
        } else {
            // If it's not a declaration, try parsing it as a statement expression
            let _stmt = self.statement()?;
            // But this is a syntax error in our language - declarations and statements
            // should be separate at the top level.
            // For now, we'll just return a dummy variable declaration for error recovery
            Err(ParserError::ExpectedStatement { position: self.peek_position() })
        }
    }

    /// Parse a function declaration
    fn function_declaration(&mut self) -> Result<FunctionDeclaration, ParserError> {
        let func_keyword_start_pos = self.previous().location.start; // 'func' token location

        // Function name
        let name = self.consume_identifier("Expected function name")?;

        // Optional generic type parameters <T, U, ...>
        let mut type_parameters = Vec::new();
        if self.match_tokens(&[TokenType::LessThan]) {
            // Parse type parameters
            type_parameters.push(self.consume_identifier("Expected type parameter name")?);
            
            while self.match_tokens(&[TokenType::Comma]) {
                type_parameters.push(self.consume_identifier("Expected type parameter name")?);
            }
            
            self.consume(TokenType::GreaterThan, "Expected '>' after type parameters")?;
        }

        // Parameter list
        self.consume(TokenType::LeftParen, "Expected '(' after function name")?;
        let parameters = if !self.check(TokenType::RightParen) {
            self.parameter_list()?
        } else {
            Vec::new()
        };
        self.consume(TokenType::RightParen, "Expected ')' after parameters")?;

        // Return type
        let return_type = if self.match_tokens(&[TokenType::Arrow]) {
            Some(self.parse_type()?)
        } else {
            None
        };

        // Function body
        self.consume(TokenType::LeftBrace, "Expected '{' before function body")?;
        let body = self.block()?;
        let func_end_pos = body.location.end;

        Ok(FunctionDeclaration {
            name,
            type_parameters,
            parameters,
            return_type,
            body,
            location: Location::new(func_keyword_start_pos, func_end_pos),
        })
    }

    /// Parse a parameter list
    fn parameter_list(&mut self) -> Result<Vec<Parameter>, ParserError> {
        let mut parameters = Vec::new();

        loop {
            let name = self.consume_identifier("Expected parameter name")?;
            self.consume(TokenType::Colon, "Expected ':' after parameter name")?;
            let param_type = self.parse_type()?;

            parameters.push(Parameter {
                name,
                param_type,
                location: Location::new(self.peek_position(), self.peek_position()), // Could be improved to track actual location
            });

            if !self.match_tokens(&[TokenType::Comma]) {
                break;
            }
        }

        Ok(parameters)
    }

    /// Parse a variable declaration
    fn variable_declaration(&mut self) -> Result<VariableDeclaration, ParserError> {
        let start_pos = self.previous().location.start;
        let is_mutable = self.previous().token_type == TokenType::Var;

        let name = self.consume_identifier("Expected variable name")?;

        // Optional type annotation
        let var_type = if self.match_tokens(&[TokenType::Colon]) {
            Some(self.parse_type()?)
        } else {
            None
        };

        self.consume(TokenType::Assign, "Expected '=' after variable name")?;
        let initializer = Box::new(self.expression()?);
        self.consume(TokenType::Semicolon, "Expected ';' after variable declaration")?;

        let end_pos = self.previous().location.end;

        Ok(VariableDeclaration {
            is_mutable,
            name,
            var_type,
            initializer,
            location: Location::new(start_pos, end_pos),
        })
    }

    /// Parse a struct declaration
    fn struct_declaration(&mut self) -> Result<StructDeclaration, ParserError> {
        let start_pos = self.previous().location.start; // 'struct' token location

        // Struct name
        let name = self.consume_identifier("Expected struct name")?;

        // Optional generic type parameters <T, U, ...>
        let mut type_parameters = Vec::new();
        if self.match_tokens(&[TokenType::LessThan]) {
            // Parse type parameters
            type_parameters.push(self.consume_identifier("Expected type parameter name")?);
            
            while self.match_tokens(&[TokenType::Comma]) {
                type_parameters.push(self.consume_identifier("Expected type parameter name")?);
            }
            
            self.consume(TokenType::GreaterThan, "Expected '>' after type parameters")?;
        }

        // Opening brace
        self.consume(TokenType::LeftBrace, "Expected '{' after struct name")?;

        // Parse fields
        let mut fields = Vec::new();
        let mut field_names = std::collections::HashSet::new();

        while !self.check(TokenType::RightBrace) && !self.is_at_end() {
            let field = self.struct_field()?;

            // Check for duplicate field names
            if !field_names.insert(field.name.clone()) {
                return Err(ParserError::UnexpectedToken {
                    expected: "unique field name".to_string(),
                    got: format!("duplicate field '{}'", field.name),
                    position: field.location.start,
                });
            }

            fields.push(field);

            // Handle optional comma
            if self.match_tokens(&[TokenType::Comma]) {
                // Consumed comma, continue
            } else if !self.check(TokenType::RightBrace) {
                return Err(ParserError::UnexpectedToken {
                    expected: "',' or '}'".to_string(),
                    got: self.peek().lexeme.clone(),
                    position: self.peek_position(),
                });
            }
        }

        // Closing brace
        self.consume(TokenType::RightBrace, "Expected '}' after struct fields")?;
        let end_pos = self.previous().location.end;

        Ok(StructDeclaration {
            name,
            type_parameters,
            fields,
            location: Location::new(start_pos, end_pos),
        })
    }

    /// Parse an enum declaration
    fn enum_declaration(&mut self) -> Result<EnumDeclaration, ParserError> {
        let enum_keyword_start_pos = self.previous().location.start; // 'enum' token location

        // Enum name
        let name = self.consume_identifier("Expected enum name")?;

        // Optional generic parameters <T, E, ...>
        let mut type_parameters = Vec::new();
        if self.match_tokens(&[TokenType::LessThan]) {
            // Parse type parameters
            type_parameters.push(self.consume_identifier("Expected type parameter name")?);
            
            while self.match_tokens(&[TokenType::Comma]) {
                type_parameters.push(self.consume_identifier("Expected type parameter name")?);
            }
            
            self.consume(TokenType::GreaterThan, "Expected '>' after type parameters")?;
        }

        // Opening brace
        self.consume(TokenType::LeftBrace, "Expected '{' after enum name")?;

        // Parse enum variants
        let mut variants = Vec::new();
        let mut variant_names = std::collections::HashSet::new();

        while !self.check(TokenType::RightBrace) && !self.is_at_end() {
            let variant = self.enum_variant()?;

            // Check for duplicate variant names
            if !variant_names.insert(variant.name.clone()) {
                return Err(ParserError::UnexpectedToken {
                    expected: "unique variant name".to_string(),
                    got: format!("duplicate variant '{}'", variant.name),
                    position: variant.location.start,
                });
            }

            variants.push(variant);

            // Handle optional comma
            if self.match_tokens(&[TokenType::Comma]) {
                // Consumed comma, continue
            } else if !self.check(TokenType::RightBrace) {
                return Err(ParserError::UnexpectedToken {
                    expected: "',' or '}'".to_string(),
                    got: self.peek().lexeme.clone(),
                    position: self.peek_position(),
                });
            }
        }

        // Closing brace
        self.consume(TokenType::RightBrace, "Expected '}' after enum variants")?;
        let end_pos = self.previous().location.end;

        Ok(EnumDeclaration {
            name,
            type_parameters,
            variants,
            location: Location::new(enum_keyword_start_pos, end_pos),
        })
    }

    /// Parse an enum variant
    fn enum_variant(&mut self) -> Result<EnumVariant, ParserError> {
        let start_pos = self.peek_position();

        // Variant name
        let name = self.consume_identifier("Expected variant name")?;

        // Optional data types (for variants with data)
        let mut data = None;
        if self.match_tokens(&[TokenType::LeftParen]) {
            let mut types = Vec::new();

            if !self.check(TokenType::RightParen) {
                loop {
                    types.push(self.parse_type()?);

                    if !self.match_tokens(&[TokenType::Comma]) {
                        break;
                    }
                }
            }

            self.consume(TokenType::RightParen, "Expected ')' after variant data types")?;
            data = Some(types);
        }

        let end_pos = self.previous().location.end;

        Ok(EnumVariant {
            name,
            data,
            location: Location::new(start_pos, end_pos),
        })
    }

    /// Parse an enum literal expression (EnumName.Variant or EnumName.Variant(args))
    fn enum_literal(&mut self, enum_name: String, start_loc: Position) -> Result<Expression, ParserError> {
        self.enum_literal_with_generics(enum_name, None, start_loc)
    }

    /// Parse an enum literal expression with optional generic type arguments
    fn enum_literal_with_generics(&mut self, enum_name: String, type_arguments: Option<Vec<Type>>, start_loc: Position) -> Result<Expression, ParserError> {
        // Consume the dot
        self.consume(TokenType::Dot, "Expected '.' after enum name")?;

        // Get the variant name
        let variant_name = self.consume_identifier("Expected variant name after '.'")?;

        // Check for arguments (for variants with data)
        let mut arguments = None;
        if self.match_tokens(&[TokenType::LeftParen]) {
            let mut args = Vec::new();

            if !self.check(TokenType::RightParen) {
                loop {
                    args.push(self.expression()?);

                    if !self.match_tokens(&[TokenType::Comma]) {
                        break;
                    }
                }
            }

            self.consume(TokenType::RightParen, "Expected ')' after enum variant arguments")?;
            arguments = Some(args);
        }

        let end_loc = self.previous().location.end;

        Ok(Expression::EnumLiteral(EnumLiteralExpression {
            enum_name,
            type_arguments,
            variant_name,
            arguments,
            location: Location::new(start_loc, end_loc),
        }))
    }

    /// Parse a struct literal expression
    fn struct_literal(&mut self, struct_name: String, start_loc: Position) -> Result<Expression, ParserError> {
        self.consume(TokenType::LeftBrace, "Expected '{' after struct name")?;

        let mut fields = Vec::new();

        while !self.check(TokenType::RightBrace) && !self.is_at_end() {
            let field_start = self.peek_position();

            // Parse field name
            let field_name = self.consume_identifier("Expected field name")?;

            // Consume colon
            self.consume(TokenType::Colon, "Expected ':' after field name")?;

            // Parse field value
            let value = Box::new(self.expression()?);
            let field_end = self.previous().location.end;

            fields.push(StructFieldInit {
                field_name,
                value,
                location: Location::new(field_start, field_end),
            });

            // Handle optional comma
            if self.match_tokens(&[TokenType::Comma]) {
                // Consumed comma, continue
            } else if !self.check(TokenType::RightBrace) {
                return Err(ParserError::UnexpectedToken {
                    expected: "',' or '}'".to_string(),
                    got: self.peek().lexeme.clone(),
                    position: self.peek_position(),
                });
            }
        }

        self.consume(TokenType::RightBrace, "Expected '}' after struct fields")?;
        let end_loc = self.previous().location.end;

        Ok(Expression::StructLiteral(StructLiteralExpression {
            struct_name,
            fields,
            location: Location::new(start_loc, end_loc),
        }))
    }

    /// Parse a struct field
    fn struct_field(&mut self) -> Result<StructField, ParserError> {
        let start_pos = self.peek_position();

        // Field name
        let name = self.consume_identifier("Expected field name")?;

        // Colon
        self.consume(TokenType::Colon, "Expected ':' after field name")?;

        // Field type
        let field_type = self.parse_type()?;
        let end_pos = self.previous().location.end;

        Ok(StructField {
            name,
            field_type,
            location: Location::new(start_pos, end_pos),
        })
    }

    /// Parse a type annotation
    fn parse_type(&mut self) -> Result<Type, ParserError> {
        // Check for pointer type syntax: *Type
        if self.match_tokens(&[TokenType::Multiply]) {
            let inner_type = Box::new(self.parse_type()?);
            return Ok(Type::Pointer(inner_type));
        }

        // Check for array type syntax: [Type]
        if self.match_tokens(&[TokenType::LeftBracket]) {
            let inner_type = Box::new(self.parse_type()?);
            self.consume(TokenType::RightBracket, "Expected ']' after array type")?;
            return Ok(Type::Array(inner_type));
        }

        if self.check(TokenType::Identifier) {
            let type_name = self.advance().lexeme.clone();

            // Check for generic/array types
            if self.match_tokens(&[TokenType::LessThan]) {
                let mut type_args = Vec::new();
                
                // Parse first type argument
                type_args.push(self.parse_type()?);
                
                // Parse additional type arguments separated by commas
                while self.match_tokens(&[TokenType::Comma]) {
                    type_args.push(self.parse_type()?);
                }
                
                self.consume(TokenType::GreaterThan, "Expected '>' after type parameters")?;

                match type_name.as_str() {
                    "Array" => {
                        // Arrays have only one type parameter
                        if type_args.len() != 1 {
                            return Err(ParserError::UnexpectedToken {
                                expected: "exactly one type parameter for Array".to_string(),
                                got: format!("{} type parameters", type_args.len()),
                                position: self.peek_position(),
                            });
                        }
                        Ok(Type::Array(Box::new(type_args.into_iter().next().unwrap())))
                    },
                    _ => {
                        // Generic types like Option<T>, Result<T, E>, etc.
                        Ok(Type::Generic(type_name, type_args))
                    }
                }
            } else if self.match_tokens(&[TokenType::Question]) {
                // Optional type
                Ok(Type::Simple(format!("{}?", type_name)))
            } else {
                // Simple type
                Ok(Type::Simple(type_name))
            }
        } else if self.match_tokens(&[TokenType::LeftParen]) {
            // Function type
            let mut param_types = Vec::new();

            if !self.check(TokenType::RightParen) {
                loop {
                    param_types.push(self.parse_type()?);
                    if !self.match_tokens(&[TokenType::Comma]) {
                        break;
                    }
                }
            }

            self.consume(TokenType::RightParen, "Expected ')' in function type")?;
            self.consume(TokenType::Arrow, "Expected '->' in function type")?;
            let _return_type = Box::new(self.parse_type()?);

            // Function types don't exist in AST, so represent as string for now
            Ok(Type::Simple("Function".to_string()))
        } else {
            Err(ParserError::ExpectedType { position: self.peek_position() })
        }
    }

    /// Parse a statement
    fn statement(&mut self) -> Result<Statement, ParserError> {
        if self.match_tokens(&[TokenType::Val, TokenType::Var]) {
            // This is a variable declaration statement
            // We need to capture the start location before consuming 'val'/'var'
            // However, variable_declaration itself handles its full location.
            // The previous token ('val' or 'var') is already consumed by match_tokens.
            let var_decl = self.variable_declaration()?;
            Ok(Statement::DeclarationStatement(Declaration::Variable(var_decl)))
        } else if self.match_tokens(&[TokenType::LeftBrace]) {
            // Block statement
            self.block().map(Statement::Block)
        } else if self.match_tokens(&[TokenType::Return]) {
            // Return statement
            self.return_statement().map(Statement::Return)
        } else if self.match_tokens(&[TokenType::If]) {
            // If statement
            self.if_statement().map(Statement::If)
        } else if self.match_tokens(&[TokenType::While]) {
            // While statement
            self.while_statement().map(Statement::While)
        } else if self.match_tokens(&[TokenType::For]) {
            // For statement
            self.for_statement().map(Statement::For)
        } else if self.match_tokens(&[TokenType::Match]) {
            // Match statement
            self.match_statement().map(Statement::Match)
        } else if self.match_tokens(&[TokenType::Func]) {
            // Nested function declaration
            self.function_declaration().map(|f| Statement::DeclarationStatement(Declaration::Function(f)))
        } else if self.match_tokens(&[TokenType::Println]) {
            // Print statement
            self.print_statement().map(Statement::Print)
        } else {
            // Expression statement
            self.expression_statement().map(Statement::Expression)
        }
    }

    /// Parse a block statement
    fn block(&mut self) -> Result<Block, ParserError> {
        let start_pos = self.previous().location.start;
        let mut statements = Vec::new();

        while !self.check(TokenType::RightBrace) && !self.is_at_end() {
            statements.push(self.statement()?);
        }

        self.consume(TokenType::RightBrace, "Expected '}' after block")?;
        let end_pos = self.previous().location.end;

        Ok(Block {
            statements,
            location: Location::new(start_pos, end_pos),
        })
    }

    /// Parse a return statement
    fn return_statement(&mut self) -> Result<ReturnStatement, ParserError> {
        let start_pos = self.previous().location.start;

        let value = if self.check(TokenType::Semicolon) {
            None
        } else {
            Some(Box::new(self.expression()?))
        };

        self.consume(TokenType::Semicolon, "Expected ';' after return statement")?;
        let end_pos = self.previous().location.end;

        Ok(ReturnStatement {
            value,
            location: Location::new(start_pos, end_pos),
        })
    }

    /// Parse an if statement
    fn if_statement(&mut self) -> Result<IfStatement, ParserError> {
        let start_pos = self.previous().location.start;

        // Support both styles: if (condition) and if condition
        let condition = if self.match_tokens(&[TokenType::LeftParen]) {
            let cond = Box::new(self.expression()?);
            self.consume(TokenType::RightParen, "Expected ')' after if condition")?;
            cond
        } else {
            // Direct expression without parentheses
            Box::new(self.expression()?)
        };

        let then_branch = Box::new(self.statement()?);

        let else_branch = if self.match_tokens(&[TokenType::Else]) {
            Some(Box::new(self.statement()?))
        } else {
            None
        };

        let end_pos = if let Some(ref else_stmt) = else_branch {
            else_stmt.location().end
        } else {
            then_branch.location().end
        };

        Ok(IfStatement {
            condition,
            then_branch,
            else_branch,
            location: Location::new(start_pos, end_pos),
        })
    }

    /// Parse a while statement
    fn while_statement(&mut self) -> Result<WhileStatement, ParserError> {
        let start_pos = self.previous().location.start;

        // Support both styles: while (condition) and while condition
        let condition = if self.match_tokens(&[TokenType::LeftParen]) {
            let cond = Box::new(self.expression()?);
            self.consume(TokenType::RightParen, "Expected ')' after while condition")?;
            cond
        } else {
            // Direct expression without parentheses
            Box::new(self.expression()?)
        };

        let body = Box::new(self.statement()?);
        let end_pos = body.location().end;

        Ok(WhileStatement {
            condition,
            body,
            location: Location::new(start_pos, end_pos),
        })
    }

    /// Parse a for statement
    fn for_statement(&mut self) -> Result<ForStatement, ParserError> {
        let start_pos = self.previous().location.start;

        let variable = if let TokenType::Identifier = self.peek().token_type {
            self.advance().lexeme.clone()
        } else {
            return Err(self.error("Expected variable name after 'for'"));
        };

        self.consume(TokenType::In, "Expected 'in' after for loop variable")?;

        let iterable = self.expression()?;

        let body = Box::new(self.statement()?);
        let end_pos = body.location().end;

        Ok(ForStatement {
            variable,
            iterable,
            body,
            location: Location::new(start_pos, end_pos),
        })
    }

    /// Parse a match statement
    fn match_statement(&mut self) -> Result<MatchStatement, ParserError> {
        let start_pos = self.previous().location.start; // 'match' token location

        // Parse the value expression to match against
        let value = Box::new(self.expression()?);

        // Opening brace
        self.consume(TokenType::LeftBrace, "Expected '{' after match value")?;

        // Parse match arms
        let mut arms = Vec::new();

        while !self.check(TokenType::RightBrace) && !self.is_at_end() {
            let arm = self.match_arm()?;
            arms.push(arm);

            // Arms can be separated by commas or newlines (optional)
            self.match_tokens(&[TokenType::Comma]);
        }

        // Closing brace
        self.consume(TokenType::RightBrace, "Expected '}' after match arms")?;
        let end_pos = self.previous().location.end;

        Ok(MatchStatement {
            value,
            arms,
            location: Location::new(start_pos, end_pos),
        })
    }

    /// Parse a match arm (pattern => expression)
    fn match_arm(&mut self) -> Result<MatchArm, ParserError> {
        let start_pos = self.peek_position();

        // Parse the pattern
        let pattern = self.parse_pattern()?;

        // Expect '=>' arrow
        self.consume(TokenType::FatArrow, "Expected '=>' after pattern")?;

        // Parse the expression
        let expression = Box::new(self.expression()?);

        let end_pos = expression.location().end;

        Ok(MatchArm {
            pattern,
            expression,
            location: Location::new(start_pos, end_pos),
        })
    }

    /// Parse a pattern for match expressions
    fn parse_pattern(&mut self) -> Result<Pattern, ParserError> {
        // Check for wildcard pattern first
        if self.match_tokens(&[TokenType::Underscore]) {
            let location = self.previous().location;
            return Ok(Pattern::Wildcard(WildcardPattern { location }));
        }

        // Check for literals (numbers, strings, booleans)
        if self.check(TokenType::IntLiteral) || self.check(TokenType::FloatLiteral) ||
           self.check(TokenType::StringLiteral) || self.check(TokenType::True) || self.check(TokenType::False) {
            let expr = self.primary()?;
            if let Expression::Literal(lit) = expr {
                let location = match &lit {
                    LiteralExpression::Number(n) => n.location.clone(),
                    LiteralExpression::String(s) => s.location.clone(),
                    LiteralExpression::Boolean(b) => b.location.clone(),
                    LiteralExpression::Null(n) => n.location.clone(),
                };
                return Ok(Pattern::Literal(LiteralPattern {
                    value: lit,
                    location,
                }));
            }
        }

        // Check for identifier pattern (could be variable binding or enum variant)
        if self.check(TokenType::Identifier) {
            let name = self.advance().lexeme.clone();
            let start_loc = self.previous().location.start;

            // Check if this is an enum variant pattern (EnumName.Variant)
            if self.match_tokens(&[TokenType::Dot]) {
                let variant_name = self.consume_identifier("Expected variant name after '.'")?;
                
                // Check for pattern arguments
                let mut patterns = None;
                if self.match_tokens(&[TokenType::LeftParen]) {
                    let mut args = Vec::new();
                    
                    if !self.check(TokenType::RightParen) {
                        loop {
                            args.push(self.parse_pattern()?);
                            if !self.match_tokens(&[TokenType::Comma]) {
                                break;
                            }
                        }
                    }
                    
                    self.consume(TokenType::RightParen, "Expected ')' after enum variant patterns")?;
                    patterns = Some(args);
                }

                let end_loc = self.previous().location.end;
                return Ok(Pattern::EnumVariant(EnumVariantPattern {
                    enum_name: name,
                    variant_name,
                    patterns,
                    location: Location::new(start_loc, end_loc),
                }));
            } else {
                // Simple identifier pattern (variable binding)
                let end_loc = self.previous().location.end;
                return Ok(Pattern::Identifier(IdentifierPattern {
                    name,
                    location: Location::new(start_loc, end_loc),
                }));
            }
        }

        Err(ParserError::ExpectedExpression { position: self.peek_position() })
    }

    /// Parse a match expression (returns a value)
    fn match_expression(&mut self) -> Result<Expression, ParserError> {
        let start_pos = self.previous().location.start; // 'match' token location

        // Parse the value expression to match against
        let value = Box::new(self.expression()?);

        // Opening brace
        self.consume(TokenType::LeftBrace, "Expected '{' after match value")?;

        // Parse match arms
        let mut arms = Vec::new();

        while !self.check(TokenType::RightBrace) && !self.is_at_end() {
            let arm = self.match_arm()?;
            arms.push(arm);

            // Arms can be separated by commas (optional)
            self.match_tokens(&[TokenType::Comma]);
        }

        // Closing brace
        self.consume(TokenType::RightBrace, "Expected '}' after match arms")?;
        let end_pos = self.previous().location.end;

        Ok(Expression::Match(MatchExpression {
            value,
            arms,
            location: Location::new(start_pos, end_pos),
        }))
    }

    /// Parse a print statement
    fn print_statement(&mut self) -> Result<PrintStatement, ParserError> {
        let start_pos = self.previous().location.start;

        self.consume(TokenType::LeftParen, "Expected '(' after 'println'")?;

        let mut arguments = Vec::new();
        if !self.check(TokenType::RightParen) {
            loop {
                arguments.push(self.expression()?);
                if !self.match_tokens(&[TokenType::Comma]) {
                    break;
                }
            }
        }

        self.consume(TokenType::RightParen, "Expected ')' after println arguments")?;
        self.consume(TokenType::Semicolon, "Expected ';' after println statement")?;
        let end_pos = self.previous().location.end;

        Ok(PrintStatement {
            arguments,
            location: Location::new(start_pos, end_pos),
        })
    }

    /// Parse an expression statement
    fn expression_statement(&mut self) -> Result<ExpressionStatement, ParserError> {
        let expr = self.expression()?;
        let start_pos = expr.location().start;
        self.consume(TokenType::Semicolon, "Expected ';' after expression")?;
        let end_pos = self.previous().location.end;

        Ok(ExpressionStatement {
            expression: Box::new(expr),
            location: Location::new(start_pos, end_pos),
        })
    }

    /// Parse an expression
    fn expression(&mut self) -> Result<Expression, ParserError> {
        self.assignment()
    }

    /// Parse an assignment expression
    fn assignment(&mut self) -> Result<Expression, ParserError> {
        let expr = self.logical_or()?;

        if self.match_tokens(&[TokenType::Assign]) {
            let value = Box::new(self.assignment()?);

            // Check that the left-hand side is a valid assignment target
            if let Expression::Identifier(ident) = expr {
                let start_pos = ident.location.start;
                let end_pos = value.location().end;

                return Ok(Expression::Assignment(AssignmentExpression {
                    name: ident.name,
                    value,
                    location: Location::new(start_pos, end_pos),
                }));
            } else {
                return Err(ParserError::UnexpectedToken {
                    expected: "identifier".to_string(),
                    got: "expression".to_string(),
                    position: expr.location().start,
                });
            }
        }

        Ok(expr)
    }

    /// Parse a logical OR expression
    fn logical_or(&mut self) -> Result<Expression, ParserError> {
        let mut expr = self.logical_and()?;

        while self.match_tokens(&[TokenType::Or]) {
            let operator = BinaryOperator::Or;
            let right = Box::new(self.logical_and()?);
            let start_pos = expr.location().start;
            let end_pos = right.location().end;

            expr = Expression::Binary(BinaryExpression {
                left: Box::new(expr),
                operator,
                right,
                location: Location::new(start_pos, end_pos),
            });
        }

        Ok(expr)
    }

    /// Parse a logical AND expression
    fn logical_and(&mut self) -> Result<Expression, ParserError> {
        let mut expr = self.equality()?;

        while self.match_tokens(&[TokenType::And]) {
            let operator = BinaryOperator::And;
            let right = Box::new(self.equality()?);
            let start_pos = expr.location().start;
            let end_pos = right.location().end;

            expr = Expression::Binary(BinaryExpression {
                left: Box::new(expr),
                operator,
                right,
                location: Location::new(start_pos, end_pos),
            });
        }

        Ok(expr)
    }

    /// Parse an equality expression
    fn equality(&mut self) -> Result<Expression, ParserError> {
        let mut expr = self.comparison()?;

        while self.match_tokens(&[TokenType::Equal, TokenType::NotEqual]) {
            let operator = match self.previous().token_type {
                TokenType::Equal => BinaryOperator::Equal,
                TokenType::NotEqual => BinaryOperator::NotEqual,
                _ => unreachable!(),
            };
            let right = Box::new(self.comparison()?);
            let start_pos = expr.location().start;
            let end_pos = right.location().end;

            expr = Expression::Binary(BinaryExpression {
                left: Box::new(expr),
                operator,
                right,
                location: Location::new(start_pos, end_pos),
            });
        }

        Ok(expr)
    }

    /// Parse a comparison expression
    fn comparison(&mut self) -> Result<Expression, ParserError> {
        let mut expr = self.range()?;

        while self.match_tokens(&[TokenType::GreaterThan, TokenType::GreaterEqual,
            TokenType::LessThan, TokenType::LessEqual]) {
            let operator = match self.previous().token_type {
                TokenType::LessThan => BinaryOperator::LessThan,
                TokenType::GreaterThan => BinaryOperator::GreaterThan,
                TokenType::LessEqual => BinaryOperator::LessEqual,
                TokenType::GreaterEqual => BinaryOperator::GreaterEqual,
                _ => unreachable!(),
            };
            let right = Box::new(self.term()?);
            let start_pos = expr.location().start;
            let end_pos = right.location().end;

            expr = Expression::Binary(BinaryExpression {
                left: Box::new(expr),
                operator,
                right,
                location: Location::new(start_pos, end_pos),
            });
        }

        Ok(expr)
    }

    /// Parse a range expression
    fn range(&mut self) -> Result<Expression, ParserError> {
        let mut expr = self.term()?;

        if self.match_tokens(&[TokenType::DotDot]) {
            let start_loc = expr.location().start;
            let end = Box::new(self.term()?);
            let end_loc = end.location().end;

            expr = Expression::Range(RangeExpression {
                start: Box::new(expr),
                end,
                location: Location::new(start_loc, end_loc),
            });
        }

        Ok(expr)
    }

    /// Parse a term expression (addition/subtraction)
    fn term(&mut self) -> Result<Expression, ParserError> {
        let mut expr = self.factor()?;

        while self.match_tokens(&[TokenType::Plus, TokenType::Minus]) {
            let operator = match self.previous().token_type {
                TokenType::Plus => BinaryOperator::Add,
                TokenType::Minus => BinaryOperator::Subtract,
                _ => unreachable!(),
            };
            let right = Box::new(self.factor()?);
            let start_pos = expr.location().start;
            let end_pos = right.location().end;

            expr = Expression::Binary(BinaryExpression {
                left: Box::new(expr),
                operator,
                right,
                location: Location::new(start_pos, end_pos),
            });
        }

        Ok(expr)
    }

    /// Parse a factor expression (multiplication/division/modulo)
    fn factor(&mut self) -> Result<Expression, ParserError> {
        let mut expr = self.unary()?;

        while self.match_tokens(&[TokenType::Multiply, TokenType::Divide, TokenType::Modulo]) {
            let operator = match self.previous().token_type {
                TokenType::Multiply => BinaryOperator::Multiply,
                TokenType::Divide => BinaryOperator::Divide,
                TokenType::Modulo => BinaryOperator::Modulo,
                _ => unreachable!(),
            };
            let right = Box::new(self.unary()?);
            let start_pos = expr.location().start;
            let end_pos = right.location().end;

            expr = Expression::Binary(BinaryExpression {
                left: Box::new(expr),
                operator,
                right,
                location: Location::new(start_pos, end_pos),
            });
        }

        Ok(expr)
    }

    /// Parse a unary expression
    fn unary(&mut self) -> Result<Expression, ParserError> {
        if self.match_tokens(&[TokenType::Not, TokenType::Minus]) {
            let operator = match self.previous().token_type {
                TokenType::Minus => UnaryOperator::Negate,
                TokenType::Not => UnaryOperator::Not,
                _ => unreachable!(),
            };
            let right = Box::new(self.unary()?);
            let start_pos = self.previous().location.start;
            let end_pos = right.location().end;

            return Ok(Expression::Unary(UnaryExpression {
                operator,
                operand: right,
                location: Location::new(start_pos, end_pos),
            }));
        }

        self.call()
    }

    /// Parse a call expression
    fn call(&mut self) -> Result<Expression, ParserError> {
        let mut expr = self.primary()?;

        loop {
            if self.match_tokens(&[TokenType::LeftParen]) {
                expr = self.finish_call(expr)?;
            } else if self.match_tokens(&[TokenType::LeftBracket]) {
                // Handle index expressions
                let start_loc = expr.location().start;
                let index = self.expression()?;
                self.consume(TokenType::RightBracket, "Expected ']' after array index")?;
                let end_loc = self.previous().location.end;

                expr = Expression::Index(IndexExpression {
                    object: Box::new(expr),
                    index: Box::new(index),
                    location: Location::new(start_loc, end_loc),
                });
            } else if self.match_tokens(&[TokenType::Dot]) {
                // Handle field access
                let start_loc = expr.location().start;
                let field_name = if let TokenType::Identifier = self.peek().token_type {
                    self.advance().lexeme.clone()
                } else {
                    return Err(self.error("Expected field name after '.'"));
                };
                let end_loc = self.previous().location.end;

                expr = Expression::FieldAccess(FieldAccessExpression {
                    object: Box::new(expr),
                    field: field_name,
                    location: Location::new(start_loc, end_loc),
                });
            } else if self.match_tokens(&[TokenType::Question]) {
                // Handle ? operator (try expression)
                let start_loc = expr.location().start;
                let end_loc = self.previous().location.end;

                expr = Expression::Try(TryExpression {
                    expression: Box::new(expr),
                    location: Location::new(start_loc, end_loc),
                });
            } else {
                break;
            }
        }

        Ok(expr)
    }

    /// Finish parsing a function call
    fn finish_call(&mut self, callee: Expression) -> Result<Expression, ParserError> {
        let mut arguments = Vec::new();

        if !self.check(TokenType::RightParen) {
            loop {
                arguments.push(self.expression()?);
                if !self.match_tokens(&[TokenType::Comma]) {
                    break;
                }
            }
        }

        self.consume(TokenType::RightParen, "Expected ')' after arguments")?;
        let end_pos = self.previous().location.end;

        if let Expression::Identifier(ident) = callee {
            Ok(Expression::Call(CallExpression {
                callee: ident.name,
                arguments,
                location: Location::new(ident.location.start, end_pos),
            }))
        } else {
            Err(ParserError::UnexpectedToken {
                expected: "identifier".to_string(),
                got: "expression".to_string(),
                position: callee.location().start,
            })
        }
    }

    /// Parse a primary expression
    fn primary(&mut self) -> Result<Expression, ParserError> {
        if self.match_tokens(&[TokenType::True, TokenType::False]) {
            let value = self.previous().token_type == TokenType::True;
            let location = self.previous().location;
            return Ok(Expression::Literal(LiteralExpression::Boolean(BooleanLiteral {
                value,
                location,
            })));
        }

        if self.match_tokens(&[TokenType::Null]) {
            let location = self.previous().location;
            return Ok(Expression::Literal(LiteralExpression::Null(NullLiteral {
                location,
            })));
        }

        if self.match_tokens(&[TokenType::IntLiteral]) {
            let value = self.previous().lexeme.clone();
            let location = self.previous().location;
            return Ok(Expression::Literal(LiteralExpression::Number(NumberLiteral {
                value,
                is_float: false,
                location,
            })));
        }

        if self.match_tokens(&[TokenType::FloatLiteral]) {
            let value = self.previous().lexeme.clone();
            let location = self.previous().location;
            return Ok(Expression::Literal(LiteralExpression::Number(NumberLiteral {
                value,
                is_float: true,
                location,
            })));
        }

        if self.match_tokens(&[TokenType::StringLiteral]) {
            let value = self.previous().lexeme.clone();
            let location = self.previous().location;
            return Ok(Expression::Literal(LiteralExpression::String(StringLiteral {
                value,
                location,
            })));
        }

        if self.match_tokens(&[TokenType::Identifier]) {
            let name = self.previous().lexeme.clone();
            let start_loc = self.previous().location.start;

            // Check for generic type arguments <T, U, ...>
            let mut type_arguments = None;
            if self.check(TokenType::LessThan) {
                self.advance(); // consume '<'
                let mut args = Vec::new();
                
                args.push(self.parse_type()?);
                while self.match_tokens(&[TokenType::Comma]) {
                    args.push(self.parse_type()?);
                }
                
                self.consume(TokenType::GreaterThan, "Expected '>' after type arguments")?;
                type_arguments = Some(args);
            }

            // Check if this is a struct literal (with or without generics)
            if self.check(TokenType::LeftBrace) {
                if type_arguments.is_some() {
                    // Generic struct literal - for now, treat as error
                    return Err(ParserError::UnexpectedToken {
                        expected: "generic struct literals not yet supported".to_string(),
                        got: "generic struct literal".to_string(),
                        position: start_loc,
                    });
                }
                return self.struct_literal(name, start_loc);
            }

            // Check if this is an enum literal (EnumName.Variant or EnumName<T>.Variant)
            if self.check(TokenType::Dot) {
                return self.enum_literal_with_generics(name, type_arguments, start_loc);
            }

            // If we have type arguments but no dot, this is an error
            if type_arguments.is_some() {
                return Err(ParserError::UnexpectedToken {
                    expected: "'.', '{', or function call after generic type".to_string(),
                    got: self.peek().lexeme.clone(),
                    position: self.peek_position(),
                });
            }

            return Ok(Expression::Identifier(IdentifierExpression {
                name,
                location: Location::new(start_loc, self.previous().location.end),
            }));
        }

        if self.match_tokens(&[TokenType::LeftParen]) {
            let expr = self.expression()?;
            self.consume(TokenType::RightParen, "Expected ')' after expression")?;
            return Ok(expr);
        }

        // Match expression
        if self.match_tokens(&[TokenType::Match]) {
            return self.match_expression();
        }

        if self.match_tokens(&[TokenType::LeftBracket]) {
            // Array literal
            let start_loc = self.previous().location.start;
            let mut elements = Vec::new();

            if !self.check(TokenType::RightBracket) {
                loop {
                    elements.push(self.expression()?);
                    if !self.match_tokens(&[TokenType::Comma]) {
                        break;
                    }
                }
            }

            self.consume(TokenType::RightBracket, "Expected ']' after array elements")?;
            let end_loc = self.previous().location.end;

            return Ok(Expression::ArrayLiteral(ArrayLiteralExpression {
                elements,
                location: Location::new(start_loc, end_loc),
            }));
        }

        Err(ParserError::ExpectedExpression { position: self.peek_position() })
    }

    // Helper methods

    /// Create an error with a message
    fn error(&self, message: &str) -> ParserError {
        ParserError::UnexpectedToken {
            expected: message.to_string(),
            got: self.peek().lexeme.clone(),
            position: self.peek_position(),
        }
    }

    /// Check if we're at the end of the token stream
    fn is_at_end(&self) -> bool {
        self.peek().token_type == TokenType::EOF
    }

    /// Get the current token without consuming it
    fn peek(&self) -> &Token {
        &self.tokens[self.current]
    }

    /// Get the current position
    fn peek_position(&self) -> Position {
        self.peek().location.start
    }

    /// Get the previous token
    fn previous(&self) -> &Token {
        &self.tokens[self.current - 1]
    }

    /// Advance to the next token and return the current one
    fn advance(&mut self) -> &Token {
        if !self.is_at_end() {
            self.current += 1;
        }
        self.previous()
    }

    /// Check if the current token is of the given type
    fn check(&self, token_type: TokenType) -> bool {
        if self.is_at_end() {
            false
        } else {
            self.peek().token_type == token_type
        }
    }

    /// Check if the current token matches any of the given types, and consume it if so
    fn match_tokens(&mut self, types: &[TokenType]) -> bool {
        for token_type in types {
            if self.check(*token_type) {
                self.advance();
                return true;
            }
        }
        false
    }

    /// Consume a token of the given type, or return an error
    fn consume(&mut self, token_type: TokenType, _message: &str) -> Result<&Token, ParserError> {
        if self.check(token_type) {
            Ok(self.advance())
        } else {
            Err(ParserError::UnexpectedToken {
                expected: format!("{:?}", token_type),
                got: format!("{:?}", self.peek().token_type),
                position: self.peek_position(),
            })
        }
    }

    /// Consume an identifier token
    fn consume_identifier(&mut self, _message: &str) -> Result<String, ParserError> {
        if self.check(TokenType::Identifier) {
            Ok(self.advance().lexeme.clone())
        } else {
            Err(ParserError::ExpectedIdentifier { position: self.peek_position() })
        }
    }
}

/// Extension trait for Statement AST nodes to get their location
trait StatementLocation {
    fn location(&self) -> &Location;
}

impl StatementLocation for Statement {
    fn location(&self) -> &Location {
        match self {
            Statement::Expression(stmt) => &stmt.location,
            Statement::Block(stmt) => &stmt.location,
            Statement::Return(stmt) => &stmt.location,
            Statement::If(stmt) => &stmt.location,
            Statement::While(stmt) => &stmt.location,
            Statement::Print(stmt) => &stmt.location,
            Statement::DeclarationStatement(decl) => decl.location(),
            Statement::For(stmt) => &stmt.location,
            Statement::Match(stmt) => &stmt.location,
        }
    }
}

impl Statement {
    pub fn location(&self) -> &Location {
        StatementLocation::location(self)
    }
}

impl Declaration {
    pub fn location(&self) -> &Location {
        match self {
            Declaration::Function(decl) => &decl.location,
            Declaration::Variable(decl) => &decl.location,
            Declaration::Struct(decl) => &decl.location,
            Declaration::Enum(decl) => &decl.location,
        }
    }
}

/// Extension trait for Expression AST nodes to get their location
trait ExpressionLocation {
    fn location(&self) -> &Location;
}

impl ExpressionLocation for Expression {
    fn location(&self) -> &Location {
        match self {
            Expression::Assignment(expr) => &expr.location,
            Expression::Binary(expr) => &expr.location,
            Expression::Unary(expr) => &expr.location,
            Expression::Literal(LiteralExpression::Number(expr)) => &expr.location,
            Expression::Literal(LiteralExpression::String(expr)) => &expr.location,
            Expression::Literal(LiteralExpression::Boolean(expr)) => &expr.location,
            Expression::Literal(LiteralExpression::Null(expr)) => &expr.location,
            Expression::Identifier(expr) => &expr.location,
            Expression::Call(expr) => &expr.location,
            Expression::Parenthesized(expr) => &expr.location,
            Expression::StructLiteral(expr) => &expr.location,
            Expression::FieldAccess(expr) => &expr.location,
            Expression::ArrayLiteral(expr) => &expr.location,
            Expression::Index(expr) => &expr.location,
            Expression::Range(expr) => &expr.location,
            Expression::Match(expr) => &expr.location,
            Expression::EnumLiteral(expr) => &expr.location,
            Expression::Try(expr) => &expr.location,
        }
    }
}

impl Expression {
    pub fn location(&self) -> &Location {
        ExpressionLocation::location(self)
    }
}