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
    pub fn parse_top_level_item(&mut self) -> ParseResult<Expression> {
        // Check for contracts (requires/ensures/invariant)
        if self.check_keyword(KeywordType::KeywordRequires) || 
           self.check_keyword(KeywordType::KeywordEnsures) ||
           self.check_keyword(KeywordType::KeywordInvariant) {
            return self.parse_contracted_function();
        }
        
        // Check for pure functions
        if self.check_keyword(KeywordType::KeywordPure) {
            return self.parse_pure_function();
        }
        
        // Check for external functions
        if self.check_keyword(KeywordType::KeywordExternal) {
            return self.parse_external_function();
        }
        
        // Check for function definitions
        if self.check_keyword(KeywordType::KeywordFun) {
            return self.parse_function();
        }
        
        // Check for async functions and blocks
        if self.check_keyword(KeywordType::KeywordAsync) {
            return self.parse_async_construct();
        }
        
        // Check for type alias
        if self.check_keyword(KeywordType::KeywordType) {
            return self.parse_type_alias();
        }
        
        // Check for struct definitions
        if self.check_keyword(KeywordType::KeywordStruct) {
            return self.parse_struct_definition();
        }
        
        // Check for enum definitions
        if self.check_keyword(KeywordType::KeywordEnum) {
            return self.parse_enum_definition();
        }
        
        // Check for interface definitions
        if self.check_keyword(KeywordType::KeywordInterface) {
            return self.parse_interface();
        }
        
        // Check for sealed class definitions
        if self.check_keyword(KeywordType::KeywordSealed) {
            return self.parse_sealed_class();
        }
        
        // Check for class definitions
        if self.check_keyword(KeywordType::KeywordClass) {
            return self.parse_class_definition();
        }
        
        // Check for companion object
        if self.check_keyword(KeywordType::KeywordCompanion) {
            return self.parse_companion_object();
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
                    // Check if this is a trailing lambda (list.Map { it * 2 })
                    if self.is_trailing_lambda_context(&expr) {
                        let pos = expr.position().clone(); // Get position before moving
                        let lambda_arg = self.parse_lambda()?;
                        expr = Expression::Call {
                            callee: Box::new(expr),
                            args: vec![lambda_arg],
                            pos,
                        };
                        continue;
                    }
                    // Check if this is a struct literal (only for type identifiers)
                    else if let Expression::Identifier { name, is_public, pos } = &expr {
                        // Type names start with uppercase, so check if this is a type
                        if *is_public || name.chars().next().map_or(false, |c| c.is_uppercase()) {
                            let struct_name = name.clone();
                            let struct_expr = self.parse_struct_literal(struct_name)?;
                            expr = struct_expr;
                            continue;
                        }
                    }
                    // Not a struct literal or trailing lambda, so stop parsing postfix
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
        
        // Check for preprocessor directives
        if self.check(&TokenType::Hash) {
            return self.parse_conditional_compilation();
        }
        
        // Check for annotations
        if self.check(&TokenType::At) {
            let annotations = self.parse_annotations()?;
            
            // Parse the annotated expression
            let expr = self.parse_primary()?;
            
            return Ok(Expression::Annotated {
                annotations,
                expr: Box::new(expr),
                pos,
            });
        }
        
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
        
        if self.check_keyword(KeywordType::KeywordSend) {
            return self.parse_send();
        }
        
        if self.check_keyword(KeywordType::KeywordRequest) {
            return self.parse_request();
        }
        
        if self.check_keyword(KeywordType::KeywordReceive) {
            return self.parse_receive();
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
        
        // Async functions and blocks
        if self.check_keyword(KeywordType::KeywordAsync) {
            return self.parse_async_construct();
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
                
                if self.check(&TokenType::LeftParen) {
                    // Enum pattern: Success(x, y) or Failure(msg)
                    self.advance();
                    let mut field_patterns = Vec::new();
                    
                    while !self.check(&TokenType::RightParen) {
                        let field_pattern = self.parse_pattern()?;
                        field_patterns.push(Box::new(field_pattern));
                        
                        if !self.check(&TokenType::RightParen) {
                            self.expect(&TokenType::Comma)?;
                        }
                    }
                    
                    self.expect(&TokenType::RightParen)?;
                    
                    // For now, assume it's an enum pattern (we might need more sophisticated disambiguation later)
                    return Ok(Pattern::Enum { 
                        enum_name: "".to_string(), // We'll need to resolve this during type checking
                        variant: name, 
                        fields: field_patterns 
                    });
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
        
        // Check for delegation (by lazy, by observable, etc.)
        let delegation = if self.check_keyword(KeywordType::KeywordBy) {
            self.advance(); // consume 'by'
            Some(self.parse_delegation_type()?)
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
            delegation,
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
        
        // Check for delegation (by lazy, by observable, etc.)
        let delegation = if self.check_keyword(KeywordType::KeywordBy) {
            self.advance(); // consume 'by'
            Some(self.parse_delegation_type()?)
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
            delegation,
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
        
        // Parse 'uses' clause for effects
        let uses_effects = if self.check_keyword(KeywordType::KeywordUses) {
            self.advance(); // consume 'uses'
            let mut effects = Vec::new();
            
            // Parse comma-separated list of effects
            loop {
                effects.push(self.expect_identifier()?);
                
                if self.check(&TokenType::Comma) {
                    self.advance();
                } else {
                    break;
                }
            }
            
            effects
        } else {
            Vec::new()
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
            uses_effects,
            is_pure: false,
            is_external: false,
            doc_comment: None,
            pos,
        })
    }
    
    /// Parse async construct (function or block)
    fn parse_async_construct(&mut self) -> ParseResult<Expression> {
        let pos = self.current.position.clone();
        self.advance(); // consume 'async'
        
        // Check what follows 'async'
        if self.check_keyword(KeywordType::KeywordFun) {
            // async fun - parse async function
            self.parse_async_function_body(pos)
        } else if self.check(&TokenType::LeftBrace) {
            // async { - parse async block
            let body = Box::new(self.parse_block()?);
            Ok(Expression::AsyncBlock { body, pos })
        } else {
            Err(ParseError::UnexpectedToken {
                expected: "KeywordFun or LeftBrace".to_string(),
                found: self.current.token_type.clone(),
                pos: self.current.position.clone(),
            })
        }
    }
    
    /// Parse async function body (helper)
    fn parse_async_function_body(&mut self, pos: Position) -> ParseResult<Expression> {
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
        
        // Parse 'uses' clause for effects
        let uses_effects = if self.check_keyword(KeywordType::KeywordUses) {
            self.advance(); // consume 'uses'
            let mut effects = Vec::new();
            
            // Parse comma-separated list of effects
            loop {
                effects.push(self.expect_identifier()?);
                
                if self.check(&TokenType::Comma) {
                    self.advance();
                } else {
                    break;
                }
            }
            
            effects
        } else {
            Vec::new()
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
            uses_effects,
            is_pure: false,
            is_external: false,
            doc_comment: None,
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
        
        // Parse lambda parameters - support both typed and untyped
        let params = if self.check(&TokenType::RightBrace) {
            // Empty lambda: { }
            Vec::new()
        } else if self.is_at_lambda_arrow() {
            // No parameters, direct arrow: { -> body }
            Vec::new()
        } else {
            // Check if this might be a trailing lambda with implicit 'it' parameter
            // If we don't see an arrow within reasonable distance, assume implicit 'it'
            if self.is_implicit_it_lambda() {
                // Implicit 'it' parameter for trailing lambda syntax
                vec![Parameter {
                    name: "it".to_string(),
                    type_annotation: None,
                    default_value: None,
                    memory_modifier: None,
                }]
            } else {
                // Parse explicit parameters: { x -> ... } or { x: Int, y: String -> ... }
                let mut params = Vec::new();
                
                // First parameter
                let param_name = self.expect_identifier()?;
                let type_annotation = if self.check(&TokenType::Colon) {
                    self.advance(); // consume ':'
                    Some(self.parse_type()?)
                } else {
                    None
                };
                
                params.push(Parameter {
                    name: param_name,
                    type_annotation,
                    default_value: None,
                    memory_modifier: None,
                });
                
                // Additional parameters
                while self.check(&TokenType::Comma) {
                    self.advance(); // consume ','
                    let param_name = self.expect_identifier()?;
                    let type_annotation = if self.check(&TokenType::Colon) {
                        self.advance(); // consume ':'
                        Some(self.parse_type()?)
                    } else {
                        None
                    };
                    
                    params.push(Parameter {
                        name: param_name,
                        type_annotation,
                        default_value: None,
                        memory_modifier: None,
                    });
                }
                
                params
            }
        };
        
        // Check for optional return type: { x: Int, y: Int }: ReturnType -> body
        let return_type = if self.check(&TokenType::RightBrace) && params.is_empty() {
            // Empty lambda with no return type
            None
        } else if self.check(&TokenType::Colon) {
            // Return type specified: { params }: ReturnType -> body
            self.advance(); // consume ':'
            Some(self.parse_type()?)
        } else {
            None
        };
        
        // Expect arrow for explicit parameters, but not for implicit 'it'
        let has_implicit_it = params.len() == 1 && params[0].name == "it";
        
        if !has_implicit_it && (!params.is_empty() || return_type.is_some()) {
            self.expect(&TokenType::Arrow)?;
        } else if !has_implicit_it && self.check(&TokenType::Arrow) {
            // Empty parameter list with explicit arrow
            self.advance();
        }
        // For implicit 'it', no arrow expected
        
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
        let pos = self.current.position.clone();
        self.expect(&TokenType::LeftBrace)?;
        
        // Check if this is actually a lambda disguised as a block
        if self.is_lambda_in_braces() {
            // Parse as lambda but consume the braces
            let lambda = self.parse_lambda_body()?;
            self.expect(&TokenType::RightBrace)?;
            return Ok(lambda);
        }
        
        let mut expressions = Vec::new();
        
        while !self.check(&TokenType::RightBrace) && !self.is_at_end() {
            // Skip any leading newlines
            if self.check(&TokenType::Newline) {
                self.advance();
                continue;
            }
            
            let expr = self.parse_expression()?;
            expressions.push(expr);
            
            // Skip trailing newlines
            while self.check(&TokenType::Newline) {
                self.advance();
            }
        }
        
        self.expect(&TokenType::RightBrace)?;
        
        Ok(Expression::Block { expressions, pos })
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
            
            while !self.check(&TokenType::Greater) && !self.check(&TokenType::RightShift) && !self.is_at_end() {
                generics.push(self.parse_type()?);
                
                if self.check(&TokenType::Comma) {
                    self.advance();
                } else if !self.check(&TokenType::Greater) && !self.check(&TokenType::RightShift) {
                    return Err(ParseError::UnexpectedToken {
                        expected: "comma, >, or >>".to_string(),
                        found: self.current.token_type.clone(),
                        pos: self.current.position.clone(),
                    });
                }
            }
            
            // Handle >> as two > tokens for nested generics
            if self.check(&TokenType::RightShift) {
                // Convert >> to > by changing the current token
                // This is a hack, but it works for this case
                self.current.token_type = TokenType::Greater;
                self.current.lexeme = ">".to_string();
            } else {
                self.expect(&TokenType::Greater)?;
            }
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
    
    /// Check if we're at a lambda arrow (->)
    fn is_at_lambda_arrow(&self) -> bool {
        self.check(&TokenType::Arrow)
    }
    
    /// Check if current expression context supports trailing lambda syntax
    fn is_trailing_lambda_context(&self, expr: &Expression) -> bool {
        match expr {
            // Method calls like list.Map can have trailing lambdas
            Expression::MemberAccess { .. } => true,
            // Function identifiers can have trailing lambdas, but only if they're lowercase (functions)
            // Uppercase identifiers are types and should be struct literals
            Expression::Identifier { name, is_public, .. } => {
                !(*is_public || name.chars().next().map_or(false, |c| c.is_uppercase()))
            },
            // Chained calls can have trailing lambdas
            Expression::Call { .. } => true,
            _ => false,
        }
    }
    
    /// Check if this lambda uses implicit 'it' parameter (no arrow found)
    fn is_implicit_it_lambda(&mut self) -> bool {
        // Look ahead for arrow to distinguish { x -> ... } from { x * 2 }
        // If no arrow found within reasonable distance, assume implicit 'it'
        
        // Increase lookahead distance for typed parameters: { x: Int, y: String -> ... }
        for i in 0..10 {
            match self.peek_ahead(i) {
                Some(token) if matches!(token.token_type, TokenType::Arrow) => {
                    return false; // Found arrow, explicit parameters
                }
                Some(token) if matches!(token.token_type, TokenType::RightBrace) => {
                    return true; // Found closing brace before arrow, implicit it
                }
                Some(_) => continue,
                None => return true, // EOF before arrow, implicit it
            }
        }
        
        // If we can't decide in 10 tokens, assume explicit parameters and let parser handle errors
        false
    }
    
    fn is_lambda_in_braces(&mut self) -> bool {
        // Look ahead to determine if this is a lambda within braces (already consumed opening brace)
        // Lambda: x -> ... or (x: Type) -> ... or (x, y) -> ...
        
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
        
        // Restore tokens and assume false
        for t in lookahead_tokens.into_iter().rev() {
            self.peek_buffer.push_front(t);
        }
        false
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
        
        // Expect 'to' keyword
        if !self.check_keyword(KeywordType::KeywordTo) {
            return Err(ParseError::UnexpectedToken {
                expected: "keyword 'to'".to_string(),
                found: self.current.token_type.clone(),
                pos: self.current.position.clone(),
            });
        }
        self.advance(); // consume 'to'
        
        let target = Box::new(self.parse_expression()?);
        Ok(Expression::Send { message, target, pos })
    }
    
    fn parse_request(&mut self) -> ParseResult<Expression> {
        let pos = self.current.position.clone();
        self.advance(); // consume 'request'
        let message = Box::new(self.parse_expression()?);
        
        // Expect 'from' keyword
        if !self.check_keyword(KeywordType::KeywordFrom) {
            return Err(ParseError::UnexpectedToken {
                expected: "keyword 'from'".to_string(),
                found: self.current.token_type.clone(),
                pos: self.current.position.clone(),
            });
        }
        self.advance(); // consume 'from'
        
        let source = Box::new(self.parse_expression()?);
        Ok(Expression::Request { message, source, pos })
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
            
            // Parse annotations if present
            let annotations = if self.check(&TokenType::At) {
                self.parse_annotations()?
            } else {
                Vec::new()
            };
            
            let field_name = self.expect_identifier()?;
            let is_public = field_name.chars().next().map_or(false, |c| c.is_uppercase());
            
            self.expect(&TokenType::Colon)?;
            let field_type = self.parse_type()?;
            
            fields.push(crate::ast::StructField {
                name: field_name,
                field_type,
                is_public,
                annotations,
            });
            
            // Allow optional comma or newline between fields
            if self.check(&TokenType::Comma) {
                self.advance();
            }
            // Newlines are handled at the start of the loop
        }
        
        self.expect(&TokenType::RightBrace)?;
        
        Ok(Expression::StructDefinition { name, fields, doc_comment: None, pos })
    }
    
    fn parse_enum_definition(&mut self) -> ParseResult<Expression> {
        let pos = self.current.position.clone();
        self.advance(); // consume 'enum'
        
        let name = self.expect_identifier()?;
        self.expect(&TokenType::LeftBrace)?;
        
        let mut variants = Vec::new();
        
        while !self.check(&TokenType::RightBrace) {
            // Skip any leading newlines
            while self.check(&TokenType::Newline) {
                self.advance();
            }
            
            // Check for end of enum after skipping newlines
            if self.check(&TokenType::RightBrace) {
                break;
            }
            
            let variant_name = self.expect_identifier()?;
            
            // Check for tuple variant with parameters: Success(value: T)
            let fields = if self.check(&TokenType::LeftParen) {
                self.advance(); // consume '('
                let mut variant_fields = Vec::new();
                
                while !self.check(&TokenType::RightParen) && !self.is_at_end() {
                    let field_name = self.expect_identifier()?;
                    self.expect(&TokenType::Colon)?;
                    let field_type = self.parse_type()?;
                    
                    variant_fields.push(crate::ast::Field {
                        name: field_name,
                        type_annotation: field_type,
                        is_public: true, // Enum variant fields are public by default
                        is_mutable: false, // Enum variant fields are immutable
                        default_value: None,
                    });
                    
                    if !self.check(&TokenType::RightParen) {
                        self.expect(&TokenType::Comma)?;
                    }
                }
                
                self.expect(&TokenType::RightParen)?;
                Some(variant_fields)
            } else {
                None // Simple variant like Success
            };
            
            variants.push(crate::ast::EnumVariant {
                name: variant_name,
                fields,
            });
            
            // Allow optional comma or newline between variants
            if self.check(&TokenType::Comma) {
                self.advance();
            }
            // Newlines are handled at the start of the loop
        }
        
        self.expect(&TokenType::RightBrace)?;
        
        Ok(Expression::EnumDefinition { name, variants, doc_comment: None, pos })
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
            
            // Parse annotations if present
            let annotations = if self.check(&TokenType::At) {
                self.parse_annotations()?
            } else {
                Vec::new()
            };
            
            // Check if it's a method (starts with 'fun')
            if self.check_keyword(KeywordType::KeywordFun) {
                // TODO: Add annotations support for methods
                methods.push(self.parse_method()?);
            } else {
                // It's a field
                fields.push(self.parse_class_field(annotations)?);
            }
        }
        
        self.expect(&TokenType::RightBrace)?;
        
        Ok(Expression::ClassDefinition { 
            name, 
            superclass, 
            fields, 
            methods,
            is_sealed: false,
            doc_comment: None,
            pos 
        })
    }
    
    fn parse_class_field(&mut self, annotations: Vec<Annotation>) -> ParseResult<crate::ast::ClassField> {
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
            annotations,
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
            // Skip newlines
            while self.check(&TokenType::Newline) {
                self.advance();
            }
            
            if self.check(&TokenType::RightBrace) {
                break;
            }
            
            // Parse method
            if self.check_keyword(KeywordType::KeywordFun) {
                methods.push(self.parse_method()?);
            }
            
            // Allow optional comma or newline between methods
            if self.check(&TokenType::Comma) {
                self.advance();
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
            // Skip any leading newlines
            while self.check(&TokenType::Newline) {
                self.advance();
            }
            
            // Check for end of interface after skipping newlines
            if self.check(&TokenType::RightBrace) {
                break;
            }
            
            // Expect 'fun' keyword for method definition
            self.expect_keyword(KeywordType::KeywordFun)?;
            
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
            
            // Allow optional newline or comma between methods
            if self.check(&TokenType::Newline) || self.check(&TokenType::Comma) {
                self.advance();
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
    
    /// Parse a function with contracts (requires/ensures/invariant)
    fn parse_contracted_function(&mut self) -> ParseResult<Expression> {
        let pos = self.current.position.clone();
        let mut requires = None;
        let mut ensures = None;
        let mut invariants = Vec::new();
        
        // Parse contract clauses (can appear in any order)
        while self.check_keyword(KeywordType::KeywordRequires) ||
              self.check_keyword(KeywordType::KeywordEnsures) ||
              self.check_keyword(KeywordType::KeywordInvariant) {
            
            if self.check_keyword(KeywordType::KeywordRequires) {
                self.advance();
                self.expect(&TokenType::LeftBrace)?;
                requires = Some(Box::new(self.parse_expression()?));
                self.expect(&TokenType::RightBrace)?;
            } else if self.check_keyword(KeywordType::KeywordEnsures) {
                self.advance();
                self.expect(&TokenType::LeftBrace)?;
                ensures = Some(Box::new(self.parse_expression()?));
                self.expect(&TokenType::RightBrace)?;
            } else if self.check_keyword(KeywordType::KeywordInvariant) {
                self.advance();
                self.expect(&TokenType::LeftBrace)?;
                invariants.push(self.parse_expression()?);
                self.expect(&TokenType::RightBrace)?;
            }
        }
        
        // Now parse the actual function
        if !self.check_keyword(KeywordType::KeywordFun) {
            return Err(ParseError::UnexpectedToken {
                expected: format!("function keyword ({})", self.lexer.keyword_manager().get_keyword_text(&KeywordType::KeywordFun).unwrap_or("function".to_string())),
                found: self.current.token_type.clone(),
                pos: self.current.position.clone(),
            });
        }
        
        let function = Box::new(self.parse_function()?);
        
        Ok(Expression::ContractedFunction {
            function,
            requires,
            ensures,
            invariants,
            pos,
        })
    }
    
    /// Parse a pure function (no side effects)
    fn parse_pure_function(&mut self) -> ParseResult<Expression> {
        let pos = self.current.position.clone();
        self.advance(); // consume 'pure'
        
        if !self.check_keyword(KeywordType::KeywordFun) {
            return Err(ParseError::UnexpectedToken {
                expected: format!("function keyword ({})", self.lexer.keyword_manager().get_keyword_text(&KeywordType::KeywordFun).unwrap_or("function".to_string())),
                found: self.current.token_type.clone(),
                pos: self.current.position.clone(),
            });
        }
        
        // Parse the function and set is_pure flag
        let mut func = self.parse_function()?;
        
        // Update the is_pure flag
        if let Expression::Function { ref mut is_pure, .. } = func {
            *is_pure = true;
        }
        
        Ok(func)
    }
    
    /// Parse an external function (FFI)
    fn parse_external_function(&mut self) -> ParseResult<Expression> {
        let pos = self.current.position.clone();
        self.advance(); // consume 'external'
        
        if !self.check_keyword(KeywordType::KeywordFun) {
            return Err(ParseError::UnexpectedToken {
                expected: format!("function keyword ({})", self.lexer.keyword_manager().get_keyword_text(&KeywordType::KeywordFun).unwrap_or("function".to_string())),
                found: self.current.token_type.clone(),
                pos: self.current.position.clone(),
            });
        }
        
        // Parse the function and set is_external flag
        let mut func = self.parse_function()?;
        
        // Update the is_external flag
        if let Expression::Function { ref mut is_external, .. } = func {
            *is_external = true;
        }
        
        Ok(func)
    }
    
    /// Parse a when expression (similar to match but more natural)
    fn parse_when(&mut self) -> ParseResult<Expression> {
        let pos = self.current.position.clone();
        self.advance(); // consume 'when'
        
        // Check if there's a value to match against
        let expr = if self.check(&TokenType::LeftBrace) {
            // No value, just conditions
            None
        } else {
            Some(Box::new(self.parse_expression()?))
        };
        
        self.expect(&TokenType::LeftBrace)?;
        let mut arms = Vec::new();
        
        while !self.check(&TokenType::RightBrace) {
            // Parse pattern or condition
            let pattern = if expr.is_some() {
                self.parse_pattern()?
            } else {
                // For when without value, parse condition as pattern
                Pattern::Literal(Box::new(self.parse_expression()?))
            };
            
            self.expect(&TokenType::Arrow)?;
            let body = self.parse_expression()?;
            
            arms.push(MatchArm {
                pattern,
                guard: None,
                body,
            });
            
            if !self.check(&TokenType::RightBrace) {
                if self.check(&TokenType::Comma) {
                    self.advance();
                }
            }
        }
        
        self.expect(&TokenType::RightBrace)?;
        
        // Convert to match expression
        if let Some(value) = expr {
            Ok(Expression::Match {
                expr: value,
                arms,
                pos,
            })
        } else {
            // When without value becomes a series of if-else
            self.convert_when_to_if_chain(arms, pos)
        }
    }
    
    /// Parse type alias
    fn parse_type_alias(&mut self) -> ParseResult<Expression> {
        let pos = self.current.position.clone();
        self.advance(); // consume 'type'
        
        let name = self.expect_identifier()?;
        self.expect(&TokenType::Assign)?;
        let target_type = self.parse_type()?;
        
        Ok(Expression::TypeAlias {
            name,
            target_type,
            pos,
        })
    }
    
    /// Parse sealed class
    fn parse_sealed_class(&mut self) -> ParseResult<Expression> {
        let pos = self.current.position.clone();
        self.advance(); // consume 'sealed'
        
        if !self.check_keyword(KeywordType::KeywordClass) {
            return Err(ParseError::UnexpectedToken {
                expected: "class".to_string(),
                found: self.current.token_type.clone(),
                pos: self.current.position.clone(),
            });
        }
        
        // Parse as regular class but mark as sealed
        let mut class_expr = self.parse_class_definition()?;
        
        if let Expression::ClassDefinition { ref mut is_sealed, .. } = class_expr {
            *is_sealed = true;
        }
        
        Ok(class_expr)
    }
    
    /// Parse companion object
    fn parse_companion_object(&mut self) -> ParseResult<Expression> {
        let pos = self.current.position.clone();
        self.advance(); // consume 'companion'
        
        if !self.check_keyword(KeywordType::KeywordObject) {
            return Err(ParseError::UnexpectedToken {
                expected: "object".to_string(),
                found: self.current.token_type.clone(),
                pos: self.current.position.clone(),
            });
        }
        self.advance(); // consume 'object'
        
        // The class name is inferred from context in real usage
        let class_name = "Companion".to_string(); // Default name
        
        self.expect(&TokenType::LeftBrace)?;
        
        let mut fields = Vec::new();
        let mut methods = Vec::new();
        
        while !self.check(&TokenType::RightBrace) {
            // Skip newlines
            while self.check(&TokenType::Newline) {
                self.advance();
            }
            
            if self.check(&TokenType::RightBrace) {
                break;
            }
            
            // Parse annotations if present
            let annotations = if self.check(&TokenType::At) {
                self.parse_annotations()?
            } else {
                Vec::new()
            };
            
            // Check if it's a method or field
            if self.check_keyword(KeywordType::KeywordFun) {
                methods.push(self.parse_method()?);
            } else {
                fields.push(self.parse_class_field(annotations)?);
            }
        }
        
        self.expect(&TokenType::RightBrace)?;
        
        Ok(Expression::CompanionObject {
            class_name,
            fields,
            methods,
            pos,
        })
    }
    
    /// Parse conditional compilation (#if)
    fn parse_conditional_compilation(&mut self) -> ParseResult<Expression> {
        let pos = self.current.position.clone();
        self.advance(); // consume '#'
        
        // Expect if keyword 
        if !self.check_keyword(KeywordType::KeywordIf) {
            return Err(ParseError::UnexpectedToken {
                expected: format!("conditional keyword ({})", self.lexer.keyword_manager().get_keyword_text(&KeywordType::KeywordIf).unwrap_or("conditional".to_string())),
                found: self.current.token_type.clone(),
                pos: self.current.position.clone(),
            });
        }
        self.advance(); // consume 'if'
        
        // Parse condition
        let condition = Box::new(self.parse_expression()?);
        
        self.expect(&TokenType::LeftBrace)?;
        let then_branch = Box::new(self.parse_block_body()?);
        self.expect(&TokenType::RightBrace)?;
        
        // Check for else branch
        let else_branch = if self.check_keyword(KeywordType::KeywordElse) {
            self.advance();
            self.expect(&TokenType::LeftBrace)?;
            let else_body = Box::new(self.parse_block_body()?);
            self.expect(&TokenType::RightBrace)?;
            Some(else_body)
        } else {
            None
        };
        
        Ok(Expression::ConditionalCompilation {
            condition,
            then_branch,
            else_branch,
            pos,
        })
    }
    
    /// Parse delegation type (lazy, observable, etc.)
    fn parse_delegation_type(&mut self) -> ParseResult<DelegationType> {
        if let TokenType::PrivateIdentifier(name) = &self.current.token_type {
            let delegation = match name.as_str() {
                "lazy" => DelegationType::Lazy,
                "observable" => DelegationType::Observable,
                "computed" => DelegationType::Computed,
                other => DelegationType::Custom(other.to_string()),
            };
            self.advance();
            Ok(delegation)
        } else {
            Err(ParseError::UnexpectedToken {
                expected: "delegation type".to_string(),
                found: self.current.token_type.clone(),
                pos: self.current.position.clone(),
            })
        }
    }
    
    /// Parse annotations (@Reactive, @Computed, etc.)
    fn parse_annotations(&mut self) -> ParseResult<Vec<Annotation>> {
        let mut annotations = Vec::new();
        
        while self.check(&TokenType::At) {
            self.advance(); // consume '@'
            
            let name = self.expect_identifier()?;
            let mut args = Vec::new();
            
            // Check for annotation arguments
            if self.check(&TokenType::LeftParen) {
                self.advance(); // consume '('
                
                if !self.check(&TokenType::RightParen) {
                    loop {
                        args.push(self.parse_expression()?);
                        
                        if self.check(&TokenType::Comma) {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                }
                
                self.expect(&TokenType::RightParen)?;
            }
            
            annotations.push(Annotation { name, args });
        }
        
        Ok(annotations)
    }
    
    /// Convert when expression without value to if-else chain
    fn convert_when_to_if_chain(&self, arms: Vec<MatchArm>, pos: Position) -> ParseResult<Expression> {
        if arms.is_empty() {
            return Ok(Expression::Block {
                expressions: vec![],
                pos,
            });
        }
        
        let mut iter = arms.into_iter().rev();
        let mut result = None;
        
        for arm in iter {
            if let Pattern::Literal(cond) = arm.pattern {
                if result.is_none() {
                    // Last arm becomes else branch
                    result = Some(arm.body);
                } else {
                    // Build if-else chain
                    result = Some(Expression::If {
                        condition: cond,
                        then_branch: Box::new(arm.body),
                        else_branch: result.map(Box::new),
                        pos: pos.clone(),
                    });
                }
            } else {
                return Err(ParseError::InvalidExpression { pos });
            }
        }
        
        result.ok_or(ParseError::InvalidExpression { pos })
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