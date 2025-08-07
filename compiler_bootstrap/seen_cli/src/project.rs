//! Project management utilities

use anyhow::{Result, Context};
use std::path::{Path, PathBuf};
use seen_lexer::LanguageConfig;
use serde::{Deserialize, Serialize};

/// Represents a Seen project
#[derive(Debug, Clone)]
pub struct Project {
    root_dir: PathBuf,
    config: ProjectConfig,
}

/// Project configuration loaded from Seen.toml
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub project: ProjectInfo,
    #[serde(default)]
    pub build: BuildSettings,
    #[serde(default)]
    pub dependencies: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub format: FormatSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub name: String,
    pub version: String,
    pub author: Option<String>,
    #[serde(default = "default_language")]
    pub language: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildSettings {
    #[serde(default = "default_targets")]
    pub targets: Vec<String>,
    #[serde(default = "default_optimize")]
    pub optimize: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatSettings {
    #[serde(default = "default_line_width")]
    pub line_width: u32,
    #[serde(default = "default_indent")]
    pub indent: u32,
    #[serde(default = "default_trailing_comma")]
    pub trailing_comma: bool,
    #[serde(default = "default_document_types")]
    pub document_types: Vec<String>,
}

impl Default for BuildSettings {
    fn default() -> Self {
        Self {
            targets: default_targets(),
            optimize: default_optimize(),
        }
    }
}

impl Default for FormatSettings {
    fn default() -> Self {
        Self {
            line_width: default_line_width(),
            indent: default_indent(),
            trailing_comma: default_trailing_comma(),
            document_types: default_document_types(),
        }
    }
}

fn default_language() -> String { "en".to_string() }
fn default_targets() -> Vec<String> { vec!["native".to_string()] }
fn default_optimize() -> String { "speed".to_string() }
fn default_line_width() -> u32 { 100 }
fn default_indent() -> u32 { 4 }
fn default_trailing_comma() -> bool { true }
fn default_document_types() -> Vec<String> { 
    vec![".seen".to_string(), ".md".to_string(), ".toml".to_string()] 
}

impl Project {
    /// Find and load a project from the given directory or current directory
    pub fn find_and_load(manifest_path: Option<PathBuf>) -> Result<Self> {
        let project_dir = if let Some(path) = manifest_path {
            if path.is_file() {
                path.parent()
                    .ok_or_else(|| anyhow::anyhow!("Invalid manifest path"))?
                    .to_path_buf()
            } else {
                path
            }
        } else {
            std::env::current_dir()
                .context("Failed to get current directory")?
        };
        
        Self::load_from_dir(&project_dir)
    }
    
    /// Load a project from a specific directory
    pub fn load_from_dir(dir: &Path) -> Result<Self> {
        let config_path = dir.join("Seen.toml");
        
        if !config_path.exists() {
            return Err(anyhow::anyhow!(
                "No Seen.toml found in {}. Run 'seen init' to create a new project.",
                dir.display()
            ));
        }
        
        let config_content = std::fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read {}", config_path.display()))?;
        
        let config: ProjectConfig = toml::from_str(&config_content)
            .context("Failed to parse Seen.toml")?;
        
        Ok(Self {
            root_dir: dir.to_path_buf(),
            config,
        })
    }
    
    /// Get the project name
    pub fn name(&self) -> &str {
        &self.config.project.name
    }
    
    /// Get the project version
    pub fn version(&self) -> &str {
        &self.config.project.version
    }
    
    /// Get the project root directory
    pub fn root_dir(&self) -> &Path {
        &self.root_dir
    }
    
    /// Get the build directory
    pub fn build_dir(&self) -> PathBuf {
        self.root_dir.join("target")
    }
    
    /// Get the source directory
    pub fn src_dir(&self) -> PathBuf {
        self.root_dir.join("src")
    }
    
    /// Get the benchmark results directory
    pub fn benchmark_dir(&self) -> PathBuf {
        self.build_dir().join("benchmarks")
    }
    
    /// Get the output path for a specific target and mode
    pub fn output_path(&self, target: &str, release: bool) -> PathBuf {
        let mode = if release { "release" } else { "debug" };
        let mut path = self.build_dir().join(target).join(mode).join(&self.config.project.name);
        
        // Add appropriate extension based on target
        match target {
            "wasm" => {
                path.set_extension("wasm");
            }
            "js" => {
                path.set_extension("js");
            }
            _ => {
                // Native executable - no extension on Unix, .exe on Windows
                #[cfg(windows)]
                path.set_extension("exe");
            }
        }
        
        path
    }
    
    /// Load the language configuration for this project
    pub fn load_language_config(&self) -> Result<LanguageConfig> {
        let lang = &self.config.project.language;
        
        // For English, use the built-in configuration
        if lang == "en" {
            return Ok(LanguageConfig::new_english());
        }
        
        // For Arabic, use the built-in configuration
        if lang == "ar" {
            return Ok(LanguageConfig::new_arabic());
        }
        
        // Try project-local language file first
        let lang_file = self.root_dir
            .join("languages")
            .join(format!("{}.toml", lang));
        
        if lang_file.exists() {
            return LanguageConfig::load_from_file(&lang_file)
                .map_err(|e| anyhow::anyhow!("Failed to load language config: {}", e));
        }
        
        // Try to find language files relative to the executable location
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                // Try various possible locations relative to the executable
                let possible_paths = vec![
                    exe_dir.join("languages").join(format!("{}.toml", lang)),
                    exe_dir.parent().and_then(|p| Some(p.join("languages").join(format!("{}.toml", lang)))).unwrap_or_default(),
                    PathBuf::from("/mnt/d/Projects/Rust/seenlang/languages").join(format!("{}.toml", lang)),
                ];
                
                for path in possible_paths {
                    if path.exists() {
                        return LanguageConfig::load_from_file(&path)
                            .map_err(|e| anyhow::anyhow!("Failed to load language config: {}", e));
                    }
                }
            }
        }
        
        // Fall back to global language files
        let global_lang_file = PathBuf::from("languages").join(format!("{}.toml", lang));
        if global_lang_file.exists() {
            return LanguageConfig::load_from_file(&global_lang_file)
                .map_err(|e| anyhow::anyhow!("Failed to load language config: {}", e));
        }
        
        Err(anyhow::anyhow!(
            "Language configuration not found for '{}'. \
             Looked for: {} and {}",
            lang,
            lang_file.display(),
            global_lang_file.display()
        ))
    }
    
    /// Find all source files in the project
    pub fn find_source_files(&self) -> Result<Vec<PathBuf>> {
        let mut source_files = Vec::new();
        
        // Recursively search for .seen files in the entire project
        for entry in walkdir::WalkDir::new(&self.root_dir)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| {
                // Skip target directories and other build artifacts
                let name = e.file_name().to_string_lossy();
                !name.starts_with("target") && !name.starts_with(".")
            }) {
            let entry = entry.context("Failed to read directory entry")?;
            let path = entry.path();
            
            if path.is_file() {
                if let Some(extension) = path.extension() {
                    if extension == "seen" {
                        source_files.push(path.to_path_buf());
                    }
                }
            }
        }
        
        // Sort for consistent ordering
        source_files.sort();
        Ok(source_files)
    }
    
    /// Get the project configuration
    pub fn config(&self) -> &ProjectConfig {
        &self.config
    }
}