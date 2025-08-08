//! Tests for RISC-V standard extensions (MAFDC)
//! Covers multiplication/division, atomics, floating-point, and compressed instructions

use seen_ir::ir::{Target, RiscVExtensions, RiscVInstruction, Value, Module, Function, BasicBlock, Instruction};
use seen_ir::codegen::CodeGenerator;

#[test]
fn test_riscv_m_extension_multiplication() {
    let target = Target::riscv32_linux();
    let extensions = RiscVExtensions { i: true, m: true, a: false, f: false, d: false, c: false, v: false };
    let mut codegen = CodeGenerator::new("riscv_m_mul_test".to_string());
    
    let test_module = Module {
        name: "riscv_m_mul_test".to_string(),
        target,
        functions: vec![Function {
            name: "test_multiplication".to_string(),
            params: vec!["a".to_string(), "b".to_string()],
            blocks: vec![BasicBlock {
                label: "entry".to_string(),
                instructions: vec![
                    // MUL r3, r1, r2 (standard multiplication)
                    Instruction::RiscV(RiscVInstruction::Mul {
                        dest: 3,
                        src1: Value::Register(1),
                        src2: Value::Register(2),
                    }),
                    // MULH r4, r1, r2 (high signed multiplication)
                    Instruction::RiscV(RiscVInstruction::Mulh {
                        dest: 4,
                        src1: Value::Register(1),
                        src2: Value::Register(2),
                    }),
                    // MULHU r5, r1, r2 (high unsigned multiplication)
                    Instruction::RiscV(RiscVInstruction::Mulhu {
                        dest: 5,
                        src1: Value::Register(1),
                        src2: Value::Register(2),
                    }),
                    Instruction::Return { value: Some(Value::Register(3)) },
                ],
            }],
        }],
    };
    
    let llvm_ir = codegen.generate_llvm_ir(&test_module).expect("Failed to generate LLVM IR");
    
    // Verify basic multiplication
    assert!(llvm_ir.contains("%3 = mul i32 %1, %2"));
    
    // Verify high signed multiplication (MULH)
    assert!(llvm_ir.contains("%4.ext1 = sext i32 %1 to i64"));
    assert!(llvm_ir.contains("%4.ext2 = sext i32 %2 to i64"));
    assert!(llvm_ir.contains("%4.full = mul i64 %4.ext1, %4.ext2"));
    assert!(llvm_ir.contains("%4.shifted = lshr i64 %4.full, 32"));
    assert!(llvm_ir.contains("%4 = trunc i64 %4.shifted to i32"));
    
    // Verify high unsigned multiplication (MULHU)
    assert!(llvm_ir.contains("%5.ext1 = zext i32 %1 to i64"));
    assert!(llvm_ir.contains("%5.ext2 = zext i32 %2 to i64"));
    assert!(llvm_ir.contains("%5.full = mul i64 %5.ext1, %5.ext2"));
}

#[test]
fn test_riscv_m_extension_division() {
    let target = Target::riscv64_linux();
    let extensions = RiscVExtensions { i: true, m: true, a: false, f: false, d: false, c: false, v: false };
    let mut codegen = CodeGenerator::new("riscv_m_div_test".to_string());
    
    let test_module = Module {
        name: "riscv_m_div_test".to_string(),
        target,
        functions: vec![Function {
            name: "test_division".to_string(),
            params: vec!["dividend".to_string(), "divisor".to_string()],
            blocks: vec![BasicBlock {
                label: "entry".to_string(),
                instructions: vec![
                    // DIV r3, r1, r2 (signed division)
                    Instruction::RiscV(RiscVInstruction::Div {
                        dest: 3,
                        src1: Value::Register(1),
                        src2: Value::Register(2),
                    }),
                    // DIVU r4, r1, r2 (unsigned division)
                    Instruction::RiscV(RiscVInstruction::Divu {
                        dest: 4,
                        src1: Value::Register(1),
                        src2: Value::Register(2),
                    }),
                    // REM r5, r1, r2 (signed remainder)
                    Instruction::RiscV(RiscVInstruction::Rem {
                        dest: 5,
                        src1: Value::Register(1),
                        src2: Value::Register(2),
                    }),
                    // REMU r6, r1, r2 (unsigned remainder)
                    Instruction::RiscV(RiscVInstruction::Remu {
                        dest: 6,
                        src1: Value::Register(1),
                        src2: Value::Register(2),
                    }),
                    // DIVW r7, r1, r2 (RV64M: word division with sign extension)
                    Instruction::RiscV(RiscVInstruction::Divw {
                        dest: 7,
                        src1: Value::Register(1),
                        src2: Value::Register(2),
                    }),
                    Instruction::Return { value: Some(Value::Register(3)) },
                ],
            }],
        }],
    };
    
    let llvm_ir = codegen.generate_llvm_ir(&test_module).expect("Failed to generate LLVM IR");
    
    assert!(llvm_ir.contains("%3 = sdiv i32 %1, %2"));
    assert!(llvm_ir.contains("%4 = udiv i32 %1, %2"));
    assert!(llvm_ir.contains("%5 = srem i32 %1, %2"));
    assert!(llvm_ir.contains("%6 = urem i32 %1, %2"));
    
    // RV64M specific: DIVW with sign extension
    assert!(llvm_ir.contains("%7.tmp = sdiv i32 %1, %2"));
    assert!(llvm_ir.contains("%7 = sext i32 %7.tmp to i64"));
}

