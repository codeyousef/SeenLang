//! LLVM backend

use seen_common::{SeenResult, SeenError};

/// LLVM code generator
pub struct LLVMBackend {
    module_name: String,
}

impl LLVMBackend {
    pub fn new(module_name: String) -> Self {
        Self { module_name }
    }
    
    pub fn generate(&self) -> SeenResult<String> {
        let mut llvm_ir = String::new();
        
        llvm_ir.push_str(&format!("; Module: {}\n", self.module_name));
        llvm_ir.push_str("target triple = \"x86_64-unknown-linux-gnu\"\n");
        llvm_ir.push_str("\n");
        
        llvm_ir.push_str("define i32 @main() {\n");
        llvm_ir.push_str("entry:\n");
        llvm_ir.push_str("  ret i32 0\n");
        llvm_ir.push_str("}\n");
        
        Ok(llvm_ir)
    }
}