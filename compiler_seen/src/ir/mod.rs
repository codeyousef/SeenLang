// compiler_seen/src/ir/mod.rs
// Main module for the Intermediate Representation.

// This file will re-export key components of the IR for easier access
// from other parts of the compiler.

pub mod types;
pub mod instruction;
pub mod basic_block;
pub mod function;
pub mod module;

// Example re-exports (to be filled in as structs are defined):
// pub use types::IrType;
// pub use instruction::{Instruction, OpCode, Operand};
// pub use basic_block::BasicBlock;
// pub use function::Function;
// pub use module::Module;

// --- Bilingual Note Placeholder ---
// While the compiler's internal code (like this IR implementation) will be in Rust (and later Seen),
// the IR itself must be capable of representing concepts from Seen code written in either English or Arabic.
// For example, debug information attached to IR entities (like function names or variable names)
// should be able to store the original bilingual names from the source code.
// The IR's operational identifiers (e.g., register names, block labels) will typically be canonical and language-neutral.
