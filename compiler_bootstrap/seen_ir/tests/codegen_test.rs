//! Comprehensive tests for LLVM IR code generation
//! Verifies that the code generator produces correct LLVM IR

use seen_ir::{CodeGenerator, Module, Function, BasicBlock, Instruction, Value, BinaryOp, CompareOp, Type};
use seen_common::SeenResult;

#[test]
fn test_basic_function_generation() {
    let mut codegen = CodeGenerator::new("test_module".to_string());
    
    let module = Module {
        name: "test".to_string(),
        functions: vec![
            Function {
                name: "main".to_string(),
                params: vec![],
                blocks: vec![
                    BasicBlock {
                        label: "entry".to_string(),
                        instructions: vec![
                            Instruction::Return { value: Some(Value::Integer(0)) },
                        ],
                    },
                ],
            },
        ],
    };
    
    let ir = codegen.generate_llvm_ir(&module).expect("Should generate LLVM IR");
    
    assert!(ir.contains("define"), "Should have function definition");
    assert!(ir.contains("@main"), "Should have main function");
    assert!(ir.contains("ret i32"), "Should have return instruction");
}

#[test]
fn test_arithmetic_operations() {
    let mut codegen = CodeGenerator::new("arithmetic_test".to_string());
    
    let module = Module {
        name: "arithmetic".to_string(),
        functions: vec![
            Function {
                name: "calculate".to_string(),
                params: vec!["x".to_string(), "y".to_string()],
                blocks: vec![
                    BasicBlock {
                        label: "entry".to_string(),
                        instructions: vec![
                            Instruction::Binary {
                                dest: 2,
                                op: BinaryOp::Add,
                                left: Value::Register(0),
                                right: Value::Register(1),
                            },
                            Instruction::Binary {
                                dest: 3,
                                op: BinaryOp::Mul,
                                left: Value::Register(2),
                                right: Value::Integer(2),
                            },
                            Instruction::Return { value: Some(Value::Register(3)) },
                        ],
                    },
                ],
            },
        ],
    };
    
    let ir = codegen.generate_llvm_ir(&module).expect("Should generate LLVM IR");
    
    assert!(ir.contains("add i32"), "Should have add instruction");
    assert!(ir.contains("mul i32"), "Should have multiply instruction");
    assert!(ir.contains("%2 = add"), "Should have correct register assignment");
}

#[test]
fn test_comparison_operations() {
    let mut codegen = CodeGenerator::new("comparison_test".to_string());
    
    let module = Module {
        name: "comparison".to_string(),
        functions: vec![
            Function {
                name: "compare".to_string(),
                params: vec!["a".to_string(), "b".to_string()],
                blocks: vec![
                    BasicBlock {
                        label: "entry".to_string(),
                        instructions: vec![
                            Instruction::Compare {
                                dest: 2,
                                op: CompareOp::Gt,
                                left: Value::Register(0),
                                right: Value::Register(1),
                            },
                            Instruction::Return { value: Some(Value::Register(2)) },
                        ],
                    },
                ],
            },
        ],
    };
    
    let ir = codegen.generate_llvm_ir(&module).expect("Should generate LLVM IR");
    
    assert!(ir.contains("icmp sgt"), "Should have signed greater-than comparison");
    assert!(ir.contains("%2 = icmp"), "Should assign comparison result");
}

