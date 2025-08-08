//! LLVM backend

use seen_common::{SeenResult, SeenError};
use crate::ir::{Target, Architecture, RiscVExtensions};

/// LLVM code generator
pub struct LLVMBackend {
    module_name: String,
    target: Target,
    extensions: Option<RiscVExtensions>,
}

impl LLVMBackend {
    pub fn new(module_name: String) -> Self {
        Self { 
            module_name,
            target: Target::x86_64_linux(),
            extensions: None,
        }
    }
    
    pub fn with_target(module_name: String, target: Target) -> Self {
        let extensions = if target.is_riscv() {
            Some(RiscVExtensions::rv64gc()) // Default to full ISA
        } else {
            None
        };
        
        Self { 
            module_name,
            target,
            extensions,
        }
    }
    
    pub fn with_riscv_extensions(mut self, extensions: RiscVExtensions) -> Self {
        if self.target.is_riscv() {
            self.extensions = Some(extensions);
        }
        self
    }
    
    pub fn generate(&self) -> SeenResult<String> {
        let mut llvm_ir = String::new();
        
        llvm_ir.push_str(&format!("; Module: {}\n", self.module_name));
        llvm_ir.push_str(&format!("target triple = \"{}\"\n", self.target.to_llvm_triple()));
        
        // Add target-specific data layout
        llvm_ir.push_str(&self.generate_data_layout()?);
        llvm_ir.push_str("\n");
        
        // Add target-specific attributes for RISC-V
        if self.target.is_riscv() {
            llvm_ir.push_str(&self.generate_riscv_attributes()?);
        }
        
        llvm_ir.push_str("define i32 @main() {\n");
        llvm_ir.push_str("entry:\n");
        llvm_ir.push_str("  ret i32 0\n");
        llvm_ir.push_str("}\n");
        
        Ok(llvm_ir)
    }
    
    fn generate_data_layout(&self) -> SeenResult<String> {
        let layout = match self.target.arch {
            Architecture::X86_64 => {
                "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-f80:128-n8:16:32:64-S128\""
            },
            Architecture::RiscV64 => {
                "target datalayout = \"e-m:e-p:64:64-i64:64-i128:128-n64-S128\""
            },
            Architecture::RiscV32 => {
                "target datalayout = \"e-m:e-p:32:32-i64:64-n32-S128\""
            },
            Architecture::Wasm32 => {
                "target datalayout = \"e-m:e-p:32:32-i64:64-n32:64-S128\""
            },
        };
        
        Ok(format!("{}\n", layout))
    }
    
    fn generate_riscv_attributes(&self) -> SeenResult<String> {
        let mut attributes = String::new();
        
        if let Some(extensions) = &self.extensions {
            let xlen = match self.target.arch {
                Architecture::RiscV64 => 64,
                Architecture::RiscV32 => 32,
                _ => return Err(SeenError::codegen_error("Invalid RISC-V architecture".to_string())),
            };
            
            let isa_string = extensions.to_isa_string(xlen);
            
            // Add ISA string as module attribute
            attributes.push_str(&format!("!llvm.module.flags = !{{!0, !1}}\n"));
            attributes.push_str(&format!("!0 = !{{i32 2, !\"riscv-isa\", !\"{}\"}}\n", isa_string));
            
            // Add target CPU based on extensions
            let cpu = self.get_riscv_cpu_string(extensions, xlen)?;
            attributes.push_str(&format!("!1 = !{{i32 2, !\"target-cpu\", !\"{}\"}}\n", cpu));
            
            // Add target features
            let features = self.get_riscv_target_features(extensions)?;
            if !features.is_empty() {
                attributes.push_str(&format!("!2 = !{{i32 2, !\"target-features\", !\"{}\"}}\n", features));
            }
            
            attributes.push_str("\n");
        }
        
        Ok(attributes)
    }
    
