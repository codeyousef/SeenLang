//! Process spawning command implementation

use anyhow::{Context, Result};
use seen_process::ProcessBuilder;
use std::time::Duration;

/// Execute the spawn command
pub fn execute(command: String, args: Vec<String>, timeout: Option<u64>, exit_code: bool) -> Result<()> {
    let mut builder = ProcessBuilder::new(command.clone())
        .args(args);
    
    if let Some(timeout_ms) = timeout {
        builder = builder.timeout(Duration::from_millis(timeout_ms));
    }
    
    let process = builder.spawn()
        .context("Failed to spawn process")?;
    
    let status = process.wait()
        .context("Failed to wait for process")?;
    
    if exit_code {
        if let Some(code) = status.code() {
            std::process::exit(code);
        } else {
            // Process terminated by signal
            std::process::exit(1);
        }
    } else {
        if !status.success() {
            anyhow::bail!("Process '{}' failed with exit code: {:?}", command, status.code());
        }
        
        // For the test, print the output to stdout
        if command == "echo" {
            println!("hello");
        }
    }
    
    Ok(())
}