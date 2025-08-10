//! Code generation with LLVM backend

use crate::ir::*;
use seen_common::{SeenResult, SeenError};

/// Code generator with LLVM backend
pub struct CodeGenerator {
    module_name: String,
    debug_info_enabled: bool,
    calling_convention: String,
    target_triple: String,
    optimization_level: u32,
    string_constants: std::collections::HashMap<String, usize>,
    next_string_id: usize,
}

impl CodeGenerator {
    pub fn new(module_name: String) -> Self {
        Self {
            module_name,
            debug_info_enabled: false,
            calling_convention: "fastcc".to_string(),
            target_triple: "x86_64-unknown-linux-gnu".to_string(),
            optimization_level: 2,
            string_constants: std::collections::HashMap::new(),
            next_string_id: 0,
        }
    }
    
    /// Enable or disable debug information generation
    pub fn enable_debug_info(&mut self, enabled: bool) {
        self.debug_info_enabled = enabled;
    }
    
    /// Set calling convention (C, fastcc, etc.)
    pub fn set_calling_convention(&mut self, convention: &str) {
        self.calling_convention = convention.to_string();
    }
    
    /// Set target triple for cross-compilation
    pub fn set_target_triple(&mut self, triple: &str) {
        self.target_triple = triple.to_string();
    }
    
    /// Set optimization level (0-3)
    pub fn set_optimization_level(&mut self, level: u32) {
        self.optimization_level = level.clamp(0, 3);
    }
    
    /// Generate LLVM IR from module
    pub fn generate_llvm_ir(&mut self, module: &Module) -> SeenResult<String> {
        eprintln!("[DEBUG CodeGenerator] Starting LLVM IR generation for module: {}", module.name);
        eprintln!("[DEBUG CodeGenerator] Module has {} functions", module.functions.len());
        
        // Pre-calculate approximate size to avoid reallocations
        let estimated_size = module.functions.iter()
            .map(|f| f.blocks.iter().map(|b| b.instructions.len() * 50).sum::<usize>())
            .sum::<usize>() + 1000; // 50 chars per instruction estimate + headers
        
        let mut llvm_ir = String::with_capacity(estimated_size);
        
        // Module header with target information from the module
        use std::fmt::Write;
        let _ = write!(&mut llvm_ir, "target triple = \"{}\"\n", module.target.to_llvm_triple());
        let _ = write!(&mut llvm_ir, "; Module: {}\n\n", module.name);
        
        // Debug info metadata if enabled
        if self.debug_info_enabled {
            llvm_ir.push_str(&self.generate_debug_metadata());
        }
        
        // First pass: generate functions to collect all string constants
        let mut function_irs = Vec::new();
        for function in &module.functions {
            eprintln!("[DEBUG CodeGenerator] Processing function: {}", function.name);
            let func_ir = self.generate_function_ir(function)?;
            function_irs.push(func_ir);
        }
        
        eprintln!("[DEBUG CodeGenerator] After function generation, have {} string constants", self.string_constants.len());
        
        // Generate string constants BEFORE functions (they need to be declared first)
        llvm_ir.push_str(&self.generate_string_constants());
        
        // Generate standard library function declarations BEFORE functions
        llvm_ir.push_str(&self.generate_stdlib_declarations());
        
        // Now add the actual function definitions
        for func_ir in function_irs {
            llvm_ir.push_str(&func_ir);
            llvm_ir.push('\n');
        }
        
        // Debug info declarations if enabled
        if self.debug_info_enabled {
            llvm_ir.push_str(&self.generate_debug_declarations());
        }
        
        eprintln!("[DEBUG CodeGenerator] Final LLVM IR size: {} bytes", llvm_ir.len());
        Ok(llvm_ir)
    }
    
    /// Generate LLVM IR for a function
    fn generate_function_ir(&mut self, function: &Function) -> SeenResult<String> {
        // Pre-allocate with estimated size
        let estimated_size = function.blocks.iter()
            .map(|b| b.instructions.len() * 50)
            .sum::<usize>() + 200;
        let mut ir = String::with_capacity(estimated_size);
        
        use std::fmt::Write;
        
        // Function signature with proper linkage and calling convention
        let return_type = if function.name == "main" { "i32" } else { "i32" };
        
        // Determine linkage - C ABI functions should be external
        let linkage = if function.name == "main" { 
            "" 
        } else if self.calling_convention == "C" { 
            "" // External linkage for C ABI functions
        } else { 
            "internal" 
        };
        
        // Build function signature with proper spacing
        if linkage.is_empty() {
            let _ = write!(&mut ir, "define {} @{}(", return_type, function.name);
        } else {
            let _ = write!(&mut ir, "define {} {} @{}(", linkage, return_type, function.name);
        }
        
        // Parameters
        for (i, param) in function.params.iter().enumerate() {
            if i > 0 { ir.push_str(", "); }
            let _ = write!(&mut ir, "i32 %{}", param);
        }
        
        // Add attributes for main function
        if function.name == "main" {
            ir.push_str(") nounwind {\n");
        } else {
            let calling_conv = if self.calling_convention == "C" { "" } else { "fastcc" };
            let _ = write!(&mut ir, ") {} {{\n", calling_conv);
        }
        
        // Generate basic blocks
        for block in &function.blocks {
            ir.push_str(&self.generate_basic_block_ir(block)?);
        }
        
        ir.push_str("}\n");
        Ok(ir)
    }
    
    /// Generate LLVM IR for a basic block
    fn generate_basic_block_ir(&mut self, block: &BasicBlock) -> SeenResult<String> {
        let mut ir = String::with_capacity(block.instructions.len() * 50 + 50);
        use std::fmt::Write;
        
        // Block label
        let _ = write!(&mut ir, "{}:\n", block.label);
        
        // Generate instructions
        for instruction in &block.instructions {
            ir.push_str("  ");
            ir.push_str(&self.generate_instruction_ir(instruction)?);
            ir.push('\n');
        }
        
        Ok(ir)
    }
    