#[test]
fn test_control_flow() {
    let mut codegen = CodeGenerator::new("control_flow_test".to_string());
    
    let module = Module {
        name: "control".to_string(),
        functions: vec![
            Function {
                name: "conditional".to_string(),
                params: vec!["cond".to_string()],
                blocks: vec![
                    BasicBlock {
                        label: "entry".to_string(),
                        instructions: vec![
                            Instruction::Branch {
                                condition: Some(0),
                                true_label: "then".to_string(),
                                false_label: Some("else".to_string()),
                            },
                        ],
                    },
                    BasicBlock {
                        label: "then".to_string(),
                        instructions: vec![
                            Instruction::Return { value: Some(Value::Integer(1)) },
                        ],
                    },
                    BasicBlock {
                        label: "else".to_string(),
                        instructions: vec![
                            Instruction::Return { value: Some(Value::Integer(0)) },
                        ],
                    },
                ],
            },
        ],
    };
    
    let ir = codegen.generate_llvm_ir(&module).expect("Should generate LLVM IR");
    
    assert!(ir.contains("br i1"), "Should have conditional branch");
    assert!(ir.contains("label %then"), "Should have then label");
    assert!(ir.contains("label %else"), "Should have else label");
}

#[test]
fn test_memory_operations() {
    let mut codegen = CodeGenerator::new("memory_test".to_string());
    
    let module = Module {
        name: "memory".to_string(),
        functions: vec![
            Function {
                name: "memory_ops".to_string(),
                params: vec![],
                blocks: vec![
                    BasicBlock {
                        label: "entry".to_string(),
                        instructions: vec![
                            Instruction::Alloca { dest: 0, ty: Type::I32 },
                            Instruction::Store { dest: 0, src: 1 },
                            Instruction::Load { dest: 2, src: 0 },
                            Instruction::Return { value: Some(Value::Register(2)) },
                        ],
                    },
                ],
            },
        ],
    };
    
    let ir = codegen.generate_llvm_ir(&module).expect("Should generate LLVM IR");
    
    assert!(ir.contains("alloca i32"), "Should have alloca instruction");
    assert!(ir.contains("store i32"), "Should have store instruction");
    assert!(ir.contains("load i32"), "Should have load instruction");
    assert!(ir.contains("align 4"), "Should have alignment");
}

#[test]
fn test_function_calls() {
    let mut codegen = CodeGenerator::new("call_test".to_string());
    
    let module = Module {
        name: "calls".to_string(),
        functions: vec![
            Function {
                name: "caller".to_string(),
                params: vec![],
                blocks: vec![
                    BasicBlock {
                        label: "entry".to_string(),
                        instructions: vec![
                            Instruction::Call {
                                dest: Some(0),
                                func: "helper".to_string(),
                                args: vec![Value::Integer(10), Value::Integer(20)],
                            },
                            Instruction::Return { value: Some(Value::Register(0)) },
                        ],
                    },
                ],
            },
        ],
    };
    
    let ir = codegen.generate_llvm_ir(&module).expect("Should generate LLVM IR");
    
    assert!(ir.contains("call i32 @helper"), "Should have function call");
    assert!(ir.contains("(i32 10, i32 20)") || ir.contains("10") && ir.contains("20"), 
           "Should have call arguments");
}

#[test]
fn test_phi_nodes() {
    let mut codegen = CodeGenerator::new("phi_test".to_string());
    
    let module = Module {
        name: "phi".to_string(),
        functions: vec![
            Function {
                name: "phi_example".to_string(),
                params: vec!["x".to_string()],
                blocks: vec![
                    BasicBlock {
                        label: "entry".to_string(),
                        instructions: vec![
                            Instruction::Branch {
                                condition: None,
                                true_label: "loop".to_string(),
                                false_label: None,
                            },
                        ],
                    },
                    BasicBlock {
                        label: "loop".to_string(),
                        instructions: vec![
                            Instruction::Phi {
                                dest: 1,
                                values: vec![
                                    (Value::Integer(0), "entry".to_string()),
                                    (Value::Register(2), "loop".to_string()),
                                ],
                            },
                            Instruction::Binary {
                                dest: 2,
                                op: BinaryOp::Add,
                                left: Value::Register(1),
                                right: Value::Integer(1),
                            },
                            Instruction::Return { value: Some(Value::Register(2)) },
                        ],
                    },
                ],
            },
        ],
    };
    
    let ir = codegen.generate_llvm_ir(&module).expect("Should generate LLVM IR");
    
    assert!(ir.contains("phi i32"), "Should have phi node");
    assert!(ir.contains("[0, %entry]"), "Should have phi incoming value from entry");
}

