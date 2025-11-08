//! Seen Language Command Line Interface
//!
//! This is the main entry point for the Seen language compiler and toolchain.

use clap::{Parser, Subcommand, ValueEnum};
use seen_core::ir::SimdDecisionReason;
use seen_core::parser::{Attribute, AttributeArgument, AttributeValue};
use seen_core::{
    precedence, BinaryOperator, Expression, HardwareProfile, IRGenerator, IROptimizer, IRProgram,
    Interpreter, KeywordManager, Lexer, LexerConfig, MemoryAnalysisResult, MemoryManager,
    MemoryTopologyPreference, OptimizationLevel, Position, Program, SeenError, SeenErrorKind,
    SeenParser, SeenResult, SimdPolicy, TokenType, Type, TypeChecker, UnaryOperator, Value,
    VisibilityPolicy,
};
use seen_cranelift::program_to_clif;
#[cfg(feature = "llvm")]
use seen_ir::llvm_backend::{Avx10Width, CpuFeature, MemoryTopologyHint, SveVectorLength};
use seen_mlir::program_to_mlir;
use seen_shaders::{
    ShaderCompileOptions, ShaderCompiler, ShaderEntryPoint, ShaderError, ShaderTarget,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};
#[cfg(feature = "llvm")]
use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::fs::File;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
#[cfg(feature = "llvm")]
use std::process::Command;
use std::sync::Arc;
#[cfg(feature = "llvm")]
use tempfile::tempdir;
use zip::{write::FileOptions, CompressionMethod};

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum Profile {
    Default,
    Deterministic,
}

