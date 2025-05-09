use std::fs;
use std::path::Path;
use serde::{Serialize, Deserialize};
use thiserror::Error;

/// Errors that can occur when working with project configurations
#[derive(Error, Debug)]
pub enum ProjectConfigError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("TOML parsing error: {0}")]
    TomlError(#[from] toml::de::Error),
    
    #[error("TOML serialization error: {0}")]
    TomlSerError(#[from] toml::ser::Error),
    
    #[error("Schema validation error: {0}")]
    SchemaError(String),
    
    #[error("Required setting '{0}' is missing")]
    MissingSetting(String),
    
    #[error("Invalid project configuration: {0}")]
    InvalidConfig(String),
}

/// Project metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMetadata {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub authors: Vec<String>,
}

/// Language settings for the project
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageSettings {
    pub keywords: String, // ISO 639-1 language code (e.g., 'en', 'ar')
    #[serde(default)]
    pub allow_mixed: bool,
}

/// Build settings for the project
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildSettings {
    #[serde(default = "default_target")]
    pub target: String,
    #[serde(default = "default_output_dir")]
    pub output_dir: String,
}

fn default_target() -> String {
    "debug".to_string()
}

fn default_output_dir() -> String {
    "target".to_string()
}

/// Represents a Seen project configuration from seen.toml
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub project: ProjectMetadata,
    pub language: LanguageSettings,
    #[serde(default)]
    pub build: Option<BuildSettings>,
    #[serde(default)]
    pub dependencies: std::collections::HashMap<String, String>,
}

impl ProjectConfig {
    /// Load project configuration from a TOML file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, ProjectConfigError> {
        let content = fs::read_to_string(path)?;
        let config: ProjectConfig = toml::from_str(&content)?;
        
        // Validate the configuration
        if config.project.name.is_empty() {
            return Err(ProjectConfigError::MissingSetting("project.name".to_string()));
        }
        
        if config.project.version.is_empty() {
            return Err(ProjectConfigError::MissingSetting("project.version".to_string()));
        }
        
        if config.language.keywords.is_empty() {
            return Err(ProjectConfigError::MissingSetting("language.keywords".to_string()));
        }
        
        Ok(config)
    }
    
    /// Get the active language code for keywords
    pub fn get_active_language(&self) -> &str {
        &self.language.keywords
    }
    
    /// Check if mixed language keywords are allowed
    pub fn allows_mixed_keywords(&self) -> bool {
        self.language.allow_mixed
    }
}

/// Create a new project configuration file
pub fn create_default_project_config<P: AsRef<Path>>(path: P, project_name: &str) -> Result<(), ProjectConfigError> {
    let config = ProjectConfig {
        project: ProjectMetadata {
            name: project_name.to_string(),
            version: "0.1.0".to_string(),
            description: Some("A Seen language project".to_string()),
            authors: vec!["Your Name <your.email@example.com>".to_string()],
        },
        language: LanguageSettings {
            keywords: "en".to_string(), // Default to English
            allow_mixed: false,
        },
        build: Some(BuildSettings {
            target: default_target(),
            output_dir: default_output_dir(),
        }),
        dependencies: std::collections::HashMap::new(),
    };
    
    let toml_string = toml::to_string_pretty(&config)
        .map_err(|e| ProjectConfigError::TomlSerError(e))?;
    
    fs::write(path, toml_string)?;
    
    Ok(())
}
