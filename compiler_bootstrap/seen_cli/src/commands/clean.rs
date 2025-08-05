//! Clean command implementation

use anyhow::{Result, Context};
use std::path::PathBuf;
use log::info;
use crate::project::Project;

/// Execute the clean command
pub fn execute(manifest_path: Option<PathBuf>) -> Result<()> {
    info!("Cleaning build artifacts...");
    
    let project = Project::find_and_load(manifest_path)?;
    let build_dir = project.build_dir();
    
    if build_dir.exists() {
        std::fs::remove_dir_all(&build_dir)
            .with_context(|| format!("Failed to remove build directory: {}", build_dir.display()))?;
        info!("Removed {}", build_dir.display());
    } else {
        info!("Build directory does not exist - nothing to clean");
    }
    
    info!("Clean completed successfully!");
    Ok(())
}