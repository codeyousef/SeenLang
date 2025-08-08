//! Intermediate representation definitions

use serde::{Deserialize, Serialize};
use std::fmt;

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
    /// RISC-V specific instructions
    RiscV(RiscVInstruction),
}

/// RISC-V specific IR instructions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RiscVInstruction {
    /// RV32I/RV64I Base Integer Instructions
    
    // Arithmetic instructions
    Add { dest: u32, src1: Value, src2: Value },
    Sub { dest: u32, src1: Value, src2: Value },
    Slt { dest: u32, src1: Value, src2: Value }, // Set less than
    Sltu { dest: u32, src1: Value, src2: Value }, // Set less than unsigned
    
    // Immediate arithmetic
    Addi { dest: u32, src: Value, imm: i32 },
    Slti { dest: u32, src: Value, imm: i32 }, // Set less than immediate
    Sltiu { dest: u32, src: Value, imm: i32 }, // Set less than immediate unsigned
    
    // Logical instructions
    And { dest: u32, src1: Value, src2: Value },
    Or { dest: u32, src1: Value, src2: Value },
    Xor { dest: u32, src1: Value, src2: Value },
    
    // Immediate logical
    Andi { dest: u32, src: Value, imm: i32 },
    Ori { dest: u32, src: Value, imm: i32 },
    Xori { dest: u32, src: Value, imm: i32 },
    
    // Shift instructions
    Sll { dest: u32, src1: Value, src2: Value }, // Shift left logical
    Srl { dest: u32, src1: Value, src2: Value }, // Shift right logical
    Sra { dest: u32, src1: Value, src2: Value }, // Shift right arithmetic
    
    // Immediate shift
    Slli { dest: u32, src: Value, shamt: u32 }, // Shift left logical immediate
    Srli { dest: u32, src: Value, shamt: u32 }, // Shift right logical immediate
    Srai { dest: u32, src: Value, shamt: u32 }, // Shift right arithmetic immediate
    
    // Upper immediate
    Lui { dest: u32, imm: u32 }, // Load upper immediate
    Auipc { dest: u32, imm: u32 }, // Add upper immediate to PC
    
    // Memory access
    Lw { dest: u32, base: Value, offset: i32 }, // Load word
    Sw { src: Value, base: Value, offset: i32 }, // Store word
    Lb { dest: u32, base: Value, offset: i32 }, // Load byte
    Sb { src: Value, base: Value, offset: i32 }, // Store byte
    Lh { dest: u32, base: Value, offset: i32 }, // Load halfword
    Sh { src: Value, base: Value, offset: i32 }, // Store halfword
    Lbu { dest: u32, base: Value, offset: i32 }, // Load byte unsigned
    Lhu { dest: u32, base: Value, offset: i32 }, // Load halfword unsigned
    
    // RV64I specific (64-bit operations)
    Addw { dest: u32, src1: Value, src2: Value }, // Add word (32-bit result in 64-bit)
    Subw { dest: u32, src1: Value, src2: Value }, // Subtract word
    Addiw { dest: u32, src: Value, imm: i32 }, // Add immediate word
    Sllw { dest: u32, src1: Value, src2: Value }, // Shift left logical word
    Srlw { dest: u32, src1: Value, src2: Value }, // Shift right logical word
    Sraw { dest: u32, src1: Value, src2: Value }, // Shift right arithmetic word
    Slliw { dest: u32, src: Value, shamt: u32 }, // Shift left logical immediate word
    Srliw { dest: u32, src: Value, shamt: u32 }, // Shift right logical immediate word
    Sraiw { dest: u32, src: Value, shamt: u32 }, // Shift right arithmetic immediate word
    Ld { dest: u32, base: Value, offset: i32 }, // Load doubleword
    Sd { src: Value, base: Value, offset: i32 }, // Store doubleword
    Lwu { dest: u32, base: Value, offset: i32 }, // Load word unsigned
    
    // Control flow
    Beq { src1: Value, src2: Value, label: String }, // Branch if equal
    Bne { src1: Value, src2: Value, label: String }, // Branch if not equal
    Blt { src1: Value, src2: Value, label: String }, // Branch if less than
    Bge { src1: Value, src2: Value, label: String }, // Branch if greater or equal
    Bltu { src1: Value, src2: Value, label: String }, // Branch if less than unsigned
    Bgeu { src1: Value, src2: Value, label: String }, // Branch if greater or equal unsigned
    Jal { dest: Option<u32>, label: String }, // Jump and link
    Jalr { dest: u32, base: Value, offset: i32 }, // Jump and link register
    
    // System instructions
    Ecall, // Environment call
    Ebreak, // Environment break
    
    // Fence instructions for memory ordering
    Fence { pred: u32, succ: u32 }, // Memory fence
    FenceI, // Instruction fence
    
    // CSR (Control and Status Register) instructions
    Csrrw { dest: u32, csr: u32, src: Value }, // CSR read/write
    Csrrs { dest: u32, csr: u32, src: Value }, // CSR read and set
    Csrrc { dest: u32, csr: u32, src: Value }, // CSR read and clear
    Csrrwi { dest: u32, csr: u32, imm: u32 }, // CSR read/write immediate
    Csrrsi { dest: u32, csr: u32, imm: u32 }, // CSR read and set immediate
    Csrrci { dest: u32, csr: u32, imm: u32 }, // CSR read and clear immediate
    
    /// RV32M/RV64M: Standard Extension for Integer Multiplication and Division
    
    // Multiplication
    Mul { dest: u32, src1: Value, src2: Value }, // Multiply (lower 32 bits)
    Mulh { dest: u32, src1: Value, src2: Value }, // Multiply high signed
    Mulhsu { dest: u32, src1: Value, src2: Value }, // Multiply high signed-unsigned
    Mulhu { dest: u32, src1: Value, src2: Value }, // Multiply high unsigned
    Mulw { dest: u32, src1: Value, src2: Value }, // Multiply word (RV64M)
    
    // Division
    Div { dest: u32, src1: Value, src2: Value }, // Divide signed
    Divu { dest: u32, src1: Value, src2: Value }, // Divide unsigned
    Rem { dest: u32, src1: Value, src2: Value }, // Remainder signed
    Remu { dest: u32, src1: Value, src2: Value }, // Remainder unsigned
    Divw { dest: u32, src1: Value, src2: Value }, // Divide word (RV64M)
    Divuw { dest: u32, src1: Value, src2: Value }, // Divide unsigned word (RV64M)
    Remw { dest: u32, src1: Value, src2: Value }, // Remainder word (RV64M)
    Remuw { dest: u32, src1: Value, src2: Value }, // Remainder unsigned word (RV64M)
    
    /// RV32A/RV64A: Standard Extension for Atomic Instructions
    
    // Load-Reserved/Store-Conditional
    LrW { dest: u32, addr: Value, aq: bool, rl: bool }, // Load-reserved word
    ScW { dest: u32, addr: Value, src: Value, aq: bool, rl: bool }, // Store-conditional word
    LrD { dest: u32, addr: Value, aq: bool, rl: bool }, // Load-reserved doubleword (RV64A)
    ScD { dest: u32, addr: Value, src: Value, aq: bool, rl: bool }, // Store-conditional doubleword (RV64A)
    
    // Atomic Memory Operations
    AmoswapW { dest: u32, addr: Value, src: Value, aq: bool, rl: bool }, // Atomic swap word
    AmoaddW { dest: u32, addr: Value, src: Value, aq: bool, rl: bool }, // Atomic add word
    AmoxorW { dest: u32, addr: Value, src: Value, aq: bool, rl: bool }, // Atomic XOR word
    AmoandW { dest: u32, addr: Value, src: Value, aq: bool, rl: bool }, // Atomic AND word
    AmoorW { dest: u32, addr: Value, src: Value, aq: bool, rl: bool }, // Atomic OR word
    AmominW { dest: u32, addr: Value, src: Value, aq: bool, rl: bool }, // Atomic minimum word
    AmomaxW { dest: u32, addr: Value, src: Value, aq: bool, rl: bool }, // Atomic maximum word
    AmominuW { dest: u32, addr: Value, src: Value, aq: bool, rl: bool }, // Atomic minimum unsigned word
    AmomaxuW { dest: u32, addr: Value, src: Value, aq: bool, rl: bool }, // Atomic maximum unsigned word
    
    // RV64A Doubleword Atomics
    AmoswapD { dest: u32, addr: Value, src: Value, aq: bool, rl: bool }, // Atomic swap doubleword
    AmoaddD { dest: u32, addr: Value, src: Value, aq: bool, rl: bool }, // Atomic add doubleword
    AmoxorD { dest: u32, addr: Value, src: Value, aq: bool, rl: bool }, // Atomic XOR doubleword
    AmoandD { dest: u32, addr: Value, src: Value, aq: bool, rl: bool }, // Atomic AND doubleword
    AmoorD { dest: u32, addr: Value, src: Value, aq: bool, rl: bool }, // Atomic OR doubleword
    AmominD { dest: u32, addr: Value, src: Value, aq: bool, rl: bool }, // Atomic minimum doubleword
    AmomaxD { dest: u32, addr: Value, src: Value, aq: bool, rl: bool }, // Atomic maximum doubleword
    AmominuD { dest: u32, addr: Value, src: Value, aq: bool, rl: bool }, // Atomic minimum unsigned doubleword
    AmomaxuD { dest: u32, addr: Value, src: Value, aq: bool, rl: bool }, // Atomic maximum unsigned doubleword
    
    /// RV32F: Standard Extension for Single-Precision Floating-Point
    
    // Single-precision floating-point loads/stores
    Flw { dest: u32, base: Value, offset: i32 }, // Load single-precision float
    Fsw { src: Value, base: Value, offset: i32 }, // Store single-precision float
    
    // Single-precision floating-point arithmetic
    FaddS { dest: u32, src1: Value, src2: Value, rm: u32 }, // Add single-precision
    FsubS { dest: u32, src1: Value, src2: Value, rm: u32 }, // Subtract single-precision
    FmulS { dest: u32, src1: Value, src2: Value, rm: u32 }, // Multiply single-precision
    FdivS { dest: u32, src1: Value, src2: Value, rm: u32 }, // Divide single-precision
    FsqrtS { dest: u32, src: Value, rm: u32 }, // Square root single-precision
    
    // Single-precision floating-point sign injection
    FsgnjS { dest: u32, src1: Value, src2: Value }, // Sign inject
    FsgnjnS { dest: u32, src1: Value, src2: Value }, // Sign inject negative
    FsgnjxS { dest: u32, src1: Value, src2: Value }, // Sign inject XOR
    
    // Single-precision floating-point min/max
    FminS { dest: u32, src1: Value, src2: Value }, // Minimum
    FmaxS { dest: u32, src1: Value, src2: Value }, // Maximum
    
    // Single-precision floating-point conversion
    FcvtWS { dest: u32, src: Value, rm: u32 }, // Convert float to word
    FcvtWuS { dest: u32, src: Value, rm: u32 }, // Convert float to unsigned word
    FmvXW { dest: u32, src: Value }, // Move float to integer register
    
    // Single-precision floating-point comparison
    FeqS { dest: u32, src1: Value, src2: Value }, // Equal
    FltS { dest: u32, src1: Value, src2: Value }, // Less than
    FleS { dest: u32, src1: Value, src2: Value }, // Less than or equal
    
    // Single-precision floating-point convert from integer
    FcvtSW { dest: u32, src: Value, rm: u32 }, // Convert word to float
    FcvtSWu { dest: u32, src: Value, rm: u32 }, // Convert unsigned word to float
    FmvWX { dest: u32, src: Value }, // Move integer to float register
    
    /// RV64F additional instructions
    FcvtLS { dest: u32, src: Value, rm: u32 }, // Convert float to long
    FcvtLuS { dest: u32, src: Value, rm: u32 }, // Convert float to unsigned long
    FcvtSL { dest: u32, src: Value, rm: u32 }, // Convert long to float
    FcvtSLu { dest: u32, src: Value, rm: u32 }, // Convert unsigned long to float
    
    /// RV32D: Standard Extension for Double-Precision Floating-Point
    
    // Double-precision floating-point loads/stores
    Fld { dest: u32, base: Value, offset: i32 }, // Load double-precision float
    Fsd { src: Value, base: Value, offset: i32 }, // Store double-precision float
    
    // Double-precision floating-point arithmetic
    FaddD { dest: u32, src1: Value, src2: Value, rm: u32 }, // Add double-precision
    FsubD { dest: u32, src1: Value, src2: Value, rm: u32 }, // Subtract double-precision
    FmulD { dest: u32, src1: Value, src2: Value, rm: u32 }, // Multiply double-precision
    FdivD { dest: u32, src1: Value, src2: Value, rm: u32 }, // Divide double-precision
    FsqrtD { dest: u32, src: Value, rm: u32 }, // Square root double-precision
    
    // Double-precision floating-point sign injection
    FsgnjD { dest: u32, src1: Value, src2: Value }, // Sign inject
    FsgnjnD { dest: u32, src1: Value, src2: Value }, // Sign inject negative
    FsgnjxD { dest: u32, src1: Value, src2: Value }, // Sign inject XOR
    
    // Double-precision floating-point min/max
    FminD { dest: u32, src1: Value, src2: Value }, // Minimum
    FmaxD { dest: u32, src1: Value, src2: Value }, // Maximum
    
    // Double-precision floating-point conversion
    FcvtSD { dest: u32, src: Value, rm: u32 }, // Convert double to single
    FcvtDS { dest: u32, src: Value, rm: u32 }, // Convert single to double
    
    // Double-precision floating-point comparison
    FeqD { dest: u32, src1: Value, src2: Value }, // Equal
    FltD { dest: u32, src1: Value, src2: Value }, // Less than
    FleD { dest: u32, src1: Value, src2: Value }, // Less than or equal
    
    // Double-precision integer conversion
    FcvtWD { dest: u32, src: Value, rm: u32 }, // Convert double to word
    FcvtWuD { dest: u32, src: Value, rm: u32 }, // Convert double to unsigned word
    FcvtDW { dest: u32, src: Value, rm: u32 }, // Convert word to double
    FcvtDWu { dest: u32, src: Value, rm: u32 }, // Convert unsigned word to double
    
    /// RV64D additional instructions
    FcvtLD { dest: u32, src: Value, rm: u32 }, // Convert double to long
    FcvtLuD { dest: u32, src: Value, rm: u32 }, // Convert double to unsigned long
    FmvXD { dest: u32, src: Value }, // Move double to integer register
    FcvtDL { dest: u32, src: Value, rm: u32 }, // Convert long to double
    FcvtDLu { dest: u32, src: Value, rm: u32 }, // Convert unsigned long to double
    FmvDX { dest: u32, src: Value }, // Move integer to double register
    
    /// RV32C: Standard Extension for Compressed Instructions
    /// Note: These are 16-bit encodings of common 32-bit instructions
    /// In LLVM IR, they map to the same operations as their 32-bit counterparts
    
    // Compressed arithmetic
    CAddi { dest: u32, imm: i32 }, // Compressed add immediate
    CAddi16sp { imm: i32 }, // Compressed add immediate to SP (special case)
    CAddi4spn { dest: u32, imm: u32 }, // Compressed add immediate 4*sp to dest
    CLi { dest: u32, imm: i32 }, // Compressed load immediate
    CLui { dest: u32, imm: u32 }, // Compressed load upper immediate
    
    // Compressed loads/stores
    CLw { dest: u32, base: Value, offset: u32 }, // Compressed load word
    CSw { src: Value, base: Value, offset: u32 }, // Compressed store word
    CLwsp { dest: u32, offset: u32 }, // Compressed load word from stack
    CSwsp { src: Value, offset: u32 }, // Compressed store word to stack
    
    // Compressed control flow
    CJ { offset: i32 }, // Compressed jump
    CJr { src: Value }, // Compressed jump register
    CJalr { src: Value }, // Compressed jump and link register
    CBeqz { src: Value, offset: i32 }, // Compressed branch if equal zero
    CBnez { src: Value, offset: i32 }, // Compressed branch if not equal zero
    
    // Compressed register operations
    CAdd { dest: u32, src: Value }, // Compressed add
    CMv { dest: u32, src: Value }, // Compressed move
    CAnd { dest: u32, src: Value }, // Compressed and
    COr { dest: u32, src: Value }, // Compressed or
    CXor { dest: u32, src: Value }, // Compressed xor
    CSub { dest: u32, src: Value }, // Compressed subtract
    
    // RV64C additional
    CLd { dest: u32, base: Value, offset: u32 }, // Compressed load doubleword
    CSd { src: Value, base: Value, offset: u32 }, // Compressed store doubleword
    CLdsp { dest: u32, offset: u32 }, // Compressed load doubleword from stack
    CSdsp { src: Value, offset: u32 }, // Compressed store doubleword to stack
    CAddiw { dest: u32, imm: i32 }, // Compressed add immediate word
    CSubw { dest: u32, src: Value }, // Compressed subtract word
    CAddw { dest: u32, src: Value }, // Compressed add word
    
    /// RVV 1.0: RISC-V Vector Extension
    /// Scalable vector operations for reactive programming optimization
    
    // Vector configuration
    Vsetvli { dest: u32, avl: Value, vtype: VectorType }, // Set vector length immediate
    Vsetvl { dest: u32, avl: Value, vtype_reg: Value }, // Set vector length
    Vsetivli { dest: u32, avl: u32, vtype: VectorType }, // Set vector length immediate (immediate AVL)
    
    // Vector loads and stores
    Vle32 { dest: u32, base: Value, vm: bool }, // Vector load 32-bit elements
    Vle64 { dest: u32, base: Value, vm: bool }, // Vector load 64-bit elements
    Vse32 { src: u32, base: Value, vm: bool }, // Vector store 32-bit elements
    Vse64 { src: u32, base: Value, vm: bool }, // Vector store 64-bit elements
    
    // Strided loads and stores
    Vlse32 { dest: u32, base: Value, stride: Value, vm: bool }, // Strided load 32-bit
    Vlse64 { dest: u32, base: Value, stride: Value, vm: bool }, // Strided load 64-bit
    Vsse32 { src: u32, base: Value, stride: Value, vm: bool }, // Strided store 32-bit
    Vsse64 { src: u32, base: Value, stride: Value, vm: bool }, // Strided store 64-bit
    
    // Vector integer arithmetic
    VaddVv { dest: u32, src1: u32, src2: u32, vm: bool }, // Vector-vector add
    VaddVx { dest: u32, src1: u32, src2: Value, vm: bool }, // Vector-scalar add
    VaddVi { dest: u32, src1: u32, imm: i32, vm: bool }, // Vector-immediate add
    VsubVv { dest: u32, src1: u32, src2: u32, vm: bool }, // Vector-vector subtract
    VsubVx { dest: u32, src1: u32, src2: Value, vm: bool }, // Vector-scalar subtract
    VmulVv { dest: u32, src1: u32, src2: u32, vm: bool }, // Vector-vector multiply
    VmulVx { dest: u32, src1: u32, src2: Value, vm: bool }, // Vector-scalar multiply
    VdivVv { dest: u32, src1: u32, src2: u32, vm: bool }, // Vector-vector divide
    VdivVx { dest: u32, src1: u32, src2: Value, vm: bool }, // Vector-scalar divide
    
    // Vector floating-point arithmetic
    VfaddVv { dest: u32, src1: u32, src2: u32, vm: bool }, // Vector-vector float add
    VfaddVf { dest: u32, src1: u32, src2: Value, vm: bool }, // Vector-scalar float add
    VfsubVv { dest: u32, src1: u32, src2: u32, vm: bool }, // Vector-vector float subtract
    VfsubVf { dest: u32, src1: u32, src2: Value, vm: bool }, // Vector-scalar float subtract
    VfmulVv { dest: u32, src1: u32, src2: u32, vm: bool }, // Vector-vector float multiply
    VfmulVf { dest: u32, src1: u32, src2: Value, vm: bool }, // Vector-scalar float multiply
    VfdivVv { dest: u32, src1: u32, src2: u32, vm: bool }, // Vector-vector float divide
    VfdivVf { dest: u32, src1: u32, src2: Value, vm: bool }, // Vector-scalar float divide
    
    // Vector reduction operations (for reduce in reactive programming)
    VredSumVs { dest: u32, src: u32, scalar: Value, vm: bool }, // Reduce sum
    VredMaxVs { dest: u32, src: u32, scalar: Value, vm: bool }, // Reduce max
    VredMinVs { dest: u32, src: u32, scalar: Value, vm: bool }, // Reduce min
    VredAndVs { dest: u32, src: u32, scalar: Value, vm: bool }, // Reduce AND
    VredOrVs { dest: u32, src: u32, scalar: Value, vm: bool }, // Reduce OR
    VredXorVs { dest: u32, src: u32, scalar: Value, vm: bool }, // Reduce XOR
    
    // Vector floating-point reductions
    VfredSumVs { dest: u32, src: u32, scalar: Value, vm: bool }, // Float reduce sum
    VfredMaxVs { dest: u32, src: u32, scalar: Value, vm: bool }, // Float reduce max
    VfredMinVs { dest: u32, src: u32, scalar: Value, vm: bool }, // Float reduce min
    
    // Vector mask operations (for filter in reactive programming)
    VmseqVv { dest: u32, src1: u32, src2: u32, vm: bool }, // Mask set equal vector-vector
    VmseqVx { dest: u32, src1: u32, src2: Value, vm: bool }, // Mask set equal vector-scalar
    VmsneVv { dest: u32, src1: u32, src2: u32, vm: bool }, // Mask set not equal vector-vector
    VmsneVx { dest: u32, src1: u32, src2: Value, vm: bool }, // Mask set not equal vector-scalar
    VmsltVv { dest: u32, src1: u32, src2: u32, vm: bool }, // Mask set less than vector-vector
    VmsltVx { dest: u32, src1: u32, src2: Value, vm: bool }, // Mask set less than vector-scalar
    VmsleVv { dest: u32, src1: u32, src2: u32, vm: bool }, // Mask set less equal vector-vector
    VmsleVx { dest: u32, src1: u32, src2: Value, vm: bool }, // Mask set less equal vector-scalar
    VmsgtVx { dest: u32, src1: u32, src2: Value, vm: bool }, // Mask set greater than vector-scalar
    VmsgeVx { dest: u32, src1: u32, src2: Value, vm: bool }, // Mask set greater equal vector-scalar
    
    // Vector merge and move (for conditional operations)
    VmergeVvm { dest: u32, src1: u32, src2: u32, mask: u32 }, // Merge vector-vector with mask
    VmergeVxm { dest: u32, src1: u32, src2: Value, mask: u32 }, // Merge vector-scalar with mask
    VmergeVim { dest: u32, src1: u32, imm: i32, mask: u32 }, // Merge vector-immediate with mask
    VmvVv { dest: u32, src: u32 }, // Vector move vector-to-vector
    VmvVx { dest: u32, src: Value }, // Vector move scalar-to-vector
    VmvSx { dest: u32, src: Value }, // Move scalar to vector element 0
    VmvXs { dest: u32, src: u32 }, // Move vector element 0 to scalar
    
    // Vector slide operations (for stream operations)
    VslideupVx { dest: u32, src: u32, offset: Value, vm: bool }, // Slide up by scalar
    VslideupVi { dest: u32, src: u32, offset: u32, vm: bool }, // Slide up by immediate
    VslidedownVx { dest: u32, src: u32, offset: Value, vm: bool }, // Slide down by scalar
    VslidedownVi { dest: u32, src: u32, offset: u32, vm: bool }, // Slide down by immediate
    Vslide1upVx { dest: u32, src: u32, scalar: Value, vm: bool }, // Slide up by 1 with scalar
    Vslide1downVx { dest: u32, src: u32, scalar: Value, vm: bool }, // Slide down by 1 with scalar
    
    // Vector gather/scatter (for complex data access patterns)
    VluxeiVv { dest: u32, base: Value, index: u32, vm: bool }, // Indexed unordered load
    VsuxeiVv { src: u32, base: Value, index: u32, vm: bool }, // Indexed unordered store
    VloxeiVv { dest: u32, base: Value, index: u32, vm: bool }, // Indexed ordered load
    VsoxeiVv { src: u32, base: Value, index: u32, vm: bool }, // Indexed ordered store
}

