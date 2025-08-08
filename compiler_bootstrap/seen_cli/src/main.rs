//! Seen programming language command-line interface
//! 
//! This is the main entry point for the Seen compiler and toolchain.
//! It provides cargo-like commands for building, running, testing, and managing Seen projects.

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use anyhow::Result;

mod commands;
mod config;
mod project;
mod utils;

use commands::*;

/// Seen programming language compiler and toolchain
#[derive(Parser)]
#[command(name = "seen")]
#[command(about = "The Seen programming language compiler and toolchain")]
#[command(version = env!("CARGO_PKG_VERSION"))]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    
    /// Enable verbose output
    #[arg(short, long, global = true)]
    verbose: bool,
    
    /// Suppress output
    #[arg(short, long, global = true)]
    quiet: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Build the current project
    Build {
        /// Build target (native, wasm, js)
        #[arg(long, default_value = "native")]
        target: String,
        
        /// Build in release mode with optimizations
        #[arg(long)]
        release: bool,
        
        /// Path to the project directory
        #[arg(long)]
        manifest_path: Option<PathBuf>,
    },
    
    /// Run the current project (JIT mode)
    Run {
        /// Arguments to pass to the program
        args: Vec<String>,
        
        /// Build in release mode before running
        #[arg(long)]
        release: bool,
        
        /// Path to the project directory
        #[arg(long)]
        manifest_path: Option<PathBuf>,
    },
    
    /// Check the current project for errors without building
    Check {
        /// Path to the project directory
        #[arg(long)]
        manifest_path: Option<PathBuf>,
        
        /// Watch for changes and re-check automatically
        #[arg(long)]
        watch: bool,
    },
    
    /// Clean build artifacts
    Clean {
        /// Path to the project directory
        #[arg(long)]
        manifest_path: Option<PathBuf>,
    },
    
    /// Run tests
    Test {
        /// Run benchmarks instead of tests
        #[arg(long)]
        bench: bool,
        
        /// Generate code coverage report
        #[arg(long)]
        coverage: bool,
        
        /// Filter to run only tests matching this pattern
        filter: Option<String>,
        
        /// Path to the project directory
        #[arg(long)]
        manifest_path: Option<PathBuf>,
    },
    
    /// Format source code and documents
    Format {
        /// Check formatting without making changes
        #[arg(long)]
        check: bool,
        
        /// Specific files or directories to format
        paths: Vec<PathBuf>,
    },
    
    /// Create a new Seen project
    Init {
        /// Name of the new project
        name: String,
        
        /// Directory to create the project in
        #[arg(long)]
        path: Option<PathBuf>,
        
        /// Language to use (en, ar)
        #[arg(long, default_value = "en")]
        language: String,
    },
    
    /// Add a dependency to the current project
    Add {
        /// Dependency name
        dependency: String,
        
        /// Specific version to add
        #[arg(long)]
        version: Option<String>,
    },
    
    /// Update dependencies
    Update {
        /// Specific dependency to update
        dependency: Option<String>,
    },
    
    /// Start the language server
    Lsp {
        /// Port to bind the language server to
        #[arg(long)]
        port: Option<u16>,
        
        /// Use stdio for communication
        #[arg(long)]
        stdio: bool,
    },
    
    /// Generate documentation
    Doc {
        /// Open documentation in browser after generation
        #[arg(long)]
        open: bool,
        
        /// Include private items in documentation
        #[arg(long)]
        document_private_items: bool,
    },
    
    /// Spawn a subprocess (for self-hosting infrastructure)
    Spawn {
        /// Command to execute
        command: String,
        
        /// Arguments to pass to the command
        args: Vec<String>,
        
        /// Timeout in milliseconds
        #[arg(long)]
        timeout: Option<u64>,
        
        /// Return the exit code
        #[arg(long)]
        exit_code: bool,
    },
    
    /// Pipe data between processes
    Pipe {
        /// Mode: producer or consumer
        mode: String,
    },
    
    /// Environment variable management
    Env {
        /// Action: get or set
        action: String,
        
        /// Variable name
        var_name: String,
        
        /// Variable value (for set action)
        var_value: Option<String>,
    },
    
    /// Print working directory
    Pwd,
    
    /// Change directory
    Cd {
        /// Directory to change to
        path: String,
    },
    
    /// Exit with a specific code
    Exit {
        /// Exit code
        code: i32,
    },
    
    /// Capture process output
    Capture {
        /// What to capture: stdout, stderr, or both
        stream: String,
        
        /// Command to execute
        command: String,
        
        /// Arguments for the command
        args: Vec<String>,
    },
    
    /// Run performance benchmarks
    Benchmark {
        /// Filter benchmarks by name pattern
        filter: Option<String>,
        
        /// Compare against a baseline
        #[arg(long)]
        compare_baseline: Option<String>,
        
        /// Save results with a name
        #[arg(long)]
        save_name: Option<String>,
        
        /// Output results as JSON
        #[arg(long)]
        json: bool,
        
        /// Path to the project directory
        #[arg(long)]
        manifest_path: Option<PathBuf>,
    },
    
    /// RISC-V architecture support tools
    #[command(subcommand)]
    Riscv(RiscvCommands),
}

