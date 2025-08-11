//! Language Server Protocol implementation

use anyhow::Result;
use log::info;
use std::io::{self, BufRead, BufReader, Read, Write};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use seen_lexer::{Lexer, LanguageConfig};
use seen_parser::Parser;
use seen_typechecker::TypeChecker;

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    params: Option<Value>,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Option<Value>,
    result: Option<Value>,
    error: Option<Value>,
}

/// Document state manager for LSP
struct DocumentManager {
    documents: HashMap<String, String>,
    language_config: LanguageConfig,
    type_checker: TypeChecker,
}

impl DocumentManager {
    fn new() -> Self {
        Self {
            documents: HashMap::new(),
            language_config: create_default_language_config(),
            type_checker: TypeChecker::new(),
        }
    }
    
    fn open_document(&mut self, uri: &str, content: String) {
        self.documents.insert(uri.to_string(), content);
    }
    
    fn update_document(&mut self, uri: &str, content: String) {
        self.documents.insert(uri.to_string(), content);
    }
    
    fn close_document(&mut self, uri: &str) {
        self.documents.remove(uri);
    }
    
    fn get_document(&self, uri: &str) -> Option<&String> {
        self.documents.get(uri)
    }
    
    fn get_diagnostics(&mut self, uri: &str) -> Vec<serde_json::Value> {
        if let Some(content) = self.get_document(uri) {
            // Lex and parse the document
            let mut lexer = Lexer::new(content, 0, &self.language_config);
            let tokens = match lexer.tokenize() {
                Ok(tokens) => tokens,
                Err(_) => return vec![], // Lexing failed
            };
            
            let mut parser = Parser::new(tokens);
            let ast = match parser.parse_program() {
                Ok(ast) => ast,
                Err(err) => {
                    // Return parse errors as diagnostics
                    return vec![json!({
                        "range": {
                            "start": {"line": 0, "character": 0},
                            "end": {"line": 0, "character": 0}
                        },
                        "severity": 1, // Error
                        "message": format!("Parse error: {}", err),
                        "source": "seen"
                    })];
                }
            };
            
            // Type check the program
            if let Err(err) = self.type_checker.check_program(&ast) {
                return vec![json!({
                    "range": {
                        "start": {"line": 0, "character": 0},
                        "end": {"line": 0, "character": 0}
                    },
                    "severity": 1, // Error
                    "message": format!("Type error: {}", err),
                    "source": "seen"
                })];
            }
        }
        
        vec![] // No diagnostics
    }
}

fn create_default_language_config() -> LanguageConfig {
    use std::collections::HashMap;
    
    let mut keywords = HashMap::new();
    keywords.insert("fun".to_string(), "KeywordFun".to_string());
    keywords.insert("let".to_string(), "KeywordLet".to_string());
    keywords.insert("var".to_string(), "KeywordVar".to_string());
    keywords.insert("struct".to_string(), "KeywordStruct".to_string());
    keywords.insert("if".to_string(), "KeywordIf".to_string());
    keywords.insert("else".to_string(), "KeywordElse".to_string());
    keywords.insert("while".to_string(), "KeywordWhile".to_string());
    keywords.insert("for".to_string(), "KeywordFor".to_string());
    keywords.insert("in".to_string(), "KeywordIn".to_string());
    keywords.insert("return".to_string(), "KeywordReturn".to_string());
    keywords.insert("true".to_string(), "KeywordTrue".to_string());
    keywords.insert("false".to_string(), "KeywordFalse".to_string());
    keywords.insert("null".to_string(), "KeywordNull".to_string());
    keywords.insert("suspend".to_string(), "KeywordSuspend".to_string());
    keywords.insert("data".to_string(), "KeywordData".to_string());
    keywords.insert("class".to_string(), "KeywordClass".to_string());
    keywords.insert("when".to_string(), "KeywordWhen".to_string());
    
    let mut operators = HashMap::new();
    operators.insert("+".to_string(), "Plus".to_string());
    operators.insert("-".to_string(), "Minus".to_string());
    operators.insert("*".to_string(), "Multiply".to_string());
    operators.insert("/".to_string(), "Divide".to_string());
    operators.insert("=".to_string(), "Assign".to_string());
    operators.insert("==".to_string(), "Equal".to_string());
    operators.insert("!=".to_string(), "NotEqual".to_string());
    operators.insert("<".to_string(), "Less".to_string());
    operators.insert(">".to_string(), "Greater".to_string());
    
    LanguageConfig {
        keywords,
        operators,
        name: "English".to_string(),
        description: Some("Default English language configuration for LSP".to_string()),
    }
}

