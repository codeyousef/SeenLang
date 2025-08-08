//! RISC-V base ISA (RV32I/RV64I) code generation tests

use seen_ir::ir::{Target, Architecture, RiscVExtensions, RiscVInstruction, Value, Module, Function, BasicBlock, Instruction};
use seen_ir::codegen::CodeGenerator;

#[test]
fn test_riscv_arithmetic_instructions() {
    let target = Target::riscv32_linux();
    let extensions = RiscVExtensions::rv32i();
    let mut codegen = CodeGenerator::new("riscv_arith_test".to_string());
    
    let test_module = Module {
        name: "riscv_arith_test".to_string(),
        target,
        functions: vec![Function {
            name: "test_arithmetic".to_string(),
            params: vec!["a".to_string(), "b".to_string()],
            blocks: vec![BasicBlock {
                label: "entry".to_string(),
                instructions: vec![
                    // ADD r3, r1, r2
                    Instruction::RiscV(RiscVInstruction::Add {
                        dest: 3,
                        src1: Value::Register(1),
                        src2: Value::Register(2),
                    }),
                    // ADDI r4, r1, 42
                    Instruction::RiscV(RiscVInstruction::Addi {
                        dest: 4,
                        src: Value::Register(1),
                        imm: 42,
                    }),
                    // SUB r5, r3, r4
                    Instruction::RiscV(RiscVInstruction::Sub {
                        dest: 5,
                        src1: Value::Register(3),
                        src2: Value::Register(4),
                    }),
                    Instruction::Return { value: Some(Value::Register(5)) },
                ],
            }],
        }],
    };
    
    let llvm_ir = codegen.generate_llvm_ir(&test_module).expect("Failed to generate LLVM IR");
    
    // Verify target triple and basic structure
    assert!(llvm_ir.contains("target triple = \"riscv32-unknown-linux-gnu\""));
    assert!(llvm_ir.contains("define internal i32 @test_arithmetic(i32 %a, i32 %b)"));
    
    // Verify RISC-V arithmetic instructions are correctly translated
    assert!(llvm_ir.contains("%3 = add i32 %1, %2"));
    assert!(llvm_ir.contains("%4 = add i32 %1, 42"));
    assert!(llvm_ir.contains("%5 = sub i32 %3, %4"));
    assert!(llvm_ir.contains("ret i32 %5"));
}

#[test]
fn test_riscv_logical_instructions() {
    let target = Target::riscv64_linux();
    let extensions = RiscVExtensions::rv64i();
    let mut codegen = CodeGenerator::new("riscv_logic_test".to_string());
    
    let test_module = Module {
        name: "riscv_logic_test".to_string(),
        target,
        functions: vec![Function {
            name: "test_logical".to_string(),
            params: vec!["x".to_string(), "y".to_string()],
            blocks: vec![BasicBlock {
                label: "entry".to_string(),
                instructions: vec![
                    // AND r3, r1, r2
                    Instruction::RiscV(RiscVInstruction::And {
                        dest: 3,
                        src1: Value::Register(1),
                        src2: Value::Register(2),
                    }),
                    // OR r4, r1, r2
                    Instruction::RiscV(RiscVInstruction::Or {
                        dest: 4,
                        src1: Value::Register(1),
                        src2: Value::Register(2),
                    }),
                    // XOR r5, r3, r4
                    Instruction::RiscV(RiscVInstruction::Xor {
                        dest: 5,
                        src1: Value::Register(3),
                        src2: Value::Register(4),
                    }),
                    // ANDI r6, r5, 255
                    Instruction::RiscV(RiscVInstruction::Andi {
                        dest: 6,
                        src: Value::Register(5),
                        imm: 255,
                    }),
                    Instruction::Return { value: Some(Value::Register(6)) },
                ],
            }],
        }],
    };
    
    let llvm_ir = codegen.generate_llvm_ir(&test_module).expect("Failed to generate LLVM IR");
    
    assert!(llvm_ir.contains("target triple = \"riscv64-unknown-linux-gnu\""));
    assert!(llvm_ir.contains("%3 = and i32 %1, %2"));
    assert!(llvm_ir.contains("%4 = or i32 %1, %2"));
    assert!(llvm_ir.contains("%5 = xor i32 %3, %4"));
    assert!(llvm_ir.contains("%6 = and i32 %5, 255"));
}

