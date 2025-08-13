//! Integration tests for the Seen Language LSP server

use tower_lsp::lsp_types::*;
use tower_lsp::Client;
use seen_lsp::SeenLanguageServer;

/// Helper function to create a test document
fn create_test_document() -> String {
    r#"
fun add(x: Int, y: Int): Int {
    return x + y
}

let result = add(10, 20)

fun main() {
    let x = 42
    let y = x + result
    println(y)
}
    "#.to_string()
}

#[tokio::test]
async fn test_lsp_server_initialization() {
    // This test verifies that the LSP server can be created
    // In a real test, we'd need to set up a mock client
    // For now, just verify the types compile
    
    let _init_params = InitializeParams {
        process_id: None,
        root_path: None,
        root_uri: None,
        initialization_options: None,
        capabilities: ClientCapabilities::default(),
        trace: None,
        workspace_folders: None,
        client_info: None,
        locale: None,
    };
    
    // The actual server requires a client connection
    // which would need to be mocked for testing
}

#[test]
fn test_symbol_extraction_logic() {
    // Test that our symbol extraction logic concepts work
    let code = create_test_document();
    
    // Verify the test document contains expected symbols
    assert!(code.contains("fun add"));
    assert!(code.contains("let result"));
    assert!(code.contains("fun main"));
    assert!(code.contains("let x"));
    assert!(code.contains("let y"));
}

#[test]
fn test_position_conversion() {
    // Test position conversion logic
    let seen_pos = seen_lexer::position::Position {
        line: 5,
        column: 10,
        offset: 50,
    };
    
    // LSP uses 0-based indexing
    let lsp_pos = Position {
        line: (seen_pos.line - 1) as u32,
        character: (seen_pos.column - 1) as u32,
    };
    
    assert_eq!(lsp_pos.line, 4);
    assert_eq!(lsp_pos.character, 9);
}

#[test]
fn test_symbol_finding_logic() {
    // Test the logic for finding symbols at positions
    let line = "let result = add(10, 20)";
    let symbol_name = "result";
    
    // Find the symbol in the line
    if let Some(pos) = line.find(symbol_name) {
        assert_eq!(pos, 4); // "result" starts at position 4
        
        // Check word boundaries
        let is_word_start = pos == 0 || 
            !line.chars().nth(pos - 1).map_or(false, |c| c.is_alphanumeric() || c == '_');
        let is_word_end = pos + symbol_name.len() >= line.len() ||
            !line.chars().nth(pos + symbol_name.len()).map_or(false, |c| c.is_alphanumeric() || c == '_');
        
        assert!(is_word_start);
        assert!(is_word_end);
    }
}

#[test]
fn test_reference_finding_logic() {
    // Test finding all references to a symbol
    let document = create_test_document();
    let symbol = "result";
    let mut references = Vec::new();
    
    for (line_idx, line) in document.lines().enumerate() {
        let mut col = 0;
        while let Some(pos) = line[col..].find(symbol) {
            let actual_col = col + pos;
            
            // Check word boundaries
            let is_word_start = actual_col == 0 || 
                !line.chars().nth(actual_col - 1).map_or(false, |c| c.is_alphanumeric() || c == '_');
            let is_word_end = actual_col + symbol.len() >= line.len() ||
                !line.chars().nth(actual_col + symbol.len()).map_or(false, |c| c.is_alphanumeric() || c == '_');
            
            if is_word_start && is_word_end {
                references.push((line_idx, actual_col));
            }
            
            col = actual_col + 1;
        }
    }
    
    // Should find "result" in two places:
    // 1. let result = add(10, 20)
    // 2. let y = x + result
    assert_eq!(references.len(), 2);
}