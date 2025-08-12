//! Parser implementation for Seen Language
//! 
//! Implements recursive descent parsing with everything as expressions.
//! NO hardcoded keywords - all loaded dynamically from TOML files.

use crate::ast::*;
use crate::error::{ParseError, ParseResult};
use seen_lexer::{Lexer, Token, TokenType, Position};
use seen_lexer::keyword_manager::KeywordType;
use std::collections::VecDeque;

pub struct Parser {
    lexer: Lexer,
    current: Token,
    peek_buffer: VecDeque<Token>,
}

impl Parser {
    pub fn new(mut lexer: Lexer) -> Self {
        let current = lexer.next_token().unwrap_or(Token {
            token_type: TokenType::EOF,
            lexeme: String::new(),
            position: Position::new(0, 0, 0),
        });
        
        Self {
            lexer,
            current,
            peek_buffer: VecDeque::new(),
        }
    }
    
    /// Parse a complete program
    pub fn parse_program(&mut self) -> ParseResult<Program> {
        let mut expressions = Vec::new();
        
        while !self.is_at_end() {
            expressions.push(self.parse_expression()?);
            
            // Optional semicolons between top-level expressions
            if self.check(&TokenType::Semicolon) {
                self.advance();
            }
        }
        
        Ok(Program { expressions })
    }
    
    /// Parse any expression
    pub fn parse_expression(&mut self) -> ParseResult<Expression> {
        self.parse_assignment()
    }
    
    /// Parse assignment or regular expression
    fn parse_assignment(&mut self) -> ParseResult<Expression> {
        let expr = self.parse_logical_or()?;
        
        if self.check(&TokenType::Assign) {
            let pos = self.current.position.clone();
            self.advance();
            let value = self.parse_assignment()?;
            return Ok(Expression::Assignment {
                target: Box::new(expr),
                value: Box::new(value),
                pos,
            });
        }
        
        Ok(expr)
    }
    
    /// Parse logical OR expressions (including word operator 'or')
    fn parse_logical_or(&mut self) -> ParseResult<Expression> {
        let mut expr = self.parse_logical_and()?;
        
        while self.check_keyword(KeywordType::KeywordOr) {
            let pos = self.current.position.clone();
            self.advance();
            let right = self.parse_logical_and()?;
            expr = Expression::BinaryOp {
                left: Box::new(expr),
                op: BinaryOperator::Or,
                right: Box::new(right),
                pos,
            };
        }
        
        Ok(expr)
    }
    
    /// Parse logical AND expressions (including word operator 'and')
    fn parse_logical_and(&mut self) -> ParseResult<Expression> {
        let mut expr = self.parse_equality()?;
        
        while self.check_keyword(KeywordType::KeywordAnd) {
            let pos = self.current.position.clone();
            self.advance();
            let right = self.parse_equality()?;
            expr = Expression::BinaryOp {
                left: Box::new(expr),
                op: BinaryOperator::And,
                right: Box::new(right),
                pos,
            };
        }
        
        Ok(expr)
    }
    
    /// Parse equality expressions (==, !=)
    fn parse_equality(&mut self) -> ParseResult<Expression> {
        let mut expr = self.parse_comparison()?;
        
        while let Some(op) = self.match_equality_op() {
            let pos = self.current.position.clone();
            self.advance();
            let right = self.parse_comparison()?;
            expr = Expression::BinaryOp {
                left: Box::new(expr),
                op,
                right: Box::new(right),
                pos,
            };
        }
        
        Ok(expr)
    }
    
    /// Parse comparison expressions (<, >, <=, >=)
    fn parse_comparison(&mut self) -> ParseResult<Expression> {
        let mut expr = self.parse_range()?;
        
        while let Some(op) = self.match_comparison_op() {
            let pos = self.current.position.clone();
            self.advance();
            let right = self.parse_range()?;
            expr = Expression::BinaryOp {
                left: Box::new(expr),
                op,
                right: Box::new(right),
                pos,
            };
        }
        
        Ok(expr)
    }
    
    /// Parse range expressions (.., ..<)
    fn parse_range(&mut self) -> ParseResult<Expression> {
        let expr = self.parse_elvis()?;
        
        if self.check(&TokenType::InclusiveRange) {
            let pos = self.current.position.clone();
            self.advance();
            let right = self.parse_elvis()?;
            return Ok(Expression::BinaryOp {
                left: Box::new(expr),
                op: BinaryOperator::InclusiveRange,
                right: Box::new(right),
                pos,
            });
        }
        
        if self.check(&TokenType::ExclusiveRange) {
            let pos = self.current.position.clone();
            self.advance();
            let right = self.parse_elvis()?;
            return Ok(Expression::BinaryOp {
                left: Box::new(expr),
                op: BinaryOperator::ExclusiveRange,
                right: Box::new(right),
                pos,
            });
        }
        
        Ok(expr)
    }
    