impl Default for Profile {
    fn default() -> Self {
        Profile::Default
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum Backend {
    Ir,
    Mlir,
    Clif,
    Llvm,
}

impl Default for Backend {
    fn default() -> Self {
        Backend::Ir
    }
}

impl fmt::Display for Backend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Backend::Ir => "ir",
            Backend::Mlir => "mlir",
            Backend::Clif => "clif",
            Backend::Llvm => "llvm",
        };
        write!(f, "{}", label)
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum CpuFeatureFlagArg {
    Apx,
    Avx10_256,
    Avx10_512,
    Sve128,
    Sve256,
    Sve512,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum MemoryTopologyArg {
    Default,
    CxlNear,
    CxlFar,
}

impl Default for MemoryTopologyArg {
    fn default() -> Self {
        MemoryTopologyArg::Default
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum SimdPolicyArg {
    Off,
    Auto,
    Max,
}

impl Default for SimdPolicyArg {
    fn default() -> Self {
        SimdPolicyArg::Auto
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum ShaderTargetArg {
    Spirv,
    Wgsl,
    Msl,
}

#[derive(Parser)]
#[command(name = "seen")]
#[command(about = "The Seen Programming Language Compiler")]
#[command(version = "0.1.0")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Language for keywords (en, ar, es, fr, etc.)
    #[arg(short = 'l', long, default_value = "en", global = true)]
    language: String,

    /// Execution profile (use `deterministic` for reproducible builds)
    #[arg(long, value_enum, default_value_t = Profile::Default, global = true)]
    profile: Profile,
}

#[derive(Subcommand)]
enum Commands {
    /// Compile a Seen source file
    Build {
        /// Input file to compile
        #[arg(value_name = "FILE")]
        input: PathBuf,

        /// Target triple for cross-compilation (defaults to host target)
        #[arg(long)]
        target: Option<String>,

        /// Target CPU string (passed to LLVM backends)
        #[arg(long = "target-cpu")]
        target_cpu: Option<String>,

        /// Output file path (default: a.ir for IR, a.mlir for MLIR, a.clif for CLIF, a.out for LLVM)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Optimization level
        #[arg(short = 'O', long, default_value = "0")]
        opt_level: u8,

        /// Backend to use: "ir" (textual IR), "mlir" (MLIR export with transforms), "clif" (Cranelift IR prototype), or "llvm" (native binary)
        #[arg(long, default_value_t = Backend::Ir)]
        backend: Backend,

        /// Emit LLVM IR (.ll) instead of linking an executable (only with --backend llvm)
        #[arg(long)]
        emit_ll: bool,

        /// Emit a JavaScript loader for wasm targets (only valid with --backend llvm and --target wasm32-*)
        #[arg(long)]
        wasm_loader: bool,

        /// Bundle wasm outputs (wasm/js/html) into a .zip archive (requires --wasm-loader)
        #[arg(long)]
        bundle: bool,

        /// SIMD policy (auto/off/max)
        #[arg(long = "simd", value_enum, default_value_t = SimdPolicyArg::Auto)]
        simd: SimdPolicyArg,

        /// Produce a shared library (only valid with --backend llvm)
        #[arg(long, conflicts_with = "static_lib")]
        shared: bool,

        /// Produce a static library (only valid with --backend llvm)
        #[arg(long = "static", conflicts_with = "shared")]
        static_lib: bool,

        /// Hardware feature hint (repeatable; only valid with --backend llvm)
        #[arg(long = "cpu-feature", value_enum)]
        cpu_features: Vec<CpuFeatureFlagArg>,

        /// Memory topology hint for the allocator/runtime (only valid with --backend llvm)
        #[arg(long = "memory-topology", value_enum, default_value_t = MemoryTopologyArg::Default)]
        memory_topology: MemoryTopologyArg,

        /// Path to write a JSON SIMD report (applies to all backends)
        #[arg(long = "simd-report")]
        simd_report: Option<PathBuf>,
    },

    /// Generate IR twice and compare SHA-256 hashes (determinism check)
    Determinism {
        /// Input file to evaluate
        #[arg(value_name = "FILE")]
        input: PathBuf,

        /// Optimization level
        #[arg(short = 'O', long, default_value = "0")]
        opt_level: u8,

        /// Backend to hash for determinism
        #[arg(long, default_value_t = Backend::Ir)]
        backend: Backend,
    },

    /// Run a Seen source file directly
    Run {
        /// Input file to run
        #[arg(value_name = "FILE")]
        input: PathBuf,

        /// Optimization level
        #[arg(short = 'O', long, default_value = "0")]
        opt_level: u8,

        /// Target triple for LLVM runs (defaults to host)
        #[arg(long)]
        target: Option<String>,

        /// Target CPU string for LLVM runs
        #[arg(long = "target-cpu")]
        target_cpu: Option<String>,

        /// Backend to execute with: "ir" (interpreter) or "llvm" (native binary)
        #[arg(long, default_value_t = Backend::Ir)]
        backend: Backend,

        /// SIMD policy (auto/off/max)
        #[arg(long = "simd", value_enum, default_value_t = SimdPolicyArg::Auto)]
        simd: SimdPolicyArg,

        /// Hardware feature hint (repeatable; only valid with --backend llvm)
        #[arg(long = "cpu-feature", value_enum)]
        cpu_features: Vec<CpuFeatureFlagArg>,

        /// Memory topology hint for the allocator/runtime (only valid with --backend llvm)
        #[arg(long = "memory-topology", value_enum, default_value_t = MemoryTopologyArg::Default)]
        memory_topology: MemoryTopologyArg,

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

    /// Generate IR and show it
    Ir {
        /// Input file to generate IR for
        #[arg(value_name = "FILE")]
        input: PathBuf,

        /// Optimization level
        #[arg(short = 'O', long, default_value = "0")]
        opt_level: u8,
    },

    /// Emit optimized IR trace for inspection
    Trace {
        /// Input file to trace
        #[arg(value_name = "FILE")]
        input: PathBuf,

        /// Optimization level
        #[arg(short = 'O', long, default_value = "0")]
        opt_level: u8,
    },

    /// Package a directory into a zip archive
    Pkg {
        /// Directory to package
        #[arg(value_name = "DIR")]
        input: PathBuf,

        /// Output archive path (defaults to <dir>.zip)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Validate SPIR-V shaders and emit alternative targets
    Shaders {
        /// Input .spv file or directory containing shaders
        #[arg(value_name = "PATH")]
        input: PathBuf,

        /// Output directory for generated artifacts
        #[arg(long)]
        output: Option<PathBuf>,

        /// Shader targets to emit (comma separated or repeated)
        #[arg(
            long = "target",
            value_enum,
            value_delimiter = ',',
            num_args = 1..,
            default_values = ["wgsl", "msl"]
        )]
        targets: Vec<ShaderTargetArg>,

        /// Only validate inputs without emitting files
        #[arg(long)]
        validate_only: bool,

        /// Recursively traverse directories for .spv files
        #[arg(long)]
        recursive: bool,
    },

    /// Start an interactive REPL
    Repl {
        /// Show AST for each expression
        #[arg(long)]
        show_ast: bool,

        /// Show type information
        #[arg(long)]
        show_types: bool,
    },

    /// Format Seen source code
    Format {
        /// Input file to format
        #[arg(value_name = "FILE")]
        input: PathBuf,

        /// Format in place
        #[arg(short, long)]
        in_place: bool,

        /// Check if the file is already formatted (no changes written)
        #[arg(long, conflicts_with = "in_place")]
        check: bool,
    },

    /// Run tests
    Test {
        /// Test file or directory
        #[arg(value_name = "PATH")]
        path: Option<PathBuf>,
    },

    /// Parse a file and show the AST
    Parse {
        /// Input file to parse
        #[arg(value_name = "FILE")]
        input: PathBuf,

        /// Output format (json, pretty)
        #[arg(short, long, default_value = "pretty")]
        format: String,
    },

    /// Tokenize a file and show tokens
    Lex {
        /// Input file to tokenize
        #[arg(value_name = "FILE")]
        input: PathBuf,
    },
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum BuildLinkMode {
    Executable,
    SharedLibrary,
    StaticLibrary,
}

#[derive(Debug, Deserialize)]
struct ProjectConfig {
    #[serde(default)]
    project: ProjectSection,
    #[serde(default)]
    visibility: Option<VisibilityConfig>,
    #[serde(skip)]
    root_dir: PathBuf,
}

impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            project: ProjectSection::default(),
            visibility: None,
            root_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        }
    }
}

#[derive(Debug, Deserialize, Default)]
struct ProjectSection {
    #[serde(default)]
    visibility: Option<VisibilityMode>,
    #[serde(default)]
    modules: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum VisibilityConfig {
    Table { mode: VisibilityMode },
    Simple(VisibilityMode),
}

impl Default for VisibilityConfig {
    fn default() -> Self {
        VisibilityConfig::Table {
            mode: VisibilityMode::Caps,
        }
    }
}

impl VisibilityConfig {
    fn mode(&self) -> VisibilityMode {
        match self {
            VisibilityConfig::Table { mode } => *mode,
            VisibilityConfig::Simple(mode) => *mode,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum VisibilityMode {
    Caps,
    Explicit,
}

impl Default for VisibilityMode {
    fn default() -> Self {
        VisibilityMode::Caps
    }
}

impl VisibilityMode {
    fn to_policy(self) -> VisibilityPolicy {
        match self {
            VisibilityMode::Caps => VisibilityPolicy::Caps,
            VisibilityMode::Explicit => VisibilityPolicy::Explicit,
        }
    }
}

impl ProjectConfig {
    fn visibility_policy(&self) -> VisibilityPolicy {
        if let Some(vis) = &self.visibility {
            vis.mode().to_policy()
        } else if let Some(mode) = self.project.visibility {
            mode.to_policy()
        } else {
            VisibilityPolicy::Caps
        }
    }

    fn module_paths(&self) -> Vec<PathBuf> {
        self.project
            .modules
            .iter()
            .map(|entry| self.root_dir.join(entry))
            .collect()
    }
}

fn manifest_modules_enabled() -> bool {
    std::env::var("SEEN_ENABLE_MANIFEST_MODULES")
        .ok()
        .map(|value| {
            let lowered = value.trim().to_ascii_lowercase();
            matches!(lowered.as_str(), "1" | "true" | "yes" | "on")
        })
        .unwrap_or(false)
}

fn manifest_module_roots(config: &ProjectConfig) -> Vec<PathBuf> {
    if manifest_modules_enabled() {
        config.module_paths()
    } else {
        Vec::new()
    }
}

fn project_context_for(path: &Path) -> SeenResult<(ProjectConfig, LexerConfig)> {
    let config = load_project_config(path)?;
    let lexer_config = LexerConfig {
        visibility_policy: config.visibility_policy(),
    };
    Ok((config, lexer_config))
}

fn load_project_config(starting_path: &Path) -> SeenResult<ProjectConfig> {
    let mut dir = if starting_path.is_dir() {
        starting_path.to_path_buf()
    } else {
        starting_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
    };

    loop {
        let candidate = dir.join("Seen.toml");
        if candidate.exists() {
            let content = fs::read_to_string(&candidate).map_err(|err| {
                SeenError::new(
                    SeenErrorKind::Io,
                    format!(
                        "Failed to read project config {}: {}",
                        candidate.display(),
                        err
                    ),
                )
            })?;
            let mut config: ProjectConfig = toml::from_str(&content).map_err(|err| {
                SeenError::new(
                    SeenErrorKind::Tooling,
                    format!(
                        "Failed to parse project config {}: {}",
                        candidate.display(),
                        err
                    ),
                )
            })?;
            config.root_dir = dir.clone();
            return Ok(config);
        }

        if !dir.pop() {
            break;
        }
    }

    Ok(ProjectConfig::default())
}

fn main() -> SeenResult<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    let cli = Cli::parse();

    apply_profile(cli.profile)?;

    // Load keyword manager with specified language
    let mut keyword_manager = KeywordManager::new();
    keyword_manager
        .load_from_toml(&cli.language)
        .map_err(|err| {
            SeenError::new(
                SeenErrorKind::Tooling,
                format!("Failed to load language {}: {}", cli.language, err),
            )
        })?;
    keyword_manager
        .switch_language(&cli.language)
        .map_err(|err| {
            SeenError::new(
                SeenErrorKind::Tooling,
                format!("Failed to switch language {}: {}", cli.language, err),
            )
        })?;
    let keyword_manager = Arc::new(keyword_manager);

    match cli.command {
        Some(Commands::Build {
            input,
            output,
            opt_level,
            backend,
            emit_ll,
                 wasm_loader,
                 bundle,
                 target,
                 target_cpu,
                 simd,
            shared,
            static_lib,
                 cpu_features,
                 memory_topology,
                 simd_report,
             }) => {
            if matches!(cli.profile, Profile::Deterministic) {
                if memory_topology != MemoryTopologyArg::Default {
                    return Err(SeenError::new(
                        SeenErrorKind::Tooling,
                        "--memory-topology is locked in --profile deterministic builds".to_string(),
                    ));
                }
                if simd != SimdPolicyArg::Off {
                    return Err(SeenError::new(
                        SeenErrorKind::Tooling,
                        "--simd is locked in --profile deterministic builds".to_string(),
                    ));
                }
                if target_cpu.is_some() {
                    return Err(SeenError::new(
                        SeenErrorKind::Tooling,
                        "--target-cpu is locked in --profile deterministic builds".to_string(),
                    ));
                }
            }

            match backend {
                Backend::Llvm => {
                    if emit_ll && (shared || static_lib) {
                        return Err(SeenError::new(
                            SeenErrorKind::Tooling,
                            "--emit-ll cannot be combined with --shared or --static".to_string(),
                        ));
                    }

                    let mut link_mode = if shared {
                        BuildLinkMode::SharedLibrary
                    } else if static_lib {
                        BuildLinkMode::StaticLibrary
                    } else {
                        BuildLinkMode::Executable
                    };

                    if let Some(triple) = target.as_deref() {
                        if is_android_target(triple)
                            && matches!(link_mode, BuildLinkMode::Executable)
                        {
                            eprintln!(
                            "info: defaulting to --shared for Android target {}; pass --shared or --static explicitly to override",
                            triple
                        );
                            link_mode = BuildLinkMode::SharedLibrary;
                        }
                    }

                    compile_file_llvm(
                        &input,
                        output.as_ref(),
                        opt_level,
                        emit_ll,
                        link_mode,
                        keyword_manager,
                        target.as_deref(),
                        wasm_loader,
                        bundle,
                        cpu_features.as_slice(),
                        memory_topology,
                        simd,
                        target_cpu.as_deref(),
                        simd_report.as_ref(),
                        cli.profile,
                        false,
                    )?;
                }
                Backend::Ir => {
                    if !cpu_features.is_empty() {
                        return Err(hardware_flag_backend_error());
                    }
                    if target_cpu.is_some() {
                        return Err(SeenError::new(
                            SeenErrorKind::Tooling,
                            "--target-cpu is not supported for the IR text backend".to_string(),
                        ));
                    }
                    if wasm_loader {
                        return Err(SeenError::new(
                            SeenErrorKind::Tooling,
                            "--wasm-loader requires the LLVM backend".to_string(),
                        ));
                    }
                    if bundle {
                        return Err(SeenError::new(
                            SeenErrorKind::Tooling,
                            "--bundle requires the LLVM backend".to_string(),
                        ));
                    }
                    if let Some(target) = target {
                        eprintln!(
                            "warning: --target={} is ignored by the IR text backend",
                            target
                        );
                    }
                    if shared || static_lib {
                        return Err(SeenError::new(
                            SeenErrorKind::Tooling,
                            "--shared/--static require the LLVM backend".to_string(),
                        ));
                    }
                    compile_file_ir(
                        &input,
                        output.as_ref(),
                        opt_level,
                        keyword_manager,
                        cpu_features.as_slice(),
                        memory_topology,
                        simd,
                        target_cpu.as_deref(),
                        simd_report.as_ref(),
                    )?;
                }
                Backend::Mlir => {
                    if wasm_loader {
                        return Err(SeenError::new(
                            SeenErrorKind::Tooling,
                            "--wasm-loader requires the LLVM backend".to_string(),
                        ));
                    }
                    if bundle {
                        return Err(SeenError::new(
                            SeenErrorKind::Tooling,
                            "--bundle requires the LLVM backend".to_string(),
                        ));
                    }
                    if let Some(target) = target {
                        eprintln!(
                            "warning: --target={} is ignored by the experimental MLIR backend",
                            target
                        );
                    }
                    if let Some(cpu) = target_cpu.as_deref() {
                        eprintln!(
                            "warning: --target-cpu={} is ignored by the experimental MLIR backend",
                            cpu
                        );
                    }
                    if emit_ll || shared || static_lib {
                        return Err(SeenError::new(
                            SeenErrorKind::Tooling,
                            "--emit-ll/--shared/--static are not supported with the MLIR backend"
                                .to_string(),
                        ));
                    }
                    compile_file_mlir(
                        &input,
                        output.as_ref(),
                        opt_level,
                        keyword_manager,
                        cpu_features.as_slice(),
                        memory_topology,
                        simd,
                        target_cpu.as_deref(),
                        simd_report.as_ref(),
                    )?;
                }
                Backend::Clif => {
                    if wasm_loader || bundle {
                        return Err(SeenError::new(
                            SeenErrorKind::Tooling,
                            "--wasm-loader/--bundle require the LLVM backend".to_string(),
                        ));
                    }
                    if let Some(target) = target {
                        eprintln!(
                            "warning: --target={} is ignored by the Cranelift IR backend",
                            target
                        );
                    }
                    if let Some(cpu) = target_cpu.as_deref() {
                        eprintln!(
                            "warning: --target-cpu={} is ignored by the Cranelift IR backend",
                            cpu
                        );
                    }
                    if emit_ll || shared || static_lib {
                        return Err(SeenError::new(
                        SeenErrorKind::Tooling,
                        "--emit-ll/--shared/--static are not supported with the Cranelift backend"
                            .to_string(),
                    ));
                    }
                    compile_file_clif(
                        &input,
                        output.as_ref(),
                        opt_level,
                        keyword_manager,
                        cpu_features.as_slice(),
                        memory_topology,
                        simd,
                        target_cpu.as_deref(),
                        simd_report.as_ref(),
                    )?;
                }
            }
        }

        Some(Commands::Run {
                 input,
                 args,
                 backend,
                 opt_level,
                 target,
                 target_cpu,
                 simd,
                 cpu_features,
                 memory_topology,
             }) => {
            if matches!(cli.profile, Profile::Deterministic)
                && (!cpu_features.is_empty()
                || memory_topology != MemoryTopologyArg::Default
                || simd != SimdPolicyArg::Off
                || target_cpu.is_some())
            {
                return Err(SeenError::new(
                    SeenErrorKind::Tooling,
                    "--cpu-feature/--memory-topology/--simd/--target-cpu are locked in --profile deterministic builds"
                        .to_string(),
                ));
            }
            match backend {
                Backend::Ir => {
                    if !cpu_features.is_empty() {
                        return Err(hardware_flag_backend_error());
                    }
                    if memory_topology != MemoryTopologyArg::Default {
                        eprintln!(
                            "warning: --memory-topology is ignored by the interpreter backend; rerun with --backend llvm to honor it"
                        );
                    }
                    if target_cpu.is_some() {
                        eprintln!(
                            "warning: --target-cpu is ignored by the interpreter backend; rerun with --backend llvm to honor it"
                        );
                    }
                    if !matches!(simd, SimdPolicyArg::Auto) {
                        eprintln!(
                            "warning: --simd is ignored by the interpreter backend; rerun with --backend llvm to honor it"
                        );
                    }
                    run_file(&input, &args, keyword_manager)?
                }
                Backend::Llvm => {
                    #[cfg(feature = "llvm")]
                    {
                        run_file_llvm(
                            &input,
                            &args,
                            opt_level,
                            target.as_deref(),
                            target_cpu.as_deref(),
                            keyword_manager,
                            cpu_features.as_slice(),
                            memory_topology,
                            simd,
                            cli.profile,
                        )?;
                    }
                    #[cfg(not(feature = "llvm"))]
                    {
                        let _ = (
                            opt_level,
                            target,
                            target_cpu,
                            cpu_features,
                            memory_topology,
                            simd,
                        );
                        return Err(SeenError::new(
                        SeenErrorKind::Tooling,
                        "LLVM backend disabled at build time; rebuild seen_cli with --features llvm".to_string(),
                    ));
                    }
                }
                Backend::Mlir => {
                    return Err(SeenError::new(
                        SeenErrorKind::Tooling,
                        "MLIR backend is not supported for `seen run`".to_string(),
                    ));
                }
                Backend::Clif => {
                    return Err(SeenError::new(
                        SeenErrorKind::Tooling,
                        "Cranelift backend is not supported for `seen run`".to_string(),
                    ));
                }
            }
        }

        Some(Commands::Check { input }) => {
            check_file(&input, keyword_manager)?;
        }

        Some(Commands::Ir { input, opt_level }) => {
            generate_ir(&input, opt_level, keyword_manager)?;
        }
        Some(Commands::Trace { input, opt_level }) => {
            trace_ir(&input, opt_level, keyword_manager)?;
        }
        Some(Commands::Pkg { input, output }) => {
            package_directory(&input, output.as_deref())?;
        }

        Some(Commands::Shaders {
                 input,
                 output,
                 targets,
                 validate_only,
                 recursive,
             }) => {
            build_shaders_command(
                &input,
                output.as_deref(),
                targets.as_slice(),
                validate_only,
                recursive,
            )?;
        }

        Some(Commands::Repl {
            show_ast,
            show_types,
        }) => {
            run_repl(keyword_manager, show_ast, show_types)?;
        }

        Some(Commands::Format {
                 input,
                 in_place,
                 check,
             }) => {
            format_file(&input, in_place, check)?;
        }

        Some(Commands::Test { path }) => {
            run_tests(path.as_deref())?;
        }

        Some(Commands::Parse { input, format }) => {
            parse_file(&input, &format, keyword_manager)?;
        }

        Some(Commands::Determinism {
            input,
            opt_level,
            backend,
        }) => {
            determinism_check(&input, opt_level, backend, keyword_manager)?;
        }

        Some(Commands::Lex { input }) => {
            lex_file(&input, keyword_manager)?;
        }

        None => {
            // If no command is given, start REPL
            println!("Seen Language v0.1.0 - Interactive Mode");
            println!("Type 'exit' or Ctrl+D to quit\n");
            run_repl(keyword_manager, false, false)?;
        }
    }

    Ok(())
}

fn apply_profile(profile: Profile) -> SeenResult<()> {
    match profile {
        Profile::Default => {
            std::env::set_var("SEEN_PROFILE", "default");
            std::env::remove_var("SEEN_DETERMINISTIC");
            Ok(())
        }
        Profile::Deterministic => {
            let cwd = std::env::current_dir().map_err(|err| {
                SeenError::new(
                    SeenErrorKind::Tooling,
                    format!("Failed to resolve current directory: {}", err),
                )
            })?;
            let deterministic_root = cwd.join(".seen").join("tmp");
            fs::create_dir_all(&deterministic_root).map_err(|err| {
                SeenError::new(
                    SeenErrorKind::Tooling,
                    format!(
                        "Failed to create deterministic temp directory {}: {}",
                        deterministic_root.display(),
                        err
                    ),
                )
            })?;
            let deterministic_root = deterministic_root
                .canonicalize()
                .unwrap_or(deterministic_root);
            let temp_dir = deterministic_root.to_string_lossy().to_string();
            for var in ["TMPDIR", "TEMP", "TMP"] {
                std::env::set_var(var, &temp_dir);
            }
            std::env::set_var("SEEN_PROFILE", "deterministic");
            std::env::set_var("SEEN_DETERMINISTIC", "1");
            std::env::set_var("SOURCE_DATE_EPOCH", "0");
            Ok(())
        }
    }
}

fn generate_optimized_ir(
    input: &Path,
    opt_level: u8,
    keyword_manager: Arc<KeywordManager>,
    cpu_features: &[CpuFeatureFlagArg],
    memory_topology: MemoryTopologyArg,
    simd_policy: SimdPolicyArg,
    target_cpu: Option<&str>,
) -> SeenResult<IRProgram> {
    println!(
        "Compiling {} with optimization level {}",
        input.display(),
        opt_level
    );

    let source = fs::read_to_string(input).map_err(|err| {
        SeenError::new(
            SeenErrorKind::Io,
            format!("Failed to read source file {}: {}", input.display(), err),
        )
    })?;

    let (project_config, lexer_config) = project_context_for(input)?;
    let visibility_policy = lexer_config.visibility_policy;
    let manifest_modules = manifest_module_roots(&project_config);

    let lexer = Lexer::with_config(source.clone(), keyword_manager.clone(), lexer_config);
    let mut parser = SeenParser::new_with_visibility(lexer, visibility_policy);
    let ast_parsed = parser
        .parse_program()
        .map_err(|err| annotate_parser_error(err.into(), input))?;

    let ast = bundle_imports(
        ast_parsed,
        input,
        keyword_manager.clone(),
        visibility_policy,
        &manifest_modules,
    )?;

    let bootstrap_mode = false;

    let mut type_checker = TypeChecker::new();
    let type_result = type_checker.check_program(&ast);
    if !type_result.errors.is_empty() {
        for error in &type_result.errors {
            eprintln!("Type error: {}", error);
        }
        if !bootstrap_mode {
            let primary = type_result
                .errors
                .first()
                .cloned()
                .map(SeenError::from)
                .unwrap_or_else(|| {
                    SeenError::new(
                        SeenErrorKind::TypeChecker,
                        format!(
                            "Type checking failed with {} errors",
                            type_result.errors.len()
                        ),
                    )
                });
            return Err(primary);
        } else {
            eprintln!(
                "[bootstrap] Proceeding despite {} type errors",
                type_result.errors.len()
            );
        }
    }

    let topology_pref = convert_memory_topology_pref(memory_topology);
    let mut memory_manager = MemoryManager::with_topology_hint(topology_pref);
    let memory_result = memory_manager.analyze_program(&ast);
    if memory_result.has_errors() {
        for error in memory_result.get_errors() {
            eprintln!("Memory error: {}", error);
        }
        if !bootstrap_mode {
            let primary = memory_result
                .get_errors()
                .first()
                .cloned()
                .map(SeenError::from)
                .unwrap_or_else(|| {
                    SeenError::new(
                        SeenErrorKind::Memory,
                        format!(
                            "Memory analysis failed with {} errors",
                            memory_result.get_errors().len()
                        ),
                    )
                });
            return Err(primary);
        } else {
            eprintln!(
                "[bootstrap] Proceeding despite {} memory analysis errors",
                memory_result.get_errors().len()
            );
        }
    }

    for optimization in memory_result.get_optimizations() {
        println!("Memory optimization: {}", optimization);
    }

    if memory_topology != MemoryTopologyArg::Default {
        log_memory_topology_summary(memory_topology, &memory_result);
    }

    let mut ir_generator = IRGenerator::new();
    let ir_program = ir_generator.generate(&ast).map_err(SeenError::from)?;

    println!("Generated IR with {} modules", ir_program.modules.len());

    let optimization_level = OptimizationLevel::from_level(opt_level);
    let mut optimizer = IROptimizer::new(optimization_level);
    let mut optimized_ir = ir_program;
    optimized_ir.hardware_profile = build_hardware_profile(cpu_features, simd_policy, target_cpu);
    optimizer
        .optimize_program(&mut optimized_ir)
        .map_err(SeenError::from)?;

    println!("Applied optimization level {}", opt_level);

    Ok(optimized_ir)
}

fn compile_file_ir(
    input: &Path,
    output: Option<&PathBuf>,
    opt_level: u8,
    keyword_manager: Arc<KeywordManager>,
    cpu_features: &[CpuFeatureFlagArg],
    memory_topology: MemoryTopologyArg,
    simd_policy: SimdPolicyArg,
    target_cpu: Option<&str>,
    simd_report: Option<&PathBuf>,
) -> SeenResult<()> {
    let optimized_ir = generate_optimized_ir(
        input,
        opt_level,
        keyword_manager,
        cpu_features,
        memory_topology,
        simd_policy,
        target_cpu,
    )?;
    maybe_emit_simd_report(simd_report, &optimized_ir, simd_policy)?;
    let ir_text = format!("{}", optimized_ir);
    let default_output = PathBuf::from("a.ir");
    let output_path = output.unwrap_or(&default_output);
    fs::write(output_path, ir_text).map_err(|err| {
        SeenError::new(
            SeenErrorKind::Io,
            format!(
                "Failed to write output file {}: {}",
                output_path.display(),
                err
            ),
        )
    })?;
    println!("Generated IR: {}", output_path.display());
    println!("Build completed (Rust backend: IR)");

    Ok(())
}

fn compile_file_mlir(
    input: &Path,
    output: Option<&PathBuf>,
    opt_level: u8,
    keyword_manager: Arc<KeywordManager>,
    cpu_features: &[CpuFeatureFlagArg],
    memory_topology: MemoryTopologyArg,
    simd_policy: SimdPolicyArg,
    target_cpu: Option<&str>,
    simd_report: Option<&PathBuf>,
) -> SeenResult<()> {
    let optimized_ir = generate_optimized_ir(
        input,
        opt_level,
        keyword_manager,
        cpu_features,
        memory_topology,
        simd_policy,
        target_cpu,
    )?;
    maybe_emit_simd_report(simd_report, &optimized_ir, simd_policy)?;
    let mlir_text = program_to_mlir(&optimized_ir);
    let default_output = PathBuf::from("a.mlir");
    let output_path = output.unwrap_or(&default_output);
    fs::write(output_path, mlir_text).map_err(|err| {
        SeenError::new(
            SeenErrorKind::Io,
            format!(
                "Failed to write MLIR output file {}: {}",
                output_path.display(),
                err
            ),
        )
    })?;
    println!("Generated MLIR: {}", output_path.display());
    println!("Build completed (Rust backend: MLIR)");
    Ok(())
}

fn compile_file_clif(
    input: &Path,
    output: Option<&PathBuf>,
    opt_level: u8,
    keyword_manager: Arc<KeywordManager>,
    cpu_features: &[CpuFeatureFlagArg],
    memory_topology: MemoryTopologyArg,
    simd_policy: SimdPolicyArg,
    target_cpu: Option<&str>,
    simd_report: Option<&PathBuf>,
) -> SeenResult<()> {
    let optimized_ir = generate_optimized_ir(
        input,
        opt_level,
        keyword_manager,
        cpu_features,
        memory_topology,
        simd_policy,
        target_cpu,
    )?;
    maybe_emit_simd_report(simd_report, &optimized_ir, simd_policy)?;
    let clif_text = program_to_clif(&optimized_ir);
    let default_output = PathBuf::from("a.clif");
    let output_path = output.unwrap_or(&default_output);
    fs::write(output_path, clif_text).map_err(|err| {
        SeenError::new(
            SeenErrorKind::Io,
            format!(
                "Failed to write CLIF output file {}: {}",
                output_path.display(),
                err
            ),
        )
    })?;
    println!("Generated Cranelift IR: {}", output_path.display());
    println!("Build completed (Rust backend: CLIF)");
    Ok(())
}

fn hardware_flag_backend_error() -> SeenError {
    SeenError::new(
        SeenErrorKind::Tooling,
        "--cpu-feature is not supported for the IR text backend".to_string(),
    )
}

#[derive(Serialize)]
struct SimdFunctionReport {
    module: String,
    function: String,
    policy: String,
    mode: String,
    reason: String,
    hot_loops: usize,
    arithmetic_ops: usize,
    memory_ops: usize,
    vector_ops: usize,
    estimated_speedup: f32,
    vector_width_bits: Option<u32>,
}

fn maybe_emit_simd_report(
    report_path: Option<&PathBuf>,
    program: &IRProgram,
    policy: SimdPolicyArg,
) -> SeenResult<()> {
    let path = match report_path {
        Some(p) => p,
        None => return Ok(()),
    };
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|err| {
                SeenError::new(
                    SeenErrorKind::Io,
                    format!(
                        "Failed to create SIMD report directory {}: {}",
                        parent.display(),
                        err
                    ),
                )
            })?;
        }
    }

    let mut functions = Vec::new();
    for module in program.modules.iter() {
        for function in module.functions_iter() {
            let metadata = &function.simd_metadata;
            let should_emit = metadata.arithmetic_ops > 0
                || metadata.hot_loops > 0
                || metadata.existing_vector_ops > 0
                || !matches!(metadata.reason, SimdDecisionReason::Unknown);
            if !should_emit {
                continue;
            }
            functions.push(SimdFunctionReport {
                module: module.name.clone(),
                function: function.name.clone(),
                policy: metadata.policy.as_str().to_string(),
                mode: metadata.mode.as_str().to_string(),
                reason: metadata.reason.as_str().to_string(),
                hot_loops: metadata.hot_loops,
                arithmetic_ops: metadata.arithmetic_ops,
                memory_ops: metadata.memory_ops,
                vector_ops: metadata.existing_vector_ops,
                estimated_speedup: metadata.estimated_speedup,
                vector_width_bits: metadata.vector_width_bits,
            });
        }
    }

    #[derive(Serialize)]
    struct Report<'a> {
        requested_policy: &'a str,
        functions: Vec<SimdFunctionReport>,
    }

    let report = Report {
        requested_policy: simd_policy_label(policy),
        functions,
    };

    let data = serde_json::to_vec_pretty(&report).map_err(|err| {
        SeenError::new(
            SeenErrorKind::Tooling,
            format!("Failed to serialize SIMD report: {}", err),
        )
    })?;
    fs::write(path, data).map_err(|err| {
        SeenError::new(
            SeenErrorKind::Io,
            format!("Failed to write SIMD report {}: {}", path.display(), err),
        )
    })?;
    println!("Wrote SIMD report to {}", path.display());
    Ok(())
}

#[cfg(feature = "llvm")]
fn convert_cpu_features(flags: &[CpuFeatureFlagArg]) -> Vec<CpuFeature> {
    flags
        .iter()
        .map(|flag| match flag {
            CpuFeatureFlagArg::Apx => CpuFeature::IntelApx,
            CpuFeatureFlagArg::Avx10_256 => CpuFeature::IntelAvx10(Avx10Width::Bits256),
            CpuFeatureFlagArg::Avx10_512 => CpuFeature::IntelAvx10(Avx10Width::Bits512),
            CpuFeatureFlagArg::Sve128 => CpuFeature::ArmSve(SveVectorLength::Bits128),
            CpuFeatureFlagArg::Sve256 => CpuFeature::ArmSve(SveVectorLength::Bits256),
            CpuFeatureFlagArg::Sve512 => CpuFeature::ArmSve(SveVectorLength::Bits512),
        })
        .collect()
}

#[cfg(feature = "llvm")]
fn describe_cpu_features(flags: &[CpuFeatureFlagArg]) -> String {
    if flags.is_empty() {
        return "baseline".to_string();
    }
    flags
        .iter()
        .map(cpu_feature_label)
        .collect::<Vec<_>>()
        .join(", ")
}

fn vector_bits_for_feature(flag: &CpuFeatureFlagArg) -> Option<u32> {
    match flag {
        CpuFeatureFlagArg::Avx10_512 => Some(512),
        CpuFeatureFlagArg::Avx10_256 => Some(256),
        CpuFeatureFlagArg::Sve128 => Some(128),
        CpuFeatureFlagArg::Sve256 => Some(256),
        CpuFeatureFlagArg::Sve512 => Some(512),
        _ => None,
    }
}

fn convert_simd_policy(arg: SimdPolicyArg) -> SimdPolicy {
    match arg {
        SimdPolicyArg::Auto => SimdPolicy::Auto,
        SimdPolicyArg::Off => SimdPolicy::Off,
        SimdPolicyArg::Max => SimdPolicy::Max,
    }
}

fn simd_policy_label(arg: SimdPolicyArg) -> &'static str {
    match arg {
        SimdPolicyArg::Auto => "auto",
        SimdPolicyArg::Off => "off",
        SimdPolicyArg::Max => "max",
    }
}

fn build_hardware_profile(
    flags: &[CpuFeatureFlagArg],
    simd_policy: SimdPolicyArg,
    target_cpu: Option<&str>,
) -> HardwareProfile {
    let mut profile = HardwareProfile::default();
    if flags.is_empty() {
        profile.simd_policy = convert_simd_policy(simd_policy);
        profile.target_cpu = target_cpu.map(|s| s.to_string());
        return profile;
    }

    profile.cpu_features = flags
        .iter()
        .map(|flag| cpu_feature_label(flag).to_string())
        .collect();
    profile.apx_enabled = flags
        .iter()
        .any(|flag| matches!(flag, CpuFeatureFlagArg::Apx));
    profile.sve_enabled = flags.iter().any(|flag| {
        matches!(
            flag,
            CpuFeatureFlagArg::Sve128 | CpuFeatureFlagArg::Sve256 | CpuFeatureFlagArg::Sve512
        )
    });
    for flag in flags {
        if let Some(bits) = vector_bits_for_feature(flag) {
            profile.max_vector_bits = Some(
                profile
                    .max_vector_bits
                    .map_or(bits, |current| current.max(bits)),
            );
        }
    }
    profile.simd_policy = convert_simd_policy(simd_policy);
    profile.target_cpu = target_cpu.map(|s| s.to_string());
    profile
}

#[cfg(feature = "llvm")]
fn convert_memory_topology_hint(arg: MemoryTopologyArg) -> MemoryTopologyHint {
    match arg {
        MemoryTopologyArg::Default => MemoryTopologyHint::Default,
        MemoryTopologyArg::CxlNear => MemoryTopologyHint::CxlNear,
        MemoryTopologyArg::CxlFar => MemoryTopologyHint::CxlFar,
    }
}

fn convert_memory_topology_pref(arg: MemoryTopologyArg) -> MemoryTopologyPreference {
    match arg {
        MemoryTopologyArg::Default => MemoryTopologyPreference::Default,
        MemoryTopologyArg::CxlNear => MemoryTopologyPreference::CxlNear,
        MemoryTopologyArg::CxlFar => MemoryTopologyPreference::CxlFar,
    }
}

fn log_memory_topology_summary(arg: MemoryTopologyArg, result: &MemoryAnalysisResult) {
    use seen_core::MemoryTier;
    let mut host = 0usize;
    let mut near = 0usize;
    let mut far = 0usize;
    for region in result.region_manager.regions() {
        match region.memory_tier() {
            MemoryTier::Host => host += 1,
            MemoryTier::CxlNear => near += 1,
            MemoryTier::CxlFar => far += 1,
        }
    }

    println!(
        "Memory topology [{}]: host={} near={} far={}",
        memory_topology_label(arg),
        host,
        near,
        far
    );
}

fn memory_topology_label(arg: MemoryTopologyArg) -> &'static str {
    match arg {
        MemoryTopologyArg::Default => "default",
        MemoryTopologyArg::CxlNear => "cxl-near",
        MemoryTopologyArg::CxlFar => "cxl-far",
    }
}

