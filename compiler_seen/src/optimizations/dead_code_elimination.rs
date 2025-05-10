// compiler_seen/src/optimizations/dead_code_elimination.rs
// Implements the dead code elimination optimization pass.

use crate::ir::function::Function;
use crate::ir::module::Module;
use crate::ir::basic_block::BasicBlockId;
use crate::ir::instruction::{Instruction, OpCode};
use crate::ir::types::IrType;
use crate::analysis::liveness::{analyze_liveness, LivenessResult};
use std::collections::{HashSet, VecDeque};

// --- Bilingual Note Placeholder ---
// Dead code elimination operates on the IR. Constants or conditions evaluated
// to be always true/false by prior passes (like constant folding) can make
// entire blocks of code unreachable, which this pass aims to remove.

/// Checks if an instruction can be safely removed if its result is unused.
/// This is a simplified check. A more robust check would consider volatility, function calls, etc.
fn instruction_has_no_side_effects_for_dse(instr: &Instruction) -> bool {
    match instr.opcode {
        OpCode::Store | OpCode::Call => false, // These have side effects beyond their result register.
        // Alloc might be considered to have side effects (memory allocation), but if the resulting
        // pointer is not used, the allocation might be dead. For now, treat as side-effecting.
        OpCode::Alloc => false,
        // Terminator instructions are not removed by DSE based on their result.
        OpCode::Br | OpCode::BrCond | OpCode::Return | OpCode::Unreachable => false,
        // Other instructions are generally side-effect free if their result is not used.
        // Load can have side effects (e.g., volatile loads), but we simplify for now.
        _ => true,
    }
}

/// Performs dead code elimination on a single function.
pub fn eliminate_dead_code_in_function(func: &mut Function) {
    // --- 1. Unreachable Code Elimination ---
    if func.entry_block_id.is_none() || func.basic_blocks.is_empty() {
        return;
    }
    let mut reachable_blocks = HashSet::new();
    let mut work_list = VecDeque::new();
    let entry_id = func.entry_block_id.unwrap();
    work_list.push_back(entry_id);
    reachable_blocks.insert(entry_id);

    while let Some(current_block_id) = work_list.pop_front() {
        if let Some(current_block) = func.get_basic_block(current_block_id) {
            if let Some(terminator) = current_block.get_terminator() {
                match terminator.opcode {
                    OpCode::Br => {
                        if let Some(crate::ir::instruction::Operand::Label(target_id)) = terminator.operands.get(0) {
                            if reachable_blocks.insert(*target_id) {
                                work_list.push_back(*target_id);
                            }
                        }
                    }
                    OpCode::BrCond => {
                        if let Some(crate::ir::instruction::Operand::Label(true_target_id)) = terminator.operands.get(1) {
                            if reachable_blocks.insert(*true_target_id) {
                                work_list.push_back(*true_target_id);
                            }
                        }
                        if let Some(crate::ir::instruction::Operand::Label(false_target_id)) = terminator.operands.get(2) {
                            if reachable_blocks.insert(*false_target_id) {
                                work_list.push_back(*false_target_id);
                            }
                        }
                    }
                    _ => {}
                }
            }
        } else {
            // log warning
        }
    }
    let original_block_count = func.basic_blocks.len();
    func.basic_blocks.retain(|block| reachable_blocks.contains(&block.id));
    if func.basic_blocks.len() < original_block_count {
        let remaining_block_ids: HashSet<BasicBlockId> = func.basic_blocks.iter().map(|b| b.id).collect();
        for block in &mut func.basic_blocks {
            block.predecessors.retain(|pred_id| remaining_block_ids.contains(pred_id));
            block.successors.retain(|succ_id| remaining_block_ids.contains(succ_id));
        }
        println!("DCE: Removed {} unreachable blocks from function '{}'.", original_block_count - func.basic_blocks.len(), func.name);
    } else if original_block_count > 0 && original_block_count == func.basic_blocks.len() {
        // Check added to prevent printing if basic_blocks was empty to begin with
        println!("DCE: No unreachable blocks found in function '{}'.", func.name);
    }

    // --- 2. Dead Store Elimination ---
    // This should ideally be iterative with liveness analysis until no more changes occur.
    // For now, run one pass of liveness analysis and DSE.
    let liveness_info = analyze_liveness(func);
    let mut instructions_removed_count = 0;

    for block in &mut func.basic_blocks {
        let mut new_instructions = Vec::new();
        let num_instr_in_block = block.instructions.len();

        for instr_idx in 0..num_instr_in_block {
            let instr = &block.instructions[instr_idx];
            let mut dead_store = false;

            if let Some(defined_reg) = instr.get_defined_register() {
                // Check liveness *after* the current instruction
                if !liveness_info.is_live_after_instr(defined_reg, instr_idx, block.id, func) {
                    if instruction_has_no_side_effects_for_dse(instr) {
                        dead_store = true;
                        instructions_removed_count += 1;
                        println!("DCE: Identified dead store in func '{}', block {}: {:?}", func.name, block.id.0, instr);
                    }
                }
            }

            if !dead_store {
                new_instructions.push(instr.clone());
            }
        }
        block.instructions = new_instructions;
    }

    if instructions_removed_count > 0 {
        println!(
            "DCE: Removed {} dead store instructions from function '{}'.",
            instructions_removed_count, func.name
        );
    } else if original_block_count > 0 { // Only print if there were blocks to begin with
         println!("DCE: No dead stores found in function '{}'.", func.name);
    }
}

