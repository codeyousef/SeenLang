//! IR optimization passes for the Seen programming language

use std::collections::{HashMap, HashSet};
use crate::{
    IRProgram, IRResult,
    instruction::{Instruction, BasicBlock, BinaryOp, UnaryOp},
    value::{IRValue, IRType},
    function::IRFunction,
    module::IRModule,
};

/// Optimization level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizationLevel {
    None,    // O0 - No optimizations
    Basic,   // O1 - Basic optimizations
    Standard, // O2 - Standard optimizations
    Aggressive, // O3 - Aggressive optimizations
}

impl OptimizationLevel {
    pub fn from_level(level: u8) -> Self {
        match level {
            0 => OptimizationLevel::None,
            1 => OptimizationLevel::Basic,
            2 => OptimizationLevel::Standard,
            3 | _ => OptimizationLevel::Aggressive,
        }
    }
    
    pub fn should_run_pass(&self, pass_level: OptimizationLevel) -> bool {
        match (self, pass_level) {
            (OptimizationLevel::None, _) => false,
            (OptimizationLevel::Basic, OptimizationLevel::Basic) => true,
            (OptimizationLevel::Standard, OptimizationLevel::Basic | OptimizationLevel::Standard) => true,
            (OptimizationLevel::Aggressive, _) => true,
            _ => false,
        }
    }
}

/// Statistics about optimization passes
#[derive(Debug, Clone, Default)]
pub struct OptimizationStats {
    pub instructions_eliminated: usize,
    pub constant_folding_operations: usize,
    pub dead_code_blocks_removed: usize,
    pub redundant_moves_eliminated: usize,
    pub passes_run: Vec<String>,
}

/// IR optimizer that applies various optimization passes
pub struct IROptimizer {
    level: OptimizationLevel,
    stats: OptimizationStats,
}

impl IROptimizer {
    pub fn new(level: OptimizationLevel) -> Self {
        Self {
            level,
            stats: OptimizationStats::default(),
        }
    }
    
    pub fn get_stats(&self) -> &OptimizationStats {
        &self.stats
    }
    
    /// Optimize an entire IR program
    pub fn optimize_program(&mut self, program: &mut IRProgram) -> IRResult<()> {
        for module in &mut program.modules {
            self.optimize_module(module)?;
        }
        Ok(())
    }
    
    /// Optimize a single module
    pub fn optimize_module(&mut self, module: &mut IRModule) -> IRResult<()> {
        for function in module.functions.values_mut() {
            self.optimize_function(function)?;
        }
        Ok(())
    }
    
    /// Optimize a single function
    pub fn optimize_function(&mut self, function: &mut IRFunction) -> IRResult<()> {
        // Run optimization passes in order
        
        if self.level.should_run_pass(OptimizationLevel::Basic) {
            self.constant_folding(function)?;
            self.stats.passes_run.push("constant_folding".to_string());
            
            self.eliminate_redundant_moves(function)?;
            self.stats.passes_run.push("eliminate_redundant_moves".to_string());
            
            self.eliminate_nops(function)?;
            self.stats.passes_run.push("eliminate_nops".to_string());
        }
        
        if self.level.should_run_pass(OptimizationLevel::Standard) {
            self.dead_code_elimination(function)?;
            self.stats.passes_run.push("dead_code_elimination".to_string());
            
            self.strength_reduction(function)?;
            self.stats.passes_run.push("strength_reduction".to_string());
        }
        
        if self.level.should_run_pass(OptimizationLevel::Aggressive) {
            self.common_subexpression_elimination(function)?;
            self.stats.passes_run.push("common_subexpression_elimination".to_string());
            
            self.loop_optimization(function)?;
            self.stats.passes_run.push("loop_optimization".to_string());
        }
        
        Ok(())
    }
    