fn cpu_feature_label(flag: &CpuFeatureFlagArg) -> &'static str {
    match flag {
        CpuFeatureFlagArg::Apx => "apx",
        CpuFeatureFlagArg::Avx10_256 => "avx10-256",
        CpuFeatureFlagArg::Avx10_512 => "avx10-512",
        CpuFeatureFlagArg::Sve128 => "sve128",
        CpuFeatureFlagArg::Sve256 => "sve256",
        CpuFeatureFlagArg::Sve512 => "sve512",
    }
}

fn compile_file_llvm(
    input: &Path,
    output: Option<&PathBuf>,
    opt_level: u8,
    emit_ll: bool,
    link_mode: BuildLinkMode,
    keyword_manager: Arc<KeywordManager>,
    target_triple: Option<&str>,
    wasm_loader: bool,
    bundle: bool,
    cpu_features: &[CpuFeatureFlagArg],
    memory_topology: MemoryTopologyArg,
    simd_policy: SimdPolicyArg,
    target_cpu: Option<&str>,
    simd_report: Option<&PathBuf>,
    profile: Profile,
    cli_mode: bool,
) -> SeenResult<()> {
    println!(
        "Compiling {} with optimization level {} (LLVM)",
        input.display(),
        opt_level
    );

    let triple_str = target_triple.unwrap_or("");
    let bundling_wasm = bundle && is_wasm_target(triple_str);
    let bundling_android = bundle && is_android_target(triple_str);

    if wasm_loader {
        if !is_wasm_target(triple_str) {
            return Err(SeenError::new(
                SeenErrorKind::Tooling,
                "--wasm-loader requires --target wasm32-...".to_string(),
            ));
        }
        if emit_ll {
            return Err(SeenError::new(
                SeenErrorKind::Tooling,
                "--wasm-loader cannot be combined with --emit-ll".to_string(),
            ));
        }
        if matches!(
            link_mode,
            BuildLinkMode::SharedLibrary | BuildLinkMode::StaticLibrary
        ) {
            return Err(SeenError::new(
                SeenErrorKind::Tooling,
                "--wasm-loader only supports executable outputs".to_string(),
            ));
        }
    }

    if bundle {
        if bundling_wasm {
            if !wasm_loader {
                return Err(SeenError::new(
                    SeenErrorKind::Tooling,
                    "--bundle requires --wasm-loader".to_string(),
                ));
            }
            if emit_ll {
                return Err(SeenError::new(
                    SeenErrorKind::Tooling,
                    "--bundle cannot be combined with --emit-ll".to_string(),
                ));
            }
        } else if bundling_android {
            if emit_ll {
                return Err(SeenError::new(
                    SeenErrorKind::Tooling,
                    "Android bundling does not support --emit-ll".to_string(),
                ));
            }
            if !matches!(link_mode, BuildLinkMode::SharedLibrary) {
                return Err(SeenError::new(
                    SeenErrorKind::Tooling,
                    "Android bundling requires a shared library output".to_string(),
                ));
            }
        } else {
            return Err(SeenError::new(
                SeenErrorKind::Tooling,
                "--bundle currently supports wasm32 and android cross builds".to_string(),
            ));
        }
    }

    if let Some(triple) = target_triple {
        if is_android_target(triple) {
            let ndk_home = std::env::var("ANDROID_NDK_HOME").ok();
            let ndk_home = ndk_home.as_deref().map(str::trim).filter(|v| !v.is_empty());
            if ndk_home.is_none() {
                return Err(SeenError::new(
                    SeenErrorKind::Tooling,
                    "ANDROID_NDK_HOME must be set to cross-compile for Android targets".to_string(),
                ));
            }
        }
    }

    if matches!(profile, Profile::Deterministic)
        && (!cpu_features.is_empty()
        || memory_topology != MemoryTopologyArg::Default
        || simd_policy != SimdPolicyArg::Off
        || target_cpu.is_some())
    {
        return Err(SeenError::new(
            SeenErrorKind::Tooling,
            "--cpu-feature/--memory-topology/--simd/--target-cpu are locked in --profile deterministic builds"
                .to_string(),
        ));
    }

    let source = fs::read_to_string(input).map_err(|err| {
        SeenError::new(
            SeenErrorKind::Io,
            format!("Failed to read source file {}: {}", input.display(), err),
        )
    })?;

    let (project_config, lexer_config) = project_context_for(input)?;
    let visibility_policy = lexer_config.visibility_policy;
    let manifest_modules = manifest_module_roots(&project_config);
    // Lex + parse
    let lexer = Lexer::with_config(source.clone(), keyword_manager.clone(), lexer_config);
    let mut parser = SeenParser::new_with_visibility(lexer, visibility_policy);
    let ast_parsed = parser.parse_program().map_err(SeenError::from)?;

    // Bundle imports
    let ast = bundle_imports(
        ast_parsed,
        input,
        keyword_manager.clone(),
        visibility_policy,
        &manifest_modules,
    )?;

    // Type check
    let mut type_checker = TypeChecker::new();
    let type_result = type_checker.check_program(&ast);
    if !type_result.errors.is_empty() {
        for error in &type_result.errors {
            eprintln!("Type error: {}", error);
        }
        let primary = type_result
            .errors
            .first()
            .cloned()
            .map(SeenError::from)
            .unwrap_or_else(|| {
                SeenError::new(
                    SeenErrorKind::TypeChecker,
                    format!(
                        "Type checking failed with {} errors",
                        type_result.errors.len()
                    ),
                )
            });
        return Err(primary);
    }

    // Generate + optimize IR
    let mut ir_generator = IRGenerator::new();
    let ir_program = ir_generator.generate(&ast).map_err(SeenError::from)?;
    let optimization_level = OptimizationLevel::from_level(opt_level);
    let mut optimizer = IROptimizer::new(optimization_level);
    let mut optimized_ir = ir_program;
    optimized_ir.hardware_profile = build_hardware_profile(cpu_features, simd_policy, target_cpu);
    optimizer
        .optimize_program(&mut optimized_ir)
        .map_err(SeenError::from)?;
    maybe_emit_simd_report(simd_report, &optimized_ir, simd_policy)?;

    // LLVM codegen
    #[cfg(feature = "llvm")]
    {
        use seen_core::{LinkOutput, LlvmBackend, TargetOptions};
        let mut backend = LlvmBackend::new();
        backend.set_cli_mode(cli_mode);
        let default_target = if emit_ll {
            PathBuf::from("a.ll")
        } else {
            default_link_output_path(link_mode, target_triple)
        };
        let mut out_path = if let Some(path) = output.cloned() {
            path
        } else if bundling_android {
            PathBuf::from("bundle.aab")
        } else {
            default_target
        };
        let mut compile_out_path = out_path.clone();
        if bundling_android {
            if out_path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("aab"))
                .unwrap_or(false)
            {
                compile_out_path = out_path.with_extension("so");
            } else {
                return Err(SeenError::new(
                    SeenErrorKind::Tooling,
                    "Android bundling requires --output <name>.aab".to_string(),
                ));
            }
        }
        if let Some(triple) = target_triple {
            println!("Configuring LLVM backend for target {}", triple);
        }
        let mut target_options = TargetOptions {
            triple: target_triple,
            cpu: target_cpu,
            hardware_features: convert_cpu_features(cpu_features),
            memory_topology: convert_memory_topology_hint(memory_topology),
            ..Default::default()
        };
        if !cpu_features.is_empty() {
            println!(
                "Applying CPU feature overrides: {}",
                describe_cpu_features(cpu_features)
            );
        }
        if memory_topology != MemoryTopologyArg::Default {
            println!(
                "Applying memory topology hint: {}",
                memory_topology_label(memory_topology)
            );
        }
        if let Some(cpu) = target_cpu {
            println!("Applying target CPU override: {}", cpu);
        }
        if !emit_ll {
            if let Some(runtime_lib) = ensure_runtime_staticlib(target_triple)? {
                target_options.static_libraries.push(runtime_lib);
            }
        }
        if emit_ll {
            let ll_path = compile_out_path.as_path();
            backend
                .emit_llvm_ir(&optimized_ir, ll_path, target_options.clone())
                .map_err(|err| {
                    SeenError::new(
                        SeenErrorKind::Ir,
                        format!("Failed to emit LLVM IR to {}: {:?}", ll_path.display(), err),
                    )
                })?;
            println!("Generated LLVM IR: {}", ll_path.display());
        } else {
            let target = compile_out_path.clone();

            let link_output = match link_mode {
                BuildLinkMode::Executable => LinkOutput::Executable,
                BuildLinkMode::SharedLibrary => LinkOutput::SharedLibrary,
                BuildLinkMode::StaticLibrary => LinkOutput::StaticLibrary,
            };

            backend
                .emit_executable(&optimized_ir, &target, link_output, target_options)
                .map_err(|err| {
                    SeenError::new(
                        SeenErrorKind::Ir,
                        format!("Failed to produce output {}: {:?}", target.display(), err),
                    )
                })?;
            let artifact_path = if bundling_android { &out_path } else { &target };
            println!("Generated artifact: {}", artifact_path.display());
            if wasm_loader {
                emit_wasm_loader(&target)?;
            }
            if bundle {
                if bundling_wasm {
                    bundle_wasm_artifacts(&target)?;
                } else if bundling_android {
                    let project_dir = input
                        .parent()
                        .map(|p| p.to_path_buf())
                        .unwrap_or_else(|| PathBuf::from("."));
                    bundle_android_artifacts(&target, &out_path, &project_dir, target_triple)?;
                    if target != out_path {
                        let _ = fs::remove_file(&target);
                    }
                }
            }
        }
        Ok(())
    }
    #[cfg(not(feature = "llvm"))]
    {
        let _ = (
            emit_ll,
            link_mode,
            target_triple,
            wasm_loader,
            bundle,
            output,
            cpu_features,
            memory_topology,
            profile,
        );
        Err(SeenError::new(
            SeenErrorKind::Tooling,
            "LLVM backend not enabled at build time. Rebuild with: cargo build -p seen_cli --release --features llvm",
        ))
    }
}

#[cfg(feature = "llvm")]
fn ensure_runtime_staticlib(target_triple: Option<&str>) -> SeenResult<Option<PathBuf>> {
    use std::borrow::Cow;

    let host_target = std::env::var("TARGET")
        .or_else(|_| std::env::var("HOST"))
        .unwrap_or_else(|_| String::from("x86_64-unknown-linux-gnu"));
    let triple_owned = match target_triple {
        Some(explicit) => Cow::Borrowed(explicit),
        None => Cow::Owned(host_target),
    };
    let triple = triple_owned.as_ref();

    let runtime_src = runtime_source_path();
    if !runtime_src.exists() {
        return Ok(None);
    }

    let runtime_lib = runtime_staticlib_path(triple);
    if let Some(parent) = runtime_lib.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            SeenError::new(
                SeenErrorKind::Tooling,
                format!(
                    "Failed to create runtime output directory {}: {}",
                    parent.display(),
                    err
                ),
            )
        })?;
    }

    if needs_runtime_rebuild(&runtime_lib, &runtime_src)? {
        build_runtime_staticlib(triple)?;
    }
    Ok(Some(runtime_lib))
}

