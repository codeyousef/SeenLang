//! Change directory command implementation

use anyhow::{Context, Result};
use seen_process::working_dir::change_dir;

/// Execute the cd command
pub fn execute(path: String) -> Result<()> {
    change_dir(&path)
        .context(format!("Failed to change directory to '{}'", path))?;
    
    println!("Changed directory to: {}", path);
    Ok(())
}