    /// Constant folding optimization
    fn constant_folding(&mut self, function: &mut IRFunction) -> IRResult<()> {
        for block in function.cfg.blocks.values_mut() {
            let mut new_instructions = Vec::new();
            
            for instruction in &block.instructions {
                match instruction {
                    Instruction::Binary { op, left, right, result } => {
                        if let (IRValue::Integer(l), IRValue::Integer(r)) = (left, right) {
                            let folded_value = match op {
                                BinaryOp::Add => Some(IRValue::Integer(l + r)),
                                BinaryOp::Subtract => Some(IRValue::Integer(l - r)),
                                BinaryOp::Multiply => Some(IRValue::Integer(l * r)),
                                BinaryOp::Divide if *r != 0 => Some(IRValue::Integer(l / r)),
                                BinaryOp::Modulo if *r != 0 => Some(IRValue::Integer(l % r)),
                                BinaryOp::Equal => Some(IRValue::Boolean(l == r)),
                                BinaryOp::NotEqual => Some(IRValue::Boolean(l != r)),
                                BinaryOp::LessThan => Some(IRValue::Boolean(l < r)),
                                BinaryOp::LessEqual => Some(IRValue::Boolean(l <= r)),
                                BinaryOp::GreaterThan => Some(IRValue::Boolean(l > r)),
                                BinaryOp::GreaterEqual => Some(IRValue::Boolean(l >= r)),
                                _ => None,
                            };
                            
                            if let Some(value) = folded_value {
                                // Replace with a move instruction
                                new_instructions.push(Instruction::Move {
                                    source: value,
                                    dest: result.clone(),
                                });
                                self.stats.constant_folding_operations += 1;
                                continue;
                            }
                        }
                        
                        // Handle float constants
                        if let (IRValue::Float(l), IRValue::Float(r)) = (left, right) {
                            let folded_value = match op {
                                BinaryOp::Add => Some(IRValue::Float(l + r)),
                                BinaryOp::Subtract => Some(IRValue::Float(l - r)),
                                BinaryOp::Multiply => Some(IRValue::Float(l * r)),
                                BinaryOp::Divide if *r != 0.0 => Some(IRValue::Float(l / r)),
                                BinaryOp::Equal => Some(IRValue::Boolean((l - r).abs() < f64::EPSILON)),
                                BinaryOp::NotEqual => Some(IRValue::Boolean((l - r).abs() >= f64::EPSILON)),
                                BinaryOp::LessThan => Some(IRValue::Boolean(l < r)),
                                BinaryOp::LessEqual => Some(IRValue::Boolean(l <= r)),
                                BinaryOp::GreaterThan => Some(IRValue::Boolean(l > r)),
                                BinaryOp::GreaterEqual => Some(IRValue::Boolean(l >= r)),
                                _ => None,
                            };
                            
                            if let Some(value) = folded_value {
                                new_instructions.push(Instruction::Move {
                                    source: value,
                                    dest: result.clone(),
                                });
                                self.stats.constant_folding_operations += 1;
                                continue;
                            }
                        }
                        
                        new_instructions.push(instruction.clone());
                    },
                    
                    Instruction::Unary { op, operand, result } => {
                        match (op, operand) {
                            (UnaryOp::Negate, IRValue::Integer(i)) => {
                                new_instructions.push(Instruction::Move {
                                    source: IRValue::Integer(-i),
                                    dest: result.clone(),
                                });
                                self.stats.constant_folding_operations += 1;
                            },
                            (UnaryOp::Negate, IRValue::Float(f)) => {
                                new_instructions.push(Instruction::Move {
                                    source: IRValue::Float(-f),
                                    dest: result.clone(),
                                });
                                self.stats.constant_folding_operations += 1;
                            },
                            (UnaryOp::Not, IRValue::Boolean(b)) => {
                                new_instructions.push(Instruction::Move {
                                    source: IRValue::Boolean(!b),
                                    dest: result.clone(),
                                });
                                self.stats.constant_folding_operations += 1;
                            },
                            _ => new_instructions.push(instruction.clone()),
                        }
                    },
                    
                    _ => new_instructions.push(instruction.clone()),
                }
            }
            
            block.instructions = new_instructions;
        }
        
        Ok(())
    }
    
