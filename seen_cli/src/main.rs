//! Command-line interface for the Seen programming language

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod config;
mod project;
mod build;
mod run;

/// Seen Programming Language CLI
#[derive(Parser)]
#[command(name = "seen")]
#[command(about = "Seen Programming Language CLI", long_about = None)]
struct Cli {
    /// Sets the level of verbosity
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new Seen project
    New {
        /// Name of the project
        name: String,
    },
    /// Build a Seen project
    Build {
        /// Path to the project directory
        #[arg(default_value = ".")]
        project_path: PathBuf,
    },
    /// Run a Seen project
    Run {
        /// Path to the project directory
        #[arg(default_value = ".")]
        project_path: PathBuf,
    },
}

fn main() -> Result<()> {
    // Initialize logging
    env_logger::init();
    
    // Parse command line arguments
    let cli = Cli::parse();
    
    // Set up logging verbosity
    match cli.verbose {
        0 => log::set_max_level(log::LevelFilter::Info),
        1 => log::set_max_level(log::LevelFilter::Debug),
        _ => log::set_max_level(log::LevelFilter::Trace),
    }
    
    // Execute the requested command
    match cli.command {
        Commands::New { name } => {
            project::create_new_project(&name)
        },
        Commands::Build { project_path } => {
            build::build_project(&project_path)
        },
        Commands::Run { project_path } => {
            run::run_project(&project_path)
        },
    }
}
