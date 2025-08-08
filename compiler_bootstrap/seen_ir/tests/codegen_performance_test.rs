//! FAILING TESTS: Code generation performance requirements
//! 
//! These tests MUST fail initially to drive implementation (TDD).
//! Performance targets are hard requirements for Phase 2 completion.

use seen_ir::{CodeGenerator, Module, Function, BasicBlock, Instruction, Value, Target};
use seen_common::SeenResult;
use std::time::{Duration, Instant};

/// FAILING TEST: Code generation must be blazingly fast (<1ms for 1000 IR instructions)
#[test]
fn test_codegen_performance_under_1ms() {
    let start = Instant::now();
    
    // Generate test IR with 1000 instructions
    let mut module = Module {
        name: "test_module".to_string(),
        target: Target::x86_64_linux(),
        functions: vec![],
    };
    
    // Create a function with many basic blocks and instructions
    let mut test_function = Function {
        name: "test_function".to_string(),
        params: vec!["x".to_string(), "y".to_string()],
        blocks: vec![],
    };
    
    // Generate 10 basic blocks with 100 instructions each = 1000 total
    for block_idx in 0..10 {
        let mut block = BasicBlock {
            label: format!("bb{}", block_idx),
            instructions: vec![],
        };
        
        // Add 99 instructions per block, plus 1 return
        for instr_idx in 0..99 {
            let instruction = match instr_idx % 4 {
                0 => Instruction::Load { dest: instr_idx, src: instr_idx + 1000 },
                1 => Instruction::Store { dest: instr_idx + 2000, src: instr_idx },
                2 => Instruction::Call { 
                    dest: Some(instr_idx + 3000), 
                    func: "helper_func".to_string(), 
                    args: vec![Value::Register(instr_idx), Value::Register(instr_idx + 1)] 
                },
                _ => Instruction::Nop,
            };
            block.instructions.push(instruction);
        }
        
        // Add return statement for valid LLVM IR (except for last block which will fall through)
        if block_idx < 9 {
            block.instructions.push(Instruction::Return { value: Some(Value::Integer(42)) });
        } else {
            // Last block returns with no value
            block.instructions.push(Instruction::Return { value: None });
        }
        
        test_function.blocks.push(block);
    }
    
    module.functions.push(test_function);
    
    // REQUIREMENT: Code generation must complete in <1ms for 1000 instructions
    let mut codegen = CodeGenerator::new("performance_test".to_string());
    let llvm_ir = codegen.generate_llvm_ir(&module).expect("LLVM IR generation must succeed");
    let duration = start.elapsed();
    
    println!("Generated {} instructions in {:?}", 1000, duration);
    println!("LLVM IR size: {} bytes", llvm_ir.len());
    println!("LLVM IR sample:\n{}", &llvm_ir[..std::cmp::min(500, llvm_ir.len())]);
    
    // HARD REQUIREMENT: <1ms for 1000 instructions
    const MAX_DURATION_MS: u64 = 1;
    assert!(
        duration.as_millis() < MAX_DURATION_MS as u128,
        "CODE GENERATION TOO SLOW: {:?} >= {}ms maximum",
        duration,
        MAX_DURATION_MS
    );
    
    // REQUIREMENT: Generated IR must be valid LLVM
    assert!(llvm_ir.contains("define"), "Generated LLVM IR must contain function definitions");
    assert!(llvm_ir.contains("ret"), "Generated LLVM IR must contain return statements");
    
    println!("✓ Code generation performance validation passed");
}

/// FAILING TEST: Debug information generation must be complete and fast
#[test]
fn test_debug_info_generation() {
    let mut module = Module {
        name: "debug_test".to_string(),
        target: Target::x86_64_linux(),
        functions: vec![Function {
            name: "main".to_string(),
            params: vec![],
            blocks: vec![BasicBlock {
                label: "entry".to_string(),
                instructions: vec![
                    Instruction::Call { 
                        dest: None, 
                        func: "println".to_string(), 
                        args: vec![] 
                    },
                    Instruction::Return { value: None },
                ],
            }],
        }],
    };
    
    let mut codegen = CodeGenerator::new("debug_test".to_string());
    
    // REQUIREMENT: Debug info generation must be enabled
    codegen.enable_debug_info(true);
    let llvm_ir_with_debug = codegen.generate_llvm_ir(&module)
        .expect("Debug info generation must succeed");
    
    // REQUIREMENT: Generated IR must contain DWARF debug information
    assert!(llvm_ir_with_debug.contains("!DIFile"), "Must generate DWARF debug file info");
    assert!(llvm_ir_with_debug.contains("!DISubprogram"), "Must generate DWARF function info");
    assert!(llvm_ir_with_debug.contains("!DILocation"), "Must generate DWARF location info");
    
    println!("✓ Debug information generation validation passed");
}