#[test]
fn test_riscv_shift_instructions() {
    let target = Target::riscv32_linux();
    let extensions = RiscVExtensions::rv32i();
    let mut codegen = CodeGenerator::new("riscv_shift_test".to_string());
    
    let test_module = Module {
        name: "riscv_shift_test".to_string(),
        target,
        functions: vec![Function {
            name: "test_shifts".to_string(),
            params: vec!["value".to_string(), "amount".to_string()],
            blocks: vec![BasicBlock {
                label: "entry".to_string(),
                instructions: vec![
                    // SLL r3, r1, r2 (shift left logical)
                    Instruction::RiscV(RiscVInstruction::Sll {
                        dest: 3,
                        src1: Value::Register(1),
                        src2: Value::Register(2),
                    }),
                    // SLLI r4, r1, 4 (shift left logical immediate)
                    Instruction::RiscV(RiscVInstruction::Slli {
                        dest: 4,
                        src: Value::Register(1),
                        shamt: 4,
                    }),
                    // SRL r5, r3, r2 (shift right logical)
                    Instruction::RiscV(RiscVInstruction::Srl {
                        dest: 5,
                        src1: Value::Register(3),
                        src2: Value::Register(2),
                    }),
                    // SRA r6, r4, r2 (shift right arithmetic)
                    Instruction::RiscV(RiscVInstruction::Sra {
                        dest: 6,
                        src1: Value::Register(4),
                        src2: Value::Register(2),
                    }),
                    Instruction::Return { value: Some(Value::Register(6)) },
                ],
            }],
        }],
    };
    
    let llvm_ir = codegen.generate_llvm_ir(&test_module).expect("Failed to generate LLVM IR");
    
    assert!(llvm_ir.contains("%3 = shl i32 %1, %2"));
    assert!(llvm_ir.contains("%4 = shl i32 %1, 4"));
    assert!(llvm_ir.contains("%5 = lshr i32 %3, %2"));
    assert!(llvm_ir.contains("%6 = ashr i32 %4, %2"));
}

#[test]
fn test_riscv_comparison_instructions() {
    let target = Target::riscv64_linux();
    let extensions = RiscVExtensions::rv64i();
    let mut codegen = CodeGenerator::new("riscv_cmp_test".to_string());
    
    let test_module = Module {
        name: "riscv_cmp_test".to_string(),
        target,
        functions: vec![Function {
            name: "test_comparisons".to_string(),
            params: vec!["a".to_string(), "b".to_string()],
            blocks: vec![BasicBlock {
                label: "entry".to_string(),
                instructions: vec![
                    // SLT r3, r1, r2 (set less than)
                    Instruction::RiscV(RiscVInstruction::Slt {
                        dest: 3,
                        src1: Value::Register(1),
                        src2: Value::Register(2),
                    }),
                    // SLTU r4, r1, r2 (set less than unsigned)
                    Instruction::RiscV(RiscVInstruction::Sltu {
                        dest: 4,
                        src1: Value::Register(1),
                        src2: Value::Register(2),
                    }),
                    // SLTI r5, r1, 100 (set less than immediate)
                    Instruction::RiscV(RiscVInstruction::Slti {
                        dest: 5,
                        src: Value::Register(1),
                        imm: 100,
                    }),
                    Instruction::Return { value: Some(Value::Register(5)) },
                ],
            }],
        }],
    };
    
    let llvm_ir = codegen.generate_llvm_ir(&test_module).expect("Failed to generate LLVM IR");
    
    assert!(llvm_ir.contains("%3 = icmp slt i32 %1, %2"));
    assert!(llvm_ir.contains("%4 = icmp ult i32 %1, %2"));
    assert!(llvm_ir.contains("%5 = icmp slt i32 %1, 100"));
}

