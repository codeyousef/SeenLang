//! Seen Language Command Line Interface
//!
//! This is the main entry point for the Seen language compiler and toolchain.

use clap::{Parser, Subcommand, ValueEnum};
use seen_core::{
    precedence, BinaryOperator, Expression, IRGenerator, IROptimizer, Interpreter, KeywordManager,
    Lexer, LexerConfig, MemoryManager, OptimizationLevel, Position, Program, SeenError,
    SeenErrorKind, SeenParser, SeenResult, TokenType, Type, TypeChecker, UnaryOperator, Value,
    VisibilityPolicy,
};
use serde::Deserialize;
use std::collections::{HashSet, VecDeque};
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

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

        /// Output file path (default: a.ir for IR, a.out for LLVM)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Optimization level
        #[arg(short = 'O', long, default_value = "0")]
        opt_level: u8,

        /// Backend to use: "ir" (emit textual IR) or "llvm" (native binary via LLVM)
        #[arg(long, default_value = "ir")]
        backend: String,

        /// Emit LLVM IR (.ll) instead of linking an executable (only with --backend llvm)
        #[arg(long)]
        emit_ll: bool,
    },

    /// Generate IR twice and compare SHA-256 hashes (determinism check)
    Determinism {
        /// Input file to evaluate
        #[arg(value_name = "FILE")]
        input: PathBuf,

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

    /// Generate IR and show it
    Ir {
        /// Input file to generate IR for
        #[arg(value_name = "FILE")]
        input: PathBuf,

        /// Optimization level
        #[arg(short = 'O', long, default_value = "0")]
        opt_level: u8,
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

#[derive(Debug, Deserialize, Default)]
struct ProjectConfig {
    #[serde(default)]
    project: ProjectSection,
    #[serde(default)]
    visibility: Option<VisibilityConfig>,
}

#[derive(Debug, Deserialize, Default)]
struct ProjectSection {
    #[serde(default)]
    visibility: Option<VisibilityMode>,
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
}

fn lexer_config_for(path: &Path) -> SeenResult<LexerConfig> {
    let project_config = load_project_config(path)?;
    Ok(LexerConfig {
        visibility_policy: project_config.visibility_policy(),
    })
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
            let config: ProjectConfig = toml::from_str(&content).map_err(|err| {
                SeenError::new(
                    SeenErrorKind::Tooling,
                    format!(
                        "Failed to parse project config {}: {}",
                        candidate.display(),
                        err
                    ),
                )
            })?;
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
        }) => {
            if backend == "llvm" {
                compile_file_llvm(&input, output.as_ref(), opt_level, emit_ll, keyword_manager)?;
            } else {
                compile_file(&input, output.as_ref(), opt_level, keyword_manager)?;
            }
        }

        Some(Commands::Run { input, args }) => {
            run_file(&input, &args, keyword_manager)?;
        }

        Some(Commands::Check { input }) => {
            check_file(&input, keyword_manager)?;
        }

        Some(Commands::Ir { input, opt_level }) => {
            generate_ir(&input, opt_level, keyword_manager)?;
        }

        Some(Commands::Repl {
            show_ast,
            show_types,
        }) => {
            run_repl(keyword_manager, show_ast, show_types)?;
        }

        Some(Commands::Format { input, in_place }) => {
            format_file(&input, in_place)?;
        }

        Some(Commands::Test { path }) => {
            run_tests(path.as_deref())?;
        }

        Some(Commands::Parse { input, format }) => {
            parse_file(&input, &format, keyword_manager)?;
        }

        Some(Commands::Determinism { input, opt_level }) => {
            determinism_check(&input, opt_level, keyword_manager)?;
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

fn compile_file(
    input: &Path,
    output: Option<&PathBuf>,
    opt_level: u8,
    keyword_manager: Arc<KeywordManager>,
) -> SeenResult<()> {
    println!(
        "Compiling {} with optimization level {}",
        input.display(),
        opt_level
    );

    // Read source file
    let source = fs::read_to_string(input).map_err(|err| {
        SeenError::new(
            SeenErrorKind::Io,
            format!("Failed to read source file {}: {}", input.display(), err),
        )
    })?;

    let lexer_config = lexer_config_for(input)?;
    let visibility_policy = lexer_config.visibility_policy;

    // Lex and parse
    let lexer = Lexer::with_config(source.clone(), keyword_manager.clone(), lexer_config);
    let mut parser = SeenParser::new_with_visibility(lexer, visibility_policy);
    let ast_parsed = parser.parse_program().map_err(SeenError::from)?;

    // Bundle imports (merge imported modules into a single program)
    let ast = bundle_imports(
        ast_parsed,
        input,
        keyword_manager.clone(),
        visibility_policy,
    )?;

    // Bootstrap mode removed (C backend is gone)
    let bootstrap_mode = false;

    // Type check
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

    // Memory analysis
    let mut memory_manager = MemoryManager::new();
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

    // Apply memory optimizations
    for optimization in memory_result.get_optimizations() {
        println!("Memory optimization: {}", optimization);
    }

    // Generate IR
    let mut ir_generator = IRGenerator::new();
    let ir_program = ir_generator.generate(&ast).map_err(SeenError::from)?;

    println!("Generated IR with {} modules", ir_program.modules.len());

    // Optimize IR
    let optimization_level = OptimizationLevel::from_level(opt_level);
    let mut optimizer = IROptimizer::new(optimization_level);
    let mut optimized_ir = ir_program;
    optimizer
        .optimize_program(&mut optimized_ir)
        .map_err(SeenError::from)?;

    println!("Applied optimization level {}", opt_level);

    // Emit IR text as build artifact
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

fn compile_file_llvm(
    input: &Path,
    output: Option<&PathBuf>,
    opt_level: u8,
    emit_ll: bool,
    keyword_manager: Arc<KeywordManager>,
) -> SeenResult<()> {
    println!(
        "Compiling {} with optimization level {} (LLVM)",
        input.display(),
        opt_level
    );
    let source = fs::read_to_string(input).map_err(|err| {
        SeenError::new(
            SeenErrorKind::Io,
            format!("Failed to read source file {}: {}", input.display(), err),
        )
    })?;

    let lexer_config = lexer_config_for(input)?;
    let visibility_policy = lexer_config.visibility_policy;
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
    optimizer
        .optimize_program(&mut optimized_ir)
        .map_err(SeenError::from)?;

    // LLVM codegen
    #[cfg(feature = "llvm")]
    {
        use seen_core::{LinkOutput, LlvmBackend};
        let mut backend = LlvmBackend::new();
        let default_exe = PathBuf::from("a.out");
        let out_path = output.unwrap_or(&default_exe);
        if emit_ll {
            let ll_path = out_path;
            backend
                .emit_llvm_ir(&optimized_ir, ll_path)
                .map_err(|err| {
                    SeenError::new(
                        SeenErrorKind::Ir,
                        format!(
                            "Failed to emit LLVM IR to {}: {:?}",
                            ll_path.display(),
                            err
                        ),
                    )
                })?;
            println!("Generated LLVM IR: {}", ll_path.display());
        } else {
            let target_exe = out_path.clone();
            backend
                .emit_executable(&optimized_ir, &target_exe, LinkOutput::Executable)
                .map_err(|err| {
                    SeenError::new(
                        SeenErrorKind::Ir,
                        format!(
                            "Failed to produce executable {}: {:?}",
                            target_exe.display(),
                            err
                        ),
                    )
                })?;
            println!("Generated executable: {}", target_exe.display());
        }
        Ok(())
    }
    #[cfg(not(feature = "llvm"))]
    {
        let _ = (output, emit_ll);
        Err(SeenError::new(
            SeenErrorKind::Tooling,
            "LLVM backend not enabled at build time. Rebuild with: cargo build -p seen_cli --release --features llvm",
        ))
    }
}

/// Recursively resolve and inline imports by parsing target .seen files
fn bundle_imports(
    program: Program,
    input_path: &Path,
    keyword_manager: Arc<KeywordManager>,
    visibility_policy: VisibilityPolicy,
) -> SeenResult<Program> {
    let base_dir = input_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    let mut queue: VecDeque<Program> = VecDeque::new();
    queue.push_back(program);
    let mut visited: HashSet<String> = HashSet::new();
    let mut merged = Program {
        expressions: Vec::new(),
    };

    while let Some(prog) = queue.pop_front() {
        for expr in prog.expressions {
            match &expr {
                Expression::Import { module_path, .. } => {
                    let key = module_path.join(".");
                    if visited.contains(&key) {
                        continue;
                    }
                    visited.insert(key.clone());
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
                            let parsed = p.parse_program().map_err(|err| SeenError::from(err))?;
                            queue.push_back(parsed);
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
                _ => merged.expressions.push(expr),
            }
        }
    }
    Ok(merged)
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
    let lexer_config = lexer_config_for(input)?;
    let visibility_policy = lexer_config.visibility_policy;
    let lexer = Lexer::with_config(source.clone(), keyword_manager.clone(), lexer_config);
    let mut parser = SeenParser::new_with_visibility(lexer, visibility_policy);
    let parsed = parser.parse_program().map_err(SeenError::from)?;
    // Bundle imports so interpreter sees imported functions
    let ast = bundle_imports(parsed, input, keyword_manager.clone(), visibility_policy)?;

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
    let lexer_config = lexer_config_for(input)?;
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
    let lexer_config = lexer_config_for(input)?;
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
    keyword_manager: Arc<KeywordManager>,
) -> SeenResult<()> {
    use sha2::{Digest, Sha256};
    let lexer_config = lexer_config_for(input)?;
    let visibility_policy = lexer_config.visibility_policy;

    let run_once = |input: &Path| -> SeenResult<String> {
        let source = fs::read_to_string(input).map_err(|err| {
            SeenError::new(
                SeenErrorKind::Io,
                format!("Failed to read source file {}: {}", input.display(), err),
            )
        })?;
        let lexer = Lexer::with_config(source.clone(), keyword_manager.clone(), lexer_config);
        let mut parser = SeenParser::new_with_visibility(lexer, visibility_policy);
        let ast = parser.parse_program().map_err(SeenError::from)?;
        let ast = bundle_imports(ast, input, keyword_manager.clone(), visibility_policy)?;
        let mut type_checker = TypeChecker::new();
        let type_result = type_checker.check_program(&ast);
        if !type_result.errors.is_empty() {
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
        let mut ir_generator = IRGenerator::new();
        let ir_program = ir_generator.generate(&ast).map_err(SeenError::from)?;
        let optimization_level = OptimizationLevel::from_level(opt_level);
        let mut optimizer = IROptimizer::new(optimization_level);
        let mut optimized_ir = ir_program;
        optimizer
            .optimize_program(&mut optimized_ir)
            .map_err(SeenError::from)?;
        Ok(format!("{}", optimized_ir))
    };

    let ir1 = run_once(input)?;
    let ir2 = run_once(input)?;

    let mut h1 = Sha256::new();
    h1.update(ir1.as_bytes());
    let hash1 = format!("{:x}", h1.finalize());

    let mut h2 = Sha256::new();
    h2.update(ir2.as_bytes());
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

fn format_file(input: &Path, in_place: bool) -> SeenResult<()> {
    println!("Formatting {} (in-place: {})", input.display(), in_place);

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
    let lexer_config = lexer_config_for(input)?;

    // Parse the source code
    let visibility_policy = lexer_config.visibility_policy;
    let lexer = Lexer::with_config(source.clone(), Arc::new(keyword_manager), lexer_config);
    let mut parser = SeenParser::new_with_visibility(lexer, visibility_policy);
    let program = parser.parse_program().map_err(|err| SeenError::from(err))?;

    // Format the code
    let formatted_code = format_program(&program);

    if in_place {
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
    let lexer_config = lexer_config_for(input)?;
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
    let lexer_config = lexer_config_for(input)?;
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
    let lexer_config = lexer_config_for(test_file)?;
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
