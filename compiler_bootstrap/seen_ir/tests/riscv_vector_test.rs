//! Tests for RISC-V Vector Extension (RVV 1.0) support
//! Verifies vector-optimized reactive operations

use seen_ir::ir::{Target, RiscVExtensions};
use seen_ir::llvm_backend::LLVMBackend;

#[test]
fn test_riscv_vector_backend_support() {
    let target = Target::riscv64_linux();
    let extensions = RiscVExtensions { i: true, m: true, a: true, f: true, d: true, c: true, v: true };
    let backend = LLVMBackend::with_target("vector_backend".to_string(), target)
        .with_riscv_extensions(extensions);
    
    // Verify vector support is enabled
    assert!(backend.supports_vector_operations());
    
    // Verify optimization flags include vector support
    let flags = backend.get_optimization_flags();
    assert!(flags.iter().any(|f| f.contains("+v")));
    assert!(flags.iter().any(|f| f.contains("-riscv-v-vector-bits-min")));
}

#[test]
fn test_riscv_vector_reactive_map() {
    let target = Target::riscv64_linux();
    let extensions = RiscVExtensions { i: true, m: true, a: true, f: true, d: true, c: true, v: true };
    let backend = LLVMBackend::with_target("reactive_map".to_string(), target)
        .with_riscv_extensions(extensions);
    
    let map_code = backend.generate_vector_optimized_reactive("map")
        .expect("Failed to generate vector map");
    
    // Verify map operation uses vector instructions
    assert!(map_code.contains("@vector_map_i32"));
    assert!(map_code.contains("@vector_map_f32"));
    assert!(map_code.contains("@llvm.riscv.vsetvli"));
    assert!(map_code.contains("@llvm.riscv.vle.nxv4i32"));
    assert!(map_code.contains("@llvm.riscv.vadd.nxv4i32"));
    assert!(map_code.contains("@llvm.riscv.vse.nxv4i32"));
    assert!(map_code.contains("vector.body:"));
    assert!(map_code.contains("vscale x 4"));
    
    // Verify float version
    assert!(map_code.contains("@llvm.riscv.vle.nxv4f32"));
    assert!(map_code.contains("@llvm.riscv.vfadd.nxv4f32"));
    assert!(map_code.contains("@llvm.riscv.vse.nxv4f32"));
}

#[test]
fn test_riscv_vector_reactive_filter() {
    let target = Target::riscv32_linux();
    let extensions = RiscVExtensions { i: true, m: false, a: false, f: false, d: false, c: false, v: true };
    let backend = LLVMBackend::with_target("reactive_filter".to_string(), target)
        .with_riscv_extensions(extensions);
    
    let filter_code = backend.generate_vector_optimized_reactive("filter")
        .expect("Failed to generate vector filter");
    
    // Verify filter operation uses mask and compress
    assert!(filter_code.contains("@vector_filter_i32"));
    assert!(filter_code.contains("@llvm.riscv.vmslt.nxv4i32"));
    assert!(filter_code.contains("@llvm.riscv.vcompress.nxv4i32"));
    assert!(filter_code.contains("@llvm.riscv.vcpop"));
    assert!(filter_code.contains("vscale x 4 x i1"));
    assert!(filter_code.contains("threshold"));
}

#[test]
fn test_riscv_vector_reactive_reduce() {
    let target = Target::riscv64_linux();
    let extensions = RiscVExtensions::rv64gcv(); // Full ISA with vector
    let backend = LLVMBackend::with_target("reactive_reduce".to_string(), target)
        .with_riscv_extensions(extensions);
    
    let reduce_code = backend.generate_vector_optimized_reactive("reduce")
        .expect("Failed to generate vector reduce");
    
    // Verify reduction operations
    assert!(reduce_code.contains("@vector_reduce_sum_i32"));
    assert!(reduce_code.contains("@vector_reduce_max_i32"));
    assert!(reduce_code.contains("@llvm.riscv.vredsum.nxv4i32"));
    assert!(reduce_code.contains("@llvm.riscv.vredmax.nxv4i32"));
    assert!(reduce_code.contains("vector.body:"));
    assert!(reduce_code.contains("phi i32"));
}