#[test]
fn test_riscv_upper_immediate_instructions() {
    let target = Target::riscv32_linux();
    let extensions = RiscVExtensions::rv32i();
    let mut codegen = CodeGenerator::new("riscv_ui_test".to_string());
    
    let test_module = Module {
        name: "riscv_ui_test".to_string(),
        target,
        functions: vec![Function {
            name: "test_upper_immediate".to_string(),
            params: vec![],
            blocks: vec![BasicBlock {
                label: "entry".to_string(),
                instructions: vec![
                    // LUI r3, 0x12345 (load upper immediate)
                    Instruction::RiscV(RiscVInstruction::Lui {
                        dest: 3,
                        imm: 0x12345,
                    }),
                    // AUIPC r4, 0x1000 (add upper immediate to PC)
                    Instruction::RiscV(RiscVInstruction::Auipc {
                        dest: 4,
                        imm: 0x1000,
                    }),
                    Instruction::Return { value: Some(Value::Register(3)) },
                ],
            }],
        }],
    };
    
    let llvm_ir = codegen.generate_llvm_ir(&test_module).expect("Failed to generate LLVM IR");
    
    // LUI: Load 20-bit immediate to upper 20 bits (0x12345 << 12)
    let expected_lui = (0x12345u32 << 12) as i32;
    assert!(llvm_ir.contains(&format!("%3 = add i32 0, {}", expected_lui)));
    
    // AUIPC: Add upper immediate to PC
    let expected_auipc = (0x1000u32 << 12) as i32;
    assert!(llvm_ir.contains(&format!("add i32 ptrtoint (i8* blockaddress(@main, %entry) to i32), {}", expected_auipc)));
}

#[test]
fn test_riscv_memory_instructions() {
    let target = Target::riscv32_linux();
    let extensions = RiscVExtensions::rv32i();
    let mut codegen = CodeGenerator::new("riscv_mem_test".to_string());
    
    let test_module = Module {
        name: "riscv_mem_test".to_string(),
        target,
        functions: vec![Function {
            name: "test_memory".to_string(),
            params: vec!["base_addr".to_string()],
            blocks: vec![BasicBlock {
                label: "entry".to_string(),
                instructions: vec![
                    // LW r2, 8(r1) (load word)
                    Instruction::RiscV(RiscVInstruction::Lw {
                        dest: 2,
                        base: Value::Register(1),
                        offset: 8,
                    }),
                    // SW r2, 12(r1) (store word)
                    Instruction::RiscV(RiscVInstruction::Sw {
                        src: Value::Register(2),
                        base: Value::Register(1),
                        offset: 12,
                    }),
                    // LB r3, 0(r1) (load byte)
                    Instruction::RiscV(RiscVInstruction::Lb {
                        dest: 3,
                        base: Value::Register(1),
                        offset: 0,
                    }),
                    // SB r3, 1(r1) (store byte)
                    Instruction::RiscV(RiscVInstruction::Sb {
                        src: Value::Register(3),
                        base: Value::Register(1),
                        offset: 1,
                    }),
                    Instruction::Return { value: Some(Value::Register(2)) },
                ],
            }],
        }],
    };
    
    let llvm_ir = codegen.generate_llvm_ir(&test_module).expect("Failed to generate LLVM IR");
    
    // Load word: LW r2, 8(r1)
    assert!(llvm_ir.contains("%2.addr = add i32 %1, 8"));
    assert!(llvm_ir.contains("%2.ptr = inttoptr i32 %2.addr to i32*"));
    assert!(llvm_ir.contains("%2 = load i32, i32* %2.ptr, align 4"));
    
    // Store word: SW r2, 12(r1)
    assert!(llvm_ir.contains("%sw.addr = add i32 %1, 12"));
    assert!(llvm_ir.contains("%sw.ptr = inttoptr i32 %sw.addr to i32*"));
    assert!(llvm_ir.contains("store i32 %2, i32* %sw.ptr, align 4"));
    
    // Load byte: LB r3, 0(r1)
    assert!(llvm_ir.contains("%3.addr = add i32 %1, 0"));
    assert!(llvm_ir.contains("%3.ptr = inttoptr i32 %3.addr to i8*"));
    assert!(llvm_ir.contains("%3.byte = load i8, i8* %3.ptr, align 1"));
    assert!(llvm_ir.contains("%3 = sext i8 %3.byte to i32"));
    
    // Store byte: SB r3, 1(r1)
    assert!(llvm_ir.contains("%sb.addr = add i32 %1, 1"));
    assert!(llvm_ir.contains("%sb.ptr = inttoptr i32 %sb.addr to i8*"));
    assert!(llvm_ir.contains("%sb.byte = trunc i32 %3 to i8"));
    assert!(llvm_ir.contains("store i8 %sb.byte, i8* %sb.ptr, align 1"));
}

