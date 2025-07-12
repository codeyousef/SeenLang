use seen_lexer::token::{Token, TokenType, Position, Location};
use crate::ast::*;

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
            fields,
            location: Location::new(start_pos, end_pos),
        })
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
        if self.check(TokenType::Identifier) {
            let type_name = self.advance().lexeme.clone();
            
            // Check for generic/array types
            if self.match_tokens(&[TokenType::LessThan]) {
                let inner_type = Box::new(self.parse_type()?);
                self.consume(TokenType::GreaterThan, "Expected '>' after type parameter")?;
                
                match type_name.as_str() {
                    "Array" => Ok(Type::Array(inner_type)),
                    _ => {
                        // For now, just treat other generics as simple types since Generic doesn't exist in AST
                        Ok(Type::Simple(format!("{}<...>", type_name)))
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

    /// Parse a factor expression (multiplication/division)
    fn factor(&mut self) -> Result<Expression, ParserError> {
        let mut expr = self.unary()?;

        while self.match_tokens(&[TokenType::Multiply, TokenType::Divide]) {
            let operator = match self.previous().token_type {
                TokenType::Multiply => BinaryOperator::Multiply,
                TokenType::Divide => BinaryOperator::Divide,
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
            let location = self.previous().location;
            return Ok(Expression::Identifier(IdentifierExpression {
                name,
                location,
            }));
        }

        if self.match_tokens(&[TokenType::LeftParen]) {
            let expr = self.expression()?;
            self.consume(TokenType::RightParen, "Expected ')' after expression")?;
            return Ok(expr);
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
        }
    }
}

impl Expression {
    pub fn location(&self) -> &Location {
        ExpressionLocation::location(self)
    }
}