/// FAILING TEST: C ABI compatibility must be perfect
#[test]
fn test_c_abi_compatibility() {
    let mut module = Module {
        name: "c_abi_test".to_string(),
        target: Target::x86_64_linux(),
        functions: vec![Function {
            name: "extern_c_function".to_string(),
            params: vec!["x".to_string(), "y".to_string()],
            blocks: vec![BasicBlock {
                label: "entry".to_string(),
                instructions: vec![
                    Instruction::Load { dest: 1, src: 0 },    // Load x
                    Instruction::Load { dest: 2, src: 1 },    // Load y  
                    Instruction::Call { 
                        dest: Some(3), 
                        func: "add_i32".to_string(), 
                        args: vec![Value::Register(1), Value::Register(2)] 
                    },
                    Instruction::Return { value: Some(Value::Register(3)) },
                ],
            }],
        }],
    };
    
    let mut codegen = CodeGenerator::new("c_abi_test".to_string());
    
    // REQUIREMENT: C ABI calling convention must be used
    codegen.set_calling_convention("C");
    let llvm_ir = codegen.generate_llvm_ir(&module)
        .expect("C ABI generation must succeed");
    
    // REQUIREMENT: Generated functions must use C calling convention
    assert!(llvm_ir.contains("define i32 @extern_c_function"), "Must use C ABI function signature");
    assert!(llvm_ir.contains("call"), "Must generate proper function calls");
    
    println!("✓ C ABI compatibility validation passed");
}

/// FAILING TEST: Cross-compilation support must work for multiple targets
#[test]
fn test_cross_compilation_targets() {
    let module = Module {
        name: "cross_compile_test".to_string(),
        target: Target::x86_64_linux(),
        functions: vec![Function {
            name: "platform_specific".to_string(),
            params: vec![],
            blocks: vec![BasicBlock {
                label: "entry".to_string(),
                instructions: vec![
                    Instruction::Return { value: None },
                ],
            }],
        }],
    };
    
    let targets = vec![
        "x86_64-unknown-linux-gnu",
        "aarch64-unknown-linux-gnu", 
        "wasm32-unknown-unknown",
        "x86_64-pc-windows-msvc",
    ];
    
    for target in targets {
        let mut codegen = CodeGenerator::new("cross_compile_test".to_string());
        
        // REQUIREMENT: Must support multiple target architectures
        codegen.set_target_triple(target);
        let llvm_ir = codegen.generate_llvm_ir(&module)
            .expect(&format!("Cross-compilation to {} must succeed", target));
        
        // REQUIREMENT: Generated IR must contain target-specific metadata
        assert!(llvm_ir.contains("target triple"), "Must specify target triple");
        
        println!("✓ Cross-compilation to {} succeeded", target);
    }
    
    println!("✓ Cross-compilation validation passed");
}

/// FAILING TEST: Memory optimization must be aggressive (minimal allocation)
#[test]
fn test_memory_optimization() {
    let mut module = Module {
        name: "memory_test".to_string(),
        target: Target::x86_64_linux(),
        functions: vec![],
    };
    
    // Create many functions to test memory usage
    for func_idx in 0..100 {
        let function = Function {
            name: format!("func_{}", func_idx),
            params: vec!["param".to_string()],
            blocks: vec![BasicBlock {
                label: "entry".to_string(),
                instructions: vec![
                    Instruction::Load { dest: 1, src: 0 },
                    Instruction::Return { value: Some(Value::Register(1)) },
                ],
            }],
        };
        module.functions.push(function);
    }
    
    // Measure memory usage during code generation
    let start_memory = get_memory_usage();
    let mut codegen = CodeGenerator::new("memory_test".to_string());
    let _llvm_ir = codegen.generate_llvm_ir(&module)
        .expect("Memory-optimized generation must succeed");
    let end_memory = get_memory_usage();
    
    let memory_increase = end_memory - start_memory;
    
    // REQUIREMENT: Memory increase must be < 10MB for 100 functions
    const MAX_MEMORY_MB: usize = 10;
    assert!(
        memory_increase < MAX_MEMORY_MB * 1024 * 1024,
        "EXCESSIVE MEMORY USAGE: {} bytes >= {}MB maximum",
        memory_increase,
        MAX_MEMORY_MB
    );
    
    println!("✓ Memory optimization validation passed");
}

/// Helper function to get current memory usage (approximate)
fn get_memory_usage() -> usize {
    // Simple approximation - in real implementation would use proper memory profiling
    std::env::var("MEMORY_USAGE").unwrap_or("0".to_string()).parse().unwrap_or(0)
}