#[cfg(feature = "llvm")]
fn runtime_source_path() -> PathBuf {
    workspace_root()
        .join("seen_runtime")
        .join("src")
        .join("lib.rs")
}

#[cfg(feature = "llvm")]
fn runtime_staticlib_path(triple: &str) -> PathBuf {
    cargo_target_dir()
        .join("seen-runtime")
        .join(triple)
        .join("libseen_runtime.a")
}

#[cfg(feature = "llvm")]
fn needs_runtime_rebuild(output: &Path, source: &Path) -> SeenResult<bool> {
    if !output.exists() {
        return Ok(true);
    }
    let out_meta = fs::metadata(output).map_err(|err| {
        SeenError::new(
            SeenErrorKind::Tooling,
            format!("Failed to read metadata for {}: {}", output.display(), err),
        )
    })?;
    let src_meta = fs::metadata(source).map_err(|err| {
        SeenError::new(
            SeenErrorKind::Tooling,
            format!("Failed to read metadata for {}: {}", source.display(), err),
        )
    })?;
    let out_mod = out_meta.modified().map_err(|err| {
        SeenError::new(
            SeenErrorKind::Tooling,
            format!(
                "Failed to read modification timestamp for {}: {}",
                output.display(),
                err
            ),
        )
    })?;
    let src_mod = src_meta.modified().map_err(|err| {
        SeenError::new(
            SeenErrorKind::Tooling,
            format!(
                "Failed to read modification timestamp for {}: {}",
                source.display(),
                err
            ),
        )
    })?;
    Ok(src_mod > out_mod)
}

#[cfg(feature = "llvm")]
fn build_runtime_staticlib(triple: &str) -> SeenResult<()> {
    println!("Building seen_runtime for target {}", triple);
    let workspace = workspace_root();
    let target_dir = cargo_target_dir();
    let mut cmd = Command::new("cargo");
    cmd.current_dir(&workspace);
    cmd.env("CARGO_TARGET_DIR", &target_dir);
    cmd.arg("build")
        .arg("-p")
        .arg("seen_runtime")
        .arg("--target")
        .arg(triple)
        .arg("--release");
    let status = cmd.status().map_err(|err| {
        SeenError::new(
            SeenErrorKind::Tooling,
            format!("Failed to invoke cargo for seen_runtime: {}", err),
        )
    })?;
    if !status.success() {
        return Err(SeenError::new(
            SeenErrorKind::Tooling,
            "cargo failed while building seen_runtime static library".to_string(),
        ));
    }
    let produced = target_dir
        .join(triple)
        .join("release")
        .join("libseen_runtime.a");
    if !produced.exists() {
        return Err(SeenError::new(
            SeenErrorKind::Tooling,
            format!(
                "Expected runtime artifact {} but it was not produced",
                produced.display()
            ),
        ));
    }
    let dest = runtime_staticlib_path(triple);
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            SeenError::new(
                SeenErrorKind::Tooling,
                format!(
                    "Failed to create runtime output directory {}: {}",
                    parent.display(),
                    err
                ),
            )
        })?;
    }
    fs::copy(&produced, &dest).map_err(|err| {
        SeenError::new(
            SeenErrorKind::Tooling,
            format!(
                "Failed to copy runtime artifact from {} to {}: {}",
                produced.display(),
                dest.display(),
                err
            ),
        )
    })?;
    Ok(())
}

#[cfg(feature = "llvm")]
fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf()
}

#[cfg(feature = "llvm")]
fn cargo_target_dir() -> PathBuf {
    std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace_root().join("target"))
}

/// Recursively resolve and inline imports by parsing target .seen files
fn bundle_imports(
    program: Program,
    input_path: &Path,
    keyword_manager: Arc<KeywordManager>,
    visibility_policy: VisibilityPolicy,
    manifest_modules: &[PathBuf],
) -> SeenResult<Program> {
    let base_dir = input_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    let mut queue: VecDeque<(Program, PathBuf)> = VecDeque::new();
    queue.push_back((program, base_dir.clone()));
    let mut visited_modules: HashSet<String> = HashSet::new();
    let mut processed_files: HashSet<PathBuf> = HashSet::new();
    let input_canonical = canonicalize_lossy(input_path);
    processed_files.insert(input_canonical.clone());
    let mut merged = Program {
        expressions: Vec::new(),
    };

    while let Some((prog, module_dir)) = queue.pop_front() {
        for expr in prog.expressions {
            match &expr {
                Expression::Import { module_path, .. } => {
                    let key = module_path.join(".");
                    if visited_modules.contains(&key) {
                        continue;
                    }
                    visited_modules.insert(key.clone());
                    // Resolve candidate paths
                    let module_file = format!("{}.seen", module_path.join("/"));
                    let mut candidates = vec![
                        base_dir.join(&module_file),
                        PathBuf::from("compiler_seen/src").join(&module_file),
                        PathBuf::from("compiler_seen/src").join(format!(
                            "{}.seen",
                            module_path.last().unwrap_or(&String::new())
                        )),
                    ];
                    candidates.dedup();
                    let mut loaded = false;
                    for cand in candidates {
                        if cand.exists() {
                            let canonical = canonicalize_lossy(&cand);
                            if processed_files.contains(&canonical) {
                                loaded = true;
                                break;
                            }
                            let src = fs::read_to_string(&cand).map_err(|err| {
                                SeenError::new(
                                    SeenErrorKind::Io,
                                    format!(
                                        "Failed to read imported module {}: {}",
                                        cand.display(),
                                        err
                                    ),
                                )
                            })?;
                            let lex = Lexer::with_config(
                                src,
                                keyword_manager.clone(),
                                LexerConfig { visibility_policy },
                            );
                            let mut p = SeenParser::new_with_visibility(lex, visibility_policy);
                            let parsed = p
                                .parse_program()
                                .map_err(|err| annotate_parser_error(err.into(), &cand))?;
                            let module_base = cand
                                .parent()
                                .map(|p| p.to_path_buf())
                                .unwrap_or_else(|| base_dir.clone());
                            processed_files.insert(canonical.clone());
                            queue.push_back((parsed, module_base));
                            loaded = true;
                            break;
                        }
                    }
                    if !loaded {
                        eprintln!(
                            "Warning: could not resolve import module {}",
                            module_path.join(".")
                        );
                    }
                }
                _ => merged
                    .expressions
                    .push(rewrite_embed_attributes(expr, &module_dir)),
            }
        }
    }

    let manifest_files = collect_manifest_module_files(manifest_modules)?;
    for file in manifest_files {
        let canonical = canonicalize_lossy(&file);
        if processed_files.contains(&canonical) {
            continue;
        }
        let src = fs::read_to_string(&file).map_err(|err| {
            SeenError::new(
                SeenErrorKind::Io,
                format!("Failed to read manifest module {}: {}", file.display(), err),
            )
        })?;
        let module_dir = canonical
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        let lex = Lexer::with_config(
            src,
            keyword_manager.clone(),
            LexerConfig { visibility_policy },
        );
        let mut parser = SeenParser::new_with_visibility(lex, visibility_policy);
        let parsed = parser
            .parse_program()
            .map_err(|err| annotate_parser_error(err.into(), &file))?;
        for expr in parsed.expressions {
            merged
                .expressions
                .push(rewrite_embed_attributes(expr, &module_dir));
        }
        processed_files.insert(canonical);
    }
    Ok(merged)
}

fn canonicalize_lossy(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn annotate_parser_error(err: SeenError, file: &Path) -> SeenError {
    match err {
        SeenError::Categorised {
            kind,
            message,
            location,
        } => SeenError::Categorised {
            kind,
            message: format!("{} (while parsing {})", message, file.display()),
            location,
        },
        other => other,
    }
}

fn collect_manifest_module_files(entries: &[PathBuf]) -> SeenResult<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in entries {
        gather_module_files(entry, &mut files)?;
    }
    files.sort_by(|a, b| a.to_string_lossy().cmp(&b.to_string_lossy()));
    files.dedup();
    Ok(files)
}

fn gather_module_files(path: &Path, acc: &mut Vec<PathBuf>) -> SeenResult<()> {
    let metadata = match fs::metadata(path) {
        Ok(meta) => meta,
        Err(err) => {
            return Err(SeenError::new(
                SeenErrorKind::Io,
                format!("Failed to read module path {}: {}", path.display(), err),
            ))
        }
    };
    if metadata.is_file() {
        if path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("seen"))
            .unwrap_or(false)
        {
            acc.push(path.to_path_buf());
        }
        return Ok(());
    }
    if metadata.is_dir() {
        let mut children: Vec<PathBuf> = fs::read_dir(path)
            .map_err(|err| {
                SeenError::new(
                    SeenErrorKind::Io,
                    format!(
                        "Failed to read module directory {}: {}",
                        path.display(),
                        err
                    ),
                )
            })?
            .filter_map(|entry| entry.ok().map(|e| e.path()))
            .collect();
        children.sort_by(|a, b| a.to_string_lossy().cmp(&b.to_string_lossy()));
        for child in children {
            gather_module_files(&child, acc)?;
        }
    }
    Ok(())
}

#[cfg(any(test, feature = "llvm"))]
fn default_link_output_path(link_mode: BuildLinkMode, target_triple: Option<&str>) -> PathBuf {
    let triple = target_triple.unwrap_or_default();
    match link_mode {
        BuildLinkMode::Executable => {
            if is_wasm_target(triple) {
                PathBuf::from("module.wasm")
            } else if is_windows_target(triple) {
                PathBuf::from("a.exe")
            } else {
                PathBuf::from("a.out")
            }
        }
        BuildLinkMode::SharedLibrary => {
            if is_wasm_target(triple) {
                PathBuf::from("module.wasm")
            } else if is_android_target(triple) {
                PathBuf::from("liba.so")
            } else if is_macos_target(triple) {
                PathBuf::from("liba.dylib")
            } else if is_windows_target(triple) {
                PathBuf::from("a.dll")
            } else {
                PathBuf::from("liba.so")
            }
        }
        BuildLinkMode::StaticLibrary => {
            if is_windows_target(triple) {
                PathBuf::from("a.lib")
            } else {
                PathBuf::from("liba.a")
            }
        } // Object-only not exposed via CLI
    }
}

fn is_wasm_target(triple: &str) -> bool {
    !triple.is_empty() && triple.contains("wasm32")
}

fn is_android_target(triple: &str) -> bool {
    !triple.is_empty() && triple.contains("android")
}

#[cfg(any(test, feature = "llvm"))]
fn is_windows_target(triple: &str) -> bool {
    !triple.is_empty() && triple.contains("windows")
}

#[cfg(any(test, feature = "llvm"))]
fn is_macos_target(triple: &str) -> bool {
    !triple.is_empty() && (triple.contains("apple") || triple.contains("darwin"))
}

#[cfg(any(test, feature = "llvm"))]
fn emit_wasm_loader(wasm_path: &Path) -> SeenResult<()> {
    let wasm_file = wasm_path.file_name().ok_or_else(|| {
        SeenError::new(
            SeenErrorKind::Tooling,
            format!(
                "Unable to derive wasm file name from output path {}",
                wasm_path.display()
            ),
        )
    })?;

    let js_path = wasm_path.with_extension("js");
    let html_path = wasm_path.with_extension("html");
    let module_name = wasm_file.to_string_lossy();
    let js_content = format!(
        "async function main() {{
    const response = await fetch('{module_name}');
    const bytes = await response.arrayBuffer();
    const {{ instance }} = await WebAssembly.instantiate(bytes, {{}});
    if (instance.exports.seen_main) {{
        instance.exports.seen_main();
    }} else {{
        console.warn('seen_main export not found; available exports:', Object.keys(instance.exports));
    }}
}}

main().catch((err) => console.error('Error bootstrapping Seen wasm module:', err));
"
    );
    let html_content = format!(
        "<!doctype html>
<html lang=\"en\">
<head>
    <meta charset=\"utf-8\" />
    <title>Seen Wasm Loader</title>
</head>
<body>
    <script type=\"module\" src=\"{js}\"></script>
</body>
</html>
",
        js = js_path
            .file_name()
            .unwrap_or_else(|| std::ffi::OsStr::new("module.js"))
            .to_string_lossy()
    );

    fs::write(&js_path, js_content).map_err(|err| {
        SeenError::new(
            SeenErrorKind::Io,
            format!("Failed to write wasm loader {}: {}", js_path.display(), err),
        )
    })?;
    fs::write(&html_path, html_content).map_err(|err| {
        SeenError::new(
            SeenErrorKind::Io,
            format!(
                "Failed to write wasm HTML bootstrap {}: {}",
                html_path.display(),
                err
            ),
        )
    })?;

    println!(
        "Generated wasm loader: {} (HTML bootstrap: {})",
        js_path.display(),
        html_path.display()
    );
    Ok(())
}

#[cfg(feature = "llvm")]
fn bundle_wasm_artifacts(wasm_path: &Path) -> SeenResult<()> {
    let js_path = wasm_path.with_extension("js");
    let html_path = wasm_path.with_extension("html");
    for asset in [&js_path, wasm_path, &html_path] {
        if !asset.exists() {
            return Err(SeenError::new(
                SeenErrorKind::Tooling,
                format!(
                    "Cannot bundle wasm artifacts: {} is missing",
                    asset.display()
                ),
            ));
        }
    }

    let zip_path = wasm_path.with_extension("zip");
    let zip_file = fs::File::create(&zip_path).map_err(|err| {
        SeenError::new(
            SeenErrorKind::Io,
            format!(
                "Failed to create wasm bundle {}: {}",
                zip_path.display(),
                err
            ),
        )
    })?;

    let mut writer = zip::ZipWriter::new(zip_file);
    let options = FileOptions::default().compression_method(CompressionMethod::Deflated);

    for asset in [&js_path, wasm_path, &html_path] {
        let name = asset
            .file_name()
            .ok_or_else(|| {
                SeenError::new(
                    SeenErrorKind::Tooling,
                    format!("Failed to derive file name for {}", asset.display()),
                )
            })?
            .to_string_lossy()
            .to_string();
        let contents = fs::read(asset).map_err(|err| {
            SeenError::new(
                SeenErrorKind::Io,
                format!("Failed to read {}: {}", asset.display(), err),
            )
        })?;
        writer
            .start_file(name, options)
            .map_err(|err| SeenError::new(SeenErrorKind::Tooling, format!("{:?}", err)))?;
        writer
            .write_all(&contents)
            .map_err(|err| SeenError::new(SeenErrorKind::Io, format!("{:?}", err)))?;
    }

    writer
        .finish()
        .map_err(|err| SeenError::new(SeenErrorKind::Tooling, format!("{:?}", err)))?;

    println!("Generated wasm bundle: {}", zip_path.display());
    Ok(())
}

#[cfg(feature = "llvm")]
const DEFAULT_BUNDLE_CONFIG: &str = r#"modules {
  name: "base"
  module_type: BUNDLE_MODULE
  assets_config {}
}
"#;

#[cfg(feature = "llvm")]
const DEFAULT_ANDROID_MANIFEST: &str = r#"<manifest xmlns:android="http://schemas.android.com/apk/res/android"
    package="com.example.seenapp">
    <uses-sdk android:minSdkVersion="24" android:targetSdkVersion="34" />
    <application android:label="SeenApp" android:hasCode="true">
        <activity android:name=".MainActivity" android:exported="true">
            <intent-filter>
                <action android:name="android.intent.action.MAIN" />
                <category android:name="android.intent.category.LAUNCHER" />
            </intent-filter>
        </activity>
    </application>
</manifest>
"#;

#[cfg(feature = "llvm")]
const STUB_CLASSES_DEX: &[u8] = include_bytes!("android_stub/classes.dex");