/// Vector type configuration for RISC-V Vector Extension
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct VectorType {
    pub sew: VectorSEW,     // Selected element width
    pub lmul: VectorLMUL,    // Length multiplier
    pub vta: bool,           // Vector tail agnostic
    pub vma: bool,           // Vector mask agnostic
}

impl VectorType {
    /// Create a new vector type configuration
    pub fn new(sew: VectorSEW, lmul: VectorLMUL) -> Self {
        Self {
            sew,
            lmul,
            vta: true,  // Tail agnostic by default for performance
            vma: true,  // Mask agnostic by default for performance
        }
    }
    
    /// Encode as vtype immediate for vsetvli instruction
    pub fn encode(&self) -> u32 {
        let sew_bits = match self.sew {
            VectorSEW::E8 => 0b000,
            VectorSEW::E16 => 0b001,
            VectorSEW::E32 => 0b010,
            VectorSEW::E64 => 0b011,
        };
        
        let lmul_bits = match self.lmul {
            VectorLMUL::MF8 => 0b101,
            VectorLMUL::MF4 => 0b110,
            VectorLMUL::MF2 => 0b111,
            VectorLMUL::M1 => 0b000,
            VectorLMUL::M2 => 0b001,
            VectorLMUL::M4 => 0b010,
            VectorLMUL::M8 => 0b011,
        };
        
        let vta_bit = if self.vta { 1 << 6 } else { 0 };
        let vma_bit = if self.vma { 1 << 7 } else { 0 };
        
        (sew_bits << 3) | lmul_bits | vta_bit | vma_bit
    }
}

