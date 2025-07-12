// compiler_seen/src/optimizations/constant_folding.rs
// Implements the constant folding optimization pass.

use crate::ir::function::Function;
use crate::ir::instruction::{ComparisonPredicate, ConstantValue, FloatComparePredicate, Instruction, IntegerComparePredicate, OpCode, Operand};
use crate::ir::module::Module;
use crate::ir::types::IrType;

// --- Bilingual Note Placeholder ---
// Optimization passes operate on the IR, which is language-agnostic.
// Any language-specific nuances (e.g., differences in operator behavior, if any)
// should have been canonicalized into the IR before this stage.

/// Attempts to fold a single instruction.
/// If successful, returns the resulting ConstantValue.
fn try_fold_instruction(instr: &Instruction) -> Option<ConstantValue> {
    // Ensure there are operands to fold.
    if instr.operands.is_empty() {
        return None;
    }

    match instr.opcode {
        OpCode::Add | OpCode::Sub | OpCode::Mul | OpCode::Div | OpCode::SDiv | OpCode::UDiv |
        OpCode::Rem | OpCode::SRem | OpCode::URem |
        OpCode::And | OpCode::Or | OpCode::Xor | OpCode::Shl | OpCode::LShr | OpCode::AShr => {
            if instr.operands.len() == 2 {
                if let (Operand::Constant(op1_val), Operand::Constant(op2_val)) = (&instr.operands[0], &instr.operands[1]) {
                    return evaluate_binary_op(instr.opcode, op1_val, op2_val, instr.result_type.as_ref());
                }
            }
        }
        OpCode::Neg | OpCode::Not => {
            if instr.operands.len() == 1 {
                if let Operand::Constant(op_val) = &instr.operands[0] {
                    return evaluate_unary_op(instr.opcode, op_val, instr.result_type.as_ref());
                }
            }
        }
        OpCode::Icmp | OpCode::Fcmp => {
            if instr.operands.len() == 2 {
                if let (Operand::Constant(op1_val), Operand::Constant(op2_val)) = (&instr.operands[0], &instr.operands[1]) {
                    if let Some(predicate) = instr.predicate {
                        return evaluate_comparison(predicate, op1_val, op2_val);
                    }
                }
            }
        }
        // TODO: Handle Casts, etc.
        _ => return None, // Opcode not foldable or not yet implemented
    }
    None
}

/// Performs constant folding on a single function.
pub fn fold_constants_in_function(func: &mut Function) {
    let mut changed = true;
    // Loop until no more changes can be made in one pass (for iterative folding)
    // For now, one pass is fine, but iterative can catch more (e.g. x=2, y=3, z=x+y, w=z+1)
    while changed {
        changed = false;
        for block in &mut func.basic_blocks {
            for instr in &mut block.instructions {
                // Skip if it's already a Mov from a constant (e.g. from previous folding)
                if instr.opcode == OpCode::Mov && instr.operands.len() == 1 {
                    if matches!(instr.operands[0], Operand::Constant(_)) {
                        continue;
                    }
                }

                if let Some(folded_value) = try_fold_instruction(instr) {
                    // Ensure the folded value's type matches the instruction's result type, if any.
                    // This is a basic check; more robust type checking might be needed.
                    let types_compatible = match (instr.result_type.as_ref(), &folded_value) {
                        (Some(IrType::Bool), ConstantValue::Bool(_)) => true,
                        (Some(IrType::I8), ConstantValue::I8(_)) => true,
                        (Some(IrType::I16), ConstantValue::I16(_)) => true,
                        (Some(IrType::I32), ConstantValue::I32(_)) => true,
                        (Some(IrType::I64), ConstantValue::I64(_)) => true,
                        (Some(IrType::I128), ConstantValue::I128(_)) => true,
                        (Some(IrType::U8), ConstantValue::U8(_)) => true,
                        (Some(IrType::U16), ConstantValue::U16(_)) => true,
                        (Some(IrType::U32), ConstantValue::U32(_)) => true,
                        (Some(IrType::U64), ConstantValue::U64(_)) => true,
                        (Some(IrType::U128), ConstantValue::U128(_)) => true,
                        (Some(IrType::F32), ConstantValue::F32(_)) => true,
                        (Some(IrType::F64), ConstantValue::F64(_)) => true,
                        (None, _) => true, // No result type to check (e.g. for Store, should not happen here)
                        _ => false,
                    };

                    if types_compatible {
                        instr.opcode = OpCode::Mov;
                        instr.operands = vec![Operand::Constant(folded_value)];
                        instr.predicate = None; // No longer a comparison
                        // instr.source_span remains for debugging where the original operation was
                        changed = true;
                    }
                }
            }
        }
    }
}