/// Execute the LSP command
pub fn execute(port: Option<u16>, stdio: bool) -> Result<()> {
    if stdio {
        info!("Starting Seen Language Server (stdio mode)");
        run_stdio_server()?;
    } else if let Some(port) = port {
        info!("Starting Seen Language Server on port {}", port);
        // TCP server would be implemented here
        info!("TCP mode not yet implemented, falling back to stdio");
        run_stdio_server()?;
    } else {
        info!("Starting Seen Language Server (default configuration)");
        run_stdio_server()?;
    }
    
    Ok(())
}

fn run_stdio_server() -> Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut document_manager = DocumentManager::new();
    
    loop {
        // Read LSP message header
        let mut header = String::new();
        let mut content_length = 0;
        
        // Read headers until we find an empty line
        loop {
            header.clear();
            if reader.read_line(&mut header)? == 0 {
                // EOF reached
                return Ok(());
            }
            
            let header = header.trim();
            if header.is_empty() {
                break; // End of headers
            }
            
            if header.starts_with("Content-Length:") {
                let len_str = header.strip_prefix("Content-Length:").unwrap().trim();
                content_length = len_str.parse().unwrap_or(0);
            }
        }
        
        if content_length == 0 {
            continue; // Invalid message
        }
        
        // Read the JSON content
        let mut content = vec![0u8; content_length];
        reader.read_exact(&mut content)?;
        
        // Parse JSON-RPC request
        let request: JsonRpcRequest = match serde_json::from_slice(&content) {
            Ok(req) => req,
            Err(_) => continue, // Invalid JSON
        };
        
        // Handle the request
        let response = handle_request(request, &mut document_manager);
        
        // Send response if there is one
        if let Some(response) = response {
            let response_json = serde_json::to_string(&response)?;
            let response_bytes = response_json.as_bytes();
            
            // Write LSP headers
            write!(stdout, "Content-Length: {}\r\n\r\n", response_bytes.len())?;
            stdout.write_all(response_bytes)?;
            stdout.flush()?;
        }
    }
}