#[test]
fn test_riscv64_specific_instructions() {
    let target = Target::riscv64_linux();
    let extensions = RiscVExtensions::rv64i();
    let mut codegen = CodeGenerator::new("riscv64_test".to_string());
    
    let test_module = Module {
        name: "riscv64_test".to_string(),
        target,
        functions: vec![Function {
            name: "test_rv64i".to_string(),
            params: vec!["a".to_string(), "b".to_string()],
            blocks: vec![BasicBlock {
                label: "entry".to_string(),
                instructions: vec![
                    // ADDW r3, r1, r2 (32-bit add with sign extension)
                    Instruction::RiscV(RiscVInstruction::Addw {
                        dest: 3,
                        src1: Value::Register(1),
                        src2: Value::Register(2),
                    }),
                    // ADDIW r4, r1, 100 (32-bit add immediate with sign extension)
                    Instruction::RiscV(RiscVInstruction::Addiw {
                        dest: 4,
                        src: Value::Register(1),
                        imm: 100,
                    }),
                    // LD r5, 16(r1) (load doubleword)
                    Instruction::RiscV(RiscVInstruction::Ld {
                        dest: 5,
                        base: Value::Register(1),
                        offset: 16,
                    }),
                    // SD r5, 24(r1) (store doubleword)
                    Instruction::RiscV(RiscVInstruction::Sd {
                        src: Value::Register(5),
                        base: Value::Register(1),
                        offset: 24,
                    }),
                    Instruction::Return { value: Some(Value::Register(3)) },
                ],
            }],
        }],
    };
    
    let llvm_ir = codegen.generate_llvm_ir(&test_module).expect("Failed to generate LLVM IR");
    
    // ADDW: 32-bit add with sign extension to 64-bit
    assert!(llvm_ir.contains("%3.tmp = add i32 %1, %2"));
    assert!(llvm_ir.contains("%3 = sext i32 %3.tmp to i64"));
    
    // ADDIW: 32-bit add immediate with sign extension
    assert!(llvm_ir.contains("%4.tmp = add i32 %1, 100"));
    assert!(llvm_ir.contains("%4 = sext i32 %4.tmp to i64"));
    
    // LD: Load doubleword
    assert!(llvm_ir.contains("%5.addr = add i64 %1, 16"));
    assert!(llvm_ir.contains("%5.ptr = inttoptr i64 %5.addr to i64*"));
    assert!(llvm_ir.contains("%5 = load i64, i64* %5.ptr, align 8"));
    
    // SD: Store doubleword
    assert!(llvm_ir.contains("%sd.addr = add i64 %1, 24"));
    assert!(llvm_ir.contains("%sd.ptr = inttoptr i64 %sd.addr to i64*"));
    assert!(llvm_ir.contains("store i64 %5, i64* %sd.ptr, align 8"));
}

#[test]
fn test_riscv_branch_instructions() {
    let target = Target::riscv32_linux();
    let extensions = RiscVExtensions::rv32i();
    let mut codegen = CodeGenerator::new("riscv_branch_test".to_string());
    
    let test_module = Module {
        name: "riscv_branch_test".to_string(),
        target,
        functions: vec![Function {
            name: "test_branches".to_string(),
            params: vec!["a".to_string(), "b".to_string()],
            blocks: vec![BasicBlock {
                label: "entry".to_string(),
                instructions: vec![
                    // BEQ r1, r2, equal_label
                    Instruction::RiscV(RiscVInstruction::Beq {
                        src1: Value::Register(1),
                        src2: Value::Register(2),
                        label: "equal_label".to_string(),
                    }),
                    // BNE r1, r2, not_equal_label
                    Instruction::RiscV(RiscVInstruction::Bne {
                        src1: Value::Register(1),
                        src2: Value::Register(2),
                        label: "not_equal_label".to_string(),
                    }),
                    // BLT r1, r2, less_than_label
                    Instruction::RiscV(RiscVInstruction::Blt {
                        src1: Value::Register(1),
                        src2: Value::Register(2),
                        label: "less_than_label".to_string(),
                    }),
                    Instruction::Return { value: Some(Value::Integer(0)) },
                ],
            }],
        }],
    };
    
    let llvm_ir = codegen.generate_llvm_ir(&test_module).expect("Failed to generate LLVM IR");
    
    // BEQ: Branch if equal
    assert!(llvm_ir.contains("%eq.cond = icmp eq i32 %1, %2"));
    assert!(llvm_ir.contains("br i1 %eq.cond, label %equal_label, label %next"));
    
    // BNE: Branch if not equal
    assert!(llvm_ir.contains("%ne.cond = icmp ne i32 %1, %2"));
    assert!(llvm_ir.contains("br i1 %ne.cond, label %not_equal_label, label %next"));
    
    // BLT: Branch if less than
    assert!(llvm_ir.contains("%lt.cond = icmp slt i32 %1, %2"));
    assert!(llvm_ir.contains("br i1 %lt.cond, label %less_than_label, label %next"));
}

