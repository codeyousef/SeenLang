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

    /// Parse a declaration
    fn declaration(&mut self) -> Result<Declaration, ParserError> {
        if self.match_tokens(&[TokenType::Func]) {
            // Function declaration
            self.function_declaration().map(Declaration::Function)
        } else if self.match_tokens(&[TokenType::Val, TokenType::Var]) {
            // Variable declaration
            self.variable_declaration().map(Declaration::Variable)
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
            Some(self.type_specifier()?)
        } else {
            None
        };

        // Function body
        self.consume(TokenType::LeftBrace, "Expected '{' before function body")?;
        let body_start_pos = self.previous().location.start; // Position of '{'

        let mut statements = Vec::new();
        while !self.check(TokenType::RightBrace) && !self.is_at_end() {
            statements.push(self.statement()?);
        }

        self.consume(TokenType::RightBrace, "Expected '}' after function body")?;
        let body_end_pos = self.previous().location.end; // Position of '}'
        
        let body = Block {
            statements,
            location: Location::new(body_start_pos, body_end_pos),
        };

        // The overall location of the function declaration is from the 'func' keyword to the closing '}' of the body.
        Ok(FunctionDeclaration {
            name,
            parameters,
            return_type,
            body,
            location: Location::new(func_keyword_start_pos, body_end_pos),
        })
    }

    /// Parse a parameter list
    fn parameter_list(&mut self) -> Result<Vec<Parameter>, ParserError> {
        let mut parameters = Vec::new();

        // Parse the first parameter
        parameters.push(self.parameter()?);

        // Parse any additional parameters
        while self.match_tokens(&[TokenType::Comma]) {
            parameters.push(self.parameter()?);
        }

        Ok(parameters)
    }

    /// Parse a single parameter
    fn parameter(&mut self) -> Result<Parameter, ParserError> {
        let start_pos = self.peek_position();
        
        // Parameter name
        let name = self.consume_identifier("Expected parameter name")?;
        
        // Parameter type
        self.consume(TokenType::Colon, "Expected ':' after parameter name")?;
        let param_type = self.type_specifier()?;
        
        let end_pos = self.previous().location.end;
        
        Ok(Parameter {
            name,
            param_type,
            location: Location::new(start_pos, end_pos),
        })
    }

    /// Parse a variable declaration
    fn variable_declaration(&mut self) -> Result<VariableDeclaration, ParserError> {
        let start_pos = self.previous().location.start;
        
        // Check if it's mutable
        let is_mutable = self.previous().token_type == TokenType::Var;
        
        // Variable name
        let name = self.consume_identifier("Expected variable name")?;
        
        // Variable type (optional)
        let var_type = if self.match_tokens(&[TokenType::Colon]) {
            Some(self.type_specifier()?)
        } else {
            None
        };
        
        // Initializer
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

    /// Parse a type specifier
    fn type_specifier(&mut self) -> Result<Type, ParserError> {
        if self.match_tokens(&[TokenType::LeftBracket]) {
            // Array type
            let element_type = self.type_specifier()?;
            self.consume(TokenType::RightBracket, "Expected ']' after array element type")?;
            Ok(Type::Array(Box::new(element_type)))
        } else {
            // Simple type
            let type_name = self.consume_identifier("Expected type name")?;
            Ok(Type::Simple(type_name))
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
        
        // Return value (optional)
        let value = if !self.check(TokenType::Semicolon) {
            Some(Box::new(self.expression()?))
        } else {
            None
        };
        
        self.consume(TokenType::Semicolon, "Expected ';' after return value")?;
        let end_pos = self.previous().location.end;
        
        Ok(ReturnStatement {
            value,
            location: Location::new(start_pos, end_pos),
        })
    }

    /// Parse an if statement
    fn if_statement(&mut self) -> Result<IfStatement, ParserError> {
        let start_pos = self.previous().location.start;
        
        // Condition
        self.consume(TokenType::LeftParen, "Expected '(' after 'if'")?;
        let condition = Box::new(self.expression()?);
        self.consume(TokenType::RightParen, "Expected ')' after if condition")?;
        
        // Then branch
        let then_branch = Box::new(self.statement()?);
        
        // Else branch (optional)
        let else_branch = if self.match_tokens(&[TokenType::Else]) {
            Some(Box::new(self.statement()?))
        } else {
            None
        };
        
        let end_pos = if let Some(ref else_branch) = else_branch {
            else_branch.location().end
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
        
        // Condition
        self.consume(TokenType::LeftParen, "Expected '(' after 'while'")?;
        let condition = Box::new(self.expression()?);
        self.consume(TokenType::RightParen, "Expected ')' after while condition")?;
        
        // Body
        let body = Box::new(self.statement()?);
        let end_pos = body.location().end;
        
        Ok(WhileStatement {
            condition,
            body,
            location: Location::new(start_pos, end_pos),
        })
    }

    /// Parse a print statement
    fn print_statement(&mut self) -> Result<PrintStatement, ParserError> {
        let start_pos = self.previous().location.start; // 'println' keyword
        self.consume(TokenType::LeftParen, "Expected '(' after println keyword")?;

        let arguments = if self.check(TokenType::RightParen) {
            Vec::new() // No arguments
        } else {
            self.argument_list()? // Use existing argument_list helper
        };

        self.consume(TokenType::RightParen, "Expected ')' after println arguments")?;
        self.consume(TokenType::Semicolon, "Expected ';' after print statement")?;
        let end_pos = self.previous().location.end; // Semicolon

        Ok(PrintStatement {
            arguments, // Use the new field
            location: Location::new(start_pos, end_pos),
        })
    }

    /// Parse an expression statement
    fn expression_statement(&mut self) -> Result<ExpressionStatement, ParserError> {
        let start_pos = self.peek_position();
        let expression = Box::new(self.expression()?);
        self.consume(TokenType::Semicolon, "Expected ';' after expression")?;
        let end_pos = self.previous().location.end;
        
        Ok(ExpressionStatement {
            expression,
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
            }
            
            // If we get here, the left-hand side is not a valid assignment target
            return Err(ParserError::ExpectedIdentifier { position: self.previous().location.start });
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
        let mut expr = self.term()?;
        
        while self.match_tokens(&[
            TokenType::LessThan,
            TokenType::GreaterThan,
            TokenType::LessEqual,
            TokenType::GreaterEqual,
        ]) {
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

    /// Parse a term expression
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

    /// Parse a factor expression
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
        if self.match_tokens(&[TokenType::Minus, TokenType::Not, TokenType::Plus]) {
            let operator = match self.previous().token_type {
                TokenType::Minus => UnaryOperator::Negate,
                TokenType::Not => UnaryOperator::Not,
                TokenType::Plus => UnaryOperator::Plus,
                _ => unreachable!(),
            };
            
            let start_pos = self.previous().location.start;
            let operand = Box::new(self.unary()?);
            let end_pos = operand.location().end;
            
            Ok(Expression::Unary(UnaryExpression {
                operator,
                operand,
                location: Location::new(start_pos, end_pos),
            }))
        } else {
            self.call()
        }
    }

    /// Parse a function call expression
    fn call(&mut self) -> Result<Expression, ParserError> {
        let mut expr = self.primary()?;
        
        if let Expression::Identifier(ident) = &expr {
            if self.match_tokens(&[TokenType::LeftParen]) {
                let start_pos = ident.location.start;
                let name = ident.name.clone();
                
                // Parse arguments
                let arguments = if !self.check(TokenType::RightParen) {
                    self.argument_list()?
                } else {
                    Vec::new()
                };
                
                self.consume(TokenType::RightParen, "Expected ')' after arguments")?;
                let end_pos = self.previous().location.end;
                
                expr = Expression::Call(CallExpression {
                    callee: name,
                    arguments,
                    location: Location::new(start_pos, end_pos),
                });
            }
        }
        
        Ok(expr)
    }

    /// Parse a list of function call arguments
    fn argument_list(&mut self) -> Result<Vec<Expression>, ParserError> {
        let mut arguments = Vec::new();
        
        // Parse the first argument
        arguments.push(self.expression()?);
        
        // Parse any additional arguments
        while self.match_tokens(&[TokenType::Comma]) {
            arguments.push(self.expression()?);
        }
        
        Ok(arguments)
    }

    /// Parse a primary expression
    fn primary(&mut self) -> Result<Expression, ParserError> {
        if self.match_tokens(&[TokenType::True, TokenType::False]) {
            // Boolean literal
            let value = self.previous().token_type == TokenType::True;
            let location = self.previous().location;
            
            Ok(Expression::Literal(LiteralExpression::Boolean(BooleanLiteral {
                value,
                location,
            })))
        } else if self.match_tokens(&[TokenType::Null]) {
            // Null literal
            let location = self.previous().location;
            
            Ok(Expression::Literal(LiteralExpression::Null(NullLiteral {
                location,
            })))
        } else if self.match_tokens(&[TokenType::IntLiteral, TokenType::FloatLiteral]) {
            // Number literal
            let token = self.previous();
            let is_float = token.token_type == TokenType::FloatLiteral;
            
            Ok(Expression::Literal(LiteralExpression::Number(NumberLiteral {
                value: token.lexeme.clone(),
                is_float,
                location: token.location,
            })))
        } else if self.match_tokens(&[TokenType::StringLiteral]) {
            // String literal
            let token = self.previous();
            
            Ok(Expression::Literal(LiteralExpression::String(StringLiteral {
                value: token.lexeme.clone(),
                location: token.location,
            })))
        } else if self.match_tokens(&[TokenType::Identifier]) {
            // Identifier
            let token = self.previous();
            
            Ok(Expression::Identifier(IdentifierExpression {
                name: token.lexeme.clone(),
                location: token.location,
            }))
        } else if self.match_tokens(&[TokenType::LeftParen]) {
            // Parenthesized expression
            let start_pos = self.previous().location.start;
            let expression = Box::new(self.expression()?);
            self.consume(TokenType::RightParen, "Expected ')' after expression")?;
            let end_pos = self.previous().location.end;
            
            Ok(Expression::Parenthesized(ParenthesizedExpression {
                expression,
                location: Location::new(start_pos, end_pos),
            }))
        } else {
            Err(ParserError::ExpectedExpression { position: self.peek_position() })
        }
    }

    // Helper methods

    /// Check if the current token has any of the given types
    fn check(&self, token_type: TokenType) -> bool {
        if self.is_at_end() {
            false
        } else {
            self.peek().token_type == token_type
        }
    }

    /// Consume the current token if it matches any of the given types
    fn match_tokens(&mut self, token_types: &[TokenType]) -> bool {
        for &token_type in token_types {
            if self.check(token_type) {
                self.advance();
                return true;
            }
        }
        false
    }

    /// Consume the current token if it matches the expected type, otherwise error
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

    /// Consume the current token if it's an identifier, otherwise error
    fn consume_identifier(&mut self, _message: &str) -> Result<String, ParserError> {
        if self.check(TokenType::Identifier) {
            let identifier = self.advance().lexeme.clone();
            Ok(identifier)
        } else {
            Err(ParserError::ExpectedIdentifier { position: self.peek_position() })
        }
    }

    /// Advance to the next token and return the previous token
    fn advance(&mut self) -> &Token {
        if !self.is_at_end() {
            self.current += 1;
        }
        self.previous()
    }

    /// Check if we've reached the end of the token stream
    fn is_at_end(&self) -> bool {
        self.peek().token_type == TokenType::EOF
    }

    /// Get the current token without advancing
    fn peek(&self) -> &Token {
        &self.tokens[self.current]
    }

    /// Get the position of the current token
    fn peek_position(&self) -> Position {
        self.peek().location.start
    }

    /// Get the previous token
    fn previous(&self) -> &Token {
        &self.tokens[self.current - 1]
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
            Statement::DeclarationStatement(decl) => decl.location(), // Added this line
        }
    }
}

impl Declaration {
    pub fn location(&self) -> &Location {
        match self {
            Declaration::Function(decl) => &decl.location,
            Declaration::Variable(decl) => &decl.location,
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
        }
    }
}