#[cfg(feature = "llvm")]
fn bundle_android_artifacts(
    shared_lib: &Path,
    bundle_path: &Path,
    project_dir: &Path,
    target_triple: Option<&str>,
) -> SeenResult<()> {
    use std::io::Write;

    if let Some(parent) = bundle_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|err| {
                SeenError::new(
                    SeenErrorKind::Io,
                    format!("Failed to create directory {}: {}", parent.display(), err),
                )
            })?;
        }
    }

    let lib_bytes = std::fs::read(shared_lib).map_err(|err| {
        SeenError::new(
            SeenErrorKind::Io,
            format!("Failed to read {}: {}", shared_lib.display(), err),
        )
    })?;

    let bundle_file = File::create(bundle_path).map_err(|err| {
        SeenError::new(
            SeenErrorKind::Io,
            format!(
                "Failed to create Android bundle {}: {}",
                bundle_path.display(),
                err
            ),
        )
    })?;

    let mut writer = zip::ZipWriter::new(bundle_file);
    let deflated = FileOptions::default().compression_method(CompressionMethod::Deflated);

    let bundle_config_override = project_dir.join("BundleConfig.pb");
    if bundle_config_override.is_file() {
        let bytes = std::fs::read(&bundle_config_override).map_err(|err| {
            SeenError::new(
                SeenErrorKind::Io,
                format!(
                    "Failed to read BundleConfig override {}: {}",
                    bundle_config_override.display(),
                    err
                ),
            )
        })?;
        writer
            .start_file("BundleConfig.pb", deflated)
            .map_err(|err| SeenError::new(SeenErrorKind::Tooling, format!("{:?}", err)))?;
        writer
            .write_all(&bytes)
            .map_err(|err| SeenError::new(SeenErrorKind::Io, format!("{:?}", err)))?;
    } else {
        writer
            .start_file("BundleConfig.pb", deflated)
            .map_err(|err| SeenError::new(SeenErrorKind::Tooling, format!("{:?}", err)))?;
        writer
            .write_all(DEFAULT_BUNDLE_CONFIG.as_bytes())
            .map_err(|err| SeenError::new(SeenErrorKind::Io, format!("{:?}", err)))?;
    }

    let manifest_override = project_dir.join("AndroidManifest.xml");
    if manifest_override.is_file() {
        let bytes = std::fs::read(&manifest_override).map_err(|err| {
            SeenError::new(
                SeenErrorKind::Io,
                format!(
                    "Failed to read AndroidManifest.xml override {}: {}",
                    manifest_override.display(),
                    err
                ),
            )
        })?;
        writer
            .start_file("base/manifest/AndroidManifest.xml", deflated)
            .map_err(|err| SeenError::new(SeenErrorKind::Tooling, format!("{:?}", err)))?;
        writer
            .write_all(&bytes)
            .map_err(|err| SeenError::new(SeenErrorKind::Io, format!("{:?}", err)))?;
    } else {
        writer
            .start_file("base/manifest/AndroidManifest.xml", deflated)
            .map_err(|err| SeenError::new(SeenErrorKind::Tooling, format!("{:?}", err)))?;
        writer
            .write_all(DEFAULT_ANDROID_MANIFEST.as_bytes())
            .map_err(|err| SeenError::new(SeenErrorKind::Io, format!("{:?}", err)))?;
    }

    let abi_dir = android_abi_directory(target_triple);
    let lib_file_name = shared_lib
        .file_name()
        .unwrap_or_else(|| OsStr::new("libapp.so"));
    let lib_entry = format!("base/lib/{}/{}", abi_dir, lib_file_name.to_string_lossy());
    writer
        .start_file(
            lib_entry,
            FileOptions::default().compression_method(CompressionMethod::Stored),
        )
        .map_err(|err| SeenError::new(SeenErrorKind::Tooling, format!("{:?}", err)))?;
    writer
        .write_all(&lib_bytes)
        .map_err(|err| SeenError::new(SeenErrorKind::Io, format!("{:?}", err)))?;

    let assets_stats =
        copy_directory_to_bundle(&mut writer, &project_dir.join("assets"), "base/assets")?;
    let res_stats = copy_directory_to_bundle(&mut writer, &project_dir.join("res"), "base/res")?;
    let dex_stats = copy_directory_to_bundle(&mut writer, &project_dir.join("dex"), "base/dex")?;
    let _root_stats =
        copy_directory_to_bundle(&mut writer, &project_dir.join("root"), "base/root")?;

    let resources_pb = project_dir.join("resources.pb");
    let alt_resources_pb = project_dir.join("base").join("resources.pb");
    let resources_source = if resources_pb.is_file() {
        Some(resources_pb)
    } else if alt_resources_pb.is_file() {
        Some(alt_resources_pb)
    } else {
        None
    };
    if let Some(res_path) = resources_source {
        let bytes = std::fs::read(&res_path).map_err(|err| {
            SeenError::new(
                SeenErrorKind::Io,
                format!(
                    "Failed to read resources.pb {}: {}",
                    res_path.display(),
                    err
                ),
            )
        })?;
        writer
            .start_file("base/resources.pb", deflated)
            .map_err(|err| SeenError::new(SeenErrorKind::Tooling, format!("{:?}", err)))?;
        writer
            .write_all(&bytes)
            .map_err(|err| SeenError::new(SeenErrorKind::Io, format!("{:?}", err)))?;
    }

    let mut inserted_stub_dex = false;
    if dex_stats.dex_files == 0 {
        writer
            .start_file(
                "base/dex/classes.dex",
                FileOptions::default().compression_method(CompressionMethod::Stored),
            )
            .map_err(|err| SeenError::new(SeenErrorKind::Tooling, format!("{:?}", err)))?;
        writer
            .write_all(STUB_CLASSES_DEX)
            .map_err(|err| SeenError::new(SeenErrorKind::Io, format!("{:?}", err)))?;
        inserted_stub_dex = true;
    }

    writer
        .finish()
        .map_err(|err| SeenError::new(SeenErrorKind::Tooling, format!("{:?}", err)))?;

    maybe_sign_android_bundle(bundle_path)?;

    if inserted_stub_dex {
        println!(
            "Generated Android bundle: {} (note: inserted stub classes.dex; add dex/ to override)",
            bundle_path.display()
        );
    } else {
        println!("Generated Android bundle: {}", bundle_path.display());
    }

    if assets_stats.files > 0 {
        println!(
            "Bundled {} asset file(s) from {}",
            assets_stats.files,
            project_dir.join("assets").display()
        );
    }
    if res_stats.files > 0 {
        println!(
            "Bundled {} resource file(s) from {}",
            res_stats.files,
            project_dir.join("res").display()
        );
    }

    Ok(())
}

#[cfg(feature = "llvm")]
#[derive(Default)]
struct DirectoryStats {
    files: usize,
    dex_files: usize,
}

#[cfg(feature = "llvm")]
fn copy_directory_to_bundle(
    writer: &mut zip::ZipWriter<File>,
    source_dir: &Path,
    dest_prefix: &str,
) -> SeenResult<DirectoryStats> {
    if !source_dir.exists() {
        return Ok(DirectoryStats::default());
    }
    if !source_dir.is_dir() {
        return Err(SeenError::new(
            SeenErrorKind::Tooling,
            format!(
                "Expected Android bundle directory {}, but found a non-directory entry",
                source_dir.display()
            ),
        ));
    }

    let mut files = Vec::new();
    collect_files_sorted(source_dir, &mut files)?;

    let mut stats = DirectoryStats::default();
    for file in files {
        let relative = file.strip_prefix(source_dir).map_err(|err| {
            SeenError::new(
                SeenErrorKind::Tooling,
                format!(
                    "Failed to derive relative path for {}: {}",
                    file.display(),
                    err
                ),
            )
        })?;
        let entry = if relative.as_os_str().is_empty() {
            dest_prefix.trim_end_matches('/').to_string()
        } else {
            format!(
                "{}/{}",
                dest_prefix.trim_end_matches('/'),
                path_to_bundle_entry(relative)
            )
        };
        let contents = std::fs::read(&file).map_err(|err| {
            SeenError::new(
                SeenErrorKind::Io,
                format!("Failed to read {}: {}", file.display(), err),
            )
        })?;

        let method = compression_for_path(&file);
        let options = FileOptions::default().compression_method(method);
        writer
            .start_file(entry, options)
            .map_err(|err| SeenError::new(SeenErrorKind::Tooling, format!("{:?}", err)))?;
        writer
            .write_all(&contents)
            .map_err(|err| SeenError::new(SeenErrorKind::Io, format!("{:?}", err)))?;

        stats.files += 1;
        if file
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("dex"))
            .unwrap_or(false)
        {
            stats.dex_files += 1;
        }
    }

    Ok(stats)
}

#[cfg(feature = "llvm")]
fn collect_files_sorted(dir: &Path, out: &mut Vec<PathBuf>) -> SeenResult<()> {
    let mut entries = std::fs::read_dir(dir)
        .map_err(|err| {
            SeenError::new(
                SeenErrorKind::Io,
                format!("Failed to read directory {}: {}", dir.display(), err),
            )
        })?
        .map(|res| res.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| {
            SeenError::new(
                SeenErrorKind::Io,
                format!("Failed to iterate directory {}: {}", dir.display(), err),
            )
        })?;

    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect_files_sorted(&path, out)?;
        } else if path.is_file() {
            out.push(path);
        }
    }
    Ok(())
}

#[cfg(feature = "llvm")]
fn path_to_bundle_entry(path: &Path) -> String {
    path.components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(feature = "llvm")]
fn compression_for_path(path: &Path) -> CompressionMethod {
    if let Some(ext) = path.extension().and_then(|ext| ext.to_str()) {
        if ext.eq_ignore_ascii_case("so")
            || ext.eq_ignore_ascii_case("dex")
            || ext.eq_ignore_ascii_case("jar")
            || ext.eq_ignore_ascii_case("apk")
            || ext.eq_ignore_ascii_case("aab")
        {
            return CompressionMethod::Stored;
        }
    }
    CompressionMethod::Deflated
}

#[cfg(feature = "llvm")]
fn android_abi_directory(target_triple: Option<&str>) -> &'static str {
    if let Some(triple) = target_triple {
        if triple.contains("aarch64") {
            return "arm64-v8a";
        }
        if triple.contains("armv7") || triple.contains("thumbv7") {
            return "armeabi-v7a";
        }
        if triple.contains("x86_64") {
            return "x86_64";
        }
        if triple.contains("i686") || triple.contains("x86") {
            return "x86";
        }
    }
    "arm64-v8a"
}

#[cfg(feature = "llvm")]
fn maybe_sign_android_bundle(bundle_path: &Path) -> SeenResult<()> {
    let keystore = match std::env::var("SEEN_ANDROID_KEYSTORE") {
        Ok(value) if !value.trim().is_empty() => value,
        _ => return Ok(()),
    };

    let alias = std::env::var("SEEN_ANDROID_KEY_ALIAS").unwrap_or_else(|_| "seenapp".to_string());
    let jarsigner =
        std::env::var("SEEN_ANDROID_JARSIGNER").unwrap_or_else(|_| "jarsigner".to_string());

    let mut cmd = Command::new(&jarsigner);
    cmd.arg("-keystore").arg(&keystore);

    if let Ok(store_type) = std::env::var("SEEN_ANDROID_KEYSTORE_TYPE") {
        if !store_type.trim().is_empty() {
            cmd.arg("-storetype").arg(store_type);
        }
    }
    if let Ok(provider) = std::env::var("SEEN_ANDROID_KEYSTORE_PROVIDER") {
        if !provider.trim().is_empty() {
            cmd.arg("-provider").arg(provider);
        }
    }

    if let Ok(storepass) = std::env::var("SEEN_ANDROID_KEYSTORE_PASS") {
        if !storepass.is_empty() {
            cmd.arg("-storepass").arg(&storepass);
            let keypass =
                std::env::var("SEEN_ANDROID_KEY_PASS").unwrap_or_else(|_| storepass.clone());
            if !keypass.is_empty() {
                cmd.arg("-keypass").arg(&keypass);
            }
        }
    } else if let Ok(keypass) = std::env::var("SEEN_ANDROID_KEY_PASS") {
        if !keypass.is_empty() {
            cmd.arg("-keypass").arg(&keypass);
        }
    }

    if let Ok(sigalg) = std::env::var("SEEN_ANDROID_SIGALG") {
        if !sigalg.trim().is_empty() {
            cmd.arg("-sigalg").arg(sigalg);
        }
    } else {
        cmd.arg("-sigalg").arg("SHA256withRSA");
    }

    if let Ok(digestalg) = std::env::var("SEEN_ANDROID_DIGESTALG") {
        if !digestalg.trim().is_empty() {
            cmd.arg("-digestalg").arg(digestalg);
        }
    } else {
        cmd.arg("-digestalg").arg("SHA-256");
    }

    if let Ok(tsa_url) = std::env::var("SEEN_ANDROID_TIMESTAMP_URL") {
        if !tsa_url.trim().is_empty() {
            cmd.arg("-tsa").arg(tsa_url);
        }
    }

    cmd.arg(bundle_path);
    cmd.arg(&alias);

    let status = cmd.status().map_err(|err| {
        SeenError::new(
            SeenErrorKind::Tooling,
            format!(
                "Failed to invoke {} for Android bundle signing: {}",
                jarsigner, err
            ),
        )
    })?;

    if !status.success() {
        return Err(SeenError::new(
            SeenErrorKind::Tooling,
            format!(
                "{} exited with status {} while signing {}",
                jarsigner,
                status,
                bundle_path.display()
            ),
        ));
    }

    println!(
        "Signed Android bundle {} using keystore {} (alias {})",
        bundle_path.display(),
        keystore,
        alias
    );
    Ok(())
}

fn rewrite_embed_attributes(expr: Expression, module_dir: &Path) -> Expression {
    match expr {
        Expression::Const {
            name,
            type_annotation,
            value,
            attributes,
            pos,
        } => {
            let rewritten_attrs = attributes
                .into_iter()
                .map(|attr| rewrite_embed_attribute(attr, module_dir))
                .collect();
            Expression::Const {
                name,
                type_annotation,
                value,
                attributes: rewritten_attrs,
                pos,
            }
        }
        other => other,
    }
}

fn rewrite_embed_attribute(attr: Attribute, module_dir: &Path) -> Attribute {
    let Attribute { name, args, pos } = attr;

    if name != "embed" {
        return Attribute { name, args, pos };
    }

    let rewritten_args = args
        .into_iter()
        .map(|arg| match arg {
            AttributeArgument::Named { name, value } if name == "path" => {
                AttributeArgument::Named {
                    name,
                    value: resolve_embed_path(value, module_dir),
                }
            }
            other => other,
        })
        .collect();

    Attribute {
        name,
        args: rewritten_args,
        pos,
    }
}

fn resolve_embed_path(value: AttributeValue, module_dir: &Path) -> AttributeValue {
    match value {
        AttributeValue::String(original) => {
            let candidate = module_dir.join(&original);
            let resolved = if Path::new(&original).is_absolute() {
                PathBuf::from(original)
            } else {
                candidate
            };

            let canonical = resolved.canonicalize().unwrap_or_else(|_| resolved.clone());

            AttributeValue::String(canonical.to_string_lossy().to_string())
        }
        other => other,
    }
}

fn run_file(input: &Path, args: &[String], keyword_manager: Arc<KeywordManager>) -> SeenResult<()> {
    // Read source file
    let source = fs::read_to_string(input).map_err(|err| {
        SeenError::new(
            SeenErrorKind::Io,
            format!("Failed to read source file {}: {}", input.display(), err),
        )
    })?;

    // Lex and parse
    let (project_config, lexer_config) = project_context_for(input)?;
    let visibility_policy = lexer_config.visibility_policy;
    let manifest_modules = manifest_module_roots(&project_config);
    let lexer = Lexer::with_config(source.clone(), keyword_manager.clone(), lexer_config);
    let mut parser = SeenParser::new_with_visibility(lexer, visibility_policy);
    let parsed = parser.parse_program().map_err(SeenError::from)?;
    // Bundle imports so interpreter sees imported functions
    let ast = bundle_imports(
        parsed,
        input,
        keyword_manager.clone(),
        visibility_policy,
        &manifest_modules,
    )?;

    // Type check
    let mut type_checker = TypeChecker::new();
    let type_result = type_checker.check_program(&ast);
    if !type_result.errors.is_empty() {
        for error in &type_result.errors {
            eprintln!("Type error: {}", error);
        }
        if let Some(primary) = type_result.errors.first().cloned() {
            return Err(SeenError::from(primary));
        }
        return Err(SeenError::new(
            SeenErrorKind::TypeChecker,
            format!(
                "Type checking failed with {} errors",
                type_result.errors.len()
            ),
        ));
    }

    // Interpret the program
    let mut interpreter = Interpreter::new();
    // Forward program args via env for __GetCommandLineArgs override
    if !args.is_empty() {
        let mut s = String::from("seen");
        for a in args {
            s.push(' ');
            s.push_str(a);
        }
        std::env::set_var("SEEN_PROGRAM_ARGS", s);
    }
    let result = interpreter.interpret(&ast).map_err(SeenError::from)?;

    // Only print result if it's not Unit
    if !matches!(result, Value::Unit) {
        println!("{}", result);
    }

    Ok(())
}

#[cfg(feature = "llvm")]
fn run_file_llvm(
    input: &Path,
    args: &[String],
    opt_level: u8,
    target_triple: Option<&str>,
    target_cpu: Option<&str>,
    keyword_manager: Arc<KeywordManager>,
    cpu_features: &[CpuFeatureFlagArg],
    memory_topology: MemoryTopologyArg,
    simd_policy: SimdPolicyArg,
    profile: Profile,
) -> SeenResult<()> {
    let temp = tempdir().map_err(|err| {
        SeenError::new(
            SeenErrorKind::Tooling,
            format!("Failed to create temporary directory: {}", err),
        )
    })?;
    let artifact = temp.path().join("seen_run");
    compile_file_llvm(
        input,
        Some(&artifact),
        opt_level,
        false,
        BuildLinkMode::Executable,
        keyword_manager,
        target_triple,
        false,
        false,
        cpu_features,
        memory_topology,
        simd_policy,
        target_cpu,
        None,
        profile,
        true,
    )?;
    let mut cmd = Command::new(&artifact);
    if let Some(parent) = input.parent() {
        cmd.current_dir(parent);
    }
    if !args.is_empty() {
        cmd.args(args);
    }
    let status = cmd.status().map_err(|err| {
        SeenError::new(
            SeenErrorKind::Tooling,
            format!(
                "Failed to run compiled binary {}: {}",
                artifact.display(),
                err
            ),
        )
    })?;
    if !status.success() {
        return Err(SeenError::new(
            SeenErrorKind::Tooling,
            format!(
                "LLVM run failed with status {} when executing {}",
                status,
                artifact.display()
            ),
        ));
    }
    Ok(())
}

