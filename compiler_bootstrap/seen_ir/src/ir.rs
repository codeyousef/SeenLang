//! Intermediate representation definitions

use serde::{Deserialize, Serialize};

/// IR instruction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Instruction {
    /// Load a value
    Load { dest: u32, src: u32 },
    /// Store a value
    Store { dest: u32, src: u32 },
    /// Function call
    Call { dest: Option<u32>, func: String, args: Vec<u32> },
    /// Return from function
    Return { value: Option<u32> },
    /// Placeholder
    Nop,
}

/// IR basic block
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BasicBlock {
    pub label: String,
    pub instructions: Vec<Instruction>,
}

/// IR function
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Function {
    pub name: String,
    pub params: Vec<String>,
    pub blocks: Vec<BasicBlock>,
}

/// IR module
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Module {
    pub name: String,
    pub functions: Vec<Function>,
}