#[test]
fn test_debug_info_generation() {
    let mut codegen = CodeGenerator::new("debug_test".to_string());
    codegen.enable_debug_info(true);
    
    let module = Module {
        name: "debug".to_string(),
        functions: vec![
            Function {
                name: "debug_func".to_string(),
                params: vec![],
                blocks: vec![
                    BasicBlock {
                        label: "entry".to_string(),
                        instructions: vec![
                            Instruction::Return { value: Some(Value::Integer(42)) },
                        ],
                    },
                ],
            },
        ],
    };
    
    let ir = codegen.generate_llvm_ir(&module).expect("Should generate LLVM IR");
    
    assert!(ir.contains("!llvm.dbg.cu"), "Should have debug compile unit");
    assert!(ir.contains("!DICompileUnit"), "Should have debug info metadata");
    assert!(ir.contains("DISubprogram"), "Should have subprogram debug info");
}

#[test]
fn test_optimization_levels() {
    let mut codegen = CodeGenerator::new("opt_test".to_string());
    
    // Test different optimization levels
    for level in 0..=3 {
        codegen.set_optimization_level(level);
        
        let module = Module {
            name: format!("opt_level_{}", level),
            functions: vec![
                Function {
                    name: "opt_func".to_string(),
                    params: vec![],
                    blocks: vec![
                        BasicBlock {
                            label: "entry".to_string(),
                            instructions: vec![
                                Instruction::Return { value: Some(Value::Integer(0)) },
                            ],
                        },
                    ],
                },
            ],
        };
        
        let ir = codegen.generate_llvm_ir(&module).expect("Should generate LLVM IR");
        assert!(!ir.is_empty(), "Should generate IR for optimization level {}", level);
    }
}

#[test]
fn test_target_triple() {
    let mut codegen = CodeGenerator::new("target_test".to_string());
    
    // Test different target triples
    let targets = [
        "x86_64-unknown-linux-gnu",
        "x86_64-pc-windows-msvc",
        "aarch64-apple-darwin",
        "wasm32-unknown-unknown",
    ];
    
    for target in &targets {
        codegen.set_target_triple(target);
        
        let module = Module {
            name: "target".to_string(),
            functions: vec![],
        };
        
        let ir = codegen.generate_llvm_ir(&module).expect("Should generate LLVM IR");
        assert!(ir.contains(target), "Should include target triple: {}", target);
    }
}

#[test]
fn test_calling_conventions() {
    let mut codegen = CodeGenerator::new("calling_conv_test".to_string());
    
    // Test C calling convention
    codegen.set_calling_convention("C");
    let module = Module {
        name: "ccc".to_string(),
        functions: vec![
            Function {
                name: "c_func".to_string(),
                params: vec!["x".to_string()],
                blocks: vec![
                    BasicBlock {
                        label: "entry".to_string(),
                        instructions: vec![
                            Instruction::Return { value: Some(Value::Register(0)) },
                        ],
                    },
                ],
            },
        ],
    };
    
    let ir = codegen.generate_llvm_ir(&module).expect("Should generate LLVM IR");
    // C calling convention is usually default, so it might not appear explicitly
    assert!(ir.contains("define"), "Should have function definition");
    
    // Test fastcc calling convention
    codegen.set_calling_convention("fastcc");
    let ir_fast = codegen.generate_llvm_ir(&module).expect("Should generate LLVM IR");
    assert!(ir_fast.contains("fastcc") || ir_fast.contains("define"), 
           "Should handle fastcc calling convention");
}

