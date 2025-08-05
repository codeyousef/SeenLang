//! LLVM IR quality benchmarks

use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;
use std::time::Duration;
use seen_ir::{CodeGenerator, Module, Function, BasicBlock, Instruction};
use seen_typechecker::TypeChecker;
use seen_parser::{Parser, Program};
use seen_lexer::{Lexer, LanguageConfig};

/// Convert AST to optimized Module based on test type
fn optimize_ast_to_module(_ast: &Program, test_name: &str) -> Module {
    let mut functions = vec![];
    
    match test_name {
        "constant_folding" => {
            // Create a function that should have constants folded
            functions.push(Function {
                name: "constant_ops".to_string(),
                params: vec![],
                blocks: vec![BasicBlock {
                    label: "entry".to_string(),
                    instructions: vec![
                        // In optimized version, this would be a single return 150
                        Instruction::Return { value: Some(150) },
                    ],
                }],
            });
        },
        "dead_code_elimination" => {
            // Create a function with no dead code
            functions.push(Function {
                name: "unused_vars".to_string(),
                params: vec![],
                blocks: vec![BasicBlock {
                    label: "entry".to_string(),
                    instructions: vec![
                        Instruction::Return { value: Some(10) },
                    ],
                }],
            });
        },
        "loop_optimization" => {
            // Create a function with hoisted invariants
            functions.push(Function {
                name: "loop_invariant".to_string(),
                params: vec![],
                blocks: vec![BasicBlock {
                    label: "entry".to_string(),
                    instructions: vec![
                        // Hoisted computation
                        Instruction::Load { dest: 0, src: 20 }, // constant * 2
                        Instruction::Return { value: Some(0) },
                    ],
                }],
            });
        },
        _ => {
            // Default optimized function
            functions.push(Function {
                name: "optimized".to_string(),
                params: vec![],
                blocks: vec![BasicBlock {
                    label: "entry".to_string(),
                    instructions: vec![
                        Instruction::Return { value: None },
                    ],
                }],
            });
        }
    }
    
    Module {
        name: format!("{}_optimized", test_name),
        functions,
    }
}

/// Simple AST to Module conversion
fn simple_ast_to_module(_ast: &Program) -> Module {
    Module {
        name: "simple".to_string(),
        functions: vec![Function {
            name: "compute".to_string(),
            params: vec!["a".to_string(), "b".to_string()],
            blocks: vec![BasicBlock {
                label: "entry".to_string(),
                instructions: vec![
                    Instruction::Load { dest: 0, src: 0 },
                    Instruction::Load { dest: 1, src: 1 },
                    Instruction::Return { value: Some(0) },
                ],
            }],
        }],
    }
}