#[test]
fn test_riscv_vector_reactive_scan() {
    let target = Target::riscv64_linux();
    let extensions = RiscVExtensions { i: true, m: true, a: false, f: false, d: false, c: false, v: true };
    let backend = LLVMBackend::with_target("reactive_scan".to_string(), target)
        .with_riscv_extensions(extensions);
    
    let scan_code = backend.generate_vector_optimized_reactive("scan")
        .expect("Failed to generate vector scan");
    
    // Verify scan (prefix sum) operation
    assert!(scan_code.contains("@vector_scan_i32"));
    assert!(scan_code.contains("@llvm.riscv.vadd.vx.nxv4i32"));
    assert!(scan_code.contains("@llvm.riscv.vslide1up.nxv4i32"));
    assert!(scan_code.contains("carry"));
    assert!(scan_code.contains("prefix sum"));
}

#[test]
fn test_riscv_vector_reactive_zip() {
    let target = Target::riscv32_linux();
    let extensions = RiscVExtensions { i: true, m: false, a: false, f: false, d: false, c: false, v: true };
    let backend = LLVMBackend::with_target("reactive_zip".to_string(), target)
        .with_riscv_extensions(extensions);
    
    let zip_code = backend.generate_vector_optimized_reactive("zip")
        .expect("Failed to generate vector zip");
    
    // Verify zip operation uses segmented store
    assert!(zip_code.contains("@vector_zip_i32"));
    assert!(zip_code.contains("@llvm.riscv.vsseg2.nxv4i32"));
    assert!(zip_code.contains("Interleaves two vectors"));
    assert!(zip_code.contains("Load both vectors"));
}

#[test]
fn test_riscv_vector_reactive_merge() {
    let target = Target::riscv64_linux();
    let extensions = RiscVExtensions { i: true, m: false, a: false, f: false, d: false, c: false, v: true };
    let backend = LLVMBackend::with_target("reactive_merge".to_string(), target)
        .with_riscv_extensions(extensions);
    
    let merge_code = backend.generate_vector_optimized_reactive("merge")
        .expect("Failed to generate vector merge");
    
    // Verify merge operation uses vmerge
    assert!(merge_code.contains("@vector_merge_i32"));
    assert!(merge_code.contains("@llvm.riscv.vmerge.nxv4i32"));
    assert!(merge_code.contains("@llvm.riscv.vlm.nxv4i1"));
    assert!(merge_code.contains("selector"));
}

#[test]
fn test_riscv_vector_unsupported_operation() {
    let target = Target::riscv64_linux();
    let extensions = RiscVExtensions { i: true, m: false, a: false, f: false, d: false, c: false, v: true };
    let backend = LLVMBackend::with_target("unsupported".to_string(), target)
        .with_riscv_extensions(extensions);
    
    // Test that unsupported operations return appropriate errors
    let result = backend.generate_vector_optimized_reactive("unsupported_op");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Unsupported vector operation"));
}

#[test]
fn test_riscv_vector_disabled() {
    let target = Target::riscv64_linux();
    let extensions = RiscVExtensions { i: true, m: true, a: true, f: true, d: true, c: true, v: false }; // Vector disabled
    let backend = LLVMBackend::with_target("no_vector".to_string(), target)
        .with_riscv_extensions(extensions);
    
    // Test that vector operations fail when extension is disabled
    let result = backend.generate_vector_optimized_reactive("map");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("RISC-V Vector extension not enabled"));
    
    // Verify vector support check returns false
    assert!(!backend.supports_vector_operations());
}

#[test]
fn test_riscv_vector_optimization_flags() {
    // Test RV32 with vector
    let target32 = Target::riscv32_linux();
    let extensions32 = RiscVExtensions { i: true, m: true, a: false, f: true, d: true, c: true, v: true };
    let backend32 = LLVMBackend::with_target("rv32_vector".to_string(), target32)
        .with_riscv_extensions(extensions32);
    
    let flags32 = backend32.get_optimization_flags();
    assert!(flags32.iter().any(|f| f == "-mcpu=generic"));
    assert!(flags32.iter().any(|f| f.contains("+v")));
    assert!(flags32.iter().any(|f| f.contains("+c")));
    assert!(flags32.iter().any(|f| f.contains("+f,+d")));
    
    // Test RV64 with vector
    let target64 = Target::riscv64_linux();
    let extensions64 = RiscVExtensions::rv64gcv();
    let backend64 = LLVMBackend::with_target("rv64_vector".to_string(), target64)
        .with_riscv_extensions(extensions64);
    
    let flags64 = backend64.get_optimization_flags();
    assert!(flags64.iter().any(|f| f == "-mcpu=generic"));
    assert!(flags64.iter().any(|f| f.contains("+v")));
}