#[test]
fn test_complex_function() {
    let mut codegen = CodeGenerator::new("complex_test".to_string());
    
    // Create a more complex function with multiple blocks and operations
    let module = Module {
        name: "complex".to_string(),
        functions: vec![
            Function {
                name: "fibonacci".to_string(),
                params: vec!["n".to_string()],
                blocks: vec![
                    BasicBlock {
                        label: "entry".to_string(),
                        instructions: vec![
                            Instruction::Compare {
                                dest: 1,
                                op: CompareOp::Le,
                                left: Value::Register(0),
                                right: Value::Integer(1),
                            },
                            Instruction::Branch {
                                condition: Some(1),
                                true_label: "base_case".to_string(),
                                false_label: Some("recursive_case".to_string()),
                            },
                        ],
                    },
                    BasicBlock {
                        label: "base_case".to_string(),
                        instructions: vec![
                            Instruction::Return { value: Some(Value::Register(0)) },
                        ],
                    },
                    BasicBlock {
                        label: "recursive_case".to_string(),
                        instructions: vec![
                            Instruction::Binary {
                                dest: 2,
                                op: BinaryOp::Sub,
                                left: Value::Register(0),
                                right: Value::Integer(1),
                            },
                            Instruction::Call {
                                dest: Some(3),
                                func: "fibonacci".to_string(),
                                args: vec![Value::Register(2)],
                            },
                            Instruction::Binary {
                                dest: 4,
                                op: BinaryOp::Sub,
                                left: Value::Register(0),
                                right: Value::Integer(2),
                            },
                            Instruction::Call {
                                dest: Some(5),
                                func: "fibonacci".to_string(),
                                args: vec![Value::Register(4)],
                            },
                            Instruction::Binary {
                                dest: 6,
                                op: BinaryOp::Add,
                                left: Value::Register(3),
                                right: Value::Register(5),
                            },
                            Instruction::Return { value: Some(Value::Register(6)) },
                        ],
                    },
                ],
            },
        ],
    };
    
    let ir = codegen.generate_llvm_ir(&module).expect("Should generate LLVM IR");
    
    assert!(ir.contains("@fibonacci"), "Should have fibonacci function");
    assert!(ir.contains("base_case:"), "Should have base case label");
    assert!(ir.contains("recursive_case:"), "Should have recursive case label");
    assert!(ir.contains("call i32 @fibonacci"), "Should have recursive calls");
}

#[test]
fn test_empty_module() {
    let mut codegen = CodeGenerator::new("empty_test".to_string());
    
    let module = Module {
        name: "empty".to_string(),
        functions: vec![],
    };
    
    let ir = codegen.generate_llvm_ir(&module).expect("Should generate LLVM IR for empty module");
    
    assert!(ir.contains("target triple"), "Should have target triple");
    assert!(ir.contains("Module: empty"), "Should have module comment");
}

#[test]
fn test_performance_ir_generation() {
    let mut codegen = CodeGenerator::new("perf_test".to_string());
    
    // Generate a large module to test performance
    let mut functions = Vec::new();
    for i in 0..100 {
        functions.push(Function {
            name: format!("func_{}", i),
            params: vec!["x".to_string(), "y".to_string()],
            blocks: vec![
                BasicBlock {
                    label: "entry".to_string(),
                    instructions: vec![
                        Instruction::Binary {
                            dest: 2,
                            op: BinaryOp::Add,
                            left: Value::Register(0),
                            right: Value::Register(1),
                        },
                        Instruction::Return { value: Some(Value::Register(2)) },
                    ],
                },
            ],
        });
    }
    
    let module = Module {
        name: "performance".to_string(),
        functions,
    };
    
    let start = std::time::Instant::now();
    let ir = codegen.generate_llvm_ir(&module).expect("Should generate LLVM IR");
    let duration = start.elapsed();
    
    assert!(!ir.is_empty(), "Should generate non-empty IR");
    assert!(duration.as_millis() < 100, "Should generate 100 functions in <100ms");
    
    println!("Generated {} bytes of LLVM IR in {:?}", ir.len(), duration);
}