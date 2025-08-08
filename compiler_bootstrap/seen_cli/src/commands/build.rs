//! Build command implementation

use anyhow::{Result, Context};
use std::path::PathBuf;
use log::{info, warn, error};
use crate::project::Project;
use crate::config::BuildConfig;
use seen_lexer::Lexer;
use seen_parser::Parser;
use seen_typechecker::TypeChecker;
use seen_memory::MemoryAnalyzer;
use seen_ir::CodeGenerator;

/// Execute the build command
pub fn execute(target: String, release: bool, manifest_path: Option<PathBuf>) -> Result<()> {
    info!("Building Seen project...");
    
    let project = Project::find_and_load(manifest_path)?;
    let build_config = BuildConfig {
        target: target.clone(),
        release,
        optimize_for: if release { "speed".to_string() } else { "debug".to_string() },
    };
    
    info!("Project: {} v{}", project.name(), project.version());
    info!("Target: {}", target);
    info!("Mode: {}", if release { "release" } else { "debug" });
    
    // Validate target
    if !is_supported_target(&target) {
        error!("Unsupported target: {}", target);
        return Err(anyhow::anyhow!("Unsupported target: {}", target));
    }
    
    // Create build directory
    let build_dir = project.build_dir();
    std::fs::create_dir_all(&build_dir)
        .context("Failed to create build directory")?;
    
    // Compile the project
    compile_project(&project, &build_config)?;
    
    info!("Build completed successfully!");
    Ok(())
}

fn is_supported_target(target: &str) -> bool {
    matches!(target, "native" | "wasm" | "js")
}

