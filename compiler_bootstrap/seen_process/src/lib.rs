//! Process management and system interaction for the Seen compiler
//! Provides self-hosting infrastructure for process spawning, pipes, and environment

pub mod process;
pub mod pipe;
pub mod environment;
pub mod working_dir;

pub use process::{Process, ProcessBuilder, ExitStatus};
pub use pipe::{Pipe, PipeEnd};
pub use environment::{Environment, EnvVar};
pub use working_dir::WorkingDirectory;

use seen_common::SeenResult;

/// Initialize the process management subsystem
pub fn initialize() -> SeenResult<()> {
    // Set up signal handlers for proper process management
    process::setup_signal_handlers()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_process_module_exports() {
        // Verify all modules are properly exported
        let _ = ProcessBuilder::new("test");
        let _ = Environment::new();
        let _ = WorkingDirectory::current();
    }
}