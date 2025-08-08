//! RISC-V target triple and code generation tests

use seen_ir::ir::{Target, Architecture, OperatingSystem, Environment, RiscVExtensions};
use seen_ir::llvm_backend::LLVMBackend;
use seen_ir::codegen::CodeGenerator;

#[test]
fn test_riscv64_linux_target() {
    let target = Target::riscv64_linux();
    assert_eq!(target.arch, Architecture::RiscV64);
    assert_eq!(target.vendor, "unknown");
    assert_eq!(target.os, OperatingSystem::Linux);
    assert_eq!(target.env, Environment::Gnu);
    assert_eq!(target.to_llvm_triple(), "riscv64-unknown-linux-gnu");
    assert!(target.is_riscv());
    assert!(target.supports_rvv());
    assert_eq!(target.register_size(), 64);
}

#[test]
fn test_riscv32_linux_target() {
    let target = Target::riscv32_linux();
    assert_eq!(target.arch, Architecture::RiscV32);
    assert_eq!(target.vendor, "unknown");
    assert_eq!(target.os, OperatingSystem::Linux);
    assert_eq!(target.env, Environment::Gnu);
    assert_eq!(target.to_llvm_triple(), "riscv32-unknown-linux-gnu");
    assert!(target.is_riscv());
    assert!(target.supports_rvv());
    assert_eq!(target.register_size(), 32);
}

#[test]
fn test_riscv64_bare_metal_target() {
    let target = Target::riscv64_bare_metal();
    assert_eq!(target.arch, Architecture::RiscV64);
    assert_eq!(target.vendor, "unknown");
    assert_eq!(target.os, OperatingSystem::None);
    assert_eq!(target.env, Environment::None);
    assert_eq!(target.to_llvm_triple(), "riscv64-unknown-none-");
    assert!(target.is_riscv());
    assert!(target.supports_rvv());
    assert_eq!(target.register_size(), 64);
}

#[test]
fn test_riscv32_bare_metal_target() {
    let target = Target::riscv32_bare_metal();
    assert_eq!(target.arch, Architecture::RiscV32);
    assert_eq!(target.vendor, "unknown");
    assert_eq!(target.os, OperatingSystem::None);
    assert_eq!(target.env, Environment::None);
    assert_eq!(target.to_llvm_triple(), "riscv32-unknown-none-");
    assert!(target.is_riscv());
    assert!(target.supports_rvv());
    assert_eq!(target.register_size(), 32);
}

#[test]
fn test_target_triple_parsing() {
    let target = Target::from_llvm_triple("riscv64-unknown-linux-gnu").unwrap();
    assert_eq!(target.arch, Architecture::RiscV64);
    assert_eq!(target.vendor, "unknown");
    assert_eq!(target.os, OperatingSystem::Linux);
    assert_eq!(target.env, Environment::Gnu);
    
    let target32 = Target::from_llvm_triple("riscv32-unknown-none").unwrap();
    assert_eq!(target32.arch, Architecture::RiscV32);
    assert_eq!(target32.vendor, "unknown");
    assert_eq!(target32.os, OperatingSystem::None);
    assert_eq!(target32.env, Environment::None);
}

#[test]
fn test_architecture_string_parsing() {
    // Test various RISC-V architecture string formats
    assert_eq!(Architecture::from_str("riscv64"), Some(Architecture::RiscV64));
    assert_eq!(Architecture::from_str("rv64i"), Some(Architecture::RiscV64));
    assert_eq!(Architecture::from_str("rv64imafdc"), Some(Architecture::RiscV64));
    assert_eq!(Architecture::from_str("riscv64imafdc"), Some(Architecture::RiscV64));
    
    assert_eq!(Architecture::from_str("riscv32"), Some(Architecture::RiscV32));
    assert_eq!(Architecture::from_str("rv32i"), Some(Architecture::RiscV32));
    assert_eq!(Architecture::from_str("rv32imafdc"), Some(Architecture::RiscV32));
    assert_eq!(Architecture::from_str("riscv32imafdc"), Some(Architecture::RiscV32));
    
    assert_eq!(Architecture::from_str("x86_64"), Some(Architecture::X86_64));
    assert_eq!(Architecture::from_str("wasm32"), Some(Architecture::Wasm32));
    
    assert_eq!(Architecture::from_str("unknown"), None);
}

#[test]
fn test_riscv_extensions_basic() {
    let rv32i = RiscVExtensions::rv32i();
    assert!(rv32i.i);
    assert!(!rv32i.m);
    assert!(!rv32i.a);
    assert!(!rv32i.f);
    assert!(!rv32i.d);
    assert!(!rv32i.c);
    assert!(!rv32i.v);
    assert!(rv32i.is_valid());
    assert_eq!(rv32i.to_isa_string(32), "rv32i");
    
    let rv64i = RiscVExtensions::rv64i();
    assert!(rv64i.i);
    assert!(!rv64i.m);
    assert!(!rv64i.a);
    assert!(!rv64i.f);
    assert!(!rv64i.d);
    assert!(!rv64i.c);
    assert!(!rv64i.v);
    assert!(rv64i.is_valid());
    assert_eq!(rv64i.to_isa_string(64), "rv64i");
}