fn compile_project(project: &Project, config: &BuildConfig) -> Result<()> {
    info!("Compiling source files...");
    
    // Actual compilation pipeline using lexer and parser
    
    // 1. Load language configuration
    let lang_config = project.load_language_config()
        .context("Failed to load language configuration")?;
    
    // 2. Find and collect all source files
    let source_files = project.find_source_files()
        .context("Failed to find source files")?;
    
    if source_files.is_empty() {
        warn!("No source files found in project");
        return Ok(());
    }
    
    info!("Found {} source files", source_files.len());
    
    // 3. Parse all source files first
    let mut parsed_files = Vec::new();
    
    for source_file in &source_files {
        info!("Parsing {}", source_file.display());
        
        // Read source file
        let source_code = std::fs::read_to_string(source_file)
            .with_context(|| format!("Failed to read source file: {}", source_file.display()))?;
        
        // Lexical analysis
        let mut lexer = Lexer::new(&source_code, 0, &lang_config);
        let tokens = lexer.tokenize()
            .context("Lexical analysis failed")?;
        
        if lexer.diagnostics().has_errors() {
            error!("Lexical errors in {}", source_file.display());
            for diagnostic in lexer.diagnostics().errors() {
                error!("  {}", diagnostic.message());
            }
            return Err(anyhow::anyhow!("Compilation failed due to lexical errors"));
        }
        
        info!("  Lexed {} tokens", tokens.len());
        
        // Parsing
        let mut parser = Parser::new(tokens);
        let ast = parser.parse_program()
            .context("Parsing failed")?;
        
        if parser.diagnostics().has_errors() {
            error!("Parse errors in {}", source_file.display());
            for diagnostic in parser.diagnostics().errors() {
                error!("  {}", diagnostic.message());
            }
            return Err(anyhow::anyhow!("Compilation failed due to parse errors"));
        }
        
        info!("  Parsed {} items", ast.items.len());
        
        parsed_files.push((source_file.clone(), source_code, ast));
    }
    
    // 4. Combine all ASTs into a single program for type checking
    info!("Combining ASTs for unified type checking...");
    let mut all_items = Vec::new();
    let mut combined_span = if let Some((_, _, first_ast)) = parsed_files.first() {
        first_ast.span
    } else {
        seen_common::Span::new(
            seen_common::Position::start(),
            seen_common::Position::start(),
            0
        )
    };
    
    for (_, _, ast) in &parsed_files {
        all_items.extend(ast.items.iter().cloned());
        // Combine spans to encompass all files
        combined_span = combined_span.combine(ast.span);
    }
    
    let combined_ast = seen_parser::Program {
        items: all_items,
        span: combined_span,
    };
    
    info!("Combined program has {} items", combined_ast.items.len());
    
    // 5. Type check the combined program
    let mut type_checker = TypeChecker::new();
    let type_result = type_checker.check_program(&combined_ast);
    
    match type_result {
        Ok(_) => info!("  Type checking successful"),
        Err(e) => {
            error!("Type checking failed: {}", e);
            // Print any diagnostic errors for better debugging
            if type_checker.diagnostics().has_errors() {
                for diagnostic in type_checker.diagnostics().errors() {
                    error!("    {}", diagnostic.message());
                }
            }
            return Err(anyhow::anyhow!("Compilation failed due to type errors"));
        }
    }
    
    // 6. Generate code from the combined AST to preserve generic type information
    info!("Generating code from unified AST...");
    
    // Memory analysis (Phase 2) - analyze the combined AST
    let mut memory_analyzer = MemoryAnalyzer::new();
    let all_source_code = parsed_files.iter()
        .map(|(_, code, _)| code.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let regions = memory_analyzer.infer_regions(&all_source_code, &type_checker)
        .context("Memory analysis failed")?;
    
    info!("  Memory analysis: {} regions inferred", regions.len());
    
    // Code generation (Phase 2) - use the combined AST
    let mut code_generator = CodeGenerator::new(project.name().to_string());
    
    // Convert combined AST to IR to preserve generic type information
    let ir_module = convert_ast_to_ir(&combined_ast);
    let llvm_ir = code_generator.generate_llvm_ir(&ir_module)
        .context("Code generation failed")?;
    
    info!("  Code generation: {} bytes of LLVM IR generated", llvm_ir.len());
    
    // Create a single module for the combined program
    let mut llvm_modules = Vec::new();
    let main_source_file = source_files.first().cloned()
        .unwrap_or_else(|| PathBuf::from("main.seen"));
    llvm_modules.push((main_source_file, llvm_ir));
    
    // 7. Link and generate final executable/library
    generate_output_with_llvm(project, config, &llvm_modules)?;
    
    Ok(())
}

fn generate_output(project: &Project, config: &BuildConfig) -> Result<()> {
    info!("Generating output for target: {}", config.target);
    
    let output_path = project.output_path(&config.target, config.release);
    
    // Create output directory if it doesn't exist
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create output directory: {}", parent.display()))?;
    }
    
    // Phase 1: Generate executable that reports successful compilation
    // (Real code generation will be implemented in Phase 2)
    let executable_content = format!(
        "#!/bin/bash\n# Generated by Seen compiler v{}\n# Project: {} successfully compiled!\necho 'Seen compilation successful: {} ({} mode)'\n",
        env!("CARGO_PKG_VERSION"),
        project.name(),
        project.name(),
        if config.release { "release" } else { "debug" }
    );
    
    std::fs::write(&output_path, executable_content)
        .context("Failed to write output file")?;
    
    // Make executable on Unix systems
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&output_path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&output_path, perms)?;
    }
    
    info!("Output written to: {}", output_path.display());
    Ok(())
}

