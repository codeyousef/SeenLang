//! IR instruction system for the Seen programming language

use std::fmt;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use crate::value::{IRValue, IRType};

/// A label for jumps and basic block identification
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Label(pub String);

impl Label {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }
}

impl fmt::Display for Label {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, ".{}", self.0)
    }
}

/// Binary operation types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BinaryOp {
    // Arithmetic
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
    
    // Comparison
    Equal,
    NotEqual,
    LessThan,
    LessEqual,
    GreaterThan,
    GreaterEqual,
    
    // Logical
    And,
    Or,
    
    // Bitwise
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
    LeftShift,
    RightShift,
}

impl fmt::Display for BinaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BinaryOp::Add => write!(f, "add"),
            BinaryOp::Subtract => write!(f, "sub"),
            BinaryOp::Multiply => write!(f, "mul"),
            BinaryOp::Divide => write!(f, "div"),
            BinaryOp::Modulo => write!(f, "mod"),
            BinaryOp::Equal => write!(f, "eq"),
            BinaryOp::NotEqual => write!(f, "ne"),
            BinaryOp::LessThan => write!(f, "lt"),
            BinaryOp::LessEqual => write!(f, "le"),
            BinaryOp::GreaterThan => write!(f, "gt"),
            BinaryOp::GreaterEqual => write!(f, "ge"),
            BinaryOp::And => write!(f, "and"),
            BinaryOp::Or => write!(f, "or"),
            BinaryOp::BitwiseAnd => write!(f, "band"),
            BinaryOp::BitwiseOr => write!(f, "bor"),
            BinaryOp::BitwiseXor => write!(f, "bxor"),
            BinaryOp::LeftShift => write!(f, "shl"),
            BinaryOp::RightShift => write!(f, "shr"),
        }
    }
}

/// Unary operation types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum UnaryOp {
    Negate,
    Not,
    BitwiseNot,
    Reference,   // Take address
    Dereference, // Dereference pointer
}

impl fmt::Display for UnaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UnaryOp::Negate => write!(f, "neg"),
            UnaryOp::Not => write!(f, "not"),
            UnaryOp::BitwiseNot => write!(f, "bnot"),
            UnaryOp::Reference => write!(f, "ref"),
            UnaryOp::Dereference => write!(f, "deref"),
        }
    }
}

/// IR Instructions
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Instruction {
    // Control flow
    Label(Label),
    Jump(Label),
    JumpIf { condition: IRValue, target: Label },
    JumpIfNot { condition: IRValue, target: Label },
    Call { target: IRValue, args: Vec<IRValue>, result: Option<IRValue> },
    Return(Option<IRValue>),
    
    // Data operations
    Load { source: IRValue, dest: IRValue },
    Store { value: IRValue, dest: IRValue },
    Move { source: IRValue, dest: IRValue },
    
    // Arithmetic and logic
    Binary { op: BinaryOp, left: IRValue, right: IRValue, result: IRValue },
    Unary { op: UnaryOp, operand: IRValue, result: IRValue },
    
    // Memory management
    Allocate { size: IRValue, result: IRValue },
    Deallocate { pointer: IRValue },
    
    // Array operations
    ArrayAccess { array: IRValue, index: IRValue, result: IRValue },
    ArraySet { array: IRValue, index: IRValue, value: IRValue },
    ArrayLength { array: IRValue, result: IRValue },
    
    // Struct operations
    FieldAccess { struct_val: IRValue, field: String, result: IRValue },
    FieldSet { struct_val: IRValue, field: String, value: IRValue },
    
    // Type operations
    Cast { value: IRValue, target_type: IRType, result: IRValue },
    TypeCheck { value: IRValue, target_type: IRType, result: IRValue },
    
    // String operations
    StringConcat { left: IRValue, right: IRValue, result: IRValue },
    StringLength { string: IRValue, result: IRValue },
    
    // Function operations
    PushFrame,
    PopFrame,
    
    // Debug and intrinsics
    Print(IRValue),
    Debug { message: String, value: Option<IRValue> },
    
    // No-op for optimization
    Nop,
}

