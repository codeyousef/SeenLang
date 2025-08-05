//! Configuration types for the Seen CLI

use serde::{Deserialize, Serialize};

/// Build configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildConfig {
    pub target: String,
    pub release: bool,
    pub optimize_for: String,
}

/// Test configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestConfig {
    pub bench: bool,
    pub coverage: bool,
    pub filter: Option<String>,
    pub parallel: bool,
}

/// Format configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatConfig {
    pub check_only: bool,
    pub line_width: u32,
    pub indent_size: u32,
    pub trailing_comma: bool,
}

impl Default for BuildConfig {
    fn default() -> Self {
        Self {
            target: "native".to_string(),
            release: false,
            optimize_for: "debug".to_string(),
        }
    }
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            bench: false,
            coverage: false,
            filter: None,
            parallel: true,
        }
    }
}

impl Default for FormatConfig {
    fn default() -> Self {
        Self {
            check_only: false,
            line_width: 100,
            indent_size: 4,
            trailing_comma: true,
        }
    }
}