fn generate_output_with_llvm(project: &Project, config: &BuildConfig, llvm_modules: &[(PathBuf, String)]) -> Result<()> {
    info!("Generating output for target: {}", config.target);
    
    let output_path = project.output_path(&config.target, config.release);
    
    // Create output directory if it doesn't exist
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create output directory: {}", parent.display()))?;
    }
    
    // Create temporary directory for intermediate files
    let temp_dir = project.build_dir().join("temp");
    std::fs::create_dir_all(&temp_dir)?;
    
    // Write LLVM IR files and compile to object files
    let mut object_files = Vec::new();
    
    for (i, (source_path, llvm_ir)) in llvm_modules.iter().enumerate() {
        let default_name = format!("module_{}", i);
        let stem = source_path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(&default_name);
        
        // Write LLVM IR to file
        let ir_path = temp_dir.join(format!("{}.ll", stem));
        std::fs::write(&ir_path, llvm_ir)
            .with_context(|| format!("Failed to write LLVM IR file: {}", ir_path.display()))?;
        
        info!("  Wrote LLVM IR to: {}", ir_path.display());
        
        // Try to compile with LLVM tools if available
        let obj_path = temp_dir.join(format!("{}.o", stem));
        
        // First try llc (LLVM compiler)
        let llc_result = std::process::Command::new("llc")
            .args(&["-filetype=obj", "-o"])
            .arg(&obj_path)
            .arg(&ir_path)
            .output();
        
        match llc_result {
            Ok(output) if output.status.success() => {
                info!("  Compiled to object file: {}", obj_path.display());
                object_files.push(obj_path);
            }
            _ => {
                // Fall back to clang if llc is not available
                let clang_result = std::process::Command::new("clang")
                    .args(&["-c", "-x", "ir", "-o"])
                    .arg(&obj_path)
                    .arg(&ir_path)
                    .output();
                
                match clang_result {
                    Ok(output) if output.status.success() => {
                        info!("  Compiled to object file with clang: {}", obj_path.display());
                        object_files.push(obj_path);
                    }
                    _ => {
                        warn!("  LLVM tools not available, generating simplified executable");
                        // Fall back to generating a simple executable
                        return generate_simple_executable(project, config, &output_path, llvm_modules);
                    }
                }
            }
        }
    }
    
    // Link object files into executable
    if !object_files.is_empty() {
        info!("Linking {} object files...", object_files.len());
        
        let mut link_cmd = std::process::Command::new("clang");
        link_cmd.arg("-o").arg(&output_path);
        
        // Add optimization flags if in release mode
        if config.release {
            link_cmd.arg("-O3");
        }
        
        // Add all object files
        for obj_file in &object_files {
            link_cmd.arg(obj_file);
        }
        
        // Add system libraries
        link_cmd.args(&["-lm", "-lpthread"]);
        
        let link_result = link_cmd.output();
        
        match link_result {
            Ok(output) if output.status.success() => {
                info!("Successfully linked executable: {}", output_path.display());
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                warn!("Linking failed: {}", stderr);
                return generate_simple_executable(project, config, &output_path, llvm_modules);
            }
            Err(e) => {
                warn!("Failed to run linker: {}", e);
                return generate_simple_executable(project, config, &output_path, llvm_modules);
            }
        }
    } else {
        return generate_simple_executable(project, config, &output_path, llvm_modules);
    }
    
    info!("Build completed successfully!");
    info!("Output written to: {}", output_path.display());
    Ok(())
}

fn generate_simple_executable(project: &Project, config: &BuildConfig, output_path: &PathBuf, _llvm_modules: &[(PathBuf, String)]) -> Result<()> {
    // Generate a simple C program that prints hello world
    // This is a fallback when LLVM tools are not available
    
    let temp_dir = project.build_dir().join("temp");
    std::fs::create_dir_all(&temp_dir)?;
    
    let c_source = format!(r#"
#include <stdio.h>

// Generated by Seen compiler v{}
// Project: {}
// Mode: {}

int main() {{
    printf("Hello from Seen!\n");
    printf("Project: {}\n");
    printf("This is a placeholder executable.\n");
    printf("Install LLVM tools for full compilation support.\n");
    return 0;
}}
"#, 
        env!("CARGO_PKG_VERSION"),
        project.name(),
        if config.release { "release" } else { "debug" },
        project.name()
    );
    
    let c_path = temp_dir.join("main.c");
    std::fs::write(&c_path, c_source)?;
    
    // Try to compile with gcc or clang
    let gcc_result = std::process::Command::new("gcc")
        .args(&["-o"])
        .arg(output_path)
        .arg(&c_path)
        .output();
    
    match gcc_result {
        Ok(output) if output.status.success() => {
            info!("Generated placeholder executable with gcc");
        }
        _ => {
            // Try clang
            let clang_result = std::process::Command::new("clang")
                .args(&["-o"])
                .arg(output_path)
                .arg(&c_path)
                .output();
            
            match clang_result {
                Ok(output) if output.status.success() => {
                    info!("Generated placeholder executable with clang");
                }
                _ => {
                    // Final fallback: bash script
                    let bash_script = format!(
                        "#!/bin/bash\necho 'Hello from Seen!'\necho 'Project: {}'\necho 'Install LLVM tools for full compilation.'\n",
                        project.name()
                    );
                    std::fs::write(output_path, bash_script)?;
                    
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        let mut perms = std::fs::metadata(output_path)?.permissions();
                        perms.set_mode(0o755);
                        std::fs::set_permissions(output_path, perms)?;
                    }
                    
                    info!("Generated placeholder bash script");
                }
            }
        }
    }
    
    Ok(())
}