/// Performs dead code elimination on an entire IR module.
pub fn eliminate_dead_code_in_module(module: &mut Module) {
    println!("Running dead code elimination pass for module: {}", module.name);
    for func in &mut module.functions {
        eliminate_dead_code_in_function(func);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::function::Function;
    use crate::ir::basic_block::{BasicBlock, BasicBlockId};
    use crate::ir::instruction::{Instruction, OpCode, Operand, VirtualRegister, ConstantValue};
    use crate::ir::types::IrType;

    #[test]
    fn test_remove_unreachable_block() {
        let mut func = Function::new("test_unreachable".to_string(), IrType::Void);

        // Entry block (B0)
        let b0_id = func.new_basic_block(); // Becomes entry block
        
        // Reachable block (B1), successor of B0
        let b1_id = func.new_basic_block();

        // Unreachable block (B2)
        let b2_id = func.new_basic_block();

        // Setup B0: br B1
        let b0 = func.get_basic_block_mut(b0_id).unwrap();
        b0.add_instruction(Instruction {
            opcode: OpCode::Br,
            operands: vec![Operand::Label(b1_id)],
            result: None,
            result_type: None,
            parent_block: Some(b0_id),
            source_span: None,
            predicate: None,
        });
        b0.successors.push(b1_id);

        // Setup B1: (no explicit terminator, implicitly ends, or add a Ret for completeness)
        let b1 = func.get_basic_block_mut(b1_id).unwrap();
        b1.predecessors.push(b0_id);
        b1.add_instruction(Instruction { // Add a dummy instruction
            opcode: OpCode::Mov,
            operands: vec![Operand::Constant(ConstantValue::I32(1))],
            result: Some(func.new_virtual_register()),
            result_type: Some(IrType::I32),
            parent_block: Some(b1_id),
            source_span: None,
            predicate: None,
        });
         b1.add_instruction(Instruction { // Add a Ret terminator
            opcode: OpCode::Return,
            operands: vec![],
            result: None,
            result_type: None,
            parent_block: Some(b1_id),
            source_span: None,
            predicate: None,
        });

        // B2 is not connected to anything from B0 or B1

        assert_eq!(func.basic_blocks.len(), 3);

        eliminate_dead_code_in_function(&mut func);

        assert_eq!(func.basic_blocks.len(), 2, "Function should have 2 blocks after DCE");
        assert!(func.get_basic_block(b0_id).is_some(), "B0 should remain");
        assert!(func.get_basic_block(b1_id).is_some(), "B1 should remain");
        assert!(func.get_basic_block(b2_id).is_none(), "B2 should be removed");

        // Check successor/predecessor integrity
        let b0_after_dce = func.get_basic_block(b0_id).unwrap();
        assert_eq!(b0_after_dce.successors, vec![b1_id]);
        
        let b1_after_dce = func.get_basic_block(b1_id).unwrap();
        assert_eq!(b1_after_dce.predecessors, vec![b0_id]);
    }

    #[test]
    fn test_remove_simple_dead_store() {
        let mut func = Function::new("test_simple_dead_store".to_string(), IrType::Void);
        let vr0 = func.new_virtual_register(); // Target of dead store
        let vr1 = func.new_virtual_register(); // Used register

        let b0_id = func.new_basic_block();
        let b0 = func.get_basic_block_mut(b0_id).unwrap();

        // vr0 = 10  (dead store)
        b0.add_instruction(Instruction {
            opcode: OpCode::Mov,
            operands: vec![Operand::Constant(ConstantValue::I32(10))],
            result: Some(vr0),
            result_type: Some(IrType::I32),
            parent_block: Some(b0_id), source_span: None, predicate: None,
        });

        // vr1 = 20 (used store)
        b0.add_instruction(Instruction {
            opcode: OpCode::Mov,
            operands: vec![Operand::Constant(ConstantValue::I32(20))],
            result: Some(vr1),
            result_type: Some(IrType::I32),
            parent_block: Some(b0_id), source_span: None, predicate: None,
        });

        // Use vr1: ret vr1 (or some other op that uses vr1)
        b0.add_instruction(Instruction {
            opcode: OpCode::Return, // Using Return to implicitly mark vr1 as live if it's the return value
            operands: vec![Operand::Register(vr1)], // For simplicity, assume Return uses its operand
            result: None,
            result_type: None,
            parent_block: Some(b0_id), source_span: None, predicate: None,
        });

        eliminate_dead_code_in_function(&mut func);

        let b0_after_dce = func.get_basic_block(b0_id).unwrap();
        assert_eq!(b0_after_dce.instructions.len(), 2, "Should have 2 instructions after DSE");
        // The first instruction (mov vr0, 10) should be removed.
        // The second instruction (mov vr1, 20) should remain.
        assert_eq!(b0_after_dce.instructions[0].result, Some(vr1));
        assert_eq!(b0_after_dce.instructions[1].opcode, OpCode::Return);
    }

    #[test]
    fn test_keep_used_store() {
        let mut func = Function::new("test_keep_used_store".to_string(), IrType::Void);
        let vr0 = func.new_virtual_register();

        let b0_id = func.new_basic_block();
        let b0 = func.get_basic_block_mut(b0_id).unwrap();

        // vr0 = 10
        b0.add_instruction(Instruction {
            opcode: OpCode::Mov,
            operands: vec![Operand::Constant(ConstantValue::I32(10))],
            result: Some(vr0),
            result_type: Some(IrType::I32),
            parent_block: Some(b0_id), source_span: None, predicate: None,
        });

        // Use vr0: ret vr0
        b0.add_instruction(Instruction {
            opcode: OpCode::Return,
            operands: vec![Operand::Register(vr0)],
            result: None,
            result_type: None,
            parent_block: Some(b0_id), source_span: None, predicate: None,
        });

        eliminate_dead_code_in_function(&mut func);

        let b0_after_dce = func.get_basic_block(b0_id).unwrap();
        assert_eq!(b0_after_dce.instructions.len(), 2, "Should still have 2 instructions");
        assert_eq!(b0_after_dce.instructions[0].result, Some(vr0));
    }

    #[test]
    fn test_keep_side_effect_instruction() {
        let mut func = Function::new("test_side_effect".to_string(), IrType::Void);
        let vr_addr = func.new_virtual_register(); // Some address register
        let vr_val_unused_result = func.new_virtual_register(); // Result of a Load, potentially unused

        let b0_id = func.new_basic_block();
        let b0 = func.get_basic_block_mut(b0_id).unwrap();

        // Assume vr_addr is initialized somewhere (e.g. an Alloca)
        // For this test, we'll focus on the Store and Call instructions.

        // store some_val, vr_addr (side effect, no result register to check for liveness for DSE itself)
        b0.add_instruction(Instruction {
            opcode: OpCode::Store,
            operands: vec![Operand::Constant(ConstantValue::I32(5)), Operand::Register(vr_addr)],
            result: None, // Store has no result register
            result_type: None,
            parent_block: Some(b0_id), source_span: None, predicate: None,
        });

        // vr_val_unused_result = call some_func() (Call has side effects)
        // Even if vr_val_unused_result is not live, the Call instruction should remain.
        b0.add_instruction(Instruction {
            opcode: OpCode::Call,
            operands: vec![Operand::Constant(ConstantValue::StringLiteral("some_func".to_string()))], // Simplified Call
            result: Some(vr_val_unused_result),
            result_type: Some(IrType::I32), // Assuming func returns i32
            parent_block: Some(b0_id), source_span: None, predicate: None,
        });

        // ret
        b0.add_instruction(Instruction {
            opcode: OpCode::Return,
            operands: vec![],
            result: None, result_type: None,
            parent_block: Some(b0_id), source_span: None, predicate: None,
        });
        
        let original_instruction_count = b0.instructions.len();
        eliminate_dead_code_in_function(&mut func);

        let b0_after_dce = func.get_basic_block(b0_id).unwrap();
        assert_eq!(b0_after_dce.instructions.len(), original_instruction_count, "Side-effecting instructions should remain even if result unused");
        assert_eq!(b0_after_dce.instructions[0].opcode, OpCode::Store);
        assert_eq!(b0_after_dce.instructions[1].opcode, OpCode::Call);
    }
}
