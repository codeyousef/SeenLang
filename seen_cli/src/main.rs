//! Seen Language Command Line Interface
//! 
//! This is the main entry point for the Seen language compiler and toolchain.

use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::fs;
use std::io::{self, Write, BufRead};
use std::sync::Arc;
use seen_lexer::{Lexer, KeywordManager, TokenType};
use seen_parser::{Parser as SeenParser, Program, Expression, Type};
use seen_interpreter::Interpreter;
use seen_typechecker::TypeChecker;
use seen_ir::{IRGenerator, IROptimizer, CCodeGenerator, OptimizationLevel};
use seen_memory_manager::MemoryManager;
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
    
    // Memory analysis
    let mut memory_manager = MemoryManager::new();
    let memory_result = memory_manager.analyze_program(&ast);
    if memory_result.has_errors() {
        for error in memory_result.get_errors() {
            eprintln!("Memory error: {}", error);
        }
        anyhow::bail!("Memory analysis failed with {} errors", memory_result.get_errors().len());
    }
    
    // Apply memory optimizations
    for optimization in memory_result.get_optimizations() {
        println!("Memory optimization: {}", optimization);
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
    
    // Read source file
    let source = fs::read_to_string(input)
        .with_context(|| format!("Failed to read source file: {}", input.display()))?;
    
    // Create keyword manager
    let mut keyword_manager = KeywordManager::new();
    keyword_manager.load_from_toml("en")
        .with_context(|| "Failed to load keywords")?;
    keyword_manager.switch_language("en")
        .with_context(|| "Failed to switch to English keywords")?;
    
    // Parse the source code
    let lexer = Lexer::new(source.clone(), Arc::new(keyword_manager));
    let mut parser = SeenParser::new(lexer);
    let program = parser.parse_program()
        .with_context(|| "Failed to parse source code for formatting")?;
    
    // Format the code
    let formatted_code = format_program(&program);
    
    if in_place {
        // Write back to the original file
        fs::write(input, formatted_code)
            .with_context(|| format!("Failed to write formatted code to: {}", input.display()))?;
        println!("Formatted {} in place", input.display());
    } else {
        // Output to stdout
        println!("{}", formatted_code);
    }
    
    Ok(())
}

fn run_tests(path: Option<&Path>) -> Result<()> {
    let base_path = path.unwrap_or_else(|| Path::new("."));
    
    // Discover test files
    let test_files = discover_test_files(base_path)?;
    
    if test_files.is_empty() {
        println!("No test files found in {}", base_path.display());
        return Ok(());
    }
    
    println!("Running {} test file(s) from {}", test_files.len(), base_path.display());
    
    // Create keyword manager
    let mut keyword_manager = KeywordManager::new();
    keyword_manager.load_from_toml("en")
        .with_context(|| "Failed to load keywords")?;
    keyword_manager.switch_language("en")
        .with_context(|| "Failed to switch to English keywords")?;
    
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
    println!("Total: {}, Passed: {}, Failed: {}", total_tests, passed_tests, failed_tests);
    
    if failed_tests > 0 {
        std::process::exit(1);
    }
    
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
        for expression in &program.expressions {
            self.format_expression(expression);
            self.output.push('\n');
        }
        
        // Remove trailing newline
        if self.output.ends_with('\n') {
            self.output.pop();
        }
        
        self.output.clone()
    }

    fn format_expression(&mut self, expr: &Expression) -> &mut Self {
        match expr {
            Expression::Function { name, params, return_type, body, is_async, receiver, .. } => {
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
                    if i > 0 { self.output.push_str(", "); }
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
                    if i > 0 { self.output.push('\n'); }
                    self.format_expression(expr);
                }
            }
            
            Expression::Let { name, type_annotation, value, is_mutable, .. } => {
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
            
            Expression::If { condition, then_branch, else_branch, .. } => {
                self.add_indent();
                self.output.push_str("if ");
                self.format_expression(condition);
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
            
            Expression::Call { callee, args, .. } => {
                self.format_expression(callee);
                self.output.push('(');
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 { self.output.push_str(", "); }
                    self.format_expression(arg);
                }
                self.output.push(')');
            }
            
            Expression::Identifier { name, .. } => {
                self.output.push_str(name);
            }
            
            Expression::StringLiteral { value, .. } => {
                self.output.push('"');
                self.output.push_str(value);
                self.output.push('"');
            }
            
            Expression::IntegerLiteral { value, .. } => {
                self.output.push_str(&value.to_string());
            }
            
            Expression::BooleanLiteral { value, .. } => {
                self.output.push_str(if *value { "true" } else { "false" });
            }
            
            Expression::BinaryOp { left, op, right, .. } => {
                self.format_expression(left);
                self.output.push(' ');
                self.output.push_str(&format!("{:?}", op).to_lowercase());
                self.output.push(' ');
                self.format_expression(right);
            }
            
            Expression::Return { value, .. } => {
                self.add_indent();
                self.output.push_str("return");
                if let Some(val) = value {
                    self.output.push(' ');
                    self.format_expression(val);
                }
            }
            
            // Add more expression types as needed
            _ => {
                // For unhandled expressions, add a placeholder comment
                self.output.push_str(&format!("/* {} */", std::any::type_name::<Expression>()));
            }
        }
        
        self
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
                if i > 0 { self.output.push_str(", "); }
                self.format_type(generic);
            }
            self.output.push('>');
        }
        self
    }
}

/// Discover test files in the given directory tree
fn discover_test_files(base_path: &Path) -> Result<Vec<PathBuf>> {
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
            return extension == "seen" && 
                   (file_name.ends_with("_test.seen") || file_name.starts_with("test_"));
        }
    }
    false
}

/// Recursively search for test files
fn search_test_files_recursive(dir: &Path, test_files: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
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
fn run_test_file(test_file: &Path, keyword_manager: Arc<KeywordManager>) -> Result<(usize, usize)> {
    // Read the test file
    let source = fs::read_to_string(test_file)
        .with_context(|| format!("Failed to read test file: {}", test_file.display()))?;
    
    // Parse the test file
    let lexer = Lexer::new(source, keyword_manager.clone());
    let mut parser = SeenParser::new(lexer);
    let program = parser.parse_program()
        .with_context(|| format!("Failed to parse test file: {}", test_file.display()))?;
    
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
    program.expressions.iter()
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
fn run_single_test(interpreter: &mut Interpreter, test_func: &Expression, program: &Program) -> Result<()> {
    // Create a test program that just calls this test function
    let test_program = Program {
        expressions: vec![
            test_func.clone(),
            Expression::Call {
                callee: Box::new(Expression::Identifier {
                    name: get_function_name(test_func),
                    is_public: false,
                    pos: seen_lexer::Position { line: 1, column: 1, offset: 0 },
                }),
                args: vec![],
                pos: seen_lexer::Position { line: 1, column: 1, offset: 0 },
            }
        ],
    };
    
    // Run the test
    match interpreter.interpret(&test_program) {
        Ok(_) => Ok(()),
        Err(e) => Err(anyhow::anyhow!("Test execution failed: {}", e)),
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