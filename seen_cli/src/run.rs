use anyhow::{bail, Context, Result};
use colored::*;
use std::path::Path;
use std::process::Command;

use crate::build::build_project;
use crate::config::load_project_config;

/// Run a Seen project
pub fn run_project(project_path: &Path) -> Result<()> {
    // First, build the project to ensure it's up-to-date
    build_project(project_path).context("Failed to build project")?;

    println!("{} Running Seen project...", "→".cyan());

    // Load project configuration
    let config =
        load_project_config(project_path).context("Failed to load project configuration")?;

    // Get the path to the executable
    let output_dir = project_path.join(&config.build.output_dir);
    let executable = output_dir.join(&config.name);

    if !executable.exists() {
        bail!("Executable not found: {}", executable.display());
    }

    // Run the executable
    println!(
        "  {} Executing: {}",
        "•".yellow(),
        executable.display().to_string().blue()
    );

    let status = Command::new(&executable)
        .current_dir(project_path)
        .status()
        .with_context(|| format!("Failed to execute: {}", executable.display()))?;

    if !status.success() {
        let exit_code = status.code().unwrap_or(-1);
        bail!("Program exited with code: {}", exit_code);
    }

    println!("{} Program completed successfully", "✓".green().bold());

    Ok(())
}
