//! Parser implementation

use crate::ast::*;
use seen_lexer::{Token, TokenType};
use seen_common::{SeenResult, SeenError, Diagnostics};

/// Parser for the Seen language
pub struct Parser {
    tokens: Vec<Token>,
    current: usize,
    next_node_id: NodeId,
    diagnostics: Diagnostics,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            current: 0,
            next_node_id: 0,
            diagnostics: Diagnostics::new(),
        }
    }
    
    pub fn diagnostics(&self) -> &Diagnostics {
        &self.diagnostics
    }
    
    pub fn parse_program(&mut self) -> SeenResult<Program<'static>> {
        let mut items = Vec::new();
        
        // Skip any leading whitespace/comments
        self.skip_non_significant_tokens();
        
        // Parse top-level items until end of file
        while !self.is_at_end() {
            match self.parse_item() {
                Ok(item) => {
                    items.push(item);
                    self.skip_non_significant_tokens();
                }
                Err(_) => {
                    // Error recovery: skip to next likely item start and continue parsing
                    self.recover_to_item_boundary();
                    
                    // After recovery, try to parse the next item if we're not at end
                    if !self.is_at_end() {
                        // Skip one token to avoid infinite loop on same error
                        self.advance();
                    }
                }
            }
        }
        
        Ok(Program {
            items,
            span: seen_common::Span::single(seen_common::Position::start(), 0),
        })
    }
    
    fn parse_item(&mut self) -> SeenResult<Item<'static>> {
        let start_pos = self.current_position();
        
        match self.current_token() {
            Some(Token { value: TokenType::KeywordFunc, .. }) => self.parse_function(),
            Some(Token { value: TokenType::KeywordStruct, .. }) => self.parse_struct(),
            Some(Token { value: TokenType::KeywordEnum, .. }) => self.parse_enum(),
            Some(Token { value: TokenType::Identifier(name), .. }) => {
                // For backwards compatibility, also check identifier keywords
                match name.as_str() {
                    "func" => self.parse_function(),
                    "struct" => self.parse_struct(),
                    "enum" => self.parse_enum(),
                    _ => {
                        self.error("Expected 'func', 'struct', or 'enum'");
                        Err(seen_common::SeenError::parse_error("Unexpected token"))
                    }
                }
            }
            _ => {
                self.error("Expected item declaration");
                Err(seen_common::SeenError::parse_error("Expected item"))
            }
        }
    }
    
    fn parse_function(&mut self) -> SeenResult<Item<'static>> {
        let start_span = self.current_span();
        
        // Consume 'func'
        self.expect_keyword(TokenType::KeywordFunc)?;
        
        // Parse function name
        let name = self.expect_identifier_value()?;
        let name_span = self.previous_span();
        
        // Parse parameters
        self.expect_token(TokenType::LeftParen)?;
        let params = self.parse_parameter_list()?;
        self.expect_token(TokenType::RightParen)?;
        
        // Parse optional return type
        let return_type = if self.match_token(&TokenType::Arrow) {
            Some(self.parse_type()?)
        } else {
            None
        };
        
        // Parse function body
        let body = self.parse_block()?;
        
        let func = Function {
            name: seen_common::Spanned::new(name.leak(), name_span),
            params,
            return_type,
            body,
            visibility: Visibility::Private,
            attributes: Vec::new(),
        };
        
        Ok(Item {
            kind: ItemKind::Function(func),
            span: start_span,
            id: self.next_node_id(),
        })
    }
    
    fn parse_struct(&mut self) -> SeenResult<Item<'static>> {
        let start_span = self.current_span();
        
        // Consume 'struct'
        self.expect_keyword(TokenType::KeywordStruct)?;
        
        // Parse struct name
        let name = self.expect_identifier_value()?;
        let name_span = self.previous_span();
        
        // Parse struct body
        self.expect_token(TokenType::LeftBrace)?;
        let fields = self.parse_field_list()?;
        self.expect_token(TokenType::RightBrace)?;
        
        let struct_def = Struct {
            name: seen_common::Spanned::new(name.leak(), name_span),
            fields,
            visibility: Visibility::Private,
            generic_params: Vec::new(),
            attributes: Vec::new(),
        };
        
        Ok(Item {
            kind: ItemKind::Struct(struct_def),
            span: start_span,
            id: self.next_node_id(),
        })
    }
    
    fn parse_enum(&mut self) -> SeenResult<Item<'static>> {
        let start_span = self.current_span();
        
        // Consume 'enum'
        self.expect_keyword(TokenType::KeywordEnum)?;
        
        // Parse enum name
        let name = self.expect_identifier_value()?;
        let name_span = self.previous_span();
        
        // Parse enum body
        self.expect_token(TokenType::LeftBrace)?;
        let variants = self.parse_variant_list()?;
        self.expect_token(TokenType::RightBrace)?;
        
        let enum_def = Enum {
            name: seen_common::Spanned::new(name.leak(), name_span),
            variants,
            visibility: Visibility::Private,
            generic_params: Vec::new(),
            attributes: Vec::new(),
        };
        
        Ok(Item {
            kind: ItemKind::Enum(enum_def),
            span: start_span,
            id: self.next_node_id(),
        })
    }
    
    fn next_node_id(&mut self) -> NodeId {
        let id = self.next_node_id;
        self.next_node_id += 1;
        id
    }
    
    // Helper methods for parsing
    fn current_token(&self) -> Option<&Token> {
        self.tokens.get(self.current)
    }
    
    fn current_span(&self) -> seen_common::Span {
        self.current_token()
            .map(|t| t.span)
            .unwrap_or_else(|| seen_common::Span::single(seen_common::Position::start(), 0))
    }
    
    fn previous_span(&self) -> seen_common::Span {
        if self.current > 0 {
            self.tokens[self.current - 1].span
        } else {
            seen_common::Span::single(seen_common::Position::start(), 0)
        }
    }
    
    fn current_position(&self) -> usize {
        self.current
    }
    
    fn is_at_end(&self) -> bool {
        self.current >= self.tokens.len() || 
        matches!(self.current_token(), Some(Token { value: TokenType::EndOfFile, .. }))
    }
    
    fn advance(&mut self) -> Option<&Token> {
        if !self.is_at_end() {
            self.current += 1;
        }
        self.tokens.get(self.current.saturating_sub(1))
    }
    
    fn match_token(&mut self, token_type: &TokenType) -> bool {
        if let Some(token) = self.current_token() {
            if std::mem::discriminant(&token.value) == std::mem::discriminant(token_type) {
                self.advance();
                return true;
            }
        }
        false
    }
    
    fn expect_token(&mut self, token_type: TokenType) -> SeenResult<()> {
        if let Some(token) = self.current_token() {
            if std::mem::discriminant(&token.value) == std::mem::discriminant(&token_type) {
                self.advance();
                return Ok(());
            }
        }
        
        self.error(&format!("Expected {:?}", token_type));
        Err(seen_common::SeenError::parse_error("Unexpected token"))
    }
    
    fn expect_keyword(&mut self, expected: TokenType) -> SeenResult<()> {
        if let Some(token) = self.current_token() {
            if std::mem::discriminant(&token.value) == std::mem::discriminant(&expected) {
                self.advance();
                return Ok(());
            }
        }
        
        self.error(&format!("Expected {:?}", expected));
        Err(seen_common::SeenError::parse_error("Expected keyword"))
    }
    
    fn expect_identifier(&mut self, expected: &str) -> SeenResult<()> {
        if let Some(Token { value: TokenType::Identifier(name), .. }) = self.current_token() {
            if name == expected {
                self.advance();
                return Ok(());
            }
        }
        
        self.error(&format!("Expected '{}'", expected));
        Err(seen_common::SeenError::parse_error("Expected identifier"))
    }
    
    fn expect_identifier_value(&mut self) -> SeenResult<String> {
        if let Some(Token { value: TokenType::Identifier(name), .. }) = self.current_token() {
            let result = name.clone();
            self.advance();
            return Ok(result);
        }
        
        self.error("Expected identifier");
        Err(seen_common::SeenError::parse_error("Expected identifier"))
    }
    
    fn skip_non_significant_tokens(&mut self) {
        while let Some(token) = self.current_token() {
            match token.value {
                TokenType::Whitespace => {
                    self.advance();
                }
                _ => break,
            }
        }
    }
    
    fn error(&mut self, message: &str) {
        let span = self.current_span();
        self.diagnostics.error(message, span);
    }
    
    fn recover_to_item_boundary(&mut self) {
        // Skip tokens until we find a likely item start
        while !self.is_at_end() {
            if let Some(Token { value: TokenType::Identifier(name), .. }) = self.current_token() {
                if matches!(name.as_str(), "func" | "struct" | "enum") {
                    break;
                }
            }
            self.advance();
        }
    }
    
    fn parse_parameter_list(&mut self) -> SeenResult<Vec<Parameter<'static>>> {
        let mut params = Vec::new();
        
        if let Some(Token { value: TokenType::RightParen, .. }) = self.current_token() {
            return Ok(params); // Empty parameter list
        }
        
        loop {
            let name = self.expect_identifier_value()?;
            let name_span = self.previous_span();
            
            self.expect_token(TokenType::Colon)?;
            let ty = self.parse_type()?;
            
            params.push(Parameter {
                name: seen_common::Spanned::new(name.leak(), name_span),
                ty,
                is_mutable: false,
                span: name_span,
            });
            
            if !self.match_token(&TokenType::Comma) {
                break;
            }
        }
        
        Ok(params)
    }
    
    fn parse_field_list(&mut self) -> SeenResult<Vec<Field<'static>>> {
        let mut fields = Vec::new();
        
        while !matches!(self.current_token(), Some(Token { value: TokenType::RightBrace, .. })) {
            if self.is_at_end() {
                self.error("Unexpected end of file in field list");
                break;
            }
            
            let name = self.expect_identifier_value()?;
            let name_span = self.previous_span();
            
            self.expect_token(TokenType::Colon)?;
            let ty = self.parse_type()?;
            
            fields.push(Field {
                name: seen_common::Spanned::new(name.leak(), name_span),
                ty,
                visibility: Visibility::Private,
                span: name_span,
            });
            
            // Optional comma
            self.match_token(&TokenType::Comma);
        }
        
        Ok(fields)
    }
    
    fn parse_variant_list(&mut self) -> SeenResult<Vec<Variant<'static>>> {
        let mut variants = Vec::new();
        
        while !matches!(self.current_token(), Some(Token { value: TokenType::RightBrace, .. })) {
            if self.is_at_end() {
                self.error("Unexpected end of file in variant list");
                break;
            }
            
            let name = self.expect_identifier_value()?;
            let name_span = self.previous_span();
            
            let data = if self.match_token(&TokenType::LeftParen) {
                // Tuple variant
                let mut types = Vec::new();
                
                while !matches!(self.current_token(), Some(Token { value: TokenType::RightParen, .. })) {
                    types.push(self.parse_type()?);
                    if !self.match_token(&TokenType::Comma) {
                        break;
                    }
                }
                
                self.expect_token(TokenType::RightParen)?;
                VariantData::Tuple(types)
            } else if self.match_token(&TokenType::LeftBrace) {
                // Struct variant
                let fields = self.parse_field_list()?;
                self.expect_token(TokenType::RightBrace)?;
                VariantData::Struct(fields)
            } else {
                // Unit variant
                VariantData::Unit
            };
            
            variants.push(Variant {
                name: seen_common::Spanned::new(name.leak(), name_span),
                data,
                span: name_span,
            });
            
            // Optional comma
            self.match_token(&TokenType::Comma);
        }
        
        Ok(variants)
    }
    
    fn parse_type(&mut self) -> SeenResult<Type<'static>> {
        let span = self.current_span();
        
        let kind = if let Some(Token { value: TokenType::Identifier(name), .. }) = self.current_token() {
            let type_name = name.clone();
            self.advance();
            
            // Check for primitive types
            let primitive = match type_name.as_str() {
                "i8" => Some(PrimitiveType::I8),
                "i16" => Some(PrimitiveType::I16),
                "i32" => Some(PrimitiveType::I32),
                "i64" => Some(PrimitiveType::I64),
                "i128" => Some(PrimitiveType::I128),
                "u8" => Some(PrimitiveType::U8),
                "u16" => Some(PrimitiveType::U16),
                "u32" => Some(PrimitiveType::U32),
                "u64" => Some(PrimitiveType::U64),
                "u128" => Some(PrimitiveType::U128),
                "f32" => Some(PrimitiveType::F32),
                "f64" => Some(PrimitiveType::F64),
                "bool" => Some(PrimitiveType::Bool),
                "char" => Some(PrimitiveType::Char),
                "str" => Some(PrimitiveType::Str),
                _ => None,
            };
            
            if let Some(prim) = primitive {
                TypeKind::Primitive(prim)
            } else {
                // Named type
                let path = Path {
                    segments: vec![PathSegment {
                        name: seen_common::Spanned::new(type_name.leak(), span),
                        generic_args: Vec::new(),
                    }],
                    span,
                };
                TypeKind::Named { path, generic_args: Vec::new() }
            }
        } else {
            self.error("Expected type");
            return Err(seen_common::SeenError::parse_error("Expected type"));
        };
        
        Ok(Type {
            kind: Box::new(kind),
            span,
        })
    }
    
    fn parse_block(&mut self) -> SeenResult<Block<'static>> {
        let start_span = self.current_span();
        
        self.expect_token(TokenType::LeftBrace)?;
        
        let mut statements = Vec::new();
        
        while !matches!(self.current_token(), Some(Token { value: TokenType::RightBrace, .. })) {
            if self.is_at_end() {
                self.error("Unexpected end of file in block");
                break;
            }
            
            // Parse statement
            statements.push(self.parse_statement()?);
        }
        
        self.expect_token(TokenType::RightBrace)?;
        
        Ok(Block {
            statements,
            span: start_span,
        })
    }
    
    fn parse_statement(&mut self) -> SeenResult<Stmt<'static>> {
        let span = self.current_span();
        
        let kind = match self.current_token() {
            Some(Token { value: TokenType::KeywordLet, .. }) => {
                self.advance(); // consume 'let'
                    
                    let pattern_name = self.expect_identifier_value()?;
                let pattern_span = self.previous_span();
                
                let ty = if self.match_token(&TokenType::Colon) {
                    Some(self.parse_type()?)
                } else {
                    None
                };
                
                let initializer = if self.match_token(&TokenType::Assign) {
                    Some(self.parse_expression()?)
                } else {
                    None
                };
                
                let pattern = Pattern {
                    kind: PatternKind::Identifier(seen_common::Spanned::new(pattern_name.leak(), pattern_span)),
                    span: pattern_span,
                    id: self.next_node_id(),
                };
                
                StmtKind::Let(Let {
                    pattern,
                    ty,
                    initializer,
                    is_mutable: false,
                })
            }
            Some(Token { value: TokenType::KeywordReturn, .. }) => {
                self.advance(); // consume 'return'
                
                let value = if matches!(self.current_token(), Some(Token { value: TokenType::Semicolon, .. })) {
                    None
                } else {
                    Some(Box::new(self.parse_expression()?))
                };
                
                let expr = Expr {
                    kind: Box::new(ExprKind::Return(value)),
                    span,
                    id: self.next_node_id(),
                };
                
                StmtKind::Expr(expr)
            }
            Some(Token { value: TokenType::KeywordIf, .. }) => {
                let if_expr = self.parse_if_expression()?;
                StmtKind::Expr(if_expr)
            }
            Some(Token { value: TokenType::Identifier(name), .. }) => {
                match name.as_str() {
                    "let" => {
                        self.advance(); // consume 'let' (backward compatibility)
                        
                        let pattern_name = self.expect_identifier_value()?;
                        let pattern_span = self.previous_span();
                        
                        let ty = if self.match_token(&TokenType::Colon) {
                            Some(self.parse_type()?)
                        } else {
                            None
                        };
                        
                        let initializer = if self.match_token(&TokenType::Assign) {
                            Some(self.parse_expression()?)
                        } else {
                            None
                        };
                        
                        let pattern = Pattern {
                            kind: PatternKind::Identifier(seen_common::Spanned::new(pattern_name.leak(), pattern_span)),
                            span: pattern_span,
                            id: self.next_node_id(),
                        };
                        
                        StmtKind::Let(Let {
                            pattern,
                            ty,
                            initializer,
                            is_mutable: false,
                        })
                    }
                    "return" => {
                        self.advance(); // consume 'return' (backward compatibility)
                        
                        let value = if matches!(self.current_token(), Some(Token { value: TokenType::Semicolon, .. })) {
                            None
                        } else {
                            Some(Box::new(self.parse_expression()?))
                        };
                        
                        let expr = Expr {
                            kind: Box::new(ExprKind::Return(value)),
                            span,
                            id: self.next_node_id(),
                        };
                        
                        StmtKind::Expr(expr)
                    }
                    "if" => {
                        let if_expr = self.parse_if_expression()?;
                        StmtKind::Expr(if_expr)
                    }
                    _ => {
                        // Parse as expression statement
                        let expr = self.parse_expression()?;
                        StmtKind::Expr(expr)
                    }
                }
            }
            _ => {
                // Parse as expression statement
                let expr = self.parse_expression()?;
                StmtKind::Expr(expr)
            }
        };
        
        // Optional semicolon
        self.match_token(&TokenType::Semicolon);
        
        Ok(Stmt {
            kind,
            span,
            id: self.next_node_id(),
        })
    }
    
    fn parse_expression(&mut self) -> SeenResult<Expr<'static>> {
        self.parse_binary_expression(0)
    }
    
    fn parse_binary_expression(&mut self, min_precedence: u8) -> SeenResult<Expr<'static>> {
        let mut left = self.parse_primary_expression()?;
        
        while let Some(token) = self.current_token() {
            let (precedence, is_right_associative) = self.get_precedence(&token.value);
            
            if precedence < min_precedence {
                break;
            }
            
            let op = self.token_to_binary_op(&token.value);
            if op.is_none() {
                break;
            }
            
            self.advance(); // consume operator
            
            let next_min_precedence = if is_right_associative {
                precedence
            } else {
                precedence + 1
            };
            
            let right = self.parse_binary_expression(next_min_precedence)?;
            
            left = Expr {
                kind: Box::new(ExprKind::Binary {
                    op: op.unwrap(),
                    left: Box::new(left),
                    right: Box::new(right),
                }),
                span: self.current_span(),
                id: self.next_node_id(),
            };
        }
        
        Ok(left)
    }
    
    fn parse_primary_expression(&mut self) -> SeenResult<Expr<'static>> {
        let span = self.current_span();
        
        if let Some(token) = self.current_token() {
            let kind = match &token.value {
                TokenType::IntegerLiteral(value) => {
                    let val = *value;
                    self.advance();
                    ExprKind::Literal(Literal {
                        kind: LiteralKind::Integer(val),
                        span,
                    })
                }
                TokenType::FloatLiteral(value) => {
                    let val = value.parse::<f64>().unwrap_or(0.0);
                    self.advance();
                    ExprKind::Literal(Literal {
                        kind: LiteralKind::Float(val),
                        span,
                    })
                }
                TokenType::StringLiteral(value) => {
                    let val = value.clone();
                    self.advance();
                    ExprKind::Literal(Literal {
                        kind: LiteralKind::String(val.leak()),
                        span,
                    })
                }
                TokenType::BooleanLiteral(value) => {
                    let val = *value;
                    self.advance();
                    ExprKind::Literal(Literal {
                        kind: LiteralKind::Boolean(val),
                        span,
                    })
                }
                TokenType::Identifier(name) => {
                    let name_val = name.clone();
                    self.advance();
                    ExprKind::Identifier(seen_common::Spanned::new(name_val.leak(), span))
                }
                TokenType::LeftParen => {
                    self.advance(); // consume '('
                    let expr = self.parse_expression()?;
                    self.expect_token(TokenType::RightParen)?;
                    return Ok(expr);
                }
                _ => {
                    self.error("Expected expression");
                    return Err(seen_common::SeenError::parse_error("Expected expression"));
                }
            };
            
            Ok(Expr {
                kind: Box::new(kind),
                span,
                id: self.next_node_id(),
            })
        } else {
            self.error("Unexpected end of file");
            Err(seen_common::SeenError::parse_error("Unexpected EOF"))
        }
    }
    
    fn parse_if_expression(&mut self) -> SeenResult<Expr<'static>> {
        let span = self.current_span();
        
        self.expect_keyword(TokenType::KeywordIf)?;
        let condition = Box::new(self.parse_expression()?);
        let then_branch = self.parse_block()?;
        
        let else_branch = if self.match_token(&TokenType::Identifier("else".to_string())) {
            Some(Box::new(self.parse_expression()?))
        } else {
            None
        };
        
        Ok(Expr {
            kind: Box::new(ExprKind::If {
                condition,
                then_branch,
                else_branch,
            }),
            span,
            id: self.next_node_id(),
        })
    }
    
    fn get_precedence(&self, token_type: &TokenType) -> (u8, bool) {
        // Precedence levels (higher number = higher precedence)
        // Based on Kotlin operator precedence
        match token_type {
            TokenType::LogicalOr => (1, false),
            TokenType::LogicalAnd => (2, false),
            TokenType::Equal | TokenType::NotEqual => (3, false),
            TokenType::Less | TokenType::LessEqual | TokenType::Greater | TokenType::GreaterEqual => (4, false),
            TokenType::Plus | TokenType::Minus => (5, false),
            TokenType::Multiply | TokenType::Divide | TokenType::Modulo => (6, false),
            _ => (0, false), // No precedence
        }
    }
    
    fn token_to_binary_op(&self, token_type: &TokenType) -> Option<BinaryOp> {
        match token_type {
            TokenType::Plus => Some(BinaryOp::Add),
            TokenType::Minus => Some(BinaryOp::Sub),
            TokenType::Multiply => Some(BinaryOp::Mul),
            TokenType::Divide => Some(BinaryOp::Div),
            TokenType::Modulo => Some(BinaryOp::Mod),
            TokenType::Equal => Some(BinaryOp::Eq),
            TokenType::NotEqual => Some(BinaryOp::Ne),
            TokenType::Less => Some(BinaryOp::Lt),
            TokenType::LessEqual => Some(BinaryOp::Le),
            TokenType::Greater => Some(BinaryOp::Gt),
            TokenType::GreaterEqual => Some(BinaryOp::Ge),
            TokenType::LogicalAnd => Some(BinaryOp::And),
            TokenType::LogicalOr => Some(BinaryOp::Or),
            _ => None,
        }
    }
}