impl fmt::Display for Instruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Instruction::Label(label) => write!(f, "{}:", label),
            Instruction::Jump(label) => write!(f, "  jmp {}", label),
            Instruction::JumpIf { condition, target } => {
                write!(f, "  jif {} {}", condition, target)
            },
            Instruction::JumpIfNot { condition, target } => {
                write!(f, "  jnot {} {}", condition, target)
            },
            Instruction::Call { target, args, result } => {
                write!(f, "  call {} (", target)?;
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", arg)?;
                }
                write!(f, ")")?;
                if let Some(res) = result {
                    write!(f, " -> {}", res)?;
                }
                Ok(())
            },
            Instruction::Return(value) => {
                if let Some(val) = value {
                    write!(f, "  ret {}", val)
                } else {
                    write!(f, "  ret")
                }
            },
            Instruction::Load { source, dest } => {
                write!(f, "  load {} -> {}", source, dest)
            },
            Instruction::Store { value, dest } => {
                write!(f, "  store {} -> {}", value, dest)
            },
            Instruction::Move { source, dest } => {
                write!(f, "  mov {} -> {}", source, dest)
            },
            Instruction::Binary { op, left, right, result } => {
                write!(f, "  {} {} {} -> {}", op, left, right, result)
            },
            Instruction::Unary { op, operand, result } => {
                write!(f, "  {} {} -> {}", op, operand, result)
            },
            Instruction::Allocate { size, result } => {
                write!(f, "  alloc {} -> {}", size, result)
            },
            Instruction::Deallocate { pointer } => {
                write!(f, "  free {}", pointer)
            },
            Instruction::ArrayAccess { array, index, result } => {
                write!(f, "  arr_get {}[{}] -> {}", array, index, result)
            },
            Instruction::ArraySet { array, index, value } => {
                write!(f, "  arr_set {}[{}] = {}", array, index, value)
            },
            Instruction::ArrayLength { array, result } => {
                write!(f, "  arr_len {} -> {}", array, result)
            },
            Instruction::FieldAccess { struct_val, field, result } => {
                write!(f, "  field_get {}.{} -> {}", struct_val, field, result)
            },
            Instruction::FieldSet { struct_val, field, value } => {
                write!(f, "  field_set {}.{} = {}", struct_val, field, value)
            },
            Instruction::Cast { value, target_type, result } => {
                write!(f, "  cast {} as {} -> {}", value, target_type, result)
            },
            Instruction::TypeCheck { value, target_type, result } => {
                write!(f, "  is {} {} -> {}", value, target_type, result)
            },
            Instruction::StringConcat { left, right, result } => {
                write!(f, "  str_concat {} {} -> {}", left, right, result)
            },
            Instruction::StringLength { string, result } => {
                write!(f, "  str_len {} -> {}", string, result)
            },
            Instruction::PushFrame => write!(f, "  push_frame"),
            Instruction::PopFrame => write!(f, "  pop_frame"),
            Instruction::Print(value) => write!(f, "  print {}", value),
            Instruction::Debug { message, value } => {
                if let Some(val) = value {
                    write!(f, "  debug \"{}\" {}", message, val)
                } else {
                    write!(f, "  debug \"{}\"", message)
                }
            },
            Instruction::Nop => write!(f, "  nop"),
        }
    }
}

/// A basic block contains a sequence of instructions with a single entry and exit
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BasicBlock {
    pub label: Label,
    pub instructions: Vec<Instruction>,
    pub terminator: Option<Instruction>, // Jump, Return, etc.
}

impl BasicBlock {
    pub fn new(label: Label) -> Self {
        Self {
            label,
            instructions: Vec::new(),
            terminator: None,
        }
    }
    
    pub fn add_instruction(&mut self, instruction: Instruction) {
        // Check if this is a terminator instruction
        match instruction {
            Instruction::Jump(_) | 
            Instruction::JumpIf { .. } | 
            Instruction::JumpIfNot { .. } | 
            Instruction::Return(_) => {
                self.terminator = Some(instruction);
            },
            _ => {
                self.instructions.push(instruction);
            }
        }
    }
    
    pub fn is_terminated(&self) -> bool {
        self.terminator.is_some()
    }
    
    /// Get all the labels this block can jump to
    pub fn successors(&self) -> Vec<&Label> {
        match &self.terminator {
            Some(Instruction::Jump(label)) => vec![label],
            Some(Instruction::JumpIf { target, .. }) => vec![target],
            Some(Instruction::JumpIfNot { target, .. }) => vec![target],
            _ => vec![],
        }
    }
    
