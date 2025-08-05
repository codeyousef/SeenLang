//! Pipe communication command implementation

use anyhow::{Context, Result};
use std::io::{Read, Write};

/// Execute the pipe command
pub fn execute(mode: String) -> Result<()> {
    match mode.as_str() {
        "producer" => {
            // Producer writes data to stdout
            println!("Data from producer");
            std::io::stdout().flush()?;
            Ok(())
        }
        "consumer" => {
            // Consumer reads from stdin and processes
            let mut buffer = String::new();
            std::io::stdin().read_to_string(&mut buffer)
                .context("Failed to read from stdin")?;
            
            if buffer.contains("Data from producer") {
                println!("Received from producer: {}", buffer.trim());
            } else {
                println!("Consumer received: {}", buffer.trim());
            }
            Ok(())
        }
        _ => {
            anyhow::bail!("Invalid pipe mode: {}. Use 'producer' or 'consumer'", mode);
        }
    }
}