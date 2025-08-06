//! Format command implementation

use anyhow::{Result, Context};
use std::path::PathBuf;
use log::{info, warn};
use seen_std::formatting::{
    format_document, FileType, FormatConfig, FormatStats
};
use crate::project::Project;

/// Execute the format command
pub fn execute(check: bool, paths: Vec<PathBuf>) -> Result<()> {
    // Load format configuration from project
    let config = load_format_config()?;
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
    
    let mut stats = FormatStats::new();
    
    for file_path in &files_to_format {
        info!("Processing {}", file_path.display());
        
        match format_file(file_path, check, &config) {
            Ok(result) => {
                stats.add_result(&result);
                
                if let Some(error) = &result.error {
                    warn!("  Error: {}", error);
                } else if result.changed {
                    if check {
                        warn!("  Would reformat");
                    } else {
                        info!("  Formatted");
                    }
                } else {
                    info!("  Already formatted");
                }
            }
            Err(e) => {
                warn!("  Failed to process: {}", e);
            }
        }
    }
    
    // Print comprehensive statistics
    print_format_stats(&stats, check);
    
    if check && stats.files_changed > 0 {
        return Err(anyhow::anyhow!("Some files need formatting"));
    }
    
    if stats.files_with_errors > 0 {
        return Err(anyhow::anyhow!("Some files had formatting errors"));
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

fn format_file(file_path: &PathBuf, check_only: bool, config: &FormatConfig) -> Result<seen_std::formatting::FormatResult> {
    let content = std::fs::read_to_string(file_path)
        .with_context(|| format!("Failed to read file: {}", file_path.display()))?;
    
    let file_type = FileType::from_path(file_path);
    let result = format_document(&content, &file_type, config);
    
    // Write formatted content if not in check mode and content changed
    if result.changed && !check_only && result.error.is_none() {
        let content_str: &str = result.formatted_content.as_str();
        std::fs::write(file_path, content_str)
            .with_context(|| format!("Failed to write formatted file: {}", file_path.display()))?;
    }
    
    Ok(result)
}

/// Load format configuration from project or use defaults
fn load_format_config() -> Result<FormatConfig> {
    // Try to load project-specific configuration
    if let Ok(project) = Project::find_and_load(None) {
        load_project_format_config(&project)
    } else {
        // Use default configuration
        Ok(FormatConfig::default())
    }
}

/// Load format configuration from project settings
fn load_project_format_config(project: &Project) -> Result<FormatConfig> {
    let project_settings = &project.config().format;
    
    let mut config = FormatConfig::default();
    
    // Map project settings to format config
    config.indent_size = project_settings.indent;
    config.max_line_length = project_settings.line_width;
    config.trailing_comma = project_settings.trailing_comma;
    
    // Set file type formatting based on document_types configuration
    let doc_types = &project_settings.document_types;
    config.format_seen = doc_types.contains(&".seen".to_string());
    config.format_markdown = doc_types.contains(&".md".to_string());
    config.format_toml = doc_types.contains(&".toml".to_string());
    
    Ok(config)
}

/// Print comprehensive formatting statistics
fn print_format_stats(stats: &FormatStats, check_mode: bool) {
    let action = if check_mode { "checked" } else { "processed" };
    
    info!("Formatting complete:");
    info!("  Files {}: {}", action, stats.files_processed);
    
    if check_mode {
        info!("  Files that would be reformatted: {}", stats.files_changed);
    } else {
        info!("  Files reformatted: {}", stats.files_changed);
    }
    
    if stats.files_with_errors > 0 {
        warn!("  Files with errors: {}", stats.files_with_errors);
    }
    
    let unchanged = stats.files_processed - stats.files_changed - stats.files_with_errors;
    info!("  Files already correctly formatted: {}", unchanged);
    
    if stats.files_processed > 0 {
        info!("  Success rate: {:.1}%", stats.success_rate() * 100.0);
    }
}