    /// Parse elvis operator (?:)
    fn parse_elvis(&mut self) -> ParseResult<Expression> {
        let mut expr = self.parse_term()?;
        
        if self.check(&TokenType::Elvis) {
            let pos = self.current.position.clone();
            self.advance();
            let default = self.parse_term()?;
            expr = Expression::Elvis {
                nullable: Box::new(expr),
                default: Box::new(default),
                pos,
            };
        }
        
        Ok(expr)
    }
    
    /// Parse term expressions (+, -)
    fn parse_term(&mut self) -> ParseResult<Expression> {
        let mut expr = self.parse_factor()?;
        
        while let Some(op) = self.match_term_op() {
            let pos = self.current.position.clone();
            self.advance();
            let right = self.parse_factor()?;
            expr = Expression::BinaryOp {
                left: Box::new(expr),
                op,
                right: Box::new(right),
                pos,
            };
        }
        
        Ok(expr)
    }
    
    /// Parse factor expressions (*, /, %)
    fn parse_factor(&mut self) -> ParseResult<Expression> {
        let mut expr = self.parse_unary()?;
        
        while let Some(op) = self.match_factor_op() {
            let pos = self.current.position.clone();
            self.advance();
            let right = self.parse_unary()?;
            expr = Expression::BinaryOp {
                left: Box::new(expr),
                op,
                right: Box::new(right),
                pos,
            };
        }
        
        Ok(expr)
    }
    
    /// Parse unary expressions (not, -, !!)
    fn parse_unary(&mut self) -> ParseResult<Expression> {
        // Check for 'not' keyword
        if self.check_keyword(KeywordType::KeywordNot) {
            let pos = self.current.position.clone();
            self.advance();
            let operand = self.parse_unary()?;
            return Ok(Expression::UnaryOp {
                op: UnaryOperator::Not,
                operand: Box::new(operand),
                pos,
            });
        }
        
        // Check for minus
        if self.check(&TokenType::Minus) {
            let pos = self.current.position.clone();
            self.advance();
            let operand = self.parse_unary()?;
            return Ok(Expression::UnaryOp {
                op: UnaryOperator::Negate,
                operand: Box::new(operand),
                pos,
            });
        }
        
        self.parse_postfix()
    }
    
    /// Parse postfix expressions (calls, member access, indexing, force unwrap)
    fn parse_postfix(&mut self) -> ParseResult<Expression> {
        let mut expr = self.parse_await()?;
        
        loop {
            match &self.current.token_type {
                TokenType::LeftParen => {
                    let pos = self.current.position.clone();
                    self.advance();
                    let args = self.parse_arguments()?;
                    self.expect(&TokenType::RightParen)?;
                    expr = Expression::Call {
                        callee: Box::new(expr),
                        args,
                        pos,
                    };
                }
                TokenType::Dot => {
                    let pos = self.current.position.clone();
                    self.advance();
                    let member = self.expect_identifier()?;
                    expr = Expression::MemberAccess {
                        object: Box::new(expr),
                        member,
                        is_safe: false,
                        pos,
                    };
                }
                TokenType::SafeNavigation => {
                    let pos = self.current.position.clone();
                    self.advance();
                    let member = self.expect_identifier()?;
                    expr = Expression::MemberAccess {
                        object: Box::new(expr),
                        member,
                        is_safe: true,
                        pos,
                    };
                }
                TokenType::LeftBracket => {
                    let pos = self.current.position.clone();
                    self.advance();
                    let index = self.parse_expression()?;
                    self.expect(&TokenType::RightBracket)?;
                    expr = Expression::IndexAccess {
                        object: Box::new(expr),
                        index: Box::new(index),
                        pos,
                    };
                }
                TokenType::ForceUnwrap => {
                    let pos = self.current.position.clone();
                    self.advance();
                    expr = Expression::ForceUnwrap {
                        nullable: Box::new(expr),
                        pos,
                    };
                }
                _ => break,
            }
        }
        
        Ok(expr)
    }
    
    /// Parse await expression
    fn parse_await(&mut self) -> ParseResult<Expression> {
        if self.check_keyword(KeywordType::KeywordAwait) {
            let pos = self.current.position.clone();
            self.advance();
            let expr = self.parse_primary()?;
            return Ok(Expression::Await {
                expr: Box::new(expr),
                pos,
            });
        }
        
        self.parse_primary()
    }
    