/// Performs constant folding on an entire IR module.
pub fn fold_constants_in_module(module: &mut Module) {
    println!("Running constant folding pass for module: {}", module.name);
    for func in &mut module.functions {
        fold_constants_in_function(func);
    }
}

#[allow(clippy::too_many_lines, clippy::cognitive_complexity)] // TODO: Refactor later
fn evaluate_binary_op(op: OpCode, v1: &ConstantValue, v2: &ConstantValue, res_type: Option<&IrType>) -> Option<ConstantValue> {
    // Note: Using wrapping arithmetic for integers. Language spec should define behavior.
    // Division by zero: currently returns None, effectively not folding.
    match op {
        OpCode::Add => match (v1, v2) {
            (ConstantValue::I32(a), ConstantValue::I32(b)) => Some(ConstantValue::I32(a.wrapping_add(*b))),
            (ConstantValue::I64(a), ConstantValue::I64(b)) => Some(ConstantValue::I64(a.wrapping_add(*b))),
            (ConstantValue::F32(a), ConstantValue::F32(b)) => Some(ConstantValue::F32(a + b)),
            (ConstantValue::F64(a), ConstantValue::F64(b)) => Some(ConstantValue::F64(a + b)),
            // TODO: Add other integer types (I8, I16, I128, U*)
            _ => None,
        },
        OpCode::Sub => match (v1, v2) {
            (ConstantValue::I32(a), ConstantValue::I32(b)) => Some(ConstantValue::I32(a.wrapping_sub(*b))),
            (ConstantValue::I64(a), ConstantValue::I64(b)) => Some(ConstantValue::I64(a.wrapping_sub(*b))),
            (ConstantValue::F32(a), ConstantValue::F32(b)) => Some(ConstantValue::F32(a - b)),
            (ConstantValue::F64(a), ConstantValue::F64(b)) => Some(ConstantValue::F64(a - b)),
            _ => None,
        },
        OpCode::Mul => match (v1, v2) {
            (ConstantValue::I32(a), ConstantValue::I32(b)) => Some(ConstantValue::I32(a.wrapping_mul(*b))),
            (ConstantValue::I64(a), ConstantValue::I64(b)) => Some(ConstantValue::I64(a.wrapping_mul(*b))),
            (ConstantValue::F32(a), ConstantValue::F32(b)) => Some(ConstantValue::F32(a * b)),
            (ConstantValue::F64(a), ConstantValue::F64(b)) => Some(ConstantValue::F64(a * b)),
            _ => None,
        },
        OpCode::Div | OpCode::SDiv | OpCode::UDiv => match (v1, v2) {
            // Integer division by zero check
            (ConstantValue::I32(a), ConstantValue::I32(b)) if *b != 0 => Some(ConstantValue::I32(a / b)),
            (ConstantValue::I64(a), ConstantValue::I64(b)) if *b != 0 => Some(ConstantValue::I64(a / b)),
            // TODO: UDiv (unsigned) needs separate handling or rely on type info for SDiv vs UDiv choice
            // For SDiv/UDiv distinction, we'd need to know the type from res_type or operands.
            // Assuming signed division for I* types for now if SDiv is chosen, and standard for Div.
            
            // Floating point division by zero yields infinity/NaN, which is a valid ConstantValue state
            (ConstantValue::F32(a), ConstantValue::F32(b)) => Some(ConstantValue::F32(a / b)), 
            (ConstantValue::F64(a), ConstantValue::F64(b)) => Some(ConstantValue::F64(a / b)),
            _ => None, // Division by zero for integers, or type mismatch
        },
        OpCode::Rem | OpCode::SRem | OpCode::URem => match (v1, v2) {
            (ConstantValue::I32(a), ConstantValue::I32(b)) if *b != 0 => Some(ConstantValue::I32(a % b)),
            (ConstantValue::I64(a), ConstantValue::I64(b)) if *b != 0 => Some(ConstantValue::I64(a % b)),
            // TODO: URem (unsigned) needs separate handling or type info.
            // F32/F64 do not have a direct % in ConstantValue, but fmod can be used.
            // For now, not folding float remainders.
            _ => None,
        },
        // Bitwise Operations (example for I32)
        OpCode::And => match (v1, v2) {
            (ConstantValue::I32(a), ConstantValue::I32(b)) => Some(ConstantValue::I32(a & b)),
            (ConstantValue::Bool(a), ConstantValue::Bool(b)) => Some(ConstantValue::Bool(a & b)), // Logical AND for booleans
            _ => None,
        },
        OpCode::Or => match (v1, v2) {
            (ConstantValue::I32(a), ConstantValue::I32(b)) => Some(ConstantValue::I32(a | b)),
            (ConstantValue::Bool(a), ConstantValue::Bool(b)) => Some(ConstantValue::Bool(a | b)), // Logical OR for booleans
            _ => None,
        },
        OpCode::Xor => match (v1, v2) {
            (ConstantValue::I32(a), ConstantValue::I32(b)) => Some(ConstantValue::I32(a ^ b)),
            (ConstantValue::Bool(a), ConstantValue::Bool(b)) => Some(ConstantValue::Bool(a ^ b)),
            _ => None,
        },
        OpCode::Shl => match (v1, v2) {
            (ConstantValue::I32(a), ConstantValue::I32(b)) if *b >= 0 && *b < 32 => Some(ConstantValue::I32(a.wrapping_shl(*b as u32))),
            (ConstantValue::I64(a), ConstantValue::I64(b)) if *b >= 0 && *b < 64 => Some(ConstantValue::I64(a.wrapping_shl(*b as u32))),
            // TODO: Check shift amount for other types if they are u* types.
            _ => None,
        },
        OpCode::LShr | OpCode::AShr => { // Distinction might matter for signed vs unsigned based on res_type or operand type
            match (v1, v2) {
                (ConstantValue::I32(a), ConstantValue::I32(b)) if *b >= 0 && *b < 32 => {
                    if op == OpCode::AShr { Some(ConstantValue::I32(a.wrapping_shr(*b as u32))) } // Rust >> on signed is arithmetic
                    else { Some(ConstantValue::I32(((*a as u32).wrapping_shr(*b as u32)) as i32)) } // Simulate logical for signed
                }
                (ConstantValue::I64(a), ConstantValue::I64(b)) if *b >= 0 && *b < 64 => {
                    if op == OpCode::AShr { Some(ConstantValue::I64(a.wrapping_shr(*b as u32))) }
                    else { Some(ConstantValue::I64(((*a as u64).wrapping_shr(*b as u32)) as i64)) }
                }
                // TODO: U* types for LShr
                _ => None,
            }
        }
        _ => None, // Opcode not a binary op or not handled
    }
}