/// Vector selected element width
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VectorSEW {
    E8,   // 8-bit elements
    E16,  // 16-bit elements
    E32,  // 32-bit elements
    E64,  // 64-bit elements
}

/// Vector length multiplier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VectorLMUL {
    MF8,  // 1/8 of a vector register
    MF4,  // 1/4 of a vector register
    MF2,  // 1/2 of a vector register
    M1,   // 1 vector register
    M2,   // 2 vector registers
    M4,   // 4 vector registers
    M8,   // 8 vector registers
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
    pub target: Target,
}

/// Target architecture specification
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Target {
    pub arch: Architecture,
    pub vendor: String,
    pub os: OperatingSystem,
    pub env: Environment,
}

impl Target {
    pub fn new(arch: Architecture, vendor: String, os: OperatingSystem, env: Environment) -> Self {
        Self { arch, vendor, os, env }
    }
    
    pub fn x86_64_linux() -> Self {
        Self {
            arch: Architecture::X86_64,
            vendor: "unknown".to_string(),
            os: OperatingSystem::Linux,
            env: Environment::Gnu,
        }
    }
    
    pub fn riscv64_linux() -> Self {
        Self {
            arch: Architecture::RiscV64,
            vendor: "unknown".to_string(),
            os: OperatingSystem::Linux,
            env: Environment::Gnu,
        }
    }
    