    /// Parse primary expressions (literals, identifiers, control flow, etc.)
    pub fn parse_primary(&mut self) -> ParseResult<Expression> {
        let pos = self.current.position.clone();
        
        // Literals
        if let TokenType::IntegerLiteral(value) = &self.current.token_type {
            let value = *value;
            self.advance();
            return Ok(Expression::IntegerLiteral { value, pos });
        }
        
        if let TokenType::FloatLiteral(value) = &self.current.token_type {
            let value = *value;
            self.advance();
            return Ok(Expression::FloatLiteral { value, pos });
        }
        
        if let TokenType::StringLiteral(value) = &self.current.token_type {
            let value = value.clone();
            self.advance();
            return Ok(Expression::StringLiteral { value, pos });
        }
        
        if let TokenType::InterpolatedString(_) = &self.current.token_type {
            return self.parse_interpolated_string();
        }
        
        // Boolean literals
        if self.check_keyword(KeywordType::KeywordTrue) {
            self.advance();
            return Ok(Expression::BooleanLiteral { value: true, pos });
        }
        
        if self.check_keyword(KeywordType::KeywordFalse) {
            self.advance();
            return Ok(Expression::BooleanLiteral { value: false, pos });
        }
        
        // Null literal
        if self.check_keyword(KeywordType::KeywordNull) {
            self.advance();
            return Ok(Expression::NullLiteral { pos });
        }
        
        // Control flow keywords
        if self.check_keyword(KeywordType::KeywordIf) {
            return self.parse_if();
        }
        
        if self.check_keyword(KeywordType::KeywordMatch) {
            return self.parse_match();
        }
        
        if self.check_keyword(KeywordType::KeywordWhile) {
            return self.parse_while();
        }
        
        if self.check_keyword(KeywordType::KeywordFor) {
            return self.parse_for();
        }
        
        if self.check_keyword(KeywordType::KeywordBreak) {
            return self.parse_break();
        }
        
        if self.check_keyword(KeywordType::KeywordContinue) {
            self.advance();
            return Ok(Expression::Continue { pos });
        }
        
        if self.check_keyword(KeywordType::KeywordReturn) {
            return self.parse_return();
        }
        
        // Variable declarations
        if self.check_keyword(KeywordType::KeywordLet) {
            return self.parse_let();
        }
        
        if self.check_keyword(KeywordType::KeywordVar) {
            return self.parse_var();
        }
        
        // Function/lambda definitions
        if self.check_keyword(KeywordType::KeywordFun) {
            return self.parse_function();
        }
        
        // Async functions require lexer support for async/await keywords
        // if self.check_keyword(KeywordType::KeywordAsync) {
        //     return self.parse_async_function();
        // }
        
        // Blocks and parentheses
        if self.check(&TokenType::LeftBrace) {
            // Could be a lambda or a block
            if self.is_lambda() {
                return self.parse_lambda();
            } else {
                return self.parse_block();
            }
        }
        
        if self.check(&TokenType::LeftParen) {
            self.advance();
            let expr = self.parse_expression()?;
            self.expect(&TokenType::RightParen)?;
            return Ok(expr);
        }
        
        // Arrays
        if self.check(&TokenType::LeftBracket) {
            return self.parse_array();
        }
        
        // Identifiers (could be struct literals)
        if let TokenType::PublicIdentifier(name) = &self.current.token_type {
            let name = name.clone();
            let is_public = true;
            self.advance();
            
            // Check for struct literal
            if self.check(&TokenType::LeftBrace) {
                return self.parse_struct_literal(name);
            }
            
            return Ok(Expression::Identifier { name, is_public, pos });
        }
        
        if let TokenType::PrivateIdentifier(name) = &self.current.token_type {
            let name = name.clone();
            let is_public = false;
            self.advance();
            
            // Check for struct literal
            if self.check(&TokenType::LeftBrace) {
                return self.parse_struct_literal(name);
            }
            
            return Ok(Expression::Identifier { name, is_public, pos });
        }
        
        Err(ParseError::InvalidExpression { pos })
    }
    