fn evaluate_unary_op(op: OpCode, v: &ConstantValue, _res_type: Option<&IrType>) -> Option<ConstantValue> {
    match op {
        OpCode::Neg => match v {
            ConstantValue::I32(a) => Some(ConstantValue::I32(a.wrapping_neg())),
            ConstantValue::I64(a) => Some(ConstantValue::I64(a.wrapping_neg())),
            ConstantValue::F32(a) => Some(ConstantValue::F32(-a)),
            ConstantValue::F64(a) => Some(ConstantValue::F64(-a)),
            _ => None,
        },
        OpCode::Not => match v {
            ConstantValue::I32(a) => Some(ConstantValue::I32(!a)), // Bitwise NOT
            ConstantValue::I64(a) => Some(ConstantValue::I64(!a)), // Bitwise NOT
            ConstantValue::Bool(a) => Some(ConstantValue::Bool(!a)), // Logical NOT
            _ => None,
        },
        _ => None,
    }
}

fn evaluate_comparison(predicate: ComparisonPredicate, v1: &ConstantValue, v2: &ConstantValue) -> Option<ConstantValue> {
    match predicate {
        ComparisonPredicate::Int(int_pred) => {
            // Promote to i64 for comparison if types are different but compatible integers (simplification)
            // A more robust solution would handle all type pairs or use type from operands.
            let val1_i64 = v1.as_i64()?; 
            let val2_i64 = v2.as_i64()?;
            match int_pred {
                IntegerComparePredicate::EQ => Some(ConstantValue::Bool(val1_i64 == val2_i64)),
                IntegerComparePredicate::NE => Some(ConstantValue::Bool(val1_i64 != val2_i64)),
                IntegerComparePredicate::SGT => Some(ConstantValue::Bool(val1_i64 > val2_i64)),
                IntegerComparePredicate::SGE => Some(ConstantValue::Bool(val1_i64 >= val2_i64)),
                IntegerComparePredicate::SLT => Some(ConstantValue::Bool(val1_i64 < val2_i64)),
                IntegerComparePredicate::SLE => Some(ConstantValue::Bool(val1_i64 <= val2_i64)),
                // UGT, UGE, ULT, ULE would need unsigned comparison logic (e.g. v1.as_u64())
                _ => None, // Unsigned predicates not fully handled here yet
            }
        }
        ComparisonPredicate::Float(float_pred) => {
            let val1_f64 = v1.as_f64()?;
            let val2_f64 = v2.as_f64()?;
            // Note: LLVM's FCmp predicates (OEQ, OGT, etc.) handle NaN according to IEEE 754 'ordered' rules.
            // Direct Rust comparison (==, >, etc.) on floats also largely follows this.
            match float_pred {
                FloatComparePredicate::OEQ => Some(ConstantValue::Bool(val1_f64 == val2_f64)),
                FloatComparePredicate::ONE => Some(ConstantValue::Bool(val1_f64 != val2_f64)),
                FloatComparePredicate::OGT => Some(ConstantValue::Bool(val1_f64 > val2_f64)),
                FloatComparePredicate::OGE => Some(ConstantValue::Bool(val1_f64 >= val2_f64)),
                FloatComparePredicate::OLT => Some(ConstantValue::Bool(val1_f64 < val2_f64)),
                FloatComparePredicate::OLE => Some(ConstantValue::Bool(val1_f64 <= val2_f64)),
                // Unordered predicates (UNO, UEQ etc.) require checking for NaN explicitly.
                // For example, UEQ is true if (v1 is NaN or v2 is NaN or v1 == v2)
                _ => None, // Unordered predicates not fully handled
            }
        }
    }
}

