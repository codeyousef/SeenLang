use serde::{Deserialize, Serialize};
use std::fs;
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
    pub fn from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self, ProjectConfigError> {
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
pub fn create_default_project_config<P: AsRef<std::path::Path>>(path: P, project_name: &str) -> Result<(), ProjectConfigError> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_create_default_project_config_success() {
        let test_project_name = "my_test_project";
        let test_file_path = "./test_seen.toml";

        // Ensure the file doesn't exist before the test
        let _ = fs::remove_file(test_file_path);

        let result = create_default_project_config(test_file_path, test_project_name);
        assert!(result.is_ok(), "Failed to create default project config: {:?}", result.err());

        // Verify file content
        let content = fs::read_to_string(test_file_path)
            .expect("Failed to read created config file");
        let config: ProjectConfig = toml::from_str(&content)
            .expect("Failed to parse created config file");

        // Assert project metadata
        assert_eq!(config.project.name, test_project_name);
        assert_eq!(config.project.version, "0.1.0");
        assert_eq!(config.project.description, Some("A Seen language project".to_string()));
        assert_eq!(config.project.authors, vec!["Your Name <your.email@example.com>".to_string()]);

        // Assert language settings
        assert_eq!(config.language.keywords, "en");
        assert!(!config.language.allow_mixed);

        // Assert build settings
        let build_settings = config.build.expect("Build settings should exist");
        assert_eq!(build_settings.target, default_target());
        assert_eq!(build_settings.output_dir, default_output_dir());

        // Assert dependencies
        assert!(config.dependencies.is_empty());

        // Clean up the test file
        fs::remove_file(test_file_path).expect("Failed to remove test config file");
    }
}