    /// Parse if expression
    fn parse_if(&mut self) -> ParseResult<Expression> {
        let pos = self.current.position.clone();
        self.advance(); // consume 'if'
        
        let condition = self.parse_expression()?;
        self.expect(&TokenType::LeftBrace)?;
        let then_branch = self.parse_block_body()?;
        self.expect(&TokenType::RightBrace)?;
        
        let else_branch = if self.check_keyword(KeywordType::KeywordElse) {
            self.advance();
            if self.check_keyword(KeywordType::KeywordIf) {
                Some(Box::new(self.parse_if()?))
            } else {
                self.expect(&TokenType::LeftBrace)?;
                let else_body = self.parse_block_body()?;
                self.expect(&TokenType::RightBrace)?;
                Some(Box::new(else_body))
            }
        } else {
            None
        };
        
        Ok(Expression::If {
            condition: Box::new(condition),
            then_branch: Box::new(then_branch),
            else_branch,
            pos,
        })
    }
    
    /// Parse match expression
    fn parse_match(&mut self) -> ParseResult<Expression> {
        let pos = self.current.position.clone();
        self.advance(); // consume 'match'
        
        let expr = self.parse_expression()?;
        self.expect(&TokenType::LeftBrace)?;
        
        let mut arms = Vec::new();
        
        while !self.check(&TokenType::RightBrace) && !self.is_at_end() {
            let pattern = self.parse_pattern()?;
            
            let guard = if self.check_keyword(KeywordType::KeywordIf) {
                self.advance();
                Some(self.parse_expression()?)
            } else {
                None
            };
            
            self.expect(&TokenType::Arrow)?;
            let body = self.parse_expression()?;
            
            arms.push(MatchArm {
                pattern,
                guard: guard.map(|g| *Box::new(g)),
                body,
            });
            
            // Optional comma or newline
            if self.check(&TokenType::Comma) {
                self.advance();
            }
        }
        
        self.expect(&TokenType::RightBrace)?;
        
        Ok(Expression::Match {
            expr: Box::new(expr),
            arms,
            pos,
        })
    }
    
    /// Parse pattern for match expressions
    fn parse_pattern(&mut self) -> ParseResult<Pattern> {
        // Wildcard pattern
        // Wildcard pattern - check for underscore identifier
        if self.check_identifier_value("_") {
            self.advance();
            return Ok(Pattern::Wildcard);
        }
        
        // Range pattern
        if let Ok(start) = self.try_parse_literal() {
            if self.check(&TokenType::InclusiveRange) {
                self.advance();
                let inclusive = !self.check(&TokenType::Less);
                if !inclusive {
                    self.advance();
                }
                let end = self.parse_primary()?;
                return Ok(Pattern::Range { start, end, inclusive });
            }
            return Ok(Pattern::Literal(start));
        }
        
        // Identifier or struct pattern
        match &self.current.token_type {
            TokenType::PublicIdentifier(name) | TokenType::PrivateIdentifier(name) => {
                let name = name.clone();
                self.advance();
            
            if self.check(&TokenType::LeftBrace) {
                self.advance();
                let mut fields = Vec::new();
                
                while !self.check(&TokenType::RightBrace) {
                    let field_name = self.expect_identifier()?;
                    self.expect(&TokenType::Colon)?;
                    let field_pattern = self.parse_pattern()?;
                    fields.push((field_name, field_pattern));
                    
                    if !self.check(&TokenType::RightBrace) {
                        self.expect(&TokenType::Comma)?;
                    }
                }
                
                self.expect(&TokenType::RightBrace)?;
                return Ok(Pattern::Struct { name, fields });
            }
            
                return Ok(Pattern::Identifier(name));
            }
            _ => {}
        }
        
        // Array pattern
        if self.check(&TokenType::LeftBracket) {
            self.advance();
            let mut patterns = Vec::new();
            
            while !self.check(&TokenType::RightBracket) {
                patterns.push(self.parse_pattern()?);
                if !self.check(&TokenType::RightBracket) {
                    self.expect(&TokenType::Comma)?;
                }
            }
            
            self.expect(&TokenType::RightBracket)?;
            return Ok(Pattern::Array(patterns));
        }
        
        Err(ParseError::InvalidPattern {
            message: "Expected pattern".to_string(),
            pos: self.current.position.clone(),
        })
    }
    
    /// Parse while loop
    fn parse_while(&mut self) -> ParseResult<Expression> {
        let pos = self.current.position.clone();
        self.advance(); // consume 'while'
        
        let condition = self.parse_expression()?;
        self.expect(&TokenType::LeftBrace)?;
        let body = self.parse_block_body()?;
        self.expect(&TokenType::RightBrace)?;
        
        Ok(Expression::While {
            condition: Box::new(condition),
            body: Box::new(body),
            pos,
        })
    }
    