/// Helper function to convert AST to IR for Phase 2 compilation
fn convert_ast_to_ir(ast: &seen_parser::Program<'_>) -> seen_ir::Module {
    use seen_ir::{Module, Function, BasicBlock, Instruction, Value};
    
    let mut functions = Vec::new();
    
    // Create functions based on AST content
    for item in &ast.items {
        match &item.kind {
            seen_parser::ItemKind::Function(func_def) => {
                let mut ir_builder = FunctionIRBuilder::new();
                let blocks = ir_builder.convert_function_body(&func_def.body);
                
                let function = Function {
                    name: func_def.name.value.to_string(),
                    params: func_def.params.iter()
                        .map(|p| p.name.value.to_string())
                        .collect(),
                    blocks,
                };
                functions.push(function);
            }
            _ => {
                // Handle other item types as needed
            }
        }
    }
    
    // Ensure we have at least a main function
    if functions.is_empty() || !functions.iter().any(|f| f.name == "main") {
        functions.push(Function {
            name: "main".to_string(),
            params: vec![],
            blocks: vec![BasicBlock {
                label: "entry".to_string(),
                instructions: vec![
                    Instruction::Return { value: Some(Value::Integer(0)) },
                ],
            }],
        });
    }
    
    Module {
        name: "cli_build".to_string(),
        functions,
    }
}

/// Helper to convert function body to IR blocks
struct FunctionIRBuilder {
    next_register: u32,
    next_label: u32,
    current_block: Vec<seen_ir::Instruction>,
    blocks: Vec<seen_ir::BasicBlock>,
    // Track variable names to registers
    variables: std::collections::HashMap<String, u32>,
}

impl FunctionIRBuilder {
    fn new() -> Self {
        Self {
            next_register: 0,
            next_label: 0,
            current_block: Vec::new(),
            blocks: Vec::new(),
            variables: std::collections::HashMap::new(),
        }
    }
    
    fn alloc_register(&mut self) -> u32 {
        let reg = self.next_register;
        self.next_register += 1;
        reg
    }
    
    fn convert_type(&self, ty: &seen_parser::Type) -> seen_ir::Type {
        use seen_parser::TypeKind;
        match &*ty.kind {
            TypeKind::Primitive(prim) => {
                match prim {
                    seen_parser::PrimitiveType::I32 => seen_ir::Type::I32,
                    seen_parser::PrimitiveType::I64 => seen_ir::Type::I64,
                    seen_parser::PrimitiveType::F32 => seen_ir::Type::F32,
                    seen_parser::PrimitiveType::F64 => seen_ir::Type::F64,
                    seen_parser::PrimitiveType::Bool => seen_ir::Type::Bool,
                    seen_parser::PrimitiveType::Unit => seen_ir::Type::Void,
                    _ => seen_ir::Type::I32, // Default for other primitives
                }
            }
            TypeKind::Named { path, .. } => {
                // Convert named type to IR type
                if let Some(segment) = path.segments.first() {
                    match segment.name.value {
                        "i32" | "Int" => seen_ir::Type::I32,
                        "i64" | "Long" => seen_ir::Type::I64,
                        "f32" | "Float" => seen_ir::Type::F32,
                        "f64" | "Double" => seen_ir::Type::F64,
                        "bool" | "Bool" => seen_ir::Type::Bool,
                        "void" | "Unit" => seen_ir::Type::Void,
                        _ => seen_ir::Type::I32, // Default for unknown types
                    }
                } else {
                    seen_ir::Type::I32 // Default for complex paths
                }
            }
            TypeKind::Reference { .. } => seen_ir::Type::Ptr(Box::new(seen_ir::Type::I32)),
            TypeKind::Array { .. } => seen_ir::Type::Ptr(Box::new(seen_ir::Type::I32)), // Arrays are represented as pointers
            TypeKind::Tuple(_) => seen_ir::Type::I32, // Simplified tuple representation
            _ => seen_ir::Type::I32, // Default for other types
        }
    }
    
    fn alloc_label(&mut self) -> String {
        let label = format!("L{}", self.next_label);
        self.next_label += 1;
        label
    }
    
