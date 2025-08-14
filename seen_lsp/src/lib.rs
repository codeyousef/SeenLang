//! Language Server Protocol implementation for Seen Language
//! 
//! Provides IDE features like auto-completion, diagnostics, hover info,
//! go-to-definition, and more for the Seen programming language.

use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use seen_lexer::{Lexer, KeywordManager};
use seen_parser::Parser;
use seen_parser::ast::Program;
use seen_typechecker::{TypeChecker, TypeCheckResult};
use seen_memory_manager::MemoryManager;
use seen_parser::ast::Expression;
use seen_lexer::position::Position as SeenPosition;

/// Symbol information for definitions and references
#[derive(Debug, Clone)]
struct SymbolInfo {
    /// Symbol name
    name: String,
    /// Symbol kind (function, variable, struct, etc.)
    kind: SymbolKind,
    /// Definition location
    definition: Location,
    /// All references to this symbol
    references: Vec<Location>,
}

/// Document information stored by the LSP server
#[derive(Debug, Clone)]
struct DocumentInfo {
    /// Document content
    content: String,
    /// Document version
    version: i32,
    /// Parsed AST
    ast: Option<Program>,
    /// Type checking results
    type_info: Option<TypeCheckResult>,
    /// Diagnostic results
    diagnostics: Vec<Diagnostic>,
    /// Symbol table for this document
    symbols: HashMap<String, SymbolInfo>,
}

/// Main LSP backend for Seen Language
pub struct SeenLanguageServer {
    /// LSP client handle
    client: Client,
    /// Documents currently open in the editor
    documents: Arc<RwLock<HashMap<Url, DocumentInfo>>>,
    /// Language configuration (keywords for different languages)
    language_config: Arc<RwLock<String>>,
    /// Global symbol index across all documents
    global_symbols: Arc<RwLock<HashMap<String, Vec<SymbolInfo>>>>,
}