    /// Parse for loop
    fn parse_for(&mut self) -> ParseResult<Expression> {
        let pos = self.current.position.clone();
        self.advance(); // consume 'for'
        
        let variable = self.expect_identifier()?;
        self.expect_keyword(KeywordType::KeywordIn)?;
        let iterable = self.parse_expression()?;
        self.expect(&TokenType::LeftBrace)?;
        let body = self.parse_block_body()?;
        self.expect(&TokenType::RightBrace)?;
        
        Ok(Expression::For {
            variable,
            iterable: Box::new(iterable),
            body: Box::new(body),
            pos,
        })
    }
    
    /// Parse break expression
    fn parse_break(&mut self) -> ParseResult<Expression> {
        let pos = self.current.position.clone();
        self.advance(); // consume 'break'
        
        // Check if there's a value to break with
        let value = if !self.is_end_of_expression() {
            Some(Box::new(self.parse_expression()?))
        } else {
            None
        };
        
        Ok(Expression::Break { value, pos })
    }
    
    /// Parse return expression
    fn parse_return(&mut self) -> ParseResult<Expression> {
        let pos = self.current.position.clone();
        self.advance(); // consume 'return'
        
        let value = if !self.is_end_of_expression() {
            Some(Box::new(self.parse_expression()?))
        } else {
            None
        };
        
        Ok(Expression::Return { value, pos })
    }
    
    /// Parse let binding
    fn parse_let(&mut self) -> ParseResult<Expression> {
        let pos = self.current.position.clone();
        self.advance(); // consume 'let'
        
        let name = self.expect_identifier()?;
        
        let type_annotation = if self.check(&TokenType::Colon) {
            self.advance();
            Some(self.parse_type()?)
        } else {
            None
        };
        
        self.expect(&TokenType::Assign)?;
        let value = self.parse_expression()?;
        
        Ok(Expression::Let {
            name,
            type_annotation,
            value: Box::new(value),
            is_mutable: false,
            pos,
        })
    }
    
    /// Parse var binding (mutable)
    fn parse_var(&mut self) -> ParseResult<Expression> {
        let pos = self.current.position.clone();
        self.advance(); // consume 'var'
        
        let name = self.expect_identifier()?;
        
        let type_annotation = if self.check(&TokenType::Colon) {
            self.advance();
            Some(self.parse_type()?)
        } else {
            None
        };
        
        self.expect(&TokenType::Assign)?;
        let value = self.parse_expression()?;
        
        Ok(Expression::Let {
            name,
            type_annotation,
            value: Box::new(value),
            is_mutable: true,
            pos,
        })
    }
    
    /// Parse function definition
    fn parse_function(&mut self) -> ParseResult<Expression> {
        let pos = self.current.position.clone();
        self.advance(); // consume 'fun'
        
        // Check for receiver syntax
        let receiver = if self.check(&TokenType::LeftParen) && self.is_receiver_syntax() {
            Some(self.parse_receiver()?)
        } else {
            None
        };
        
        let name = self.expect_identifier()?;
        
        self.expect(&TokenType::LeftParen)?;
        let params = self.parse_parameters()?;
        self.expect(&TokenType::RightParen)?;
        
        let return_type = if self.check(&TokenType::Colon) {
            self.advance();
            Some(self.parse_type()?)
        } else {
            None
        };
        
        self.expect(&TokenType::LeftBrace)?;
        let body = self.parse_block_body()?;
        self.expect(&TokenType::RightBrace)?;
        
        Ok(Expression::Function {
            name,
            params,
            return_type,
            body: Box::new(body),
            is_async: false,
            receiver,
            pos,
        })
    }
    
    /// Parse async function
    fn parse_async_function(&mut self) -> ParseResult<Expression> {
        let pos = self.current.position.clone();
        self.advance(); // consume 'async'
        self.expect_keyword(KeywordType::KeywordFun)?;
        
        // Check for receiver syntax
        let receiver = if self.check(&TokenType::LeftParen) && self.is_receiver_syntax() {
            Some(self.parse_receiver()?)
        } else {
            None
        };
        
        let name = self.expect_identifier()?;
        
        self.expect(&TokenType::LeftParen)?;
        let params = self.parse_parameters()?;
        self.expect(&TokenType::RightParen)?;
        
        let return_type = if self.check(&TokenType::Colon) {
            self.advance();
            Some(self.parse_type()?)
        } else {
            None
        };
        
        self.expect(&TokenType::LeftBrace)?;
        let body = self.parse_block_body()?;
        self.expect(&TokenType::RightBrace)?;
        
        Ok(Expression::Function {
            name,
            params,
            return_type,
            body: Box::new(body),
            is_async: true,
            receiver,
            pos,
        })
    }
    
