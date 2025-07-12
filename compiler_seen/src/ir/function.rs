// compiler_seen/src/ir/function.rs
// Defines a Function in the IR, which contains Basic Blocks, arguments, etc.

use super::basic_block::{BasicBlock, BasicBlockId};
use super::instruction::VirtualRegister;
use super::types::IrType;
// use super::module::ModuleId; // If needed for context

/// Represents a Function in the IR.
#[derive(Debug, Clone)]
pub struct Function {
    pub name: String, // Function name (mangled for uniqueness if necessary)
    // pub parent_module: ModuleId, // ID of the module this function belongs to
    pub parameters: Vec<Parameter>,
    pub return_type: IrType,

    pub basic_blocks: Vec<BasicBlock>, // Owned by the function
    pub entry_block_id: Option<BasicBlockId>,

    pub local_vars: Vec<LocalVarAllocation>, // For stack-allocated variables not in SSA registers initially
    next_virtual_reg_id: u32,                // Counter for allocating new virtual registers
    next_basic_block_id: u32,                // Counter for allocating new basic block IDs
}

#[derive(Debug, Clone)]
pub struct Parameter {
    pub name: String, // Original parameter name (for debugging)
    pub ty: IrType,
    pub register: VirtualRegister, // Virtual register holding this parameter's value at entry
}

/// Represents a local variable allocated on the stack (e.g., via `alloca`).
/// These are variables whose address might be taken or that are not (yet) promoted to SSA registers.
#[derive(Debug, Clone)]
pub struct LocalVarAllocation {
    pub name: Option<String>, // Original variable name (for debugging)
    pub ty: IrType,           // Type of the allocated variable (the type pointed to by the alloca)
    pub alloca_result_reg: VirtualRegister, // The virtual register holding the pointer from the 'alloca' instruction
                                            // pub size: Option<u32>,      // Size in bytes, if known at compile time (might be dynamic for VLAs)
                                            // pub alignment: Option<u32>,
}

impl Function {
    pub fn new(name: String, return_type: IrType /*, parent_module: ModuleId*/) -> Self {
        Function {
            name,
            // parent_module,
            parameters: Vec::new(),
            return_type,
            basic_blocks: Vec::new(),
            entry_block_id: None,
            local_vars: Vec::new(),
            next_virtual_reg_id: 0,
            next_basic_block_id: 0,
        }
    }

    pub fn add_parameter(&mut self, name: String, ty: IrType) -> VirtualRegister {
        let reg = self.new_virtual_register();
        self.parameters.push(Parameter {
            name,
            ty,
            register: reg,
        });
        reg
    }

    pub fn new_virtual_register(&mut self) -> VirtualRegister {
        let id = self.next_virtual_reg_id;
        self.next_virtual_reg_id += 1;
        VirtualRegister { id }
    }

    pub fn new_basic_block(&mut self /*, name: Option<String>*/) -> BasicBlockId {
        let id_val = self.next_basic_block_id;
        self.next_basic_block_id += 1;
        let bb_id = BasicBlockId(id_val);
        let mut bb = BasicBlock::new(bb_id /*, self.id (FunctionId)*/);
        // if let Some(n) = name { bb.name = Some(n); }
        if self.entry_block_id.is_none() {
            self.entry_block_id = Some(bb_id);
        }
        self.basic_blocks.push(bb);
        bb_id
    }

    pub fn get_basic_block_mut(&mut self, id: BasicBlockId) -> Option<&mut BasicBlock> {
        self.basic_blocks.iter_mut().find(|bb| bb.id == id)
    }

    pub fn get_basic_block(&self, id: BasicBlockId) -> Option<&BasicBlock> {
        self.basic_blocks.iter().find(|bb| bb.id == id)
    }
}

// TODO:
// - CFG construction within a function (linking basic blocks).
// - SSA construction pass will operate on this structure.
// - Storing local variable allocations that are not (yet) SSA registers.
// - Function signature (mangled name, param types, return type) for calls.
// - Attributes (e.g., inline hints, linkage type like public/private).