#[test]
fn test_riscv_a_extension_atomics() {
    let target = Target::riscv32_linux();
    let extensions = RiscVExtensions { i: true, m: false, a: true, f: false, d: false, c: false, v: false };
    let mut codegen = CodeGenerator::new("riscv_a_test".to_string());
    
    let test_module = Module {
        name: "riscv_a_test".to_string(),
        target,
        functions: vec![Function {
            name: "test_atomics".to_string(),
            params: vec!["addr".to_string(), "value".to_string()],
            blocks: vec![BasicBlock {
                label: "entry".to_string(),
                instructions: vec![
                    // LR.W r3, (r1) (load-reserved word)
                    Instruction::RiscV(RiscVInstruction::LrW {
                        dest: 3,
                        addr: Value::Register(1),
                        aq: true,
                        rl: false,
                    }),
                    // SC.W r4, r2, (r1) (store-conditional word)
                    Instruction::RiscV(RiscVInstruction::ScW {
                        dest: 4,
                        addr: Value::Register(1),
                        src: Value::Register(2),
                        aq: false,
                        rl: true,
                    }),
                    // AMOSWAP.W r5, r2, (r1) (atomic swap)
                    Instruction::RiscV(RiscVInstruction::AmoswapW {
                        dest: 5,
                        addr: Value::Register(1),
                        src: Value::Register(2),
                        aq: true,
                        rl: true,
                    }),
                    // AMOADD.W r6, r2, (r1) (atomic add)
                    Instruction::RiscV(RiscVInstruction::AmoaddW {
                        dest: 6,
                        addr: Value::Register(1),
                        src: Value::Register(2),
                        aq: false,
                        rl: false,
                    }),
                    // AMOAND.W r7, r2, (r1) (atomic AND)
                    Instruction::RiscV(RiscVInstruction::AmoandW {
                        dest: 7,
                        addr: Value::Register(1),
                        src: Value::Register(2),
                        aq: false,
                        rl: false,
                    }),
                    Instruction::Return { value: Some(Value::Register(5)) },
                ],
            }],
        }],
    };
    
    let llvm_ir = codegen.generate_llvm_ir(&test_module).expect("Failed to generate LLVM IR");
    
    // Load-reserved
    assert!(llvm_ir.contains("%3.ptr = inttoptr i32 %1 to i32*"));
    assert!(llvm_ir.contains("%3 = load atomic i32, i32* %3.ptr seq_cst, align 4"));
    
    // Store-conditional
    assert!(llvm_ir.contains("%4.ptr = inttoptr i32 %1 to i32*"));
    assert!(llvm_ir.contains("store atomic i32 %2, i32* %4.ptr seq_cst, align 4"));
    assert!(llvm_ir.contains("%4 = add i32 0, 0")); // Success flag
    
    // Atomic operations
    assert!(llvm_ir.contains("%5 = atomicrmw xchg i32* %5.ptr, i32 %2 seq_cst"));
    assert!(llvm_ir.contains("%6 = atomicrmw add i32* %6.ptr, i32 %2 seq_cst"));
    assert!(llvm_ir.contains("%7 = atomicrmw and i32* %7.ptr, i32 %2 seq_cst"));
}