    /// Parse lambda expression
    fn parse_lambda(&mut self) -> ParseResult<Expression> {
        let pos = self.current.position.clone();
        self.expect(&TokenType::LeftBrace)?;
        
        let params = if self.check(&TokenType::LeftParen) {
            // Typed parameters: { (x: Int, y: Int) -> ... }
            self.advance();
            let params = self.parse_parameters()?;
            self.expect(&TokenType::RightParen)?;
            params
        } else {
            // Simple parameters: { x, y -> ... }
            let mut params = Vec::new();
            params.push(Parameter {
                name: self.expect_identifier()?,
                type_annotation: None,
                default_value: None,
            });
            
            while self.check(&TokenType::Comma) {
                self.advance();
                params.push(Parameter {
                    name: self.expect_identifier()?,
                    type_annotation: None,
                    default_value: None,
                });
            }
            params
        };
        
        self.expect(&TokenType::Arrow)?;
        
        let return_type = if self.check_identifier_value("in") {
            self.advance();
            // Type comes before 'in'
            None // Return type parsed when full lambda syntax is implemented
        } else {
            None
        };
        
        let body = if self.check(&TokenType::LeftBrace) {
            self.parse_block()?
        } else {
            self.parse_expression()?
        };
        
        self.expect(&TokenType::RightBrace)?;
        
        Ok(Expression::Lambda {
            params,
            body: Box::new(body),
            return_type,
            pos,
        })
    }
    
    /// Parse block expression
    fn parse_block(&mut self) -> ParseResult<Expression> {
        let _pos = self.current.position.clone();
        self.expect(&TokenType::LeftBrace)?;
        let body = self.parse_block_body()?;
        self.expect(&TokenType::RightBrace)?;
        Ok(body)
    }
    
    /// Parse block body (returns last expression value)
    fn parse_block_body(&mut self) -> ParseResult<Expression> {
        let pos = self.current.position.clone();
        let mut expressions = Vec::new();
        
        while !self.check(&TokenType::RightBrace) && !self.is_at_end() {
            expressions.push(self.parse_expression()?);
            
            // Optional semicolon
            if self.check(&TokenType::Semicolon) {
                self.advance();
            }
        }
        
        if expressions.is_empty() {
            // Empty block returns null
            Ok(Expression::NullLiteral { pos })
        } else if expressions.len() == 1 {
            // Single expression
            Ok(expressions.into_iter().next().unwrap())
        } else {
            // Multiple expressions - block
            Ok(Expression::Block { expressions, pos })
        }
    }
    
    /// Parse array literal
    fn parse_array(&mut self) -> ParseResult<Expression> {
        let pos = self.current.position.clone();
        self.expect(&TokenType::LeftBracket)?;
        
        let mut elements = Vec::new();
        
        while !self.check(&TokenType::RightBracket) && !self.is_at_end() {
            elements.push(self.parse_expression()?);
            
            if !self.check(&TokenType::RightBracket) {
                self.expect(&TokenType::Comma)?;
            }
        }
        
        self.expect(&TokenType::RightBracket)?;
        
        Ok(Expression::ArrayLiteral { elements, pos })
    }
    
    /// Parse struct literal
    fn parse_struct_literal(&mut self, name: String) -> ParseResult<Expression> {
        let pos = self.current.position.clone();
        self.expect(&TokenType::LeftBrace)?;
        
        let mut fields = Vec::new();
        
        while !self.check(&TokenType::RightBrace) && !self.is_at_end() {
            let field_name = self.expect_identifier()?;
            self.expect(&TokenType::Colon)?;
            let field_value = self.parse_expression()?;
            fields.push((field_name, field_value));
            
            if !self.check(&TokenType::RightBrace) {
                self.expect(&TokenType::Comma)?;
            }
        }
        
        self.expect(&TokenType::RightBrace)?;
        
        Ok(Expression::StructLiteral { name, fields, pos })
    }
    
    /// Parse interpolated string
    fn parse_interpolated_string(&mut self) -> ParseResult<Expression> {
        let pos = self.current.position.clone();
        let parts = Vec::new();
        
        // The lexer should have already tokenized the interpolated string
        // We need to collect the interpolation parts
        self.advance(); // consume the interpolated string token
        
        // For now, return a simple string literal
        // String interpolation requires lexer support for embedded expressions
        Ok(Expression::InterpolatedString { parts, pos })
    }
    