// Helper methods for ConstantValue to facilitate comparisons
impl ConstantValue {
    fn as_i64(&self) -> Option<i64> {
        match *self {
            ConstantValue::I8(v) => Some(v as i64),
            ConstantValue::I16(v) => Some(v as i64),
            ConstantValue::I32(v) => Some(v as i64),
            ConstantValue::I64(v) => Some(v),
            // Add U* types if they should be comparable to signed, or have separate as_u64
            _ => None,
        }
    }
    fn as_f64(&self) -> Option<f64> {
        match *self {
            ConstantValue::F32(v) => Some(v as f64),
            ConstantValue::F64(v) => Some(v),
            _ => None,
        }
    }
    // fn as_u64(&self) -> Option<u64> { ... } // For unsigned comparisons
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::basic_block::BasicBlockId;
    use crate::ir::function::Function;
    use crate::ir::instruction::{ConstantValue, Instruction, OpCode, Operand, VirtualRegister};
    use crate::ir::types::IrType;

    // Helper to create a simple function with one basic block and specific instructions
    fn setup_function_with_instructions(instructions: Vec<Instruction>) -> Function {
        let mut func = Function::new("test_func".to_string(), IrType::Void);
        let bb_id = func.new_basic_block();
        let bb = func.get_basic_block_mut(bb_id).unwrap();
        for instr in instructions {
            bb.add_instruction(instr);
        }
        func
    }

