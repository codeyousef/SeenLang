//! Seen Language Command Line Interface
//! 
//! This is the main entry point for the Seen language compiler and toolchain.

use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::fs;
use std::io::{self, Write, BufRead};
use std::sync::Arc;
use seen_lexer::{Lexer, KeywordManager, TokenType};
use seen_parser::Parser as SeenParser;
use seen_interpreter::Interpreter;
use seen_typechecker::TypeChecker;
use seen_ir::{IRGenerator, IROptimizer, CCodeGenerator, OptimizationLevel};
use anyhow::{Result, Context};

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
}

#[derive(Subcommand)]
enum Commands {
    /// Compile a Seen source file to C code
    Build {
        /// Input file to compile
        #[arg(value_name = "FILE")]
        input: PathBuf,
        
        /// Output file path (defaults to a.c for C generation)
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

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    
    let cli = Cli::parse();
    
    // Load keyword manager with specified language
    let mut keyword_manager = KeywordManager::new();
    keyword_manager.load_from_toml(&cli.language)
        .with_context(|| format!("Failed to load language: {}", cli.language))?;
    keyword_manager.switch_language(&cli.language)?;
    let keyword_manager = Arc::new(keyword_manager);
    
    match cli.command {
        Some(Commands::Build { input, output, opt_level }) => {
            compile_file(&input, output.as_ref(), opt_level, keyword_manager)?;
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
        
        Some(Commands::Repl { show_ast, show_types }) => {
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

fn compile_file(input: &Path, output: Option<&PathBuf>, opt_level: u8, keyword_manager: Arc<KeywordManager>) -> Result<()> {
    println!("Compiling {} with optimization level {}", input.display(), opt_level);
    
    // Read source file
    let source = fs::read_to_string(input)
        .with_context(|| format!("Failed to read source file: {}", input.display()))?;
    
    // Lex and parse
    let lexer = Lexer::new(source, keyword_manager);
    let mut parser = SeenParser::new(lexer);
    let ast = parser.parse_program()
        .context("Failed to parse source")?;
    
    // Type check
    let mut type_checker = TypeChecker::new();
    let type_result = type_checker.check_program(&ast);
    if !type_result.errors.is_empty() {
        for error in &type_result.errors {
            eprintln!("Type error: {}", error);
        }
        anyhow::bail!("Type checking failed with {} errors", type_result.errors.len());
    }
    
    // Generate IR
    let mut ir_generator = IRGenerator::new();
    let ir_program = ir_generator.generate(&ast)
        .context("Failed to generate IR")?;
    
    println!("Generated IR with {} modules", ir_program.modules.len());
    
    // Optimize IR
    let optimization_level = OptimizationLevel::from_level(opt_level);
    let mut optimizer = IROptimizer::new(optimization_level);
    let mut optimized_ir = ir_program;
    optimizer.optimize_program(&mut optimized_ir)
        .context("Failed to optimize IR")?;
    
    println!("Applied optimization level {}", opt_level);
    
    // Generate C code
    let mut codegen = CCodeGenerator::new();
    let c_code = codegen.generate_program(&optimized_ir)
        .context("Failed to generate C code")?;
    
    // Determine output file
    let default_output = PathBuf::from("a.c");
    let output_path = output.unwrap_or(&default_output);
    
    // Write C code to file
    fs::write(output_path, c_code)
        .with_context(|| format!("Failed to write output file: {}", output_path.display()))?;
    
    println!("Generated C code: {}", output_path.display());
    println!("Compilation successful!");
    
    Ok(())
}

fn run_file(input: &Path, _args: &[String], keyword_manager: Arc<KeywordManager>) -> Result<()> {
    // Read source file
    let source = fs::read_to_string(input)
        .with_context(|| format!("Failed to read source file: {}", input.display()))?;
    
    // Lex and parse
    let lexer = Lexer::new(source, keyword_manager);
    let mut parser = SeenParser::new(lexer);
    let ast = parser.parse_program()
        .context("Failed to parse source")?;
    
    // Type check
    let mut type_checker = TypeChecker::new();
    let type_result = type_checker.check_program(&ast);
    if !type_result.errors.is_empty() {
        for error in &type_result.errors {
            eprintln!("Type error: {}", error);
        }
        anyhow::bail!("Type checking failed with {} errors", type_result.errors.len());
    }
    
    // Interpret the program
    let mut interpreter = Interpreter::new();
    let result = interpreter.interpret(&ast)
        .context("Runtime error")?;
    
    // Only print result if it's not Unit
    if !matches!(result, seen_interpreter::Value::Unit) {
        println!("{}", result);
    }
    
    Ok(())
}

fn check_file(input: &Path, keyword_manager: Arc<KeywordManager>) -> Result<()> {
    println!("Checking {}", input.display());
    
    // Read source file
    let source = fs::read_to_string(input)
        .with_context(|| format!("Failed to read source file: {}", input.display()))?;
    
    // Lex and parse
    let lexer = Lexer::new(source, keyword_manager);
    let mut parser = SeenParser::new(lexer);
    let ast = parser.parse_program()
        .context("Failed to parse source")?;
    
    // Type check
    let mut type_checker = TypeChecker::new();
    let type_result = type_checker.check_program(&ast);
    if !type_result.errors.is_empty() {
        for error in &type_result.errors {
            eprintln!("Type error: {}", error);
        }
        anyhow::bail!("Type checking failed with {} errors", type_result.errors.len());
    }
    
    println!("✓ No errors found");
    Ok(())
}

fn generate_ir(input: &Path, opt_level: u8, keyword_manager: Arc<KeywordManager>) -> Result<()> {
    println!("Generating IR for {}", input.display());
    
    // Read source file
    let source = fs::read_to_string(input)
        .with_context(|| format!("Failed to read source file: {}", input.display()))?;
    
    // Lex and parse
    let lexer = Lexer::new(source, keyword_manager);
    let mut parser = SeenParser::new(lexer);
    let ast = parser.parse_program()
        .context("Failed to parse source")?;
    
    // Type check
    let mut type_checker = TypeChecker::new();
    let type_result = type_checker.check_program(&ast);
    if !type_result.errors.is_empty() {
        for error in &type_result.errors {
            eprintln!("Type error: {}", error);
        }
        anyhow::bail!("Type checking failed with {} errors", type_result.errors.len());
    }
    
    // Generate IR
    let mut ir_generator = IRGenerator::new();
    let ir_program = ir_generator.generate(&ast)
        .context("Failed to generate IR")?;
    
    println!("\n=== Generated IR ===");
    println!("{}", ir_program);
    
    // Optimize IR if requested
    if opt_level > 0 {
        let optimization_level = OptimizationLevel::from_level(opt_level);
        let mut optimizer = IROptimizer::new(optimization_level);
        let mut optimized_ir = ir_program;
        optimizer.optimize_program(&mut optimized_ir)
            .context("Failed to optimize IR")?;
        
        println!("\n=== Optimized IR (level {}) ===", opt_level);
        println!("{}", optimized_ir);
    }
    
    Ok(())
}

fn run_repl(keyword_manager: Arc<KeywordManager>, show_ast: bool, show_types: bool) -> Result<()> {
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
                match evaluate_line(&input, &keyword_manager, &mut interpreter, show_ast, show_types) {
                    Ok(Some(result)) => {
                        if !matches!(result, seen_interpreter::Value::Unit) {
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
            match evaluate_line(line, &keyword_manager, &mut interpreter, show_ast, show_types) {
                Ok(Some(result)) => {
                    if !matches!(result, seen_interpreter::Value::Unit) {
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
    show_types: bool
) -> Result<Option<seen_interpreter::Value>> {
    // Lex and parse
    let lexer = Lexer::new(line.to_string(), keyword_manager.clone());
    let mut parser = SeenParser::new(lexer);
    let ast = parser.parse_program()?;
    
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
        anyhow::bail!("Type checking failed with {} errors", type_result.errors.len());
    }
    
    if show_types {
        println!("Types: {:?}", type_result);
    }
    
    // Interpret
    let result = interpreter.interpret(&ast)?;
    Ok(Some(result))
}

fn format_file(input: &Path, in_place: bool) -> Result<()> {
    println!("Formatting {} (in-place: {})", input.display(), in_place);
    // TODO: Implement code formatter
    println!("Code formatting not yet implemented");
    Ok(())
}

fn run_tests(path: Option<&Path>) -> Result<()> {
    if let Some(test_path) = path {
        println!("Running tests in {}", test_path.display());
    } else {
        println!("Running all tests");
    }
    // TODO: Implement test runner
    println!("Test runner not yet implemented");
    Ok(())
}

fn parse_file(input: &Path, format: &str, keyword_manager: Arc<KeywordManager>) -> Result<()> {
    // Read source file
    let source = fs::read_to_string(input)
        .with_context(|| format!("Failed to read source file: {}", input.display()))?;
    
    // Lex and parse
    let lexer = Lexer::new(source, keyword_manager);
    let mut parser = SeenParser::new(lexer);
    let ast = parser.parse_program()
        .context("Failed to parse source")?;
    
    // Output AST
    match format {
        "json" => {
            let json = serde_json::to_string_pretty(&ast)?;
            println!("{}", json);
        }
        "pretty" | _ => {
            println!("{:#?}", ast);
        }
    }
    
    Ok(())
}

fn lex_file(input: &Path, keyword_manager: Arc<KeywordManager>) -> Result<()> {
    // Read source file
    let source = fs::read_to_string(input)
        .with_context(|| format!("Failed to read source file: {}", input.display()))?;
    
    // Lex the source
    let mut lexer = Lexer::new(source, keyword_manager);
    let mut tokens = Vec::new();
    
    loop {
        let token = lexer.next_token()
            .context("Failed to tokenize source")?;
        
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