fn check_file(input: &Path, keyword_manager: Arc<KeywordManager>) -> SeenResult<()> {
    println!("Checking {}", input.display());

    // Read source file
    let source = fs::read_to_string(input).map_err(|err| {
        SeenError::new(
            SeenErrorKind::Io,
            format!("Failed to read source file {}: {}", input.display(), err),
        )
    })?;

    // Lex and parse
    let (_project_config, lexer_config) = project_context_for(input)?;
    let visibility_policy = lexer_config.visibility_policy;
    let lexer = Lexer::with_config(source, keyword_manager, lexer_config);
    let mut parser = SeenParser::new_with_visibility(lexer, visibility_policy);
    let ast = parser.parse_program().map_err(SeenError::from)?;

    // Type check
    let mut type_checker = TypeChecker::new();
    let type_result = type_checker.check_program(&ast);
    if !type_result.errors.is_empty() {
        for error in &type_result.errors {
            eprintln!("Type error: {}", error);
        }
        if let Some(primary) = type_result.errors.first().cloned() {
            return Err(SeenError::from(primary));
        }
        return Err(SeenError::new(
            SeenErrorKind::TypeChecker,
            format!(
                "Type checking failed with {} errors",
                type_result.errors.len()
            ),
        ));
    }

    println!("✓ No errors found");
    Ok(())
}

fn generate_ir(
    input: &Path,
    opt_level: u8,
    keyword_manager: Arc<KeywordManager>,
) -> SeenResult<()> {
    println!("Generating IR for {}", input.display());

    // Read source file
    let source = fs::read_to_string(input).map_err(|err| {
        SeenError::new(
            SeenErrorKind::Io,
            format!("Failed to read source file {}: {}", input.display(), err),
        )
    })?;

    // Lex and parse
    let (_project_config, lexer_config) = project_context_for(input)?;
    let visibility_policy = lexer_config.visibility_policy;
    let lexer = Lexer::with_config(source, keyword_manager, lexer_config);
    let mut parser = SeenParser::new_with_visibility(lexer, visibility_policy);
    let ast = parser.parse_program().map_err(SeenError::from)?;

    // Type check
    let mut type_checker = TypeChecker::new();
    let type_result = type_checker.check_program(&ast);
    if !type_result.errors.is_empty() {
        for error in &type_result.errors {
            eprintln!("Type error: {}", error);
        }
        if let Some(primary) = type_result.errors.first().cloned() {
            return Err(SeenError::from(primary));
        }
        return Err(SeenError::new(
            SeenErrorKind::TypeChecker,
            format!(
                "Type checking failed with {} errors",
                type_result.errors.len()
            ),
        ));
    }

    // Generate IR
    let mut ir_generator = IRGenerator::new();
    let ir_program = ir_generator.generate(&ast).map_err(SeenError::from)?;

    println!("\n=== Generated IR ===");
    println!("{}", ir_program);

    // Optimize IR if requested
    if opt_level > 0 {
        let optimization_level = OptimizationLevel::from_level(opt_level);
        let mut optimizer = IROptimizer::new(optimization_level);
        let mut optimized_ir = ir_program;
        optimizer
            .optimize_program(&mut optimized_ir)
            .map_err(SeenError::from)?;

        println!("\n=== Optimized IR (level {}) ===", opt_level);
        println!("{}", optimized_ir);
    }

    Ok(())
}

fn determinism_check(
    input: &Path,
    opt_level: u8,
    backend: Backend,
    keyword_manager: Arc<KeywordManager>,
) -> SeenResult<()> {
    use sha2::{Digest, Sha256};

    if backend == Backend::Llvm {
        return Err(SeenError::new(
            SeenErrorKind::Tooling,
            "Determinism for the LLVM backend requires artifact inspection; rerun with --backend ir, --backend mlir, or --backend clif"
                .to_string(),
        ));
    }

    let run_once = || -> SeenResult<String> {
        let optimized_ir = generate_optimized_ir(
            input,
            opt_level,
            keyword_manager.clone(),
            &[],
            MemoryTopologyArg::Default,
            SimdPolicyArg::Auto,
            None,
        )?;
        let text = match backend {
            Backend::Ir => format!("{}", optimized_ir),
            Backend::Mlir => program_to_mlir(&optimized_ir),
            Backend::Clif => program_to_clif(&optimized_ir),
            Backend::Llvm => unreachable!(),
        };
        Ok(text)
    };

    let text1 = run_once()?;
    let text2 = run_once()?;

    let mut h1 = Sha256::new();
    h1.update(text1.as_bytes());
    let hash1 = format!("{:x}", h1.finalize());

    let mut h2 = Sha256::new();
    h2.update(text2.as_bytes());
    let hash2 = format!("{:x}", h2.finalize());

    println!("Stage2 hash: {}", hash1);
    println!("Stage3 hash: {}", hash2);
    if hash1 == hash2 {
        println!("Determinism: OK (Stage2 == Stage3)");
        Ok(())
    } else {
        Err(SeenError::new(
            SeenErrorKind::Tooling,
            "Determinism: FAILED (hash mismatch)",
        ))
    }
}

fn trace_ir(input: &Path, opt_level: u8, keyword_manager: Arc<KeywordManager>) -> SeenResult<()> {
    let optimized_ir = generate_optimized_ir(
        input,
        opt_level,
        keyword_manager,
        &[],
        MemoryTopologyArg::Default,
        SimdPolicyArg::Auto,
        None,
    )?;

    println!(
        "IR trace for {} (optimization level {})",
        input.display(),
        opt_level
    );
    for module in &optimized_ir.modules {
        println!("module {}", module.name);
        for function in module.functions_iter() {
            println!("{}", function);
        }
    }

    Ok(())
}

fn build_shaders_command(
    input: &Path,
    output: Option<&Path>,
    targets: &[ShaderTargetArg],
    validate_only: bool,
    recursive: bool,
) -> SeenResult<()> {
    if !input.exists() {
        return Err(SeenError::new(
            SeenErrorKind::Io,
            format!("Shader input {} does not exist", input.display()),
        ));
    }

    let files = collect_spirv_inputs(input, recursive)?;
    if files.is_empty() {
        return Err(SeenError::new(
            SeenErrorKind::Tooling,
            format!(
                "No .spv files found under {} (pass --recursive to traverse subdirectories)",
                input.display()
            ),
        ));
    }

    let resolved_targets = if validate_only {
        convert_shader_targets(targets)
    } else {
        let converted = convert_shader_targets(targets);
        if converted.is_empty() {
            return Err(SeenError::new(
                SeenErrorKind::Tooling,
                "Specify at least one --target when emitting shader artifacts".to_string(),
            ));
        }
        converted
    };

    let output_root = output
        .map(PathBuf::from)
        .unwrap_or_else(|| default_shader_output_root(input));
    let shader_root = shader_input_root(input);
    let compiler = ShaderCompiler::new();

    for file in files {
        let relative_dir = file
            .parent()
            .and_then(|p| p.strip_prefix(&shader_root).ok())
            .filter(|p| !p.as_os_str().is_empty());
        let file_output_dir = relative_dir
            .map(|rel| output_root.join(rel))
            .unwrap_or_else(|| output_root.clone());
        let options = ShaderCompileOptions {
            output_dir: file_output_dir.as_path(),
            targets: resolved_targets.as_slice(),
            validate_only,
        };
        let result = compiler
            .compile_file(&file, &options)
            .map_err(shader_error_to_seen)?;
        let entry_summary = format_entry_points(&result.entry_points);
        if validate_only {
            println!("validated {} [{}]", file.display(), entry_summary);
        } else {
            println!("validated {} [{}]", file.display(), entry_summary);
            for artifact in result.artifacts {
                println!(
                    "  {} {}",
                    artifact.target.as_label(),
                    artifact.path.display()
                );
            }
        }
    }

    Ok(())
}

fn convert_shader_targets(args: &[ShaderTargetArg]) -> Vec<ShaderTarget> {
    let mut targets = Vec::new();
    for arg in args {
        let target = match arg {
            ShaderTargetArg::Spirv => ShaderTarget::Spirv,
            ShaderTargetArg::Wgsl => ShaderTarget::Wgsl,
            ShaderTargetArg::Msl => ShaderTarget::Msl,
        };
        if !targets.contains(&target) {
            targets.push(target);
        }
    }
    targets
}

fn collect_spirv_inputs(input: &Path, recursive: bool) -> SeenResult<Vec<PathBuf>> {
    if input.is_file() {
        if is_spirv_file(input) {
            return Ok(vec![input.to_path_buf()]);
        }
        return Err(SeenError::new(
            SeenErrorKind::Tooling,
            format!("{} is not a .spv shader file", input.display()),
        ));
    }
    if !input.is_dir() {
        return Err(SeenError::new(
            SeenErrorKind::Tooling,
            format!("{} is not a directory", input.display()),
        ));
    }

    let mut queue = VecDeque::new();
    let mut files = Vec::new();
    queue.push_back(input.to_path_buf());
    while let Some(dir) = queue.pop_front() {
        for entry in fs::read_dir(&dir).map_err(|err| {
            SeenError::new(
                SeenErrorKind::Io,
                format!("Failed to read directory {}: {}", dir.display(), err),
            )
        })? {
            let entry = entry.map_err(|err| {
                SeenError::new(
                    SeenErrorKind::Io,
                    format!("Failed to read entry in {}: {}", dir.display(), err),
                )
            })?;
            let path = entry.path();
            if path.is_dir() && recursive {
                queue.push_back(path);
            } else if path.is_file() && is_spirv_file(&path) {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

fn is_spirv_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("spv"))
        .unwrap_or(false)
}

fn default_shader_output_root(input: &Path) -> PathBuf {
    if input.is_file() {
        input
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."))
            .join("shaders_out")
    } else {
        input.join("shaders_out")
    }
}

fn shader_input_root(input: &Path) -> PathBuf {
    if input.is_file() {
        input
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."))
    } else {
        input.to_path_buf()
    }
}

fn shader_error_to_seen(err: ShaderError) -> SeenError {
    SeenError::new(SeenErrorKind::Tooling, err.to_string())
}

fn format_entry_points(entry_points: &[ShaderEntryPoint]) -> String {
    if entry_points.is_empty() {
        return "no entry points".to_string();
    }
    entry_points
        .iter()
        .map(|ep| format!("{} {}", ep.stage.as_str(), ep.name))
        .collect::<Vec<_>>()
        .join(", ")
}

fn package_directory(input: &Path, output: Option<&Path>) -> SeenResult<()> {
    if !input.exists() {
        return Err(SeenError::new(
            SeenErrorKind::Io,
            format!("Package directory {} does not exist", input.display()),
        ));
    }
    if !input.is_dir() {
        return Err(SeenError::new(
            SeenErrorKind::Io,
            format!("{} is not a directory", input.display()),
        ));
    }

    let mut out_path = output.map(PathBuf::from).unwrap_or_else(|| {
        let mut candidate = input.to_path_buf();
        candidate.set_extension("zip");
        candidate
    });
    if out_path.extension().is_none() {
        out_path.set_extension("zip");
    }
    if let Some(parent) = out_path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|err| {
                SeenError::new(
                    SeenErrorKind::Io,
                    format!("Failed to create directory {}: {}", parent.display(), err),
                )
            })?;
        }
    }

    let mut files = Vec::new();
    collect_files_recursively(input, input, &mut files)?;
    files.sort();

    let file = File::create(&out_path).map_err(|err| {
        SeenError::new(
            SeenErrorKind::Io,
            format!("Failed to create archive {}: {}", out_path.display(), err),
        )
    })?;
    let mut writer = zip::ZipWriter::new(file);
    let options = FileOptions::default().compression_method(CompressionMethod::Deflated);

    for (absolute, relative) in files {
        let contents = fs::read(&absolute).map_err(|err| {
            SeenError::new(
                SeenErrorKind::Io,
                format!("Failed to read {}: {}", absolute.display(), err),
            )
        })?;
        let name = relative
            .components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        writer
            .start_file(name, options)
            .map_err(|err| SeenError::new(SeenErrorKind::Tooling, format!("{:?}", err)))?;
        writer
            .write_all(&contents)
            .map_err(|err| SeenError::new(SeenErrorKind::Io, format!("{:?}", err)))?;
    }

    writer
        .finish()
        .map_err(|err| SeenError::new(SeenErrorKind::Tooling, format!("{:?}", err)))?;

    println!("Created package {}", out_path.display());
    Ok(())
}

fn collect_files_recursively(
    root: &Path,
    current: &Path,
    files: &mut Vec<(PathBuf, PathBuf)>,
) -> SeenResult<()> {
    for entry in fs::read_dir(current).map_err(|err| {
        SeenError::new(
            SeenErrorKind::Io,
            format!("Failed to read directory {}: {}", current.display(), err),
        )
    })? {
        let entry = entry.map_err(|err| {
            SeenError::new(
                SeenErrorKind::Io,
                format!(
                    "Failed to read directory entry in {}: {}",
                    current.display(),
                    err
                ),
            )
        })?;
        let path = entry.path();
        if path.is_dir() {
            collect_files_recursively(root, &path, files)?;
        } else if path.is_file() {
            let rel = path.strip_prefix(root).map_err(|err| {
                SeenError::new(
                    SeenErrorKind::Io,
                    format!(
                        "Failed to compute relative path for {}: {}",
                        path.display(),
                        err
                    ),
                )
            })?;
            files.push((path.clone(), rel.to_path_buf()));
        }
    }
    Ok(())
}

fn run_repl(
    keyword_manager: Arc<KeywordManager>,
    show_ast: bool,
    show_types: bool,
) -> SeenResult<()> {
    let mut interpreter = Interpreter::new();
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    // Store multi-line input
    let mut input_buffer = String::new();
    let mut in_multiline = false;

    loop {
        if in_multiline {
            print!("... ");
        } else {
            print!("> ");
        }
        stdout.flush()?;

        let mut line = String::new();
        if stdin.lock().read_line(&mut line)? == 0 {
            // EOF
            println!("\nGoodbye!");
            break;
        }

        let line = line.trim_end(); // Keep leading whitespace for multi-line

        if !in_multiline && line.is_empty() {
            continue;
        }

        if !in_multiline && (line == "exit" || line == "quit") {
            println!("Goodbye!");
            break;
        }

        // Check for multi-line input
        if line.ends_with('{') || line.ends_with('(') {
            in_multiline = true;
            input_buffer.push_str(line);
            input_buffer.push('\n');
            continue;
        }

        if in_multiline {
            input_buffer.push_str(line);
            input_buffer.push('\n');

            // Simple check for end of multi-line (can be improved)
            let open_braces = input_buffer.chars().filter(|&c| c == '{').count();
            let close_braces = input_buffer.chars().filter(|&c| c == '}').count();
            let open_parens = input_buffer.chars().filter(|&c| c == '(').count();
            let close_parens = input_buffer.chars().filter(|&c| c == ')').count();

            if open_braces <= close_braces && open_parens <= close_parens {
                in_multiline = false;
                let input = input_buffer.clone();
                input_buffer.clear();

                // Evaluate the complete input
                match evaluate_line(
                    &input,
                    &keyword_manager,
                    &mut interpreter,
                    show_ast,
                    show_types,
                ) {
                    Ok(Some(result)) => {
                        if !matches!(result, Value::Unit) {
                            println!("=> {}", result);
                        }
                    }
                    Ok(None) => {}
                    Err(e) => {
                        eprintln!("Error: {:#}", e);
                    }
                }
            }
        } else {
            // Single-line input
            match evaluate_line(
                line,
                &keyword_manager,
                &mut interpreter,
                show_ast,
                show_types,
            ) {
                Ok(Some(result)) => {
                    if !matches!(result, Value::Unit) {
                        println!("=> {}", result);
                    }
                }
                Ok(None) => {}
                Err(e) => {
                    eprintln!("Error: {:#}", e);
                }
            }
        }
    }

    Ok(())
}

fn evaluate_line(
    line: &str,
    keyword_manager: &Arc<KeywordManager>,
    interpreter: &mut Interpreter,
    show_ast: bool,
    show_types: bool,
) -> SeenResult<Option<Value>> {
    // Lex and parse
    let lexer = Lexer::with_config(
        line.to_string(),
        keyword_manager.clone(),
        LexerConfig::default(),
    );
    let mut parser = SeenParser::new(lexer);
    let ast = parser.parse_program().map_err(SeenError::from)?;

    if show_ast {
        println!("AST: {:#?}", ast);
    }

    // Type check
    let mut type_checker = TypeChecker::new();
    let type_result = type_checker.check_program(&ast);
    if !type_result.errors.is_empty() {
        for error in &type_result.errors {
            eprintln!("Type error: {}", error);
        }
        if let Some(primary) = type_result.errors.first().cloned() {
            return Err(SeenError::from(primary));
        }
        return Err(SeenError::new(
            SeenErrorKind::TypeChecker,
            format!(
                "Type checking failed with {} errors",
                type_result.errors.len()
            ),
        ));
    }

    if show_types {
        println!("Types: {:?}", type_result);
    }

    // Interpret
    let result = interpreter.interpret(&ast).map_err(SeenError::from)?;
    Ok(Some(result))
}