    /// Eliminate redundant move instructions
    fn eliminate_redundant_moves(&mut self, function: &mut IRFunction) -> IRResult<()> {
        for block in function.cfg.blocks.values_mut() {
            let mut new_instructions = Vec::new();
            
            for instruction in &block.instructions {
                match instruction {
                    Instruction::Move { source, dest } if source == dest => {
                        // Skip redundant moves (mov %r1 -> %r1)
                        self.stats.redundant_moves_eliminated += 1;
                        self.stats.instructions_eliminated += 1;
                    },
                    _ => new_instructions.push(instruction.clone()),
                }
            }
            
            block.instructions = new_instructions;
        }
        
        Ok(())
    }
    
    /// Eliminate no-op instructions
    fn eliminate_nops(&mut self, function: &mut IRFunction) -> IRResult<()> {
        for block in function.cfg.blocks.values_mut() {
            let original_len = block.instructions.len();
            block.instructions.retain(|inst| !matches!(inst, Instruction::Nop));
            
            let eliminated = original_len - block.instructions.len();
            self.stats.instructions_eliminated += eliminated;
        }
        
        Ok(())
    }
    
    /// Dead code elimination
    fn dead_code_elimination(&mut self, function: &mut IRFunction) -> IRResult<()> {
        // Find all used values
        let mut used_values = HashSet::new();
        
        // Mark values used in instructions
        for block in function.cfg.blocks.values() {
            for instruction in &block.instructions {
                self.mark_used_values(instruction, &mut used_values);
            }
            if let Some(terminator) = &block.terminator {
                self.mark_used_values(terminator, &mut used_values);
            }
        }
        
        // Remove instructions that produce unused values
        for block in function.cfg.blocks.values_mut() {
            let mut new_instructions = Vec::new();
            
            for instruction in &block.instructions {
                if self.instruction_produces_used_value(instruction, &used_values) {
                    new_instructions.push(instruction.clone());
                } else if self.has_side_effects(instruction) {
                    new_instructions.push(instruction.clone());
                } else {
                    self.stats.instructions_eliminated += 1;
                }
            }
            
            block.instructions = new_instructions;
        }
        
        Ok(())
    }
    
    /// Strength reduction (replace expensive operations with cheaper ones)
    fn strength_reduction(&mut self, function: &mut IRFunction) -> IRResult<()> {
        for block in function.cfg.blocks.values_mut() {
            let mut new_instructions = Vec::new();
            
            for instruction in &block.instructions {
                match instruction {
                    Instruction::Binary { op: BinaryOp::Multiply, left, right, result } => {
                        // Replace multiplication by power of 2 with left shift
                        if let IRValue::Integer(n) = right {
                            if *n > 0 && (*n & (*n - 1)) == 0 { // Check if power of 2
                                let shift_amount = (*n as u64).trailing_zeros();
                                new_instructions.push(Instruction::Binary {
                                    op: BinaryOp::LeftShift,
                                    left: left.clone(),
                                    right: IRValue::Integer(shift_amount as i64),
                                    result: result.clone(),
                                });
                                continue;
                            }
                        }
                        new_instructions.push(instruction.clone());
                    },
                    
                    Instruction::Binary { op: BinaryOp::Divide, left, right, result } => {
                        // Replace division by power of 2 with right shift
                        if let IRValue::Integer(n) = right {
                            if *n > 0 && (*n & (*n - 1)) == 0 { // Check if power of 2
                                let shift_amount = (*n as u64).trailing_zeros();
                                new_instructions.push(Instruction::Binary {
                                    op: BinaryOp::RightShift,
                                    left: left.clone(),
                                    right: IRValue::Integer(shift_amount as i64),
                                    result: result.clone(),
                                });
                                continue;
                            }
                        }
                        new_instructions.push(instruction.clone());
                    },
                    
                    _ => new_instructions.push(instruction.clone()),
                }
            }
            
            block.instructions = new_instructions;
        }
        
        Ok(())
    }
    