    #[test]
    fn test_fold_simple_add() {
        // Create an instruction: result_reg = 1 + 2
        let r0 = VirtualRegister { id: 0 };
        let r1 = VirtualRegister { id: 1 };
        let r2 = VirtualRegister { id: 2 };

        // Setup: Create a function that effectively has:
        // %r0 = mov 1
        // %r1 = mov 2
        // %r2 = add %r0, %r1
        // We will directly create the add instruction with constant operands for this unit test
        // to focus on the try_fold_instruction logic.
        // A more complete test would involve running the pass on a function with Mov instructions first.

        let add_instr = Instruction {
            opcode: OpCode::Add,
            operands: vec![
                Operand::Constant(ConstantValue::I32(1)),
                Operand::Constant(ConstantValue::I32(2)),
            ],
            result: Some(r2),
            result_type: Some(IrType::I32),
            parent_block: Some(BasicBlockId(0)), // Dummy parent block ID for the test
            source_span: None,
            predicate: None,
        };

        let mut func = Function::new("test_add_fold".to_string(), IrType::I32);
        let bb_id = func.new_basic_block();
        let main_block = func.get_basic_block_mut(bb_id).unwrap();
        main_block.add_instruction(add_instr);
        
        // Run the constant folding pass on the function
        fold_constants_in_function(&mut func);

        // Check the instruction in the block
        let folded_instr = &func.get_basic_block(bb_id).unwrap().instructions[0];

        assert_eq!(folded_instr.opcode, OpCode::Mov);
        match &folded_instr.operands[0] {
            Operand::Constant(ConstantValue::I32(val)) => assert_eq!(*val, 3),
            _ => panic!("Expected constant I32 operand after folding"),
        }
        assert_eq!(folded_instr.result, Some(r2));
    }

    #[test]
    fn test_fold_division_by_zero() {
        let r0 = VirtualRegister { id: 0 };
        let div_instr = Instruction {
            opcode: OpCode::Div,
            operands: vec![
                Operand::Constant(ConstantValue::I32(10)),
                Operand::Constant(ConstantValue::I32(0)), // Division by zero
            ],
            result: Some(r0),
            result_type: Some(IrType::I32),
            parent_block: Some(BasicBlockId(0)),
            source_span: None,
            predicate: None,
        };

        let mut func = Function::new("test_div_zero".to_string(), IrType::I32);
        let bb_id = func.new_basic_block();
        let main_block = func.get_basic_block_mut(bb_id).unwrap();
        main_block.add_instruction(div_instr.clone()); // Clone because we assert original opcode later

        fold_constants_in_function(&mut func);

        let potentially_folded_instr = &func.get_basic_block(bb_id).unwrap().instructions[0];
        // Division by zero should not be folded and the original instruction should remain.
        assert_eq!(potentially_folded_instr.opcode, OpCode::Div);
        assert_eq!(potentially_folded_instr.operands, div_instr.operands);
    }
}
