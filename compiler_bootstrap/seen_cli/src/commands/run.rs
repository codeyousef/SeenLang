//! Run command implementation (JIT mode)

use anyhow::{Result, Context};
use std::path::PathBuf;
use std::process::Command;
use log::{info, error};
use crate::project::Project;
use crate::commands::build;

/// Execute the run command
pub fn execute(args: Vec<String>, release: bool, manifest_path: Option<PathBuf>) -> Result<()> {
    info!("Running Seen project in JIT mode...");
    
    let project = Project::find_and_load(manifest_path)?;
    
    // Build the project first
    build::execute("native".to_string(), release, Some(project.root_dir().to_path_buf()))?;
    
    // Get the executable path
    let executable = project.output_path("native", release);
    
    if !executable.exists() {
        error!("Executable not found: {}", executable.display());
        return Err(anyhow::anyhow!("Build output not found"));
    }
    
    info!("Running: {}", executable.display());
    
    // Execute the program
    let mut cmd = Command::new(&executable);
    cmd.args(&args);
    
    let status = cmd.status()
        .with_context(|| format!("Failed to execute {}", executable.display()))?;
    
    if !status.success() {
        let code = status.code().unwrap_or(-1);
        error!("Program exited with code: {}", code);
        std::process::exit(code);
    }
    
    Ok(())
}