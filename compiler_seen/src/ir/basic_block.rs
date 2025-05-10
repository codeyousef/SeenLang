// compiler_seen/src/ir/basic_block.rs
// Defines Basic Blocks, which are sequences of non-branching instructions
// followed by a single terminator instruction.

use super::instruction::{Instruction, BasicBlockId};
// use super::function::FunctionId; // If needed for context

/// Represents a Basic Block in the Control Flow Graph (CFG).
#[derive(Debug, Clone)]
pub struct BasicBlock {
    pub id: BasicBlockId,         // Unique identifier for this block
    // pub parent_function: FunctionId, // ID of the function this block belongs to
    pub instructions: Vec<Instruction>, // Instructions in this block, terminator is last
    
    // For CFG construction - these would store BasicBlockIds
    pub predecessors: Vec<BasicBlockId>,
    pub successors: Vec<BasicBlockId>,

    // pub name: Option<String>, // Optional name for debugging (e.g., "entry", "loop.header")
}

impl BasicBlock {
    pub fn new(id: BasicBlockId /*, parent_function: FunctionId*/) -> Self {
        BasicBlock {
            id,
            // parent_function,
            instructions: Vec::new(),
            predecessors: Vec::new(),
            successors: Vec::new(),
            // name: None,
        }
    }

    pub fn add_instruction(&mut self, mut instr: Instruction) {
        // TODO: Ensure instr.parent_block is set to self.id
        // instr.parent_block = Some(self.id);
        self.instructions.push(instr);
    }

    /// Returns the terminator instruction if the block is well-formed.
    /// A well-formed basic block must end with a terminator instruction.
    pub fn get_terminator(&self) -> Option<&Instruction> {
        self.instructions.last().filter(|instr| {
            matches!(instr.opcode, 
                super::instruction::OpCode::Br |
                super::instruction::OpCode::BrCond |
                super::instruction::OpCode::Return |
                super::instruction::OpCode::Unreachable
            )
        })
    }
}

// TODO:
// - Methods for managing CFG links (predecessors/successors).
// - Ensure that only the last instruction is a terminator and there's only one.
// - Phi node handling might involve inspecting predecessor blocks.
