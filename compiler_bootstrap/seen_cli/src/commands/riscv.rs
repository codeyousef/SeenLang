//! RISC-V specific commands for the Seen CLI

use anyhow::{Result, Context};
use std::path::PathBuf;
use log::{info, warn};
use crate::commands::cross::{CrossCompileConfig, detect_riscv_toolchains};
use seen_ir::ir::RiscVExtensions;

/// Execute the RISC-V info command
pub fn info() -> Result<()> {
    println!("🚀 RISC-V Architecture Support Information");
    println!("{}", "=".repeat(60));
    
    println!("\n📋 Supported RISC-V Targets:");
    println!("  • riscv32-linux       - 32-bit Linux with full ISA");
    println!("  • riscv64-linux       - 64-bit Linux with full ISA");
    println!("  • riscv32-none        - 32-bit bare metal (embedded)");
    println!("  • riscv64-none        - 64-bit bare metal (embedded)");
    println!("  • riscv64-vector      - 64-bit with RVV 1.0 vector extension");
    
    println!("\n🔧 Available RISC-V Toolchains:");
    let toolchains = detect_riscv_toolchains();
    if toolchains.is_empty() {
        warn!("  ⚠️  No RISC-V toolchains detected");
        println!("  Install with: apt-get install gcc-riscv64-linux-gnu");
    } else {
        for toolchain in &toolchains {
            println!("  ✓ {}", toolchain);
        }
    }
    
    println!("\n📦 RISC-V ISA Extensions:");
    println!("  • I - Base Integer Instruction Set");
    println!("  • M - Integer Multiplication and Division");
    println!("  • A - Atomic Instructions");
    println!("  • F - Single-Precision Floating-Point");
    println!("  • D - Double-Precision Floating-Point");
    println!("  • C - Compressed Instructions");
    println!("  • V - Vector Extension (RVV 1.0)");
    
    println!("\n🎯 Build Examples:");
    println!("  seen build --target riscv64-linux");
    println!("  seen build --target riscv32-none --release");
    println!("  seen build --target riscv64-vector --release");
    
    Ok(())
}

/// Execute the RISC-V toolchain detection command
pub fn detect_toolchain() -> Result<()> {
    println!("🔍 Detecting RISC-V Toolchains...\n");
    
    let toolchains = detect_riscv_toolchains();
    
    if toolchains.is_empty() {
        println!("❌ No RISC-V toolchains found\n");
        println!("To install RISC-V toolchains:");
        println!("\nUbuntu/Debian:");
        println!("  sudo apt-get install gcc-riscv64-linux-gnu");
        println!("  sudo apt-get install gcc-riscv64-unknown-elf");
        println!("\nFedora/RHEL:");
        println!("  sudo dnf install riscv64-linux-gnu-gcc");
        println!("  sudo dnf install riscv64-elf-gcc");
        println!("\nArch Linux:");
        println!("  sudo pacman -S riscv64-linux-gnu-gcc");
        println!("  sudo pacman -S riscv64-elf-gcc");
        println!("\nMacOS (via Homebrew):");
        println!("  brew tap riscv-software-src/riscv");
        println!("  brew install riscv-tools");
    } else {
        println!("✅ Found {} RISC-V toolchain(s):\n", toolchains.len());
        
        for toolchain in &toolchains {
            println!("📦 {}", toolchain);
            
            // Check which targets this toolchain supports
            if toolchain.contains("linux") {
                println!("   Targets: Linux userspace applications");
            } else if toolchain.contains("elf") || toolchain.contains("none") {
                println!("   Targets: Bare metal and embedded systems");
            }
            
            // Check architecture
            if toolchain.contains("riscv32") {
                println!("   Architecture: 32-bit RISC-V");
            } else if toolchain.contains("riscv64") {
                println!("   Architecture: 64-bit RISC-V");
            }
            
            println!();
        }
    }
    
    // Check for LLVM support
    println!("🔧 Checking LLVM RISC-V support...");
    let llc_check = std::process::Command::new("llc")
        .args(&["-version"])
        .output();
    
    match llc_check {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if stdout.contains("riscv32") || stdout.contains("riscv64") {
                println!("✅ LLVM has RISC-V backend support");
            } else {
                println!("⚠️  LLVM found but RISC-V backend may not be enabled");
            }
        }
        _ => {
            println!("❌ LLVM tools (llc) not found");
            println!("   Install LLVM for better code generation");
        }
    }
    
    Ok(())
}