    /// Common subexpression elimination
    fn common_subexpression_elimination(&mut self, function: &mut IRFunction) -> IRResult<()> {
        // This is a simplified CSE that works within basic blocks
        for block in function.cfg.blocks.values_mut() {
            let mut expression_map: HashMap<String, IRValue> = HashMap::new();
            let mut new_instructions = Vec::new();
            
            for instruction in &block.instructions {
                match instruction {
                    Instruction::Binary { op, left, right, result } => {
                        let expr_key = format!("{}:{}:{}", op, left, right);
                        
                        if let Some(existing_result) = expression_map.get(&expr_key) {
                            // Replace with move from existing result
                            new_instructions.push(Instruction::Move {
                                source: existing_result.clone(),
                                dest: result.clone(),
                            });
                        } else {
                            new_instructions.push(instruction.clone());
                            expression_map.insert(expr_key, result.clone());
                        }
                    },
                    
                    Instruction::Unary { op, operand, result } => {
                        let expr_key = format!("{}:{}", op, operand);
                        
                        if let Some(existing_result) = expression_map.get(&expr_key) {
                            new_instructions.push(Instruction::Move {
                                source: existing_result.clone(),
                                dest: result.clone(),
                            });
                        } else {
                            new_instructions.push(instruction.clone());
                            expression_map.insert(expr_key, result.clone());
                        }
                    },
                    
                    _ => {
                        new_instructions.push(instruction.clone());
                        // Invalidate expressions that might be affected by this instruction
                        if self.has_side_effects(instruction) {
                            expression_map.clear();
                        }
                    }
                }
            }
            
            block.instructions = new_instructions;
        }
        
        Ok(())
    }
    
    /// Loop optimization (basic loop invariant code motion)
    fn loop_optimization(&mut self, _function: &mut IRFunction) -> IRResult<()> {
        // TODO: Implement loop optimization passes:
        // - Loop invariant code motion
        // - Loop unrolling
        // - Loop fusion
        
        // For now, this is a placeholder
        Ok(())
    }
    
    /// Helper: Mark values used by an instruction
    fn mark_used_values(&self, instruction: &Instruction, used_values: &mut HashSet<String>) {
        match instruction {
            Instruction::Binary { left, right, .. } => {
                self.mark_value_used(left, used_values);
                self.mark_value_used(right, used_values);
            },
            Instruction::Unary { operand, .. } => {
                self.mark_value_used(operand, used_values);
            },
            Instruction::Move { source, .. } => {
                self.mark_value_used(source, used_values);
            },
            Instruction::Store { value, dest } => {
                self.mark_value_used(value, used_values);
                self.mark_value_used(dest, used_values);
            },
            Instruction::Load { source, .. } => {
                self.mark_value_used(source, used_values);
            },
            Instruction::Call { target, args, .. } => {
                self.mark_value_used(target, used_values);
                for arg in args {
                    self.mark_value_used(arg, used_values);
                }
            },
            Instruction::Return(Some(value)) => {
                self.mark_value_used(value, used_values);
            },
            Instruction::JumpIf { condition, .. } => {
                self.mark_value_used(condition, used_values);
            },
            Instruction::JumpIfNot { condition, .. } => {
                self.mark_value_used(condition, used_values);
            },
            Instruction::Print(value) => {
                self.mark_value_used(value, used_values);
            },
            // Add other instruction types as needed
            _ => {},
        }
    }
    
    /// Helper: Mark a specific value as used
    fn mark_value_used(&self, value: &IRValue, used_values: &mut HashSet<String>) {
        match value {
            IRValue::Variable(name) => { used_values.insert(name.clone()); },
            IRValue::Register(reg) => { used_values.insert(format!("r{}", reg)); },
            IRValue::GlobalVariable(name) => { used_values.insert(name.clone()); },
            _ => {},
        }
    }
    
    /// Helper: Check if instruction produces a used value
    fn instruction_produces_used_value(&self, instruction: &Instruction, used_values: &HashSet<String>) -> bool {
        match instruction {
            Instruction::Binary { result, .. } |
            Instruction::Unary { result, .. } |
            Instruction::Load { dest: result, .. } |
            Instruction::ArrayAccess { result, .. } |
            Instruction::FieldAccess { result, .. } => {
                self.value_is_used(result, used_values)
            },
            Instruction::Call { result: Some(result), .. } => {
                self.value_is_used(result, used_values)
            },
            _ => true, // Conservative: assume other instructions are needed
        }
    }
    