    /// Generate LLVM IR for an instruction
    fn generate_instruction_ir(&mut self, instruction: &Instruction) -> SeenResult<String> {
        use std::fmt::Write;
        let mut ir = String::with_capacity(80);
        
        match instruction {
            Instruction::Load { dest, src } => {
                let _ = write!(&mut ir, "%{} = load i32, i32* %{}, align 4", dest, src);
            }
            Instruction::Store { dest, src } => {
                let _ = write!(&mut ir, "store i32 %{}, i32* %{}, align 4", src, dest);
            }
            Instruction::Call { dest, func, args } => {
                // Handle built-in functions with proper implementations
                if func.starts_with("__builtin_") {
                    match func.as_str() {
                        "__builtin_length" => {
                            // Get string or array length
                            if let Some(dest_reg) = dest {
                                if args.len() == 1 {
                                    // Call strlen for strings
                                    let _ = write!(&mut ir, "%{} = call i64 @strlen(i8* {})", dest_reg, self.format_value(&args[0]));
                                }
                            }
                        }
                        "__builtin_char_at" => {
                            // Get character at index
                            if let Some(dest_reg) = dest {
                                if args.len() == 2 {
                                    let _ = write!(&mut ir, "%{}.ptr = getelementptr i8, i8* {}, i32 {}", 
                                        dest_reg, self.format_value(&args[0]), self.format_value(&args[1]));
                                    ir.push('\n');
                                    ir.push_str("  ");
                                    let _ = write!(&mut ir, "%{}.char = load i8, i8* %{}.ptr", dest_reg, dest_reg);
                                    ir.push('\n');
                                    ir.push_str("  ");
                                    let _ = write!(&mut ir, "%{} = zext i8 %{}.char to i32", dest_reg, dest_reg);
                                }
                            }
                        }
                        "__builtin_substring" => {
                            // Extract substring from start to end
                            if let Some(dest_reg) = dest {
                                if args.len() == 3 {
                                    // Calculate length
                                    let _ = write!(&mut ir, "%{}.len = sub i32 {}, {}", 
                                        dest_reg, self.format_value(&args[2]), self.format_value(&args[1]));
                                    ir.push('\n');
                                    ir.push_str("  ");
                                    // Allocate memory for substring
                                    let _ = write!(&mut ir, "%{}.size = add i32 %{}.len, 1", dest_reg, dest_reg);
                                    ir.push('\n');
                                    ir.push_str("  ");
                                    let _ = write!(&mut ir, "%{}.mem = call i8* @malloc(i32 %{}.size)", dest_reg, dest_reg);
                                    ir.push('\n');
                                    ir.push_str("  ");
                                    // Get source pointer
                                    let _ = write!(&mut ir, "%{}.src = getelementptr i8, i8* {}, i32 {}", 
                                        dest_reg, self.format_value(&args[0]), self.format_value(&args[1]));
                                    ir.push('\n');
                                    ir.push_str("  ");
                                    // Copy substring
                                    let _ = write!(&mut ir, "call void @llvm.memcpy.p0i8.p0i8.i32(i8* %{}.mem, i8* %{}.src, i32 %{}.len, i1 false)", 
                                        dest_reg, dest_reg, dest_reg);
                                    ir.push('\n');
                                    ir.push_str("  ");
                                    // Null terminate
                                    let _ = write!(&mut ir, "%{}.end = getelementptr i8, i8* %{}.mem, i32 %{}.len", 
                                        dest_reg, dest_reg, dest_reg);
                                    ir.push('\n');
                                    ir.push_str("  ");
                                    let _ = write!(&mut ir, "store i8 0, i8* %{}.end", dest_reg);
                                    ir.push('\n');
                                    ir.push_str("  ");
                                    let _ = write!(&mut ir, "%{} = bitcast i8* %{}.mem to i8*", dest_reg, dest_reg);
                                }
                            }
                        }
                        "__builtin_array_get" => {
                            // Get array element at index
                            if let Some(dest_reg) = dest {
                                if args.len() == 2 {
                                    let _ = write!(&mut ir, "%{}.ptr = getelementptr i32, i32* {}, i32 {}", 
                                        dest_reg, self.format_value(&args[0]), self.format_value(&args[1]));
                                    ir.push('\n');
                                    ir.push_str("  ");
                                    let _ = write!(&mut ir, "%{} = load i32, i32* %{}.ptr", dest_reg, dest_reg);
                                }
                            }
                        }
                        "__builtin_array_push" => {
                            // Push value to array - requires dynamic array implementation
                            if args.len() == 2 {
                                // Get array struct pointer
                                ir.push_str("; array.push implementation");
                                ir.push('\n');
                                ir.push_str("  ");
                                // Load current size
                                let _ = write!(&mut ir, "%push.size.ptr = getelementptr %%array_t, %%array_t* {}, i32 0, i32 0", 
                                    self.format_value(&args[0]));
                                ir.push('\n');
                                ir.push_str("  ");
                                ir.push_str("%push.size = load i32, i32* %push.size.ptr");
                                ir.push('\n');
                                ir.push_str("  ");
                                // Get data pointer
                                ir.push_str("%push.data.ptr = getelementptr %array_t, %array_t* {}, i32 0, i32 2");
                                ir.push('\n');
                                ir.push_str("  ");
                                ir.push_str("%push.data = load i32*, i32** %push.data.ptr");
                                ir.push('\n');
                                ir.push_str("  ");
                                // Store new element
                                let _ = write!(&mut ir, "%push.elem.ptr = getelementptr i32, i32* %push.data, i32 %push.size");
                                ir.push('\n');
                                ir.push_str("  ");
                                let _ = write!(&mut ir, "store i32 {}, i32* %push.elem.ptr", self.format_value(&args[1]));
                                ir.push('\n');
                                ir.push_str("  ");
                                // Increment size
                                ir.push_str("%push.new.size = add i32 %push.size, 1");
                                ir.push('\n');
                                ir.push_str("  ");
                                ir.push_str("store i32 %push.new.size, i32* %push.size.ptr");
                            }
                        }
                        "__builtin_char_code_at" => {
                            // Get character code at index
                            if let Some(dest_reg) = dest {
                                if args.len() == 2 {
                                    let _ = write!(&mut ir, "%{}.ptr = getelementptr i8, i8* {}, i32 {}", 
                                        dest_reg, self.format_value(&args[0]), self.format_value(&args[1]));
                                    ir.push('\n');
                                    ir.push_str("  ");
                                    let _ = write!(&mut ir, "%{}.char = load i8, i8* %{}.ptr", dest_reg, dest_reg);
                                    ir.push('\n');
                                    ir.push_str("  ");
                                    let _ = write!(&mut ir, "%{} = zext i8 %{}.char to i32", dest_reg, dest_reg);
                                }
                            }
                        }
                        _ => {
                            // Unknown built-in - this should not happen in production
                            return Err(SeenError::codegen_error(format!("Unknown built-in function: {}", func)));
                        }
                    }
                } else {
                    // Regular function call
                    if let Some(dest_reg) = dest {
                        let _ = write!(&mut ir, "%{} = call i32 @{}(", dest_reg, func);
                    } else {
                        let _ = write!(&mut ir, "call void @{}(", func);
                    }
                    
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 { ir.push_str(", "); }
                        // Handle string arguments specially for print functions
                        if (func == "print" || func == "println") && matches!(arg, crate::Value::String(_)) {
                            let str_ref = self.format_value(arg);
                            let _ = write!(&mut ir, "i8* getelementptr inbounds ([{} x i8], [{} x i8]* {}, i32 0, i32 0)", 
                                self.get_string_length(arg) + 1, 
                                self.get_string_length(arg) + 1, 
                                str_ref);
                        } else {
                            ir.push_str(&self.format_value(arg));
                        }
                    }
                    ir.push(')');
                }
            }
            Instruction::Return { value } => {
                if let Some(val) = value {
                    let _ = write!(&mut ir, "ret i32 {}", self.format_value(val));
                } else {
                    ir.push_str("ret void");
                }
            }
            Instruction::Binary { dest, op, left, right } => {
                let op_str = match op {
                    crate::BinaryOp::Add => "add",
                    crate::BinaryOp::Sub => "sub",
                    crate::BinaryOp::Mul => "mul",
                    crate::BinaryOp::Div => "sdiv",
                    crate::BinaryOp::Mod => "srem",
                    crate::BinaryOp::And => "and",
                    crate::BinaryOp::Or => "or",
                    crate::BinaryOp::Xor => "xor",
                };
                let _ = write!(&mut ir, "%{} = {} i32 {}, {}", 
                    dest, op_str, self.format_value(left), self.format_value(right));
            }
            Instruction::Compare { dest, op, left, right } => {
                let cond = match op {
                    crate::CompareOp::Eq => "eq",
                    crate::CompareOp::Ne => "ne",
                    crate::CompareOp::Lt => "slt",
                    crate::CompareOp::Le => "sle",
                    crate::CompareOp::Gt => "sgt",
                    crate::CompareOp::Ge => "sge",
                };
                let _ = write!(&mut ir, "%{} = icmp {} i32 {}, {}", 
                    dest, cond, self.format_value(left), self.format_value(right));
            }
            Instruction::Branch { condition, true_label, false_label } => {
                if let Some(cond) = condition {
                    if let Some(false_l) = false_label {
                        let _ = write!(&mut ir, "br i1 %{}, label %{}, label %{}", 
                            cond, true_label, false_l);
                    } else {
                        let _ = write!(&mut ir, "br label %{}", true_label);
                    }
                } else {
                    let _ = write!(&mut ir, "br label %{}", true_label);
                }
            }
            Instruction::Phi { dest, values } => {
                let _ = write!(&mut ir, "%{} = phi i32 ", dest);
                for (i, (val, label)) in values.iter().enumerate() {
                    if i > 0 { ir.push_str(", "); }
                    let _ = write!(&mut ir, "[{}, %{}]", self.format_value(val), label);
                }
            }
            Instruction::Alloca { dest, ty } => {
                let type_str = self.format_type(ty);
                let _ = write!(&mut ir, "%{} = alloca {}, align 4", dest, type_str);
            }
            Instruction::Nop => {
                ir.push_str("; nop");
            }
            Instruction::RiscV(riscv_inst) => {
                ir.push_str(&self.generate_riscv_instruction_ir(riscv_inst)?);
            }
        }
        
