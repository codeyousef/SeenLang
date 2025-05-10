// compiler_seen/src/analysis/liveness.rs
// Implements liveness analysis for virtual registers.

use crate::ir::function::Function;
use crate::ir::basic_block::BasicBlockId;
use crate::ir::instruction::VirtualRegister;
use std::collections::{HashMap, HashSet};

// --- Bilingual Note Placeholder ---
// Liveness analysis determines, for each point in a program, which variables (here, virtual registers)
// hold a value that might be used in the future. If a register is assigned a value but is not live
// immediately after the assignment, that assignment is a "dead store" and can be removed.

/// Stores the results of liveness analysis for a single function.
#[derive(Debug, Default)]
pub struct LivenessResult {
    /// Maps each basic block ID to the set of registers live at its entry.
    pub live_in: HashMap<BasicBlockId, HashSet<VirtualRegister>>,
    /// Maps each basic block ID to the set of registers live at its exit.
    pub live_out: HashMap<BasicBlockId, HashSet<VirtualRegister>>,
}

impl LivenessResult {
    pub fn new() -> Self {
        LivenessResult { live_in: HashMap::new(), live_out: HashMap::new() }
    }

    /// Checks if a register is live immediately before a specific instruction in a block.
    /// This would require more detailed per-instruction liveness, not just per-block.
    /// For now, this is a placeholder for more advanced queries.
    pub fn is_live_before_instr(&self, _reg: VirtualRegister, _instr_idx: usize, _block_id: BasicBlockId) -> bool {
        // TODO: Implement this by calculating liveness backward through the block from live_out[block_id]
        unimplemented!("Per-instruction liveness check not yet implemented.");
    }

    /// Checks if a register is live immediately after a specific instruction in a block.
    pub fn is_live_after_instr(&self, reg: VirtualRegister, instr_idx: usize, block_id: BasicBlockId, func: &Function) -> bool {
        if let Some(block) = func.get_basic_block(block_id) {
            if instr_idx + 1 >= block.instructions.len() {
                // After the last instruction (or if instruction is the last one), liveness is determined by live_out of the block.
                return self.live_out.get(&block_id).map_or(false, |s| s.contains(&reg));
            }

            // Calculate liveness backwards from the end of the block (or next instruction) to this point.
            let mut current_live_set = self.live_out.get(&block_id).cloned().unwrap_or_default();

            for i in (instr_idx + 1..block.instructions.len()).rev() {
                let current_instr = &block.instructions[i];
                // Remove defined register
                if let Some(defined_reg) = current_instr.get_defined_register() {
                    current_live_set.remove(&defined_reg);
                }
                // Add used registers
                for used_reg in current_instr.get_used_registers() {
                    current_live_set.insert(used_reg);
                }
            }
            // After iterating through subsequent instructions, check if 'reg' (defined by original instr_idx) is used.
            // The liveness we need is *immediately after* instr_idx, so we compute based on instructions *after* it.
            // This means the logic above IS correct for `is_live_after_instr` because it processes instructions *after* `instr_idx`.
            return current_live_set.contains(&reg);
        }
        false // Block not found
    }
}

/// Performs liveness analysis on a function.
/// 
/// This is a standard iterative dataflow analysis algorithm.
/// It computes `live_in` and `live_out` sets for each basic block.
pub fn analyze_liveness(func: &Function) -> LivenessResult {
    let mut liveness_result = LivenessResult::new();
    if func.basic_blocks.is_empty() {
        return liveness_result;
    }

    let mut use_map: HashMap<BasicBlockId, HashSet<VirtualRegister>> = HashMap::new();
    let mut def_map: HashMap<BasicBlockId, HashSet<VirtualRegister>> = HashMap::new();
    
    let block_ids: Vec<BasicBlockId> = func.basic_blocks.iter().map(|bb| bb.id).collect();

    // 1. Compute USE and DEF sets for each basic block
    for bb in &func.basic_blocks {
        let mut block_use = HashSet::new();
        let mut block_def = HashSet::new();
        liveness_result.live_in.insert(bb.id, HashSet::new()); // Initialize live_in
        liveness_result.live_out.insert(bb.id, HashSet::new()); // Initialize live_out

        for instr in &bb.instructions {
            // Add to USE if not defined yet in this block
            for used_reg in instr.get_used_registers() {
                if !block_def.contains(&used_reg) {
                    block_use.insert(used_reg);
                }
            }
            // Add to DEF
            if let Some(defined_reg) = instr.get_defined_register() {
                block_def.insert(defined_reg);
            }
        }
        use_map.insert(bb.id, block_use);
        def_map.insert(bb.id, block_def);
    }

    // 2. Iteratively compute live_in and live_out
    let mut changed = true;
    while changed {
        changed = false;
        // Process blocks in reverse order (approximates reverse post-order for simplicity for now)
        // A true reverse post-order traversal of CFG would be better for convergence.
        for &block_id in block_ids.iter().rev() { // Iterating over a clone or collected IDs is safer if modifying func
            // live_out[B] = Union of live_in[S] for all successors S of B
            let mut new_live_out = HashSet::new();
            if let Some(block) = func.get_basic_block(block_id) {
                // Successors determined by terminator instruction
                if let Some(terminator) = block.get_terminator() {
                    match terminator.opcode {
                        crate::ir::instruction::OpCode::Br => {
                            if let Some(crate::ir::instruction::Operand::Label(succ_id)) = terminator.operands.get(0) {
                                if let Some(live_in_succ) = liveness_result.live_in.get(succ_id) {
                                    new_live_out.extend(live_in_succ);
                                }
                            }
                        }
                        crate::ir::instruction::OpCode::BrCond => {
                            if let Some(crate::ir::instruction::Operand::Label(true_succ_id)) = terminator.operands.get(1) {
                                if let Some(live_in_succ) = liveness_result.live_in.get(true_succ_id) {
                                    new_live_out.extend(live_in_succ);
                                }
                            }
                            if let Some(crate::ir::instruction::Operand::Label(false_succ_id)) = terminator.operands.get(2) {
                                if let Some(live_in_succ) = liveness_result.live_in.get(false_succ_id) {
                                    new_live_out.extend(live_in_succ);
                                }
                            }
                        }
                        crate::ir::instruction::OpCode::Return => {
                            // For return, live_out is typically empty unless there are global side effects
                            // or if return value is considered live out of the function.
                            // For intra-procedural liveness, it's often empty.
                        }
                        _ => {}
                    }
                }
            }
            
            let current_live_out = liveness_result.live_out.get(&block_id).unwrap(); // Should exist
            if *current_live_out != new_live_out {
                liveness_result.live_out.insert(block_id, new_live_out.clone());
                changed = true;
            }

            // live_in[B] = use[B] U (live_out[B] - def[B])
            let live_out_b = &new_live_out; // Use the potentially updated live_out for this iteration
            let def_b = def_map.get(&block_id).unwrap(); // Should exist
            let use_b = use_map.get(&block_id).unwrap(); // Should exist

            let mut live_out_minus_def = live_out_b.clone();
            for def_reg in def_b {
                live_out_minus_def.remove(def_reg);
            }
            
            let mut new_live_in = use_b.clone();
            new_live_in.extend(live_out_minus_def);
            
            let current_live_in = liveness_result.live_in.get(&block_id).unwrap(); // Should exist
            if *current_live_in != new_live_in {
                liveness_result.live_in.insert(block_id, new_live_in);
                changed = true;
            }
        }
    }

    liveness_result
}