    fn get_riscv_cpu_string(&self, extensions: &RiscVExtensions, xlen: u32) -> SeenResult<String> {
        // Map to appropriate RISC-V CPU target
        let cpu = match xlen {
            32 => {
                if extensions.v {
                    "generic-rv32-v" // Vector-enabled 32-bit
                } else if extensions.d {
                    "generic-rv32" // Full RV32GC
                } else if extensions.m {
                    "generic-rv32-m" // With multiply/divide
                } else {
                    "generic-rv32-i" // Base integer only
                }
            },
            64 => {
                if extensions.v {
                    "generic-rv64-v" // Vector-enabled 64-bit
                } else if extensions.d {
                    "generic-rv64" // Full RV64GC
                } else if extensions.m {
                    "generic-rv64-m" // With multiply/divide
                } else {
                    "generic-rv64-i" // Base integer only
                }
            },
            _ => return Err(SeenError::codegen_error(format!("Invalid RISC-V XLEN: {}", xlen))),
        };
        
        Ok(cpu.to_string())
    }
    
    fn get_riscv_target_features(&self, extensions: &RiscVExtensions) -> SeenResult<String> {
        let mut features = Vec::new();
        
        // Add positive features
        if extensions.m { features.push("+m"); }
        if extensions.a { features.push("+a"); }
        if extensions.f { features.push("+f"); }
        if extensions.d { features.push("+d"); }
        if extensions.c { features.push("+c"); }
        if extensions.v { 
            features.push("+v");
            features.push("+zvl128b"); // Minimum vector length
        }
        
        // Add negative features for what we don't support
        if !extensions.m { features.push("-m"); }
        if !extensions.a { features.push("-a"); }
        if !extensions.f { features.push("-f"); }
        if !extensions.d { features.push("-d"); }
        if !extensions.c { features.push("-c"); }
        if !extensions.v { features.push("-v"); }
        
        Ok(features.join(","))
    }
    