#[test]
fn test_riscv_jump_instructions() {
    let target = Target::riscv64_linux();
    let extensions = RiscVExtensions::rv64i();
    let mut codegen = CodeGenerator::new("riscv_jump_test".to_string());
    
    let test_module = Module {
        name: "riscv_jump_test".to_string(),
        target,
        functions: vec![Function {
            name: "test_jumps".to_string(),
            params: vec!["target_addr".to_string()],
            blocks: vec![BasicBlock {
                label: "entry".to_string(),
                instructions: vec![
                    // JAL r3, function_label (jump and link)
                    Instruction::RiscV(RiscVInstruction::Jal {
                        dest: Some(3),
                        label: "function_label".to_string(),
                    }),
                    // JALR r4, r1, 8 (jump and link register)
                    Instruction::RiscV(RiscVInstruction::Jalr {
                        dest: 4,
                        base: Value::Register(1),
                        offset: 8,
                    }),
                    Instruction::Return { value: Some(Value::Register(4)) },
                ],
            }],
        }],
    };
    
    let llvm_ir = codegen.generate_llvm_ir(&test_module).expect("Failed to generate LLVM IR");
    
    // JAL: Jump and link with return address storage
    assert!(llvm_ir.contains("%3 = ptrtoint i8* blockaddress(@main, %return) to i32"));
    assert!(llvm_ir.contains("br label %function_label"));
    
    // JALR: Jump and link register
    assert!(llvm_ir.contains("%4 = ptrtoint i8* blockaddress(@main, %return) to i32"));
    assert!(llvm_ir.contains("%jalr.addr = add i32 %1, 8"));
    assert!(llvm_ir.contains("indirectbr i8* inttoptr (i32 %jalr.addr to i8*), []"));
}

#[test]
fn test_riscv_system_instructions() {
    let target = Target::riscv32_bare_metal();
    let extensions = RiscVExtensions::rv32i();
    let mut codegen = CodeGenerator::new("riscv_sys_test".to_string());
    
    let test_module = Module {
        name: "riscv_sys_test".to_string(),
        target,
        functions: vec![Function {
            name: "test_system".to_string(),
            params: vec![],
            blocks: vec![BasicBlock {
                label: "entry".to_string(),
                instructions: vec![
                    // ECALL (environment call)
                    Instruction::RiscV(RiscVInstruction::Ecall),
                    // EBREAK (environment break)
                    Instruction::RiscV(RiscVInstruction::Ebreak),
                    // FENCE (memory fence)
                    Instruction::RiscV(RiscVInstruction::Fence { pred: 0xf, succ: 0xf }),
                    // FENCE.I (instruction fence)
                    Instruction::RiscV(RiscVInstruction::FenceI),
                    Instruction::Return { value: Some(Value::Integer(0)) },
                ],
            }],
        }],
    };
    
    let llvm_ir = codegen.generate_llvm_ir(&test_module).expect("Failed to generate LLVM IR");
    
    assert!(llvm_ir.contains("call void @__riscv_ecall()"));
    assert!(llvm_ir.contains("call void @llvm.debugtrap()"));
    assert!(llvm_ir.contains("fence seq_cst, seq_cst"));
    assert!(llvm_ir.contains("call void @llvm.instruction.fence()"));
}

