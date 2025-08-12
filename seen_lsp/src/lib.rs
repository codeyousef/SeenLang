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
}

/// Main LSP backend for Seen Language
pub struct SeenLanguageServer {
    /// LSP client handle
    client: Client,
    /// Documents currently open in the editor
    documents: Arc<RwLock<HashMap<Url, DocumentInfo>>>,
    /// Language configuration (keywords for different languages)
    language_config: Arc<RwLock<String>>,
}

impl SeenLanguageServer {
    /// Create a new Seen Language Server
    pub fn new(client: Client) -> Self {
        Self {
            client,
            documents: Arc::new(RwLock::new(HashMap::new())),
            language_config: Arc::new(RwLock::new("en".to_string())),
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

        items
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

        // Store document info
        let mut documents = self.documents.write().await;
        documents.insert(
            uri.clone(),
            DocumentInfo {
                content,
                version,
                ast: None,
                type_info: None,
                diagnostics: diagnostics.clone(),
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

            // Update document info
            let mut documents = self.documents.write().await;
            if let Some(doc) = documents.get_mut(&uri) {
                doc.content = content.clone();
                doc.version = version;
                doc.diagnostics = diagnostics.clone();
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
        _params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        // TODO: Implement go-to-definition
        Ok(None)
    }

    async fn references(&self, _params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        // TODO: Implement find references
        Ok(None)
    }

    async fn document_highlight(
        &self,
        _params: DocumentHighlightParams,
    ) -> Result<Option<Vec<DocumentHighlight>>> {
        // TODO: Implement document highlight
        Ok(None)
    }

    async fn document_symbol(
        &self,
        _params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        // TODO: Implement document symbols
        Ok(None)
    }

    async fn code_action(&self, _params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        // TODO: Implement code actions
        Ok(None)
    }

    async fn code_lens(&self, _params: CodeLensParams) -> Result<Option<Vec<CodeLens>>> {
        // TODO: Implement code lens
        Ok(None)
    }

    async fn formatting(&self, _params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        // TODO: Implement formatting
        Ok(None)
    }

    async fn rename(&self, _params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        // TODO: Implement rename
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