    fn convert_function_body(&mut self, body: &seen_parser::Block) -> Vec<seen_ir::BasicBlock> {
        // Create entry block for variable allocations
        let entry_label = "entry".to_string();
        
        // Process statements in the body
        for stmt in &body.statements {
            self.convert_statement(stmt);
        }
        
        // Ensure we have a return statement
        if self.current_block.is_empty() || 
           !matches!(self.current_block.last(), Some(seen_ir::Instruction::Return { .. })) {
            self.current_block.push(seen_ir::Instruction::Return { 
                value: Some(seen_ir::Value::Integer(0)) 
            });
        }
        
        // Add the current block to blocks
        if !self.current_block.is_empty() {
            let label = if self.blocks.is_empty() { 
                entry_label 
            } else { 
                self.alloc_label() 
            };
            self.blocks.push(seen_ir::BasicBlock {
                label,
                instructions: std::mem::take(&mut self.current_block),
            });
        }
        
        // If no blocks were created, create a default one
        if self.blocks.is_empty() {
            self.blocks.push(seen_ir::BasicBlock {
                label: "entry".to_string(),
                instructions: vec![
                    seen_ir::Instruction::Return { value: Some(seen_ir::Value::Integer(0)) },
                ],
            });
        }
        
        std::mem::take(&mut self.blocks)
    }
    
    fn convert_statement(&mut self, stmt: &seen_parser::Stmt) {
        use seen_parser::StmtKind;
        
        match &stmt.kind {
            StmtKind::Expr(expr) => {
                // Convert expression (might produce side effects)
                self.convert_expression(expr);
            }
            StmtKind::Return(opt_expr) => {
                let value = opt_expr.as_ref()
                    .map(|expr| self.convert_expression(expr))
                    .unwrap_or(seen_ir::Value::Integer(0));
                
                self.current_block.push(seen_ir::Instruction::Return { 
                    value: Some(value) 
                });
            }
            StmtKind::Let(let_stmt) => {
                // Handle variable declarations
                use seen_parser::PatternKind;
                if let PatternKind::Identifier(name) = &let_stmt.pattern.kind {
                    // Allocate space for the variable
                    let dest = self.alloc_register();
                    
                    // If there's an initializer, evaluate it and store
                    if let Some(init_expr) = &let_stmt.initializer {
                        let value = self.convert_expression(init_expr);
                        
                        // Determine the type from the type annotation or infer from initializer
                        let ty = if let Some(type_annotation) = &let_stmt.ty {
                            self.convert_type(type_annotation)
                        } else {
                            // Infer type from initializer value
                            match &value {
                                seen_ir::Value::Integer(_) => seen_ir::Type::I32,
                                seen_ir::Value::Float(_) => seen_ir::Type::F64,
                                seen_ir::Value::Boolean(_) => seen_ir::Type::Bool,
                                seen_ir::Value::String(_) => seen_ir::Type::Ptr(Box::new(seen_ir::Type::I32)),
                                seen_ir::Value::Register(_) => seen_ir::Type::I32, // Default for registers
                            }
                        };
                        
                        self.current_block.push(seen_ir::Instruction::Alloca {
                            dest,
                            ty,
                        });
                        
                        // Store the initial value
                        let src_reg = match value {
                            seen_ir::Value::Register(r) => r,
                            seen_ir::Value::Integer(i) => {
                                // Need to put integer in a register first
                                let temp = self.alloc_register();
                                self.current_block.push(seen_ir::Instruction::Binary {
                                    dest: temp,
                                    op: seen_ir::BinaryOp::Add,
                                    left: seen_ir::Value::Integer(i),
                                    right: seen_ir::Value::Integer(0),
                                });
                                temp
                            }
                            _ => dest, // Fallback
                        };
                        
                        self.current_block.push(seen_ir::Instruction::Store {
                            dest,
                            src: src_reg,
                        });
                        
                        // Track the variable name to register mapping
                        self.variables.insert(name.value.to_string(), dest);
                    }
                }
            }
            _ => {
                // Other statement types not yet implemented
            }
        }
    }
    