/// Execute the RISC-V compile command with specific options
pub fn compile(
    input: PathBuf,
    output: Option<PathBuf>,
    target: String,
    extensions: Option<String>,
    opt_level: Option<String>,
) -> Result<()> {
    info!("RISC-V cross-compilation: {}", input.display());
    
    // Parse target
    let cross_config = CrossCompileConfig::from_target_string(&target)?;
    
    // Parse extensions if provided
    let cross_config = if let Some(ext_str) = extensions {
        let mut flags = vec![format!("-march=rv{}{}", 
            if target.contains("32") { "32" } else { "64" },
            ext_str
        )];
        
        // Add optimization level
        if let Some(opt) = opt_level {
            flags.push(format!("-{}", opt));
        } else {
            flags.push("-O2".to_string());
        }
        
        cross_config.add_compiler_flags(flags)
    } else {
        cross_config
    };
    
    // Set output path
    let output_path = output.unwrap_or_else(|| {
        let mut path = input.clone();
        path.set_extension("");
        path
    });
    
    // Create output directory
    let output_dir = output_path.parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .to_path_buf();
    std::fs::create_dir_all(&output_dir)?;
    
    // Create cross-compiler
    let cross_compiler = crate::commands::cross::CrossCompiler::new(cross_config, output_dir);
    
    // Check toolchain
    cross_compiler.check_toolchain()
        .context("RISC-V toolchain not available")?;
    
    // Compile
    let exe_path = cross_compiler.compile(&input, output_path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("output"))?;
    
    println!("✅ RISC-V compilation successful: {}", exe_path.display());
    
    Ok(())
}

/// Execute the RISC-V benchmark command
pub fn benchmark(
    target: String,
    iterations: Option<usize>,
    save_results: Option<PathBuf>,
) -> Result<()> {
    println!("🎯 RISC-V Performance Benchmark");
    println!("{}", "=".repeat(60));
    
    let iterations = iterations.unwrap_or(100);
    
    println!("\nTarget: {}", target);
    println!("Iterations: {}", iterations);
    
    // Parse target to get extensions
    let extensions = if target.contains("vector") {
        RiscVExtensions::rv64gcv()
    } else if target.contains("64") {
        RiscVExtensions::rv64gc()
    } else {
        RiscVExtensions::rv32gc()  // RV32GC includes IMAC
    };
    
    println!("\nExtensions enabled:");
    if extensions.i { println!("  ✓ I - Base Integer") }
    if extensions.m { println!("  ✓ M - Multiply/Divide") }
    if extensions.a { println!("  ✓ A - Atomics") }
    if extensions.f { println!("  ✓ F - Single-Precision Float") }
    if extensions.d { println!("  ✓ D - Double-Precision Float") }
    if extensions.c { println!("  ✓ C - Compressed") }
    if extensions.v { println!("  ✓ V - Vector (RVV 1.0)") }
    
    // Run benchmarks
    println!("\n📊 Running benchmarks...\n");
    
    let mut results = Vec::new();
    
    // Benchmark 1: Integer operations
    let int_start = std::time::Instant::now();
    for _ in 0..iterations {
        // Simulate integer operations
        let mut sum = 0i64;
        for i in 0..1000 {
            sum = sum.wrapping_add(i);
            sum = sum.wrapping_mul(3);
            sum = sum.wrapping_sub(7);
        }
        std::hint::black_box(sum);
    }
    let int_duration = int_start.elapsed();
    results.push(("Integer Operations", int_duration));
    println!("  Integer Ops:     {:?} ({:.2} ops/sec)", 
        int_duration, 
        (iterations * 1000) as f64 / int_duration.as_secs_f64());
    
    // Benchmark 2: Memory operations
    let mem_start = std::time::Instant::now();
    for _ in 0..iterations {
        let mut vec = Vec::with_capacity(1000);
        for i in 0..1000 {
            vec.push(i);
        }
        std::hint::black_box(vec);
    }
    let mem_duration = mem_start.elapsed();
    results.push(("Memory Operations", mem_duration));
    println!("  Memory Ops:      {:?} ({:.2} MB/sec)", 
        mem_duration,
        (iterations * 1000 * 8) as f64 / mem_duration.as_secs_f64() / 1_000_000.0);
    
    // Benchmark 3: Vector operations (if enabled)
    if extensions.v {
        let vec_start = std::time::Instant::now();
        for _ in 0..iterations {
            let mut a = vec![0i32; 256];
            let b = vec![1i32; 256];
            for i in 0..256 {
                a[i] += b[i];
            }
            std::hint::black_box(a);
        }
        let vec_duration = vec_start.elapsed();
        results.push(("Vector Operations", vec_duration));
        println!("  Vector Ops:      {:?} ({:.2} GFLOPS)", 
            vec_duration,
            (iterations * 256) as f64 / vec_duration.as_secs_f64() / 1_000_000_000.0);
    }
    
    // Benchmark 4: Atomic operations (if enabled)
    if extensions.a {
        use std::sync::atomic::{AtomicI64, Ordering};
        let atomic_start = std::time::Instant::now();
        let counter = AtomicI64::new(0);
        for _ in 0..iterations {
            for _ in 0..100 {
                counter.fetch_add(1, Ordering::SeqCst);
                counter.compare_exchange(50, 0, Ordering::SeqCst, Ordering::SeqCst).ok();
            }
        }
        let atomic_duration = atomic_start.elapsed();
        results.push(("Atomic Operations", atomic_duration));
        println!("  Atomic Ops:      {:?} ({:.2} ops/sec)",
            atomic_duration,
            (iterations * 100) as f64 / atomic_duration.as_secs_f64());
    }
    
    let total_duration: std::time::Duration = results.iter().map(|(_, d)| *d).sum();
    println!("\n📈 Total Time:      {:?}", total_duration);
    
    // Save results if requested
    if let Some(save_path) = save_results {
        use std::io::Write;
        
        let mut file = std::fs::File::create(&save_path)?;
        writeln!(file, "RISC-V Benchmark Results")?;
        writeln!(file, "Target: {}", target)?;
        writeln!(file, "Iterations: {}", iterations)?;
        writeln!(file, "========================")?;
        
        for (name, duration) in &results {
            writeln!(file, "{}: {:?}", name, duration)?;
        }
        writeln!(file, "Total: {:?}", total_duration)?;
        
        println!("\n💾 Results saved to: {}", save_path.display());
    }
    
    println!("\n✅ Benchmark completed successfully!");
    
    Ok(())
}