impl SeenLanguageServer {
    /// Create a new Seen Language Server
    pub fn new(client: Client) -> Self {
        Self {
            client,
            documents: Arc::new(RwLock::new(HashMap::new())),
            language_config: Arc::new(RwLock::new("en".to_string())),
            global_symbols: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Analyze a document and return diagnostics
    async fn analyze_document(&self, uri: &Url, content: &str) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        
        // Load keyword manager for the current language
        let language = self.language_config.read().await.clone();
        let keyword_manager = Arc::new(KeywordManager::new());
        
        // Lexical analysis
        let mut lexer = Lexer::new(content.to_string(), keyword_manager);

        // Parse the tokens
        let mut parser = Parser::new(lexer);
        let program = match parser.parse_program() {
            Ok(program) => program,
            Err(e) => {
                // Extract position from error variant
                let pos = match &e {
                    seen_parser::error::ParseError::UnexpectedToken { pos, .. } |
                    seen_parser::error::ParseError::UnexpectedEof { pos } |
                    seen_parser::error::ParseError::InvalidExpression { pos } |
                    seen_parser::error::ParseError::InvalidPattern { pos, .. } |
                    seen_parser::error::ParseError::MissingClosingDelimiter { pos, .. } |
                    seen_parser::error::ParseError::InvalidNumber { pos, .. } |
                    seen_parser::error::ParseError::InvalidString { pos, .. } => pos,
                    seen_parser::error::ParseError::LexerError { .. } => &seen_lexer::Position { line: 1, column: 1, offset: 0 },
                };
                
                diagnostics.push(Diagnostic {
                    range: Range {
                        start: Position { 
                            line: pos.line.saturating_sub(1) as u32, 
                            character: pos.column.saturating_sub(1) as u32 
                        },
                        end: Position { 
                            line: pos.line.saturating_sub(1) as u32, 
                            character: pos.column as u32 
                        },
                    },
                    severity: Some(DiagnosticSeverity::ERROR),
                    message: format!("Parse error: {}", e),
                    ..Default::default()
                });
                return diagnostics;
            }
        };

        // Type check the program
        let mut type_checker = TypeChecker::new();
        let type_result = type_checker.check_program(&program);
        
        // Convert type errors to diagnostics
        for error in type_result.get_errors() {
            let pos = error.position();
            diagnostics.push(Diagnostic {
                range: Range {
                    start: Position { 
                        line: pos.line.saturating_sub(1) as u32, 
                        character: pos.column.saturating_sub(1) as u32 
                    },
                    end: Position { 
                        line: pos.line.saturating_sub(1) as u32, 
                        character: pos.column as u32 
                    },
                },
                severity: Some(DiagnosticSeverity::ERROR),
                message: format!("Type error: {}", error),
                ..Default::default()
            });
        }

        // Memory safety analysis
        let mut memory_manager = MemoryManager::new();
        let memory_result = memory_manager.analyze_program(&program);
        
        // Convert memory errors to diagnostics
        for error in memory_result.get_errors() {
            // Memory errors don't have positions yet, use default position
            diagnostics.push(Diagnostic {
                range: Range {
                    start: Position { 
                        line: 0, 
                        character: 0 
                    },
                    end: Position { 
                        line: 0, 
                        character: 0 
                    },
                },
                severity: Some(DiagnosticSeverity::WARNING),
                message: format!("Memory safety: {}", error),
                ..Default::default()
            });
        }

        diagnostics
    }

    /// Get completion items at a position
    async fn get_completions(&self, _uri: &Url, _position: Position) -> Vec<CompletionItem> {
        let mut items = Vec::new();
        
        // Add keyword completions
        let keywords = vec![
            ("fun", "Function declaration", CompletionItemKind::KEYWORD),
            ("let", "Immutable variable declaration", CompletionItemKind::KEYWORD),
            ("var", "Mutable variable declaration", CompletionItemKind::KEYWORD),
            ("if", "Conditional expression", CompletionItemKind::KEYWORD),
            ("else", "Alternative branch", CompletionItemKind::KEYWORD),
            ("match", "Pattern matching", CompletionItemKind::KEYWORD),
            ("while", "While loop", CompletionItemKind::KEYWORD),
            ("for", "For loop", CompletionItemKind::KEYWORD),
            ("in", "Iterator keyword", CompletionItemKind::KEYWORD),
            ("return", "Return from function", CompletionItemKind::KEYWORD),
            ("async", "Asynchronous function", CompletionItemKind::KEYWORD),
            ("await", "Await async result", CompletionItemKind::KEYWORD),
            ("struct", "Structure definition", CompletionItemKind::KEYWORD),
            ("enum", "Enumeration definition", CompletionItemKind::KEYWORD),
            ("interface", "Interface definition", CompletionItemKind::KEYWORD),
            ("impl", "Implementation block", CompletionItemKind::KEYWORD),
            ("and", "Logical AND operator", CompletionItemKind::KEYWORD),
            ("or", "Logical OR operator", CompletionItemKind::KEYWORD),
            ("not", "Logical NOT operator", CompletionItemKind::KEYWORD),
        ];

        for (keyword, doc, kind) in keywords {
            items.push(CompletionItem {
                label: keyword.to_string(),
                kind: Some(kind),
                detail: Some(doc.to_string()),
                documentation: Some(Documentation::String(doc.to_string())),
                ..Default::default()
            });
        }

        // Add built-in type completions
        let types = vec![
            ("Int", "32-bit signed integer"),
            ("UInt", "32-bit unsigned integer"),
            ("Float", "64-bit floating point"),
            ("Bool", "Boolean value"),
            ("String", "UTF-8 string"),
            ("Char", "Unicode character"),
            ("Unit", "Unit type ()"),
        ];

        for (type_name, doc) in types {
            items.push(CompletionItem {
                label: type_name.to_string(),
                kind: Some(CompletionItemKind::CLASS),
                detail: Some(doc.to_string()),
                documentation: Some(Documentation::String(doc.to_string())),
                ..Default::default()
            });
        }

        // Add snippets
        items.push(CompletionItem {
            label: "fun".to_string(),
            kind: Some(CompletionItemKind::SNIPPET),
            detail: Some("Function definition".to_string()),
            insert_text: Some("fun ${1:name}(${2:params}): ${3:ReturnType} {\n    $0\n}".to_string()),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            ..Default::default()
        });

        items.push(CompletionItem {
            label: "if".to_string(),
            kind: Some(CompletionItemKind::SNIPPET),
            detail: Some("If expression".to_string()),
            insert_text: Some("if ${1:condition} {\n    $2\n} else {\n    $0\n}".to_string()),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            ..Default::default()
        });

        items.push(CompletionItem {
            label: "match".to_string(),
            kind: Some(CompletionItemKind::SNIPPET),
            detail: Some("Pattern matching".to_string()),
            insert_text: Some("match ${1:value} {\n    ${2:pattern} -> ${3:result}\n    _ -> $0\n}".to_string()),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            ..Default::default()
        });

        items.push(CompletionItem {
            label: "type".to_string(),
            kind: Some(CompletionItemKind::SNIPPET),
            detail: Some("Type alias".to_string()),
            insert_text: Some("type ${1:Name} = ${2:Type}".to_string()),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            ..Default::default()
        });

        items.push(CompletionItem {
            label: "extension".to_string(),
            kind: Some(CompletionItemKind::SNIPPET),
            detail: Some("Extension methods".to_string()),
            insert_text: Some("extension ${1:Type} {\n    fun ${2:method}(${3:params}): ${4:ReturnType} {\n        $0\n    }\n}".to_string()),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            ..Default::default()
        });

        items.push(CompletionItem {
            label: "sealed class".to_string(),
            kind: Some(CompletionItemKind::SNIPPET),
            detail: Some("Sealed class".to_string()),
            insert_text: Some("sealed class ${1:Name} {\n    $0\n}".to_string()),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            ..Default::default()
        });

        items.push(CompletionItem {
            label: "pure fun".to_string(),
            kind: Some(CompletionItemKind::SNIPPET),
            detail: Some("Pure function".to_string()),
            insert_text: Some("pure fun ${1:name}(${2:params}): ${3:ReturnType} = ${4:expression}".to_string()),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            ..Default::default()
        });

        items.push(CompletionItem {
            label: "when".to_string(),
            kind: Some(CompletionItemKind::SNIPPET),
            detail: Some("When expression".to_string()),
            insert_text: Some("when {\n    ${1:condition} -> ${2:result}\n    else -> $0\n}".to_string()),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            ..Default::default()
        });

        items
    }

    /// Extract symbols from the AST
    fn extract_symbols(&self, program: &Program, uri: &Url) -> HashMap<String, SymbolInfo> {
        let mut symbols = HashMap::new();
        
        for expr in &program.expressions {
            self.extract_symbols_from_expression(expr, uri, &mut symbols);
        }
        
        symbols
    }
    
    /// Extract symbols from an expression
    fn extract_symbols_from_expression(
        &self,
        expr: &Expression,
        uri: &Url,
        symbols: &mut HashMap<String, SymbolInfo>,
    ) {
        match expr {
            // Function definitions
            Expression::Function { name, pos, .. } => {
                let location = Location {
                    uri: uri.clone(),
                    range: self.position_to_range(pos),
                };
                symbols.insert(
                    name.clone(),
                    SymbolInfo {
                        name: name.clone(),
                        kind: SymbolKind::FUNCTION,
                        definition: location,
                        references: Vec::new(),
                    },
                );
            }
            // Variable bindings
            Expression::Let { name, value, pos, .. } => {
                let location = Location {
                    uri: uri.clone(),
                    range: self.position_to_range(pos),
                };
                symbols.insert(
                    name.clone(),
                    SymbolInfo {
                        name: name.clone(),
                        kind: SymbolKind::VARIABLE,
                        definition: location,
                        references: Vec::new(),
                    },
                );
                // Also check the value expression
                self.extract_symbols_from_expression(value, uri, symbols);
            }
            // Struct definitions
            Expression::StructDefinition { name, pos, .. } => {
                let location = Location {
                    uri: uri.clone(),
                    range: self.position_to_range(pos),
                };
                symbols.insert(
                    name.clone(),
                    SymbolInfo {
                        name: name.clone(),
                        kind: SymbolKind::STRUCT,
                        definition: location,
                        references: Vec::new(),
                    },
                );
            }
            // Class definitions
            Expression::ClassDefinition { name, pos, .. } => {
                let location = Location {
                    uri: uri.clone(),
                    range: self.position_to_range(pos),
                };
                symbols.insert(
                    name.clone(),
                    SymbolInfo {
                        name: name.clone(),
                        kind: SymbolKind::CLASS,
                        definition: location,
                        references: Vec::new(),
                    },
                );
            }
            // Type aliases
            Expression::TypeAlias { name, pos, .. } => {
                let location = Location {
                    uri: uri.clone(),
                    range: self.position_to_range(pos),
                };
                symbols.insert(
                    name.clone(),
                    SymbolInfo {
                        name: name.clone(),
                        kind: SymbolKind::TYPE_PARAMETER,
                        definition: location,
                        references: Vec::new(),
                    },
                );
            }
            // Extension methods
            Expression::Extension { target_type, methods, pos, .. } => {
                for method in methods {
                    let location = Location {
                        uri: uri.clone(),
                        range: self.position_to_range(pos),
                    };
                    symbols.insert(
                        method.name.clone(),
                        SymbolInfo {
                            name: format!("{}::{}", target_type.name, method.name),
                            kind: SymbolKind::METHOD,
                            definition: location,
                            references: Vec::new(),
                        },
                    );
                }
            }
            // Recursively check other expressions
            Expression::Block { expressions, .. } => {
                for expr in expressions {
                    self.extract_symbols_from_expression(expr, uri, symbols);
                }
            }
            Expression::If { then_branch, else_branch, .. } => {
                self.extract_symbols_from_expression(then_branch, uri, symbols);
                if let Some(else_expr) = else_branch {
                    self.extract_symbols_from_expression(else_expr, uri, symbols);
                }
            }
            Expression::While { body, .. } | Expression::Loop { body, .. } => {
                self.extract_symbols_from_expression(body, uri, symbols);
            }
            Expression::For { body, .. } => {
                self.extract_symbols_from_expression(body, uri, symbols);
            }
            _ => {}
        }
    }
    
    /// Convert Seen position to LSP range
    fn position_to_range(&self, pos: &SeenPosition) -> Range {
        Range {
            start: Position {
                line: pos.line.saturating_sub(1) as u32,
                character: pos.column.saturating_sub(1) as u32,
            },
            end: Position {
                line: pos.line.saturating_sub(1) as u32,
                character: pos.column as u32,
            },
        }
    }
    
    /// Find the symbol at a given position
    async fn find_symbol_at_position(
        &self,
        uri: &Url,
        position: Position,
    ) -> Option<String> {
        let documents = self.documents.read().await;
        if let Some(doc) = documents.get(uri) {
            // Parse the line to find the identifier at the position
            let lines: Vec<&str> = doc.content.lines().collect();
            if let Some(line) = lines.get(position.line as usize) {
                // Simple heuristic: find word at position
                let char_pos = position.character as usize;
                let chars: Vec<char> = line.chars().collect();
                
                // Find word boundaries
                let mut start = char_pos;
                while start > 0 && chars.get(start - 1).map_or(false, |c| c.is_alphanumeric() || *c == '_') {
                    start -= 1;
                }
                
                let mut end = char_pos;
                while end < chars.len() && chars.get(end).map_or(false, |c| c.is_alphanumeric() || *c == '_') {
                    end += 1;
                }
                
                if start < end {
                    let word: String = chars[start..end].iter().collect();
                    return Some(word);
                }
            }
        }
        None
    }

    /// Count references to a symbol in content
    fn count_references(&self, content: &str, symbol_name: &str) -> u32 {
        // Simple reference counting - in a real implementation this would be more sophisticated
        let mut count = 0;
        let mut current_pos = 0;
        
        while let Some(pos) = content[current_pos..].find(symbol_name) {
            let actual_pos = current_pos + pos;
            
            // Check if this is a whole word (not part of another identifier)
            let is_word_boundary_before = actual_pos == 0 || 
                !content.chars().nth(actual_pos - 1).unwrap_or(' ').is_alphanumeric();
            let end_pos = actual_pos + symbol_name.len();
            let is_word_boundary_after = end_pos >= content.len() || 
                !content.chars().nth(end_pos).unwrap_or(' ').is_alphanumeric();
            
            if is_word_boundary_before && is_word_boundary_after {
                count += 1;
            }
            
            current_pos = actual_pos + 1;
        }
        
        count
    }

    /// Format a parsed program back to well-formatted source code
    fn format_program(&self, program: &Program) -> String {
        let mut output = String::new();
        
        for (i, expression) in program.expressions.iter().enumerate() {
            if i > 0 {
                output.push('\n');
            }
            self.format_expression(expression, &mut output, 0);
        }
        
        output
    }

    /// Format an expression with proper indentation
    fn format_expression(&self, expr: &Expression, output: &mut String, indent_level: usize) {
        match expr {
            Expression::Function { name, params, return_type, body, is_async, receiver, .. } => {
                self.add_indent(output, indent_level);
                
                if *is_async {
                    output.push_str("async ");
                }
                
                output.push_str("fun ");
                
                if let Some(recv) = receiver {
                    output.push('(');
                    output.push_str(&recv.name);
                    output.push_str(": ");
                    if recv.is_mutable {
                        output.push_str("inout ");
                    }
                    output.push_str(&recv.type_name);
                    output.push_str(") ");
                }
                
                output.push_str(name);
                output.push('(');
                
                for (i, param) in params.iter().enumerate() {
                    if i > 0 { output.push_str(", "); }
                    output.push_str(&param.name);
                    if let Some(type_ann) = &param.type_annotation {
                        output.push_str(": ");
                        output.push_str(&type_ann.name);
                    }
                    if let Some(default) = &param.default_value {
                        output.push_str(" = ");
                        self.format_expression(default, output, 0);
                    }
                }
                
                output.push(')');
                
                if let Some(ret_type) = return_type {
                    output.push_str(": ");
                    output.push_str(&ret_type.name);
                }
                
                output.push_str(" {\n");
                self.format_expression(body, output, indent_level + 1);
                output.push('\n');
                self.add_indent(output, indent_level);
                output.push('}');
            }
            
            Expression::Let { name, type_annotation, value, is_mutable, .. } => {
                self.add_indent(output, indent_level);
                output.push_str("let ");
                if *is_mutable {
                    output.push_str("mut ");
                }
                output.push_str(name);
                if let Some(type_ann) = type_annotation {
                    output.push_str(": ");
                    output.push_str(&type_ann.name);
                }
                output.push_str(" = ");
                self.format_expression(value, output, 0);
            }
            
            Expression::StringLiteral { value, .. } => {
                output.push('"');
                output.push_str(value);
                output.push('"');
            }
            
            Expression::IntegerLiteral { value, .. } => {
                output.push_str(&value.to_string());
            }
            
            Expression::BooleanLiteral { value, .. } => {
                output.push_str(if *value { "true" } else { "false" });
            }
            
            Expression::Identifier { name, .. } => {
                output.push_str(name);
            }
            
            Expression::Block { expressions, .. } => {
                for (i, expr) in expressions.iter().enumerate() {
                    if i > 0 { output.push('\n'); }
                    self.format_expression(expr, output, indent_level);
                }
            }
            
            // Add more expression types as needed
            _ => {
                output.push_str("/* unformatted expression */");
            }
        }
    }

    /// Add indentation to output
    fn add_indent(&self, output: &mut String, indent_level: usize) {
        for _ in 0..indent_level {
            output.push_str("    "); // 4 spaces per indent level
        }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for SeenLanguageServer {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: Some(vec![
                        ".".to_string(),
                        "?".to_string(),
                        ":".to_string(),
                        "(".to_string(),
                        "{".to_string(),
                        "[".to_string(),
                    ]),
                    ..Default::default()
                }),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                document_highlight_provider: Some(OneOf::Left(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                workspace_symbol_provider: Some(OneOf::Left(true)),
                code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
                code_lens_provider: Some(CodeLensOptions {
                    resolve_provider: Some(false),
                }),
                document_formatting_provider: Some(OneOf::Left(true)),
                document_range_formatting_provider: Some(OneOf::Left(true)),
                rename_provider: Some(OneOf::Left(true)),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "Seen Language Server initialized!")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let content = params.text_document.text;
        let version = params.text_document.version;

        // Analyze the document
        let diagnostics = self.analyze_document(&uri, &content).await;

        // Parse and extract symbols
        let mut ast = None;
        let mut symbols = HashMap::new();
        
        // Try to parse for symbol extraction
        let keyword_manager = Arc::new(KeywordManager::new());
        let mut lexer = Lexer::new(content.clone(), keyword_manager);
        let mut parser = Parser::new(lexer);
        if let Ok(program) = parser.parse_program() {
            symbols = self.extract_symbols(&program, &uri);
            ast = Some(program);
        }
        
        // Store document info
        let mut documents = self.documents.write().await;
        documents.insert(
            uri.clone(),
            DocumentInfo {
                content,
                version,
                ast,
                type_info: None,
                diagnostics: diagnostics.clone(),
                symbols,
            },
        );

        // Publish diagnostics
        self.client
            .publish_diagnostics(uri, diagnostics, Some(version))
            .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let version = params.text_document.version;
        
        if let Some(change) = params.content_changes.first() {
            let content = &change.text;
            
            // Analyze the document
            let diagnostics = self.analyze_document(&uri, content).await;

            // Parse and extract symbols
            let mut ast = None;
            let mut symbols = HashMap::new();
            
            // Try to parse for symbol extraction
            let keyword_manager = Arc::new(KeywordManager::new());
            let mut lexer = Lexer::new(content.clone(), keyword_manager);
            let mut parser = Parser::new(lexer);
            if let Ok(program) = parser.parse_program() {
                symbols = self.extract_symbols(&program, &uri);
                ast = Some(program);
            }
            
            // Update document info
            let mut documents = self.documents.write().await;
            if let Some(doc) = documents.get_mut(&uri) {
                doc.content = content.clone();
                doc.version = version;
                doc.diagnostics = diagnostics.clone();
                doc.ast = ast;
                doc.symbols = symbols;
            }

            // Publish diagnostics
            self.client
                .publish_diagnostics(uri, diagnostics, Some(version))
                .await;
        }
    }

    async fn did_save(&self, _: DidSaveTextDocumentParams) {
        self.client
            .log_message(MessageType::INFO, "Document saved")
            .await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let mut documents = self.documents.write().await;
        documents.remove(&params.text_document.uri);
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        
        let items = self.get_completions(&uri, position).await;
        Ok(Some(CompletionResponse::Array(items)))
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let _position = params.text_document_position_params.position;
        
        // Get the word at the position
        let documents = self.documents.read().await;
        if let Some(_doc) = documents.get(&uri) {
            // Simple hover for keywords
            let hover_text = "Seen Language construct";
            
            return Ok(Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: hover_text.to_string(),
                }),
                range: None,
            }));
        }
        
        Ok(None)
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        
        // Find the symbol at the cursor position
        if let Some(symbol_name) = self.find_symbol_at_position(&uri, position).await {
            // Look for the symbol definition
            let documents = self.documents.read().await;
            
            // First check current document
            if let Some(doc) = documents.get(&uri) {
                if let Some(symbol_info) = doc.symbols.get(&symbol_name) {
                    return Ok(Some(GotoDefinitionResponse::Scalar(
                        symbol_info.definition.clone(),
                    )));
                }
            }
            
            // Then check other documents
            for (doc_uri, doc) in documents.iter() {
                if doc_uri != &uri {
                    if let Some(symbol_info) = doc.symbols.get(&symbol_name) {
                        return Ok(Some(GotoDefinitionResponse::Scalar(
                            symbol_info.definition.clone(),
                        )));
                    }
                }
            }
        }
        
        Ok(None)
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let include_declaration = params.context.include_declaration;
        
        // Find the symbol at the cursor position
        if let Some(symbol_name) = self.find_symbol_at_position(&uri, position).await {
            let mut locations = Vec::new();
            let documents = self.documents.read().await;
            
            // Search all documents for references
            for (doc_uri, doc) in documents.iter() {
                // Find all occurrences of the symbol in the document
                let lines: Vec<&str> = doc.content.lines().collect();
                for (line_idx, line) in lines.iter().enumerate() {
                    let mut col = 0;
                    while let Some(pos) = line[col..].find(&symbol_name) {
                        let actual_col = col + pos;
                        
                        // Check if this is a word boundary (not part of a larger identifier)
                        let is_word_start = actual_col == 0 || 
                            !line.chars().nth(actual_col - 1).map_or(false, |c| c.is_alphanumeric() || c == '_');
                        let is_word_end = actual_col + symbol_name.len() >= line.len() ||
                            !line.chars().nth(actual_col + symbol_name.len()).map_or(false, |c| c.is_alphanumeric() || c == '_');
                        
                        if is_word_start && is_word_end {
                            let location = Location {
                                uri: doc_uri.clone(),
                                range: Range {
                                    start: Position {
                                        line: line_idx as u32,
                                        character: actual_col as u32,
                                    },
                                    end: Position {
                                        line: line_idx as u32,
                                        character: (actual_col + symbol_name.len()) as u32,
                                    },
                                },
                            };
                            
                            // Check if we should include the declaration
                            let is_declaration = doc.symbols.get(&symbol_name)
                                .map_or(false, |sym| sym.definition == location);
                            
                            if !is_declaration || include_declaration {
                                locations.push(location);
                            }
                        }
                        
                        col = actual_col + 1;
                    }
                }
            }
            
            if !locations.is_empty() {
                return Ok(Some(locations));
            }
        }
        
        Ok(None)
    }

    async fn document_highlight(
        &self,
        params: DocumentHighlightParams,
    ) -> Result<Option<Vec<DocumentHighlight>>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        
        // Find the symbol at the cursor position
        if let Some(symbol_name) = self.find_symbol_at_position(&uri, position).await {
            let mut highlights = Vec::new();
            let documents = self.documents.read().await;
            
            // Only highlight in the current document
            if let Some(doc) = documents.get(&uri) {
                let lines: Vec<&str> = doc.content.lines().collect();
                for (line_idx, line) in lines.iter().enumerate() {
                    let mut col = 0;
                    while let Some(pos) = line[col..].find(&symbol_name) {
                        let actual_col = col + pos;
                        
                        // Check if this is a word boundary
                        let is_word_start = actual_col == 0 || 
                            !line.chars().nth(actual_col - 1).map_or(false, |c| c.is_alphanumeric() || c == '_');
                        let is_word_end = actual_col + symbol_name.len() >= line.len() ||
                            !line.chars().nth(actual_col + symbol_name.len()).map_or(false, |c| c.is_alphanumeric() || c == '_');
                        
                        if is_word_start && is_word_end {
                            let range = Range {
                                start: Position {
                                    line: line_idx as u32,
                                    character: actual_col as u32,
                                },
                                end: Position {
                                    line: line_idx as u32,
                                    character: (actual_col + symbol_name.len()) as u32,
                                },
                            };
                            
                            // Determine highlight kind
                            let is_definition = doc.symbols.get(&symbol_name)
                                .map_or(false, |sym| {
                                    sym.definition.range == range
                                });
                            
                            let kind = if is_definition {
                                Some(DocumentHighlightKind::WRITE)
                            } else {
                                Some(DocumentHighlightKind::READ)
                            };
                            
                            highlights.push(DocumentHighlight {
                                range,
                                kind,
                            });
                        }
                        
                        col = actual_col + 1;
                    }
                }
            }
            
            if !highlights.is_empty() {
                return Ok(Some(highlights));
            }
        }
        
        Ok(None)
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = params.text_document.uri;
        let documents = self.documents.read().await;
        
        if let Some(doc) = documents.get(&uri) {
            let mut symbols = Vec::new();
            
            for symbol_info in doc.symbols.values() {
                symbols.push(SymbolInformation {
                    name: symbol_info.name.clone(),
                    kind: symbol_info.kind,
                    tags: None,
                    deprecated: None,
                    location: symbol_info.definition.clone(),
                    container_name: None,
                });
            }
            
            if !symbols.is_empty() {
                return Ok(Some(DocumentSymbolResponse::Flat(symbols)));
            }
        }
        
        Ok(None)
    }

    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        let uri = &params.text_document.uri;
        let range = params.range;
        
        let mut actions = Vec::new();
        
        // Get document content
        if let Some(doc) = self.documents.read().await.get(uri) {
            let content = &doc.content;
            let lines: Vec<&str> = content.lines().collect();
            
            // Quick fixes for common issues
            if let Some(line_content) = lines.get(range.start.line as usize) {
                let line_content = line_content.to_string();
                
                // Quick fix: Add missing semicolon
                if !line_content.trim().is_empty() && 
                   !line_content.trim_end().ends_with(';') && 
                   !line_content.trim_end().ends_with('{') && 
                   !line_content.trim_end().ends_with('}') {
                    
                    let edit = TextEdit {
                        range: Range {
                            start: Position { line: range.start.line, character: line_content.trim_end().len() as u32 },
                            end: Position { line: range.start.line, character: line_content.trim_end().len() as u32 },
                        },
                        new_text: ";".to_string(),
                    };
                    
                    let mut changes = HashMap::new();
                    changes.insert(uri.clone(), vec![edit]);
                    
                    actions.push(CodeActionOrCommand::CodeAction(CodeAction {
                        title: "Add missing semicolon".to_string(),
                        kind: Some(CodeActionKind::QUICKFIX),
                        diagnostics: Some(vec![]),
                        edit: Some(WorkspaceEdit {
                            changes: Some(changes),
                            document_changes: None,
                            change_annotations: None,
                        }),
                        command: None,
                        is_preferred: Some(true),
                        disabled: None,
                        data: None,
                    }));
                }
                
                // Quick fix: Convert var to let
                if line_content.trim().starts_with("var ") {
                    let start_char = line_content.find("var").unwrap() as u32;
                    let edit = TextEdit {
                        range: Range {
                            start: Position { line: range.start.line, character: start_char },
                            end: Position { line: range.start.line, character: start_char + 3 },
                        },
                        new_text: "let".to_string(),
                    };
                    
                    let mut changes = HashMap::new();
                    changes.insert(uri.clone(), vec![edit]);
                    
                    actions.push(CodeActionOrCommand::CodeAction(CodeAction {
                        title: "Convert 'var' to 'let'".to_string(),
                        kind: Some(CodeActionKind::REFACTOR_REWRITE),
                        diagnostics: Some(vec![]),
                        edit: Some(WorkspaceEdit {
                            changes: Some(changes),
                            document_changes: None,
                            change_annotations: None,
                        }),
                        command: None,
                        is_preferred: Some(false),
                        disabled: None,
                        data: None,
                    }));
                }
                
                // Code action: Extract function
                if range.start.line != range.end.line || 
                   (range.start.line == range.end.line && range.end.character - range.start.character > 10) {
                    
                    actions.push(CodeActionOrCommand::CodeAction(CodeAction {
                        title: "Extract to function".to_string(),
                        kind: Some(CodeActionKind::REFACTOR_EXTRACT),
                        diagnostics: Some(vec![]),
                        edit: None, // Would implement the actual extraction logic
                        command: Some(Command {
                            title: "Extract to function".to_string(),
                            command: "seen.extractFunction".to_string(),
                            arguments: Some(vec![
                                serde_json::json!({
                                    "uri": uri.to_string(),
                                    "range": range
                                })
                            ]),
                        }),
                        is_preferred: Some(false),
                        disabled: None,
                        data: None,
                    }));
                }
                
                // Code action: Add type annotation
                if line_content.contains("let ") && !line_content.contains(": ") {
                    actions.push(CodeActionOrCommand::CodeAction(CodeAction {
                        title: "Add type annotation".to_string(),
                        kind: Some(CodeActionKind::REFACTOR_REWRITE),
                        diagnostics: Some(vec![]),
                        edit: None,
                        command: Some(Command {
                            title: "Add type annotation".to_string(),
                            command: "seen.addTypeAnnotation".to_string(),
                            arguments: Some(vec![
                                serde_json::json!({
                                    "uri": uri.to_string(),
                                    "range": range
                                })
                            ]),
                        }),
                        is_preferred: Some(false),
                        disabled: None,
                        data: None,
                    }));
                }
            }
        }
        
        if actions.is_empty() {
            Ok(None)
        } else {
            Ok(Some(actions))
        }
    }

    async fn code_lens(&self, params: CodeLensParams) -> Result<Option<Vec<CodeLens>>> {
        let uri = &params.text_document.uri;
        let mut lenses = Vec::new();
        
        if let Some(doc) = self.documents.read().await.get(uri) {
            let content = &doc.content;
            
            // Parse the document to find functions
            let mut keyword_manager = KeywordManager::new();
            if keyword_manager.load_from_toml("en").is_ok() {
                keyword_manager.switch_language("en").ok();
                
                let lexer = Lexer::new(content.clone(), Arc::new(keyword_manager));
                let mut parser = Parser::new(lexer);
                
                if let Ok(program) = parser.parse_program() {
                    // Find function definitions and add lens
                    for (i, expr) in program.expressions.iter().enumerate() {
                        if let Expression::Function { name, .. } = expr {
                            let line = i as u32; // Approximate line number
                            
                            // Add "Run" lens for test functions
                            if name.starts_with("test_") || name.ends_with("_test") {
                                lenses.push(CodeLens {
                                    range: Range {
                                        start: Position { line, character: 0 },
                                        end: Position { line, character: name.len() as u32 },
                                    },
                                    command: Some(Command {
                                        title: "▶ Run Test".to_string(),
                                        command: "seen.runTest".to_string(),
                                        arguments: Some(vec![
                                            serde_json::json!({
                                                "uri": uri.to_string(),
                                                "functionName": name
                                            })
                                        ]),
                                    }),
                                    data: None,
                                });
                            } else {
                                // Add reference count lens for regular functions
                                let ref_count = self.count_references(content, name);
                                let title = if ref_count == 1 {
                                    "1 reference".to_string()
                                } else {
                                    format!("{} references", ref_count)
                                };
                                
                                lenses.push(CodeLens {
                                    range: Range {
                                        start: Position { line, character: 0 },
                                        end: Position { line, character: name.len() as u32 },
                                    },
                                    command: Some(Command {
                                        title,
                                        command: "seen.showReferences".to_string(),
                                        arguments: Some(vec![
                                            serde_json::json!({
                                                "uri": uri.to_string(),
                                                "functionName": name
                                            })
                                        ]),
                                    }),
                                    data: None,
                                });
                            }
                        }
                    }
                }
            }
        }
        
        if lenses.is_empty() {
            Ok(None)
        } else {
            Ok(Some(lenses))
        }
    }

    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        let uri = &params.text_document.uri;
        
        if let Some(doc) = self.documents.read().await.get(uri) {
            let content = &doc.content;
            
            // Parse and format the document
            let mut keyword_manager = KeywordManager::new();
            if keyword_manager.load_from_toml("en").is_ok() {
                keyword_manager.switch_language("en").ok();
                
                let lexer = Lexer::new(content.clone(), Arc::new(keyword_manager));
                let mut parser = Parser::new(lexer);
                
                if let Ok(program) = parser.parse_program() {
                    // Format the parsed program
                    let formatted = self.format_program(&program);
                    
                    // Create a text edit that replaces the entire document
                    let lines: Vec<&str> = content.lines().collect();
                    let last_line = lines.len() as u32;
                    let last_character = lines.last()
                        .map(|line| line.len() as u32)
                        .unwrap_or(0);
                    
                    let edit = TextEdit {
                        range: Range {
                            start: Position { line: 0, character: 0 },
                            end: Position { line: last_line.saturating_sub(1), character: last_character },
                        },
                        new_text: formatted,
                    };
                    
                    return Ok(Some(vec![edit]));
                }
            }
        }
        
        Ok(None)
    }

    async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let new_name = params.new_name;
        
        // Find the symbol at the cursor position
        if let Some(symbol_name) = self.find_symbol_at_position(&uri, position).await {
            let mut changes: HashMap<Url, Vec<TextEdit>> = HashMap::new();
            let documents = self.documents.read().await;
            
            // Find all occurrences and create text edits
            for (doc_uri, doc) in documents.iter() {
                let mut edits = Vec::new();
                let lines: Vec<&str> = doc.content.lines().collect();
                
                for (line_idx, line) in lines.iter().enumerate() {
                    let mut col = 0;
                    while let Some(pos) = line[col..].find(&symbol_name) {
                        let actual_col = col + pos;
                        
                        // Check if this is a word boundary
                        let is_word_start = actual_col == 0 || 
                            !line.chars().nth(actual_col - 1).map_or(false, |c| c.is_alphanumeric() || c == '_');
                        let is_word_end = actual_col + symbol_name.len() >= line.len() ||
                            !line.chars().nth(actual_col + symbol_name.len()).map_or(false, |c| c.is_alphanumeric() || c == '_');
                        
                        if is_word_start && is_word_end {
                            edits.push(TextEdit {
                                range: Range {
                                    start: Position {
                                        line: line_idx as u32,
                                        character: actual_col as u32,
                                    },
                                    end: Position {
                                        line: line_idx as u32,
                                        character: (actual_col + symbol_name.len()) as u32,
                                    },
                                },
                                new_text: new_name.clone(),
                            });
                        }
                        
                        col = actual_col + 1;
                    }
                }
                
                if !edits.is_empty() {
                    changes.insert(doc_uri.clone(), edits);
                }
            }
            
            if !changes.is_empty() {
                return Ok(Some(WorkspaceEdit {
                    changes: Some(changes),
                    document_changes: None,
                    change_annotations: None,
                }));
            }
        }
        
        Ok(None)
    }
}

/// Run the LSP server
pub async fn run_server() {
    env_logger::init();

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| SeenLanguageServer::new(client));
    Server::new(stdin, stdout, socket).serve(service).await;
}