    /// Parse function parameters
    fn parse_parameters(&mut self) -> ParseResult<Vec<Parameter>> {
        let mut params = Vec::new();
        
        while !self.check(&TokenType::RightParen) && !self.is_at_end() {
            let name = self.expect_identifier()?;
            
            let type_annotation = if self.check(&TokenType::Colon) {
                self.advance();
                Some(self.parse_type()?)
            } else {
                None
            };
            
            let default_value = if self.check(&TokenType::Assign) {
                self.advance();
                Some(self.parse_expression()?)
            } else {
                None
            };
            
            params.push(Parameter {
                name,
                type_annotation,
                default_value,
            });
            
            if !self.check(&TokenType::RightParen) {
                self.expect(&TokenType::Comma)?;
            }
        }
        
        Ok(params)
    }
    
    /// Parse function arguments
    fn parse_arguments(&mut self) -> ParseResult<Vec<Expression>> {
        let mut args = Vec::new();
        
        while !self.check(&TokenType::RightParen) && !self.is_at_end() {
            args.push(self.parse_expression()?);
            
            if !self.check(&TokenType::RightParen) {
                self.expect(&TokenType::Comma)?;
            }
        }
        
        Ok(args)
    }
    
    /// Parse receiver for method syntax
    fn parse_receiver(&mut self) -> ParseResult<Receiver> {
        self.expect(&TokenType::LeftParen)?;
        let name = self.expect_identifier()?;
        self.expect(&TokenType::Colon)?;
        
        let is_mutable = if self.check_keyword(KeywordType::KeywordInout) {
            self.advance();
            true
        } else {
            false
        };
        
        let type_name = self.expect_identifier()?;
        self.expect(&TokenType::RightParen)?;
        
        Ok(Receiver {
            name,
            type_name,
            is_mutable,
        })
    }
    
    /// Parse type annotation
    fn parse_type(&mut self) -> ParseResult<Type> {
        let base = self.expect_identifier()?;
        let is_nullable = if self.check(&TokenType::Question) {
            self.advance();
            true
        } else {
            false
        };
        
        // Generic parameters parsed when generics system is implemented
        
        Ok(Type {
            name: base,
            is_nullable,
            generics: Vec::new(),
        })
    }
    
    /// Try to parse a literal (for patterns)
    fn try_parse_literal(&mut self) -> ParseResult<Expression> {
        let pos = self.current.position.clone();
        
        match &self.current.token_type {
            TokenType::IntegerLiteral(value) => {
                let value = *value;
                self.advance();
                Ok(Expression::IntegerLiteral { value, pos })
            }
            TokenType::FloatLiteral(value) => {
                let value = *value;
                self.advance();
                Ok(Expression::FloatLiteral { value, pos })
            }
            TokenType::StringLiteral(value) => {
                let value = value.clone();
                self.advance();
                Ok(Expression::StringLiteral { value, pos })
            }
            _ => Err(ParseError::InvalidExpression { pos })
        }
    }
    
    // Helper methods
    
    fn advance(&mut self) {
        if !self.is_at_end() {
            self.current = if let Some(token) = self.peek_buffer.pop_front() {
                token
            } else {
                self.lexer.next_token().unwrap_or(Token {
                    token_type: TokenType::EOF,
                    lexeme: String::new(),
                    position: self.current.position.clone(),
                })
            };
        }
    }
    
    fn check(&self, token_type: &TokenType) -> bool {
        self.current.token_type == *token_type
    }
    
    fn check_keyword(&self, keyword: KeywordType) -> bool {
        matches!(&self.current.token_type, TokenType::Keyword(k) if *k == keyword)
    }
    
    fn check_identifier_value(&self, value: &str) -> bool {
        match &self.current.token_type {
            TokenType::PublicIdentifier(name) | TokenType::PrivateIdentifier(name) => {
                name == value
            }
            _ => false,
        }
    }
    
    fn expect(&mut self, token_type: &TokenType) -> ParseResult<()> {
        if self.check(token_type) {
            self.advance();
            Ok(())
        } else {
            Err(ParseError::UnexpectedToken {
                found: self.current.token_type.clone(),
                expected: format!("{:?}", token_type),
                pos: self.current.position.clone(),
            })
        }
    }
    
    fn expect_keyword(&mut self, keyword: KeywordType) -> ParseResult<()> {
        let keyword_str = format!("{:?}", keyword);
        if self.check_keyword(keyword) {
            self.advance();
            Ok(())
        } else {
            Err(ParseError::UnexpectedToken {
                found: self.current.token_type.clone(),
                expected: keyword_str,
                pos: self.current.position.clone(),
            })
        }
    }
    