    pub fn riscv32_linux() -> Self {
        Self {
            arch: Architecture::RiscV32,
            vendor: "unknown".to_string(),
            os: OperatingSystem::Linux,
            env: Environment::Gnu,
        }
    }
    
    pub fn riscv64_bare_metal() -> Self {
        Self {
            arch: Architecture::RiscV64,
            vendor: "unknown".to_string(),
            os: OperatingSystem::None,
            env: Environment::None,
        }
    }
    
    pub fn riscv32_bare_metal() -> Self {
        Self {
            arch: Architecture::RiscV32,
            vendor: "unknown".to_string(),
            os: OperatingSystem::None,
            env: Environment::None,
        }
    }
    
    /// Generate LLVM target triple string
    pub fn to_llvm_triple(&self) -> String {
        format!("{}-{}-{}-{}", self.arch, self.vendor, self.os, self.env)
    }
    
    /// Parse from LLVM target triple string
    pub fn from_llvm_triple(triple: &str) -> Option<Self> {
        let parts: Vec<&str> = triple.split('-').collect();
        if parts.len() < 3 {
            return None;
        }
        
        let arch = Architecture::from_str(parts[0])?;
        let vendor = parts[1].to_string();
        let os = OperatingSystem::from_str(parts[2])?;
        let env = if parts.len() >= 4 {
            Environment::from_str(parts[3])?
        } else {
            Environment::None
        };
        
        Some(Self { arch, vendor, os, env })
    }
    