fn format_file(input: &Path, in_place: bool, check: bool) -> SeenResult<()> {
    println!(
        "Formatting {} (in-place: {}, check: {})",
        input.display(),
        in_place,
        check
    );

    // Read source file
    let source = fs::read_to_string(input).map_err(|err| {
        SeenError::new(
            SeenErrorKind::Io,
            format!("Failed to read source file {}: {}", input.display(), err),
        )
    })?;

    // Create keyword manager
    let mut keyword_manager = KeywordManager::new();
    keyword_manager.load_from_toml("en").map_err(|err| {
        SeenError::new(
            SeenErrorKind::Tooling,
            format!("Failed to load keywords: {}", err),
        )
    })?;
    keyword_manager.switch_language("en").map_err(|err| {
        SeenError::new(
            SeenErrorKind::Tooling,
            format!("Failed to switch to English keywords: {}", err),
        )
    })?;
    let (_project_config, lexer_config) = project_context_for(input)?;

    // Parse the source code
    let visibility_policy = lexer_config.visibility_policy;
    let lexer = Lexer::with_config(source.clone(), Arc::new(keyword_manager), lexer_config);
    let mut parser = SeenParser::new_with_visibility(lexer, visibility_policy);
    let program = parser.parse_program().map_err(|err| SeenError::from(err))?;

    // Format the code
    let formatted_code = format_program(&program);

    if check {
        if source == formatted_code {
            println!("{} is already formatted", input.display());
            return Ok(());
        } else {
            println!("{} would be reformatted", input.display());
            return Err(SeenError::new(
                SeenErrorKind::Tooling,
                format!(
                    "{} is not formatted; rerun with --in-place to apply changes",
                    input.display()
                ),
            ));
        }
    } else if in_place {
        // Write back to the original file
        fs::write(input, formatted_code).map_err(|err| {
            SeenError::new(
                SeenErrorKind::Io,
                format!(
                    "Failed to write formatted code to {}: {}",
                    input.display(),
                    err
                ),
            )
        })?;
        println!("Formatted {} in place", input.display());
    } else {
        // Output to stdout
        println!("{}", formatted_code);
    }

    Ok(())
}

fn run_tests(path: Option<&Path>) -> SeenResult<()> {
    let base_path = path.unwrap_or_else(|| Path::new("."));

    // Discover test files
    let test_files = discover_test_files(base_path)?;

    if test_files.is_empty() {
        println!("No test files found in {}", base_path.display());
        return Ok(());
    }

    println!(
        "Running {} test file(s) from {}",
        test_files.len(),
        base_path.display()
    );

    // Create keyword manager
    let mut keyword_manager = KeywordManager::new();
    keyword_manager.load_from_toml("en").map_err(|err| {
        SeenError::new(
            SeenErrorKind::Tooling,
            format!("Failed to load keywords: {}", err),
        )
    })?;
    keyword_manager.switch_language("en").map_err(|err| {
        SeenError::new(
            SeenErrorKind::Tooling,
            format!("Failed to switch to English keywords: {}", err),
        )
    })?;

    let keyword_manager = Arc::new(keyword_manager);

    let mut total_tests = 0;
    let mut passed_tests = 0;
    let mut failed_tests = 0;

    // Run each test file
    for test_file in test_files {
        println!("\n=== Running tests in {} ===", test_file.display());

        match run_test_file(&test_file, keyword_manager.clone()) {
            Ok((passed, failed)) => {
                total_tests += passed + failed;
                passed_tests += passed;
                failed_tests += failed;

                if failed > 0 {
                    println!("❌ {} passed, {} failed", passed, failed);
                } else {
                    println!("✅ All {} tests passed", passed);
                }
            }
            Err(e) => {
                println!("❌ Error running test file: {}", e);
                failed_tests += 1;
                total_tests += 1;
            }
        }
    }

    // Print summary
    println!("\n=== Test Summary ===");
    println!(
        "Total: {}, Passed: {}, Failed: {}",
        total_tests, passed_tests, failed_tests
    );

    if failed_tests > 0 {
        std::process::exit(1);
    }

    Ok(())
}

fn parse_file(input: &Path, format: &str, keyword_manager: Arc<KeywordManager>) -> SeenResult<()> {
    // Read source file
    let source = fs::read_to_string(input).map_err(|err| {
        SeenError::new(
            SeenErrorKind::Io,
            format!("Failed to read source file {}: {}", input.display(), err),
        )
    })?;

    // Lex and parse
    let (_project_config, lexer_config) = project_context_for(input)?;
    let visibility_policy = lexer_config.visibility_policy;
    let lexer = Lexer::with_config(source, keyword_manager, lexer_config);
    let mut parser = SeenParser::new_with_visibility(lexer, visibility_policy);
    let ast = parser.parse_program().map_err(SeenError::from)?;

    // Output AST
    match format {
        "json" => {
            let json = serde_json::to_string_pretty(&ast).map_err(|err| {
                SeenError::new(
                    SeenErrorKind::Tooling,
                    format!("Failed to serialize AST to JSON: {}", err),
                )
            })?;
            println!("{}", json);
        }
        "pretty" | _ => {
            println!("{:#?}", ast);
        }
    }

    Ok(())
}

fn lex_file(input: &Path, keyword_manager: Arc<KeywordManager>) -> SeenResult<()> {
    // Read source file
    let source = fs::read_to_string(input).map_err(|err| {
        SeenError::new(
            SeenErrorKind::Io,
            format!("Failed to read source file {}: {}", input.display(), err),
        )
    })?;

    // Lex the source
    let (_project_config, lexer_config) = project_context_for(input)?;
    let mut lexer = Lexer::with_config(source, keyword_manager, lexer_config);
    let mut tokens = Vec::new();

    loop {
        let token = lexer.next_token().map_err(SeenError::from)?;

        let is_eof = matches!(token.token_type, TokenType::EOF);
        tokens.push(token);

        if is_eof {
            break;
        }
    }

    // Output tokens
    for token in tokens {
        println!("{:?}", token);
    }

    Ok(())
}

/// Format a parsed Seen program back to well-formatted source code
fn format_program(program: &Program) -> String {
    let mut formatter = CodeFormatter::new();
    formatter.format_program(program)
}

/// Code formatter that converts AST back to formatted source code
struct CodeFormatter {
    indent_level: usize,
    output: String,
}

impl CodeFormatter {
    fn new() -> Self {
        Self {
            indent_level: 0,
            output: String::new(),
        }
    }

    fn format_program(&mut self, program: &Program) -> String {
        for (idx, expression) in program.expressions.iter().enumerate() {
            if idx > 0 {
                self.output.push('\n');
            }
            self.format_expression(expression);
        }
        self.output.clone()
    }

    fn format_expression(&mut self, expr: &Expression) -> &mut Self {
        match expr {
            Expression::Function {
                name,
                params,
                return_type,
                body,
                is_async,
                receiver,
                ..
            } => {
                self.add_indent();

                if *is_async {
                    self.output.push_str("async ");
                }

                self.output.push_str("fun ");

                if let Some(recv) = receiver {
                    self.output.push('(');
                    self.output.push_str(&recv.name);
                    self.output.push_str(": ");
                    if recv.is_mutable {
                        self.output.push_str("inout ");
                    }
                    self.output.push_str(&recv.type_name);
                    self.output.push_str(") ");
                }

                self.output.push_str(name);
                self.output.push('(');

                for (i, param) in params.iter().enumerate() {
                    if i > 0 {
                        self.output.push_str(", ");
                    }
                    self.output.push_str(&param.name);
                    self.output.push_str(": ");
                    if let Some(type_ann) = &param.type_annotation {
                        self.format_type(type_ann);
                    }
                    if let Some(default) = &param.default_value {
                        self.output.push_str(" = ");
                        self.format_expression(default);
                    }
                }

                self.output.push(')');

                if let Some(ret_type) = return_type {
                    self.output.push_str(": ");
                    self.format_type(ret_type);
                }

                self.output.push_str(" {\n");
                self.indent_level += 1;
                self.format_expression(body);
                self.indent_level -= 1;
                self.output.push('\n');
                self.add_indent();
                self.output.push('}');
            }

            Expression::Block { expressions, .. } => {
                for (i, expr) in expressions.iter().enumerate() {
                    if i > 0 {
                        self.output.push('\n');
                    }
                    self.format_expression(expr);
                }
            }

            Expression::Let {
                name,
                type_annotation,
                value,
                is_mutable,
                ..
            } => {
                self.add_indent();
                self.output.push_str("let ");
                if *is_mutable {
                    self.output.push_str("mut ");
                }
                self.output.push_str(name);
                if let Some(type_ann) = type_annotation {
                    self.output.push_str(": ");
                    self.format_type(type_ann);
                }
                self.output.push_str(" = ");
                self.format_expression(value);
            }

            Expression::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                self.add_indent();
                self.output.push_str("if ");
                self.format_expression_prec(condition, precedence::PRIMARY);
                self.output.push_str(" {\n");
                self.indent_level += 1;
                self.format_expression(then_branch);
                self.indent_level -= 1;
                self.output.push('\n');
                self.add_indent();
                self.output.push('}');

                if let Some(else_expr) = else_branch {
                    self.output.push_str(" else {\n");
                    self.indent_level += 1;
                    self.format_expression(else_expr);
                    self.indent_level -= 1;
                    self.output.push('\n');
                    self.add_indent();
                    self.output.push('}');
                }
            }

            Expression::Assignment { target, value, .. } => {
                self.add_indent();
                self.format_expression_prec(target, precedence::LOGICAL_OR);
                self.output.push_str(" = ");
                self.format_expression_prec(value, precedence::LOGICAL_OR);
            }

            Expression::Return { value, .. } => {
                self.add_indent();
                self.output.push_str("return");
                if let Some(val) = value {
                    self.output.push(' ');
                    self.format_expression_prec(val, precedence::LOGICAL_OR);
                }
            }

            _ => {
                self.add_indent();
                self.format_expression_prec(expr, precedence::PRIMARY);
            }
        }

        self
    }

    fn format_expression_prec(&mut self, expr: &Expression, parent_prec: u8) -> &mut Self {
        match expr {
            Expression::BinaryOp {
                left, op, right, ..
            } => self.format_binary_expression(left, op, right, parent_prec),
            Expression::Elvis {
                nullable, default, ..
            } => self.format_elvis_expression(nullable, default, parent_prec),
            Expression::UnaryOp { op, operand, .. } => {
                self.format_unary_expression(op, operand, parent_prec)
            }
            Expression::Call { callee, args, .. } => {
                self.format_call_expression(callee, args, parent_prec)
            }
            Expression::Await { expr, .. } => {
                let prec = precedence::UNARY;
                let needs_paren = prec < parent_prec;
                if needs_paren {
                    self.output.push('(');
                }
                self.output.push_str("await ");
                self.format_expression_prec(expr, prec);
                if needs_paren {
                    self.output.push(')');
                }
                self
            }
            Expression::Spawn { expr, detached, .. } => {
                self.output.push_str("spawn ");
                if *detached {
                    self.output.push_str("detached ");
                }
                self.format_expression_prec(expr, precedence::PRIMARY);
                self
            }
            Expression::Scope { body, .. } => {
                self.output.push_str("scope ");
                self.format_expression_prec(body, precedence::PRIMARY);
                self
            }
            Expression::Cancel { task, .. } => {
                self.output.push_str("cancel ");
                self.format_expression_prec(task, precedence::UNARY);
                self
            }
            Expression::ParallelFor {
                binding,
                iterable,
                body,
                ..
            } => {
                self.output.push_str("parallel_for ");
                self.output.push_str(binding);
                self.output.push_str(" in ");
                self.format_expression_prec(iterable, precedence::LOGICAL_OR);
                self.output.push(' ');
                self.format_expression_prec(body, precedence::PRIMARY);
                self
            }
            Expression::MemberAccess {
                object,
                member,
                is_safe,
                ..
            } => self.format_member_access(object, member, *is_safe, parent_prec),
            Expression::IndexAccess { object, index, .. } => {
                self.format_index_expression(object, index, parent_prec)
            }
            Expression::ForceUnwrap { nullable, .. } => {
                let prec = precedence::CALL;
                let needs_paren = prec < parent_prec;
                if needs_paren {
                    self.output.push('(');
                }
                self.format_expression_prec(nullable, prec);
                self.output.push_str("!!");
                if needs_paren {
                    self.output.push(')');
                }
                self
            }
            Expression::Identifier { name, .. } => {
                self.output.push_str(name);
                self
            }
            Expression::StringLiteral { value, .. } => {
                self.output.push('"');
                self.output.push_str(value);
                self.output.push('"');
                self
            }
            Expression::IntegerLiteral { value, .. } => {
                self.output.push_str(&value.to_string());
                self
            }
            Expression::FloatLiteral { value, .. } => {
                self.output.push_str(&value.to_string());
                self
            }
            Expression::BooleanLiteral { value, .. } => {
                self.output.push_str(if *value { "true" } else { "false" });
                self
            }
            Expression::NullLiteral { .. } => {
                self.output.push_str("null");
                self
            }
            Expression::ArrayLiteral { elements, .. } => {
                self.output.push('[');
                for (i, element) in elements.iter().enumerate() {
                    if i > 0 {
                        self.output.push_str(", ");
                    }
                    self.format_expression_prec(element, precedence::PRIMARY);
                }
                self.output.push(']');
                self
            }
            Expression::CharLiteral { value, .. } => {
                self.output.push('\'');
                self.output.push(*value);
                self.output.push('\'');
                self
            }
            Expression::Comptime { body, .. } => {
                self.output.push_str("comptime ");
                self.format_expression_prec(body, precedence::PRIMARY);
                self
            }
            Expression::Block { expressions, .. } => {
                self.output.push_str("{\n");
                self.indent_level += 1;
                for (idx, expression) in expressions.iter().enumerate() {
                    if idx > 0 {
                        self.output.push('\n');
                    }
                    self.format_expression(expression);
                }
                self.indent_level -= 1;
                self.output.push('\n');
                self.add_indent();
                self.output.push('}');
                self
            }
            Expression::Assignment { target, value, .. } => self.format_binary_like(
                target,
                value,
                parent_prec,
                precedence::LOGICAL_OR,
                "=",
                true,
                false,
            ),
            _ => {
                self.output
                    .push_str(&format!("/* {} */", std::any::type_name::<Expression>()));
                self
            }
        }
    }

    fn format_binary_expression(
        &mut self,
        left: &Expression,
        op: &BinaryOperator,
        right: &Expression,
        parent_prec: u8,
    ) -> &mut Self {
        let symbol = op.symbol();
        let prec = op.precedence();
        let needs_paren = prec < parent_prec;
        if needs_paren {
            self.output.push('(');
        }
        self.format_child_expression(left, prec, ChildSide::Left, op.is_right_associative());
        if op.requires_spacing() {
            self.output.push(' ');
            self.output.push_str(symbol);
            self.output.push(' ');
        } else {
            self.output.push_str(symbol);
        }
        self.format_child_expression(right, prec, ChildSide::Right, op.is_right_associative());
        if needs_paren {
            self.output.push(')');
        }
        self
    }

    fn format_elvis_expression(
        &mut self,
        nullable: &Expression,
        default: &Expression,
        parent_prec: u8,
    ) -> &mut Self {
        let prec = precedence::ELVIS;
        let needs_paren = prec < parent_prec;
        if needs_paren {
            self.output.push('(');
        }
        self.format_child_expression(nullable, prec, ChildSide::Left, false);
        self.output.push_str(" ?: ");
        self.format_child_expression(default, prec, ChildSide::Right, false);
        if needs_paren {
            self.output.push(')');
        }
        self
    }

    fn format_unary_expression(
        &mut self,
        op: &UnaryOperator,
        operand: &Expression,
        parent_prec: u8,
    ) -> &mut Self {
        let prec = precedence::UNARY;
        let needs_paren = prec < parent_prec;
        if needs_paren {
            self.output.push('(');
        }
        self.output.push_str(op.symbol());
        if op.requires_trailing_space() {
            self.output.push(' ');
        }
        let operand_prec = Self::expression_precedence(operand);
        let child_parent = if operand_prec < prec {
            operand_prec
        } else {
            prec
        };
        self.format_expression_prec(operand, child_parent);
        if needs_paren {
            self.output.push(')');
        }
        self
    }

    fn format_call_expression(
        &mut self,
        callee: &Expression,
        args: &[Expression],
        parent_prec: u8,
    ) -> &mut Self {
        let prec = precedence::CALL;
        let needs_paren = prec < parent_prec;
        if needs_paren {
            self.output.push('(');
        }

        let callee_prec = Self::expression_precedence(callee);
        let callee_parent = if callee_prec < prec {
            callee_prec
        } else {
            prec
        };
        self.format_expression_prec(callee, callee_parent);

        self.output.push('(');
        for (idx, arg) in args.iter().enumerate() {
            if idx > 0 {
                self.output.push_str(", ");
            }
            self.format_expression_prec(arg, precedence::PRIMARY);
        }
        self.output.push(')');

        if needs_paren {
            self.output.push(')');
        }
        self
    }

    fn format_member_access(
        &mut self,
        object: &Expression,
        member: &str,
        is_safe: bool,
        parent_prec: u8,
    ) -> &mut Self {
        let prec = precedence::CALL;
        let needs_paren = prec < parent_prec;
        if needs_paren {
            self.output.push('(');
        }

        let object_prec = Self::expression_precedence(object);
        let object_parent = if object_prec < prec {
            object_prec
        } else {
            prec
        };
        self.format_expression_prec(object, object_parent);

        if is_safe {
            self.output.push_str("?.");
        } else {
            self.output.push('.');
        }
        self.output.push_str(member);

        if needs_paren {
            self.output.push(')');
        }
        self
    }

    fn format_index_expression(
        &mut self,
        object: &Expression,
        index: &Expression,
        parent_prec: u8,
    ) -> &mut Self {
        let prec = precedence::CALL;
        let needs_paren = prec < parent_prec;
        if needs_paren {
            self.output.push('(');
        }

        let object_prec = Self::expression_precedence(object);
        let object_parent = if object_prec < prec {
            object_prec
        } else {
            prec
        };
        self.format_expression_prec(object, object_parent);

        self.output.push('[');
        self.format_expression_prec(index, precedence::PRIMARY);
        self.output.push(']');

        if needs_paren {
            self.output.push(')');
        }
        self
    }

    fn format_binary_like(
        &mut self,
        left: &Expression,
        right: &Expression,
        parent_prec: u8,
        prec: u8,
        symbol: &str,
        spaced: bool,
        right_associative: bool,
    ) -> &mut Self {
        let needs_paren = prec < parent_prec;
        if needs_paren {
            self.output.push('(');
        }
        self.format_child_expression(left, prec, ChildSide::Left, right_associative);
        if spaced {
            self.output.push(' ');
            self.output.push_str(symbol);
            self.output.push(' ');
        } else {
            self.output.push_str(symbol);
        }
        self.format_child_expression(right, prec, ChildSide::Right, right_associative);
        if needs_paren {
            self.output.push(')');
        }
        self
    }

    fn format_child_expression(
        &mut self,
        expr: &Expression,
        parent_prec: u8,
        side: ChildSide,
        parent_is_right_associative: bool,
    ) {
        let child_prec = Self::expression_precedence(expr);
        let needs_paren = match side {
            ChildSide::Left => {
                child_prec < parent_prec
                    || (child_prec == parent_prec && parent_is_right_associative)
            }
            ChildSide::Right => {
                child_prec < parent_prec
                    || (child_prec == parent_prec && !parent_is_right_associative)
            }
        };

        let next_parent = if needs_paren { child_prec } else { parent_prec };

        if needs_paren {
            self.output.push('(');
        }
        self.format_expression_prec(expr, next_parent);
        if needs_paren {
            self.output.push(')');
        }
    }

    fn expression_precedence(expr: &Expression) -> u8 {
        match expr {
            Expression::BinaryOp { op, .. } => op.precedence(),
            Expression::Elvis { .. } => precedence::ELVIS,
            Expression::UnaryOp { .. } => precedence::UNARY,
            Expression::Call { .. }
            | Expression::MemberAccess { .. }
            | Expression::IndexAccess { .. }
            | Expression::ForceUnwrap { .. } => precedence::CALL,
            Expression::Assignment { .. } => precedence::LOGICAL_OR,
            Expression::ArrayLiteral { .. }
            | Expression::Identifier { .. }
            | Expression::IntegerLiteral { .. }
            | Expression::FloatLiteral { .. }
            | Expression::BooleanLiteral { .. }
            | Expression::StringLiteral { .. }
            | Expression::CharLiteral { .. }
            | Expression::NullLiteral { .. } => precedence::PRIMARY,
            _ => precedence::PRIMARY,
        }
    }

    fn add_indent(&mut self) {
        for _ in 0..self.indent_level {
            self.output.push_str("    "); // 4 spaces per indent level
        }
    }

    fn format_type(&mut self, type_info: &Type) -> &mut Self {
        self.output.push_str(&type_info.name);
        if type_info.is_nullable {
            self.output.push('?');
        }
        if !type_info.generics.is_empty() {
            self.output.push('<');
            for (i, generic) in type_info.generics.iter().enumerate() {
                if i > 0 {
                    self.output.push_str(", ");
                }
                self.format_type(generic);
            }
            self.output.push('>');
        }
        self
    }
}

