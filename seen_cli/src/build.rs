use anyhow::{bail, Context, Result};
use colored::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::{load_project_config, SeenConfig};

/// Build a Seen project
pub fn build_project(project_path: &Path) -> Result<()> {
    println!("{} Building Seen project...", "→".cyan());

    // Load project configuration
    let config = load_project_config(project_path)
        .context("Failed to load project configuration")?;
    
    println!("  {} Project: {}", "•".yellow(), config.name.blue());
    println!("  {} Language: {}", "•".yellow(), config.language.blue());
    
    // Find the main source file
    let main_file = find_main_file(project_path)
        .context("Failed to find main source file")?;
    
    println!("  {} Main file: {}", "•".yellow(), main_file.display().to_string().blue());
    
    // Create output directory if it doesn't exist
    let output_dir = project_path.join(&config.build.output_dir);
    fs::create_dir_all(&output_dir)
        .with_context(|| format!("Failed to create output directory: {}", output_dir.display()))?;
    
    // Compile the project
    compile_project(project_path, &main_file, &output_dir, &config)
        .context("Failed to compile project")?;
    
    println!("{} Build successful", "✓".green().bold());
    
    Ok(())
}

/// Find the main source file in the project
fn find_main_file(project_path: &Path) -> Result<PathBuf> {
    let src_dir = project_path.join("src");
    
    if !src_dir.exists() {
        bail!("Source directory not found: {}", src_dir.display());
    }
    
    let main_file = src_dir.join("main.seen");
    
    if !main_file.exists() {
        bail!("Main file not found: {}", main_file.display());
    }
    
    Ok(main_file)
}

/// Compile a Seen project
fn compile_project(project_path: &Path, main_file: &Path, output_dir: &Path, config: &SeenConfig) -> Result<()> {
    // 1. Lexical analysis
    println!("  {} Lexical analysis...", "→".cyan());
    let source_code = fs::read_to_string(main_file)
        .with_context(|| format!("Failed to read source file: {}", main_file.display()))?;
    
    // Create the keyword manager based on the language setting
    // Look for language files in a standard location relative to the project
    let lang_dir = project_path.join("languages");
    
    // If language directory doesn't exist, try looking in the source directory
    let lang_dir = if lang_dir.exists() {
        lang_dir
    } else {
        // Fall back to using predefined language files from the Seen standard library
        // In a real implementation, this would point to where the language files are installed
        PathBuf::from("/usr/src/app/seen_lexer/languages")
    };
    
    // Create keyword configuration from the language directory
    let keyword_config = seen_lexer::keyword_config::KeywordConfig::from_directory(&lang_dir)
        .context(format!("Failed to load keyword configuration from {}", lang_dir.display()))?;
    
    // Create the keyword manager with the configuration and language setting
    let keyword_manager = seen_lexer::keyword_config::KeywordManager::new(keyword_config, config.language.clone())
        .context("Failed to create keyword manager")?;
    
    // Create the lexer
    let mut lexer = seen_lexer::lexer::Lexer::new(&source_code, &keyword_manager, config.language.clone());
    
    // Tokenize the source code
    let tokens = lexer.tokenize()
        .context("Lexical analysis failed")?;
    
    println!("    {} {} tokens generated", "✓".green(), tokens.len());
    
    // 2. Parsing
    println!("  {} Parsing...", "→".cyan());
    let ast = seen_parser::parse(tokens)
        .context("Parsing failed")?;
    
    println!("    {} AST generated", "✓".green());
    
    // 3. Code generation
    println!("  {} Code generation...", "→".cyan());
    
    // Create temporary files for intermediate representations
    let ir_file = output_dir.join(format!("{}.ll", config.name));
    let exe_file = output_dir.join(&config.name);
    
    // Create LLVM context
    let context = inkwell::context::Context::create();
    let mut codegen = seen_ir::CodeGenerator::new(&context, &config.name)
        .context("Failed to create code generator")?;
    
    // Generate LLVM IR
    let module = codegen.generate(&ast)
        .context("Failed to generate LLVM IR")?;
    
    // Print the LLVM IR to a file
    module.print_to_file(&ir_file)
        .map_err(|e| anyhow::anyhow!("Failed to write LLVM IR to file: {}", e))?;
    
    println!("    {} LLVM IR generated: {}", "✓".green(), ir_file.display().to_string().blue());
    
    // 4. Compile to executable
    println!("  {} Compiling to executable...", "→".cyan());
    
    // Create a temporary object file
    let temp_dir = tempfile::tempdir()
        .context("Failed to create temporary directory")?;
    let obj_file = temp_dir.path().join("temp.o");
    
    // Convert LLVM IR to object file
    let llc_status = Command::new("llc")
        .args(["-filetype=obj", ir_file.to_str().unwrap(), "-o", obj_file.to_str().unwrap()])
        .status()
        .context("Failed to run llc (LLVM static compiler)")?;
    
    if !llc_status.success() {
        bail!("Failed to convert LLVM IR to object file");
    }
    
    // Link the object file to create an executable
    let clang_status = Command::new("clang")
        .args([obj_file.to_str().unwrap(), "-o", exe_file.to_str().unwrap()])
        .status()
        .context("Failed to run clang (LLVM frontend compiler)")?;
    
    if !clang_status.success() {
        bail!("Failed to link object file");
    }
    
    println!("    {} Executable generated: {}", "✓".green(), exe_file.display().to_string().blue());
    
    Ok(())
}