#[test]
fn test_riscv_vector_all_reactive_operations() {
    let target = Target::riscv64_linux();
    let extensions = RiscVExtensions::rv64gcv();
    let backend = LLVMBackend::with_target("all_reactive".to_string(), target)
        .with_riscv_extensions(extensions);
    
    // Test all supported reactive operations
    let operations = vec!["map", "filter", "reduce", "scan", "zip", "merge"];
    
    for op in operations {
        let result = backend.generate_vector_optimized_reactive(op);
        assert!(result.is_ok(), "Failed to generate {} operation", op);
        
        let code = result.unwrap();
        assert!(!code.is_empty(), "{} operation generated empty code", op);
        assert!(code.contains("RISC-V Vector-optimized"), "{} missing optimization comment", op);
        assert!(code.contains("@llvm.riscv"), "{} missing RISC-V intrinsics", op);
    }
}

#[test]
fn test_riscv_vector_bare_metal_target() {
    let target = Target::riscv32_bare_metal();
    let extensions = RiscVExtensions { i: true, m: false, a: false, f: false, d: false, c: false, v: true };
    let backend = LLVMBackend::with_target("bare_metal_vector".to_string(), target)
        .with_riscv_extensions(extensions);
    
    // Verify vector operations work on bare metal targets
    assert!(backend.supports_vector_operations());
    
    let map_code = backend.generate_vector_optimized_reactive("map")
        .expect("Failed to generate vector map for bare metal");
    
    assert!(map_code.contains("@vector_map_i32"));
    assert!(map_code.contains("@llvm.riscv.vsetvli"));
}

#[test]
fn test_riscv_vector_performance_characteristics() {
    let target = Target::riscv64_linux();
    let extensions = RiscVExtensions::rv64gcv();
    let backend = LLVMBackend::with_target("performance".to_string(), target)
        .with_riscv_extensions(extensions);
    
    // Generate reduce operation and verify it uses efficient patterns
    let reduce_code = backend.generate_vector_optimized_reactive("reduce")
        .expect("Failed to generate reduce");
    
    // Verify loop structure for efficiency
    assert!(reduce_code.contains("br label %vector.body"));
    assert!(reduce_code.contains("phi i64")); // Loop counter
    assert!(reduce_code.contains("phi i32")); // Accumulator
    assert!(reduce_code.contains("icmp uge")); // Loop condition
    assert!(reduce_code.contains("br i1 %done, label %exit, label %vector.body"));
    
    // Verify memory access patterns
    assert!(reduce_code.contains("getelementptr"));
}

#[test]
fn test_comprehensive_riscv_vector_reactive() {
    let target = Target::riscv64_linux();
    let extensions = RiscVExtensions::rv64gcv();
    let backend = LLVMBackend::with_target("comprehensive_reactive".to_string(), target)
        .with_riscv_extensions(extensions);
    
    // Test that we can generate a complete reactive pipeline
    let map_code = backend.generate_vector_optimized_reactive("map").unwrap();
    let filter_code = backend.generate_vector_optimized_reactive("filter").unwrap();
    let reduce_code = backend.generate_vector_optimized_reactive("reduce").unwrap();
    
    // Verify pipeline can be composed (map -> filter -> reduce)
    assert!(map_code.contains("vector_map_i32"));
    assert!(filter_code.contains("vector_filter_i32"));
    assert!(reduce_code.contains("vector_reduce_sum_i32"));
    
    // Verify compatible signatures for chaining
    assert!(map_code.contains("i32* %dst, i32* %src"));
    assert!(filter_code.contains("i32* %dst, i32* %src"));
    assert!(reduce_code.contains("i32* %src"));
    
    println!("✓ Comprehensive RISC-V Vector Extension (RVV 1.0) reactive operations test passed");
}