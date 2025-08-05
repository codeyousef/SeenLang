//! Code generation with LLVM backend

use crate::ir::*;
use seen_common::SeenResult;

/// Code generator with LLVM backend
pub struct CodeGenerator {
    module_name: String,
    debug_info_enabled: bool,
    calling_convention: String,
    target_triple: String,
    optimization_level: u32,
}

impl CodeGenerator {
    pub fn new(module_name: String) -> Self {
        Self {
            module_name,
            debug_info_enabled: false,
            calling_convention: "fastcc".to_string(),
            target_triple: "x86_64-unknown-linux-gnu".to_string(),
            optimization_level: 2,
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
        let mut llvm_ir = String::new();
        
        // Module header with target information
        llvm_ir.push_str(&format!("target triple = \"{}\"\n", self.target_triple));
        llvm_ir.push_str(&format!("; Module: {}\n", module.name));
        llvm_ir.push_str("\n");
        
        // Debug info metadata if enabled
        if self.debug_info_enabled {
            llvm_ir.push_str(&self.generate_debug_metadata());
        }
        
        // Generate functions
        for function in &module.functions {
            llvm_ir.push_str(&self.generate_function_ir(function)?);
            llvm_ir.push_str("\n");
        }
        
        // Debug info declarations if enabled
        if self.debug_info_enabled {
            llvm_ir.push_str(&self.generate_debug_declarations());
        }
        
        Ok(llvm_ir)
    }
    
    /// Generate LLVM IR for a function
    fn generate_function_ir(&self, function: &Function) -> SeenResult<String> {
        let mut ir = String::new();
        
        // Function signature
        let calling_conv = if self.calling_convention == "C" { "ccc" } else { &self.calling_convention };
        ir.push_str(&format!("define i32 @{}(", function.name));
        
        // Parameters
        for (i, param) in function.params.iter().enumerate() {
            if i > 0 { ir.push_str(", "); }
            ir.push_str(&format!("i32 %{}", param));
        }
        ir.push_str(&format!(") {} {{\n", calling_conv));
        
        // Generate basic blocks
        for block in &function.blocks {
            ir.push_str(&self.generate_basic_block_ir(block)?);
        }
        
        ir.push_str("}\n");
        Ok(ir)
    }
    
    /// Generate LLVM IR for a basic block
    fn generate_basic_block_ir(&self, block: &BasicBlock) -> SeenResult<String> {
        let mut ir = String::new();
        
        // Block label
        ir.push_str(&format!("{}:\n", block.label));
        
        // Generate instructions
        for instruction in &block.instructions {
            ir.push_str("  ");
            ir.push_str(&self.generate_instruction_ir(instruction)?);
            ir.push_str("\n");
        }
        
        Ok(ir)
    }
    
    /// Generate LLVM IR for an instruction
    fn generate_instruction_ir(&self, instruction: &Instruction) -> SeenResult<String> {
        let ir = match instruction {
            Instruction::Load { dest, src } => {
                format!("%{} = load i32, i32* %{}, align 4", dest, src)
            }
            Instruction::Store { dest, src } => {
                format!("store i32 %{}, i32* %{}, align 4", src, dest)
            }
            Instruction::Call { dest, func, args } => {
                let args_str = args.iter()
                    .map(|arg| format!("i32 %{}", arg))
                    .collect::<Vec<_>>()
                    .join(", ");
                
                if let Some(dest_reg) = dest {
                    format!("%{} = call i32 @{}({})", dest_reg, func, args_str)
                } else {
                    format!("call void @{}({})", func, args_str)
                }
            }
            Instruction::Return { value } => {
                if let Some(val) = value {
                    format!("ret i32 %{}", val)
                } else {
                    "ret void".to_string()
                }
            }
            Instruction::Nop => {
                "; nop".to_string()
            }
        };
        
        Ok(ir)
    }
    
    /// Generate debug metadata
    fn generate_debug_metadata(&self) -> String {
        format!(r#"!llvm.dbg.cu = !{{!0}}
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

"#, self.module_name)
    }
    
    /// Generate debug declarations
    fn generate_debug_declarations(&self) -> String {
        r#"declare void @llvm.dbg.declare(metadata, metadata, metadata)

"#.to_string()
    }
    
    /// Legacy generate method for compatibility
    pub fn generate(&mut self) -> SeenResult<String> {
        // Create a simple test module
        let test_module = Module {
            name: self.module_name.clone(),
            functions: vec![Function {
                name: "main".to_string(),
                params: vec![],
                blocks: vec![BasicBlock {
                    label: "entry".to_string(),
                    instructions: vec![
                        Instruction::Return { value: None },
                    ],
                }],
            }],
        };
        
        self.generate_llvm_ir(&test_module)
    }
}