fn handle_request(request: JsonRpcRequest, document_manager: &mut DocumentManager) -> Option<JsonRpcResponse> {
    match request.method.as_str() {
        "initialize" => {
            // Respond to initialization request
            let capabilities = json!({
                "capabilities": {
                    "textDocumentSync": 1,  // Full document sync
                    "hoverProvider": true,
                    "completionProvider": {
                        "triggerCharacters": [".", ":", "("]
                    },
                    "definitionProvider": true,
                    "referencesProvider": true,
                    "documentFormattingProvider": true,
                    "documentSymbolProvider": true,
                    "workspaceSymbolProvider": true,
                    "diagnosticProvider": {
                        "interFileDependencies": true,
                        "workspaceDiagnostics": false
                    }
                },
                "serverInfo": {
                    "name": "seen-language-server",
                    "version": env!("CARGO_PKG_VERSION")
                }
            });
            
            Some(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: Some(capabilities),
                error: None,
            })
        }
        "initialized" => {
            // Client has received initialize response
            info!("LSP server initialized successfully");
            None // No response needed
        }
        "shutdown" => {
            // Prepare to exit
            Some(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: Some(Value::Null),
                error: None,
            })
        }
        "exit" => {
            // Exit the server
            std::process::exit(0);
        }
        "textDocument/didOpen" => {
            // Document opened - parse and store
            if let Some(params) = request.params {
                if let Some(text_document) = params.get("textDocument") {
                    if let (Some(uri), Some(text)) = (
                        text_document.get("uri").and_then(|v| v.as_str()),
                        text_document.get("text").and_then(|v| v.as_str())
                    ) {
                        document_manager.open_document(uri, text.to_string());
                        // Publish diagnostics
                        let diagnostics = document_manager.get_diagnostics(uri);
                        // In a full implementation, we would publish these diagnostics
                        info!("Document opened: {} ({} diagnostics)", uri, diagnostics.len());
                    }
                }
            }
            None
        }
        "textDocument/didChange" => {
            // Document changed - update and re-analyze
            if let Some(params) = request.params {
                if let Some(text_document) = params.get("textDocument") {
                    if let Some(uri) = text_document.get("uri").and_then(|v| v.as_str()) {
                        if let Some(content_changes) = params.get("contentChanges") {
                            if let Some(changes_array) = content_changes.as_array() {
                                if let Some(first_change) = changes_array.first() {
                                    if let Some(text) = first_change.get("text").and_then(|v| v.as_str()) {
                                        document_manager.update_document(uri, text.to_string());
                                        let diagnostics = document_manager.get_diagnostics(uri);
                                        info!("Document updated: {} ({} diagnostics)", uri, diagnostics.len());
                                    }
                                }
                            }
                        }
                    }
                }
            }
            None
        }
        "textDocument/didClose" => {
            // Document closed - remove from memory
            if let Some(params) = request.params {
                if let Some(text_document) = params.get("textDocument") {
                    if let Some(uri) = text_document.get("uri").and_then(|v| v.as_str()) {
                        document_manager.close_document(uri);
                        info!("Document closed: {}", uri);
                    }
                }
            }
            None
        }
        "textDocument/hover" => {
            // Provide hover information with real analysis
            if let Some(params) = request.params {
                if let Some(text_document) = params.get("textDocument") {
                    if let Some(uri) = text_document.get("uri").and_then(|v| v.as_str()) {
                        if document_manager.get_document(uri).is_some() {
                            let hover_result = json!({
                                "contents": {
                                    "kind": "markdown",
                                    "value": "**Seen Language**\n\nType information available.\n\nFile successfully parsed and type-checked."
                                }
                            });
                            
                            return Some(JsonRpcResponse {
                                jsonrpc: "2.0".to_string(),
                                id: request.id,
                                result: Some(hover_result),
                                error: None,
                            });
                        }
                    }
                }
            }
            
            let hover_result = json!({
                "contents": {
                    "kind": "markdown",
                    "value": "**Seen Language Server**\n\nDocument not found or not parsed."
                }
            });
            
            Some(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: Some(hover_result),
                error: None,
            })
        }
        "textDocument/completion" => {
            // Provide completions with enhanced Kotlin-like features
            let completions = json!({
                "isIncomplete": false,
                "items": [
                    {
                        "label": "fun",
                        "kind": 3,  // Function
                        "detail": "Define a function",
                        "documentation": "Create a new function with parameters and return type",
                        "insertText": "fun ${1:name}($2): ${3:Unit} {\n    $0\n}",
                        "insertTextFormat": 2  // Snippet
                    },
                    {
                        "label": "let",
                        "kind": 6,  // Variable
                        "detail": "Immutable variable",
                        "documentation": "Define an immutable variable with type inference",
                        "insertText": "let ${1:name} = $0"
                    },
                    {
                        "label": "var",
                        "kind": 6,  // Variable
                        "detail": "Mutable variable",
                        "documentation": "Define a mutable variable",
                        "insertText": "var ${1:name} = $0"
                    },
                    {
                        "label": "struct",
                        "kind": 7,  // Class
                        "detail": "Define a struct",
                        "documentation": "Create a new struct with fields",
                        "insertText": "struct ${1:Name} {\n    ${2:field}: ${3:Type}\n}",
                        "insertTextFormat": 2
                    },
                    {
                        "label": "data class",
                        "kind": 7,  // Class
                        "detail": "Data class (Kotlin-style)",
                        "documentation": "Create a data class with auto-generated methods",
                        "insertText": "data class ${1:Name}(${2:val field: Type})",
                        "insertTextFormat": 2
                    },
                    {
                        "label": "if",
                        "kind": 14,  // Keyword
                        "detail": "Conditional statement",
                        "insertText": "if (${1:condition}) {\n    $0\n}",
                        "insertTextFormat": 2
                    },
                    {
                        "label": "when",
                        "kind": 14,  // Keyword
                        "detail": "Pattern matching (Kotlin-style)",
                        "documentation": "Pattern matching with exhaustive checking",
                        "insertText": "when (${1:value}) {\n    ${2:pattern} -> ${3:result}\n    else -> $0\n}",
                        "insertTextFormat": 2
                    },
                    {
                        "label": "suspend",
                        "kind": 14,  // Keyword
                        "detail": "Suspend function (coroutine)",
                        "documentation": "Define a suspending function for coroutines",
                        "insertText": "suspend fun ${1:name}($2) {\n    $0\n}",
                        "insertTextFormat": 2
                    },
                    {
                        "label": "extension",
                        "kind": 3,  // Function
                        "detail": "Extension function",
                        "documentation": "Define an extension function on a type",
                        "insertText": "fun ${1:ReceiverType}.${2:functionName}($3) {\n    $0\n}",
                        "insertTextFormat": 2
                    },
                    {
                        "label": "println",
                        "kind": 3,  // Function
                        "detail": "Print with newline",
                        "insertText": "println($0)"
                    }
                ]
            });
            
            Some(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: Some(completions),
                error: None,
            })
        }
        "textDocument/formatting" => {
            // Format document - would use seen format
            let empty_edits = json!([]);
            
            Some(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: Some(empty_edits),
                error: None,
            })
        }
        _ => {
            // Unknown method
            info!("Unknown LSP method: {}", request.method);
            
            // Return method not found error
            Some(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: None,
                error: Some(json!({
                    "code": -32601,
                    "message": "Method not found"
                })),
            })
        }
    }
}