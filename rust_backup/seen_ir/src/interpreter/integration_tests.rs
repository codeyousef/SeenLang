//! Integration tests for the IR interpreter
//!
//! These tests create IR modules and execute them to verify correctness.

use crate::instruction::{Instruction, BinaryOp, UnaryOp, Label, BasicBlock, ControlFlowGraph};
use crate::function::{IRFunction, Parameter};
use crate::module::IRModule;
use crate::value::{IRValue, IRType};
use crate::interpreter::{Interpreter, InterpreterConfig, InterpreterValue, IRValidator};

/// Helper to create a simple function with one block
fn simple_function(name: &str, instructions: Vec<Instruction>, terminator: Option<Instruction>) -> IRFunction {
    let mut func = IRFunction::new(name, IRType::Void);
    func.is_public = true;
    
    let mut block = BasicBlock::new(Label::new("entry"));
    for inst in instructions {
        block.add_instruction(inst);
    }
    if let Some(term) = terminator {
        block.add_instruction(term);
    }
    func.cfg.add_block(block);
    func
}

#[test]
fn test_simple_return() {
    let mut module = IRModule::new("test");
    
    // Create function that returns 42
    let func = simple_function(
        "main",
        vec![],
        Some(Instruction::Return(Some(IRValue::Integer(42))))
    );
    module.add_function(func);

    let mut interp = Interpreter::new();
    let result = interp.execute_function(&module, "main", vec![]).unwrap();
    
    assert_eq!(result.as_integer().unwrap(), 42);
}

#[test]
fn test_arithmetic() {
    let mut module = IRModule::new("test");
    
    // Create function: return 10 + 20
    let func = simple_function(
        "add_numbers",
        vec![
            Instruction::Binary {
                op: BinaryOp::Add,
                left: IRValue::Integer(10),
                right: IRValue::Integer(20),
                result: IRValue::Variable("result".to_string()),
            },
        ],
        Some(Instruction::Return(Some(IRValue::Variable("result".to_string()))))
    );
    module.add_function(func);

    let mut interp = Interpreter::new();
    let result = interp.execute_function(&module, "add_numbers", vec![]).unwrap();
    
    assert_eq!(result.as_integer().unwrap(), 30);
}

#[test]
fn test_multiple_operations() {
    let mut module = IRModule::new("test");
    
    // Create function: (5 * 4) - 3 = 17
    let func = simple_function(
        "compute",
        vec![
            Instruction::Binary {
                op: BinaryOp::Multiply,
                left: IRValue::Integer(5),
                right: IRValue::Integer(4),
                result: IRValue::Variable("temp".to_string()),
            },
            Instruction::Binary {
                op: BinaryOp::Subtract,
                left: IRValue::Variable("temp".to_string()),
                right: IRValue::Integer(3),
                result: IRValue::Variable("result".to_string()),
            },
        ],
        Some(Instruction::Return(Some(IRValue::Variable("result".to_string()))))
    );
    module.add_function(func);

    let mut interp = Interpreter::new();
    let result = interp.execute_function(&module, "compute", vec![]).unwrap();
    
    assert_eq!(result.as_integer().unwrap(), 17);
}

#[test]
fn test_function_with_parameters() {
    let mut module = IRModule::new("test");
    
    // Create function: fn add(a, b) -> a + b
    let mut func = IRFunction::new("add", IRType::Integer);
    func.is_public = true;
    func.parameters = vec![
        Parameter::new("a", IRType::Integer),
        Parameter::new("b", IRType::Integer),
    ];
    
    let mut block = BasicBlock::new(Label::new("entry"));
    block.add_instruction(Instruction::Binary {
        op: BinaryOp::Add,
        left: IRValue::Variable("a".to_string()),
        right: IRValue::Variable("b".to_string()),
        result: IRValue::Variable("sum".to_string()),
    });
    block.add_instruction(Instruction::Return(Some(IRValue::Variable("sum".to_string()))));
    func.cfg.add_block(block);
    
    module.add_function(func);

    let mut interp = Interpreter::new();
    let result = interp.execute_function(
        &module, 
        "add", 
        vec![InterpreterValue::integer(100), InterpreterValue::integer(200)]
    ).unwrap();
    
    assert_eq!(result.as_integer().unwrap(), 300);
}

#[test]
fn test_conditional_jump() {
    let mut module = IRModule::new("test");
    
    // Create function with conditional:
    // if true { return 1 } else { return 0 }
    let mut func = IRFunction::new("conditional", IRType::Integer);
    func.is_public = true;
    
    // Entry block
    let mut entry = BasicBlock::new(Label::new("entry"));
    entry.add_instruction(Instruction::JumpIf {
        condition: IRValue::Boolean(true),
        target: Label::new("then"),
    });
    func.cfg.add_block(entry);
    
    // Then block
    let mut then_block = BasicBlock::new(Label::new("then"));
    then_block.add_instruction(Instruction::Return(Some(IRValue::Integer(1))));
    func.cfg.add_block(then_block);
    
    // Else block (fallthrough)
    let mut else_block = BasicBlock::new(Label::new("else"));
    else_block.add_instruction(Instruction::Return(Some(IRValue::Integer(0))));
    func.cfg.add_block(else_block);

    module.add_function(func);

    let mut interp = Interpreter::new();
    let result = interp.execute_function(&module, "conditional", vec![]).unwrap();
    
    assert_eq!(result.as_integer().unwrap(), 1);
}

