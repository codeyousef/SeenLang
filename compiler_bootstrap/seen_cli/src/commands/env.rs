//! Environment variable command implementation

use anyhow::{Context, Result};
use seen_process::environment::{get_env, set_env};

/// Execute the env command
pub fn execute(action: String, var_name: String, var_value: Option<String>) -> Result<()> {
    match action.as_str() {
        "get" => {
            if let Some(value) = get_env(&var_name) {
                println!("{}", value);
            } else {
                anyhow::bail!("Environment variable '{}' not found", var_name);
            }
            Ok(())
        }
        "set" => {
            if let Some(value) = var_value {
                set_env(&var_name, &value)
                    .context("Failed to set environment variable")?;
                println!("Set {} = {}", var_name, value);
            } else {
                anyhow::bail!("Value required for 'set' action");
            }
            Ok(())
        }
        _ => {
            anyhow::bail!("Invalid env action: {}. Use 'get' or 'set'", action);
        }
    }
}