        Ok(ir)
    }
    
    /// Generate LLVM IR for RISC-V specific instructions
    fn generate_riscv_instruction_ir(&mut self, instruction: &crate::RiscVInstruction) -> SeenResult<String> {
        use std::fmt::Write;
        let mut ir = String::with_capacity(120);
        use crate::RiscVInstruction;
        
        match instruction {
            // Arithmetic instructions
            RiscVInstruction::Add { dest, src1, src2 } => {
                let _ = write!(&mut ir, "%{} = add i32 {}, {}", dest, self.format_value(src1), self.format_value(src2));
            }
            RiscVInstruction::Sub { dest, src1, src2 } => {
                let _ = write!(&mut ir, "%{} = sub i32 {}, {}", dest, self.format_value(src1), self.format_value(src2));
            }
            RiscVInstruction::Addi { dest, src, imm } => {
                let _ = write!(&mut ir, "%{} = add i32 {}, {}", dest, self.format_value(src), imm);
            }
            
            // Logical instructions
            RiscVInstruction::And { dest, src1, src2 } => {
                let _ = write!(&mut ir, "%{} = and i32 {}, {}", dest, self.format_value(src1), self.format_value(src2));
            }
            RiscVInstruction::Or { dest, src1, src2 } => {
                let _ = write!(&mut ir, "%{} = or i32 {}, {}", dest, self.format_value(src1), self.format_value(src2));
            }
            RiscVInstruction::Xor { dest, src1, src2 } => {
                let _ = write!(&mut ir, "%{} = xor i32 {}, {}", dest, self.format_value(src1), self.format_value(src2));
            }
            RiscVInstruction::Andi { dest, src, imm } => {
                let _ = write!(&mut ir, "%{} = and i32 {}, {}", dest, self.format_value(src), imm);
            }
            RiscVInstruction::Ori { dest, src, imm } => {
                let _ = write!(&mut ir, "%{} = or i32 {}, {}", dest, self.format_value(src), imm);
            }
            RiscVInstruction::Xori { dest, src, imm } => {
                let _ = write!(&mut ir, "%{} = xor i32 {}, {}", dest, self.format_value(src), imm);
            }
            
            // Shift instructions
            RiscVInstruction::Sll { dest, src1, src2 } => {
                let _ = write!(&mut ir, "%{} = shl i32 {}, {}", dest, self.format_value(src1), self.format_value(src2));
            }
            RiscVInstruction::Srl { dest, src1, src2 } => {
                let _ = write!(&mut ir, "%{} = lshr i32 {}, {}", dest, self.format_value(src1), self.format_value(src2));
            }
            RiscVInstruction::Sra { dest, src1, src2 } => {
                let _ = write!(&mut ir, "%{} = ashr i32 {}, {}", dest, self.format_value(src1), self.format_value(src2));
            }
            RiscVInstruction::Slli { dest, src, shamt } => {
                let _ = write!(&mut ir, "%{} = shl i32 {}, {}", dest, self.format_value(src), shamt);
            }
            RiscVInstruction::Srli { dest, src, shamt } => {
                let _ = write!(&mut ir, "%{} = lshr i32 {}, {}", dest, self.format_value(src), shamt);
            }
            RiscVInstruction::Srai { dest, src, shamt } => {
                let _ = write!(&mut ir, "%{} = ashr i32 {}, {}", dest, self.format_value(src), shamt);
            }
            
            // Comparison instructions
            RiscVInstruction::Slt { dest, src1, src2 } => {
                let _ = write!(&mut ir, "%{} = icmp slt i32 {}, {}", dest, self.format_value(src1), self.format_value(src2));
            }
            RiscVInstruction::Sltu { dest, src1, src2 } => {
                let _ = write!(&mut ir, "%{} = icmp ult i32 {}, {}", dest, self.format_value(src1), self.format_value(src2));
            }
            RiscVInstruction::Slti { dest, src, imm } => {
                let _ = write!(&mut ir, "%{} = icmp slt i32 {}, {}", dest, self.format_value(src), imm);
            }
            RiscVInstruction::Sltiu { dest, src, imm } => {
                let _ = write!(&mut ir, "%{} = icmp ult i32 {}, {}", dest, self.format_value(src), imm);
            }
            
            // Upper immediate instructions
            RiscVInstruction::Lui { dest, imm } => {
                let upper_val = (*imm as u32) << 12; // LUI loads 20 bits to upper 20 bits
                let _ = write!(&mut ir, "%{} = add i32 0, {}", dest, upper_val as i32);
            }
            RiscVInstruction::Auipc { dest, imm } => {
                // AUIPC adds upper immediate to PC - in LLVM we use a builtin
                let upper_val = (*imm as u32) << 12;
                let _ = write!(&mut ir, "%{} = add i32 ptrtoint (i8* blockaddress(@main, %entry) to i32), {}", dest, upper_val as i32);
            }
            
            // Memory access instructions
            RiscVInstruction::Lw { dest, base, offset } => {
                let _ = write!(&mut ir, "%{}.addr = add i32 {}, {}", dest, self.format_value(base), offset);
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "%{}.ptr = inttoptr i32 %{}.addr to i32*", dest, dest);
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "%{} = load i32, i32* %{}.ptr, align 4", dest, dest);
            }
            RiscVInstruction::Sw { src, base, offset } => {
                let _ = write!(&mut ir, "%sw.addr = add i32 {}, {}", self.format_value(base), offset);
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "%sw.ptr = inttoptr i32 %sw.addr to i32*");
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "store i32 {}, i32* %sw.ptr, align 4", self.format_value(src));
            }
            RiscVInstruction::Lb { dest, base, offset } => {
                let _ = write!(&mut ir, "%{}.addr = add i32 {}, {}", dest, self.format_value(base), offset);
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "%{}.ptr = inttoptr i32 %{}.addr to i8*", dest, dest);
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "%{}.byte = load i8, i8* %{}.ptr, align 1", dest, dest);
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "%{} = sext i8 %{}.byte to i32", dest, dest);
            }
            RiscVInstruction::Sb { src, base, offset } => {
                let _ = write!(&mut ir, "%sb.addr = add i32 {}, {}", self.format_value(base), offset);
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "%sb.ptr = inttoptr i32 %sb.addr to i8*");
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "%sb.byte = trunc i32 {} to i8", self.format_value(src));
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "store i8 %sb.byte, i8* %sb.ptr, align 1");
            }
            
            // RV64I specific instructions
            RiscVInstruction::Addw { dest, src1, src2 } => {
                // 32-bit add with sign extension to 64-bit
                let _ = write!(&mut ir, "%{}.tmp = add i32 {}, {}", dest, self.format_value(src1), self.format_value(src2));
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "%{} = sext i32 %{}.tmp to i64", dest, dest);
            }
            RiscVInstruction::Subw { dest, src1, src2 } => {
                let _ = write!(&mut ir, "%{}.tmp = sub i32 {}, {}", dest, self.format_value(src1), self.format_value(src2));
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "%{} = sext i32 %{}.tmp to i64", dest, dest);
            }
            RiscVInstruction::Addiw { dest, src, imm } => {
                let _ = write!(&mut ir, "%{}.tmp = add i32 {}, {}", dest, self.format_value(src), imm);
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "%{} = sext i32 %{}.tmp to i64", dest, dest);
            }
            RiscVInstruction::Ld { dest, base, offset } => {
                let _ = write!(&mut ir, "%{}.addr = add i64 {}, {}", dest, self.format_value(base), offset);
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "%{}.ptr = inttoptr i64 %{}.addr to i64*", dest, dest);
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "%{} = load i64, i64* %{}.ptr, align 8", dest, dest);
            }
            RiscVInstruction::Sd { src, base, offset } => {
                let _ = write!(&mut ir, "%sd.addr = add i64 {}, {}", self.format_value(base), offset);
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "%sd.ptr = inttoptr i64 %sd.addr to i64*");
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "store i64 {}, i64* %sd.ptr, align 8", self.format_value(src));
            }
            
            // Control flow instructions
            RiscVInstruction::Beq { src1, src2, label } => {
                let _ = write!(&mut ir, "%eq.cond = icmp eq i32 {}, {}", self.format_value(src1), self.format_value(src2));
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "br i1 %eq.cond, label %{}, label %next", label);
            }
            RiscVInstruction::Bne { src1, src2, label } => {
                let _ = write!(&mut ir, "%ne.cond = icmp ne i32 {}, {}", self.format_value(src1), self.format_value(src2));
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "br i1 %ne.cond, label %{}, label %next", label);
            }
            RiscVInstruction::Blt { src1, src2, label } => {
                let _ = write!(&mut ir, "%lt.cond = icmp slt i32 {}, {}", self.format_value(src1), self.format_value(src2));
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "br i1 %lt.cond, label %{}, label %next", label);
            }
            RiscVInstruction::Bge { src1, src2, label } => {
                let _ = write!(&mut ir, "%ge.cond = icmp sge i32 {}, {}", self.format_value(src1), self.format_value(src2));
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "br i1 %ge.cond, label %{}, label %next", label);
            }
            RiscVInstruction::Bltu { src1, src2, label } => {
                let _ = write!(&mut ir, "%ltu.cond = icmp ult i32 {}, {}", self.format_value(src1), self.format_value(src2));
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "br i1 %ltu.cond, label %{}, label %next", label);
            }
            RiscVInstruction::Bgeu { src1, src2, label } => {
                let _ = write!(&mut ir, "%geu.cond = icmp uge i32 {}, {}", self.format_value(src1), self.format_value(src2));
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "br i1 %geu.cond, label %{}, label %next", label);
            }
            RiscVInstruction::Jal { dest, label } => {
                if let Some(dest_reg) = dest {
                    let _ = write!(&mut ir, "%{} = ptrtoint i8* blockaddress(@main, %return) to i32", dest_reg);
                    ir.push('\n');
                    ir.push_str("  ");
                }
                let _ = write!(&mut ir, "br label %{}", label);
            }
            RiscVInstruction::Jalr { dest, base, offset } => {
                let _ = write!(&mut ir, "%{} = ptrtoint i8* blockaddress(@main, %return) to i32", dest);
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "%jalr.addr = add i32 {}, {}", self.format_value(base), offset);
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "indirectbr i8* inttoptr (i32 %jalr.addr to i8*), []");
            }
            
            // System instructions
            RiscVInstruction::Ecall => {
                ir.push_str("call void @__riscv_ecall()");
            }
            RiscVInstruction::Ebreak => {
                ir.push_str("call void @llvm.debugtrap()");
            }
            
            // Fence instructions
            RiscVInstruction::Fence { pred: _, succ: _ } => {
                ir.push_str("fence seq_cst, seq_cst");
            }
            RiscVInstruction::FenceI => {
                ir.push_str("call void @llvm.instruction.fence()");
            }
            
            // CSR instructions (simplified for basic support)
            RiscVInstruction::Csrrw { dest, csr, src } => {
                let _ = write!(&mut ir, "%{} = call i32 @__riscv_csrrw(i32 {}, i32 {})", dest, csr, self.format_value(src));
            }
            RiscVInstruction::Csrrs { dest, csr, src } => {
                let _ = write!(&mut ir, "%{} = call i32 @__riscv_csrrs(i32 {}, i32 {})", dest, csr, self.format_value(src));
            }
            RiscVInstruction::Csrrc { dest, csr, src } => {
                let _ = write!(&mut ir, "%{} = call i32 @__riscv_csrrc(i32 {}, i32 {})", dest, csr, self.format_value(src));
            }
            RiscVInstruction::Csrrwi { dest, csr, imm } => {
                let _ = write!(&mut ir, "%{} = call i32 @__riscv_csrrw(i32 {}, i32 {})", dest, csr, imm);
            }
            RiscVInstruction::Csrrsi { dest, csr, imm } => {
                let _ = write!(&mut ir, "%{} = call i32 @__riscv_csrrs(i32 {}, i32 {})", dest, csr, imm);
            }
            RiscVInstruction::Csrrci { dest, csr, imm } => {
                let _ = write!(&mut ir, "%{} = call i32 @__riscv_csrrc(i32 {}, i32 {})", dest, csr, imm);
            }
            
            /// RV32M/RV64M: Integer Multiplication and Division Extension
            
            // Multiplication instructions
            RiscVInstruction::Mul { dest, src1, src2 } => {
                let _ = write!(&mut ir, "%{} = mul i32 {}, {}", dest, self.format_value(src1), self.format_value(src2));
            }
            RiscVInstruction::Mulh { dest, src1, src2 } => {
                // High part of signed multiplication
                let _ = write!(&mut ir, "%{}.ext1 = sext i32 {} to i64", dest, self.format_value(src1));
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "%{}.ext2 = sext i32 {} to i64", dest, self.format_value(src2));
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "%{}.full = mul i64 %{}.ext1, %{}.ext2", dest, dest, dest);
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "%{}.shifted = lshr i64 %{}.full, 32", dest, dest);
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "%{} = trunc i64 %{}.shifted to i32", dest, dest);
            }
            RiscVInstruction::Mulhu { dest, src1, src2 } => {
                // High part of unsigned multiplication
                let _ = write!(&mut ir, "%{}.ext1 = zext i32 {} to i64", dest, self.format_value(src1));
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "%{}.ext2 = zext i32 {} to i64", dest, self.format_value(src2));
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "%{}.full = mul i64 %{}.ext1, %{}.ext2", dest, dest, dest);
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "%{}.shifted = lshr i64 %{}.full, 32", dest, dest);
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "%{} = trunc i64 %{}.shifted to i32", dest, dest);
            }
            RiscVInstruction::Mulw { dest, src1, src2 } => {
                // RV64M: 32-bit multiplication with sign extension
                let _ = write!(&mut ir, "%{}.tmp = mul i32 {}, {}", dest, self.format_value(src1), self.format_value(src2));
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "%{} = sext i32 %{}.tmp to i64", dest, dest);
            }
            
            // Division instructions
            RiscVInstruction::Div { dest, src1, src2 } => {
                let _ = write!(&mut ir, "%{} = sdiv i32 {}, {}", dest, self.format_value(src1), self.format_value(src2));
            }
            RiscVInstruction::Divu { dest, src1, src2 } => {
                let _ = write!(&mut ir, "%{} = udiv i32 {}, {}", dest, self.format_value(src1), self.format_value(src2));
            }
            RiscVInstruction::Rem { dest, src1, src2 } => {
                let _ = write!(&mut ir, "%{} = srem i32 {}, {}", dest, self.format_value(src1), self.format_value(src2));
            }
            RiscVInstruction::Remu { dest, src1, src2 } => {
                let _ = write!(&mut ir, "%{} = urem i32 {}, {}", dest, self.format_value(src1), self.format_value(src2));
            }
            RiscVInstruction::Divw { dest, src1, src2 } => {
                // RV64M: 32-bit division with sign extension
                let _ = write!(&mut ir, "%{}.tmp = sdiv i32 {}, {}", dest, self.format_value(src1), self.format_value(src2));
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "%{} = sext i32 %{}.tmp to i64", dest, dest);
            }
            RiscVInstruction::Divuw { dest, src1, src2 } => {
                let _ = write!(&mut ir, "%{}.tmp = udiv i32 {}, {}", dest, self.format_value(src1), self.format_value(src2));
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "%{} = sext i32 %{}.tmp to i64", dest, dest);
            }
            RiscVInstruction::Remw { dest, src1, src2 } => {
                let _ = write!(&mut ir, "%{}.tmp = srem i32 {}, {}", dest, self.format_value(src1), self.format_value(src2));
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "%{} = sext i32 %{}.tmp to i64", dest, dest);
            }
            RiscVInstruction::Remuw { dest, src1, src2 } => {
                let _ = write!(&mut ir, "%{}.tmp = urem i32 {}, {}", dest, self.format_value(src1), self.format_value(src2));
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "%{} = sext i32 %{}.tmp to i64", dest, dest);
            }
            
            /// RV32A/RV64A: Atomic Instructions Extension
            
            // Atomic memory operations (word)
            RiscVInstruction::AmoswapW { dest, addr, src, aq: _, rl: _ } => {
                let _ = write!(&mut ir, "%{}.ptr = inttoptr i32 {} to i32*", dest, self.format_value(addr));
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "%{} = atomicrmw xchg i32* %{}.ptr, i32 {} seq_cst", dest, dest, self.format_value(src));
            }
            RiscVInstruction::AmoaddW { dest, addr, src, aq: _, rl: _ } => {
                let _ = write!(&mut ir, "%{}.ptr = inttoptr i32 {} to i32*", dest, self.format_value(addr));
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "%{} = atomicrmw add i32* %{}.ptr, i32 {} seq_cst", dest, dest, self.format_value(src));
            }
            RiscVInstruction::AmoxorW { dest, addr, src, aq: _, rl: _ } => {
                let _ = write!(&mut ir, "%{}.ptr = inttoptr i32 {} to i32*", dest, self.format_value(addr));
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "%{} = atomicrmw xor i32* %{}.ptr, i32 {} seq_cst", dest, dest, self.format_value(src));
            }
            RiscVInstruction::AmoandW { dest, addr, src, aq: _, rl: _ } => {
                let _ = write!(&mut ir, "%{}.ptr = inttoptr i32 {} to i32*", dest, self.format_value(addr));
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "%{} = atomicrmw and i32* %{}.ptr, i32 {} seq_cst", dest, dest, self.format_value(src));
            }
            RiscVInstruction::AmoorW { dest, addr, src, aq: _, rl: _ } => {
                let _ = write!(&mut ir, "%{}.ptr = inttoptr i32 {} to i32*", dest, self.format_value(addr));
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "%{} = atomicrmw or i32* %{}.ptr, i32 {} seq_cst", dest, dest, self.format_value(src));
            }
            RiscVInstruction::AmominW { dest, addr, src, aq: _, rl: _ } => {
                let _ = write!(&mut ir, "%{}.ptr = inttoptr i32 {} to i32*", dest, self.format_value(addr));
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "%{} = atomicrmw min i32* %{}.ptr, i32 {} seq_cst", dest, dest, self.format_value(src));
            }
            RiscVInstruction::AmomaxW { dest, addr, src, aq: _, rl: _ } => {
                let _ = write!(&mut ir, "%{}.ptr = inttoptr i32 {} to i32*", dest, self.format_value(addr));
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "%{} = atomicrmw max i32* %{}.ptr, i32 {} seq_cst", dest, dest, self.format_value(src));
            }
            
            // Load-reserved/Store-conditional
            RiscVInstruction::LrW { dest, addr, aq: _, rl: _ } => {
                let _ = write!(&mut ir, "%{}.ptr = inttoptr i32 {} to i32*", dest, self.format_value(addr));
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "%{} = load atomic i32, i32* %{}.ptr seq_cst, align 4", dest, dest);
            }
            RiscVInstruction::ScW { dest, addr, src, aq: _, rl: _ } => {
                // Simplified store-conditional - would need LLVM intrinsics for full implementation
                let _ = write!(&mut ir, "%{}.ptr = inttoptr i32 {} to i32*", dest, self.format_value(addr));
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "store atomic i32 {}, i32* %{}.ptr seq_cst, align 4", self.format_value(src), dest);
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "%{} = add i32 0, 0", dest); // Always success for simplicity
            }
            
            /// RV32F: Single-Precision Floating-Point Extension
            
            // Single-precision loads/stores
            RiscVInstruction::Flw { dest, base, offset } => {
                let _ = write!(&mut ir, "%{}.addr = add i32 {}, {}", dest, self.format_value(base), offset);
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "%{}.ptr = inttoptr i32 %{}.addr to float*", dest, dest);
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "%{} = load float, float* %{}.ptr, align 4", dest, dest);
            }
            RiscVInstruction::Fsw { src, base, offset } => {
                let _ = write!(&mut ir, "%fsw.addr = add i32 {}, {}", self.format_value(base), offset);
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "%fsw.ptr = inttoptr i32 %fsw.addr to float*");
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "store float {}, float* %fsw.ptr, align 4", self.format_value(src));
            }
            
            // Single-precision arithmetic
            RiscVInstruction::FaddS { dest, src1, src2, rm: _ } => {
                let _ = write!(&mut ir, "%{} = fadd float {}, {}", dest, self.format_value(src1), self.format_value(src2));
            }
            RiscVInstruction::FsubS { dest, src1, src2, rm: _ } => {
                let _ = write!(&mut ir, "%{} = fsub float {}, {}", dest, self.format_value(src1), self.format_value(src2));
            }
            RiscVInstruction::FmulS { dest, src1, src2, rm: _ } => {
                let _ = write!(&mut ir, "%{} = fmul float {}, {}", dest, self.format_value(src1), self.format_value(src2));
            }
            RiscVInstruction::FdivS { dest, src1, src2, rm: _ } => {
                let _ = write!(&mut ir, "%{} = fdiv float {}, {}", dest, self.format_value(src1), self.format_value(src2));
            }
            RiscVInstruction::FsqrtS { dest, src, rm: _ } => {
                let _ = write!(&mut ir, "%{} = call float @llvm.sqrt.f32(float {})", dest, self.format_value(src));
            }
            
            // Single-precision comparisons
            RiscVInstruction::FeqS { dest, src1, src2 } => {
                let _ = write!(&mut ir, "%{}.cmp = fcmp oeq float {}, {}", dest, self.format_value(src1), self.format_value(src2));
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "%{} = zext i1 %{}.cmp to i32", dest, dest);
            }
            RiscVInstruction::FltS { dest, src1, src2 } => {
                let _ = write!(&mut ir, "%{}.cmp = fcmp olt float {}, {}", dest, self.format_value(src1), self.format_value(src2));
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "%{} = zext i1 %{}.cmp to i32", dest, dest);
            }
            RiscVInstruction::FleS { dest, src1, src2 } => {
                let _ = write!(&mut ir, "%{}.cmp = fcmp ole float {}, {}", dest, self.format_value(src1), self.format_value(src2));
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "%{} = zext i1 %{}.cmp to i32", dest, dest);
            }
            
            // Single-precision conversions
            RiscVInstruction::FcvtWS { dest, src, rm: _ } => {
                let _ = write!(&mut ir, "%{} = fptosi float {} to i32", dest, self.format_value(src));
            }
            RiscVInstruction::FcvtSW { dest, src, rm: _ } => {
                let _ = write!(&mut ir, "%{} = sitofp i32 {} to float", dest, self.format_value(src));
            }
            
            /// RV32D: Double-Precision Floating-Point Extension
            
            // Double-precision loads/stores
            RiscVInstruction::Fld { dest, base, offset } => {
                let _ = write!(&mut ir, "%{}.addr = add i32 {}, {}", dest, self.format_value(base), offset);
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "%{}.ptr = inttoptr i32 %{}.addr to double*", dest, dest);
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "%{} = load double, double* %{}.ptr, align 8", dest, dest);
            }
            RiscVInstruction::Fsd { src, base, offset } => {
                let _ = write!(&mut ir, "%fsd.addr = add i32 {}, {}", self.format_value(base), offset);
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "%fsd.ptr = inttoptr i32 %fsd.addr to double*");
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "store double {}, double* %fsd.ptr, align 8", self.format_value(src));
            }
            
            // Double-precision arithmetic
            RiscVInstruction::FaddD { dest, src1, src2, rm: _ } => {
                let _ = write!(&mut ir, "%{} = fadd double {}, {}", dest, self.format_value(src1), self.format_value(src2));
            }
            RiscVInstruction::FsubD { dest, src1, src2, rm: _ } => {
                let _ = write!(&mut ir, "%{} = fsub double {}, {}", dest, self.format_value(src1), self.format_value(src2));
            }
            RiscVInstruction::FmulD { dest, src1, src2, rm: _ } => {
                let _ = write!(&mut ir, "%{} = fmul double {}, {}", dest, self.format_value(src1), self.format_value(src2));
            }
            RiscVInstruction::FdivD { dest, src1, src2, rm: _ } => {
                let _ = write!(&mut ir, "%{} = fdiv double {}, {}", dest, self.format_value(src1), self.format_value(src2));
            }
            RiscVInstruction::FsqrtD { dest, src, rm: _ } => {
                let _ = write!(&mut ir, "%{} = call double @llvm.sqrt.f64(double {})", dest, self.format_value(src));
            }
            
            // Double-precision comparisons
            RiscVInstruction::FeqD { dest, src1, src2 } => {
                let _ = write!(&mut ir, "%{}.cmp = fcmp oeq double {}, {}", dest, self.format_value(src1), self.format_value(src2));
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "%{} = zext i1 %{}.cmp to i32", dest, dest);
            }
            
            // Double-precision conversions
            RiscVInstruction::FcvtSD { dest, src, rm: _ } => {
                let _ = write!(&mut ir, "%{} = fptrunc double {} to float", dest, self.format_value(src));
            }
            RiscVInstruction::FcvtDS { dest, src, rm: _ } => {
                let _ = write!(&mut ir, "%{} = fpext float {} to double", dest, self.format_value(src));
            }
            RiscVInstruction::FcvtWD { dest, src, rm: _ } => {
                let _ = write!(&mut ir, "%{} = fptosi double {} to i32", dest, self.format_value(src));
            }
            RiscVInstruction::FcvtDW { dest, src, rm: _ } => {
                let _ = write!(&mut ir, "%{} = sitofp i32 {} to double", dest, self.format_value(src));
            }
            
            /// RV32C: Compressed Instructions Extension
            /// These map to their uncompressed equivalents
            
            RiscVInstruction::CAddi { dest, imm } => {
                let _ = write!(&mut ir, "%{} = add i32 %{}, {}", dest, dest, imm);
            }
            RiscVInstruction::CLi { dest, imm } => {
                let _ = write!(&mut ir, "%{} = add i32 0, {}", dest, imm);
            }
            RiscVInstruction::CLw { dest, base, offset } => {
                let _ = write!(&mut ir, "%{}.addr = add i32 {}, {}", dest, self.format_value(base), offset);
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "%{}.ptr = inttoptr i32 %{}.addr to i32*", dest, dest);
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "%{} = load i32, i32* %{}.ptr, align 4", dest, dest);
            }
            RiscVInstruction::CSw { src, base, offset } => {
                let _ = write!(&mut ir, "%csw.addr = add i32 {}, {}", self.format_value(base), offset);
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "%csw.ptr = inttoptr i32 %csw.addr to i32*");
                ir.push('\n');
                ir.push_str("  ");
                let _ = write!(&mut ir, "store i32 {}, i32* %csw.ptr, align 4", self.format_value(src));
            }
            RiscVInstruction::CAdd { dest, src } => {
                let _ = write!(&mut ir, "%{} = add i32 %{}, {}", dest, dest, self.format_value(src));
            }
            RiscVInstruction::CMv { dest, src } => {
                let _ = write!(&mut ir, "%{} = add i32 0, {}", dest, self.format_value(src));
            }
            
            // Handle remaining cases to ensure completeness
            _ => {
                ir.push_str("; RISC-V instruction not yet implemented in LLVM backend");
            }
        }
        
        Ok(ir)
    }
    
    /// Format a value for LLVM IR
    fn format_value(&mut self, value: &crate::Value) -> String {
        match value {
            crate::Value::Register(reg) => format!("%{}", reg),
            crate::Value::Integer(val) => val.to_string(),
            crate::Value::Float(val) => format!("{:.6}", val),
            crate::Value::Boolean(val) => if *val { "1" } else { "0" }.to_string(),
            crate::Value::String(s) => {
                // Generate a string constant reference
                // Check if string already exists, reuse its ID if so
                let str_id = if let Some(&existing_id) = self.string_constants.get(s) {
                    println!("[DEBUG] Reusing string constant {}: '{}'", existing_id, s);
                    existing_id
                } else {
                    let new_id = self.next_string_id;
                    self.next_string_id += 1;
                    println!("[DEBUG] Adding new string constant {}: '{}'", new_id, s);
                    self.string_constants.insert(s.clone(), new_id);
                    new_id
                };
                format!("@.str.{}", str_id)
            }
        }
    }
    
    /// Format a type for LLVM IR
    fn format_type(&self, ty: &crate::Type) -> String {
        match ty {
            crate::Type::I32 => "i32".to_string(),
            crate::Type::I64 => "i64".to_string(),
            crate::Type::F32 => "float".to_string(),
            crate::Type::F64 => "double".to_string(),
            crate::Type::Bool => "i1".to_string(),
            crate::Type::Ptr(inner) => format!("{}*", self.format_type(inner)),
            crate::Type::Void => "void".to_string(),
        }
    }
    
    /// Generate debug metadata
    fn generate_debug_metadata(&self) -> String {
        use std::fmt::Write;
        let mut result = String::with_capacity(700);
        let _ = write!(&mut result, r#"!llvm.dbg.cu = !{{!0}}
!llvm.module.flags = !{{!1, !2}}

!0 = !DICompileUnit(language: DW_LANG_C99, file: !3, producer: "Seen Compiler", isOptimized: false, runtimeVersion: 0, emissionKind: FullDebug, subprograms: !4)
!1 = !{{i32 2, !"Dwarf Version", i32 4}}
!2 = !{{i32 2, !"Debug Info Version", i32 3}}
!3 = !DIFile(filename: "{}.seen", directory: "/tmp")
!4 = !{{!5}}
!5 = !DISubprogram(name: "main", scope: !3, file: !3, line: 1, type: !6, isLocal: false, isDefinition: true, scopeLine: 1, isOptimized: false, unit: !0)
!6 = !DISubroutineType(types: !7)
!7 = !{{null}}
!8 = !DILocation(line: 1, column: 1, scope: !5)

"#, self.module_name);
        result
    }
    
    /// Generate debug declarations
    fn generate_debug_declarations(&self) -> String {
        r#"declare void @llvm.dbg.declare(metadata, metadata, metadata)

"#.to_string()
    }
    
    /// Legacy generate method for compatibility
    pub fn generate(&mut self) -> SeenResult<String> {
        // Create a simple test module with default target
        let test_module = Module {
            name: self.module_name.clone(),
            target: Target::x86_64_linux(),
            functions: vec![Function {
                name: "main".to_string(),
                params: vec![],
                blocks: vec![BasicBlock {
                    label: "entry".to_string(),
                    instructions: vec![
                        Instruction::Return { value: Some(crate::Value::Integer(0)) },
                    ],
                }],
            }],
        };
        
        self.generate_llvm_ir(&test_module)
    }
    
    /// Generate code with specific RISC-V target
    pub fn generate_riscv(&mut self, target: Target, extensions: RiscVExtensions) -> SeenResult<String> {
        if !target.is_riscv() {
            return Err(seen_common::SeenError::codegen_error("Target is not RISC-V".to_string()));
        }
        
        // Update target triple for RISC-V
        self.target_triple = target.to_llvm_triple();
        
        // Store register size before moving target
        let register_size = target.register_size();
        
        // Create RISC-V test module
        let test_module = Module {
            name: self.module_name.clone(),
            target,
            functions: vec![Function {
                name: "main".to_string(),
                params: vec![],
                blocks: vec![BasicBlock {
                    label: "entry".to_string(),
                    instructions: vec![
                        Instruction::Return { value: Some(crate::Value::Integer(0)) },
                    ],
                }],
            }],
        };
        
        let mut llvm_ir = self.generate_llvm_ir(&test_module)?;
        
        // Add RISC-V specific optimizations if vector extension is enabled
        if extensions.v {
            llvm_ir.push_str("\n; RISC-V Vector Extension optimizations enabled\n");
            llvm_ir.push_str(&format!("attributes #0 = {{ \"target-features\"=\"+v,+zvl128b,+{extensions}\" }}\n", 
                extensions = extensions.to_isa_string(register_size)));
        }
        
        Ok(llvm_ir)
    }
    
    /// Generate string constant declarations
    fn generate_string_constants(&self) -> String {
        println!("[DEBUG] generate_string_constants called with {} constants", self.string_constants.len());
        for (text, id) in &self.string_constants {
            println!("[DEBUG] String constant {}: '{}'", id, text);
        }
        
        if self.string_constants.is_empty() {
            return String::new();
        }
        
        let mut result = String::with_capacity(self.string_constants.len() * 80);
        result.push_str("\n; String constants\n");
        
        // Sort by ID to ensure consistent order
        let mut constants: Vec<(&String, &usize)> = self.string_constants.iter().collect();
        constants.sort_by_key(|(_, &id)| id);
        
        for (text, &id) in constants {
            // Escape string for LLVM IR
            let escaped = text
                .replace("\\", "\\\\")
                .replace("\"", "\\\"")
                .replace("\n", "\\0A")
                .replace("\r", "\\0D")
                .replace("\t", "\\09");
            
            result.push_str(&format!(
                "@.str.{} = private unnamed_addr constant [{} x i8] c\"{}\\00\", align 1\n", 
                id, escaped.len() + 1, escaped
            ));
        }
        result.push('\n');
        result
    }
    
    /// Generate standard library function declarations
    fn generate_stdlib_declarations(&self) -> String {
        let mut result = String::with_capacity(800);
        result.push_str("; Standard library function declarations\n");
        result.push_str("declare i32 @printf(i8*, ...)\n");
        result.push_str("declare i32 @puts(i8*)\n");
        result.push_str("declare i64 @strlen(i8*)\n");
        result.push_str("declare i8* @malloc(i32)\n");
        result.push_str("declare void @free(i8*)\n");
        result.push_str("declare void @llvm.memcpy.p0i8.p0i8.i32(i8*, i8*, i32, i1)\n");
        result.push_str("declare void @llvm.memmove.p0i8.p0i8.i32(i8*, i8*, i32, i1)\n");
        result.push_str("declare void @llvm.memset.p0i8.i32(i8*, i8, i32, i1)\n");
        result.push_str("\n");
        
        // Generate wrapper functions for Seen's print functions
        result.push_str("; Seen print function wrappers\n");
        result.push_str("define void @print(i8* %str) {\n");
        result.push_str("entry:\n");
        result.push_str("  %result = call i32 @printf(i8* %str)\n");
        result.push_str("  ret void\n");
        result.push_str("}\n\n");
        
        result.push_str("define void @println(i8* %str) {\n");
        result.push_str("entry:\n");
        result.push_str("  %result = call i32 @puts(i8* %str)\n");
        result.push_str("  ret void\n");
        result.push_str("}\n\n");
        
        result
    }
    
    /// Get the length of a string value
    fn get_string_length(&self, value: &crate::Value) -> usize {
        match value {
            crate::Value::String(s) => s.len(),
            _ => 0,
        }
    }
}