    /// Helper: Check if a value is used
    fn value_is_used(&self, value: &IRValue, used_values: &HashSet<String>) -> bool {
        match value {
            IRValue::Variable(name) => used_values.contains(name),
            IRValue::Register(reg) => used_values.contains(&format!("r{}", reg)),
            IRValue::GlobalVariable(name) => used_values.contains(name),
            _ => true,
        }
    }
    
    /// Helper: Check if instruction has side effects
    fn has_side_effects(&self, instruction: &Instruction) -> bool {
        matches!(instruction,
            Instruction::Store { .. } |
            Instruction::Call { .. } |
            Instruction::Print(_) |
            Instruction::Allocate { .. } |
            Instruction::Deallocate { .. } |
            Instruction::ArraySet { .. } |
            Instruction::FieldSet { .. } |
            Instruction::Return(_) |
            Instruction::Jump(_) |
            Instruction::JumpIf { .. } |
            Instruction::JumpIfNot { .. }
        )
    }
}

impl Default for IROptimizer {
    fn default() -> Self {
        Self::new(OptimizationLevel::Standard)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        instruction::{BasicBlock, Label},
        value::IRValue,
    };

    #[test]
    fn test_constant_folding() {
        let mut optimizer = IROptimizer::new(OptimizationLevel::Basic);
        let mut function = IRFunction::new("test", IRType::Integer);
        
        let mut block = BasicBlock::new(Label::new("entry"));
        block.add_instruction(Instruction::Binary {
            op: BinaryOp::Add,
            left: IRValue::Integer(5),
            right: IRValue::Integer(3),
            result: IRValue::Register(1),
        });
        
        function.add_block(block);
        
        optimizer.optimize_function(&mut function).unwrap();
        
        let block = function.get_block("entry").unwrap();
        assert_eq!(block.instructions.len(), 1);
        
        if let Instruction::Move { source, dest } = &block.instructions[0] {
            assert_eq!(*source, IRValue::Integer(8));
            assert_eq!(*dest, IRValue::Register(1));
        } else {
            panic!("Expected move instruction after constant folding");
        }
    }
    
    #[test]
    fn test_redundant_move_elimination() {
        let mut optimizer = IROptimizer::new(OptimizationLevel::Basic);
        let mut function = IRFunction::new("test", IRType::Integer);
        
        let mut block = BasicBlock::new(Label::new("entry"));
        block.add_instruction(Instruction::Move {
            source: IRValue::Register(1),
            dest: IRValue::Register(1),
        });
        
        function.add_block(block);
        
        let original_count = function.get_block("entry").unwrap().instructions.len();
        optimizer.optimize_function(&mut function).unwrap();
        
        let block = function.get_block("entry").unwrap();
        assert_eq!(block.instructions.len(), original_count - 1);
        assert_eq!(optimizer.stats.redundant_moves_eliminated, 1);
    }
    
    #[test]
    fn test_strength_reduction() {
        let mut optimizer = IROptimizer::new(OptimizationLevel::Standard);
        let mut function = IRFunction::new("test", IRType::Integer);
        
        let mut block = BasicBlock::new(Label::new("entry"));
        // Multiply by 8 (power of 2)
        block.add_instruction(Instruction::Binary {
            op: BinaryOp::Multiply,
            left: IRValue::Register(1),
            right: IRValue::Integer(8),
            result: IRValue::Register(2),
        });
        
        function.add_block(block);
        
        optimizer.optimize_function(&mut function).unwrap();
        
        let block = function.get_block("entry").unwrap();
        if let Instruction::Binary { op, right, .. } = &block.instructions[0] {
            assert_eq!(*op, BinaryOp::LeftShift);
            assert_eq!(*right, IRValue::Integer(3)); // log2(8) = 3
        } else {
            panic!("Expected left shift instruction after strength reduction");
        }
    }
}