    fn expect_identifier(&mut self) -> ParseResult<String> {
        match &self.current.token_type {
            TokenType::PublicIdentifier(name) | TokenType::PrivateIdentifier(name) => {
                let name = name.clone();
                self.advance();
                Ok(name)
            }
            _ => Err(ParseError::UnexpectedToken {
                found: self.current.token_type.clone(),
                expected: "identifier".to_string(),
                pos: self.current.position.clone(),
            })
        }
    }
    
    fn is_at_end(&self) -> bool {
        matches!(self.current.token_type, TokenType::EOF)
    }
    
    fn is_end_of_expression(&self) -> bool {
        matches!(
            self.current.token_type,
            TokenType::Semicolon | TokenType::RightBrace | TokenType::RightParen | TokenType::EOF
        )
    }
    
    fn is_lambda(&mut self) -> bool {
        // Look ahead to determine if this is a lambda
        // Lambda: { x -> ... } or { (x: Type) -> ... }
        let mut peek_count = 0;
        let mut tokens = Vec::new();
        
        // Collect tokens to check
        while peek_count < 5 && !self.is_at_end() {
            let token = if peek_count == 0 {
                self.current.clone()
            } else {
                let t = self.lexer.next_token().unwrap_or(Token {
                    token_type: TokenType::EOF,
                    lexeme: String::new(),
                    position: self.current.position.clone(),
                });
                tokens.push(t.clone());
                t
            };
            
            if matches!(token.token_type, TokenType::Arrow) {
                // Put tokens back
                for t in tokens.into_iter().rev() {
                    self.peek_buffer.push_back(t);
                }
                return true;
            }
            
            peek_count += 1;
        }
        
        // Put tokens back
        for t in tokens.into_iter().rev() {
            self.peek_buffer.push_back(t);
        }
        false
    }
    
    fn is_receiver_syntax(&mut self) -> bool {
        // Check if this is receiver syntax: (name: Type) or (name: inout Type)
        // Need to look ahead
        // Check for receiver syntax with lookahead
        let fun_keyword = self.lexer.get_keyword_text(&KeywordType::KeywordFun);
        if let Some(fun_text) = fun_keyword {
            if self.current.lexeme == fun_text {
                // Look ahead to check for method syntax
            let saved_current = self.current.clone();
            let saved_peek = self.peek_buffer.clone();
            self.advance();
                if self.current.token_type == TokenType::LeftParen {
                    // Could be a method - check for receiver
                    // Restore parser state
                    self.current = saved_current;
                    self.peek_buffer = saved_peek;
                    return true;
                }
                // Restore parser state
                self.current = saved_current;
                self.peek_buffer = saved_peek;
            }
        }
        false
    }
    
    fn match_equality_op(&self) -> Option<BinaryOperator> {
        match &self.current.token_type {
            TokenType::Equal => Some(BinaryOperator::Equal),
            TokenType::NotEqual => Some(BinaryOperator::NotEqual),
            _ => None,
        }
    }
    
    fn match_comparison_op(&self) -> Option<BinaryOperator> {
        match &self.current.token_type {
            TokenType::Less => Some(BinaryOperator::Less),
            TokenType::Greater => Some(BinaryOperator::Greater),
            TokenType::LessEqual => Some(BinaryOperator::LessEqual),
            TokenType::GreaterEqual => Some(BinaryOperator::GreaterEqual),
            _ => None,
        }
    }
    
    fn match_term_op(&self) -> Option<BinaryOperator> {
        match &self.current.token_type {
            TokenType::Plus => Some(BinaryOperator::Add),
            TokenType::Minus => Some(BinaryOperator::Subtract),
            _ => None,
        }
    }
    
    fn match_factor_op(&self) -> Option<BinaryOperator> {
        match &self.current.token_type {
            TokenType::Multiply => Some(BinaryOperator::Multiply),
            TokenType::Divide => Some(BinaryOperator::Divide),
            TokenType::Modulo => Some(BinaryOperator::Modulo),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use seen_lexer::KeywordManager;
    
    fn create_parser(input: &str) -> Parser {
        let keyword_manager = std::sync::Arc::new(KeywordManager::new());
        let lexer = Lexer::new(input.to_string(), keyword_manager);
        Parser::new(lexer)
    }
    
    #[test]
    fn test_parser_creation() {
        let parser = create_parser("42");
        assert!(!parser.is_at_end());
    }
    
    #[test]
    fn test_parse_integer() {
        let mut parser = create_parser("42");
        let expr = parser.parse_expression().unwrap();
        match expr {
            Expression::IntegerLiteral { value, .. } => assert_eq!(value, 42),
            _ => panic!("Expected integer literal"),
        }
    }
}