    /// Check if target supports RISC-V vector extension
    pub fn supports_rvv(&self) -> bool {
        matches!(self.arch, Architecture::RiscV32 | Architecture::RiscV64)
    }
    
    /// Get native register size in bits
    pub fn register_size(&self) -> u32 {
        match self.arch {
            Architecture::X86_64 => 64,
            Architecture::RiscV64 => 64,
            Architecture::RiscV32 => 32,
            Architecture::Wasm32 => 32,
        }
    }
    
    /// Check if target is RISC-V
    pub fn is_riscv(&self) -> bool {
        matches!(self.arch, Architecture::RiscV32 | Architecture::RiscV64)
    }
}

/// Supported target architectures
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Architecture {
    X86_64,
    RiscV32,
    RiscV64,
    Wasm32,
}

impl Architecture {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "x86_64" | "x86-64" | "amd64" => Some(Architecture::X86_64),
            "riscv32" | "riscv32i" | "riscv32im" | "riscv32imc" | "riscv32imac" 
            | "riscv32imafc" | "riscv32imafdc" | "rv32i" | "rv32im" | "rv32imc" 
            | "rv32imac" | "rv32imafc" | "rv32imafdc" => Some(Architecture::RiscV32),
            "riscv64" | "riscv64i" | "riscv64im" | "riscv64imc" | "riscv64imac" 
            | "riscv64imafc" | "riscv64imafdc" | "rv64i" | "rv64im" | "rv64imc" 
            | "rv64imac" | "rv64imafc" | "rv64imafdc" => Some(Architecture::RiscV64),
            "wasm32" => Some(Architecture::Wasm32),
            _ => None,
        }
    }
    
    /// Get RISC-V extensions from architecture string
    pub fn riscv_extensions(&self, arch_str: &str) -> RiscVExtensions {
        let arch_lower = arch_str.to_lowercase();
        RiscVExtensions {
            i: true, // Base integer ISA (always present)
            m: arch_lower.contains('m'),
            a: arch_lower.contains('a'),
            f: arch_lower.contains('f'),
            d: arch_lower.contains('d'),
            c: arch_lower.contains('c'),
            v: arch_lower.contains('v'), // Vector extension
        }
    }
}