#[test]
fn test_riscv_csr_instructions() {
    let target = Target::riscv32_bare_metal();
    let extensions = RiscVExtensions::rv32i();
    let mut codegen = CodeGenerator::new("riscv_csr_test".to_string());
    
    let test_module = Module {
        name: "riscv_csr_test".to_string(),
        target,
        functions: vec![Function {
            name: "test_csr".to_string(),
            params: vec!["value".to_string()],
            blocks: vec![BasicBlock {
                label: "entry".to_string(),
                instructions: vec![
                    // CSRRW r2, mstatus, r1 (CSR read/write)
                    Instruction::RiscV(RiscVInstruction::Csrrw {
                        dest: 2,
                        csr: 0x300, // mstatus CSR
                        src: Value::Register(1),
                    }),
                    // CSRRS r3, mie, r1 (CSR read and set)
                    Instruction::RiscV(RiscVInstruction::Csrrs {
                        dest: 3,
                        csr: 0x304, // mie CSR
                        src: Value::Register(1),
                    }),
                    // CSRRWI r4, mscratch, 0x1f (CSR read/write immediate)
                    Instruction::RiscV(RiscVInstruction::Csrrwi {
                        dest: 4,
                        csr: 0x340, // mscratch CSR
                        imm: 0x1f,
                    }),
                    Instruction::Return { value: Some(Value::Register(2)) },
                ],
            }],
        }],
    };
    
    let llvm_ir = codegen.generate_llvm_ir(&test_module).expect("Failed to generate LLVM IR");
    
    assert!(llvm_ir.contains("%2 = call i32 @__riscv_csrrw(i32 768, i32 %1)"));
    assert!(llvm_ir.contains("%3 = call i32 @__riscv_csrrs(i32 772, i32 %1)"));
    assert!(llvm_ir.contains("%4 = call i32 @__riscv_csrrw(i32 832, i32 31)"));
}

#[test]
fn test_comprehensive_riscv_program() {
    let target = Target::riscv64_linux();
    let extensions = RiscVExtensions::rv64i();
    let mut codegen = CodeGenerator::new("comprehensive_riscv".to_string());
    
    // Create a comprehensive RISC-V program that uses multiple instruction types
    let test_module = Module {
        name: "comprehensive_riscv".to_string(),
        target,
        functions: vec![Function {
            name: "factorial".to_string(),
            params: vec!["n".to_string()],
            blocks: vec![
                BasicBlock {
                    label: "entry".to_string(),
                    instructions: vec![
                        // Check if n <= 1
                        Instruction::RiscV(RiscVInstruction::Slti {
                            dest: 2,
                            src: Value::Register(1),
                            imm: 2,
                        }),
                        // Branch to base_case if n <= 1
                        Instruction::RiscV(RiscVInstruction::Bne {
                            src1: Value::Register(2),
                            src2: Value::Integer(0),
                            label: "base_case".to_string(),
                        }),
                        // n > 1, compute n * factorial(n-1)
                        Instruction::RiscV(RiscVInstruction::Addi {
                            dest: 3,
                            src: Value::Register(1),
                            imm: -1,
                        }),
                        Instruction::RiscV(RiscVInstruction::Jal {
                            dest: Some(4),
                            label: "factorial_recursive".to_string(),
                        }),
                    ],
                },
                BasicBlock {
                    label: "base_case".to_string(),
                    instructions: vec![
                        // Return 1 for base case
                        Instruction::RiscV(RiscVInstruction::Addi {
                            dest: 5,
                            src: Value::Integer(0),
                            imm: 1,
                        }),
                        Instruction::Return { value: Some(Value::Register(5)) },
                    ],
                },
            ],
        }],
    };
    
    let llvm_ir = codegen.generate_llvm_ir(&test_module).expect("Failed to generate LLVM IR");
    
    // Verify the comprehensive program compiles correctly
    assert!(llvm_ir.contains("target triple = \"riscv64-unknown-linux-gnu\""));
    assert!(llvm_ir.contains("define internal i32 @factorial(i32 %n)"));
    assert!(llvm_ir.contains("entry:"));
    assert!(llvm_ir.contains("base_case:"));
    
    // Verify RISC-V instruction usage
    assert!(llvm_ir.contains("%2 = icmp slt i32 %1, 2")); // SLTI
    assert!(llvm_ir.contains("br i1 %ne.cond, label %base_case")); // BNE
    assert!(llvm_ir.contains("%3 = add i32 %1, -1")); // ADDI
    assert!(llvm_ir.contains("br label %factorial_recursive")); // JAL
    assert!(llvm_ir.contains("%5 = add i32 0, 1")); // ADDI for constant 1
}