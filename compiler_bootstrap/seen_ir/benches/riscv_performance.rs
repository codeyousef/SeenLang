//! Comprehensive RISC-V performance benchmarks
//! 
//! Measures performance of RISC-V code generation, vector operations,
//! and reactive programming optimizations

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use seen_ir::ir::{Module, Function, BasicBlock, Instruction, Value, Target, RiscVExtensions};
use seen_ir::llvm_backend::LLVMBackend;
use seen_ir::CodeGenerator;

/// Benchmark RISC-V instruction generation performance
fn bench_riscv_instruction_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("riscv_instruction_generation");
    
    // Test different RISC-V targets
    let targets = vec![
        ("rv32i", Target::riscv32_bare_metal()),
        ("rv64i", Target::riscv64_bare_metal()),
        ("rv32gc", Target::riscv32_linux()),
        ("rv64gc", Target::riscv64_linux()),
    ];
    
    for (name, target) in targets {
        group.bench_with_input(
            BenchmarkId::new("basic_instructions", name),
            &target,
            |b, target| {
                b.iter(|| {
                    let module = create_basic_module("bench_module", target.clone());
                    let mut codegen = CodeGenerator::new_with_target("bench".to_string(), target.clone());
                    let ir = codegen.generate_llvm_ir(&module).unwrap();
                    black_box(ir);
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark RISC-V vector extension performance
fn bench_riscv_vector_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("riscv_vector_operations");
    
    let target = Target::riscv64_linux();
    let extensions = RiscVExtensions::rv64gcv();
    
    // Benchmark different vector operations
    let operations = vec![
        ("map", "Vector map transformation"),
        ("filter", "Vector filter with mask"),
        ("reduce", "Vector reduction"),
        ("scan", "Vector prefix sum"),
        ("zip", "Vector interleave"),
        ("merge", "Vector conditional merge"),
    ];
    
    for (op_name, description) in operations {
        group.bench_function(
            BenchmarkId::new("vector_op", op_name),
            |b| {
                let backend = LLVMBackend::with_target(format!("bench_{}", op_name), target.clone())
                    .with_riscv_extensions(extensions.clone());
                
                b.iter(|| {
                    let code = backend.generate_vector_optimized_reactive(op_name).unwrap();
                    black_box(code);
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark reactive operation compilation performance
fn bench_reactive_compilation(c: &mut Criterion) {
    let mut group = c.benchmark_group("reactive_compilation");
    
    // Different array sizes for benchmarking
    let sizes = vec![100, 1000, 10000, 100000];
    
    for size in sizes {
        group.bench_with_input(
            BenchmarkId::new("reactive_pipeline", size),
            &size,
            |b, &size| {
                let target = Target::riscv64_linux();
                let extensions = RiscVExtensions::rv64gcv();
                let backend = LLVMBackend::with_target("pipeline".to_string(), target)
                    .with_riscv_extensions(extensions);
                
                b.iter(|| {
                    // Generate a complete reactive pipeline: map -> filter -> reduce
                    let map_code = backend.generate_vector_optimized_reactive("map").unwrap();
                    let filter_code = backend.generate_vector_optimized_reactive("filter").unwrap();
                    let reduce_code = backend.generate_vector_optimized_reactive("reduce").unwrap();
                    
                    let pipeline = format!("{}\n{}\n{}", map_code, filter_code, reduce_code);
                    black_box(pipeline);
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark RISC-V vs x86 code generation
fn bench_riscv_vs_x86(c: &mut Criterion) {
    let mut group = c.benchmark_group("riscv_vs_x86");
    
    let riscv_target = Target::riscv64_linux();
    let x86_target = Target::x86_64_linux();
    
    // Create identical modules for both targets
    let module_riscv = create_complex_module("bench_riscv", riscv_target.clone());
    let module_x86 = create_complex_module("bench_x86", x86_target.clone());
    
    group.bench_function("riscv64_codegen", |b| {
        b.iter(|| {
            let mut codegen = CodeGenerator::new_with_target("riscv".to_string(), riscv_target.clone());
            let ir = codegen.generate_llvm_ir(&module_riscv).unwrap();
            black_box(ir);
        });
    });
    
    group.bench_function("x86_64_codegen", |b| {
        b.iter(|| {
            let mut codegen = CodeGenerator::new_with_target("x86".to_string(), x86_target.clone());
            let ir = codegen.generate_llvm_ir(&module_x86).unwrap();
            black_box(ir);
        });
    });
    
    group.finish();
}

/// Benchmark RISC-V extension combinations
fn bench_riscv_extensions(c: &mut Criterion) {
    let mut group = c.benchmark_group("riscv_extensions");
    
    let target = Target::riscv64_linux();
    
    // Different extension combinations
    let extension_sets = vec![
        ("base", RiscVExtensions { i: true, m: false, a: false, f: false, d: false, c: false, v: false }),
        ("rv64im", RiscVExtensions { i: true, m: true, a: false, f: false, d: false, c: false, v: false }),
        ("rv64gc", RiscVExtensions::rv64gc()),
        ("rv64gcv", RiscVExtensions::rv64gcv()),
    ];
    
    for (name, extensions) in extension_sets {
        group.bench_function(
            BenchmarkId::new("extension_set", name),
            |b| {
                let backend = LLVMBackend::with_target(format!("bench_{}", name), target.clone())
                    .with_riscv_extensions(extensions.clone());
                
                b.iter(|| {
                    // Generate code that uses various extensions
                    let module = create_extension_test_module(&name, target.clone(), &extensions);
                    let mut codegen = CodeGenerator::new_with_target(name.to_string(), target.clone());
                    let ir = codegen.generate_llvm_ir(&module).unwrap();
                    black_box(ir);
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark memory access patterns for RISC-V
fn bench_memory_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("riscv_memory_patterns");
    
    let target = Target::riscv64_linux();
    
    // Different memory access patterns
    let patterns = vec![
        ("sequential", create_sequential_access_module),
        ("strided", create_strided_access_module),
        ("random", create_random_access_module),
        ("gather_scatter", create_gather_scatter_module),
    ];
    
    for (pattern_name, create_fn) in patterns {
        group.bench_function(
            BenchmarkId::new("memory_pattern", pattern_name),
            |b| {
                let module = create_fn("bench", target.clone());
                
                b.iter(|| {
                    let mut codegen = CodeGenerator::new_with_target(pattern_name.to_string(), target.clone());
                    let ir = codegen.generate_llvm_ir(&module).unwrap();
                    black_box(ir);
                });
            },
        );
    }
    
    group.finish();
}

/// Helper function to create a basic module
fn create_basic_module(name: &str, target: Target) -> Module {
    Module {
        name: name.to_string(),
        target,
        functions: vec![
            Function {
                name: "add".to_string(),
                params: vec!["a".to_string(), "b".to_string()],
                blocks: vec![
                    BasicBlock {
                        label: "entry".to_string(),
                        instructions: vec![
                            Instruction::Binary {
                                dest: 0,
                                op: seen_ir::ir::BinaryOp::Add,
                                left: Value::Register(1),
                                right: Value::Register(2),
                            },
                            Instruction::Return { value: Some(Value::Register(0)) },
                        ],
                    },
                ],
            },
        ],
    }
}

/// Helper function to create a complex module
fn create_complex_module(name: &str, target: Target) -> Module {
    Module {
        name: name.to_string(),
        target,
        functions: vec![
            Function {
                name: "complex_computation".to_string(),
                params: vec!["x".to_string(), "y".to_string(), "z".to_string()],
                blocks: vec![
                    BasicBlock {
                        label: "entry".to_string(),
                        instructions: vec![
                            // Multiple arithmetic operations
                            Instruction::Binary {
                                dest: 3,
                                op: seen_ir::ir::BinaryOp::Mul,
                                left: Value::Register(0),
                                right: Value::Register(1),
                            },
                            Instruction::Binary {
                                dest: 4,
                                op: seen_ir::ir::BinaryOp::Add,
                                left: Value::Register(3),
                                right: Value::Register(2),
                            },
                            Instruction::Binary {
                                dest: 5,
                                op: seen_ir::ir::BinaryOp::Div,
                                left: Value::Register(4),
                                right: Value::Integer(2),
                            },
                            // Conditional branch
                            Instruction::Compare {
                                dest: 6,
                                op: seen_ir::ir::CompareOp::Gt,
                                left: Value::Register(5),
                                right: Value::Integer(100),
                            },
                            Instruction::Branch {
                                condition: Value::Register(6),
                                true_label: "then".to_string(),
                                false_label: "else".to_string(),
                            },
                        ],
                    },
                    BasicBlock {
                        label: "then".to_string(),
                        instructions: vec![
                            Instruction::Binary {
                                dest: 7,
                                op: seen_ir::ir::BinaryOp::Mul,
                                left: Value::Register(5),
                                right: Value::Integer(2),
                            },
                            Instruction::Jump { label: "exit".to_string() },
                        ],
                    },
                    BasicBlock {
                        label: "else".to_string(),
                        instructions: vec![
                            Instruction::Binary {
                                dest: 7,
                                op: seen_ir::ir::BinaryOp::Div,
                                left: Value::Register(5),
                                right: Value::Integer(2),
                            },
                            Instruction::Jump { label: "exit".to_string() },
                        ],
                    },
                    BasicBlock {
                        label: "exit".to_string(),
                        instructions: vec![
                            Instruction::Phi {
                                dest: 8,
                                incoming: vec![
                                    (Value::Register(7), "then".to_string()),
                                    (Value::Register(7), "else".to_string()),
                                ],
                            },
                            Instruction::Return { value: Some(Value::Register(8)) },
                        ],
                    },
                ],
            },
        ],
    }
}

/// Helper function to create a module that tests specific extensions
fn create_extension_test_module(name: &str, target: Target, extensions: &RiscVExtensions) -> Module {
    let mut instructions = vec![];
    
    // Base integer operations (always available)
    instructions.push(Instruction::Binary {
        dest: 0,
        op: seen_ir::ir::BinaryOp::Add,
        left: Value::Integer(1),
        right: Value::Integer(2),
    });
    
    // Multiply/divide operations (M extension)
    if extensions.m {
        instructions.push(Instruction::Binary {
            dest: 1,
            op: seen_ir::ir::BinaryOp::Mul,
            left: Value::Register(0),
            right: Value::Integer(3),
        });
        instructions.push(Instruction::Binary {
            dest: 2,
            op: seen_ir::ir::BinaryOp::Div,
            left: Value::Register(1),
            right: Value::Integer(2),
        });
    }
    
    // Atomic operations (A extension)
    if extensions.a {
        instructions.push(Instruction::AtomicLoad {
            dest: 3,
            ptr: 100,
            ordering: seen_ir::ir::MemoryOrdering::SeqCst,
        });
        instructions.push(Instruction::AtomicStore {
            ptr: 100,
            value: Value::Register(3),
            ordering: seen_ir::ir::MemoryOrdering::SeqCst,
        });
    }
    
    // Floating point operations (F/D extensions)
    if extensions.f || extensions.d {
        instructions.push(Instruction::Binary {
            dest: 4,
            op: seen_ir::ir::BinaryOp::Add,
            left: Value::Float(1.0),
            right: Value::Float(2.0),
        });
    }
    
    instructions.push(Instruction::Return { value: Some(Value::Integer(0)) });
    
    Module {
        name: name.to_string(),
        target,
        functions: vec![
            Function {
                name: "test_extensions".to_string(),
                params: vec![],
                blocks: vec![
                    BasicBlock {
                        label: "entry".to_string(),
                        instructions,
                    },
                ],
            },
        ],
    }
}

/// Create a module with sequential memory access pattern
fn create_sequential_access_module(name: &str, target: Target) -> Module {
    Module {
        name: name.to_string(),
        target,
        functions: vec![
            Function {
                name: "sequential_sum".to_string(),
                params: vec!["array".to_string(), "size".to_string()],
                blocks: vec![
                    BasicBlock {
                        label: "entry".to_string(),
                        instructions: vec![
                            Instruction::Binary {
                                dest: 2, // sum = 0
                                op: seen_ir::ir::BinaryOp::Add,
                                left: Value::Integer(0),
                                right: Value::Integer(0),
                            },
                            Instruction::Binary {
                                dest: 3, // i = 0
                                op: seen_ir::ir::BinaryOp::Add,
                                left: Value::Integer(0),
                                right: Value::Integer(0),
                            },
                            Instruction::Jump { label: "loop".to_string() },
                        ],
                    },
                    BasicBlock {
                        label: "loop".to_string(),
                        instructions: vec![
                            // Load array[i]
                            Instruction::Load {
                                dest: 4,
                                src: 0, // array base
                            },
                            // sum += array[i]
                            Instruction::Binary {
                                dest: 2,
                                op: seen_ir::ir::BinaryOp::Add,
                                left: Value::Register(2),
                                right: Value::Register(4),
                            },
                            // i++
                            Instruction::Binary {
                                dest: 3,
                                op: seen_ir::ir::BinaryOp::Add,
                                left: Value::Register(3),
                                right: Value::Integer(1),
                            },
                            // Check loop condition
                            Instruction::Compare {
                                dest: 5,
                                op: seen_ir::ir::CompareOp::Lt,
                                left: Value::Register(3),
                                right: Value::Register(1), // size
                            },
                            Instruction::Branch {
                                condition: Value::Register(5),
                                true_label: "loop".to_string(),
                                false_label: "exit".to_string(),
                            },
                        ],
                    },
                    BasicBlock {
                        label: "exit".to_string(),
                        instructions: vec![
                            Instruction::Return { value: Some(Value::Register(2)) },
                        ],
                    },
                ],
            },
        ],
    }
}

/// Create a module with strided memory access pattern
fn create_strided_access_module(name: &str, target: Target) -> Module {
    Module {
        name: name.to_string(),
        target,
        functions: vec![
            Function {
                name: "strided_sum".to_string(),
                params: vec!["array".to_string(), "size".to_string(), "stride".to_string()],
                blocks: vec![
                    BasicBlock {
                        label: "entry".to_string(),
                        instructions: vec![
                            // Similar to sequential but with stride
                            Instruction::Binary {
                                dest: 3, // sum = 0
                                op: seen_ir::ir::BinaryOp::Add,
                                left: Value::Integer(0),
                                right: Value::Integer(0),
                            },
                            Instruction::Binary {
                                dest: 4, // i = 0
                                op: seen_ir::ir::BinaryOp::Add,
                                left: Value::Integer(0),
                                right: Value::Integer(0),
                            },
                            Instruction::Jump { label: "loop".to_string() },
                        ],
                    },
                    BasicBlock {
                        label: "loop".to_string(),
                        instructions: vec![
                            // Calculate offset = i * stride
                            Instruction::Binary {
                                dest: 5,
                                op: seen_ir::ir::BinaryOp::Mul,
                                left: Value::Register(4),
                                right: Value::Register(2), // stride
                            },
                            // Load array[offset]
                            Instruction::Load {
                                dest: 6,
                                src: 5,
                            },
                            // sum += value
                            Instruction::Binary {
                                dest: 3,
                                op: seen_ir::ir::BinaryOp::Add,
                                left: Value::Register(3),
                                right: Value::Register(6),
                            },
                            // i++
                            Instruction::Binary {
                                dest: 4,
                                op: seen_ir::ir::BinaryOp::Add,
                                left: Value::Register(4),
                                right: Value::Integer(1),
                            },
                            // Check loop condition
                            Instruction::Compare {
                                dest: 7,
                                op: seen_ir::ir::CompareOp::Lt,
                                left: Value::Register(4),
                                right: Value::Register(1), // size
                            },
                            Instruction::Branch {
                                condition: Value::Register(7),
                                true_label: "loop".to_string(),
                                false_label: "exit".to_string(),
                            },
                        ],
                    },
                    BasicBlock {
                        label: "exit".to_string(),
                        instructions: vec![
                            Instruction::Return { value: Some(Value::Register(3)) },
                        ],
                    },
                ],
            },
        ],
    }
}

/// Create a module with random memory access pattern
fn create_random_access_module(name: &str, target: Target) -> Module {
    Module {
        name: name.to_string(),
        target,
        functions: vec![
            Function {
                name: "random_sum".to_string(),
                params: vec!["array".to_string(), "indices".to_string(), "count".to_string()],
                blocks: vec![
                    BasicBlock {
                        label: "entry".to_string(),
                        instructions: vec![
                            Instruction::Binary {
                                dest: 3, // sum = 0
                                op: seen_ir::ir::BinaryOp::Add,
                                left: Value::Integer(0),
                                right: Value::Integer(0),
                            },
                            Instruction::Binary {
                                dest: 4, // i = 0
                                op: seen_ir::ir::BinaryOp::Add,
                                left: Value::Integer(0),
                                right: Value::Integer(0),
                            },
                            Instruction::Jump { label: "loop".to_string() },
                        ],
                    },
                    BasicBlock {
                        label: "loop".to_string(),
                        instructions: vec![
                            // Load index from indices[i]
                            Instruction::Load {
                                dest: 5,
                                src: 1, // indices array
                            },
                            // Load value from array[index]
                            Instruction::Load {
                                dest: 6,
                                src: 5,
                            },
                            // sum += value
                            Instruction::Binary {
                                dest: 3,
                                op: seen_ir::ir::BinaryOp::Add,
                                left: Value::Register(3),
                                right: Value::Register(6),
                            },
                            // i++
                            Instruction::Binary {
                                dest: 4,
                                op: seen_ir::ir::BinaryOp::Add,
                                left: Value::Register(4),
                                right: Value::Integer(1),
                            },
                            // Check loop condition
                            Instruction::Compare {
                                dest: 7,
                                op: seen_ir::ir::CompareOp::Lt,
                                left: Value::Register(4),
                                right: Value::Register(2), // count
                            },
                            Instruction::Branch {
                                condition: Value::Register(7),
                                true_label: "loop".to_string(),
                                false_label: "exit".to_string(),
                            },
                        ],
                    },
                    BasicBlock {
                        label: "exit".to_string(),
                        instructions: vec![
                            Instruction::Return { value: Some(Value::Register(3)) },
                        ],
                    },
                ],
            },
        ],
    }
}

/// Create a module with gather/scatter memory access pattern
fn create_gather_scatter_module(name: &str, target: Target) -> Module {
    Module {
        name: name.to_string(),
        target,
        functions: vec![
            Function {
                name: "gather_scatter".to_string(),
                params: vec!["src".to_string(), "dst".to_string(), "indices".to_string(), "count".to_string()],
                blocks: vec![
                    BasicBlock {
                        label: "entry".to_string(),
                        instructions: vec![
                            Instruction::Binary {
                                dest: 4, // i = 0
                                op: seen_ir::ir::BinaryOp::Add,
                                left: Value::Integer(0),
                                right: Value::Integer(0),
                            },
                            Instruction::Jump { label: "loop".to_string() },
                        ],
                    },
                    BasicBlock {
                        label: "loop".to_string(),
                        instructions: vec![
                            // Load index from indices[i]
                            Instruction::Load {
                                dest: 5,
                                src: 2, // indices array
                            },
                            // Gather: Load value from src[index]
                            Instruction::Load {
                                dest: 6,
                                src: 5,
                            },
                            // Transform value (multiply by 2)
                            Instruction::Binary {
                                dest: 7,
                                op: seen_ir::ir::BinaryOp::Mul,
                                left: Value::Register(6),
                                right: Value::Integer(2),
                            },
                            // Scatter: Store to dst[index]
                            Instruction::Store {
                                dest: 5,
                                src: 7,
                            },
                            // i++
                            Instruction::Binary {
                                dest: 4,
                                op: seen_ir::ir::BinaryOp::Add,
                                left: Value::Register(4),
                                right: Value::Integer(1),
                            },
                            // Check loop condition
                            Instruction::Compare {
                                dest: 8,
                                op: seen_ir::ir::CompareOp::Lt,
                                left: Value::Register(4),
                                right: Value::Register(3), // count
                            },
                            Instruction::Branch {
                                condition: Value::Register(8),
                                true_label: "loop".to_string(),
                                false_label: "exit".to_string(),
                            },
                        ],
                    },
                    BasicBlock {
                        label: "exit".to_string(),
                        instructions: vec![
                            Instruction::Return { value: Some(Value::Integer(0)) },
                        ],
                    },
                ],
            },
        ],
    }
}

criterion_group!(
    riscv_benches,
    bench_riscv_instruction_generation,
    bench_riscv_vector_operations,
    bench_reactive_compilation,
    bench_riscv_vs_x86,
    bench_riscv_extensions,
    bench_memory_patterns
);

criterion_main!(riscv_benches);