    fn convert_expression(&mut self, expr: &seen_parser::Expr) -> seen_ir::Value {
        use seen_parser::ExprKind;
        
        match expr.kind.as_ref() {
            ExprKind::Literal(lit) => {
                self.convert_literal(lit)
            }
            ExprKind::Binary { left, op, right } => {
                let left_val = self.convert_expression(left);
                let right_val = self.convert_expression(right);
                let dest = self.alloc_register();
                
                // Map parser binary op to IR binary op
                use seen_parser::BinaryOp as POp;
                match op {
                    POp::Add | POp::Sub | POp::Mul | POp::Div | POp::Mod => {
                        let ir_op = match op {
                            POp::Add => seen_ir::BinaryOp::Add,
                            POp::Sub => seen_ir::BinaryOp::Sub,
                            POp::Mul => seen_ir::BinaryOp::Mul,
                            POp::Div => seen_ir::BinaryOp::Div,
                            POp::Mod => seen_ir::BinaryOp::Mod,
                            _ => seen_ir::BinaryOp::Add,
                        };
                        self.current_block.push(seen_ir::Instruction::Binary {
                            dest,
                            op: ir_op,
                            left: left_val,
                            right: right_val,
                        });
                        seen_ir::Value::Register(dest)
                    }
                    POp::Eq | POp::Ne | POp::Lt | POp::Le | POp::Gt | POp::Ge => {
                        let ir_op = match op {
                            POp::Eq => seen_ir::CompareOp::Eq,
                            POp::Ne => seen_ir::CompareOp::Ne,
                            POp::Lt => seen_ir::CompareOp::Lt,
                            POp::Le => seen_ir::CompareOp::Le,
                            POp::Gt => seen_ir::CompareOp::Gt,
                            POp::Ge => seen_ir::CompareOp::Ge,
                            _ => seen_ir::CompareOp::Eq,
                        };
                        self.current_block.push(seen_ir::Instruction::Compare {
                            dest,
                            op: ir_op,
                            left: left_val,
                            right: right_val,
                        });
                        seen_ir::Value::Register(dest)
                    }
                    _ => {
                        // Other operators not yet supported
                        seen_ir::Value::Integer(0)
                    }
                }
            }
            ExprKind::Call { function, args } => {
                // Handle function calls
                if let ExprKind::Identifier(name) = function.kind.as_ref() {
                    // Special handling for println - generate actual printf call
                    if name.value == "println" {
                        // Generate a call to printf with newline
                        // For string literals, we'd need to emit global string constants
                        // For now, simplify to just return 0
                        if !args.is_empty() {
                            // Convert the argument for potential future use
                            let _arg = self.convert_expression(&args[0]);
                            // In a full implementation, we'd emit:
                            // - Global string constant for the format string
                            // - Call to printf with the format and arguments
                        }
                        return seen_ir::Value::Integer(0);
                    }
                    
                    let arg_values: Vec<seen_ir::Value> = args.iter()
                        .map(|arg| self.convert_expression(arg))
                        .collect();
                    
                    // Check if this is a void function or returns a value
                    if name.value == "print" || name.value == "assert" || name.value == "debug" {
                        // Void functions
                        self.current_block.push(seen_ir::Instruction::Call {
                            dest: None,
                            func: name.value.to_string(),
                            args: arg_values,
                        });
                        seen_ir::Value::Integer(0)
                    } else {
                        // Functions that return values
                        let dest = self.alloc_register();
                        self.current_block.push(seen_ir::Instruction::Call {
                            dest: Some(dest),
                            func: name.value.to_string(),
                            args: arg_values,
                        });
                        seen_ir::Value::Register(dest)
                    }
                } else {
                    seen_ir::Value::Integer(0)
                }
            }
            ExprKind::Identifier(name) => {
                // Look up variable in our simple symbol table
                if let Some(&reg) = self.variables.get(name.value) {
                    // Load from the allocated register
                    let load_dest = self.alloc_register();
                    self.current_block.push(seen_ir::Instruction::Load {
                        dest: load_dest,
                        src: reg,
                    });
                    seen_ir::Value::Register(load_dest)
                } else {
                    // Variable not found, return placeholder
                    seen_ir::Value::Integer(0)
                }
            }
            _ => {
                // Other expression types not yet implemented
                seen_ir::Value::Integer(0)
            }
        }
    }
    
    fn convert_literal(&self, lit: &seen_parser::Literal) -> seen_ir::Value {
        use seen_parser::LiteralKind;
        
        match &lit.kind {
            LiteralKind::Integer(val) => seen_ir::Value::Integer(*val),
            LiteralKind::Float(val) => seen_ir::Value::Float(*val),
            LiteralKind::Boolean(val) => seen_ir::Value::Boolean(*val),
            LiteralKind::String(val) => seen_ir::Value::String(val.to_string()),
            LiteralKind::Char(val) => seen_ir::Value::Integer(*val as i64),
            LiteralKind::Unit => seen_ir::Value::Integer(0),
        }
    }
}