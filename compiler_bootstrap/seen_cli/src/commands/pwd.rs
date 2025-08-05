//! Print working directory command implementation

use anyhow::{Context, Result};
use seen_process::working_dir::current_dir;

/// Execute the pwd command
pub fn execute() -> Result<()> {
    let cwd = current_dir()
        .context("Failed to get current directory")?;
    
    println!("{}", cwd.display());
    Ok(())
}