impl fmt::Display for Architecture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Architecture::X86_64 => write!(f, "x86_64"),
            Architecture::RiscV32 => write!(f, "riscv32"),
            Architecture::RiscV64 => write!(f, "riscv64"),
            Architecture::Wasm32 => write!(f, "wasm32"),
        }
    }
}

/// Operating system targets
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperatingSystem {
    Linux,
    Windows,
    MacOS,
    FreeBSD,
    None, // Bare metal
}

impl OperatingSystem {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "linux" | "linux-gnu" => Some(OperatingSystem::Linux),
            "windows" | "pc-windows" => Some(OperatingSystem::Windows),
            "macos" | "apple-darwin" => Some(OperatingSystem::MacOS),
            "freebsd" => Some(OperatingSystem::FreeBSD),
            "none" | "elf" => Some(OperatingSystem::None),
            _ => None,
        }
    }
}

impl fmt::Display for OperatingSystem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OperatingSystem::Linux => write!(f, "linux"),
            OperatingSystem::Windows => write!(f, "windows"),
            OperatingSystem::MacOS => write!(f, "darwin"),
            OperatingSystem::FreeBSD => write!(f, "freebsd"),
            OperatingSystem::None => write!(f, "none"),
        }
    }
}

/// Environment/ABI targets
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Environment {
    Gnu,
    Musl,
    Msvc,
    Elf,
    None,
}