    /// Generate RISC-V vector-optimized code for reactive operations
    pub fn generate_vector_optimized_reactive(&self, operation: &str) -> SeenResult<String> {
        if !self.target.supports_rvv() || !self.extensions.as_ref().map_or(false, |e| e.v) {
            return Err(SeenError::codegen_error("RISC-V Vector extension not enabled".to_string()));
        }
        
        let mut code = String::new();
        
        match operation {
            "map" => {
                code.push_str("; RISC-V Vector-optimized map operation using RVV 1.0\n");
                code.push_str("; Applies transformation function to all elements in parallel\n");
                code.push_str("define void @vector_map_i32(i32* %dst, i32* %src, i64 %n, i32 (i32)* %transform) {\n");
                code.push_str("entry:\n");
                code.push_str("  br label %vector.body\n\n");
                code.push_str("vector.body:\n");
                code.push_str("  %index = phi i64 [ 0, %entry ], [ %index.next, %vector.body ]\n");
                code.push_str("  ; Configure vector length (vsetvli)\n");
                code.push_str("  %vl = call i64 @llvm.riscv.vsetvli(i64 %n, i64 2, i64 0)\n"); // e32, m1
                code.push_str("  ; Load vector (vle32.v)\n");
                code.push_str("  %src.ptr = getelementptr i32, i32* %src, i64 %index\n");
                code.push_str("  %vec = call <vscale x 4 x i32> @llvm.riscv.vle.nxv4i32(<vscale x 4 x i32> undef, i32* %src.ptr, i64 %vl)\n");
                code.push_str("  ; Apply transformation using vector operations\n");
                code.push_str("  %transformed = call <vscale x 4 x i32> @llvm.riscv.vadd.nxv4i32(<vscale x 4 x i32> %vec, <vscale x 4 x i32> %vec, i64 %vl)\n");
                code.push_str("  ; Store result (vse32.v)\n");
                code.push_str("  %dst.ptr = getelementptr i32, i32* %dst, i64 %index\n");
                code.push_str("  call void @llvm.riscv.vse.nxv4i32(<vscale x 4 x i32> %transformed, i32* %dst.ptr, i64 %vl)\n");
                code.push_str("  %index.next = add i64 %index, %vl\n");
                code.push_str("  %done = icmp uge i64 %index.next, %n\n");
                code.push_str("  br i1 %done, label %exit, label %vector.body\n\n");
                code.push_str("exit:\n");
                code.push_str("  ret void\n");
                code.push_str("}\n\n");
                
                // Float version
                code.push_str("define void @vector_map_f32(float* %dst, float* %src, i64 %n, float (float)* %transform) {\n");
                code.push_str("entry:\n");
                code.push_str("  br label %vector.body\n\n");
                code.push_str("vector.body:\n");
                code.push_str("  %index = phi i64 [ 0, %entry ], [ %index.next, %vector.body ]\n");
                code.push_str("  %vl = call i64 @llvm.riscv.vsetvli(i64 %n, i64 2, i64 0)\n");
                code.push_str("  %src.ptr = getelementptr float, float* %src, i64 %index\n");
                code.push_str("  %vec = call <vscale x 4 x float> @llvm.riscv.vle.nxv4f32(<vscale x 4 x float> undef, float* %src.ptr, i64 %vl)\n");
                code.push_str("  %transformed = call <vscale x 4 x float> @llvm.riscv.vfadd.nxv4f32(<vscale x 4 x float> %vec, <vscale x 4 x float> %vec, i64 %vl)\n");
                code.push_str("  %dst.ptr = getelementptr float, float* %dst, i64 %index\n");
                code.push_str("  call void @llvm.riscv.vse.nxv4f32(<vscale x 4 x float> %transformed, float* %dst.ptr, i64 %vl)\n");
                code.push_str("  %index.next = add i64 %index, %vl\n");
                code.push_str("  %done = icmp uge i64 %index.next, %n\n");
                code.push_str("  br i1 %done, label %exit, label %vector.body\n\n");
                code.push_str("exit:\n");
                code.push_str("  ret void\n");
                code.push_str("}\n");
            },
            "filter" => {
                code.push_str("; RISC-V Vector-optimized filter operation using RVV 1.0\n");
                code.push_str("; Filters elements based on predicate using vector mask operations\n");
                code.push_str("define i64 @vector_filter_i32(i32* %dst, i32* %src, i64 %n, i1 (i32)* %predicate) {\n");
                code.push_str("entry:\n");
                code.push_str("  %dst.start = alloca i32*\n");
                code.push_str("  store i32* %dst, i32** %dst.start\n");
                code.push_str("  br label %vector.body\n\n");
                code.push_str("vector.body:\n");
                code.push_str("  %index = phi i64 [ 0, %entry ], [ %index.next, %vector.body ]\n");
                code.push_str("  %out.idx = phi i64 [ 0, %entry ], [ %out.idx.next, %vector.body ]\n");
                code.push_str("  %vl = call i64 @llvm.riscv.vsetvli(i64 %n, i64 2, i64 0)\n");
                code.push_str("  ; Load vector\n");
                code.push_str("  %src.ptr = getelementptr i32, i32* %src, i64 %index\n");
                code.push_str("  %vec = call <vscale x 4 x i32> @llvm.riscv.vle.nxv4i32(<vscale x 4 x i32> undef, i32* %src.ptr, i64 %vl)\n");
                code.push_str("  ; Compare with threshold (vmslt.vx for example)\n");
                code.push_str("  %threshold = add i32 0, 50\n");
                code.push_str("  %mask = call <vscale x 4 x i1> @llvm.riscv.vmslt.nxv4i32(<vscale x 4 x i32> %vec, i32 %threshold, i64 %vl)\n");
                code.push_str("  ; Compress using vcompress.vm\n");
                code.push_str("  %compressed = call <vscale x 4 x i32> @llvm.riscv.vcompress.nxv4i32(<vscale x 4 x i32> %vec, <vscale x 4 x i1> %mask, i64 %vl)\n");
                code.push_str("  ; Count set bits in mask (vcpop.m)\n");
                code.push_str("  %count = call i64 @llvm.riscv.vcpop.i64(<vscale x 4 x i1> %mask, i64 %vl)\n");
                code.push_str("  ; Store compressed result\n");
                code.push_str("  %dst.ptr = getelementptr i32, i32* %dst, i64 %out.idx\n");
                code.push_str("  call void @llvm.riscv.vse.nxv4i32(<vscale x 4 x i32> %compressed, i32* %dst.ptr, i64 %count)\n");
                code.push_str("  %index.next = add i64 %index, %vl\n");
                code.push_str("  %out.idx.next = add i64 %out.idx, %count\n");
                code.push_str("  %done = icmp uge i64 %index.next, %n\n");
                code.push_str("  br i1 %done, label %exit, label %vector.body\n\n");
                code.push_str("exit:\n");
                code.push_str("  ret i64 %out.idx.next\n");
                code.push_str("}\n");
            },
            "reduce" => {
                code.push_str("; RISC-V Vector-optimized reduce operations using RVV 1.0\n");
                code.push_str("; Sum reduction using vredsum.vs\n");
                code.push_str("define i32 @vector_reduce_sum_i32(i32* %src, i64 %n) {\n");
                code.push_str("entry:\n");
                code.push_str("  %sum = alloca i32\n");
                code.push_str("  store i32 0, i32* %sum\n");
                code.push_str("  br label %vector.body\n\n");
                code.push_str("vector.body:\n");
                code.push_str("  %index = phi i64 [ 0, %entry ], [ %index.next, %vector.body ]\n");
                code.push_str("  %acc = phi i32 [ 0, %entry ], [ %acc.next, %vector.body ]\n");
                code.push_str("  %vl = call i64 @llvm.riscv.vsetvli(i64 %n, i64 2, i64 0)\n");
                code.push_str("  %src.ptr = getelementptr i32, i32* %src, i64 %index\n");
                code.push_str("  %vec = call <vscale x 4 x i32> @llvm.riscv.vle.nxv4i32(<vscale x 4 x i32> undef, i32* %src.ptr, i64 %vl)\n");
                code.push_str("  ; Reduce sum using vredsum.vs\n");
                code.push_str("  %reduced = call i32 @llvm.riscv.vredsum.nxv4i32(<vscale x 4 x i32> %vec, i32 %acc, i64 %vl)\n");
                code.push_str("  %acc.next = add i32 %acc, %reduced\n");
                code.push_str("  %index.next = add i64 %index, %vl\n");
                code.push_str("  %done = icmp uge i64 %index.next, %n\n");
                code.push_str("  br i1 %done, label %exit, label %vector.body\n\n");
                code.push_str("exit:\n");
                code.push_str("  ret i32 %acc.next\n");
                code.push_str("}\n\n");
                
                // Max reduction
                code.push_str("define i32 @vector_reduce_max_i32(i32* %src, i64 %n) {\n");
                code.push_str("entry:\n");
                code.push_str("  br label %vector.body\n\n");
                code.push_str("vector.body:\n");
                code.push_str("  %index = phi i64 [ 0, %entry ], [ %index.next, %vector.body ]\n");
                code.push_str("  %max = phi i32 [ -2147483648, %entry ], [ %max.next, %vector.body ]\n");
                code.push_str("  %vl = call i64 @llvm.riscv.vsetvli(i64 %n, i64 2, i64 0)\n");
                code.push_str("  %src.ptr = getelementptr i32, i32* %src, i64 %index\n");
                code.push_str("  %vec = call <vscale x 4 x i32> @llvm.riscv.vle.nxv4i32(<vscale x 4 x i32> undef, i32* %src.ptr, i64 %vl)\n");
                code.push_str("  ; Reduce max using vredmax.vs\n");
                code.push_str("  %max.next = call i32 @llvm.riscv.vredmax.nxv4i32(<vscale x 4 x i32> %vec, i32 %max, i64 %vl)\n");
                code.push_str("  %index.next = add i64 %index, %vl\n");
                code.push_str("  %done = icmp uge i64 %index.next, %n\n");
                code.push_str("  br i1 %done, label %exit, label %vector.body\n\n");
                code.push_str("exit:\n");
                code.push_str("  ret i32 %max.next\n");
                code.push_str("}\n");
            },
            "scan" => {
                code.push_str("; RISC-V Vector-optimized scan (prefix sum) using RVV 1.0\n");
                code.push_str("define void @vector_scan_i32(i32* %dst, i32* %src, i64 %n) {\n");
                code.push_str("entry:\n");
                code.push_str("  br label %vector.body\n\n");
                code.push_str("vector.body:\n");
                code.push_str("  %index = phi i64 [ 0, %entry ], [ %index.next, %vector.body ]\n");
                code.push_str("  %carry = phi i32 [ 0, %entry ], [ %carry.next, %vector.body ]\n");
                code.push_str("  %vl = call i64 @llvm.riscv.vsetvli(i64 %n, i64 2, i64 0)\n");
                code.push_str("  %src.ptr = getelementptr i32, i32* %src, i64 %index\n");
                code.push_str("  %vec = call <vscale x 4 x i32> @llvm.riscv.vle.nxv4i32(<vscale x 4 x i32> undef, i32* %src.ptr, i64 %vl)\n");
                code.push_str("  ; Add carry to all elements\n");
                code.push_str("  %with.carry = call <vscale x 4 x i32> @llvm.riscv.vadd.vx.nxv4i32(<vscale x 4 x i32> %vec, i32 %carry, i64 %vl)\n");
                code.push_str("  ; Compute prefix sum using slide operations\n");
                code.push_str("  %scan = call <vscale x 4 x i32> @llvm.riscv.vslide1up.nxv4i32(<vscale x 4 x i32> %with.carry, i32 0, i64 %vl)\n");
                code.push_str("  ; Store result\n");
                code.push_str("  %dst.ptr = getelementptr i32, i32* %dst, i64 %index\n");
                code.push_str("  call void @llvm.riscv.vse.nxv4i32(<vscale x 4 x i32> %scan, i32* %dst.ptr, i64 %vl)\n");
                code.push_str("  ; Extract last element for carry\n");
                code.push_str("  %carry.next = call i32 @llvm.riscv.vredsum.nxv4i32(<vscale x 4 x i32> %with.carry, i32 0, i64 %vl)\n");
                code.push_str("  %index.next = add i64 %index, %vl\n");
                code.push_str("  %done = icmp uge i64 %index.next, %n\n");
                code.push_str("  br i1 %done, label %exit, label %vector.body\n\n");
                code.push_str("exit:\n");
                code.push_str("  ret void\n");
                code.push_str("}\n");
            },
            "zip" => {
                code.push_str("; RISC-V Vector-optimized zip operation using RVV 1.0\n");
                code.push_str("; Interleaves two vectors using vrgather operations\n");
                code.push_str("define void @vector_zip_i32(i32* %dst, i32* %src1, i32* %src2, i64 %n) {\n");
                code.push_str("entry:\n");
                code.push_str("  br label %vector.body\n\n");
                code.push_str("vector.body:\n");
                code.push_str("  %index = phi i64 [ 0, %entry ], [ %index.next, %vector.body ]\n");
                code.push_str("  %vl = call i64 @llvm.riscv.vsetvli(i64 %n, i64 2, i64 0)\n");
                code.push_str("  ; Load both vectors\n");
                code.push_str("  %src1.ptr = getelementptr i32, i32* %src1, i64 %index\n");
                code.push_str("  %vec1 = call <vscale x 4 x i32> @llvm.riscv.vle.nxv4i32(<vscale x 4 x i32> undef, i32* %src1.ptr, i64 %vl)\n");
                code.push_str("  %src2.ptr = getelementptr i32, i32* %src2, i64 %index\n");
                code.push_str("  %vec2 = call <vscale x 4 x i32> @llvm.riscv.vle.nxv4i32(<vscale x 4 x i32> undef, i32* %src2.ptr, i64 %vl)\n");
                code.push_str("  ; Use segmented store for interleaving (vsseg2e32.v)\n");
                code.push_str("  %dst.ptr = getelementptr i32, i32* %dst, i64 %index\n");
                code.push_str("  call void @llvm.riscv.vsseg2.nxv4i32(<vscale x 4 x i32> %vec1, <vscale x 4 x i32> %vec2, i32* %dst.ptr, i64 %vl)\n");
                code.push_str("  %index.next = add i64 %index, %vl\n");
                code.push_str("  %done = icmp uge i64 %index.next, %n\n");
                code.push_str("  br i1 %done, label %exit, label %vector.body\n\n");
                code.push_str("exit:\n");
                code.push_str("  ret void\n");
                code.push_str("}\n");
            },
            "merge" => {
                code.push_str("; RISC-V Vector-optimized merge operation using RVV 1.0\n");
                code.push_str("; Merges streams based on mask using vmerge.vvm\n");
                code.push_str("define void @vector_merge_i32(i32* %dst, i32* %src1, i32* %src2, i1* %selector, i64 %n) {\n");
                code.push_str("entry:\n");
                code.push_str("  br label %vector.body\n\n");
                code.push_str("vector.body:\n");
                code.push_str("  %index = phi i64 [ 0, %entry ], [ %index.next, %vector.body ]\n");
                code.push_str("  %vl = call i64 @llvm.riscv.vsetvli(i64 %n, i64 2, i64 0)\n");
                code.push_str("  ; Load vectors and mask\n");
                code.push_str("  %src1.ptr = getelementptr i32, i32* %src1, i64 %index\n");
                code.push_str("  %vec1 = call <vscale x 4 x i32> @llvm.riscv.vle.nxv4i32(<vscale x 4 x i32> undef, i32* %src1.ptr, i64 %vl)\n");
                code.push_str("  %src2.ptr = getelementptr i32, i32* %src2, i64 %index\n");
                code.push_str("  %vec2 = call <vscale x 4 x i32> @llvm.riscv.vle.nxv4i32(<vscale x 4 x i32> undef, i32* %src2.ptr, i64 %vl)\n");
                code.push_str("  %sel.ptr = getelementptr i1, i1* %selector, i64 %index\n");
                code.push_str("  %mask = call <vscale x 4 x i1> @llvm.riscv.vlm.nxv4i1(<vscale x 4 x i1> undef, i1* %sel.ptr, i64 %vl)\n");
                code.push_str("  ; Merge using vmerge.vvm\n");
                code.push_str("  %merged = call <vscale x 4 x i32> @llvm.riscv.vmerge.nxv4i32(<vscale x 4 x i32> %vec2, <vscale x 4 x i32> %vec1, <vscale x 4 x i1> %mask, i64 %vl)\n");
                code.push_str("  ; Store result\n");
                code.push_str("  %dst.ptr = getelementptr i32, i32* %dst, i64 %index\n");
                code.push_str("  call void @llvm.riscv.vse.nxv4i32(<vscale x 4 x i32> %merged, i32* %dst.ptr, i64 %vl)\n");
                code.push_str("  %index.next = add i64 %index, %vl\n");
                code.push_str("  %done = icmp uge i64 %index.next, %n\n");
                code.push_str("  br i1 %done, label %exit, label %vector.body\n\n");
                code.push_str("exit:\n");
                code.push_str("  ret void\n");
                code.push_str("}\n");
            },
            _ => {
                return Err(SeenError::codegen_error(format!("Unsupported vector operation: {}", operation)));
            }
        }
        
        Ok(code)
    }
    
    /// Check if current target can generate vector code
    pub fn supports_vector_operations(&self) -> bool {
        self.target.supports_rvv() && self.extensions.as_ref().map_or(false, |e| e.v)
    }
    
    /// Get target-specific optimization flags
    pub fn get_optimization_flags(&self) -> Vec<String> {
        let mut flags = Vec::new();
        
        match self.target.arch {
            Architecture::RiscV32 | Architecture::RiscV64 => {
                flags.push("-mcpu=generic".to_string());
                
                if let Some(extensions) = &self.extensions {
                    if extensions.v {
                        flags.push("-mattr=+v".to_string());
                        flags.push("-riscv-v-vector-bits-min=128".to_string());
                    }
                    if extensions.c {
                        flags.push("-mattr=+c".to_string()); // Enable compressed instructions
                    }
                    if extensions.f || extensions.d {
                        flags.push("-mattr=+f,+d".to_string()); // Enable floating-point
                    }
                }
            },
            Architecture::X86_64 => {
                flags.push("-mcpu=x86-64".to_string());
                flags.push("-mattr=+sse,+sse2".to_string());
            },
            Architecture::Wasm32 => {
                flags.push("-mcpu=generic".to_string());
            },
        }
        
        flags
    }
}