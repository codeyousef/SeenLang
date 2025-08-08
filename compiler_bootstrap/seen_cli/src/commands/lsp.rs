//! Language Server Protocol implementation

use anyhow::Result;
use log::info;
use std::io::{self, BufRead, BufReader, Read, Write};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

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
        let response = handle_request(request);
        
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

fn handle_request(request: JsonRpcRequest) -> Option<JsonRpcResponse> {
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
        "textDocument/didOpen" | "textDocument/didChange" => {
            // Document opened or changed - would trigger diagnostics
            // For now, just acknowledge
            None
        }
        "textDocument/hover" => {
            // Provide hover information
            let hover_result = json!({
                "contents": {
                    "kind": "markdown",
                    "value": "**Seen Language Server**\n\nHover support coming soon!"
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
            // Provide completions
            let completions = json!({
                "isIncomplete": false,
                "items": [
                    {
                        "label": "fun",
                        "kind": 3,  // Function
                        "detail": "Define a function",
                        "insertText": "fun ${1:name}($2) {\n    $0\n}"
                    },
                    {
                        "label": "val",
                        "kind": 6,  // Variable
                        "detail": "Define an immutable variable",
                        "insertText": "val ${1:name} = $0"
                    },
                    {
                        "label": "var",
                        "kind": 6,  // Variable
                        "detail": "Define a mutable variable",
                        "insertText": "var ${1:name} = $0"
                    },
                    {
                        "label": "struct",
                        "kind": 7,  // Class
                        "detail": "Define a struct",
                        "insertText": "struct ${1:Name} {\n    $0\n}"
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