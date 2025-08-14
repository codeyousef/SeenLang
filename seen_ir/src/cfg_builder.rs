//! Control Flow Graph builder for converting linear instruction sequences into proper CFG
//! This fixes the issue where multiple jumps in sequence were being lost

use std::collections::HashMap;
use crate::instruction::{Instruction, BasicBlock, Label, ControlFlowGraph};

/// Build a proper CFG from a linear sequence of instructions
/// This correctly handles:
/// - Label instructions that start new blocks
/// - Multiple jumps in sequence (like JumpIf followed by Jump)
/// - Proper terminator instructions
pub fn build_cfg_from_instructions(instructions: Vec<Instruction>) -> ControlFlowGraph {
    let mut cfg = ControlFlowGraph::new();
    let mut current_block: Option<BasicBlock> = None;
    let mut block_count = 0;
    
    // Helper to finish the current block and add it to CFG
    let finish_current_block = |current_block: Option<BasicBlock>, cfg: &mut ControlFlowGraph| {
        if let Some(block) = current_block {
            cfg.add_block(block);
        }
    };
    
    for instruction in instructions {
        match instruction {
            // Labels start new blocks
            Instruction::Label(label) => {
                // Finish current block if it exists
                if let Some(mut block) = current_block.take() {
                    // If the previous block doesn't have a terminator, 
                    // add an implicit jump to this label
                    if !block.is_terminated() {
                        block.terminator = Some(Instruction::Jump(label.clone()));
                    }
                    cfg.add_block(block);
                }
                
                // Start new block with this label
                current_block = Some(BasicBlock::new(label));
            },
            
            // Control flow instructions that terminate blocks
            Instruction::Jump(_) | 
            Instruction::JumpIf { .. } | 
            Instruction::JumpIfNot { .. } | 
            Instruction::Return(_) => {
                // Ensure we have a current block
                if current_block.is_none() {
                    let label = Label::new(format!("block_{}", block_count));
                    block_count += 1;
                    current_block = Some(BasicBlock::new(label));
                }
                
                // Check if this block already has a terminator
                if let Some(ref mut block) = current_block {
                    if block.is_terminated() {
                        // This block already has a terminator, so we need to start a new block
                        // This happens when we have multiple jumps in sequence (e.g., JumpIf followed by Jump)
                        cfg.add_block(current_block.take().unwrap());
                        
                        // Create a new block for this instruction
                        let label = Label::new(format!("block_{}", block_count));
                        block_count += 1;
                        let mut new_block = BasicBlock::new(label);
                        new_block.terminator = Some(instruction);
                        cfg.add_block(new_block);
                    } else {
                        // Set as terminator
                        block.terminator = Some(instruction);
                        // Finish this block
                        cfg.add_block(current_block.take().unwrap());
                    }
                } else {
                    // This shouldn't happen but handle it gracefully
                    let label = Label::new(format!("block_{}", block_count));
                    block_count += 1;
                    let mut new_block = BasicBlock::new(label);
                    new_block.terminator = Some(instruction);
                    cfg.add_block(new_block);
                }
            },
            
            // Regular instructions
            _ => {
                // Ensure we have a current block
                if current_block.is_none() {
                    let label = Label::new(format!("block_{}", block_count));
                    block_count += 1;
                    current_block = Some(BasicBlock::new(label));
                }
                
                // Add instruction to current block
                if let Some(ref mut block) = current_block {
                    block.instructions.push(instruction);
                }
            }
        }
    }
    
    // Finish any remaining block
    if let Some(block) = current_block {
        cfg.add_block(block);
    }
    
    // If no entry block was set, use the first block
    if cfg.entry_block.is_none() && !cfg.blocks.is_empty() {
        // Find the first block (either "entry" or "block_0")
        if cfg.blocks.contains_key("entry") {
            cfg.entry_block = Some("entry".to_string());
        } else if cfg.blocks.contains_key("block_0") {
            cfg.entry_block = Some("block_0".to_string());
        } else {
            // Use the first block in sorted order
            let mut keys: Vec<_> = cfg.blocks.keys().cloned().collect();
            keys.sort();
            if let Some(first) = keys.first() {
                cfg.entry_block = Some(first.clone());
            }
        }
    }
    
    cfg
}