#[test]
fn test_riscv_f_extension_single_precision() {
    let target = Target::riscv32_linux();
    let extensions = RiscVExtensions { i: true, m: false, a: false, f: true, d: false, c: false, v: false };
    let mut codegen = CodeGenerator::new("riscv_f_test".to_string());
    
    let test_module = Module {
        name: "riscv_f_test".to_string(),
        target,
        functions: vec![Function {
            name: "test_single_precision".to_string(),
            params: vec!["base_addr".to_string()],
            blocks: vec![BasicBlock {
                label: "entry".to_string(),
                instructions: vec![
                    // FLW f2, 4(r1) (load single-precision float)
                    Instruction::RiscV(RiscVInstruction::Flw {
                        dest: 2,
                        base: Value::Register(1),
                        offset: 4,
                    }),
                    // FLW f3, 8(r1)
                    Instruction::RiscV(RiscVInstruction::Flw {
                        dest: 3,
                        base: Value::Register(1),
                        offset: 8,
                    }),
                    // FADD.S f4, f2, f3 (single-precision add)
                    Instruction::RiscV(RiscVInstruction::FaddS {
                        dest: 4,
                        src1: Value::Register(2),
                        src2: Value::Register(3),
                        rm: 0, // Round to nearest, ties to even
                    }),
                    // FMUL.S f5, f2, f3 (single-precision multiply)
                    Instruction::RiscV(RiscVInstruction::FmulS {
                        dest: 5,
                        src1: Value::Register(2),
                        src2: Value::Register(3),
                        rm: 0,
                    }),
                    // FSQRT.S f6, f4 (single-precision square root)
                    Instruction::RiscV(RiscVInstruction::FsqrtS {
                        dest: 6,
                        src: Value::Register(4),
                        rm: 0,
                    }),
                    // FEQ.S r7, f2, f3 (single-precision equal comparison)
                    Instruction::RiscV(RiscVInstruction::FeqS {
                        dest: 7,
                        src1: Value::Register(2),
                        src2: Value::Register(3),
                    }),
                    // FSW f6, 12(r1) (store single-precision float)
                    Instruction::RiscV(RiscVInstruction::Fsw {
                        src: Value::Register(6),
                        base: Value::Register(1),
                        offset: 12,
                    }),
                    Instruction::Return { value: Some(Value::Register(7)) },
                ],
            }],
        }],
    };
    
    let llvm_ir = codegen.generate_llvm_ir(&test_module).expect("Failed to generate LLVM IR");
    
    // Single-precision loads
    assert!(llvm_ir.contains("%2.addr = add i32 %1, 4"));
    assert!(llvm_ir.contains("%2.ptr = inttoptr i32 %2.addr to float*"));
    assert!(llvm_ir.contains("%2 = load float, float* %2.ptr, align 4"));
    
    // Single-precision arithmetic
    assert!(llvm_ir.contains("%4 = fadd float %2, %3"));
    assert!(llvm_ir.contains("%5 = fmul float %2, %3"));
    assert!(llvm_ir.contains("%6 = call float @llvm.sqrt.f32(float %4)"));
    
    // Single-precision comparison
    assert!(llvm_ir.contains("%7.cmp = fcmp oeq float %2, %3"));
    assert!(llvm_ir.contains("%7 = zext i1 %7.cmp to i32"));
    
    // Single-precision store
    assert!(llvm_ir.contains("%fsw.addr = add i32 %1, 12"));
    assert!(llvm_ir.contains("%fsw.ptr = inttoptr i32 %fsw.addr to float*"));
    assert!(llvm_ir.contains("store float %6, float* %fsw.ptr, align 4"));
}