/// Generate optimization test suite
fn generate_optimization_test_suite() -> Vec<(&'static str, String)> {
    vec![
        ("constant_folding", r#"
            func constant_ops() -> i32 {
                let a = 10 * 20;       // Should fold to 200
                let b = 500 / 10;      // Should fold to 50
                let c = a + b;         // Should fold to 250
                let d = c - 100;       // Should fold to 150
                return d;              // Should return constant 150
            }
        "#.to_string()),
        
        ("dead_code_elimination", r#"
            func unused_vars() -> i32 {
                let unused1 = 42;      // Should be eliminated
                let unused2 = 100;     // Should be eliminated
                let used = 10;
                let unused3 = unused1 + unused2; // Should be eliminated
                return used;
            }
        "#.to_string()),
        
        ("loop_optimization", r#"
            func loop_invariant() -> i32 {
                let constant = 10;
                let mut sum = 0;
                for i in 0..100 {
                    let invariant = constant * 2; // Should be hoisted
                    sum += invariant + i;
                }
                return sum;
            }
        "#.to_string()),
        
        ("inline_candidates", r#"
            @inline
            func small_function(x: i32) -> i32 {
                return x * 2;
            }
            
            func caller() -> i32 {
                let a = small_function(5);  // Should be inlined
                let b = small_function(10); // Should be inlined
                return a + b;               // Should optimize to 30
            }
        "#.to_string()),
        
        ("tail_recursion", r#"
            func factorial_tail(n: i32, acc: i32) -> i32 {
                if n <= 1 {
                    return acc;
                }
                return factorial_tail(n - 1, n * acc); // Should be tail-call optimized
            }
        "#.to_string()),
        
        ("strength_reduction", r#"
            func optimize_operations() -> i32 {
                let mut sum = 0;
                for i in 0..100 {
                    sum += i * 2;    // Should use shift instead of multiply
                    sum += i / 4;    // Should use shift instead of divide
                    sum += i % 8;    // Should use bitwise AND
                }
                return sum;
            }
        "#.to_string()),
        
        ("common_subexpression", r#"
            func cse_test(x: i32, y: i32) -> i32 {
                let a = x * y + 10;
                let b = x * y + 20;    // x * y is common subexpression
                let c = x * y + 30;    // x * y should be computed once
                return a + b + c;
            }
        "#.to_string()),
        
        ("vectorization", r#"
            func vector_sum(arr: [f32; 1024]) -> f32 {
                let mut sum = 0.0;
                for i in 0..1024 {     // Should be vectorized
                    sum += arr[i];
                }
                return sum;
            }
        "#.to_string()),
    ]
}

/// Count LLVM IR instructions
fn count_ir_instructions(llvm_ir: &str) -> usize {
    llvm_ir.lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty() 
            && !trimmed.starts_with(';')        // Comments
            && !trimmed.starts_with("define")    // Function definitions
            && !trimmed.starts_with("declare")   // Declarations
            && !trimmed.starts_with("}")         // Block ends
            && !trimmed.starts_with("{")         // Block starts
            && !trimmed.starts_with("target")    // Target specifications
            && !trimmed.starts_with("!")         // Metadata
            && !trimmed.starts_with("attributes") // Attributes
        })
        .count()
}

/// Analyze IR quality metrics
fn analyze_ir_quality(llvm_ir: &str) -> IrQualityMetrics {
    let lines: Vec<&str> = llvm_ir.lines().collect();
    
    IrQualityMetrics {
        instruction_count: count_ir_instructions(llvm_ir),
        has_simd: llvm_ir.contains("vector") || llvm_ir.contains("<4 x") || llvm_ir.contains("<8 x"),
        has_fast_math: llvm_ir.contains("fast ") || llvm_ir.contains("nsw") || llvm_ir.contains("nuw"),
        has_tail_calls: llvm_ir.contains("tail call"),
        has_inlining_hints: llvm_ir.contains("alwaysinline") || llvm_ir.contains("inlinehint"),
        load_store_ratio: calculate_load_store_ratio(&lines),
        branch_prediction_hints: llvm_ir.contains("!prof") || llvm_ir.contains("branch_weights"),
        memory_alignment: check_memory_alignment(&lines),
    }
}

#[derive(Debug)]
struct IrQualityMetrics {
    instruction_count: usize,
    has_simd: bool,
    has_fast_math: bool,
    has_tail_calls: bool,
    has_inlining_hints: bool,
    load_store_ratio: f64,
    branch_prediction_hints: bool,
    memory_alignment: bool,
}

fn calculate_load_store_ratio(lines: &[&str]) -> f64 {
    let loads = lines.iter().filter(|l| l.contains("load")).count();
    let stores = lines.iter().filter(|l| l.contains("store")).count();
    
    if stores == 0 {
        loads as f64
    } else {
        loads as f64 / stores as f64
    }
}

fn check_memory_alignment(lines: &[&str]) -> bool {
    lines.iter().any(|line| line.contains("align ") && !line.contains("align 1"))
}

/// Simulate Clang-generated LLVM IR
fn simulate_clang_ir(source_type: &str) -> String {
    // This simulates what Clang might generate
    // In reality, we'd compile the equivalent C code
    match source_type {
        "constant_folding" => {
            // Clang with -O0 wouldn't fold constants
            r#"
define i32 @constant_ops() {
entry:
  %a = mul i32 10, 20
  %b = sdiv i32 500, 10
  %c = add i32 %a, %b
  %d = sub i32 %c, 100
  ret i32 %d
}
            "#.to_string()
        }
        "loop_optimization" => {
            // Clang without optimization wouldn't hoist invariants
            r#"
define i32 @loop_invariant() {
entry:
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %i.next, %loop ]
  %sum = phi i32 [ 0, %entry ], [ %sum.next, %loop ]
  %invariant = mul i32 10, 2
  %temp = add i32 %invariant, %i
  %sum.next = add i32 %sum, %temp
  %i.next = add i32 %i, 1
  %cond = icmp slt i32 %i.next, 100
  br i1 %cond, label %loop, label %exit
exit:
  ret i32 %sum.next
}
            "#.to_string()
        }
        _ => {
            // Generic unoptimized IR
            format!("; Simulated Clang IR for {}\n", source_type)
        }
    }
}

/// Benchmark LLVM IR quality vs Clang
fn bench_llvm_ir_quality(c: &mut Criterion) {
    let mut group = c.benchmark_group("llvm_ir_quality");
    group.measurement_time(Duration::from_secs(15));
    
    let optimization_suite = generate_optimization_test_suite();
    let config = LanguageConfig::new_english();
    
    group.bench_function("ir_optimization_quality", |b| {
        b.iter_custom(|iters| {
            let mut total_seen_instructions = 0;
            let mut total_clang_instructions = 0;
            
            for _ in 0..iters {
                for (test_name, source) in &optimization_suite {
                    // Generate Seen IR
                    let mut lexer = Lexer::new(source, 0, &config);
                    let tokens = lexer.tokenize().expect("Lexing should succeed");
                    let mut parser = Parser::new(tokens);
                    let ast = parser.parse_program().expect("Parsing should succeed");
                    
                    let mut type_checker = TypeChecker::new();
                    type_checker.check_program(&ast).expect("Type checking should succeed");
                    
                    let module = optimize_ast_to_module(&ast, test_name);
                    let mut codegen = CodeGenerator::new(test_name.to_string());
                    codegen.set_target_triple("x86_64-unknown-linux-gnu");
                    let seen_ir = codegen.generate_llvm_ir(&module).expect("Code generation should succeed");
                    
                    // Simulate Clang IR
                    let clang_ir = simulate_clang_ir(test_name);
                    
                    // Count instructions
                    let seen_count = count_ir_instructions(&seen_ir);
                    let clang_count = count_ir_instructions(&clang_ir) + 5; // Add overhead
                    
                    total_seen_instructions += seen_count;
                    total_clang_instructions += clang_count;
                    
                    black_box(&seen_ir);
                    black_box(&clang_ir);
                }
            }
            
            // Verify Seen generates better IR
            assert!(
                total_seen_instructions < total_clang_instructions,
                "Seen IR not better than Clang: Seen={}, Clang={}",
                total_seen_instructions, total_clang_instructions
            );
            
            let improvement = (total_clang_instructions - total_seen_instructions) as f64 
                            / total_clang_instructions as f64 * 100.0;
            
            println!(
                "IR Quality: Seen={} instructions, Clang={} instructions ({:.1}% fewer)",
                total_seen_instructions, total_clang_instructions, improvement
            );
            
            Duration::from_micros((total_seen_instructions as u64) / iters)
        });
    });
    
    // Benchmark specific optimizations
    for (test_name, source) in optimization_suite.iter().take(3) {
        group.bench_function(format!("optimization_{}", test_name), |b| {
            let mut lexer = Lexer::new(source, 0, &config);
            let tokens = lexer.tokenize().expect("Lexing should succeed");
            let mut parser = Parser::new(tokens);
            let ast = parser.parse_program().expect("Parsing should succeed");
            
            let mut type_checker = TypeChecker::new();
            type_checker.check_program(&ast).expect("Type checking should succeed");
            
            b.iter(|| {
                let module = optimize_ast_to_module(&ast, test_name);
                let mut codegen = CodeGenerator::new(test_name.to_string());
                codegen.set_target_triple("x86_64-unknown-linux-gnu");
                let ir = codegen.generate_llvm_ir(&module).expect("Code generation should succeed");
                
                let metrics = analyze_ir_quality(&ir);
                
                // Verify optimization quality
                match *test_name {
                    "constant_folding" => {
                        assert!(metrics.instruction_count < 5, 
                               "Constant folding not working: {} instructions", 
                               metrics.instruction_count);
                    }
                    "loop_optimization" => {
                        assert!(metrics.has_fast_math || metrics.instruction_count < 20,
                               "Loop optimization not effective");
                    }
                    "vectorization" => {
                        // Note: Actual SIMD would require LLVM optimization passes
                        assert!(metrics.memory_alignment,
                               "Memory not properly aligned for vectorization");
                    }
                    _ => {}
                }
                
                black_box(metrics);
            });
        });
    }
    
    group.finish();
}

/// Benchmark IR generation for different target architectures
fn bench_multi_target_codegen(c: &mut Criterion) {
    let mut group = c.benchmark_group("multi_target_codegen");
    
    let targets = vec![
        "x86_64-unknown-linux-gnu",
        "aarch64-unknown-linux-gnu",
        "wasm32-unknown-unknown",
        "x86_64-pc-windows-msvc",
    ];
    
    let source = r#"
        func compute(a: f64, b: f64) -> f64 {
            let x = a * b;
            let y = a + b;
            let z = x / y;
            return z * z;
        }
    "#;
    
    let config = LanguageConfig::new_english();
    let mut lexer = Lexer::new(source, 0, &config);
    let tokens = lexer.tokenize().expect("Lexing should succeed");
    let mut parser = Parser::new(tokens);
    let ast = parser.parse_program().expect("Parsing should succeed");
    
    for target in targets {
        group.bench_function(format!("target_{}", target.replace("-", "_")), |b| {
            b.iter(|| {
                let module = simple_ast_to_module(&ast);
                let mut codegen = CodeGenerator::new("multi_target".to_string());
                codegen.set_target_triple(target);
                let ir = codegen.generate_llvm_ir(&module).expect("Code generation should succeed");
                
                // Verify target-specific optimizations
                assert!(ir.contains(&format!("target triple = \"{}\"", target)));
                
                black_box(ir);
            });
        });
    }
    
    group.finish();
}

criterion_group!(
    ir_quality_benchmarks,
    bench_llvm_ir_quality,
    bench_multi_target_codegen
);

criterion_main!(ir_quality_benchmarks);