#[test]
fn test_riscv_extensions_full() {
    let rv32gc = RiscVExtensions::rv32gc();
    assert!(rv32gc.i);
    assert!(rv32gc.m);
    assert!(rv32gc.a);
    assert!(rv32gc.f);
    assert!(rv32gc.d);
    assert!(rv32gc.c);
    assert!(!rv32gc.v);
    assert!(rv32gc.is_valid());
    assert_eq!(rv32gc.to_isa_string(32), "rv32imafdc");
    
    let rv64gc = RiscVExtensions::rv64gc();
    assert!(rv64gc.i);
    assert!(rv64gc.m);
    assert!(rv64gc.a);
    assert!(rv64gc.f);
    assert!(rv64gc.d);
    assert!(rv64gc.c);
    assert!(!rv64gc.v);
    assert!(rv64gc.is_valid());
    assert_eq!(rv64gc.to_isa_string(64), "rv64imafdc");
}

#[test]
fn test_riscv_extensions_vector() {
    let rv32gcv = RiscVExtensions::rv32gcv();
    assert!(rv32gcv.i);
    assert!(rv32gcv.m);
    assert!(rv32gcv.a);
    assert!(rv32gcv.f);
    assert!(rv32gcv.d);
    assert!(rv32gcv.c);
    assert!(rv32gcv.v);
    assert!(rv32gcv.is_valid());
    assert_eq!(rv32gcv.to_isa_string(32), "rv32imafdcv");
    
    let rv64gcv = RiscVExtensions::rv64gcv();
    assert!(rv64gcv.i);
    assert!(rv64gcv.m);
    assert!(rv64gcv.a);
    assert!(rv64gcv.f);
    assert!(rv64gcv.d);
    assert!(rv64gcv.c);
    assert!(rv64gcv.v);
    assert!(rv64gcv.is_valid());
    assert_eq!(rv64gcv.to_isa_string(64), "rv64imafdcv");
}

#[test]
fn test_riscv_extensions_validation() {
    let mut invalid_ext = RiscVExtensions::rv32i();
    invalid_ext.i = false; // Invalid: no base integer ISA
    assert!(!invalid_ext.is_valid());
    
    let mut invalid_fd = RiscVExtensions::rv32i();
    invalid_fd.d = true; // Invalid: D without F
    assert!(!invalid_fd.is_valid());
}

#[test]
fn test_llvm_backend_riscv64() {
    let target = Target::riscv64_linux();
    let backend = LLVMBackend::with_target("test_module".to_string(), target);
    let llvm_ir = backend.generate().expect("Failed to generate LLVM IR");
    
    assert!(llvm_ir.contains("target triple = \"riscv64-unknown-linux-gnu\""));
    assert!(llvm_ir.contains("target datalayout = \"e-m:e-p:64:64-i64:64-i128:128-n64-S128\""));
    assert!(llvm_ir.contains("!llvm.module.flags"));
    assert!(llvm_ir.contains("riscv-isa"));
    assert!(llvm_ir.contains("rv64imafdc"));
}

#[test]
fn test_llvm_backend_riscv32() {
    let target = Target::riscv32_linux();
    let backend = LLVMBackend::with_target("test_module".to_string(), target);
    let llvm_ir = backend.generate().expect("Failed to generate LLVM IR");
    
    assert!(llvm_ir.contains("target triple = \"riscv32-unknown-linux-gnu\""));
    assert!(llvm_ir.contains("target datalayout = \"e-m:e-p:32:32-i64:64-n32-S128\""));
    assert!(llvm_ir.contains("!llvm.module.flags"));
    assert!(llvm_ir.contains("riscv-isa"));
    assert!(llvm_ir.contains("rv32imafdc"));
}

#[test]
fn test_llvm_backend_riscv_vector() {
    let target = Target::riscv64_linux();
    let extensions = RiscVExtensions::rv64gcv();
    let backend = LLVMBackend::with_target("test_module".to_string(), target)
        .with_riscv_extensions(extensions);
    
    assert!(backend.supports_vector_operations());
    
    let llvm_ir = backend.generate().expect("Failed to generate LLVM IR");
    assert!(llvm_ir.contains("rv64imafdcv"));
    assert!(llvm_ir.contains("+v"));
    assert!(llvm_ir.contains("+zvl128b"));
    
    // Test vector-optimized reactive operations
    let map_code = backend.generate_vector_optimized_reactive("map")
        .expect("Failed to generate vector map");
    assert!(map_code.contains("<vscale x 4 x i32>"));
    assert!(map_code.contains("@vector_map"));
    
    let filter_code = backend.generate_vector_optimized_reactive("filter")
        .expect("Failed to generate vector filter");
    assert!(filter_code.contains("<vscale x 4 x i1>"));
    assert!(filter_code.contains("@vector_filter"));
    
    let reduce_code = backend.generate_vector_optimized_reactive("reduce")
        .expect("Failed to generate vector reduce");
    // RISC-V uses specific vector reduction intrinsics
    assert!(reduce_code.contains("@llvm.riscv.vredsum"));
    assert!(reduce_code.contains("@vector_reduce"));
}

