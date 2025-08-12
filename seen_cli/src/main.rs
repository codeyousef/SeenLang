//! Seen Language Command Line Interface
//! 
//! This is the main entry point for the Seen language compiler and toolchain.

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "seen")]
#[command(about = "The Seen Programming Language Compiler")]
#[command(version = "0.1.0")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Compile a Seen source file
    Compile {
        /// Input file to compile
        #[arg(value_name = "FILE")]
        input: PathBuf,
        
        /// Output file path
        #[arg(short, long)]
        output: Option<PathBuf>,
        
        /// Optimization level
        #[arg(short = 'O', long, default_value = "0")]
        opt_level: u8,
    },
    
    /// Run a Seen source file directly
    Run {
        /// Input file to run
        #[arg(value_name = "FILE")]
        input: PathBuf,
        
        /// Arguments to pass to the program
        #[arg(last = true)]
        args: Vec<String>,
    },
    
    /// Check syntax without compiling
    Check {
        /// Input file to check
        #[arg(value_name = "FILE")]
        input: PathBuf,
    },
    
    /// Format Seen source code
    Format {
        /// Input file to format
        #[arg(value_name = "FILE")]
        input: PathBuf,
        
        /// Format in place
        #[arg(short, long)]
        in_place: bool,
    },
    
    /// Run tests
    Test {
        /// Test file or directory
        #[arg(value_name = "PATH")]
        path: Option<PathBuf>,
    },
}

fn main() -> anyhow::Result<()> {
    env_logger::init();
    
    let cli = Cli::parse();
    
    match cli.command {
        Commands::Compile { input, output, opt_level } => {
            println!("Compiling {} with optimization level {}", input.display(), opt_level);
            if let Some(output) = output {
                println!("Output: {}", output.display());
            }
            // Implementation will follow TDD methodology
            todo!("Compile command implementation follows TDD - tests first")
        }
        
        Commands::Run { input, args } => {
            println!("Running {} with args: {:?}", input.display(), args);
            // Implementation will follow TDD methodology
            todo!("Run command implementation follows TDD - tests first")
        }
        
        Commands::Check { input } => {
            println!("Checking syntax of {}", input.display());
            // Implementation will follow TDD methodology
            todo!("Check command implementation follows TDD - tests first")
        }
        
        Commands::Format { input, in_place } => {
            println!("Formatting {} (in-place: {})", input.display(), in_place);
            // Implementation will follow TDD methodology
            todo!("Format command implementation follows TDD - tests first")
        }
        
        Commands::Test { path } => {
            if let Some(path) = path {
                println!("Running tests in {}", path.display());
            } else {
                println!("Running all tests");
            }
            // Implementation will follow TDD methodology
            todo!("Test command implementation follows TDD - tests first")
        }
    }
}