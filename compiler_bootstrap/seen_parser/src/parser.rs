//! Parser implementation

use crate::ast::*;
use seen_lexer::{Token, TokenType};
use seen_common::{SeenResult, Diagnostics, Span, Spanned};

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
    
    fn insert_token_at_current(&mut self, token_type: TokenType) {
        let span = self.current_span();
        let token = Token::new(token_type, span);
        self.tokens.insert(self.current, token);
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
        
        // Parse attributes first
        let attributes = self.parse_attributes()?;
        
        // Debug: Print current token
        #[cfg(test)]
        if let Some(token) = self.current_token() {
            eprintln!("parse_item: Current token = {:?}", token);
        }
        
        if let Some(token) = self.current_token() {
            match &token.value {
                TokenType::KeywordFun => self.parse_function_with_attributes(attributes),
                TokenType::KeywordSuspend => self.parse_suspend_function(),
                TokenType::KeywordInline => self.parse_inline_function(),
                TokenType::KeywordStruct => self.parse_struct(),
                TokenType::KeywordEnum => self.parse_enum(),
                TokenType::KeywordTrait => self.parse_trait(),
                TokenType::KeywordImpl => self.parse_impl(),
                TokenType::KeywordData => self.parse_data_class(),
                TokenType::KeywordSealed => self.parse_sealed_class(),
                TokenType::KeywordObject => self.parse_object_declaration(),
                TokenType::KeywordInterface => self.parse_interface(),
                TokenType::Identifier(name) => {
                // Check for Kotlin-style features and backwards compatibility
                match name.as_str() {
                    "extension" => self.parse_extension_function(),
                    "data" => {
                        self.advance(); // Consume the 'data' token
                        self.parse_data_class()
                    },
                    "sealed" => self.parse_sealed_class(),
                    "inline" => {
                        self.advance(); // Consume the 'inline' token
                        self.parse_inline_function()
                    },
                    "tailrec" => {
                        self.advance(); // Consume the 'tailrec' token
                        self.parse_tailrec_function()
                    },
                    "operator" => {
                        self.advance(); // Consume the 'operator' token
                        self.parse_operator_function()
                    },
                    "infix" => {
                        self.advance(); // Consume the 'infix' token
                        self.parse_infix_function()
                    },
                    "typealias" | "type" => {
                        self.advance(); // Consume the 'typealias' or 'type' token
                        self.parse_type_alias()
                    },
                    "fun" => self.parse_function(),
                    "struct" => self.parse_struct(),
                    "enum" => self.parse_enum(),
                    "trait" => self.parse_trait(),
                    "impl" => self.parse_impl(),
                    _ => {
                        eprintln!("parse_item: Unexpected identifier '{}'", name);
                        self.error("Expected 'fun', 'struct', 'enum', 'extension', 'data', 'sealed', 'inline', 'tailrec', 'operator', 'infix', or 'typealias'");
                        Err(seen_common::SeenError::parse_error("Unexpected token"))
                    }
                }
                }
                _ => {
                    self.error("Expected item declaration");
                    Err(seen_common::SeenError::parse_error("Expected item"))
                }
            }
        } else {
            self.error("Expected item declaration");
            Err(seen_common::SeenError::parse_error("Expected item"))
        }
    }
    
    fn parse_function(&mut self) -> SeenResult<Item<'static>> {
        self.parse_function_with_attributes(Vec::new())
    }
    
    fn parse_function_with_attributes(&mut self, attributes: Vec<Attribute<'static>>) -> SeenResult<Item<'static>> {
        let start_span = self.current_span();
        
        // Consume 'fun'
        #[cfg(test)]
        eprintln!("parse_function: About to consume 'fun', current token = {:?}", self.current_token());
        self.expect_keyword(TokenType::KeywordFun)?;
        
        #[cfg(test)]
        eprintln!("parse_function: After consuming 'fun', current token = {:?}", self.current_token());
        
        // Parse function name
        let name = self.expect_identifier_value()?;
        let name_span = self.previous_span();
        
        #[cfg(test)]
        eprintln!("parse_function: Parsed function name '{}', checking for generic params", name);
        
        // Parse optional generic type parameters (after function name)
        let type_params = if self.check(&TokenType::Less) {
            #[cfg(test)]
            eprintln!("parse_function: Found '<', parsing generic type params");
            self.parse_generic_type_params()?
        } else {
            #[cfg(test)]
            eprintln!("parse_function: No '<' found, continuing to parameters");
            Vec::new()
        };
        
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
            type_params,
            params,
            return_type,
            body,
            visibility: Visibility::Private,
            attributes,
            is_inline: false,
            is_suspend: false,
            is_operator: false,
            is_infix: false,
            is_tailrec: false,
        };
        
        Ok(Item {
            kind: ItemKind::Function(func),
            span: start_span,
            id: self.next_node_id(),
        })
    }

    fn try_parse_generic_args(&mut self) -> SeenResult<Option<Vec<Type<'static>>>> {
        // Try to parse generic type arguments like <T>, <T, U>, <User?>
        if !self.check(&TokenType::Less) {
            return Ok(None);
        }
        
        let saved_pos = self.current;
        self.advance(); // consume '<'
        
        let mut args = Vec::new();
        
        // Parse first type argument
        match self.parse_type() {
            Ok(ty) => args.push(ty),
            Err(_) => {
                // Failed to parse type, restore position
                self.current = saved_pos;
                return Ok(None);
            }
        }
        
        // Parse additional type arguments
        while self.match_token(&TokenType::Comma) {
            match self.parse_type() {
                Ok(ty) => args.push(ty),
                Err(_) => {
                    // Failed to parse type, restore position
                    self.current = saved_pos;
                    return Ok(None);
                }
            }
        }
        
        // Expect closing '>'
        if !self.match_token(&TokenType::Greater) {
            // No closing '>', restore position
            self.current = saved_pos;
            return Ok(None);
        }
        
        Ok(Some(args))
    }
    
    fn parse_generic_params_with_reified(&mut self) -> SeenResult<Vec<TypeParam<'static>>> {
        // Same as parse_generic_type_params but supports 'reified' modifier
        self.expect_token(TokenType::Less)?;
        let mut params = Vec::new();
        
        loop {
            let span = self.current_span();
            
            // Check for 'reified' modifier
            let _is_reified = self.match_token(&TokenType::KeywordReified);
            
            let name = self.expect_identifier_value()?;
            let name_span = self.previous_span();
            
            // Parse optional bounds
            let bounds = if self.match_token(&TokenType::Colon) {
                let mut bounds = Vec::new();
                loop {
                    bounds.push(self.parse_type()?);
                    if !self.match_token(&TokenType::Plus) {
                        break;
                    }
                }
                bounds
            } else {
                Vec::new()
            };
            
            // Parse optional default type
            let default_type = if self.match_token(&TokenType::Assign) {
                Some(self.parse_type()?)
            } else {
                None
            };
            
            let param = TypeParam {
                name: seen_common::Spanned::new(name.leak(), name_span),
                bounds,
                default_type,
                span,
            };
            
            // TODO: Add is_reified field to TypeParam to track reified status
            
            params.push(param);
            
            if !self.match_token(&TokenType::Comma) {
                break;
            }
        }
        
        self.expect_token(TokenType::Greater)?;
        Ok(params)
    }
    
    fn parse_generic_type_params(&mut self) -> SeenResult<Vec<TypeParam<'static>>> {
        let mut type_params = Vec::new();
        
        // Consume '<'
        self.expect_token(TokenType::Less)?;
        
        if !self.check(&TokenType::Greater) {
            loop {
                let param_start_span = self.current_span();
                let param_name = self.expect_identifier_value()?;
                let param_name_span = self.previous_span();
                
                // Parse optional bounds (T: Trait1 + Trait2)
                let bounds = if self.match_token(&TokenType::Colon) {
                    let mut bounds = Vec::new();
                    bounds.push(self.parse_type()?);
                    
                    while self.match_token(&TokenType::Plus) {
                        bounds.push(self.parse_type()?);
                    }
                    bounds
                } else {
                    Vec::new()
                };
                
                // Parse optional default type (T = String)
                let default_type = if self.match_token(&TokenType::Assign) {
                    Some(self.parse_type()?)
                } else {
                    None
                };
                
                let param_name_static: &'static str = Box::leak(param_name.into_boxed_str());
                type_params.push(TypeParam {
                    name: seen_common::Spanned::new(param_name_static, param_name_span),
                    bounds,
                    default_type,
                    span: param_start_span.combine(self.previous_span()),
                });
                
                if !self.match_token(&TokenType::Comma) {
                    break;
                }
            }
        }
        
        // Handle >> as two > tokens for nested generics
        if matches!(self.current_token(), Some(Token { value: TokenType::RightShift, .. })) {
            self.advance();
            self.insert_token_at_current(TokenType::Greater);
        } else {
            self.expect_token(TokenType::Greater)?;
        }
        
        Ok(type_params)
    }

    fn parse_tailrec_function(&mut self) -> SeenResult<Item<'static>> {
        let start_span = self.previous_span(); // 'tailrec' already consumed
        
        // 'tailrec' has already been consumed by parse_item
        
        // Consume 'fun'
        self.expect_keyword(TokenType::KeywordFun)?;
        
        // Parse function name
        let name = self.expect_identifier_value()?;
        let name_span = self.previous_span();
        
        // Parse optional generic type parameters
        let type_params = if self.check(&TokenType::Less) {
            self.parse_generic_type_params()?
        } else {
            Vec::new()
        };
        
        // Parse parameters
        self.expect_token(TokenType::LeftParen)?;
        let params = self.parse_parameter_list()?;
        self.expect_token(TokenType::RightParen)?;
        
        // Parse optional return type
        let return_type = if self.match_token(&TokenType::Colon) {
            Some(self.parse_type()?)
        } else {
            None
        };
        
        // Parse body
        let body = self.parse_block()?;
        
        let func = Function {
            name: seen_common::Spanned::new(name.leak(), name_span),
            type_params,
            params,
            return_type,
            body,
            visibility: Visibility::Private,
            attributes: Vec::new(),
            is_inline: false,
            is_suspend: false,
            is_operator: false,
            is_infix: false,
            is_tailrec: true,
        };
        
        Ok(Item {
            kind: ItemKind::Function(func),
            span: start_span,
            id: self.next_node_id(),
        })
    }
    
    fn parse_operator_function(&mut self) -> SeenResult<Item<'static>> {
        let start_span = self.previous_span(); // 'operator' already consumed
        
        // Consume 'fun'
        self.expect_keyword(TokenType::KeywordFun)?;
        
        // Parse function name (can be an operator like 'plus', 'minus', etc.)
        let name = self.expect_identifier_value()?;
        let name_span = self.previous_span();
        
        // Parse optional generic type parameters
        let type_params = if self.check(&TokenType::Less) {
            self.parse_generic_type_params()?
        } else {
            Vec::new()
        };
        
        // Parse parameters
        self.expect_token(TokenType::LeftParen)?;
        let params = self.parse_parameter_list()?;
        self.expect_token(TokenType::RightParen)?;
        
        // Parse optional return type
        let return_type = if self.match_token(&TokenType::Colon) {
            Some(self.parse_type()?)
        } else {
            None
        };
        
        // Parse body
        let body = self.parse_block()?;
        
        let func = Function {
            name: seen_common::Spanned::new(name.leak(), name_span),
            type_params,
            params,
            return_type,
            body,
            visibility: Visibility::Private,
            attributes: Vec::new(),
            is_inline: false,
            is_suspend: false,
            is_operator: true,
            is_infix: false,
            is_tailrec: false,
        };
        
        Ok(Item {
            kind: ItemKind::Function(func),
            span: start_span,
            id: self.next_node_id(),
        })
    }
    
    fn parse_infix_function(&mut self) -> SeenResult<Item<'static>> {
        let start_span = self.previous_span(); // 'infix' already consumed
        
        // Consume 'fun'
        self.expect_keyword(TokenType::KeywordFun)?;
        
        // Parse function name
        let name = self.expect_identifier_value()?;
        let name_span = self.previous_span();
        
        // Parse optional generic type parameters
        let type_params = if self.check(&TokenType::Less) {
            self.parse_generic_type_params()?
        } else {
            Vec::new()
        };
        
        // Parse parameters (infix functions must have exactly one parameter)
        self.expect_token(TokenType::LeftParen)?;
        let params = self.parse_parameter_list()?;
        self.expect_token(TokenType::RightParen)?;
        
        // Validate infix functions have exactly one parameter
        if params.len() != 1 {
            self.error("Infix functions must have exactly one parameter");
            return Err(seen_common::SeenError::parse_error("Invalid infix function"));
        }
        
        // Parse optional return type
        let return_type = if self.match_token(&TokenType::Colon) {
            Some(self.parse_type()?)
        } else {
            None
        };
        
        // Parse body
        let body = self.parse_block()?;
        
        let func = Function {
            name: seen_common::Spanned::new(name.leak(), name_span),
            type_params,
            params,
            return_type,
            body,
            visibility: Visibility::Private,
            attributes: Vec::new(),
            is_inline: false,
            is_suspend: false,
            is_operator: false,
            is_infix: true,
            is_tailrec: false,
        };
        
        Ok(Item {
            kind: ItemKind::Function(func),
            span: start_span,
            id: self.next_node_id(),
        })
    }
    
    fn parse_inline_function(&mut self) -> SeenResult<Item<'static>> {
        let start_span = self.previous_span(); // 'inline' already consumed
        
        // 'inline' has already been consumed by parse_item
        
        // Consume 'fun'
        self.expect_keyword(TokenType::KeywordFun)?;
        
        // Parse function name
        let name = self.expect_identifier_value()?;
        let name_span = self.previous_span();
        
        // Parse optional generic type parameters (after function name, with possible reified)
        let type_params = if self.check(&TokenType::Less) {
            self.parse_generic_params_with_reified()?
        } else {
            Vec::new()
        };
        
        // Parse parameters
        self.expect_token(TokenType::LeftParen)?;
        let params = self.parse_parameter_list()?;
        self.expect_token(TokenType::RightParen)?;
        
        // Parse optional return type
        let return_type = if self.match_token(&TokenType::Colon) {
            Some(self.parse_type()?)
        } else {
            None
        };
        
        // Parse body
        let body = self.parse_block()?;
        
        // Create inline attribute
        let inline_attr = Attribute {
            name: seen_common::Spanned::new("inline", start_span),
            args: Vec::new(),
            span: start_span,
        };
        
        let func = Function {
            name: seen_common::Spanned::new(name.leak(), name_span),
            type_params,
            params,
            return_type,
            body,
            visibility: Visibility::Private,
            attributes: vec![inline_attr],
            is_inline: true,
            is_suspend: false,
            is_operator: false,
            is_infix: false,
            is_tailrec: false,
        };
        
        Ok(Item {
            kind: ItemKind::Function(func),
            span: start_span,
            id: self.next_node_id(),
        })
    }
    
    fn parse_suspend_function(&mut self) -> SeenResult<Item<'static>> {
        let start_span = self.current_span();
        
        // Consume 'suspend'
        self.expect_keyword(TokenType::KeywordSuspend)?;
        
        // Consume 'fun'
        self.expect_keyword(TokenType::KeywordFun)?;
        
        // Parse function name
        let name = self.expect_identifier_value()?;
        let name_span = self.previous_span();
        
        // Parse optional generic type parameters (after function name)
        let type_params = if self.check(&TokenType::Less) {
            self.parse_generic_type_params()?
        } else {
            Vec::new()
        };
        
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
        
        // Create a suspend attribute to mark this function as suspendable
        let suspend_attr = Attribute {
            name: seen_common::Spanned::new("suspend", start_span),
            args: Vec::new(),
            span: start_span,
        };
        
        let func = Function {
            name: seen_common::Spanned::new(name_static, name_span),
            type_params,
            params,
            return_type,
            body,
            visibility: Visibility::Private,
            attributes: vec![suspend_attr], // Mark as suspend function
            is_inline: false,
            is_suspend: true,
            is_operator: false,
            is_infix: false,
            is_tailrec: false,
        };
        
        Ok(Item {
            kind: ItemKind::Function(func),
            span: start_span,
            id: self.next_node_id(),
        })
    }
    
    fn parse_type_alias(&mut self) -> SeenResult<Item<'static>> {
        let start_span = self.previous_span(); // 'typealias' or 'type' already consumed
        
        // Parse type alias name
        let name = self.expect_identifier_value()?;
        let name_span = self.previous_span();
        
        // Parse optional generic parameters
        let generic_params = if self.check(&TokenType::Less) {
            self.parse_generic_params()?
        } else {
            Vec::new()
        };
        
        // Expect '='
        self.expect_token(TokenType::Assign)?;
        
        // Parse the actual type
        let ty = self.parse_type()?;
        
        let type_alias = TypeAlias {
            name: seen_common::Spanned::new(name.leak(), name_span),
            ty,
            generic_params,
            visibility: Visibility::Public,
        };
        
        Ok(Item {
            kind: ItemKind::TypeAlias(type_alias),
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
            companion_object: None,
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
        if let Some(token) = self.current_token() {
            if let TokenType::Identifier(name) = &token.value {
                if name == expected {
                    self.advance();
                    return Ok(());
                }
            }
        }
        
        self.error(&format!("Expected '{}'", expected));
        Err(seen_common::SeenError::parse_error("Expected identifier"))
    }
    
    fn expect_identifier_value(&mut self) -> SeenResult<String> {
        #[cfg(test)]
        eprintln!("expect_identifier_value: current token = {:?}", self.current_token());
        
        if let Some(token) = self.current_token() {
            match &token.value {
                TokenType::Identifier(name) => {
                    let result = name.clone();
                    #[cfg(test)]
                    eprintln!("expect_identifier_value: found identifier '{}', advancing", result);
                    self.advance();
                    return Ok(result);
                }
                // Allow certain keywords to be used as identifiers in contexts where they don't conflict
                TokenType::KeywordData => {
                    #[cfg(test)]
                    eprintln!("expect_identifier_value: found keyword 'data' as identifier, advancing");
                    self.advance();
                    return Ok("data".to_string());
                }
                _ => {}
            }
        }
        
        #[cfg(test)]
        eprintln!("expect_identifier_value: no identifier found, current token = {:?}", self.current_token());
        self.error("Expected identifier");
        Err(seen_common::SeenError::parse_error("Expected identifier"))
    }
    
    fn match_identifier(&mut self, expected: &str) -> bool {
        if let Some(token) = self.current_token() {
            if let TokenType::Identifier(name) = &token.value {
                if name == expected {
                    self.advance();
                    return true;
                }
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
            // Note: 'var' is tokenized as Identifier("var"), 'val' as KeywordVal
            let is_mutable = if self.match_identifier("var") {
                true
            } else if self.match_token(&TokenType::KeywordVal) {
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
                delegate: None, // For now, we don't support delegation in data class fields
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
                TokenType::Whitespace | TokenType::Semicolon => {
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
                Some(Token { value: TokenType::KeywordFun, .. }) |
                Some(Token { value: TokenType::KeywordStruct, .. }) |
                Some(Token { value: TokenType::KeywordEnum, .. }) => {
                    break;
                }
                Some(Token { value: TokenType::Identifier(name), .. }) => {
                    // Also check for keyword identifiers (backward compatibility)
                    if matches!(name.as_str(), "fun" | "struct" | "enum" | "extension" | "data" | "sealed") {
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
        
        // Handle suspend modifier for function types
        let is_suspend = self.match_token(&TokenType::KeywordSuspend);
        
        // Check for function type syntax: () -> Type or (Type, Type) -> Type
        if self.check(&TokenType::LeftParen) {
            return self.parse_function_type(is_suspend);
        }
        
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
                // Named type - check for generic arguments
                let generic_args = if self.match_token(&TokenType::Less) {
                    self.parse_generic_type_args()?
                } else {
                    Vec::new()
                };
                
                let path = Path {
                    segments: vec![PathSegment {
                        name: seen_common::Spanned::new(type_name.leak(), span),
                        generic_args: generic_args.clone(),
                    }],
                    span,
                };
                TypeKind::Named { path, generic_args }
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

    fn parse_function_type(&mut self, is_suspend: bool) -> SeenResult<Type<'static>> {
        let span = self.current_span();
        
        // Parse parameter types
        self.expect_token(TokenType::LeftParen)?;
        let mut param_types = Vec::new();
        
        if !self.check(&TokenType::RightParen) {
            loop {
                param_types.push(self.parse_type()?);
                if !self.match_token(&TokenType::Comma) {
                    break;
                }
            }
        }
        
        self.expect_token(TokenType::RightParen)?;
        self.expect_token(TokenType::Arrow)?;
        
        let return_type = Box::new(self.parse_type()?);
        
        // For now, represent function types as named types with special names
        // This is a simplification - proper function types would need their own TypeKind variant
        let func_type_name = if is_suspend {
            format!("SuspendFunc<{}>", return_type.span.file_id).leak()
        } else {
            format!("Func<{}>", return_type.span.file_id).leak()
        };
        
        Ok(Type {
            kind: Box::new(TypeKind::Named {
                path: Path {
                    segments: vec![PathSegment {
                        name: seen_common::Spanned::new(func_type_name, span),
                        generic_args: vec![],
                    }],
                    span,
                },
                generic_args: vec![],
            }),
            span,
        })
    }
    
    fn parse_generic_type_args(&mut self) -> SeenResult<Vec<Type<'static>>> {
        let mut args = Vec::new();
        
        // Parse first type argument
        args.push(self.parse_type()?);
        
        // Parse additional type arguments separated by commas
        while self.match_token(&TokenType::Comma) {
            args.push(self.parse_type()?);
        }
        
        // Expect closing '>' - handle >> as two > tokens
        if matches!(self.current_token(), Some(Token { value: TokenType::RightShift, .. })) {
            // Convert >> to two > tokens by advancing and inserting a Greater token
            self.advance(); // consume >>
            // Insert a Greater token for the next parse
            self.insert_token_at_current(TokenType::Greater);
        } else {
            self.expect_token(TokenType::Greater)?;
        }
        
        Ok(args)
    }
    
    fn parse_attributes(&mut self) -> SeenResult<Vec<Attribute<'static>>> {
        let mut attributes = Vec::new();
        
        while self.check(&TokenType::At) {
            let start_span = self.current_span();
            self.advance(); // consume '@'
            
            let name = self.expect_identifier_value()?;
            let name_span = self.previous_span();
            let name_static: &'static str = Box::leak(name.into_boxed_str());
            
            // Parse optional arguments
            let args = if self.check(&TokenType::LeftParen) {
                self.advance(); // consume '('
                let mut attr_args = Vec::new();
                
                if !self.check(&TokenType::RightParen) {
                    loop {
                        // For now, just support literal arguments
                        if let Some(token) = self.current_token() {
                            match &token.value {
                                TokenType::StringLiteral(s) => {
                                    let s_static: &'static str = Box::leak(s.clone().into_boxed_str());
                                    attr_args.push(AttrArg::Literal(
                                        Literal {
                                            kind: LiteralKind::String(s_static),
                                            span: self.current_span(),
                                        }
                                    ));
                                    self.advance();
                                }
                                TokenType::IntegerLiteral(n) => {
                                    attr_args.push(AttrArg::Literal(
                                        Literal {
                                            kind: LiteralKind::Integer(*n),
                                            span: self.current_span(),
                                        }
                                    ));
                                    self.advance();
                                }
                                _ => {
                                    return Err(seen_common::SeenError::parse_error("Expected literal in attribute argument"));
                                }
                            }
                        }
                        
                        if !self.match_token(&TokenType::Comma) {
                            break;
                        }
                    }
                }
                
                self.expect_token(TokenType::RightParen)?;
                attr_args
            } else {
                Vec::new()
            };
            
            attributes.push(Attribute {
                name: seen_common::Spanned::new(name_static, name_span),
                args,
                span: start_span,
            });
        }
        
        Ok(attributes)
    }
    
    fn parse_block(&mut self) -> SeenResult<Block<'static>> {
        let start_span = self.current_span();
        
        #[cfg(test)]
        eprintln!("parse_block: ENTER - expecting LeftBrace, current token = {:?}", self.current_token());
        
        self.expect_token(TokenType::LeftBrace)?;
        
        #[cfg(test)]
        eprintln!("parse_block: After consuming LeftBrace, current token = {:?}", self.current_token());
        
        let mut statements = Vec::new();
        
        while let Some(token) = self.current_token() {
            if matches!(token.value, TokenType::RightBrace) {
                #[cfg(test)]
                eprintln!("parse_block: Found RightBrace, breaking loop");
                break;
            }
            if self.is_at_end() {
                self.error("Unexpected end of file in block");
                break;
            }
            
            #[cfg(test)]
            eprintln!("parse_block: About to parse statement #{}, current token = {:?}", statements.len() + 1, self.current_token());
            
            // Parse statement
            statements.push(self.parse_statement()?);
            
            #[cfg(test)]
            eprintln!("parse_block: After parsing statement #{}, current token = {:?}", statements.len(), self.current_token());
        }
        
        #[cfg(test)]
        eprintln!("parse_block: Loop ended, expecting RightBrace, current token = {:?}", self.current_token());
        
        self.expect_token(TokenType::RightBrace)?;
        
        #[cfg(test)]
        eprintln!("parse_block: EXIT - consumed RightBrace, current token = {:?}", self.current_token());
        
        Ok(Block {
            statements,
            span: start_span,
        })
    }
    
    fn parse_statement(&mut self) -> SeenResult<Stmt<'static>> {
        let span = self.current_span();
        
        let kind = if let Some(token) = self.current_token() {
            match &token.value {
                TokenType::KeywordLet | TokenType::KeywordVal => {
                self.advance(); // consume 'let' or 'val'
                    
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
                TokenType::KeywordReturn => {
                    self.advance(); // consume 'return'
                    
                    #[cfg(test)]
                    eprintln!("parse_statement: Parsing return, current token = {:?}", self.current_token());
                    
                    let value = if let Some(token) = self.current_token() {
                        if matches!(token.value, TokenType::Semicolon) {
                            None
                        } else {
                            Some(Box::new(self.parse_expression()?))
                        }
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
                TokenType::KeywordIf => {
                    let if_expr = self.parse_if_expression()?;
                    StmtKind::Expr(if_expr)
                }
                TokenType::KeywordFor => {
                    let for_expr = self.parse_for_expression()?;
                    StmtKind::Expr(for_expr)
                }
                TokenType::Identifier(name) => {
                    match name.as_str() {
                    "for" => {
                        // Backward compatibility for 'for' as identifier
                        let for_expr = self.parse_for_expression()?;
                        StmtKind::Expr(for_expr)
                    }
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
            }
        } else {
            // No token available, parse as expression statement
            let expr = self.parse_expression()?;
            StmtKind::Expr(expr)
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
        #[cfg(test)]
        eprintln!("parse_expression: Starting with current token = {:?}", self.current_token());
        
        let result = self.parse_binary_expression(0);
        
        #[cfg(test)]
        if let Err(ref e) = result {
            eprintln!("parse_expression: Failed with error = {:?}", e);
        }
        
        result
    }
    
    fn parse_binary_expression(&mut self, min_precedence: u8) -> SeenResult<Expr<'static>> {
        let mut left = self.parse_postfix_expression()?;
        
        while let Some(token) = self.current_token() {
            let (precedence, is_right_associative) = self.get_precedence(&token.value);
            
            if precedence >= 255 || precedence < min_precedence {
                break;
            }
            
            // Handle assignment specially
            if matches!(token.value, TokenType::Assign) {
                self.advance(); // consume '='
                
                let right = self.parse_binary_expression(precedence)?; // Right-associative
                
                left = Expr {
                    kind: Box::new(ExprKind::Assign {
                        target: Box::new(left),
                        value: Box::new(right),
                    }),
                    span: self.current_span(),
                    id: self.next_node_id(),
                };
                continue;
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
            if let Some(token) = self.current_token() {
                match &token.value {
                    TokenType::Less => {
                        // Check if this is a generic type argument or a comparison
                        // If the expression is an identifier and we can parse type arguments, it's a generic instantiation
                        if let ExprKind::Identifier(_) = &*expr.kind {
                            // Try to parse as generic type arguments
                            let saved_pos = self.current;
                            match self.try_parse_generic_args() {
                                Ok(Some(generic_args)) => {
                                    // Successfully parsed generic args, update the expression
                                    expr = Expr {
                                        kind: Box::new(ExprKind::GenericInstantiation {
                                            base: Box::new(expr),
                                            args: generic_args,
                                        }),
                                        span: self.current_span(),
                                        id: self.next_node_id(),
                                    };
                                    continue;
                                }
                                _ => {
                                    // Not generic args, restore position and break to handle as binary op
                                    self.current = saved_pos;
                                    break;
                                }
                            }
                        } else {
                            // Not an identifier, must be a comparison operator
                            break;
                        }
                    }
                    TokenType::Dot => {
                        self.advance(); // consume '.'
                        
                        let name = self.expect_identifier_value()?;
                        let name_span = self.previous_span();
                        
                        // Check if it's a method call or field access
                        if let Some(next_token) = self.current_token() {
                            if matches!(next_token.value, TokenType::LeftParen) {
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
                        } else {
                            // Field access (no next token)
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
                    TokenType::LeftParen => {
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
                    TokenType::LeftBracket => {
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
                    TokenType::LeftBrace => {
                    // Only parse block for known DSL patterns
                    // Don't treat arbitrary expressions followed by { as function calls
                    if let ExprKind::Identifier(name) = &*expr.kind {
                        match name.value {
                            "flow" => {
                                // Parse as FlowBuilder
                                let block = self.parse_block()?;
                                expr = Expr {
                                    kind: Box::new(ExprKind::FlowBuilder { block }),
                                    span: self.current_span(),
                                    id: self.next_node_id(),
                                };
                            }
                            "launch" => {
                                // Parse as Launch block
                                let block = self.parse_block()?;
                                expr = Expr {
                                    kind: Box::new(ExprKind::Launch { block }),
                                    span: self.current_span(),
                                    id: self.next_node_id(),
                                };
                            }
                            "reactive" => {
                                // Parse as generic block call (reactive { ... })
                                let block = self.parse_block()?;
                                let closure = crate::ast::Closure {
                                    params: Vec::new(),
                                    body: crate::ast::ClosureBody::Block(block),
                                    return_type: None,
                                };
                                let closure_expr = Expr {
                                    kind: Box::new(ExprKind::Closure(closure)),
                                    span: self.current_span(),
                                    id: self.next_node_id(),
                                };
                                expr = Expr {
                                    kind: Box::new(ExprKind::Call {
                                        function: Box::new(expr),
                                        args: vec![closure_expr],
                                    }),
                                    span: self.current_span(),
                                    id: self.next_node_id(),
                                };
                            }
                            // Only parse block for known DSL patterns
                            // Regular identifiers followed by { are not automatically treated as function calls
                            _ => {
                                // Don't consume the {, let the parent parser handle it
                                break;
                            }
                        }
                    } else {
                        // Not an identifier, don't treat { as a postfix operator
                        // This fixes the issue where "null {" was being parsed as a function call
                        break;
                    }
                    }
                    _ => break,
                }
            } else {
                break;
            }
        }
        
        Ok(expr)
    }
    
    fn parse_primary_expression(&mut self) -> SeenResult<Expr<'static>> {
        let span = self.current_span();
        
        #[cfg(test)]
        eprintln!("parse_primary_expression: Starting with current token = {:?}", self.current_token());
        
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
                        #[cfg(test)]
                        eprintln!("parse_primary_expression: Found 'null' identifier, returning Null expression");
                        
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
                        if let Some(token) = self.current_token() {
                            if matches!(token.value, TokenType::LeftBrace) {
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
                        } else {
                            // No current token, just an identifier
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
                TokenType::LeftBrace => {
                    // Could be a Kotlin-style lambda { param -> expr } or a block expression
                    // Look ahead to detect the pattern
                    if self.is_kotlin_lambda() {
                        return self.parse_kotlin_lambda();
                    } else {
                        // Parse as block expression
                        let block = self.parse_block()?;
                        return Ok(Expr {
                            kind: Box::new(ExprKind::Block(block)),
                            span,
                            id: self.next_node_id(),
                        });
                    }
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
                TokenType::KeywordAwait => {
                    // Parse await expression: await expr
                    return self.parse_await_expression();
                }
                TokenType::KeywordLaunch => {
                    // Parse launch expression: launch { block }
                    return self.parse_launch_expression();
                }
                TokenType::KeywordFlow => {
                    // Parse flow expression: flow { block }
                    return self.parse_flow_expression();
                }
                TokenType::KeywordTry => {
                    // Parse try-catch-finally expression
                    return self.parse_try_catch_expression();
                }
                TokenType::KeywordData => {
                    // Allow 'data' to be used as an identifier in expressions
                    let name_val = "data".to_string();
                    self.advance();
                    ExprKind::Identifier(seen_common::Spanned::new(name_val.leak(), span))
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
    
    fn parse_for_expression(&mut self) -> SeenResult<Expr<'static>> {
        let span = self.current_span();
        
        // Consume 'for' - can be either keyword or identifier
        if let Some(token) = self.current_token() {
            match &token.value {
                TokenType::KeywordFor => {
                    self.advance();
                }
                TokenType::Identifier(name) if name == "for" => {
                    self.advance();
                }
                _ => {
                    self.error("Expected 'for'");
                    return Err(seen_common::SeenError::parse_error("Expected 'for'"));
                }
            }
        } else {
            self.error("Expected 'for'");
            return Err(seen_common::SeenError::parse_error("Expected 'for'"));
        }
        
        // Parse pattern (usually just an identifier)
        let pattern = self.parse_pattern()?;
        
        // Expect 'in'
        if let Some(token) = self.current_token() {
            match &token.value {
                TokenType::KeywordIn => {
                    self.advance();
                }
                TokenType::Identifier(name) if name == "in" => {
                    self.advance();
                }
                _ => {
                    self.error("Expected 'in' after for pattern");
                    return Err(seen_common::SeenError::parse_error("Expected 'in'"));
                }
            }
        } else {
            self.error("Expected 'in'");
            return Err(seen_common::SeenError::parse_error("Expected 'in'"));
        }
        
        // Parse iterator expression
        let iterator = Box::new(self.parse_expression()?);
        
        // Parse body block
        let body = self.parse_block()?;
        
        Ok(Expr {
            kind: Box::new(ExprKind::For {
                pattern,
                iterator,
                body,
            }),
            span,
            id: self.next_node_id(),
        })
    }
    
    fn parse_if_expression(&mut self) -> SeenResult<Expr<'static>> {
        let span = self.current_span();
        
        #[cfg(test)]
        eprintln!("parse_if_expression: ENTER, current token = {:?}", self.current_token());
        
        // Consume 'if' - can be either keyword or identifier
        if let Some(token) = self.current_token() {
            match &token.value {
                TokenType::KeywordIf => {
                    self.advance();
                }
                TokenType::Identifier(name) if name == "if" => {
                    self.advance();
                }
                _ => {
                    self.error("Expected 'if'");
                    return Err(seen_common::SeenError::parse_error("Expected 'if'"));
                }
            }
        } else {
            self.error("Expected 'if'");
            return Err(seen_common::SeenError::parse_error("Expected 'if'"));
        }
        
        #[cfg(test)]
        eprintln!("parse_if_expression: After consuming 'if', parsing condition, current = {:?}", self.current_token());
        
        let condition = Box::new(self.parse_expression()?);
        
        #[cfg(test)]
        eprintln!("parse_if_expression: After parsing condition, about to parse then block, current = {:?}", self.current_token());
        
        let then_branch = self.parse_block()?;
        
        #[cfg(test)]
        eprintln!("parse_if_expression: After then block, checking for else, current = {:?}", self.current_token());
        
        let else_branch = if self.match_token(&TokenType::KeywordElse) {
            #[cfg(test)]
            eprintln!("parse_if_expression: Found else, current after consuming = {:?}", self.current_token());
            
            // Check if it's a block or an if expression
            if let Some(token) = self.current_token() {
                if matches!(token.value, TokenType::LeftBrace) {
                    #[cfg(test)]
                    eprintln!("parse_if_expression: Else has block, parsing it");
                    
                    // else { block }
                    let block = self.parse_block()?;
                    Some(Box::new(Expr {
                        kind: Box::new(ExprKind::Block(block)),
                        span: self.current_span(),
                        id: self.next_node_id(),
                    }))
                } else {
                    #[cfg(test)]
                    eprintln!("parse_if_expression: Else has expression, parsing it");
                    
                    // else if ... or other expression
                    Some(Box::new(self.parse_expression()?))
                }
            } else {
                #[cfg(test)]
                eprintln!("parse_if_expression: Else has expression (no token), parsing it");
                
                // else if ... or other expression
                Some(Box::new(self.parse_expression()?))
            }
        } else {
            #[cfg(test)]
            eprintln!("parse_if_expression: No else branch found");
            
            None
        };
        
        #[cfg(test)]
        eprintln!("parse_if_expression: EXIT, current = {:?}", self.current_token());
        
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
    
    fn parse_await_expression(&mut self) -> SeenResult<Expr<'static>> {
        let span = self.current_span();
        
        // Consume 'await'
        self.expect_keyword(TokenType::KeywordAwait)?;
        
        // Parse the expression to await
        let expr = Box::new(self.parse_expression()?);
        
        Ok(Expr {
            kind: Box::new(ExprKind::Await { expr }),
            span,
            id: self.next_node_id(),
        })
    }

    fn parse_launch_expression(&mut self) -> SeenResult<Expr<'static>> {
        let span = self.current_span();
        
        // Consume 'launch'
        self.expect_keyword(TokenType::KeywordLaunch)?;
        
        // Parse the block expression
        let block = self.parse_block()?;
        
        Ok(Expr {
            kind: Box::new(ExprKind::Launch { block }),
            span,
            id: self.next_node_id(),
        })
    }

    fn parse_try_catch_expression(&mut self) -> SeenResult<Expr<'static>> {
        let span = self.current_span();
        self.advance(); // consume 'try'
        
        let try_block = self.parse_block()?;
        
        let mut catch_blocks = Vec::new();
        while self.match_token(&TokenType::KeywordCatch) {
            // Parse catch pattern (e: Exception)
            self.expect_token(TokenType::LeftParen)?;
            let exception_name = self.expect_identifier_value()?;
            let exception_type = if self.match_token(&TokenType::Colon) {
                Some(self.parse_type()?)
            } else {
                None
            };
            self.expect_token(TokenType::RightParen)?;
            
            let catch_block = self.parse_block()?;
            catch_blocks.push((exception_name.leak() as &str, exception_type, catch_block));
        }
        
        let finally_block = if self.match_token(&TokenType::KeywordFinally) {
            Some(Box::new(self.parse_block()?))
        } else {
            None
        };
        
        Ok(Expr {
            kind: Box::new(ExprKind::TryCatch {
                try_block: Box::new(try_block),
                catch_blocks,
                finally_block,
            }),
            span,
            id: self.next_node_id(),
        })
    }
    
    fn parse_flow_expression(&mut self) -> SeenResult<Expr<'static>> {
        let span = self.current_span();
        
        // Consume 'flow'
        self.expect_keyword(TokenType::KeywordFlow)?;
        
        // Parse the block expression
        let block = self.parse_block()?;
        
        Ok(Expr {
            kind: Box::new(ExprKind::FlowBuilder { block }),
            span,
            id: self.next_node_id(),
        })
    }

    fn is_whitespace_or_comment(&self, token: &Token) -> bool {
        // Check if token is whitespace or comment (which should have been filtered out already)
        // Since tokens are already filtered, we shouldn't have whitespace tokens
        // This is just a safety check
        false
    }
    
    fn is_kotlin_lambda(&self) -> bool {
        // Check if this is a Kotlin-style lambda: { param -> ... } or { param, param2 -> ... }
        // Look for pattern: { identifier [, identifier]* ->
        if self.current >= self.tokens.len() {
            return false;
        }
        
        // Must start with {
        if !matches!(self.tokens[self.current].value, TokenType::LeftBrace) {
            return false;
        }
        
        let mut pos = self.current + 1;
        
        // Skip whitespace
        while pos < self.tokens.len() && self.is_whitespace_or_comment(&self.tokens[pos]) {
            pos += 1;
        }
        
        // Must have identifier
        if pos >= self.tokens.len() || !matches!(self.tokens[pos].value, TokenType::Identifier(_)) {
            return false;
        }
        
        pos += 1;
        
        // Skip whitespace
        while pos < self.tokens.len() && self.is_whitespace_or_comment(&self.tokens[pos]) {
            pos += 1;
        }
        
        // Check for arrow (->) or comma for multiple params
        while pos < self.tokens.len() {
            match &self.tokens[pos].value {
                TokenType::Arrow => return true,  // Found the arrow, it's a lambda
                TokenType::Comma => {
                    // Multiple params, continue checking
                    pos += 1;
                    // Skip whitespace
                    while pos < self.tokens.len() && self.is_whitespace_or_comment(&self.tokens[pos]) {
                        pos += 1;
                    }
                    // Must have another identifier
                    if pos >= self.tokens.len() || !matches!(self.tokens[pos].value, TokenType::Identifier(_)) {
                        return false;
                    }
                    pos += 1;
                }
                _ => return false,
            }
            
            // Skip whitespace after identifier
            while pos < self.tokens.len() && self.is_whitespace_or_comment(&self.tokens[pos]) {
                pos += 1;
            }
        }
        
        false
    }
    
    fn parse_kotlin_lambda(&mut self) -> SeenResult<Expr<'static>> {
        let span = self.current_span();
        
        // Consume {
        self.expect_token(TokenType::LeftBrace)?;
        
        // Parse parameters
        let mut params = Vec::new();
        
        // Parse first parameter
        loop {
            let param_name = self.expect_identifier_value()?;
            let param_span = self.previous_span();
            
            params.push(ClosureParam {
                name: seen_common::Spanned::new(param_name.leak(), param_span),
                ty: None,  // Kotlin lambdas typically infer types
            });
            
            // Check for comma (more params) or arrow (end of params)
            if self.match_token(&TokenType::Comma) {
                continue;
            } else if self.match_token(&TokenType::Arrow) {
                break;
            } else {
                self.error("Expected ',' or '->' in lambda parameter list");
                return Err(seen_common::SeenError::parse_error("Expected ',' or '->' in lambda"));
            }
        }
        
        // Parse body - statements until }
        let mut statements = Vec::new();
        
        while !matches!(self.current_token(), Some(Token { value: TokenType::RightBrace, .. })) {
            if self.is_at_end() {
                self.error("Unexpected end of file in lambda body");
                return Err(seen_common::SeenError::parse_error("Unexpected EOF in lambda"));
            }
            
            statements.push(self.parse_statement()?);
        }
        
        // Consume }
        self.expect_token(TokenType::RightBrace)?;
        
        // Create closure body
        let body = if statements.len() == 1 {
            // Single expression lambda
            match &statements[0].kind {
                StmtKind::Expr(expr) => ClosureBody::Expression(Box::new(expr.clone())),
                _ => ClosureBody::Block(Block {
                    statements,
                    span: self.current_span(),
                }),
            }
        } else {
            // Multi-statement lambda
            ClosureBody::Block(Block {
                statements,
                span: self.current_span(),
            })
        };
        
        Ok(Expr {
            kind: Box::new(ExprKind::Closure(Closure {
                params,
                body,
                return_type: None,
            })),
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
            let guard = if self.match_token(&TokenType::KeywordIf) {
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
                TokenType::LeftParen => {
                    // Parse destructuring pattern (a, b, c)
                    self.advance(); // consume '('
                    let mut patterns = Vec::new();
                    
                    while !self.check(&TokenType::RightParen) && !self.is_at_end() {
                        patterns.push(self.parse_pattern()?);
                        
                        if !self.match_token(&TokenType::Comma) {
                            break;
                        }
                    }
                    
                    self.expect_token(TokenType::RightParen)?;
                    PatternKind::Destructuring(patterns)
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
            TokenType::Assign => (0, true), // Assignment has lowest precedence and is right-associative
            TokenType::LogicalOr => (1, false),
            TokenType::LogicalAnd => (2, false),
            TokenType::Equal | TokenType::NotEqual => (3, false),
            TokenType::Less | TokenType::LessEqual | TokenType::Greater | TokenType::GreaterEqual | TokenType::KeywordIs => (4, false),
            TokenType::Plus | TokenType::Minus => (5, false),
            TokenType::Multiply | TokenType::Divide | TokenType::Modulo => (6, false),
            _ => (255, false), // No precedence - don't treat as binary operator
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
            TokenType::KeywordIs => Some(BinaryOp::Is),
            _ => None,
        }
    }
    
    // Kotlin-inspired feature parsing methods
    
    fn parse_data_class(&mut self) -> SeenResult<Item<'static>> {
        let start_span = self.current_span();
        
        // Consume 'data'
        self.expect_keyword(TokenType::KeywordData)?;
        
        // Consume 'class'
        self.expect_keyword(TokenType::KeywordClass)?;
        
        // Parse class name
        let name = self.expect_identifier_value()?;
        let name_span = self.previous_span();
        
        // Parse constructor parameters (which become properties)
        self.expect_token(TokenType::LeftParen)?;
        let mut fields = Vec::new();
        
        if !self.check(&TokenType::RightParen) {
            loop {
                // Parse val/var
                let is_mutable = if self.match_token(&TokenType::KeywordVar) {
                    true
                } else if self.match_token(&TokenType::KeywordVal) {
                    false // val is immutable
                } else {
                    return Err(seen_common::SeenError::parse_error("Expected 'val' or 'var' in data class constructor"));
                };
                
                let field_name = self.expect_identifier_value()?;
                let field_span = self.previous_span();
                
                self.expect_token(TokenType::Colon)?;
                let field_type = self.parse_type()?;
                
                // Optional default value
                let default_value = if self.match_token(&TokenType::Assign) {
                    Some(self.parse_expression()?)
                } else {
                    None
                };
                
                fields.push(DataClassField {
                    name: seen_common::Spanned::new(field_name.leak(), field_span),
                    ty: field_type,
                    is_mutable,
                    default_value,
                    delegate: None, // For now, we don't support delegation
                    visibility: Visibility::Public,
                    span: field_span,
                });
                
                if !self.match_token(&TokenType::Comma) {
                    break;
                }
            }
        }
        
        self.expect_token(TokenType::RightParen)?;
        
        // Optional class body
        let _body = if self.check(&TokenType::LeftBrace) {
            self.parse_block()?
        } else {
            Block {
                statements: Vec::new(),
                span: self.current_span(),
            }
        };
        
        let data_class = DataClass {
            name: seen_common::Spanned::new(name.leak(), name_span),
            fields,
            visibility: Visibility::Public,
            generic_params: Vec::new(),
            attributes: Vec::new(),
        };
        
        Ok(Item {
            kind: ItemKind::DataClass(data_class),
            span: start_span,
            id: self.next_node_id(),
        })
    }
    
    fn parse_sealed_class(&mut self) -> SeenResult<Item<'static>> {
        let start_span = self.current_span();
        
        // Consume 'sealed'
        self.expect_keyword(TokenType::KeywordSealed)?;
        
        // Consume 'class' or 'interface'
        let is_interface = if self.match_token(&TokenType::KeywordInterface) {
            true
        } else {
            self.expect_keyword(TokenType::KeywordClass)?;
            false
        };
        
        // Parse class name
        let name = self.expect_identifier_value()?;
        let name_span = self.previous_span();
        
        // Parse class body with variants
        self.expect_token(TokenType::LeftBrace)?;
        let mut variants = Vec::new();
        
        while !self.check(&TokenType::RightBrace) {
            // Parse variant (could be data class, object, or regular class)
            if self.match_token(&TokenType::KeywordData) {
                // Data class variant
                self.expect_keyword(TokenType::KeywordClass)?;
                let variant_name = self.expect_identifier_value()?;
                let variant_span = self.previous_span();
                
                // Parse constructor
                self.expect_token(TokenType::LeftParen)?;
                let mut fields = Vec::new();
                
                if !self.check(&TokenType::RightParen) {
                    loop {
                        let is_mutable = self.match_token(&TokenType::KeywordVar) || !self.match_token(&TokenType::KeywordVal);
                        let field_name = self.expect_identifier_value()?;
                        self.expect_token(TokenType::Colon)?;
                        let field_type = self.parse_type()?;
                        
                        fields.push(DataClassField {
                            name: seen_common::Spanned::new(field_name.leak(), variant_span),
                            ty: field_type,
                            is_mutable,
                            default_value: None,
                            delegate: None, // For now, we don't support delegation
                            visibility: Visibility::Public,
                            span: variant_span,
                        });
                        
                        if !self.match_token(&TokenType::Comma) {
                            break;
                        }
                    }
                }
                
                self.expect_token(TokenType::RightParen)?;
                
                // Skip inheritance part for now
                if self.match_token(&TokenType::Colon) {
                    // Skip the parent class reference
                    self.parse_type()?;
                    self.expect_token(TokenType::LeftParen)?;
                    self.expect_token(TokenType::RightParen)?;
                }
                
                variants.push(SealedClassVariant {
                    name: seen_common::Spanned::new(variant_name.leak(), variant_span),
                    fields,
                    span: variant_span,
                });
            } else if self.match_token(&TokenType::KeywordObject) {
                // Object variant
                let variant_name = self.expect_identifier_value()?;
                let variant_span = self.previous_span();
                
                // Skip inheritance
                if self.match_token(&TokenType::Colon) {
                    self.parse_type()?;
                    self.expect_token(TokenType::LeftParen)?;
                    self.expect_token(TokenType::RightParen)?;
                }
                
                variants.push(SealedClassVariant {
                    name: seen_common::Spanned::new(variant_name.leak(), variant_span),
                    fields: Vec::new(),
                    span: variant_span,
                });
            } else {
                // Skip unknown items
                self.advance();
            }
        }
        
        self.expect_token(TokenType::RightBrace)?;
        
        let sealed_class = SealedClass {
            name: seen_common::Spanned::new(name.leak(), name_span),
            variants,
            visibility: Visibility::Public,
            generic_params: Vec::new(),
            attributes: Vec::new(),
        };
        
        Ok(Item {
            kind: ItemKind::SealedClass(sealed_class),
            span: start_span,
            id: self.next_node_id(),
        })
    }
    
    fn parse_object_declaration(&mut self) -> SeenResult<Item<'static>> {
        let start_span = self.current_span();
        
        // Consume 'object'
        self.expect_keyword(TokenType::KeywordObject)?;
        
        // Parse object name
        let name = self.expect_identifier_value()?;
        let name_span = self.previous_span();
        
        // Optional inheritance
        let parent_type = if self.match_token(&TokenType::Colon) {
            Some(self.parse_type()?)
        } else {
            None
        };
        
        // Parse object body
        let body = if self.check(&TokenType::LeftBrace) {
            self.parse_block()?
        } else {
            // Empty object
            Block {
                statements: Vec::new(),
                span: self.current_span(),
            }
        };
        
        // Objects are singleton structs with no fields
        let object_struct = Struct {
            name: seen_common::Spanned::new(name.leak(), name_span),
            fields: Vec::new(),
            visibility: Visibility::Public,
            generic_params: Vec::new(),
            attributes: vec![
                Attribute {
                    name: seen_common::Spanned::new("singleton", start_span),
                    args: Vec::new(),
                    span: start_span,
                }
            ],
            companion_object: None,
        };
        
        Ok(Item {
            kind: ItemKind::Struct(object_struct),
            span: start_span,
            id: self.next_node_id(),
        })
    }
    
    fn parse_trait(&mut self) -> SeenResult<Item<'static>> {
        let start_span = self.current_span();
        self.advance(); // consume 'trait'
        
        // Parse trait name
        let name_value = self.expect_identifier_value()?;
        let name_span = self.previous_span();
        let name_static: &'static str = Box::leak(name_value.into_boxed_str());
        let name = Spanned::new(name_static, name_span);
        
        // Parse optional generic parameters
        let generic_params = if self.check(&TokenType::Less) {
            self.parse_generic_params()?
        } else {
            Vec::new()
        };
        
        // Parse optional supertraits (: Trait1 + Trait2)
        let mut supertraits = Vec::new();
        if self.match_token(&TokenType::Colon) {
            loop {
                supertraits.push(self.parse_type()?);
                if !self.match_token(&TokenType::Plus) {
                    break;
                }
            }
        }
        
        // Parse trait body
        self.expect_token(TokenType::LeftBrace)?;
        let mut items = Vec::new();
        
        while !self.check(&TokenType::RightBrace) && !self.is_at_end() {
            items.push(self.parse_trait_item()?);
        }
        
        self.expect_token(TokenType::RightBrace)?;
        
        let trait_def = TraitDef {
            name,
            items,
            generic_params,
            supertraits,
            visibility: Visibility::Public,
        };
        
        Ok(Item {
            kind: ItemKind::Trait(trait_def),
            span: start_span,
            id: self.next_node_id(),
        })
    }
    
    fn parse_trait_item(&mut self) -> SeenResult<TraitItem<'static>> {
        let start_span = self.current_span();
        
        // For now, only support function declarations in traits
        if self.check(&TokenType::KeywordFun) {
            self.advance(); // consume 'fun'
            
            let name_value = self.expect_identifier_value()?;
            let name_span = self.previous_span();
            let name_static: &'static str = Box::leak(name_value.into_boxed_str());
            let name = Spanned::new(name_static, name_span);
            
            // Parse function signature (no body in trait)
            self.expect_token(TokenType::LeftParen)?;
            let params = self.parse_parameter_list()?;
            self.expect_token(TokenType::RightParen)?;
            
            // Parse optional return type
            let return_type = if self.match_token(&TokenType::Arrow) {
                Some(self.parse_type()?)
            } else {
                None
            };
            
            // Expect semicolon (no body)
            self.expect_token(TokenType::Semicolon)?;
            
            let trait_func = TraitFunction {
                name,
                params,
                return_type,
                default_body: None,
                generic_params: Vec::new(),
            };
            
            Ok(TraitItem {
                kind: TraitItemKind::Function(trait_func),
                span: start_span,
                id: self.next_node_id(),
            })
        } else {
            self.error("Expected 'fun' in trait");
            Err(seen_common::SeenError::parse_error("Expected trait item"))
        }
    }
    
    fn parse_impl(&mut self) -> SeenResult<Item<'static>> {
        let start_span = self.current_span();
        self.advance(); // consume 'impl'
        
        // Parse trait name (what we're implementing)
        let trait_ref = self.parse_type()?;
        
        // Expect 'for'
        self.expect_token(TokenType::KeywordFor)?;
        
        // Parse target type (what we're implementing for)
        let target_type = self.parse_type()?;
        
        // Parse impl body
        self.expect_token(TokenType::LeftBrace)?;
        let mut items = Vec::new();
        
        while !self.check(&TokenType::RightBrace) && !self.is_at_end() {
            // Parse function implementations
            items.push(self.parse_impl_item()?);
        }
        
        self.expect_token(TokenType::RightBrace)?;
        
        let impl_block = Impl {
            trait_ref: Some(trait_ref),
            self_type: target_type,
            generic_params: Vec::new(),
            items,
        };
        
        Ok(Item {
            kind: ItemKind::Impl(impl_block),
            span: start_span,
            id: self.next_node_id(),
        })
    }
    
    fn parse_impl_item(&mut self) -> SeenResult<ImplItem<'static>> {
        let start_span = self.current_span();
        
        // Parse a regular function but within an impl block
        if !self.check(&TokenType::KeywordFun) {
            self.error("Expected 'fun' in impl block");
            return Err(seen_common::SeenError::parse_error("Expected function in impl"));
        }
        
        // Use existing function parsing logic
        if let Ok(item) = self.parse_function() {
            if let ItemKind::Function(func) = item.kind {
                Ok(ImplItem {
                    kind: ImplItemKind::Function(func),
                    span: start_span,
                    id: self.next_node_id(),
                })
            } else {
                Err(seen_common::SeenError::parse_error("Expected function"))
            }
        } else {
            Err(seen_common::SeenError::parse_error("Failed to parse impl function"))
        }
    }

    fn parse_interface(&mut self) -> SeenResult<Item<'static>> {
        let start_span = self.current_span();
        
        // Consume 'interface'
        self.expect_keyword(TokenType::KeywordInterface)?;
        
        // Parse interface name
        let name = self.expect_identifier_value()?;
        let name_span = self.previous_span();
        
        // Parse optional generic parameters
        let generic_params = if self.check(&TokenType::Less) {
            self.parse_generic_params()?
        } else {
            Vec::new()
        };
        
        // Parse optional superinterfaces
        let supertraits = if self.match_token(&TokenType::Colon) {
            let mut supers = Vec::new();
            loop {
                supers.push(self.parse_type()?);
                if !self.match_token(&TokenType::Comma) {
                    break;
                }
            }
            supers
        } else {
            Vec::new()
        };
        
        // Parse interface body
        self.expect_token(TokenType::LeftBrace)?;
        let mut items = Vec::new();
        
        while !self.check(&TokenType::RightBrace) {
            if self.match_token(&TokenType::KeywordFun) {
                // Parse method signature
                let method_name = self.expect_identifier_value()?;
                let method_span = self.previous_span();
                
                // Parse parameters
                self.expect_token(TokenType::LeftParen)?;
                let params = self.parse_parameter_list()?;
                self.expect_token(TokenType::RightParen)?;
                
                // Parse return type
                let return_type = if self.match_token(&TokenType::Colon) {
                    Some(self.parse_type()?)
                } else {
                    None
                };
                
                // Optional default implementation
                let default_body = if self.check(&TokenType::LeftBrace) {
                    Some(self.parse_block()?)
                } else {
                    None
                };
                
                let trait_func = TraitFunction {
                    name: seen_common::Spanned::new(method_name.leak(), method_span),
                    params,
                    return_type,
                    default_body,
                    generic_params: Vec::new(),
                };
                
                items.push(TraitItem {
                    kind: TraitItemKind::Function(trait_func),
                    span: method_span,
                    id: self.next_node_id(),
                });
            } else if self.match_token(&TokenType::KeywordVal) || self.match_token(&TokenType::KeywordVar) {
                // Parse property
                let prop_name = self.expect_identifier_value()?;
                let prop_span = self.previous_span();
                
                self.expect_token(TokenType::Colon)?;
                let prop_type = self.parse_type()?;
                
                let trait_const = TraitConst {
                    name: seen_common::Spanned::new(prop_name.leak(), prop_span),
                    ty: prop_type,
                    default_value: None,
                };
                
                items.push(TraitItem {
                    kind: TraitItemKind::Const(trait_const),
                    span: prop_span,
                    id: self.next_node_id(),
                });
            } else {
                // Skip unexpected tokens
                self.advance();
            }
        }
        
        self.expect_token(TokenType::RightBrace)?;
        
        let interface = TraitDef {
            name: seen_common::Spanned::new(name.leak(), name_span),
            items,
            generic_params,
            supertraits,
            visibility: Visibility::Public,
        };
        
        Ok(Item {
            kind: ItemKind::Trait(interface),
            span: start_span,
            id: self.next_node_id(),
        })
    }
    
    fn parse_extension_function(&mut self) -> SeenResult<Item<'static>> {
        let start_span = self.current_span();
        
        #[cfg(test)]
        eprintln!("parse_extension_function: Starting");
        
        // Consume 'extension'
        self.expect_identifier("extension")?;
        
        #[cfg(test)]
        eprintln!("parse_extension_function: Consumed 'extension', current token = {:?}", self.current_token());
        
        // Expect 'fun'
        self.expect_keyword(TokenType::KeywordFun)?;
        
        #[cfg(test)]
        eprintln!("parse_extension_function: Consumed 'fun', current token = {:?}", self.current_token());
        
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
            type_params: Vec::new(), // Extension functions don't have generics yet
            params,
            return_type,
            body,
            visibility: Visibility::Public,
            attributes: vec![],
            is_inline: false,
            is_suspend: false,
            is_operator: false,
            is_infix: false,
            is_tailrec: false,
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