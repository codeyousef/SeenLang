// compiler_seen/src/ir/instruction.rs
// Defines IR instructions, opcodes, and operands.

use super::types::IrType;

/// Represents an operand in an IR instruction.
#[derive(Debug, Clone, PartialEq)]
pub enum Operand {
    Constant(ConstantValue),
    Register(VirtualRegister), // Represents a value held in a virtual register (SSA form)
    // GlobalSymbol(String),    // For global variables/functions, might be needed directly
    Label(BasicBlockId), // For branch targets
}

/// Represents a constant value.
#[derive(Debug, Clone, PartialEq)]
pub enum ConstantValue {
    Bool(bool),
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    I128(i128),
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    U128(u128),
    F32(f32), // Note: f32/f64 PartialEq is tricky due to NaN. Consider wrapping in a struct that handles this.
    F64(f64),
    Char(char),
    StringLiteral(String), // For global string constants, actual storage might be elsewhere
    NullPtr(Box<IrType>),  // Null pointer of a specific pointer type
    Undef(Box<IrType>),    // Undefined value of a specific type
}

/// Uniquely identifies a virtual register.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VirtualRegister {
    pub id: u32, // Simple unique ID for now
                 // pub ty: IrType, // Type might be stored with the register definition or here
}

/// Uniquely identifies a basic block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BasicBlockId(pub u32);

/// Uniquely identifies a function.
#[derive(Debug, Clone, PartialEq, Eq, Hash)] // Name might be String
pub struct FunctionId(pub String);

/// Represents an IR instruction.
#[derive(Debug, Clone)]
pub struct Instruction {
    pub opcode: OpCode,
    pub operands: Vec<Operand>,
    pub result: Option<VirtualRegister>, // Register where the result is stored (if any)
    pub result_type: Option<IrType>,     // Type of the result
    pub parent_block: Option<BasicBlockId>, // ID of the basic block this instruction belongs to
    pub source_span: Option<SourceSpan>, // For debug info
    pub predicate: Option<ComparisonPredicate>, // For Icmp/Fcmp instructions
}

impl Instruction {
    /// Returns the virtual register defined by this instruction, if any.
    pub fn get_defined_register(&self) -> Option<VirtualRegister> {
        self.result
    }

    /// Returns a list of virtual registers used (read) by this instruction.
    pub fn get_used_registers(&self) -> Vec<VirtualRegister> {
        let mut used_registers = Vec::new();
        for operand in &self.operands {
            if let Operand::Register(reg) = operand {
                used_registers.push(*reg);
            }
        }
        // Special handling for opcodes if necessary, e.g. Call instructions
        // where the callee itself might be a register if it's a function pointer.
        // For Phi nodes, operands are [val1, label1, val2, label2, ...].
        // The current loop correctly picks up val1, val2, etc.
        used_registers
    }
}

/// Defines the operation to be performed by an instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpCode {
    // Binary Arithmetic (result, op1, op2)
    Add,
    Sub,
    Mul,
    Div,
    SDiv,
    UDiv,
    Rem,
    SRem,
    URem,
    // Bitwise (result, op1, op2)
    And,
    Or,
    Xor,
    Shl,
    LShr,
    AShr, // Logical Shift Right, Arithmetic Shift Right
    // Unary (result, op1)
    Neg,
    Not,

    // Memory (address is usually an operand; value for store is an operand)
    Alloc,         // result_ptr = alloc <type>, <size_bytes_operand>
    Load,          // result_val = load <type>, <ptr_operand>
    Store,         // store <type>, <val_operand>, <ptr_operand> (no result register)
    GetElementPtr, // result_ptr = getelementptr <type>, <base_ptr_op>, <idx_op1>, ...

    // Terminator Instructions (operands vary)
    Br,     // br <label_target_block_operand>
    BrCond, // br_cond <cond_operand>, <label_true_block_op>, <label_false_block_op>
    Return, // return [val_operand_opt]
    Unreachable,

    // Conversion (result, op1)
    CastIntTrunc,
    CastIntSignExt,
    CastIntZeroExt,
    CastIntToFloat,
    CastFloatToInt,
    CastFloatTrunc,
    CastFloatExt,
    CastPtrToInt,
    CastIntToPtr,
    CastBitcast,

    // Call (result_opt, func_name_or_ptr_op, arg_op1, arg_op2, ...)
    Call,

    // SSA Specific
    Phi, // result, [val1, label_pred1_op], [val2, label_pred2_op], ...

    // Comparison (result_bool, op1, op2)
    Icmp, // Integer comparison (requires a predicate: Eq, Ne, Slt, Sgt, Sle, Sge, Ult, Ugt, Ule, Uge)
    Fcmp, // Float comparison (requires a predicate: OEq, ONe, Olt, Ogt, Ole, Oge, UEQ, UNE, etc.)

    // Move/Assign operation
    Mov, // result = mov <operand>
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegerComparePredicate {
    EQ,  // Equal
    NE,  // Not Equal
    SGT, // Signed Greater Than
    SGE, // Signed Greater Than or Equal
    SLT, // Signed Less Than
    SLE, // Signed Less Than or Equal
    UGT, // Unsigned Greater Than
    UGE, // Unsigned Greater Than or Equal
    ULT, // Unsigned Less Than
    ULE, // Unsigned Less Than or Equal
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloatComparePredicate {
    OEQ, // Ordered Equal
    OGT, // Ordered Greater Than
    OGE, // Ordered Greater Than or Equal
    OLT, // Ordered Less Than
    OLE, // Ordered Less Than or Equal
    ONE, // Ordered Not Equal
    ORD, // Ordered (neither operand is NaN)
    UNO, // Unordered (either operand is NaN)
    UEQ, // Unordered Equal
    UGT, // Unordered Greater Than
    UGE, // Unordered Greater Than or Equal
    ULT, // Unordered Less Than
    ULE, // Unordered Less Than or Equal
    UNE, // Unordered Not Equal
}

/// Wraps integer and float comparison predicates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComparisonPredicate {
    Int(IntegerComparePredicate),
    Float(FloatComparePredicate),
}

/// Represents a span in the source code (file, line, column) for debugging.
// This is a simplified version. A more robust version might reference a file ID
// and store byte offsets or more detailed line/column info.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourceSpan {
    // For now, let's assume line and column are enough for a basic version.
    // In a real system, you'd have file IDs, byte offsets, etc.
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
    // pub file_id: FileId, // Would require a FileId type and a way to manage source files
}

// TODO:
// - Instruction constructors for convenience.
// - Methods to get operands, result, type, etc.
// - Detailed operand structure for Phi nodes (value, predecessor block).
// - How to associate comparison predicates with Icmp/Fcmp opcodes (maybe OpCode::Icmp(pred), OpCode::Fcmp(pred)).
// - Representing global variable symbols as operands or through dedicated instructions.
// - SourceSpan for debugging.
