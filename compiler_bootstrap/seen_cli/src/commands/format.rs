//! Format command implementation

use anyhow::{Result, Context};
use std::path::PathBuf;
use log::{info, warn};

/// Execute the format command
pub fn execute(check: bool, paths: Vec<PathBuf>) -> Result<()> {
    if check {
        info!("Checking code formatting...");
    } else {
        info!("Formatting code...");
    }
    
    let target_paths = if paths.is_empty() {
        // Format current directory
        vec![std::env::current_dir().context("Failed to get current directory")?]
    } else {
        paths
    };
    
    let mut files_to_format = Vec::new();
    
    // Collect all formattable files
    for path in target_paths {
        if path.is_file() {
            if is_formattable_file(&path) {
                files_to_format.push(path);
            }
        } else if path.is_dir() {
            collect_formattable_files(&path, &mut files_to_format)?;
        }
    }
    
    if files_to_format.is_empty() {
        info!("No files to format found");
        return Ok(());
    }
    
    info!("Found {} files to format", files_to_format.len());
    
    let mut formatted_count = 0;
    let mut unchanged_count = 0;
    
    for file_path in &files_to_format {
        info!("Processing {}", file_path.display());
        
        match format_file(file_path, check) {
            Ok(changed) => {
                if changed {
                    formatted_count += 1;
                    if check {
                        warn!("  Would reformat");
                    } else {
                        info!("  Formatted");
                    }
                } else {
                    unchanged_count += 1;
                    info!("  Already formatted");
                }
            }
            Err(e) => {
                warn!("  Failed to format: {}", e);
            }
        }
    }
    
    if check {
        info!("Format check: {} files would be reformatted, {} already formatted", 
              formatted_count, unchanged_count);
        if formatted_count > 0 {
            return Err(anyhow::anyhow!("Some files need formatting"));
        }
    } else {
        info!("Formatted {} files, {} were already formatted", 
              formatted_count, unchanged_count);
    }
    
    Ok(())
}

fn is_formattable_file(path: &PathBuf) -> bool {
    if let Some(extension) = path.extension().and_then(|e| e.to_str()) {
        matches!(extension, "seen" | "md" | "toml")
    } else {
        false
    }
}

fn collect_formattable_files(dir: &PathBuf, files: &mut Vec<PathBuf>) -> Result<()> {
    for entry in walkdir::WalkDir::new(dir) {
        let entry = entry.context("Failed to read directory entry")?;
        let path = entry.path();
        
        if path.is_file() && is_formattable_file(&path.to_path_buf()) {
            files.push(path.to_path_buf());
        }
    }
    Ok(())
}

fn format_file(file_path: &PathBuf, check_only: bool) -> Result<bool> {
    let content = std::fs::read_to_string(file_path)
        .with_context(|| format!("Failed to read file: {}", file_path.display()))?;
    
    let formatted_content = match file_path.extension().and_then(|e| e.to_str()) {
        Some("seen") => format_seen_code(&content)?,
        Some("md") => format_markdown(&content)?,
        Some("toml") => format_toml(&content)?,
        _ => return Ok(false),
    };
    
    let changed = content != formatted_content;
    
    if changed && !check_only {
        std::fs::write(file_path, formatted_content)
            .with_context(|| format!("Failed to write formatted file: {}", file_path.display()))?;
    }
    
    Ok(changed)
}

fn format_seen_code(content: &str) -> Result<String> {
    // Basic formatting: normalize whitespace (full formatter in Alpha phase)
    let lines: Vec<&str> = content.lines().collect();
    let formatted_lines: Vec<String> = lines
        .into_iter()
        .map(|line| line.trim_end().to_string())
        .collect();
    
    Ok(formatted_lines.join("\n") + "\n")
}

fn format_markdown(content: &str) -> Result<String> {
    // Basic markdown formatting: normalize line endings
    Ok(content.replace("\r\n", "\n"))
}

fn format_toml(content: &str) -> Result<String> {
    // Basic TOML formatting: normalize whitespace
    let lines: Vec<&str> = content.lines().collect();
    let formatted_lines: Vec<String> = lines
        .into_iter()
        .map(|line| line.trim_end().to_string())
        .collect();
    
    Ok(formatted_lines.join("\n") + "\n")
}