impl Environment {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "gnu" => Some(Environment::Gnu),
            "musl" => Some(Environment::Musl),
            "msvc" => Some(Environment::Msvc),
            "elf" => Some(Environment::Elf),
            "none" | "" => Some(Environment::None),
            _ => None,
        }
    }
}

impl fmt::Display for Environment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Environment::Gnu => write!(f, "gnu"),
            Environment::Musl => write!(f, "musl"),
            Environment::Msvc => write!(f, "msvc"),
            Environment::Elf => write!(f, "elf"),
            Environment::None => write!(f, ""),
        }
    }
}

/// RISC-V ISA extensions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RiscVExtensions {
    pub i: bool, // Integer base ISA
    pub m: bool, // Integer multiplication and division
    pub a: bool, // Atomic instructions
    pub f: bool, // Single-precision floating-point
    pub d: bool, // Double-precision floating-point
    pub c: bool, // Compressed instructions
    pub v: bool, // Vector extension
}

impl RiscVExtensions {
    /// Create RV32I base ISA
    pub fn rv32i() -> Self {
        Self {
            i: true,
            m: false,
            a: false,
            f: false,
            d: false,
            c: false,
            v: false,
        }
    }
    
    /// Create RV64I base ISA
    pub fn rv64i() -> Self {
        Self::rv32i()
    }
    
