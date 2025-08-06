//! Parser implementation

use crate::ast::*;
use seen_lexer::{Token, TokenType};
use seen_common::{SeenResult, Diagnostics};

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
            #[cfg(test)]
            eprintln!("parse_program: About to parse item, current = {:?}", self.current_token());
            
            match self.parse_item() {
                Ok(item) => {
                    #[cfg(test)]
                    eprintln!("parse_program: Successfully parsed item");
                    
                    items.push(item);
                    self.skip_non_significant_tokens();
                }
                Err(e) => {
                    #[cfg(test)]
                    eprintln!("parse_program: Error parsing item: {:?}", e);
                    
                    // Add diagnostic for the parsing error
                    self.diagnostics.error(&format!("Failed to parse item: {}", e), self.current_span());
                    
                    // Error recovery: skip to next likely item start and continue parsing
                    self.recover_to_item_boundary();
                    
                    // Don't advance if we're already at an item boundary after recovery
                    // This prevents skipping valid items
                }
            }
        }
        
        Ok(Program {
            items,
            span: seen_common::Span::single(seen_common::Position::start(), 0),
        })
    }
    
    fn parse_item(&mut self) -> SeenResult<Item<'static>> {
        let _start_pos = self.current_position();
        
        // Debug: Print current token
        #[cfg(test)]
        if let Some(token) = self.current_token() {
            eprintln!("parse_item: Current token = {:?}", token);
        }
        
        match self.current_token() {
            Some(Token { value: TokenType::KeywordFunc, .. }) => self.parse_function(),
            Some(Token { value: TokenType::KeywordStruct, .. }) => self.parse_struct(),
            Some(Token { value: TokenType::KeywordEnum, .. }) => self.parse_enum(),
            Some(Token { value: TokenType::Identifier(name), .. }) => {
                // Check for Kotlin-style features and backwards compatibility
                match name.as_str() {
                    "extension" => self.parse_extension_function(),
                    "data" => {
                        #[cfg(test)]
                        eprintln!("parse_item: Calling parse_data_class");
                        self.parse_data_class()
                    },
                    "sealed" => self.parse_sealed_class(),
                    "func" => self.parse_function(),
                    "struct" => self.parse_struct(),
                    "enum" => self.parse_enum(),
                    _ => {
                        eprintln!("parse_item: Unexpected identifier '{}'", name);
                        self.error("Expected 'func', 'struct', 'enum', 'extension', 'data', or 'sealed'");
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
        
        // Parse optional return type (supports both ':' and '->' syntax)
        let return_type = if self.match_token(&TokenType::Colon) || self.match_token(&TokenType::Arrow) {
            Some(self.parse_type()?)
        } else {
            None
        };
        
        // Parse function body
        let body = self.parse_block()?;
        
        let name_static: &'static str = Box::leak(name.into_boxed_str());
        let func = Function {
            name: seen_common::Spanned::new(name_static, name_span),
            params,
            return_type,
            body,
            visibility: Visibility::Private,
            attributes: Vec::with_capacity(0), // Empty but avoids allocation
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
    
    fn match_identifier(&mut self, expected: &str) -> bool {
        if let Some(Token { value: TokenType::Identifier(name), .. }) = self.current_token() {
            if name == expected {
                self.advance();
                return true;
            }
        }
        false
    }
    
    fn check(&self, token_type: &TokenType) -> bool {
        if let Some(token) = self.current_token() {
            std::mem::discriminant(&token.value) == std::mem::discriminant(token_type)
        } else {
            false
        }
    }
    
    fn parse_generic_params(&mut self) -> SeenResult<Vec<GenericParam<'static>>> {
        // For now, return empty generic params
        // Full implementation would parse <T, U: Trait, ...>
        let mut params = vec![];
        
        if !self.check(&TokenType::Greater) {
            loop {
                let name = self.expect_identifier_value()?;
                let span = self.previous_span();
                
                params.push(GenericParam {
                    name: seen_common::Spanned::new(name.leak(), span),
                    bounds: vec![],
                    default: None,
                    span,
                });
                
                if !self.match_token(&TokenType::Comma) {
                    break;
                }
            }
        }
        
        self.expect_token(TokenType::Greater)?;
        Ok(params)
    }
    
    fn parse_struct_fields(&mut self) -> SeenResult<Vec<Field<'static>>> {
        let mut fields = vec![];
        
        while !self.check(&TokenType::RightBrace) && !self.is_at_end() {
            // Parse field visibility (default to public)
            let visibility = Visibility::Public;
            
            // Parse field name
            let name = self.expect_identifier_value()?;
            let name_span = self.previous_span();
            
            // Expect ':'
            self.expect_token(TokenType::Colon)?;
            
            // Parse field type
            let ty = self.parse_type()?;
            
            fields.push(Field {
                name: seen_common::Spanned::new(name.leak(), name_span),
                ty,
                visibility,
                span: name_span,
            });
            
            // Optional comma
            if !self.match_token(&TokenType::Comma) {
                // If no comma, we should be at the end
                if !self.check(&TokenType::RightBrace) {
                    self.error("Expected ',' or '}'");
                }
            }
        }
        
        Ok(fields)
    }
    
    fn parse_data_class_fields(&mut self) -> SeenResult<Vec<DataClassField<'static>>> {
        let mut fields = vec![];
        
        // Empty field list
        if self.check(&TokenType::RightParen) {
            return Ok(fields);
        }
        
        loop {
            // Parse field visibility (default to public)
            let visibility = Visibility::Public;
            
            // Check for 'var' or 'val' keywords
            let is_mutable = if self.match_identifier("var") {
                true
            } else if self.match_identifier("val") {
                false
            } else {
                false // Default to immutable
            };
            
            // Parse field name
            let name = self.expect_identifier_value()?;
            let field_span = self.current_span();
            let name_span = self.previous_span();
            
            // Expect ':'
            self.expect_token(TokenType::Colon)?;
            
            // Parse field type
            let ty = self.parse_type()?;
            
            // Check for default value
            let default_value = if self.match_token(&TokenType::Assign) {
                Some(self.parse_expression()?)
            } else {
                None
            };
            
            fields.push(DataClassField {
                name: seen_common::Spanned::new(name.leak(), name_span),
                ty,
                is_mutable,
                default_value,
                visibility,
                span: field_span,
            });
            
            // Check for comma or end of list
            if !self.match_token(&TokenType::Comma) {
                break; // No comma means end of field list
            }
        }
        
        Ok(fields)
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
            match self.current_token() {
                Some(Token { value: TokenType::KeywordFunc, .. }) |
                Some(Token { value: TokenType::KeywordStruct, .. }) |
                Some(Token { value: TokenType::KeywordEnum, .. }) => {
                    break;
                }
                Some(Token { value: TokenType::Identifier(name), .. }) => {
                    // Also check for keyword identifiers (backward compatibility)
                    if matches!(name.as_str(), "func" | "struct" | "enum" | "extension" | "data" | "sealed") {
                        break;
                    }
                }
                _ => {}
            }
            self.advance();
        }
    }
    
    fn parse_parameter_list(&mut self) -> SeenResult<Vec<Parameter<'static>>> {
        let mut params = Vec::with_capacity(4); // Pre-allocate for common case
        
        if let Some(Token { value: TokenType::RightParen, .. }) = self.current_token() {
            return Ok(params); // Empty parameter list
        }
        
        loop {
            let name = self.expect_identifier_value()?;
            let name_span = self.previous_span();
            
            self.expect_token(TokenType::Colon)?;
            let ty = self.parse_type()?;
            
            // Check for default value
            let default_value = if self.match_token(&TokenType::Assign) {
                Some(self.parse_expression()?)
            } else {
                None
            };
            
            // Use Box::leak for better performance than String::leak
            let name_static: &'static str = Box::leak(name.into_boxed_str());
            
            params.push(Parameter {
                name: seen_common::Spanned::new(name_static, name_span),
                ty,
                is_mutable: false,
                default_value,
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
        
        let mut ty = Type {
            kind: Box::new(kind),
            span,
        };
        
        // Check for nullable type marker '?'
        if self.match_token(&TokenType::Question) {
            ty = Type {
                kind: Box::new(TypeKind::Nullable(Box::new(ty))),
                span: span.combine(self.previous_span()),
            };
        }
        
        Ok(ty)
    }
    
    fn parse_block(&mut self) -> SeenResult<Block<'static>> {
        let start_span = self.current_span();
        
        self.expect_token(TokenType::LeftBrace)?;
        
        #[cfg(test)]
        eprintln!("parse_block: Starting, current token = {:?}", self.current_token());
        
        let mut statements = Vec::new();
        
        while !matches!(self.current_token(), Some(Token { value: TokenType::RightBrace, .. })) {
            if self.is_at_end() {
                self.error("Unexpected end of file in block");
                break;
            }
            
            #[cfg(test)]
            eprintln!("parse_block: About to parse statement, current token = {:?}", self.current_token());
            
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
                
                #[cfg(test)]
                eprintln!("parse_statement: Parsing return, current token = {:?}", self.current_token());
                
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
        let mut left = self.parse_postfix_expression()?;
        
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
    
    fn parse_expression_list(&mut self) -> SeenResult<Vec<Expr<'static>>> {
        let mut expressions = Vec::new();
        
        // Empty list
        if matches!(self.current_token(), Some(Token { value: TokenType::RightParen, .. }) | 
                                          Some(Token { value: TokenType::RightBracket, .. })) {
            return Ok(expressions);
        }
        
        loop {
            expressions.push(self.parse_expression()?);
            
            if !self.match_token(&TokenType::Comma) {
                break;
            }
        }
        
        Ok(expressions)
    }
    
    fn parse_call_arguments(&mut self) -> SeenResult<Vec<Expr<'static>>> {
        let mut args = Vec::new();
        
        // Empty argument list
        if matches!(self.current_token(), Some(Token { value: TokenType::RightParen, .. })) {
            return Ok(args);
        }
        
        loop {
            // Try to parse named argument (identifier followed by colon)
            if let Some(Token { value: TokenType::Identifier(name), .. }) = self.current_token() {
                // Look ahead for colon to detect named argument
                let saved_position = self.current;
                let arg_name = name.clone();
                let name_span = self.current_span();
                self.advance(); // consume identifier
                
                if self.match_token(&TokenType::Colon) {
                    // It's a named argument
                    let value = self.parse_expression()?;
                    args.push(Expr {
                        kind: Box::new(ExprKind::NamedArg {
                            name: seen_common::Spanned::new(arg_name.leak(), name_span),
                            value: Box::new(value),
                        }),
                        span: self.current_span(),
                        id: self.next_node_id(),
                    });
                } else {
                    // Not a named argument, backtrack and parse as regular expression
                    self.current = saved_position;
                    args.push(self.parse_expression()?);
                }
            } else {
                // Regular positional argument
                args.push(self.parse_expression()?);
            }
            
            if !self.match_token(&TokenType::Comma) {
                break;
            }
        }
        
        Ok(args)
    }
    
    fn parse_postfix_expression(&mut self) -> SeenResult<Expr<'static>> {
        let mut expr = self.parse_primary_expression()?;
        
        loop {
            match self.current_token() {
                Some(Token { value: TokenType::Dot, .. }) => {
                    self.advance(); // consume '.'
                    
                    let name = self.expect_identifier_value()?;
                    let name_span = self.previous_span();
                    
                    // Check if it's a method call or field access
                    if matches!(self.current_token(), Some(Token { value: TokenType::LeftParen, .. })) {
                        // Method call
                        self.advance(); // consume '('
                        let args = self.parse_call_arguments()?;
                        self.expect_token(TokenType::RightParen)?;
                        
                        expr = Expr {
                            kind: Box::new(ExprKind::MethodCall {
                                receiver: Box::new(expr),
                                method: seen_common::Spanned::new(name.leak(), name_span),
                                args,
                            }),
                            span: self.current_span(),
                            id: self.next_node_id(),
                        };
                    } else {
                        // Field access
                        expr = Expr {
                            kind: Box::new(ExprKind::FieldAccess {
                                object: Box::new(expr),
                                field: seen_common::Spanned::new(name.leak(), name_span),
                            }),
                            span: self.current_span(),
                            id: self.next_node_id(),
                        };
                    }
                }
                Some(Token { value: TokenType::LeftParen, .. }) => {
                    // Function call
                    self.advance(); // consume '('
                    let args = self.parse_call_arguments()?;
                    self.expect_token(TokenType::RightParen)?;
                    
                    expr = Expr {
                        kind: Box::new(ExprKind::Call {
                            function: Box::new(expr),
                            args,
                        }),
                        span: self.current_span(),
                        id: self.next_node_id(),
                    };
                }
                Some(Token { value: TokenType::LeftBracket, .. }) => {
                    self.advance(); // consume '['
                    let index = self.parse_expression()?;
                    self.expect_token(TokenType::RightBracket)?;
                    
                    expr = Expr {
                        kind: Box::new(ExprKind::Index {
                            array: Box::new(expr),
                            index: Box::new(index),
                        }),
                        span: self.current_span(),
                        id: self.next_node_id(),
                    };
                }
                _ => break,
            }
        }
        
        Ok(expr)
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
                    // Check for special identifiers
                    if name == "null" {
                        self.advance();
                        return Ok(Expr {
                            kind: Box::new(ExprKind::Null),
                            span,
                            id: self.next_node_id(),
                        });
                    } else if name == "match" {
                        // Parse as match expression - consume the "match" token first
                        self.advance();
                        return self.parse_match_expression_impl();
                    } else {
                        let name_val = name.clone();
                        self.advance();
                        
                        // Check for struct literal (identifier followed by '{')
                        // But not if the next token after '{' could be part of a different construct
                        if matches!(self.current_token(), Some(Token { value: TokenType::LeftBrace, .. })) {
                            // Look ahead to see if this looks like a struct literal
                            // Struct literals have field_name: value patterns
                            // We need to peek at the next few tokens to determine this
                            // For now, we'll skip struct literal parsing if we're in a return context and the brace could be for match arms
                            
                            // Save position for lookahead  
                            let _saved_pos = self.current;
                            let is_struct_literal = if self.current + 1 < self.tokens.len() {
                                // Look at the next token after '{'
                                matches!(self.tokens[self.current + 1].value, TokenType::Identifier(_))
                                    && self.current + 2 < self.tokens.len()
                                    && matches!(self.tokens[self.current + 2].value, TokenType::Colon)
                            } else {
                                false
                            };
                            
                            if is_struct_literal {
                            // Parse as struct literal
                            self.advance(); // consume '{'
                            
                            let mut fields = Vec::new();
                            
                            while !matches!(self.current_token(), Some(Token { value: TokenType::RightBrace, .. })) {
                                let field_name = self.expect_identifier_value()?;
                                let field_span = self.previous_span();
                                
                                self.expect_token(TokenType::Colon)?;
                                let value = self.parse_expression()?;
                                
                                fields.push(FieldExpr {
                                    name: seen_common::Spanned::new(field_name.leak(), field_span),
                                    value,
                                });
                                
                                if !self.match_token(&TokenType::Comma) {
                                    break;
                                }
                            }
                            
                            self.expect_token(TokenType::RightBrace)?;
                            
                            ExprKind::Struct {
                                path: Path {
                                    segments: vec![PathSegment {
                                        name: seen_common::Spanned::new(name_val.leak(), span),
                                        generic_args: vec![],
                                    }],
                                    span,
                                },
                                fields,
                            }
                            } else {
                                // Not a struct literal, just an identifier
                                ExprKind::Identifier(seen_common::Spanned::new(name_val.leak(), span))
                            }
                        } else {
                            // No '{' following, just an identifier
                            ExprKind::Identifier(seen_common::Spanned::new(name_val.leak(), span))
                        }
                    }
                }
                TokenType::LeftParen => {
                    self.advance(); // consume '('
                    let expr = self.parse_expression()?;
                    self.expect_token(TokenType::RightParen)?;
                    return Ok(expr);
                }
                TokenType::BitwiseOr => {
                    // Parse closure: |param| expr or |param| { block }
                    return self.parse_closure_expression();
                }
                TokenType::LeftBracket => {
                    // Parse array literal: [1, 2, 3]
                    self.advance(); // consume '['
                    let elements = self.parse_expression_list()?;
                    self.expect_token(TokenType::RightBracket)?;
                    ExprKind::Array(elements)
                }
                TokenType::KeywordMatch => {
                    // Parse match expression
                    return self.parse_match_expression();
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
        
        // Consume 'if' - can be either keyword or identifier
        if matches!(self.current_token(), Some(Token { value: TokenType::KeywordIf, .. })) {
            self.advance();
        } else if matches!(self.current_token(), Some(Token { value: TokenType::Identifier(name), .. }) if name == "if") {
            self.advance();
        } else {
            self.error("Expected 'if'");
            return Err(seen_common::SeenError::parse_error("Expected 'if'"));
        }
        
        let condition = Box::new(self.parse_expression()?);
        let then_branch = self.parse_block()?;
        
        let else_branch = if self.match_token(&TokenType::Identifier("else".to_string())) {
            // Check if it's a block or an if expression
            if matches!(self.current_token(), Some(Token { value: TokenType::LeftBrace, .. })) {
                // else { block }
                let block = self.parse_block()?;
                Some(Box::new(Expr {
                    kind: Box::new(ExprKind::Block(block)),
                    span: self.current_span(),
                    id: self.next_node_id(),
                }))
            } else {
                // else if ... or other expression
                Some(Box::new(self.parse_expression()?))
            }
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
    
    fn parse_closure_expression(&mut self) -> SeenResult<Expr<'static>> {
        let span = self.current_span();
        
        // Consume first |
        self.expect_token(TokenType::BitwiseOr)?;
        
        // Parse parameters
        let mut params = Vec::new();
        
        // Check for empty parameter list ||
        if !matches!(self.current_token(), Some(Token { value: TokenType::BitwiseOr, .. })) {
            loop {
                let param_name = self.expect_identifier_value()?;
                let param_span = self.previous_span();
                
                // Check for optional type annotation
                let ty = if self.match_token(&TokenType::Colon) {
                    Some(self.parse_type()?)
                } else {
                    None
                };
                
                params.push(ClosureParam {
                    name: seen_common::Spanned::new(param_name.leak(), param_span),
                    ty,
                });
                
                if !self.match_token(&TokenType::Comma) {
                    break;
                }
            }
        }
        
        // Consume closing |
        self.expect_token(TokenType::BitwiseOr)?;
        
        // Check for return type annotation
        let return_type = if self.match_token(&TokenType::Arrow) {
            Some(self.parse_type()?)
        } else {
            None
        };
        
        // Parse body - either expression or block
        let body = if matches!(self.current_token(), Some(Token { value: TokenType::LeftBrace, .. })) {
            ClosureBody::Block(self.parse_block()?)
        } else {
            ClosureBody::Expression(Box::new(self.parse_expression()?))
        };
        
        Ok(Expr {
            kind: Box::new(ExprKind::Closure(Closure {
                params,
                body,
                return_type,
            })),
            span,
            id: self.next_node_id(),
        })
    }
    
    fn parse_match_expression(&mut self) -> SeenResult<Expr<'static>> {
        // Consume 'match' - can be either keyword or identifier
        if matches!(self.current_token(), Some(Token { value: TokenType::KeywordMatch, .. })) {
            self.advance();
        } else if matches!(self.current_token(), Some(Token { value: TokenType::Identifier(name), .. }) if name == "match") {
            self.advance();
        } else {
            self.error("Expected 'match'");
            return Err(seen_common::SeenError::parse_error("Expected 'match'"));
        }
        
        self.parse_match_expression_impl()
    }
    
    fn parse_match_expression_impl(&mut self) -> SeenResult<Expr<'static>> {
        let span = self.current_span();
        
        let scrutinee = Box::new(self.parse_expression()?);
        
        self.expect_token(TokenType::LeftBrace)?;
        
        let mut arms = Vec::new();
        
        while !matches!(self.current_token(), Some(Token { value: TokenType::RightBrace, .. })) {
            // Parse pattern
            let pattern = self.parse_pattern()?;
            
            // Check for guard clause (if condition)
            let guard = if self.match_token(&TokenType::Identifier("if".to_string())) {
                Some(self.parse_expression()?)
            } else {
                None
            };
            
            // Expect =>
            self.expect_token(TokenType::FatArrow)?;
            
            // Parse arm body
            let body = self.parse_expression()?;
            
            arms.push(MatchArm {
                pattern,
                guard,
                body,
            });
            
            // Consume optional comma
            self.match_token(&TokenType::Comma);
            
            // Check for closing brace
            if matches!(self.current_token(), Some(Token { value: TokenType::RightBrace, .. })) {
                break;
            }
        }
        
        self.expect_token(TokenType::RightBrace)?;
        
        Ok(Expr {
            kind: Box::new(ExprKind::Match {
                scrutinee,
                arms,
            }),
            span,
            id: self.next_node_id(),
        })
    }
    
    fn parse_pattern(&mut self) -> SeenResult<Pattern<'static>> {
        let span = self.current_span();
        
        let kind = if let Some(token) = self.current_token() {
            match &token.value {
                TokenType::Identifier(name) => {
                    if name == "_" {
                        self.advance();
                        PatternKind::Wildcard
                    } else {
                        let name_val = name.clone();
                        self.advance();
                        PatternKind::Identifier(seen_common::Spanned::new(name_val.leak(), span))
                    }
                }
                TokenType::IntegerLiteral(val) => {
                    let value = *val;
                    self.advance();
                    PatternKind::Literal(Literal {
                        kind: LiteralKind::Integer(value),
                        span,
                    })
                }
                TokenType::StringLiteral(val) => {
                    let value = val.clone();
                    self.advance();
                    PatternKind::Literal(Literal {
                        kind: LiteralKind::String(value.leak()),
                        span,
                    })
                }
                TokenType::BooleanLiteral(val) => {
                    let value = *val;
                    self.advance();
                    PatternKind::Literal(Literal {
                        kind: LiteralKind::Boolean(value),
                        span,
                    })
                }
                _ => {
                    self.error("Expected pattern");
                    return Err(seen_common::SeenError::parse_error("Expected pattern"));
                }
            }
        } else {
            self.error("Unexpected end of file in pattern");
            return Err(seen_common::SeenError::parse_error("Unexpected EOF in pattern"));
        };
        
        Ok(Pattern {
            kind,
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
    
    // Kotlin-inspired feature parsing methods
    
    fn parse_extension_function(&mut self) -> SeenResult<Item<'static>> {
        let start_span = self.current_span();
        
        #[cfg(test)]
        eprintln!("parse_extension_function: Starting");
        
        // Consume 'extension'
        self.expect_identifier("extension")?;
        
        #[cfg(test)]
        eprintln!("parse_extension_function: Consumed 'extension', current token = {:?}", self.current_token());
        
        // Expect 'func'
        self.expect_keyword(TokenType::KeywordFunc)?;
        
        #[cfg(test)]
        eprintln!("parse_extension_function: Consumed 'func', current token = {:?}", self.current_token());
        
        // Parse receiver type (e.g., "String" in "String.isEmpty")
        let receiver_type_name = self.expect_identifier_value()?;
        let receiver_span = self.previous_span();
        
        #[cfg(test)]
        eprintln!("parse_extension_function: Got receiver type '{}', current token = {:?}", receiver_type_name, self.current_token());
        
        // Expect '.'
        self.expect_token(TokenType::Dot)?;
        
        #[cfg(test)]
        eprintln!("parse_extension_function: Consumed '.', current token = {:?}", self.current_token());
        
        // Parse function name
        let func_name = self.expect_identifier_value()?;
        let name_span = self.previous_span();
        
        #[cfg(test)]
        eprintln!("parse_extension_function: Got function name '{}', current token = {:?}", func_name, self.current_token());
        
        // Parse parameters (excluding implicit 'self')
        self.expect_token(TokenType::LeftParen)?;
        
        #[cfg(test)]
        eprintln!("parse_extension_function: About to parse params, current token = {:?}", self.current_token());
        
        let params = self.parse_parameter_list()?;
        
        #[cfg(test)]
        eprintln!("parse_extension_function: Parsed params, current token = {:?}", self.current_token());
        
        self.expect_token(TokenType::RightParen)?;
        
        #[cfg(test)]
        eprintln!("parse_extension_function: Consumed ')', current token = {:?}", self.current_token());
        
        // Parse return type
        let return_type = if self.match_token(&TokenType::Colon) {
            #[cfg(test)]
            eprintln!("parse_extension_function: Parsing return type");
            
            Some(self.parse_type()?)
        } else {
            None
        };
        
        #[cfg(test)]
        eprintln!("parse_extension_function: About to parse body, current token = {:?}", self.current_token());
        
        // Parse body (parse_block already expects LeftBrace)
        let body = self.parse_block()?;
        
        #[cfg(test)]
        eprintln!("parse_extension_function: Parsed body, creating extension function");
        
        // Create the extension function
        let receiver_type = Type {
            kind: Box::new(TypeKind::Named {
                path: Path {
                    segments: vec![PathSegment {
                        name: seen_common::Spanned::new(receiver_type_name.leak(), receiver_span),
                        generic_args: vec![],
                    }],
                    span: receiver_span,
                },
                generic_args: vec![],
            }),
            span: receiver_span,
        };
        
        let function = Function {
            name: seen_common::Spanned::new(func_name.leak(), name_span),
            params,
            return_type,
            body,
            visibility: Visibility::Public,
            attributes: vec![],
        };
        
        let ext_func = ExtensionFunction {
            receiver_type,
            function,
        };
        
        Ok(Item {
            kind: ItemKind::ExtensionFunction(ext_func),
            span: start_span.combine(self.previous_span()),
            id: self.next_node_id(),
        })
    }
    
    fn parse_data_class(&mut self) -> SeenResult<Item<'static>> {
        let start_span = self.current_span();
        
        // Consume 'data'
        self.expect_identifier("data")?;
        
        #[cfg(test)]
        eprintln!("parse_data_class: After 'data', current token = {:?}", self.current_token());
        
        // Expect 'class' (or 'struct' for compatibility)
        if !self.match_identifier("class") && !self.match_identifier("struct") {
            #[cfg(test)]
            eprintln!("parse_data_class: Failed to match 'class' or 'struct'");
            return Err(seen_common::SeenError::parse_error("Expected 'class' or 'struct' after 'data'"));
        }
        
        // Parse class name
        let name = self.expect_identifier_value()?;
        let name_span = self.previous_span();
        
        // Parse generic parameters if present
        let generic_params = if self.match_token(&TokenType::Less) {
            self.parse_generic_params()?
        } else {
            vec![]
        };
        
        // Parse fields (data classes use parentheses for constructor parameters)
        self.expect_token(TokenType::LeftParen)?;
        let fields = self.parse_data_class_fields()?;
        self.expect_token(TokenType::RightParen)?;
        
        // Expect semicolon to end the data class declaration
        self.expect_token(TokenType::Semicolon)?;
        
        // Create the data class
        let data_class = DataClass {
            name: seen_common::Spanned::new(name.leak(), name_span),
            fields,
            visibility: Visibility::Public,
            generic_params,
            attributes: vec![],
        };
        
        Ok(Item {
            kind: ItemKind::DataClass(data_class),
            span: start_span.combine(self.previous_span()),
            id: self.next_node_id(),
        })
    }
    
    fn parse_sealed_class(&mut self) -> SeenResult<Item<'static>> {
        let start_span = self.current_span();
        
        // Consume 'sealed'
        self.expect_identifier("sealed")?;
        
        // Expect 'class' (or 'enum' for compatibility)
        if !self.match_identifier("class") && !self.match_identifier("enum") {
            return Err(seen_common::SeenError::parse_error("Expected 'class' or 'enum' after 'sealed'"));
        }
        
        // Parse class name
        let name = self.expect_identifier_value()?;
        let name_span = self.previous_span();
        
        // Parse generic parameters if present
        let generic_params = if self.match_token(&TokenType::Less) {
            self.parse_generic_params()?
        } else {
            vec![]
        };
        
        // Parse variants
        self.expect_token(TokenType::LeftBrace)?;
        let mut variants = vec![];
        
        while !self.check(&TokenType::RightBrace) && !self.is_at_end() {
            let variant = self.parse_sealed_variant()?;
            variants.push(variant);
            
            // Optional comma
            self.match_token(&TokenType::Comma);
        }
        
        self.expect_token(TokenType::RightBrace)?;
        
        // Create the sealed class  
        let sealed_class = SealedClass {
            name: seen_common::Spanned::new(name.leak(), name_span),
            variants: variants.into_iter().map(|v| SealedClassVariant {
                name: v.name,
                fields: match v.data {
                    VariantData::Struct(fields) => {
                        fields.into_iter().map(|f| DataClassField {
                            name: f.name,
                            ty: f.ty,
                            is_mutable: false,
                            default_value: None,
                            visibility: f.visibility,
                            span: f.span,
                        }).collect()
                    },
                    _ => vec![], // For now, only struct variants are supported
                },
                span: v.span,
            }).collect(),
            visibility: Visibility::Public,
            generic_params,
            attributes: vec![],
        };
        
        Ok(Item {
            kind: ItemKind::SealedClass(sealed_class),
            span: start_span.combine(self.previous_span()),
            id: self.next_node_id(),
        })
    }
    
    fn parse_sealed_variant(&mut self) -> SeenResult<Variant<'static>> {
        let span = self.current_span();
        let name = self.expect_identifier_value()?;
        let name_span = self.previous_span();
        
        let data = if self.match_token(&TokenType::LeftParen) {
            // Tuple variant
            let mut types = vec![];
            if !self.check(&TokenType::RightParen) {
                loop {
                    types.push(self.parse_type()?);
                    if !self.match_token(&TokenType::Comma) {
                        break;
                    }
                }
            }
            self.expect_token(TokenType::RightParen)?;
            VariantData::Tuple(types)
        } else if self.match_token(&TokenType::LeftBrace) {
            // Struct variant
            let fields = self.parse_struct_fields()?;
            self.expect_token(TokenType::RightBrace)?;
            VariantData::Struct(fields)
        } else {
            // Unit variant
            VariantData::Unit
        };
        
        Ok(Variant {
            name: seen_common::Spanned::new(name.leak(), name_span),
            data,
            span: span.combine(self.previous_span()),
        })
    }
}