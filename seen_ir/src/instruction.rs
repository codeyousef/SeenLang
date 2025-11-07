//! IR instruction system for the Seen programming language

use crate::value::{IRType, IRValue};
use seen_parser::Expression as AstExpression;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

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
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ScopeKind {
    Task,
    Jobs,
}

/// IR representation of a channel select arm. Currently only stores the evaluated channel handle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IRSelectArm {
    pub channel: IRValue,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Instruction {
    // Control flow
    Label(Label),
    Jump(Label),
    JumpIf {
        condition: IRValue,
        target: Label,
    },
    JumpIfNot {
        condition: IRValue,
        target: Label,
    },
    Call {
        target: IRValue,
        args: Vec<IRValue>,
        result: Option<IRValue>,
    },
    Return(Option<IRValue>),

    // Data operations
    Load {
        source: IRValue,
        dest: IRValue,
    },
    Store {
        value: IRValue,
        dest: IRValue,
    },
    Move {
        source: IRValue,
        dest: IRValue,
    },

    // Arithmetic and logic
    Binary {
        op: BinaryOp,
        left: IRValue,
        right: IRValue,
        result: IRValue,
    },
    Unary {
        op: UnaryOp,
        operand: IRValue,
        result: IRValue,
    },

    // Memory management
    Allocate {
        size: IRValue,
        result: IRValue,
    },
    Deallocate {
        pointer: IRValue,
    },

    // Array operations
    ArrayAccess {
        array: IRValue,
        index: IRValue,
        result: IRValue,
    },
    ArraySet {
        array: IRValue,
        index: IRValue,
        value: IRValue,
    },
    ArrayLength {
        array: IRValue,
        result: IRValue,
    },

    // Struct operations
    FieldAccess {
        struct_val: IRValue,
        field: String,
        result: IRValue,
    },
    FieldSet {
        struct_val: IRValue,
        field: String,
        value: IRValue,
    },

    // Enum operations
    GetEnumTag {
        enum_value: IRValue,
        result: IRValue,
    },
    GetEnumField {
        enum_value: IRValue,
        field_index: u32,
        result: IRValue,
    },

    // Type operations
    Cast {
        value: IRValue,
        target_type: IRType,
        result: IRValue,
    },
    TypeCheck {
        value: IRValue,
        target_type: IRType,
        result: IRValue,
    },

    // String operations
    StringConcat {
        left: IRValue,
        right: IRValue,
        result: IRValue,
    },
    StringLength {
        string: IRValue,
        result: IRValue,
    },

    // Function operations
    PushFrame,
    PopFrame,

    // Method dispatch operations
    VirtualCall {
        receiver: IRValue,
        method_name: String,
        args: Vec<IRValue>,
        result: Option<IRValue>,
    },
    StaticCall {
        class_name: String,
        method_name: String,
        args: Vec<IRValue>,
        result: Option<IRValue>,
    },

    // Object-oriented operations
    ConstructObject {
        class_name: String,
        args: Vec<IRValue>,
        result: IRValue,
    },
    ConstructEnum {
        enum_name: String,
        variant_name: String,
        fields: Vec<IRValue>,
        result: IRValue,
    },

    // Debug and intrinsics
    Print(IRValue),
    Debug {
        message: String,
        value: Option<IRValue>,
    },

    // Concurrency primitives
    Scoped {
        kind: ScopeKind,
        body: Box<AstExpression>,
        result: IRValue,
    },
    Spawn {
        body: Box<AstExpression>,
        detached: bool,
        result: IRValue,
    },
    ChannelSelect {
        cases: Vec<IRSelectArm>,
        payload_result: IRValue,
        index_result: IRValue,
        status_result: IRValue,
    },

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
            }
            Instruction::JumpIfNot { condition, target } => {
                write!(f, "  jnot {} {}", condition, target)
            }
            Instruction::Call {
                target,
                args,
                result,
            } => {
                write!(f, "  call {} (", target)?;
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", arg)?;
                }
                write!(f, ")")?;
                if let Some(res) = result {
                    write!(f, " -> {}", res)?;
                }
                Ok(())
            }
            Instruction::Return(value) => {
                if let Some(val) = value {
                    write!(f, "  ret {}", val)
                } else {
                    write!(f, "  ret")
                }
            }
            Instruction::Load { source, dest } => {
                write!(f, "  load {} -> {}", source, dest)
            }
            Instruction::Store { value, dest } => {
                write!(f, "  store {} -> {}", value, dest)
            }
            Instruction::Move { source, dest } => {
                write!(f, "  mov {} -> {}", source, dest)
            }
            Instruction::Binary {
                op,
                left,
                right,
                result,
            } => {
                write!(f, "  {} {} {} -> {}", op, left, right, result)
            }
            Instruction::Unary {
                op,
                operand,
                result,
            } => {
                write!(f, "  {} {} -> {}", op, operand, result)
            }
            Instruction::Allocate { size, result } => {
                write!(f, "  alloc {} -> {}", size, result)
            }
            Instruction::Deallocate { pointer } => {
                write!(f, "  free {}", pointer)
            }
            Instruction::ArrayAccess {
                array,
                index,
                result,
            } => {
                write!(f, "  arr_get {}[{}] -> {}", array, index, result)
            }
            Instruction::ArraySet {
                array,
                index,
                value,
            } => {
                write!(f, "  arr_set {}[{}] = {}", array, index, value)
            }
            Instruction::ArrayLength { array, result } => {
                write!(f, "  arr_len {} -> {}", array, result)
            }
            Instruction::FieldAccess {
                struct_val,
                field,
                result,
            } => {
                write!(f, "  field_get {}.{} -> {}", struct_val, field, result)
            }
            Instruction::FieldSet {
                struct_val,
                field,
                value,
            } => {
                write!(f, "  field_set {}.{} = {}", struct_val, field, value)
            }
            Instruction::GetEnumTag { enum_value, result } => {
                write!(f, "  enum_tag {} -> {}", enum_value, result)
            }
            Instruction::GetEnumField {
                enum_value,
                field_index,
                result,
            } => {
                write!(
                    f,
                    "  enum_field {}[{}] -> {}",
                    enum_value, field_index, result
                )
            }
            Instruction::Cast {
                value,
                target_type,
                result,
            } => {
                write!(f, "  cast {} as {} -> {}", value, target_type, result)
            }
            Instruction::TypeCheck {
                value,
                target_type,
                result,
            } => {
                write!(f, "  is {} {} -> {}", value, target_type, result)
            }
            Instruction::StringConcat {
                left,
                right,
                result,
            } => {
                write!(f, "  str_concat {} {} -> {}", left, right, result)
            }
            Instruction::StringLength { string, result } => {
                write!(f, "  str_len {} -> {}", string, result)
            }
            Instruction::PushFrame => write!(f, "  push_frame"),
            Instruction::PopFrame => write!(f, "  pop_frame"),
            Instruction::VirtualCall {
                receiver,
                method_name,
                args,
                result,
            } => {
                write!(f, "  vcall {}.{}(", receiver, method_name)?;
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", arg)?;
                }
                write!(f, ")")?;
                if let Some(res) = result {
                    write!(f, " -> {}", res)?;
                }
                Ok(())
            }
            Instruction::StaticCall {
                class_name,
                method_name,
                args,
                result,
            } => {
                write!(f, "  scall {}::{} (", class_name, method_name)?;
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", arg)?;
                }
                write!(f, ")")?;
                if let Some(res) = result {
                    write!(f, " -> {}", res)?;
                }
                Ok(())
            }
            Instruction::ConstructObject {
                class_name,
                args,
                result,
            } => {
                write!(f, "  new {}(", class_name)?;
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", arg)?;
                }
                write!(f, ") -> {}", result)
            }
            Instruction::ConstructEnum {
                enum_name,
                variant_name,
                fields,
                result,
            } => {
                write!(f, "  enum {}::{}(", enum_name, variant_name)?;
                for (i, field) in fields.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", field)?;
                }
                write!(f, ") -> {}", result)
            }
            Instruction::Print(value) => write!(f, "  print {}", value),
            Instruction::Debug { message, value } => {
                if let Some(val) = value {
                    write!(f, "  debug \"{}\" {}", message, val)
                } else {
                    write!(f, "  debug \"{}\"", message)
                }
            }
            Instruction::Scoped { kind, result, .. } => {
                write!(f, "  scoped.{:?} -> {}", kind, result)
            }
            Instruction::Spawn {
                detached, result, ..
            } => {
                if *detached {
                    write!(f, "  spawn.detached -> {}", result)
                } else {
                    write!(f, "  spawn -> {}", result)
                }
            }
            Instruction::ChannelSelect {
                cases,
                payload_result,
                index_result,
                status_result,
            } => {
                write!(
                    f,
                    "  select[{} cases] -> payload={}, index={}, status={}",
                    cases.len(),
                    payload_result,
                    index_result,
                    status_result
                )
            }
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
            Instruction::Jump(_)
            | Instruction::JumpIf { .. }
            | Instruction::JumpIfNot { .. }
            | Instruction::Return(_) => {
                self.terminator = Some(instruction);
            }
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
            matches!(
                inst,
                Instruction::Store { .. }
                    | Instruction::Call { .. }
                    | Instruction::Print(_)
                    | Instruction::Allocate { .. }
                    | Instruction::Deallocate { .. }
                    | Instruction::ArraySet { .. }
                    | Instruction::FieldSet { .. }
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
#[derive(Debug, Clone)]
pub struct ControlFlowGraph {
    pub blocks: Vec<BasicBlock>,
    block_lookup: HashMap<String, u32>,
    pub entry_block: Option<String>,
    pub block_order: Vec<String>,
}

impl ControlFlowGraph {
    pub fn new() -> Self {
        Self {
            blocks: Vec::new(),
            block_lookup: HashMap::new(),
            entry_block: None,
            block_order: Vec::new(),
        }
    }

    pub fn add_block(&mut self, block: BasicBlock) {
        let label_name = block.label.0.clone();
        if let Some(index) = self.block_lookup.get(&label_name).cloned() {
            self.blocks[index as usize] = block;
        } else {
            let index = self.blocks.len() as u32;
            self.block_lookup.insert(label_name.clone(), index);
            self.blocks.push(block);
            self.block_order.push(label_name.clone());
            if self.entry_block.is_none() {
                self.entry_block = Some(label_name);
            }
        }
    }

    pub fn get_block(&self, label: &str) -> Option<&BasicBlock> {
        self.block_lookup
            .get(label)
            .and_then(|idx| self.blocks.get(*idx as usize))
    }

    pub fn get_block_mut(&mut self, label: &str) -> Option<&mut BasicBlock> {
        if let Some(idx) = self.block_lookup.get(label).cloned() {
            self.blocks.get_mut(idx as usize)
        } else {
            None
        }
    }

    pub fn contains_block(&self, label: &str) -> bool {
        self.block_lookup.contains_key(label)
    }

    pub fn block_index(&self, label: &str) -> Option<u32> {
        self.block_lookup.get(label).cloned()
    }

    pub fn replace_block(&mut self, old_label: &str, block: BasicBlock) -> Option<u32> {
        let index = self.block_lookup.get(old_label).cloned()?;
        let new_label = block.label.0.clone();
        self.blocks[index as usize] = block;

        if old_label != new_label {
            self.block_lookup.remove(old_label);
            self.block_lookup.insert(new_label.clone(), index);

            if let Some(entry) = self.entry_block.as_mut() {
                if entry == old_label {
                    *entry = new_label.clone();
                }
            }

            if let Some(pos) = self
                .block_order
                .iter_mut()
                .position(|label| label == old_label)
            {
                self.block_order[pos] = new_label;
            }
        }

        Some(index)
    }

    pub fn insert_block_at_order(&mut self, position: usize, block: BasicBlock) -> u32 {
        let label_name = block.label.0.clone();
        let index = self.blocks.len() as u32;
        self.blocks.push(block);
        self.block_lookup.insert(label_name.clone(), index);
        if position >= self.block_order.len() {
            self.block_order.push(label_name.clone());
        } else {
            self.block_order.insert(position, label_name.clone());
        }
        if self.entry_block.is_none() {
            self.entry_block = Some(label_name);
        }
        index
    }

    pub fn block_names(&self) -> impl Iterator<Item = &String> {
        self.block_order.iter()
    }

    pub fn blocks_iter(&self) -> impl Iterator<Item = &BasicBlock> {
        self.blocks.iter()
    }

    pub fn blocks_iter_mut(&mut self) -> impl Iterator<Item = &mut BasicBlock> {
        self.blocks.iter_mut()
    }

    pub fn rebuild_lookup(&mut self) {
        self.block_lookup.clear();
        for (idx, block) in self.blocks.iter().enumerate() {
            self.block_lookup.insert(block.label.0.clone(), idx as u32);
        }
    }

    /// Validate the CFG structure
    pub fn validate(&self) -> Result<(), String> {
        // Check that all jump targets exist
        for block in &self.blocks {
            for successor in block.successors() {
                if !self.block_lookup.contains_key(&successor.0) {
                    return Err(format!(
                        "Invalid jump to non-existent label: {}",
                        successor.0
                    ));
                }
            }
        }

        // Check that entry block exists
        if let Some(entry) = &self.entry_block {
            if !self.block_lookup.contains_key(entry) {
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
        for name in &self.block_order {
            if let Some(block) = self.get_block(name) {
                writeln!(f, "{}", block)?;
            }
        }

        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
struct ControlFlowGraphSerde {
    blocks: Vec<BasicBlock>,
    entry_block: Option<String>,
    block_order: Vec<String>,
}

impl Serialize for ControlFlowGraph {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        ControlFlowGraphSerde {
            blocks: self.blocks.clone(),
            entry_block: self.entry_block.clone(),
            block_order: self.block_order.clone(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ControlFlowGraph {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let data = ControlFlowGraphSerde::deserialize(deserializer)?;
        let mut cfg = ControlFlowGraph {
            blocks: data.blocks,
            block_lookup: HashMap::new(),
            entry_block: data.entry_block,
            block_order: data.block_order,
        };
        cfg.rebuild_lookup();
        Ok(cfg)
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
