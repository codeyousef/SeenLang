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
        let mut current = lexer.next_token().unwrap_or(Token {
            token_type: TokenType::EOF,
            lexeme: String::new(),
            position: Position::new(0, 0, 0),
        });
        
        // Skip initial newlines
        while matches!(current.token_type, TokenType::Newline) {
            current = lexer.next_token().unwrap_or(Token {
                token_type: TokenType::EOF,
                lexeme: String::new(),
                position: Position::new(0, 0, 0),
            });
            if matches!(current.token_type, TokenType::EOF) {
                break;
            }
        }
        
        Self {
            lexer,
            current,
            peek_buffer: VecDeque::new(),
        }
    }
    
    /// Parse a complete program
    pub fn parse_program(&mut self) -> ParseResult<Program> {
        let mut expressions = Vec::new();
        
        // Skip leading whitespace/newlines
        self.skip_whitespace();
        
        while !self.is_at_end() {
            // Skip any newlines at the top level
            while self.check(&TokenType::Newline) {
                self.advance();
            }
            
            // Check again if we're at the end after skipping newlines
            if self.is_at_end() {
                break;
            }
            
            // Parse top-level items (functions, statements, expressions)
            expressions.push(self.parse_top_level_item()?);
            
            // Optional semicolons or newlines between top-level expressions
            while self.check(&TokenType::Newline) {
                self.advance();
            }
            
            // Skip whitespace/newlines between expressions
            self.skip_whitespace();
        }
        
        Ok(Program { expressions })
    }
    
    /// Parse a top-level item (function, statement, or expression)
    fn parse_top_level_item(&mut self) -> ParseResult<Expression> {
        // Check for function definitions
        if self.check_keyword(KeywordType::KeywordFun) {
            return self.parse_function();
        }
        
        // Check for async functions
        if self.check_keyword(KeywordType::KeywordAsync) {
            return self.parse_async_function();
        }
        
        // Check for struct definitions
        if self.check_keyword(KeywordType::KeywordStruct) {
            return self.parse_struct_definition();
        }
        
        // Check for class definitions
        if self.check_keyword(KeywordType::KeywordClass) {
            return self.parse_class_definition();
        }
        
        // Otherwise, parse as an expression
        self.parse_expression()
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
        
        // FIXED: Use TokenType::LogicalOr instead of KeywordType::KeywordOr
        while self.check(&TokenType::LogicalOr) {
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
        
        // FIXED: Use TokenType::LogicalAnd instead of KeywordType::KeywordAnd
        while self.check(&TokenType::LogicalAnd) {
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
        // FIXED: Check for 'not' as TokenType::LogicalNot instead of KeywordType::KeywordNot
        if self.check(&TokenType::LogicalNot) {
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
                TokenType::LeftBrace => {
                    // Check if this is a struct literal (only for type identifiers)
                    if let Expression::Identifier { name, is_public, pos } = &expr {
                        // Type names start with uppercase, so check if this is a type
                        if *is_public || name.chars().next().map_or(false, |c| c.is_uppercase()) {
                            let struct_name = name.clone();
                            let struct_expr = self.parse_struct_literal(struct_name)?;
                            expr = struct_expr;
                            continue;
                        }
                    }
                    // Not a struct literal, so stop parsing postfix
                    break;
                }
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
            let expr = self.parse_unary()?; // Call next level down to avoid recursion
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
        if let TokenType::BoolLiteral(value) = &self.current.token_type {
            let value = *value;
            self.advance();
            return Ok(Expression::BooleanLiteral { value, pos });
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
        
        if self.check_keyword(KeywordType::KeywordLoop) {
            return self.parse_loop();
        }
        
        // Concurrency keywords
        if self.check_keyword(KeywordType::KeywordSpawn) {
            return self.parse_spawn();
        }
        
        if self.check_keyword(KeywordType::KeywordSelect) {
            return self.parse_select();
        }
        
        if self.check_keyword(KeywordType::KeywordActor) {
            return self.parse_actor();
        }
        
        // Memory management
        if self.check_keyword(KeywordType::KeywordRegion) {
            return self.parse_region();
        }
        
        if self.check_keyword(KeywordType::KeywordArena) {
            return self.parse_arena();
        }
        
        // Metaprogramming
        if self.check_keyword(KeywordType::KeywordComptime) {
            return self.parse_comptime();
        }
        
        if self.check_keyword(KeywordType::KeywordMacro) {
            return self.parse_macro();
        }
        
        // Effects
        if self.check_keyword(KeywordType::KeywordEffect) {
            return self.parse_effect();
        }
        
        if self.check_keyword(KeywordType::KeywordHandle) {
            return self.parse_handle();
        }
        
        // Error handling
        if self.check_keyword(KeywordType::KeywordDefer) {
            return self.parse_defer();
        }
        
        if self.check_keyword(KeywordType::KeywordAssert) {
            return self.parse_assert();
        }
        
        if self.check_keyword(KeywordType::KeywordTry) {
            return self.parse_try();
        }
        
        // OOP
        if self.check_keyword(KeywordType::KeywordExtension) {
            return self.parse_extension();
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
        
        // Async functions
        if self.check_keyword(KeywordType::KeywordAsync) {
            return self.parse_async_function();
        }
        
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
        
        // Identifiers
        if let TokenType::PublicIdentifier(name) = &self.current.token_type {
            let name = name.clone();
            let is_public = true;
            self.advance();
            return Ok(Expression::Identifier { name, is_public, pos });
        }
        
        if let TokenType::PrivateIdentifier(name) = &self.current.token_type {
            let name = name.clone();
            let is_public = false;
            self.advance();
            return Ok(Expression::Identifier { name, is_public, pos });
        }
        
        // Keywords used as identifiers in expressions
        if let TokenType::Keyword(keyword) = &self.current.token_type {
            let name = match keyword {
                KeywordType::KeywordData => "data".to_string(),
                // Add other keywords as needed
                _ => return Err(ParseError::UnexpectedToken {
                    found: self.current.token_type.clone(),
                    expected: "identifier".to_string(),
                    pos: self.current.position.clone(),
                }),
            };
            let is_public = false; // Keywords used as identifiers are treated as private
            self.advance();
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
                guard,
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
        
        // Try to parse literal patterns (including bool and null)
        if let Ok(start) = self.try_parse_pattern_literal() {
            if self.check(&TokenType::InclusiveRange) {
                self.advance();
                let inclusive = !self.check(&TokenType::Less);
                if !inclusive {
                    self.advance();
                }
                let end = self.parse_primary()?;
                return Ok(Pattern::Range { start: Box::new(start), end: Box::new(end), inclusive });
            }
            return Ok(Pattern::Literal(Box::new(start)));
        }
        
        // Identifier or struct pattern (including keywords used as identifiers)
        match &self.current.token_type {
            TokenType::PublicIdentifier(name) | TokenType::PrivateIdentifier(name) => {
                let name = name.clone();
                self.advance();
                
                if self.check(&TokenType::LeftBrace) {
                    // Struct pattern: Name { field: pattern, ... }
                    self.advance();
                    let mut fields = Vec::new();
                    
                    while !self.check(&TokenType::RightBrace) {
                        let field_name = self.parse_field_name()?;
                        self.expect(&TokenType::Colon)?;
                        let field_pattern = self.parse_pattern()?;
                        fields.push((field_name, Box::new(field_pattern)));
                        
                        if !self.check(&TokenType::RightBrace) {
                            self.expect(&TokenType::Comma)?;
                        }
                    }
                    
                    self.expect(&TokenType::RightBrace)?;
                    return Ok(Pattern::Struct { name, fields });
                }
                
                return Ok(Pattern::Identifier(name));
            }
            // Keywords can also be used as pattern identifiers
            TokenType::Keyword(keyword) => {
                let name = match keyword {
                    KeywordType::KeywordData => "data".to_string(),
                    // Add other keywords as needed for pattern matching
                    _ => format!("{:?}", keyword), // Fallback to debug representation
                };
                self.advance();
                return Ok(Pattern::Identifier(name));
            }
            _ => {}
        }
        
        // Array pattern
        if self.check(&TokenType::LeftBracket) {
            self.advance();
            let mut patterns = Vec::new();
            
            while !self.check(&TokenType::RightBracket) {
                patterns.push(Box::new(self.parse_pattern()?));
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
        
        // Parse generic parameters if present
        let _generic_params = if self.check(&TokenType::Less) {
            self.advance(); // consume '<'
            let mut generics = Vec::new();
            
            while !self.check(&TokenType::Greater) && !self.is_at_end() {
                generics.push(self.expect_identifier()?);
                
                if self.check(&TokenType::Comma) {
                    self.advance();
                } else if !self.check(&TokenType::Greater) {
                    return Err(ParseError::UnexpectedToken {
                        expected: "comma or >".to_string(),
                        found: self.current.token_type.clone(),
                        pos: self.current.position.clone(),
                    });
                }
            }
            
            self.expect(&TokenType::Greater)?;
            generics
        } else {
            Vec::new()
        };
        
        self.expect(&TokenType::LeftParen)?;
        let params = self.parse_parameters()?;
        self.expect(&TokenType::RightParen)?;
        
        let return_type = if self.check(&TokenType::Arrow) {
            self.advance();
            Some(self.parse_type()?)
        } else if self.check(&TokenType::Colon) {
            // Support both : and -> for backward compatibility
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
        
        let return_type = if self.check(&TokenType::Arrow) {
            self.advance();
            Some(self.parse_type()?)
        } else if self.check(&TokenType::Colon) {
            // Support both : and -> for backward compatibility
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
        // We should be at the { token when this is called
        if !self.check(&TokenType::LeftBrace) {
            return Err(ParseError::UnexpectedToken {
                found: self.current.token_type.clone(),
                expected: "'{' for lambda".to_string(),
                pos: self.current.position.clone(),
            });
        }
        self.advance(); // consume '{'
        
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
                memory_modifier: None,
            });
            
            while self.check(&TokenType::Comma) {
                self.advance();
                params.push(Parameter {
                    name: self.expect_identifier()?,
                    type_annotation: None,
                    default_value: None,
                    memory_modifier: None,
                });
            }
            params
        };
        
        self.expect(&TokenType::Arrow)?;
        
        // Look for return type syntax: Type followed by 'in' keyword  
        let return_type = if matches!(self.current.token_type, TokenType::PublicIdentifier(_) | TokenType::PrivateIdentifier(_)) {
            // Use simple lookahead to check for return type pattern without consuming tokens
            let mut lookahead_pos = 0;
            
            // Skip the type identifier(s) - could be complex like HashMap<String, Int>
            if self.is_type_at_lookahead(lookahead_pos) {
                // Find where the type ends by looking for 'in' keyword
                let mut found_in = false;
                for i in 1..10 { // Look ahead up to 10 tokens
                    match self.peek_ahead(i) {
                        Some(token) if matches!(token.token_type, TokenType::Keyword(KeywordType::KeywordIn)) => {
                            found_in = true;
                            break;
                        }
                        Some(_) => continue,
                        None => break,
                    }
                }
                
                if found_in {
                    // Parse return type and consume 'in'
                    let type_annotation = self.parse_type()?;
                    self.expect_keyword(KeywordType::KeywordIn)?;
                    Some(type_annotation)
                } else {
                    // No 'in' found, this is lambda body
                    None
                }
            } else {
                // Not a valid type, this is lambda body
                None
            }
        } else if self.check_keyword(KeywordType::KeywordIn) {
            // No return type, just 'in' keyword
            self.advance();
            None
        } else {
            // No return type, no 'in' keyword (simple lambda)
            None
        };
        
        // Parse lambda body which can be multiple statements until the closing brace
        let body = self.parse_lambda_body()?;
        
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
            // Skip any leading newlines
            while self.check(&TokenType::Newline) {
                self.advance();
            }
            
            // Check if we've reached the end after skipping newlines
            if self.check(&TokenType::RightBrace) || self.is_at_end() {
                break;
            }
            
            expressions.push(self.parse_expression()?);
            
            // Handle statement terminators (semicolons and newlines)
            while self.check(&TokenType::Newline) {
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
        
        // Extract the parts from the lexer's InterpolatedString token
        let parts = if let TokenType::InterpolatedString(lexer_parts) = &self.current.token_type {
            // Convert lexer InterpolationPart to parser InterpolationPart
            lexer_parts.iter().map(|lexer_part| -> ParseResult<InterpolationPart> {
                match &lexer_part.kind {
                    seen_lexer::InterpolationKind::Text(text) => {
                        Ok(InterpolationPart {
                            kind: InterpolationKind::Text(text.clone()),
                            pos: lexer_part.position.clone(),
                        })
                    }
                    seen_lexer::InterpolationKind::Expression(expr_str) => {
                        // Parse the expression string into an actual Expression
                        let expr = self.parse_interpolation_expression(expr_str, lexer_part.position.clone())?;
                        
                        Ok(InterpolationPart {
                            kind: InterpolationKind::Expression(Box::new(expr)),
                            pos: lexer_part.position.clone(),
                        })
                    }
                    seen_lexer::InterpolationKind::LiteralBrace => {
                        // Treat literal braces as text
                        Ok(InterpolationPart {
                            kind: InterpolationKind::Text("{".to_string()),
                            pos: lexer_part.position.clone(),
                        })
                    }
                }
            }).collect::<ParseResult<Vec<_>>>()?
        } else {
            Vec::new()
        };
        
        self.advance(); // consume the interpolated string token
        
        Ok(Expression::InterpolatedString { parts, pos })
    }
    
    /// Parse an expression within string interpolation
    fn parse_interpolation_expression(&self, expr_str: &str, pos: Position) -> ParseResult<Expression> {
        // Create a sub-lexer for the expression string
        let keyword_manager = self.lexer.keyword_manager();
        let sub_lexer = seen_lexer::Lexer::new(expr_str.to_string(), keyword_manager);
        
        // Create a sub-parser
        let mut sub_parser = Parser::new(sub_lexer);
        
        // Parse the expression
        let expr = sub_parser.parse_expression().map_err(|_e| {
            // Adjust error position to be relative to the original string
            ParseError::UnexpectedToken {
                found: TokenType::EOF, // Use a token type instead of string
                expected: "valid expression".to_string(),
                pos,
            }
        })?;
        
        // Ensure we've consumed the entire expression string
        if !sub_parser.is_at_end() {
            return Err(ParseError::UnexpectedToken {
                found: TokenType::EOF, // Use a token type instead of string
                expected: "end of expression".to_string(),
                pos,
            });
        }
        
        Ok(expr)
    }
    
    /// Parse function parameters
    fn parse_parameters(&mut self) -> ParseResult<Vec<Parameter>> {
        let mut params = Vec::new();
        
        while !self.check(&TokenType::RightParen) && !self.is_at_end() {
            // Parse memory management modifier if present
            let memory_modifier = self.parse_memory_modifier()?;
            
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
                memory_modifier,
            });
            
            if !self.check(&TokenType::RightParen) {
                self.expect(&TokenType::Comma)?;
            }
        }
        
        Ok(params)
    }
    
    /// Parse memory management modifier for parameters
    fn parse_memory_modifier(&mut self) -> ParseResult<Option<MemoryModifier>> {
        if self.check(&TokenType::Move) {
            self.advance();
            Ok(Some(MemoryModifier::Move))
        } else if self.check(&TokenType::Borrow) {
            self.advance();
            Ok(Some(MemoryModifier::Borrow))
        } else if self.check_keyword(KeywordType::KeywordMut) {
            self.advance();
            Ok(Some(MemoryModifier::Mut))
        } else if self.check(&TokenType::Inout) {
            self.advance();
            Ok(Some(MemoryModifier::Inout))
        } else {
            Ok(None)
        }
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
        
        // Check for memory management keywords: inout, mut, borrow
        let is_mutable = if self.check(&TokenType::Inout) {
            self.advance();
            true
        } else if self.check_keyword(KeywordType::KeywordMut) {
            self.advance();
            true
        } else if self.check(&TokenType::Borrow) {
            self.advance();
            // Handle 'borrow mut' pattern
            if self.check_keyword(KeywordType::KeywordMut) {
                self.advance();
                true
            } else {
                false // immutable borrow
            }
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
        
        // Parse generic parameters
        let mut generics = Vec::new();
        if self.check(&TokenType::Less) {
            self.advance(); // consume '<'
            
            while !self.check(&TokenType::Greater) && !self.is_at_end() {
                generics.push(self.parse_type()?);
                
                if self.check(&TokenType::Comma) {
                    self.advance();
                } else if !self.check(&TokenType::Greater) {
                    return Err(ParseError::UnexpectedToken {
                        expected: "comma or >".to_string(),
                        found: self.current.token_type.clone(),
                        pos: self.current.position.clone(),
                    });
                }
            }
            
            self.expect(&TokenType::Greater)?;
        }
        
        let is_nullable = if self.check(&TokenType::Question) {
            self.advance();
            true
        } else {
            false
        };
        
        Ok(Type {
            name: base,
            is_nullable,
            generics,
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
    
    /// Try to parse all literal types including boolean and null (for pattern matching)
    fn try_parse_pattern_literal(&mut self) -> ParseResult<Expression> {
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
            TokenType::BoolLiteral(value) => {
                let value = *value;
                self.advance();
                Ok(Expression::BooleanLiteral { value, pos })
            }
            TokenType::Keyword(KeywordType::KeywordNull) => {
                self.advance();
                Ok(Expression::NullLiteral { pos })
            }
            _ => Err(ParseError::InvalidExpression { pos })
        }
    }
    
    // Helper methods
    
    fn advance(&mut self) {
        if !self.is_at_end() {
            loop {
                self.current = if let Some(token) = self.peek_buffer.pop_front() {
                    token
                } else {
                    self.lexer.next_token().unwrap_or(Token {
                        token_type: TokenType::EOF,
                        lexeme: String::new(),
                        position: self.current.position.clone(),
                    })
                };
                
                // Skip newline tokens automatically
                if !matches!(self.current.token_type, TokenType::Newline) {
                    break;
                }
                
                // If we hit EOF, don't continue the loop
                if matches!(self.current.token_type, TokenType::EOF) {
                    break;
                }
            }
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
    
    /// Parse field name that can be identifier or keyword (for struct patterns)
    fn parse_field_name(&mut self) -> ParseResult<String> {
        match &self.current.token_type {
            TokenType::PublicIdentifier(name) | TokenType::PrivateIdentifier(name) => {
                let name = name.clone();
                self.advance();
                Ok(name)
            }
            TokenType::Keyword(keyword) => {
                // Allow keywords as field names in struct patterns
                let name = match keyword {
                    KeywordType::KeywordData => "data".to_string(),
                    KeywordType::KeywordType => "type".to_string(),
                    KeywordType::KeywordClass => "class".to_string(),
                    KeywordType::KeywordStruct => "struct".to_string(),
                    KeywordType::KeywordEnum => "enum".to_string(),
                    KeywordType::KeywordTrait => "trait".to_string(),
                    // Add more as needed
                    _ => format!("{:?}", keyword).to_lowercase()
                        .replace("keyword", "")
                        .replace("_", ""),
                };
                self.advance();
                Ok(name)
            }
            _ => Err(ParseError::UnexpectedToken {
                found: self.current.token_type.clone(),
                expected: "field name".to_string(),
                pos: self.current.position.clone(),
            })
        }
    }
    
    fn is_at_end(&self) -> bool {
        matches!(self.current.token_type, TokenType::EOF)
    }
    
    fn skip_whitespace(&mut self) {
        while self.check(&TokenType::Newline) {
            self.advance();
        }
    }
    
    fn is_end_of_expression(&self) -> bool {
        matches!(
            self.current.token_type,
            TokenType::RightBrace | TokenType::RightParen | TokenType::EOF
        )
    }
    
    fn is_lambda(&mut self) -> bool {
        // Look ahead to determine if this is a lambda
        // Lambda: { x -> ... } or { (x: Type) -> ... } or { (x, y) -> ... }
        // Blocks don't have arrow operators
        
        if !self.check(&TokenType::LeftBrace) {
            return false;
        }
        
        // Simple approach: save current state and look ahead without corrupting state
        // We'll use the existing peek_buffer mechanism
        let mut lookahead_tokens = Vec::new();
        
        // Look ahead to find arrow token within reasonable distance  
        for _ in 0..15 {
            // Get next token but save it for restoration
            if let Ok(token) = self.lexer.next_token() {
                match &token.token_type {
                    TokenType::Arrow => {
                        // Found arrow - put all tokens back and return true
                        lookahead_tokens.push(token);
                        for t in lookahead_tokens.into_iter().rev() {
                            self.peek_buffer.push_front(t);
                        }
                        return true;
                    }
                    TokenType::RightBrace | TokenType::EOF => {
                        // No arrow before closing - put tokens back and return false
                        lookahead_tokens.push(token);
                        for t in lookahead_tokens.into_iter().rev() {
                            self.peek_buffer.push_front(t);
                        }
                        return false;
                    }
                    _ => {
                        lookahead_tokens.push(token);
                    }
                }
            } else {
                break;
            }
        }
        
        // Put all tokens back - no arrow found
        for token in lookahead_tokens.into_iter().rev() {
            self.peek_buffer.push_front(token);
        }
        
        false
    }
    
    fn parse_lambda_body(&mut self) -> ParseResult<Expression> {
        // Parse statements until we reach the closing brace
        let mut expressions = Vec::new();
        
        while !self.check(&TokenType::RightBrace) && !self.is_at_end() {
            let expr = self.parse_expression()?;
            expressions.push(expr);
        }
        
        // If there's only one expression, return it directly
        // If there are multiple, wrap in a block
        match expressions.len() {
            0 => Err(ParseError::UnexpectedToken {
                found: self.current.token_type.clone(),
                expected: "expression".to_string(),
                pos: self.current.position.clone(),
            }),
            1 => Ok(expressions.into_iter().next().unwrap()),
            _ => Ok(Expression::Block {
                expressions,
                pos: self.current.position.clone(),
            }),
        }
    }
    
    fn is_receiver_syntax(&mut self) -> bool {
        // Simple heuristic: if we see ( followed by identifier : identifier ), it's likely a receiver
        if !self.check(&TokenType::LeftParen) {
            return false;
        }
        
        // For now, if we see a LeftParen right after 'fun', assume it's receiver syntax
        // This is a simple heuristic that avoids complex lookahead
        true
    }
    
    /// Look ahead at token at position i (0 = current token, 1 = next token, etc.)
    fn peek_ahead(&mut self, distance: usize) -> Option<Token> {
        if distance == 0 {
            return Some(self.current.clone());
        }
        
        // Fill peek buffer as needed
        while self.peek_buffer.len() < distance {
            if let Ok(token) = self.lexer.next_token() {
                self.peek_buffer.push_back(token);
            } else {
                return None; // EOF
            }
        }
        
        self.peek_buffer.get(distance - 1).cloned()
    }
    
    /// Check if the token at given lookahead position could be a type
    fn is_type_at_lookahead(&mut self, _distance: usize) -> bool {
        // For now, simple check - any identifier can be a type
        matches!(self.current.token_type, TokenType::PublicIdentifier(_) | TokenType::PrivateIdentifier(_))
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
    
    // New parsing methods for advanced features
    
    fn parse_loop(&mut self) -> ParseResult<Expression> {
        let pos = self.current.position.clone();
        self.advance(); // consume 'loop'
        let body = Box::new(self.parse_block()?);
        Ok(Expression::Loop { body, pos })
    }
    
    fn parse_spawn(&mut self) -> ParseResult<Expression> {
        let pos = self.current.position.clone();
        self.advance(); // consume 'spawn'
        let expr = Box::new(self.parse_expression()?);
        Ok(Expression::Spawn { expr, pos })
    }
    
    fn parse_select(&mut self) -> ParseResult<Expression> {
        let pos = self.current.position.clone();
        self.advance(); // consume 'select'
        self.expect(&TokenType::LeftBrace)?;
        
        let mut cases = Vec::new();
        while !self.check(&TokenType::RightBrace) {
            // Parse select case (simplified for now)
            let channel = Box::new(self.parse_expression()?);
            self.expect(&TokenType::Arrow)?;
            let pattern = self.parse_pattern()?;
            self.expect(&TokenType::FatArrow)?;
            let handler = Box::new(self.parse_expression()?);
            
            cases.push(SelectCase { channel, pattern, handler });
            
            if !self.check(&TokenType::RightBrace) {
                self.expect(&TokenType::Comma)?;
            }
        }
        
        self.expect(&TokenType::RightBrace)?;
        Ok(Expression::Select { cases, pos })
    }
    
    fn parse_actor(&mut self) -> ParseResult<Expression> {
        let pos = self.current.position.clone();
        self.advance(); // consume 'actor'
        let name = self.expect_identifier()?;
        
        self.expect(&TokenType::LeftBrace)?;
        let mut fields = Vec::new();
        let mut handlers = Vec::new();
        
        // Parse fields and handlers (simplified)
        while !self.check(&TokenType::RightBrace) {
            // This is a simplified implementation
            if self.check_keyword(KeywordType::KeywordReceive) {
                // Parse message handler
                self.advance();
                let message_type = self.expect_identifier()?;
                self.expect(&TokenType::LeftParen)?;
                let params = self.parse_parameters()?;
                self.expect(&TokenType::RightParen)?;
                self.expect(&TokenType::FatArrow)?;
                let body = Box::new(self.parse_expression()?);
                
                handlers.push(MessageHandler {
                    message_type,
                    params,
                    body,
                });
            } else {
                // Parse field
                let field_name = self.expect_identifier()?;
                self.expect(&TokenType::Colon)?;
                let field_type = self.parse_type()?;
                fields.push((field_name, field_type));
            }
            
            if !self.check(&TokenType::RightBrace) {
                self.expect(&TokenType::Comma)?;
            }
        }
        
        self.expect(&TokenType::RightBrace)?;
        Ok(Expression::Actor { name, fields, handlers, pos })
    }
    
    fn parse_send(&mut self) -> ParseResult<Expression> {
        let pos = self.current.position.clone();
        self.advance(); // consume 'send'
        let message = Box::new(self.parse_expression()?);
        // Expect 'to' keyword - but we don't have it defined yet, so parse it as identifier
        // TODO: Add 'to' keyword to the language
        if let TokenType::PrivateIdentifier(s) = &self.current.token_type {
            if s == "to" {
                self.advance();
            } else {
                return Err(ParseError::UnexpectedToken {
                    expected: "keyword 'to'".to_string(),
                    found: self.current.token_type.clone(),
                    pos: self.current.position.clone(),
                });
            }
        }
        let target = Box::new(self.parse_expression()?);
        Ok(Expression::Send { message, target, pos })
    }
    
    fn parse_receive(&mut self) -> ParseResult<Expression> {
        let pos = self.current.position.clone();
        self.advance(); // consume 'receive'
        let pattern = self.parse_pattern()?;
        self.expect(&TokenType::FatArrow)?;
        let handler = Box::new(self.parse_expression()?);
        Ok(Expression::Receive { pattern, handler, pos })
    }
    
    fn parse_region(&mut self) -> ParseResult<Expression> {
        let pos = self.current.position.clone();
        self.advance(); // consume 'region'
        
        let name = if self.check(&TokenType::LeftBrace) {
            None
        } else {
            Some(self.expect_identifier()?)
        };
        
        let body = Box::new(self.parse_block()?);
        Ok(Expression::Region { name, body, pos })
    }
    
    fn parse_arena(&mut self) -> ParseResult<Expression> {
        let pos = self.current.position.clone();
        self.advance(); // consume 'arena'
        let body = Box::new(self.parse_block()?);
        Ok(Expression::Arena { body, pos })
    }
    
    fn parse_comptime(&mut self) -> ParseResult<Expression> {
        let pos = self.current.position.clone();
        self.advance(); // consume 'comptime'
        let body = Box::new(self.parse_block()?);
        Ok(Expression::Comptime { body, pos })
    }
    
    fn parse_macro(&mut self) -> ParseResult<Expression> {
        let pos = self.current.position.clone();
        self.advance(); // consume 'macro'
        let name = self.expect_identifier()?;
        
        self.expect(&TokenType::LeftParen)?;
        let mut params = Vec::new();
        while !self.check(&TokenType::RightParen) {
            params.push(self.expect_identifier()?);
            if !self.check(&TokenType::RightParen) {
                self.expect(&TokenType::Comma)?;
            }
        }
        self.expect(&TokenType::RightParen)?;
        
        let body = Box::new(self.parse_block()?);
        Ok(Expression::Macro { name, params, body, pos })
    }
    
    fn parse_effect(&mut self) -> ParseResult<Expression> {
        let pos = self.current.position.clone();
        self.advance(); // consume 'effect'
        let name = self.expect_identifier()?;
        
        self.expect(&TokenType::LeftBrace)?;
        let mut operations = Vec::new();
        
        while !self.check(&TokenType::RightBrace) {
            let op_name = self.expect_identifier()?;
            self.expect(&TokenType::LeftParen)?;
            let params = self.parse_parameters()?;
            self.expect(&TokenType::RightParen)?;
            
            let return_type = if self.check(&TokenType::Colon) {
                self.advance();
                Some(self.parse_type()?)
            } else {
                None
            };
            
            operations.push(EffectOperation { name: op_name, params, return_type });
            
            if !self.check(&TokenType::RightBrace) {
                self.expect(&TokenType::Comma)?;
            }
        }
        
        self.expect(&TokenType::RightBrace)?;
        Ok(Expression::Effect { name, operations, pos })
    }
    
    fn parse_handle(&mut self) -> ParseResult<Expression> {
        let pos = self.current.position.clone();
        self.advance(); // consume 'handle'
        let body = Box::new(self.parse_expression()?);
        self.expect_keyword(KeywordType::KeywordWith)?;
        let effect = self.expect_identifier()?;
        
        self.expect(&TokenType::LeftBrace)?;
        let mut handlers = Vec::new();
        
        while !self.check(&TokenType::RightBrace) {
            let operation = self.expect_identifier()?;
            self.expect(&TokenType::LeftParen)?;
            let params = self.parse_parameters()?;
            self.expect(&TokenType::RightParen)?;
            self.expect(&TokenType::FatArrow)?;
            let handler_body = Box::new(self.parse_expression()?);
            
            handlers.push(EffectHandler {
                operation,
                params,
                body: handler_body,
            });
            
            if !self.check(&TokenType::RightBrace) {
                self.expect(&TokenType::Comma)?;
            }
        }
        
        self.expect(&TokenType::RightBrace)?;
        Ok(Expression::Handle { body, effect, handlers, pos })
    }
    
    fn parse_defer(&mut self) -> ParseResult<Expression> {
        let pos = self.current.position.clone();
        self.advance(); // consume 'defer'
        let body = Box::new(self.parse_expression()?);
        Ok(Expression::Defer { body, pos })
    }
    
    fn parse_struct_definition(&mut self) -> ParseResult<Expression> {
        let pos = self.current.position.clone();
        self.advance(); // consume 'struct'
        
        let name = self.expect_identifier()?;
        self.expect(&TokenType::LeftBrace)?;
        
        let mut fields = Vec::new();
        
        while !self.check(&TokenType::RightBrace) {
            // Skip any leading newlines
            while self.check(&TokenType::Newline) {
                self.advance();
            }
            
            // Check for end of struct after skipping newlines
            if self.check(&TokenType::RightBrace) {
                break;
            }
            
            let field_name = self.expect_identifier()?;
            let is_public = field_name.chars().next().map_or(false, |c| c.is_uppercase());
            
            self.expect(&TokenType::Colon)?;
            let field_type = self.parse_type()?;
            
            fields.push(crate::ast::StructField {
                name: field_name,
                field_type,
                is_public,
            });
            
            // Allow optional comma or newline between fields
            if self.check(&TokenType::Comma) {
                self.advance();
            }
            // Newlines are handled at the start of the loop
        }
        
        self.expect(&TokenType::RightBrace)?;
        
        Ok(Expression::StructDefinition { name, fields, pos })
    }
    
    fn parse_class_definition(&mut self) -> ParseResult<Expression> {
        let pos = self.current.position.clone();
        self.advance(); // consume 'class'
        
        let name = self.expect_identifier()?;
        
        // Check for superclass
        let superclass = if self.check(&TokenType::Colon) {
            self.advance();
            Some(self.expect_identifier()?)
        } else {
            None
        };
        
        self.expect(&TokenType::LeftBrace)?;
        
        let mut fields = Vec::new();
        let mut methods = Vec::new();
        
        while !self.check(&TokenType::RightBrace) {
            // Skip any leading newlines
            while self.check(&TokenType::Newline) {
                self.advance();
            }
            
            // Check for end of class after skipping newlines
            if self.check(&TokenType::RightBrace) {
                break;
            }
            
            // Check if it's a method (starts with 'fun')
            if self.check_keyword(KeywordType::KeywordFun) {
                methods.push(self.parse_method()?);
            } else {
                // It's a field
                fields.push(self.parse_class_field()?);
            }
        }
        
        self.expect(&TokenType::RightBrace)?;
        
        Ok(Expression::ClassDefinition { 
            name, 
            superclass, 
            fields, 
            methods, 
            pos 
        })
    }
    
    fn parse_class_field(&mut self) -> ParseResult<crate::ast::ClassField> {
        let is_mutable = if self.check_keyword(KeywordType::KeywordVar) {
            self.advance();
            true
        } else if self.check_keyword(KeywordType::KeywordLet) {
            self.advance();
            false
        } else {
            false // Default to immutable
        };
        
        let name = self.expect_identifier()?;
        let is_public = name.chars().next().map_or(false, |c| c.is_uppercase());
        
        self.expect(&TokenType::Colon)?;
        let field_type = self.parse_type()?;
        
        let default_value = if self.check(&TokenType::Assign) {
            self.advance();
            Some(self.parse_expression()?)
        } else {
            None
        };
        
        Ok(crate::ast::ClassField {
            name,
            field_type,
            is_public,
            is_mutable,
            default_value,
        })
    }
    
    fn parse_method(&mut self) -> ParseResult<crate::ast::Method> {
        let pos = self.current.position.clone();
        self.advance(); // consume 'fun'
        
        // Check if it's a method with receiver syntax: fun (receiver: Type) MethodName(...)
        let (receiver, method_name) = if self.check(&TokenType::LeftParen) {
            // Method with receiver
            self.advance(); // consume '('
            let receiver_name = self.expect_identifier()?;
            self.expect(&TokenType::Colon)?;
            let receiver_type = self.expect_identifier()?;
            self.expect(&TokenType::RightParen)?;
            
            let method_name = self.expect_identifier()?;
            
            let receiver = Some(crate::ast::Receiver {
                name: receiver_name,
                type_name: receiver_type,
                is_mutable: false, // TODO: handle mutable receivers
            });
            
            (receiver, method_name)
        } else {
            // Static method or regular function
            let method_name = self.expect_identifier()?;
            (None, method_name)
        };
        
        let is_public = method_name.chars().next().map_or(false, |c| c.is_uppercase());
        let is_static = receiver.is_none();
        
        self.expect(&TokenType::LeftParen)?;
        let parameters = self.parse_parameters()?;
        self.expect(&TokenType::RightParen)?;
        
        let return_type = if self.check(&TokenType::Arrow) || self.check(&TokenType::Colon) {
            self.advance();
            Some(self.parse_type()?)
        } else {
            None
        };
        
        self.expect(&TokenType::LeftBrace)?;
        let body = self.parse_block_body()?;
        self.expect(&TokenType::RightBrace)?;
        
        Ok(crate::ast::Method {
            name: method_name,
            parameters,
            return_type,
            body,
            is_public,
            is_static,
            receiver,
            pos,
        })
    }
    
    fn parse_assert(&mut self) -> ParseResult<Expression> {
        let pos = self.current.position.clone();
        self.advance(); // consume 'assert'
        self.expect(&TokenType::LeftParen)?;
        let condition = Box::new(self.parse_expression()?);
        
        let message = if self.check(&TokenType::Comma) {
            self.advance();
            if let TokenType::StringLiteral(msg) = &self.current.token_type {
                let message = Some(msg.clone());
                self.advance();
                message
            } else {
                None
            }
        } else {
            None
        };
        
        self.expect(&TokenType::RightParen)?;
        Ok(Expression::Assert { condition, message, pos })
    }
    
    fn parse_try(&mut self) -> ParseResult<Expression> {
        let pos = self.current.position.clone();
        self.advance(); // consume 'try'
        let body = Box::new(self.parse_expression()?);
        
        let mut catch_clauses = Vec::new();
        
        while self.check_keyword(KeywordType::KeywordCatch) {
            self.advance();
            
            let (exception_type, variable) = if self.check(&TokenType::LeftParen) {
                self.advance();
                let var = Some(self.expect_identifier()?);
                self.expect(&TokenType::Colon)?;
                let typ = Some(self.parse_type()?);
                self.expect(&TokenType::RightParen)?;
                (typ, var)
            } else {
                (None, None)
            };
            
            let catch_body = Box::new(self.parse_block()?);
            catch_clauses.push(CatchClause {
                exception_type,
                variable,
                body: catch_body,
            });
        }
        
        let finally = if self.check_keyword(KeywordType::KeywordFinally) {
            self.advance();
            Some(Box::new(self.parse_block()?))
        } else {
            None
        };
        
        Ok(Expression::Try { body, catch_clauses, finally, pos })
    }
    
    fn parse_extension(&mut self) -> ParseResult<Expression> {
        let pos = self.current.position.clone();
        self.advance(); // consume 'extension'
        let target_type = self.parse_type()?;
        
        self.expect(&TokenType::LeftBrace)?;
        let mut methods = Vec::new();
        
        while !self.check(&TokenType::RightBrace) {
            methods.push(self.parse_function()?);
            if !self.check(&TokenType::RightBrace) {
                self.expect(&TokenType::Comma)?;
            }
        }
        
        self.expect(&TokenType::RightBrace)?;
        Ok(Expression::Extension { target_type, methods, pos })
    }
    
    fn parse_interface(&mut self) -> ParseResult<Expression> {
        let pos = self.current.position.clone();
        self.advance(); // consume 'interface'
        let name = self.expect_identifier()?;
        
        self.expect(&TokenType::LeftBrace)?;
        let mut methods = Vec::new();
        
        while !self.check(&TokenType::RightBrace) {
            let method_name = self.expect_identifier()?;
            self.expect(&TokenType::LeftParen)?;
            let params = self.parse_parameters()?;
            self.expect(&TokenType::RightParen)?;
            
            let return_type = if self.check(&TokenType::Colon) {
                self.advance();
                Some(self.parse_type()?)
            } else {
                None
            };
            
            let (is_default, default_impl) = if self.check(&TokenType::Assign) {
                self.advance();
                (true, Some(Box::new(self.parse_expression()?)))
            } else {
                (false, None)
            };
            
            methods.push(InterfaceMethod {
                name: method_name,
                params,
                return_type,
                is_default,
                default_impl,
            });
            
            if !self.check(&TokenType::RightBrace) {
                self.expect(&TokenType::Comma)?;
            }
        }
        
        self.expect(&TokenType::RightBrace)?;
        Ok(Expression::Interface { name, methods, pos })
    }
    
    fn parse_class(&mut self) -> ParseResult<Expression> {
        let pos = self.current.position.clone();
        
        let mut is_sealed = false;
        let mut is_open = false;
        let mut is_abstract = false;
        
        // Check for class modifiers
        if self.check_keyword(KeywordType::KeywordSealed) {
            is_sealed = true;
            self.advance();
        } else if self.check_keyword(KeywordType::KeywordOpen) {
            is_open = true;
            self.advance();
        } else if self.check_keyword(KeywordType::KeywordAbstract) {
            is_abstract = true;
            self.advance();
        }
        
        self.advance(); // consume 'class'
        let name = self.expect_identifier()?;
        
        self.expect(&TokenType::LeftBrace)?;
        let mut fields = Vec::new();
        let mut methods = Vec::new();
        let mut companion = None;
        
        while !self.check(&TokenType::RightBrace) {
            if self.check_keyword(KeywordType::KeywordCompanion) {
                self.advance();
                self.expect_keyword(KeywordType::KeywordObject)?;
                companion = Some(Box::new(self.parse_block()?));
            } else if self.check_keyword(KeywordType::KeywordFun) {
                methods.push(self.parse_function()?);
            } else {
                // Parse field
                let field_name = self.expect_identifier()?;
                self.expect(&TokenType::Colon)?;
                let field_type = self.parse_type()?;
                fields.push((field_name, field_type));
            }
            
            if !self.check(&TokenType::RightBrace) {
                self.expect(&TokenType::Comma)?;
            }
        }
        
        self.expect(&TokenType::RightBrace)?;
        Ok(Expression::Class {
            name,
            is_sealed,
            is_open,
            is_abstract,
            fields,
            methods,
            companion,
            pos,
        })
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