/// Split a CFG block that has a jump in the middle of its instructions
/// This is needed when we discover jump instructions that weren't properly
/// handled as terminators
pub fn split_block_at_jump(cfg: &mut ControlFlowGraph, block_label: &str) -> bool {
    if let Some(block) = cfg.blocks.remove(block_label) {
        let mut new_blocks = Vec::new();
        let mut current_instructions = Vec::new();
        let original_label = block.label.clone();
        let mut block_counter = 0;
        
        for instruction in block.instructions {
            match instruction {
                Instruction::Jump(_) | 
                Instruction::JumpIf { .. } | 
                Instruction::JumpIfNot { .. } => {
                    // This is a terminator - finish current block
                    let label = if block_counter == 0 {
                        original_label.clone()
                    } else {
                        Label::new(format!("{}_{}", original_label.0, block_counter))
                    };
                    
                    let mut new_block = BasicBlock::new(label);
                    new_block.instructions = current_instructions.clone();
                    new_block.terminator = Some(instruction);
                    new_blocks.push(new_block);
                    
                    current_instructions.clear();
                    block_counter += 1;
                },
                _ => {
                    current_instructions.push(instruction);
                }
            }
        }
        
        // Handle remaining instructions and original terminator
        if !current_instructions.is_empty() || block.terminator.is_some() {
            let label = if block_counter == 0 {
                original_label
            } else {
                Label::new(format!("{}_{}", original_label.0, block_counter))
            };
            
            let mut new_block = BasicBlock::new(label);
            new_block.instructions = current_instructions;
            new_block.terminator = block.terminator;
            new_blocks.push(new_block);
        }
        
        // Add all new blocks back to CFG
        for new_block in new_blocks {
            let label_name = new_block.label.0.clone();
            cfg.blocks.insert(label_name, new_block);
        }
        
        return true;
    }
    
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::IRValue;
    use crate::instruction::BinaryOp;
    
    #[test]
    fn test_cfg_with_multiple_jumps() {
        // Test the case where we have JumpIf followed by Jump
        let instructions = vec![
            Instruction::Label(Label::new("loop_start")),
            Instruction::Binary {
                op: BinaryOp::LessThan,
                left: IRValue::Variable("i".to_string()),
                right: IRValue::Integer(10),
                result: IRValue::Register(0),
            },
            Instruction::JumpIf {
                condition: IRValue::Register(0),
                target: Label::new("loop_body"),
            },
            Instruction::Jump(Label::new("loop_end")),
            Instruction::Label(Label::new("loop_body")),
            Instruction::Binary {
                op: BinaryOp::Add,
                left: IRValue::Variable("i".to_string()),
                right: IRValue::Integer(1),
                result: IRValue::Register(1),
            },
            Instruction::Jump(Label::new("loop_start")),
            Instruction::Label(Label::new("loop_end")),
            Instruction::Return(Some(IRValue::Variable("i".to_string()))),
        ];
        
        let cfg = build_cfg_from_instructions(instructions);
        
        // Verify the CFG structure
        assert_eq!(cfg.blocks.len(), 4); // loop_start, block after jumps, loop_body, loop_end
        
        // Check loop_start block
        let loop_start = cfg.get_block("loop_start").unwrap();
        assert_eq!(loop_start.instructions.len(), 1); // Binary op
        assert!(matches!(loop_start.terminator, Some(Instruction::JumpIf { .. })));
        
        // Check that we have a block for the Jump after JumpIf
        assert!(cfg.blocks.keys().any(|k| k != "loop_start" && k != "loop_body" && k != "loop_end"));
        
        // Check loop_body block
        let loop_body = cfg.get_block("loop_body").unwrap();
        assert_eq!(loop_body.instructions.len(), 1); // Binary op
        assert!(matches!(loop_body.terminator, Some(Instruction::Jump(_))));
        
        // Check loop_end block
        let loop_end = cfg.get_block("loop_end").unwrap();
        assert_eq!(loop_end.instructions.len(), 0);
        assert!(matches!(loop_end.terminator, Some(Instruction::Return(_))));
    }
}