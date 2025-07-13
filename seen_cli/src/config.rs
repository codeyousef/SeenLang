use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Seen project configuration
#[derive(Debug, Serialize, Deserialize)]
pub struct SeenConfig {
    /// Project name
    pub name: String,

    /// Active language for keywords (e.g., "english", "arabic")
    pub language: String,

    /// Version of the Seen language
    #[serde(default = "default_version")]
    pub version: String,

    /// Build configuration
    #[serde(default)]
    pub build: BuildConfig,
}

/// Build configuration
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct BuildConfig {
    /// Output directory for compiled files
    #[serde(default = "default_output_dir")]
    pub output_dir: String,

    /// Optimization level
    #[serde(default = "default_optimization")]
    pub optimization: String,
}

fn default_version() -> String {
    "0.1.0".to_string()
}

fn default_output_dir() -> String {
    "target".to_string()
}

fn default_optimization() -> String {
    "default".to_string()
}

impl SeenConfig {
    /// Create a new default configuration for the given project name
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            language: "english".to_string(),
            version: default_version(),
            build: BuildConfig::default(),
        }
    }

    /// Load configuration from a file
    pub fn load(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read configuration file: {}", path.display()))?;

        let config: SeenConfig = toml::from_str(&content)
            .with_context(|| format!("Failed to parse configuration file: {}", path.display()))?;

        Ok(config)
    }

    /// Save configuration to a file
    pub fn save(&self, path: &Path) -> Result<()> {
        let content = toml::to_string_pretty(self).context("Failed to serialize configuration")?;

        fs::write(path, content)
            .with_context(|| format!("Failed to write configuration file: {}", path.display()))?;

        Ok(())
    }
}

/// Get the path to the seen.toml file in the given project directory
pub fn get_config_path(project_dir: &Path) -> std::path::PathBuf {
    project_dir.join("seen.toml")
}

/// Load the project configuration from the given project directory
pub fn load_project_config(project_dir: &Path) -> Result<SeenConfig> {
    let config_path = get_config_path(project_dir);
    SeenConfig::load(&config_path)
}