#[derive(Subcommand)]
enum RiscvCommands {
    /// Display RISC-V architecture support information
    Info,
    
    /// Detect available RISC-V toolchains
    Detect,
    
    /// Cross-compile to RISC-V target
    Compile {
        /// Input LLVM IR file
        input: PathBuf,
        
        /// Output executable path
        #[arg(short, long)]
        output: Option<PathBuf>,
        
        /// Target triple (e.g., riscv64-linux, riscv32-none)
        #[arg(short, long, default_value = "riscv64-linux")]
        target: String,
        
        /// ISA extensions (e.g., "imafdc", "imafdcv")
        #[arg(short, long)]
        extensions: Option<String>,
        
        /// Optimization level (O0, O1, O2, O3, Os)
        #[arg(long)]
        opt_level: Option<String>,
    },
    
    /// Run RISC-V performance benchmarks
    Benchmark {
        /// Target architecture (riscv32, riscv64, riscv64-vector)
        #[arg(short, long, default_value = "riscv64-linux")]
        target: String,
        
        /// Number of iterations
        #[arg(short, long)]
        iterations: Option<usize>,
        
        /// Save results to file
        #[arg(short, long)]
        save: Option<PathBuf>,
    },
    
    /// Display RISC-V ISA extensions reference
    Extensions,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    
    // Initialize logging
    let log_level = if cli.verbose {
        log::LevelFilter::Debug
    } else if cli.quiet {
        log::LevelFilter::Error
    } else {
        log::LevelFilter::Info
    };
    
    env_logger::Builder::from_default_env()
        .filter_level(log_level)
        .format_timestamp(None)
        .format_module_path(false)
        .format_target(false)
        .init();
    
    // Execute the command
    match cli.command {
        Commands::Build { target, release, manifest_path } => {
            build::execute(target, release, manifest_path)
        }
        Commands::Run { args, release, manifest_path } => {
            run::execute(args, release, manifest_path)
        }
        Commands::Check { manifest_path, watch } => {
            check::execute(manifest_path, watch)
        }
        Commands::Clean { manifest_path } => {
            clean::execute(manifest_path)
        }
        Commands::Test { bench, coverage, filter, manifest_path } => {
            test::execute(bench, coverage, filter, manifest_path)
        }
        Commands::Format { check, paths } => {
            format::execute(check, paths)
        }
        Commands::Init { name, path, language } => {
            init::execute(name, path, language)
        }
        Commands::Add { dependency, version } => {
            add::execute(dependency, version)
        }
        Commands::Update { dependency } => {
            update::execute(dependency)
        }
        Commands::Lsp { port, stdio } => {
            lsp::execute(port, stdio)
        }
        Commands::Doc { open, document_private_items } => {
            doc::execute(open, document_private_items)
        }
        Commands::Spawn { command, args, timeout, exit_code } => {
            spawn::execute(command, args, timeout, exit_code)
        }
        Commands::Pipe { mode } => {
            pipe::execute(mode)
        }
        Commands::Env { action, var_name, var_value } => {
            env::execute(action, var_name, var_value)
        }
        Commands::Pwd => {
            pwd::execute()
        }
        Commands::Cd { path } => {
            cd::execute(path)
        }
        Commands::Exit { code } => {
            std::process::exit(code)
        }
        Commands::Capture { stream, command, args } => {
            capture::execute(stream, command, args)
        }
        Commands::Benchmark { filter, compare_baseline, save_name, json, manifest_path } => {
            benchmark::execute(filter, compare_baseline, save_name, json, manifest_path)
        }
        Commands::Riscv(cmd) => {
            match cmd {
                RiscvCommands::Info => riscv::info(),
                RiscvCommands::Detect => riscv::detect_toolchain(),
                RiscvCommands::Compile { input, output, target, extensions, opt_level } => {
                    riscv::compile(input, output, target, extensions, opt_level)
                }
                RiscvCommands::Benchmark { target, iterations, save } => {
                    riscv::benchmark(target, iterations, save)
                }
                RiscvCommands::Extensions => riscv::extensions(),
            }
        }
    }
}