    /// Check if this block has any side effects
    pub fn has_side_effects(&self) -> bool {
        self.instructions.iter().any(|inst| {
            matches!(inst,
                Instruction::Store { .. } |
                Instruction::Call { .. } |
                Instruction::Print(_) |
                Instruction::Allocate { .. } |
                Instruction::Deallocate { .. } |
                Instruction::ArraySet { .. } |
                Instruction::FieldSet { .. }
            )
        }) || matches!(&self.terminator, Some(Instruction::Call { .. }))
    }
}

impl fmt::Display for BasicBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{}:", self.label.0)?;
        for instruction in &self.instructions {
            writeln!(f, "{}", instruction)?;
        }
        if let Some(terminator) = &self.terminator {
            writeln!(f, "{}", terminator)?;
        }
        Ok(())
    }
}

/// Control flow graph representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlFlowGraph {
    pub blocks: HashMap<String, BasicBlock>,
    pub entry_block: Option<String>,
}

impl ControlFlowGraph {
    pub fn new() -> Self {
        Self {
            blocks: HashMap::new(),
            entry_block: None,
        }
    }
    
    pub fn add_block(&mut self, block: BasicBlock) {
        let label_name = block.label.0.clone();
        if self.entry_block.is_none() {
            self.entry_block = Some(label_name.clone());
        }
        self.blocks.insert(label_name, block);
    }
    
    pub fn get_block(&self, label: &str) -> Option<&BasicBlock> {
        self.blocks.get(label)
    }
    
    pub fn get_block_mut(&mut self, label: &str) -> Option<&mut BasicBlock> {
        self.blocks.get_mut(label)
    }
    
    /// Validate the CFG structure
    pub fn validate(&self) -> Result<(), String> {
        // Check that all jump targets exist
        for block in self.blocks.values() {
            for successor in block.successors() {
                if !self.blocks.contains_key(&successor.0) {
                    return Err(format!("Invalid jump to non-existent label: {}", successor.0));
                }
            }
        }
        
        // Check that entry block exists
        if let Some(entry) = &self.entry_block {
            if !self.blocks.contains_key(entry) {
                return Err(format!("Entry block {} does not exist", entry));
            }
        }
        
        Ok(())
    }
}

impl Default for ControlFlowGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ControlFlowGraph {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(entry) = &self.entry_block {
            writeln!(f, "; Entry: {}", entry)?;
        }
        
        // Display blocks in a consistent order
        let mut block_names: Vec<_> = self.blocks.keys().collect();
        block_names.sort();
        
        for name in block_names {
            if let Some(block) = self.blocks.get(name) {
                writeln!(f, "{}", block)?;
            }
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_block_creation() {
        let label = Label::new("start");
        let mut block = BasicBlock::new(label.clone());
        
        assert_eq!(block.label, label);
        assert!(block.instructions.is_empty());
        assert!(block.terminator.is_none());
        assert!(!block.is_terminated());
    }
    
    #[test]
    fn test_basic_block_termination() {
        let mut block = BasicBlock::new(Label::new("test"));
        
        block.add_instruction(Instruction::Move {
            source: IRValue::Integer(42),
            dest: IRValue::Register(1),
        });
        assert!(!block.is_terminated());
        
        block.add_instruction(Instruction::Return(Some(IRValue::Register(1))));
        assert!(block.is_terminated());
        
        assert_eq!(block.instructions.len(), 1);
        assert!(block.terminator.is_some());
    }
    
    #[test]
    fn test_control_flow_graph() {
        let mut cfg = ControlFlowGraph::new();
        
        let mut block1 = BasicBlock::new(Label::new("start"));
        block1.add_instruction(Instruction::Jump(Label::new("end")));
        
        let mut block2 = BasicBlock::new(Label::new("end"));
        block2.add_instruction(Instruction::Return(None));
        
        cfg.add_block(block1);
        cfg.add_block(block2);
        
        assert_eq!(cfg.entry_block, Some("start".to_string()));
        assert!(cfg.get_block("start").is_some());
        assert!(cfg.get_block("end").is_some());
        assert!(cfg.validate().is_ok());
    }
    
    #[test]
    fn test_binary_operations() {
        let add_op = BinaryOp::Add;
        assert_eq!(add_op.to_string(), "add");
        
        let inst = Instruction::Binary {
            op: BinaryOp::Multiply,
            left: IRValue::Register(1),
            right: IRValue::Integer(2),
            result: IRValue::Register(2),
        };
        
        let display = inst.to_string();
        assert!(display.contains("mul"));
        assert!(display.contains("%r1"));
        assert!(display.contains("2"));
        assert!(display.contains("%r2"));
    }
}