#[test]
fn test_riscv_d_extension_double_precision() {
    let target = Target::riscv64_linux();
    let extensions = RiscVExtensions { i: true, m: false, a: false, f: true, d: true, c: false, v: false };
    let mut codegen = CodeGenerator::new("riscv_d_test".to_string());
    
    let test_module = Module {
        name: "riscv_d_test".to_string(),
        target,
        functions: vec![Function {
            name: "test_double_precision".to_string(),
            params: vec!["base_addr".to_string()],
            blocks: vec![BasicBlock {
                label: "entry".to_string(),
                instructions: vec![
                    // FLD f2, 8(r1) (load double-precision float)
                    Instruction::RiscV(RiscVInstruction::Fld {
                        dest: 2,
                        base: Value::Register(1),
                        offset: 8,
                    }),
                    // FLD f3, 16(r1)
                    Instruction::RiscV(RiscVInstruction::Fld {
                        dest: 3,
                        base: Value::Register(1),
                        offset: 16,
                    }),
                    // FADD.D f4, f2, f3 (double-precision add)
                    Instruction::RiscV(RiscVInstruction::FaddD {
                        dest: 4,
                        src1: Value::Register(2),
                        src2: Value::Register(3),
                        rm: 0,
                    }),
                    // FDIV.D f5, f4, f3 (double-precision divide)
                    Instruction::RiscV(RiscVInstruction::FdivD {
                        dest: 5,
                        src1: Value::Register(4),
                        src2: Value::Register(3),
                        rm: 0,
                    }),
                    // FCVT.S.D f6, f5 (convert double to single)
                    Instruction::RiscV(RiscVInstruction::FcvtSD {
                        dest: 6,
                        src: Value::Register(5),
                        rm: 0,
                    }),
                    // FCVT.W.D r7, f5 (convert double to word)
                    Instruction::RiscV(RiscVInstruction::FcvtWD {
                        dest: 7,
                        src: Value::Register(5),
                        rm: 0,
                    }),
                    // FSD f5, 24(r1) (store double-precision float)
                    Instruction::RiscV(RiscVInstruction::Fsd {
                        src: Value::Register(5),
                        base: Value::Register(1),
                        offset: 24,
                    }),
                    Instruction::Return { value: Some(Value::Register(7)) },
                ],
            }],
        }],
    };
    
    let llvm_ir = codegen.generate_llvm_ir(&test_module).expect("Failed to generate LLVM IR");
    
    // Double-precision loads
    assert!(llvm_ir.contains("%2.addr = add i32 %1, 8"));
    assert!(llvm_ir.contains("%2.ptr = inttoptr i32 %2.addr to double*"));
    assert!(llvm_ir.contains("%2 = load double, double* %2.ptr, align 8"));
    
    // Double-precision arithmetic
    assert!(llvm_ir.contains("%4 = fadd double %2, %3"));
    assert!(llvm_ir.contains("%5 = fdiv double %4, %3"));
    
    // Double-precision conversions
    assert!(llvm_ir.contains("%6 = fptrunc double %5 to float"));
    assert!(llvm_ir.contains("%7 = fptosi double %5 to i32"));
    
    // Double-precision store
    assert!(llvm_ir.contains("%fsd.addr = add i32 %1, 24"));
    assert!(llvm_ir.contains("%fsd.ptr = inttoptr i32 %fsd.addr to double*"));
    assert!(llvm_ir.contains("store double %5, double* %fsd.ptr, align 8"));
}