#[test]
fn test_comparison_operations() {
    let mut module = IRModule::new("test");
    
    // Test: 5 < 10 should be true
    let func = simple_function(
        "less_than",
        vec![
            Instruction::Binary {
                op: BinaryOp::LessThan,
                left: IRValue::Integer(5),
                right: IRValue::Integer(10),
                result: IRValue::Variable("result".to_string()),
            },
        ],
        Some(Instruction::Return(Some(IRValue::Variable("result".to_string()))))
    );
    module.add_function(func);

    let mut interp = Interpreter::new();
    let result = interp.execute_function(&module, "less_than", vec![]).unwrap();
    
    assert!(result.as_boolean().unwrap());
}

#[test]
fn test_string_operations() {
    let mut module = IRModule::new("test");
    
    // Test string concat
    let func = simple_function(
        "concat_strings",
        vec![
            Instruction::StringConcat {
                left: IRValue::String("Hello, ".to_string()),
                right: IRValue::String("World!".to_string()),
                result: IRValue::Variable("result".to_string()),
            },
        ],
        Some(Instruction::Return(Some(IRValue::Variable("result".to_string()))))
    );
    module.add_function(func);

    let mut interp = Interpreter::new();
    let result = interp.execute_function(&module, "concat_strings", vec![]).unwrap();
    
    assert_eq!(result.as_string().unwrap(), "Hello, World!");
}

#[test]
fn test_unary_negation() {
    let mut module = IRModule::new("test");
    
    let func = simple_function(
        "negate",
        vec![
            Instruction::Unary {
                op: UnaryOp::Negate,
                operand: IRValue::Integer(42),
                result: IRValue::Variable("result".to_string()),
            },
        ],
        Some(Instruction::Return(Some(IRValue::Variable("result".to_string()))))
    );
    module.add_function(func);

    let mut interp = Interpreter::new();
    let result = interp.execute_function(&module, "negate", vec![]).unwrap();
    
    assert_eq!(result.as_integer().unwrap(), -42);
}

#[test]
fn test_division_by_zero_error() {
    let mut module = IRModule::new("test");
    
    let func = simple_function(
        "divide_zero",
        vec![
            Instruction::Binary {
                op: BinaryOp::Divide,
                left: IRValue::Integer(10),
                right: IRValue::Integer(0),
                result: IRValue::Variable("result".to_string()),
            },
        ],
        Some(Instruction::Return(Some(IRValue::Variable("result".to_string()))))
    );
    module.add_function(func);

    let mut interp = Interpreter::new();
    let result = interp.execute_function(&module, "divide_zero", vec![]);
    
    assert!(result.is_err());
}

#[test]
fn test_instruction_limit() {
    let mut module = IRModule::new("test");
    
    // Create an infinite loop (for testing purposes)
    let mut func = IRFunction::new("infinite", IRType::Void);
    func.is_public = true;
    
    let mut block = BasicBlock::new(Label::new("loop"));
    block.add_instruction(Instruction::Jump(Label::new("loop")));
    func.cfg.add_block(block);

    module.add_function(func);

    // Use a config with low instruction limit
    let config = InterpreterConfig {
        max_instructions: Some(100),
        ..InterpreterConfig::default()
    };
    let mut interp = Interpreter::with_config(config);
    let result = interp.execute_function(&module, "infinite", vec![]);
    
    assert!(result.is_err()); // Should hit instruction limit
}

#[test]
fn test_print_capture() {
    let mut module = IRModule::new("test");
    
    let func = simple_function(
        "print_test",
        vec![
            Instruction::Print(IRValue::String("Hello!".to_string())),
        ],
        Some(Instruction::Return(None))
    );
    module.add_function(func);

    // This test verifies print doesn't crash
    let mut interp = Interpreter::new();
    let result = interp.execute_function(&module, "print_test", vec![]);
    
    assert!(result.is_ok());
}

#[test]
fn test_validation_passes_valid_ir() {
    let mut module = IRModule::new("test");
    
    let func = simple_function(
        "valid_func",
        vec![
            Instruction::Binary {
                op: BinaryOp::Add,
                left: IRValue::Integer(1),
                right: IRValue::Integer(2),
                result: IRValue::Variable("x".to_string()),
            },
        ],
        Some(Instruction::Return(Some(IRValue::Variable("x".to_string()))))
    );
    module.add_function(func);

    let mut validator = IRValidator::new();
    let result = validator.validate_module(&module);
    
    assert!(result.is_valid());
}

#[test]
fn test_undefined_function_detection() {
    let mut module = IRModule::new("test");
    
    // Call an undefined function
    let func = simple_function(
        "call_undefined",
        vec![
            Instruction::Call {
                target: IRValue::Function { 
                    name: "nonexistent".to_string(), 
                    parameters: vec![] 
                },
                args: vec![],
                result: None,
                arg_types: None,
                return_type: None,
            },
        ],
        Some(Instruction::Return(None))
    );
    module.add_function(func);

    let mut validator = IRValidator::new();
    let result = validator.validate_module(&module);
    
    assert!(!result.is_valid());
    assert!(result.errors.iter().any(|e| 
        matches!(e.kind, crate::interpreter::validation::ValidationErrorKind::UndefinedFunction)
    ));
}
