//! Utility functions for the Seen CLI

use anyhow::{Result, Context};
use std::path::Path;

/// Check if a directory contains a Seen project (has Seen.toml)
pub fn is_seen_project(dir: &Path) -> bool {
    dir.join("Seen.toml").exists()
}

/// Find the root of a Seen project by walking up the directory tree
pub fn find_project_root(start_dir: &Path) -> Option<&Path> {
    let mut current = start_dir;
    
    loop {
        if is_seen_project(current) {
            return Some(current);
        }
        
        match current.parent() {
            Some(parent) => current = parent,
            None => return None,
        }
    }
}

/// Ensure a directory exists, creating it if necessary
pub fn ensure_dir_exists(dir: &Path) -> Result<()> {
    if !dir.exists() {
        std::fs::create_dir_all(dir)
            .with_context(|| format!("Failed to create directory: {}", dir.display()))?;
    }
    Ok(())
}

/// Get the relative path from one directory to another
pub fn relative_path(from: &Path, to: &Path) -> Result<std::path::PathBuf> {
    pathdiff::diff_paths(to, from)
        .ok_or_else(|| anyhow::anyhow!("Cannot compute relative path"))
}

/// Check if a file has a specific extension
pub fn has_extension(path: &Path, extension: &str) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map_or(false, |ext| ext == extension)
}

/// Get the current timestamp as a string
pub fn current_timestamp() -> String {
    chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string()
}

/// Validate that a string is a valid identifier
pub fn is_valid_identifier(s: &str) -> bool {
    !s.is_empty() 
        && s.chars().next().unwrap().is_alphabetic()
        && s.chars().all(|c| c.is_alphanumeric() || c == '_')
}

/// Convert a file size in bytes to a human-readable string
pub fn format_file_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    const THRESHOLD: u64 = 1024;
    
    if bytes < THRESHOLD {
        return format!("{} B", bytes);
    }
    
    let mut size = bytes as f64;
    let mut unit_index = 0;
    
    while size >= THRESHOLD as f64 && unit_index < UNITS.len() - 1 {
        size /= THRESHOLD as f64;
        unit_index += 1;
    }
    
    format!("{:.1} {}", size, UNITS[unit_index])
}