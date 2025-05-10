// compiler_seen/src/optimizations/mod.rs
// Main module for optimization passes.

// This module will contain various optimization passes that operate on the IR.

pub mod constant_folding;
pub mod dead_code_elimination;
// pub mod inlining;
// pub mod loop_optimizations;

// Potentially a trait for all optimization passes
// pub trait OptimizationPass {
//     fn run(&self, module: &mut crate::ir::module::Module);
// }
