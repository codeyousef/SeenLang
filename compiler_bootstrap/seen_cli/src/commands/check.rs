//! Check command implementation

use anyhow::{Result, Context};
use std::path::PathBuf;
use log::{info, error};
use crate::project::Project;

/// Execute the check command
pub fn execute(manifest_path: Option<PathBuf>, watch: bool) -> Result<()> {
    if watch {
        info!("Checking Seen project (watch mode)...");
        check_with_watch(manifest_path)
    } else {
        info!("Checking Seen project...");
        check_once(manifest_path)
    }
}

fn check_once(manifest_path: Option<PathBuf>) -> Result<()> {
    let project = Project::find_and_load(manifest_path)?;
    
    info!("Project: {} v{}", project.name(), project.version());
    
    // Load language configuration
    let lang_config = project.load_language_config()
        .context("Failed to load language configuration")?;
    
    // Find source files
    let source_files = project.find_source_files()
        .context("Failed to find source files")?;
    
    if source_files.is_empty() {
        info!("No source files found - nothing to check");
        return Ok(());
    }
    
    info!("Checking {} source files...", source_files.len());
    
    let mut has_errors = false;
    
    // Check each source file
    for source_file in &source_files {
        info!("Checking {}", source_file.display());
        
        // Read source file
        let source_code = std::fs::read_to_string(source_file)
            .with_context(|| format!("Failed to read source file: {}", source_file.display()))?;
        
        // Perform lexical analysis
        let mut lexer = seen_lexer::Lexer::new(&source_code, 0, &lang_config);
        let tokens = lexer.tokenize()
            .context("Lexical analysis failed")?;
        
        // Check for lexical errors
        if lexer.diagnostics().has_errors() {
            has_errors = true;
            error!("Lexical errors in {}", source_file.display());
            for diagnostic in lexer.diagnostics().errors() {
                error!("  {}", diagnostic.message());
            }
        } else {
            info!("  ✓ Lexical analysis passed ({} tokens)", tokens.len());
        }
        
        // Parsing
        let mut parser = seen_parser::Parser::new(tokens);
        let ast = parser.parse_program()
            .context("Parsing failed")?;
        
        if parser.diagnostics().has_errors() {
            has_errors = true;
            error!("Parse errors in {}", source_file.display());
            for diagnostic in parser.diagnostics().errors() {
                error!("  {}", diagnostic.message());
            }
        } else {
            info!("  ✓ Parsing passed ({} items)", ast.items.len());
        }
        
        // Type checking
        let mut type_checker = seen_typechecker::TypeChecker::new();
        type_checker.check_program(&ast)
            .context("Type checking failed")?;
        
        if type_checker.diagnostics().has_errors() {
            has_errors = true;
            error!("Type errors in {}", source_file.display());
            for diagnostic in type_checker.diagnostics().errors() {
                error!("  {}", diagnostic.message());
            }
        } else {
            info!("  ✓ Type checking passed");
        }
    }
    
    if has_errors {
        error!("Check failed - found errors in source files");
        return Err(anyhow::anyhow!("Check failed"));
    }
    
    info!("✓ All checks passed!");
    Ok(())
}

fn check_with_watch(_manifest_path: Option<PathBuf>) -> Result<()> {
    // File watching will be implemented in a future phase
    info!("Watch mode will be available in a future release - running check once");
    check_once(_manifest_path)
}