    /// Create RV32IMAFDC (full general-purpose ISA)
    pub fn rv32gc() -> Self {
        Self {
            i: true,
            m: true,
            a: true,
            f: true,
            d: true,
            c: true,
            v: false,
        }
    }
    
    /// Create RV64IMAFDC (full general-purpose ISA)  
    pub fn rv64gc() -> Self {
        Self::rv32gc()
    }
    
    /// Create RV32IMAFDC with vector extension
    pub fn rv32gcv() -> Self {
        Self {
            i: true,
            m: true,
            a: true,
            f: true,
            d: true,
            c: true,
            v: true,
        }
    }
    
    /// Create RV64IMAFDC with vector extension
    pub fn rv64gcv() -> Self {
        Self::rv32gcv()
    }
    
    /// Generate ISA string (e.g., "rv64imafdcv")
    pub fn to_isa_string(&self, xlen: u32) -> String {
        let mut isa = format!("rv{xlen}i");
        if self.m { isa.push('m'); }
        if self.a { isa.push('a'); }
        if self.f { isa.push('f'); }
        if self.d { isa.push('d'); }
        if self.c { isa.push('c'); }
        if self.v { isa.push('v'); }
        isa
    }
    
    /// Check if extensions are valid combination
    pub fn is_valid(&self) -> bool {
        // 'I' (integer) is required
        if !self.i {
            return false;
        }
        
        // 'D' requires 'F'
        if self.d && !self.f {
            return false;
        }
        
        true
    }
}