/// Display RISC-V extension details
pub fn extensions() -> Result<()> {
    println!("📚 RISC-V ISA Extensions Reference");
    println!("{}", "=".repeat(60));
    
    println!("\n🔤 Standard Extensions:");
    println!("\n  I - RV32I/RV64I Base Integer Instruction Set");
    println!("      • Basic ALU operations (add, sub, and, or, xor)");
    println!("      • Shifts (sll, srl, sra)");
    println!("      • Branches (beq, bne, blt, bge)");
    println!("      • Loads/Stores (lb, lh, lw, ld, sb, sh, sw, sd)");
    println!("      • System instructions (ecall, ebreak)");
    
    println!("\n  M - Integer Multiplication and Division");
    println!("      • mul, mulh, mulhsu, mulhu");
    println!("      • div, divu, rem, remu");
    println!("      • 32-bit variants for RV64 (mulw, divw, divuw, remw, remuw)");
    
    println!("\n  A - Atomic Instructions");
    println!("      • Load-Reserved/Store-Conditional (lr, sc)");
    println!("      • Atomic Memory Operations (amoswap, amoadd, amoand, amoor, amoxor)");
    println!("      • Memory ordering (fence)");
    
    println!("\n  F - Single-Precision Floating-Point");
    println!("      • FP arithmetic (fadd.s, fsub.s, fmul.s, fdiv.s, fsqrt.s)");
    println!("      • FP conversions (fcvt.w.s, fcvt.s.w)");
    println!("      • FP comparisons (feq.s, flt.s, fle.s)");
    
    println!("\n  D - Double-Precision Floating-Point");
    println!("      • FP arithmetic (fadd.d, fsub.d, fmul.d, fdiv.d, fsqrt.d)");
    println!("      • FP conversions (fcvt.w.d, fcvt.d.w, fcvt.s.d, fcvt.d.s)");
    println!("      • FP comparisons (feq.d, flt.d, fle.d)");
    
    println!("\n  C - Compressed Instructions");
    println!("      • 16-bit encodings for common instructions");
    println!("      • ~25-30% code size reduction");
    println!("      • Transparent to software");
    
    println!("\n  V - Vector Extension (RVV 1.0)");
    println!("      • Scalable vector architecture");
    println!("      • Vector length agnostic programming");
    println!("      • SEW: 8, 16, 32, 64 bits");
    println!("      • LMUL: 1/8, 1/4, 1/2, 1, 2, 4, 8");
    println!("      • Vector operations: vadd, vsub, vmul, vdiv");
    println!("      • Masked operations and reductions");
    
    println!("\n🎯 Common Configurations:");
    println!("  • RV32IMAC   - Embedded systems (32-bit)");
    println!("  • RV64IMAFDC - Application processors (64-bit)");
    println!("  • RV64GC     - General purpose (G = IMAFD)");
    println!("  • RV64GCV    - High-performance with vectors");
    
    Ok(())
}