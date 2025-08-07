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
    Call { dest: Option<u32>, func: String, args: Vec<Value> },
    /// Return from function
    Return { value: Option<Value> },
    /// Binary operation
    Binary { dest: u32, op: BinaryOp, left: Value, right: Value },
    /// Comparison
    Compare { dest: u32, op: CompareOp, left: Value, right: Value },
    /// Branch instruction
    Branch { condition: Option<u32>, true_label: String, false_label: Option<String> },
    /// Phi node for SSA form
    Phi { dest: u32, values: Vec<(Value, String)> },
    /// Allocate memory on stack
    Alloca { dest: u32, ty: Type },
    /// No operation (for optimization placeholders)
    Nop,
}

/// Value that can be used in instructions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Value {
    /// Register/variable reference
    Register(u32),
    /// Integer constant
    Integer(i64),
    /// Float constant
    Float(f64),
    /// Boolean constant
    Boolean(bool),
    /// String constant
    String(String),
}

/// Binary operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    And,
    Or,
    Xor,
}

/// Comparison operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompareOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

/// Type representation in IR
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Type {
    I32,
    I64,
    F32,
    F64,
    Bool,
    Ptr(Box<Type>),
    Void,
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