#[test]
fn test_llvm_backend_optimization_flags() {
    let target = Target::riscv64_linux();
    let extensions = RiscVExtensions::rv64gcv();
    let backend = LLVMBackend::with_target("test_module".to_string(), target)
        .with_riscv_extensions(extensions);
    
    let flags = backend.get_optimization_flags();
    assert!(flags.contains(&"-mcpu=generic".to_string()));
    assert!(flags.contains(&"-mattr=+v".to_string()));
    assert!(flags.contains(&"-riscv-v-vector-bits-min=128".to_string()));
    assert!(flags.contains(&"-mattr=+c".to_string()));
    assert!(flags.contains(&"-mattr=+f,+d".to_string()));
}

#[test]
fn test_codegen_riscv_integration() {
    let target = Target::riscv64_linux();
    let extensions = RiscVExtensions::rv64gc();
    let mut codegen = CodeGenerator::new("riscv_test".to_string());
    
    let result = codegen.generate_riscv(target, extensions);
    assert!(result.is_ok());
    
    let llvm_ir = result.unwrap();
    assert!(llvm_ir.contains("target triple = \"riscv64-unknown-linux-gnu\""));
    assert!(llvm_ir.contains("define i32 @main()"));
    assert!(llvm_ir.contains("ret i32 0"));
}

#[test]
fn test_codegen_riscv_vector_features() {
    let target = Target::riscv32_linux();
    let extensions = RiscVExtensions::rv32gcv();
    let mut codegen = CodeGenerator::new("riscv_vector_test".to_string());
    
    let result = codegen.generate_riscv(target, extensions);
    assert!(result.is_ok());
    
    let llvm_ir = result.unwrap();
    assert!(llvm_ir.contains("target triple = \"riscv32-unknown-linux-gnu\""));
    assert!(llvm_ir.contains("RISC-V Vector Extension optimizations enabled"));
    assert!(llvm_ir.contains("target-features"));
    assert!(llvm_ir.contains("+v,+zvl128b"));
    assert!(llvm_ir.contains("rv32imafdcv"));
}

#[test]
fn test_codegen_non_riscv_target_error() {
    let target = Target::x86_64_linux(); // Non-RISC-V target
    let extensions = RiscVExtensions::rv64gc();
    let mut codegen = CodeGenerator::new("x86_test".to_string());
    
    let result = codegen.generate_riscv(target, extensions);
    assert!(result.is_err());
    
    let error_msg = result.err().unwrap().to_string();
    assert!(error_msg.contains("Target is not RISC-V"));
}

#[test]
fn test_unsupported_vector_operation() {
    let target = Target::riscv64_linux();
    let extensions = RiscVExtensions::rv64gcv();
    let backend = LLVMBackend::with_target("test_module".to_string(), target)
        .with_riscv_extensions(extensions);
    
    let result = backend.generate_vector_optimized_reactive("unsupported_op");
    assert!(result.is_err());
    
    let error_msg = result.err().unwrap().to_string();
    assert!(error_msg.contains("Unsupported vector operation: unsupported_op"));
}

#[test]
fn test_vector_operations_without_extension() {
    let target = Target::riscv64_linux();
    let extensions = RiscVExtensions::rv64gc(); // No vector extension
    let backend = LLVMBackend::with_target("test_module".to_string(), target)
        .with_riscv_extensions(extensions);
    
    assert!(!backend.supports_vector_operations());
    
    let result = backend.generate_vector_optimized_reactive("map");
    assert!(result.is_err());
    
    let error_msg = result.err().unwrap().to_string();
    assert!(error_msg.contains("RISC-V Vector extension not enabled"));
}

#[test]
fn test_comprehensive_target_coverage() {
    // Test all supported RISC-V configurations
    let configs = vec![
        (Target::riscv32_linux(), RiscVExtensions::rv32i()),
        (Target::riscv32_linux(), RiscVExtensions::rv32gc()),
        (Target::riscv32_linux(), RiscVExtensions::rv32gcv()),
        (Target::riscv32_bare_metal(), RiscVExtensions::rv32i()),
        (Target::riscv64_linux(), RiscVExtensions::rv64i()),
        (Target::riscv64_linux(), RiscVExtensions::rv64gc()),
        (Target::riscv64_linux(), RiscVExtensions::rv64gcv()),
        (Target::riscv64_bare_metal(), RiscVExtensions::rv64i()),
    ];
    
    for (target, extensions) in configs {
        let backend = LLVMBackend::with_target(format!("test_{}", target.arch), target.clone())
            .with_riscv_extensions(extensions);
        
        let result = backend.generate();
        assert!(result.is_ok(), "Failed to generate code for target: {:?}", target);
        
        let llvm_ir = result.unwrap();
        assert!(llvm_ir.contains(&target.to_llvm_triple()));
        assert!(llvm_ir.contains("define i32 @main()"));
        assert!(llvm_ir.contains("ret i32 0"));
        
        if extensions.v {
            assert!(backend.supports_vector_operations());
        } else {
            assert!(!backend.supports_vector_operations());
        }
    }
}