#[test]
fn test_riscv_c_extension_compressed() {
    let target = Target::riscv32_linux();
    let extensions = RiscVExtensions { i: true, m: false, a: false, f: false, d: false, c: true, v: false };
    let mut codegen = CodeGenerator::new("riscv_c_test".to_string());
    
    let test_module = Module {
        name: "riscv_c_test".to_string(),
        target,
        functions: vec![Function {
            name: "test_compressed".to_string(),
            params: vec!["base".to_string()],
            blocks: vec![BasicBlock {
                label: "entry".to_string(),
                instructions: vec![
                    // C.LI r2, 42 (compressed load immediate)
                    Instruction::RiscV(RiscVInstruction::CLi {
                        dest: 2,
                        imm: 42,
                    }),
                    // C.ADDI r2, 10 (compressed add immediate)
                    Instruction::RiscV(RiscVInstruction::CAddi {
                        dest: 2,
                        imm: 10,
                    }),
                    // C.LW r3, 4(r1) (compressed load word)
                    Instruction::RiscV(RiscVInstruction::CLw {
                        dest: 3,
                        base: Value::Register(1),
                        offset: 4,
                    }),
                    // C.ADD r2, r3 (compressed add)
                    Instruction::RiscV(RiscVInstruction::CAdd {
                        dest: 2,
                        src: Value::Register(3),
                    }),
                    // C.MV r4, r2 (compressed move)
                    Instruction::RiscV(RiscVInstruction::CMv {
                        dest: 4,
                        src: Value::Register(2),
                    }),
                    // C.SW r4, 8(r1) (compressed store word)
                    Instruction::RiscV(RiscVInstruction::CSw {
                        src: Value::Register(4),
                        base: Value::Register(1),
                        offset: 8,
                    }),
                    Instruction::Return { value: Some(Value::Register(4)) },
                ],
            }],
        }],
    };
    
    let llvm_ir = codegen.generate_llvm_ir(&test_module).expect("Failed to generate LLVM IR");
    
    // Compressed instructions should generate equivalent LLVM IR to uncompressed
    assert!(llvm_ir.contains("%2 = add i32 0, 42")); // C.LI
    assert!(llvm_ir.contains("%2 = add i32 %2, 10")); // C.ADDI
    
    // Compressed load
    assert!(llvm_ir.contains("%3.addr = add i32 %1, 4"));
    assert!(llvm_ir.contains("%3.ptr = inttoptr i32 %3.addr to i32*"));
    assert!(llvm_ir.contains("%3 = load i32, i32* %3.ptr, align 4"));
    
    // Compressed arithmetic
    assert!(llvm_ir.contains("%2 = add i32 %2, %3")); // C.ADD
    assert!(llvm_ir.contains("%4 = add i32 0, %2")); // C.MV
    
    // Compressed store
    assert!(llvm_ir.contains("%csw.addr = add i32 %1, 8"));
    assert!(llvm_ir.contains("%csw.ptr = inttoptr i32 %csw.addr to i32*"));
    assert!(llvm_ir.contains("store i32 %4, i32* %csw.ptr, align 4"));
}

#[test]
fn test_comprehensive_riscv_extensions() {
    let target = Target::riscv64_linux();
    let extensions = RiscVExtensions::rv64gc(); // Full general-purpose ISA (IMAFDC)
    let mut codegen = CodeGenerator::new("comprehensive_ext".to_string());
    
    // Create a comprehensive test that uses all extension types
    let test_module = Module {
        name: "comprehensive_ext".to_string(),
        target,
        functions: vec![Function {
            name: "test_all_extensions".to_string(),
            params: vec!["data_ptr".to_string(), "float_val".to_string()],
            blocks: vec![BasicBlock {
                label: "entry".to_string(),
                instructions: vec![
                    // M extension: multiplication
                    Instruction::RiscV(RiscVInstruction::Mul {
                        dest: 3,
                        src1: Value::Register(1),
                        src2: Value::Integer(2),
                    }),
                    // A extension: atomic add
                    Instruction::RiscV(RiscVInstruction::AmoaddW {
                        dest: 4,
                        addr: Value::Register(1),
                        src: Value::Register(3),
                        aq: true,
                        rl: false,
                    }),
                    // F extension: float arithmetic  
                    Instruction::RiscV(RiscVInstruction::FaddS {
                        dest: 5,
                        src1: Value::Register(2),
                        src2: Value::Float(3.14),
                        rm: 0,
                    }),
                    // D extension: double conversion
                    Instruction::RiscV(RiscVInstruction::FcvtDS {
                        dest: 6,
                        src: Value::Register(5),
                        rm: 0,
                    }),
                    // C extension: compressed load
                    Instruction::RiscV(RiscVInstruction::CLw {
                        dest: 7,
                        base: Value::Register(1),
                        offset: 12,
                    }),
                    Instruction::Return { value: Some(Value::Register(7)) },
                ],
            }],
        }],
    };
    
    let llvm_ir = codegen.generate_llvm_ir(&test_module).expect("Failed to generate LLVM IR");
    
    // Verify all extensions are working together
    assert!(llvm_ir.contains("target triple = \"riscv64-unknown-linux-gnu\""));
    assert!(llvm_ir.contains("%3 = mul i32 %1, 2")); // M extension
    assert!(llvm_ir.contains("%4 = atomicrmw add i32*")); // A extension
    assert!(llvm_ir.contains("fadd float")); // F extension
    assert!(llvm_ir.contains("fpext float")); // D extension
    assert!(llvm_ir.contains("%7.addr = add i32 %1, 12")); // C extension
    
    println!("✓ All RISC-V standard extensions (MAFDC) working correctly");
}