#[derive(Copy, Clone)]
enum ChildSide {
    Left,
    Right,
}

/// Discover test files in the given directory tree
fn discover_test_files(base_path: &Path) -> SeenResult<Vec<PathBuf>> {
    let mut test_files = Vec::new();

    if base_path.is_file() && is_test_file(base_path) {
        test_files.push(base_path.to_path_buf());
        return Ok(test_files);
    }

    if base_path.is_dir() {
        // Recursively search for test files
        search_test_files_recursive(base_path, &mut test_files)?;
    }

    // Sort for consistent ordering
    test_files.sort();
    Ok(test_files)
}

/// Check if a file is a test file (ends with _test.seen or test_*.seen)
fn is_test_file(path: &Path) -> bool {
    if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
        if let Some(extension) = path.extension().and_then(|e| e.to_str()) {
            return extension == "seen"
                && (file_name.ends_with("_test.seen") || file_name.starts_with("test_"));
        }
    }
    false
}

/// Recursively search for test files
fn search_test_files_recursive(dir: &Path, test_files: &mut Vec<PathBuf>) -> SeenResult<()> {
    for entry in fs::read_dir(dir).map_err(SeenError::from)? {
        let entry = entry.map_err(SeenError::from)?;
        let path = entry.path();

        if path.is_dir() {
            search_test_files_recursive(&path, test_files)?;
        } else if is_test_file(&path) {
            test_files.push(path);
        }
    }
    Ok(())
}

/// Run tests in a single file
fn run_test_file(
    test_file: &Path,
    keyword_manager: Arc<KeywordManager>,
) -> SeenResult<(usize, usize)> {
    // Read the test file
    let source = fs::read_to_string(test_file).map_err(|err| {
        SeenError::new(
            SeenErrorKind::Io,
            format!("Failed to read test file {}: {}", test_file.display(), err),
        )
    })?;

    // Parse the test file
    let (_project_config, lexer_config) = project_context_for(test_file)?;
    let visibility_policy = lexer_config.visibility_policy;
    let lexer = Lexer::with_config(source, keyword_manager.clone(), lexer_config);
    let mut parser = SeenParser::new_with_visibility(lexer, visibility_policy);
    let program = parser.parse_program().map_err(|err| SeenError::from(err))?;

    // Extract test functions (functions that start with "test_" or end with "_test")
    let test_functions = extract_test_functions(&program);

    if test_functions.is_empty() {
        println!("No test functions found in {}", test_file.display());
        return Ok((0, 0));
    }

    let mut passed = 0;
    let mut failed = 0;

    // Create an interpreter to run the tests
    let mut interpreter = Interpreter::new();

    // Run each test function
    for test_func in test_functions {
        let func_name = get_function_name(test_func);
        print!("  {} ... ", func_name);
        io::stdout().flush().unwrap();

        match run_single_test(&mut interpreter, test_func, &program) {
            Ok(_) => {
                println!("✅ PASS");
                passed += 1;
            }
            Err(e) => {
                println!("❌ FAIL: {}", e);
                failed += 1;
            }
        }
    }

    Ok((passed, failed))
}

/// Extract test functions from a program
fn extract_test_functions(program: &Program) -> Vec<&Expression> {
    program
        .expressions
        .iter()
        .filter_map(|expr| {
            if let Expression::Function { name, .. } = expr {
                if name.starts_with("test_") || name.ends_with("_test") {
                    Some(expr)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect()
}

/// Run a single test function
fn run_single_test(
    interpreter: &mut Interpreter,
    test_func: &Expression,
    _program: &Program,
) -> SeenResult<()> {
    // Create a test program that just calls this test function
    let test_program = Program {
        expressions: vec![
            test_func.clone(),
            Expression::Call {
                callee: Box::new(Expression::Identifier {
                    name: get_function_name(test_func),
                    is_public: false,
                    pos: Position {
                        line: 1,
                        column: 1,
                        offset: 0,
                    },
                }),
                args: vec![],
                pos: Position {
                    line: 1,
                    column: 1,
                    offset: 0,
                },
            },
        ],
    };

    // Run the test
    match interpreter.interpret(&test_program) {
        Ok(_) => Ok(()),
        Err(e) => Err(SeenError::from(e)),
    }
}

/// Get the name of a function from an Expression::Function
fn get_function_name(expr: &Expression) -> String {
    if let Expression::Function { name, .. } = expr {
        name.clone()
    } else {
        "unknown".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use seen_core::ir::IRValue;
    use std::fs;
    use tempfile::tempdir;

    #[cfg(feature = "llvm")]
    mod android_bundle_tests {
        use super::*;
        use std::fs::File;
        use std::io::Read;
        use zip::ZipArchive;

        #[test]
        fn bundle_inserts_stub_dex_when_missing() {
            let temp = tempdir().expect("temp dir");
            let project_dir = temp.path();

            let shared_lib = project_dir.join("libseen_stub.so");
            fs::write(&shared_lib, b"fake so").expect("write shared lib");
            let bundle_path = project_dir.join("app.aab");

            bundle_android_artifacts(
                &shared_lib,
                &bundle_path,
                project_dir,
                Some("aarch64-linux-android"),
            )
                .expect("bundle");

            let file = File::open(&bundle_path).expect("open bundle");
            let mut archive = ZipArchive::new(file).expect("zip archive");

            let mut manifest = String::new();
            archive
                .by_name("base/manifest/AndroidManifest.xml")
                .expect("manifest entry")
                .read_to_string(&mut manifest)
                .expect("read manifest");
            assert!(
                manifest.contains("SeenApp"),
                "expected default manifest, got {}",
                manifest
            );

            let mut dex_bytes = Vec::new();
            archive
                .by_name("base/dex/classes.dex")
                .expect("dex entry")
                .read_to_end(&mut dex_bytes)
                .expect("read dex");
            assert_eq!(
                dex_bytes, STUB_CLASSES_DEX,
                "stub dex should be inserted when project provides none"
            );
        }

        #[test]
        fn bundle_copies_project_directories() {
            let temp = tempdir().expect("temp dir");
            let project_dir = temp.path();

            // Stub shared object
            let shared_lib = project_dir.join("libproj.so");
            fs::write(&shared_lib, b"binary").expect("write shared lib");
            let bundle_path = project_dir.join("project.aab");

            // Custom manifest override
            let manifest_path = project_dir.join("AndroidManifest.xml");
            fs::write(
                &manifest_path,
                r#"<manifest xmlns:android="http://schemas.android.com/apk/res/android"
    package="com.example.custom">
    <application android:label="CustomApp" />
</manifest>
"#,
            )
                .expect("write manifest");

            // Assets, resources, root, and dex directories
            let asset_path = project_dir.join("assets").join("textures").join("quad.png");
            fs::create_dir_all(asset_path.parent().unwrap()).expect("asset dirs");
            fs::write(&asset_path, b"PNGDATA").expect("write asset");

            let res_path = project_dir.join("res").join("values").join("strings.xml");
            fs::create_dir_all(res_path.parent().unwrap()).expect("res dirs");
            fs::write(
                &res_path,
                "<resources><string name=\"app\">Custom</string></resources>",
            )
                .expect("write res");

            let root_path = project_dir.join("root").join("NOTICE.txt");
            fs::create_dir_all(root_path.parent().unwrap()).expect("root dirs");
            fs::write(&root_path, "root file").expect("write root");

            let dex_dir = project_dir.join("dex");
            fs::create_dir_all(&dex_dir).expect("dex dir");
            let dex_path = dex_dir.join("classes.dex");
            let custom_dex = b"custom dex bytes";
            fs::write(&dex_path, custom_dex).expect("write dex");

            bundle_android_artifacts(
                &shared_lib,
                &bundle_path,
                project_dir,
                Some("x86_64-linux-android"),
            )
                .expect("bundle");

            let file = File::open(&bundle_path).expect("open bundle");
            let mut archive = ZipArchive::new(file).expect("zip archive");

            // Manifest override should be present
            let mut manifest = String::new();
            archive
                .by_name("base/manifest/AndroidManifest.xml")
                .expect("manifest entry")
                .read_to_string(&mut manifest)
                .expect("read manifest");
            assert!(
                manifest.contains("CustomApp"),
                "expected custom manifest, got {}",
                manifest
            );

            // Asset contents propagate
            let mut asset_bytes = Vec::new();
            archive
                .by_name("base/assets/textures/quad.png")
                .expect("asset entry")
                .read_to_end(&mut asset_bytes)
                .expect("read asset");
            assert_eq!(asset_bytes, b"PNGDATA");

            // Resource file present
            let mut res_bytes = String::new();
            archive
                .by_name("base/res/values/strings.xml")
                .expect("res entry")
                .read_to_string(&mut res_bytes)
                .expect("read res");
            assert!(
                res_bytes.contains("Custom"),
                "expected resource contents, got {}",
                res_bytes
            );

            // Custom dex should override stub
            let mut dex_bytes = Vec::new();
            archive
                .by_name("base/dex/classes.dex")
                .expect("dex entry")
                .read_to_end(&mut dex_bytes)
                .expect("read dex");
            assert_eq!(dex_bytes, custom_dex);
        }
    }

    fn make_embed_const(path: AttributeValue) -> Expression {
        Expression::Const {
            name: "DATA".to_string(),
            type_annotation: None,
            value: Box::new(Expression::IntegerLiteral {
                value: 0,
                pos: Position::new(0, 0, 0),
            }),
            attributes: vec![Attribute {
                name: "embed".to_string(),
                args: vec![AttributeArgument::Named {
                    name: "path".to_string(),
                    value: path,
                }],
                pos: Position::new(0, 0, 0),
            }],
            pos: Position::new(0, 0, 0),
        }
    }

    #[test]
    fn rewrite_embed_attribute_resolves_relative_path() {
        let temp_dir = tempdir().expect("create temp dir");
        let asset_path = temp_dir.path().join("asset.bin");
        fs::write(&asset_path, [0u8; 4]).expect("write asset");

        let expr = make_embed_const(AttributeValue::String("asset.bin".to_string()));
        let rewritten = rewrite_embed_attributes(expr, temp_dir.path());

        match rewritten {
            Expression::Const { attributes, .. } => {
                let path_arg = attributes
                    .first()
                    .and_then(|attr| {
                        attr.args.iter().find_map(|arg| match arg {
                            AttributeArgument::Named { name, value } if name == "path" => {
                                if let AttributeValue::String(path) = value {
                                    Some(path.clone())
                                } else {
                                    None
                                }
                            }
                            _ => None,
                        })
                    })
                    .expect("path argument present");

                let expected = asset_path
                    .canonicalize()
                    .expect("canonicalize asset")
                    .to_string_lossy()
                    .to_string();

                assert_eq!(path_arg, expected);
            }
            other => panic!("expected const expression, got {:?}", other),
        }
    }

    #[test]
    fn ir_generator_produces_byte_array_for_embed_const() {
        let temp_dir = tempdir().expect("create temp dir");
        let asset_path = temp_dir.path().join("embed.bin");
        let bytes = vec![0xAA, 0xBB, 0xCC];
        fs::write(&asset_path, &bytes).expect("write bytes");

        let absolute = asset_path
            .canonicalize()
            .expect("canonicalize asset")
            .to_string_lossy()
            .to_string();
        let expr = make_embed_const(AttributeValue::String(absolute));

        let mut generator = IRGenerator::new();
        let (value, instructions) = generator
            .generate_expression(&expr)
            .expect("generate expression");

        match value {
            IRValue::ByteArray(data) => assert_eq!(data, bytes),
            other => panic!("expected byte array, got {:?}", other),
        }

        assert!(
            !instructions.is_empty(),
            "expected store instruction for embedded constant"
        );
    }

    #[test]
    fn default_shared_path_uses_platform_extension() {
        let path = default_link_output_path(BuildLinkMode::SharedLibrary, None);
        let name = path.to_string_lossy();
        #[cfg(target_os = "macos")]
        assert!(name.ends_with(".dylib"));
        #[cfg(target_os = "windows")]
        assert!(name.ends_with(".dll"));
        #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
        assert!(name.ends_with(".so"));
    }

    #[test]
    fn default_static_path_uses_platform_extension() {
        let path = default_link_output_path(BuildLinkMode::StaticLibrary, None);
        let name = path.to_string_lossy();
        #[cfg(target_os = "windows")]
        assert!(name.ends_with(".lib"));
        #[cfg(not(target_os = "windows"))]
        assert!(name.ends_with(".a"));
    }

    #[test]
    fn wasm_target_defaults_to_wasm_extension() {
        let path =
            default_link_output_path(BuildLinkMode::Executable, Some("wasm32-unknown-unknown"));
        assert_eq!(path.to_string_lossy(), "module.wasm");
    }

    #[test]
    fn android_targets_default_to_shared_library_paths() {
        let path =
            default_link_output_path(BuildLinkMode::SharedLibrary, Some("aarch64-linux-android"));
        assert!(path.to_string_lossy().ends_with(".so"));
    }

    #[test]
    fn emit_wasm_loader_writes_js_and_html() {
        let temp_dir = tempdir().expect("temp dir");
        let wasm_path = temp_dir.path().join("hello.wasm");
        fs::write(&wasm_path, b"").expect("touch wasm file");

        emit_wasm_loader(&wasm_path).expect("emit loader");

        let js = wasm_path.with_extension("js");
        let html = wasm_path.with_extension("html");
        assert!(js.exists(), "js loader should exist");
        assert!(html.exists(), "html bootstrap should exist");

        let js_contents = fs::read_to_string(js).expect("read js");
        assert!(js_contents.contains("hello.wasm"));
    }
}
