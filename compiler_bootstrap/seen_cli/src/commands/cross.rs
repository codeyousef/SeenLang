//! Cross-compilation support for RISC-V and other targets

use std::path::{Path, PathBuf};
use std::process::Command;
use anyhow::{Result, Context};
use seen_ir::ir::{Target, Architecture, OperatingSystem, Environment};

/// Cross-compilation configuration
#[derive(Debug, Clone)]
pub struct CrossCompileConfig {
    pub target: Target,
    pub sysroot: Option<PathBuf>,
    pub toolchain_prefix: String,
    pub linker: String,
    pub compiler_flags: Vec<String>,
    pub linker_flags: Vec<String>,
    pub use_lld: bool,
}

impl CrossCompileConfig {
    /// Create config for RISC-V 32-bit Linux target
    pub fn riscv32_linux() -> Self {
        Self {
            target: Target::riscv32_linux(),
            sysroot: None,
            toolchain_prefix: "riscv32-unknown-linux-gnu".to_string(),
            linker: "riscv32-unknown-linux-gnu-gcc".to_string(),
            compiler_flags: vec![
                "-march=rv32imafdc".to_string(),
                "-mabi=ilp32d".to_string(),
                "-O2".to_string(),
            ],
            linker_flags: vec![
                "-static".to_string(),
                "-nostdlib".to_string(),
            ],
            use_lld: false,
        }
    }
    
    /// Create config for RISC-V 64-bit Linux target
    pub fn riscv64_linux() -> Self {
        Self {
            target: Target::riscv64_linux(),
            sysroot: None,
            toolchain_prefix: "riscv64-unknown-linux-gnu".to_string(),
            linker: "riscv64-unknown-linux-gnu-gcc".to_string(),
            compiler_flags: vec![
                "-march=rv64imafdc".to_string(),
                "-mabi=lp64d".to_string(),
                "-O2".to_string(),
            ],
            linker_flags: vec![
                "-static".to_string(),
                "-nostdlib".to_string(),
            ],
            use_lld: false,
        }
    }
    
    /// Create config for RISC-V 32-bit bare metal target
    pub fn riscv32_bare_metal() -> Self {
        Self {
            target: Target::riscv32_bare_metal(),
            sysroot: None,
            toolchain_prefix: "riscv32-unknown-elf".to_string(),
            linker: "riscv32-unknown-elf-ld".to_string(),
            compiler_flags: vec![
                "-march=rv32imac".to_string(),
                "-mabi=ilp32".to_string(),
                "-Os".to_string(),
                "-ffunction-sections".to_string(),
                "-fdata-sections".to_string(),
            ],
            linker_flags: vec![
                "-nostartfiles".to_string(),
                "-nostdlib".to_string(),
                "-nodefaultlibs".to_string(),
                "--gc-sections".to_string(),
            ],
            use_lld: true,
        }
    }
    
    /// Create config for RISC-V 64-bit bare metal target
    pub fn riscv64_bare_metal() -> Self {
        Self {
            target: Target::riscv64_bare_metal(),
            sysroot: None,
            toolchain_prefix: "riscv64-unknown-elf".to_string(),
            linker: "riscv64-unknown-elf-ld".to_string(),
            compiler_flags: vec![
                "-march=rv64imac".to_string(),
                "-mabi=lp64".to_string(),
                "-Os".to_string(),
                "-ffunction-sections".to_string(),
                "-fdata-sections".to_string(),
            ],
            linker_flags: vec![
                "-nostartfiles".to_string(),
                "-nostdlib".to_string(),
                "-nodefaultlibs".to_string(),
                "--gc-sections".to_string(),
            ],
            use_lld: true,
        }
    }
    
    /// Create config for RISC-V with vector extension
    pub fn riscv64_vector() -> Self {
        Self {
            target: Target::riscv64_linux(),
            sysroot: None,
            toolchain_prefix: "riscv64-unknown-linux-gnu".to_string(),
            linker: "riscv64-unknown-linux-gnu-gcc".to_string(),
            compiler_flags: vec![
                "-march=rv64imafdcv".to_string(),
                "-mabi=lp64d".to_string(),
                "-O3".to_string(),
                "-ftree-vectorize".to_string(),
                "-ffast-math".to_string(),
            ],
            linker_flags: vec![
                "-static".to_string(),
            ],
            use_lld: false,
        }
    }
    
    /// Get config for a target string
    pub fn from_target_string(target: &str) -> Result<Self> {
        match target {
            "riscv32-linux" | "riscv32-unknown-linux-gnu" => Ok(Self::riscv32_linux()),
            "riscv64-linux" | "riscv64-unknown-linux-gnu" => Ok(Self::riscv64_linux()),
            "riscv32-none" | "riscv32-unknown-none" => Ok(Self::riscv32_bare_metal()),
            "riscv64-none" | "riscv64-unknown-none" => Ok(Self::riscv64_bare_metal()),
            "riscv64-vector" | "riscv64-linux-vector" => Ok(Self::riscv64_vector()),
            _ => anyhow::bail!("Unknown cross-compilation target: {}", target),
        }
    }
    
    /// Set custom sysroot path
    pub fn with_sysroot(mut self, path: PathBuf) -> Self {
        self.sysroot = Some(path);
        self
    }
    
    /// Add custom compiler flags
    pub fn add_compiler_flags(mut self, flags: Vec<String>) -> Self {
        self.compiler_flags.extend(flags);
        self
    }
    
    /// Add custom linker flags
    pub fn add_linker_flags(mut self, flags: Vec<String>) -> Self {
        self.linker_flags.extend(flags);
        self
    }
}

/// Cross-compilation toolchain manager
pub struct CrossCompiler {
    config: CrossCompileConfig,
    llvm_path: Option<PathBuf>,
    output_dir: PathBuf,
}

impl CrossCompiler {
    pub fn new(config: CrossCompileConfig, output_dir: PathBuf) -> Self {
        Self {
            config,
            llvm_path: None,
            output_dir,
        }
    }
    
    /// Set custom LLVM installation path
    pub fn with_llvm_path(mut self, path: PathBuf) -> Self {
        self.llvm_path = Some(path);
        self
    }
    
    /// Check if the required toolchain is available
    pub fn check_toolchain(&self) -> Result<()> {
        // Check for LLVM tools
        let llc = self.get_llc_path();
        if !llc.exists() {
            anyhow::bail!("LLVM compiler (llc) not found at: {:?}", llc);
        }
        
        // Check for linker
        let linker_check = Command::new("which")
            .arg(&self.config.linker)
            .output()
            .context("Failed to check for linker")?;
            
        if !linker_check.status.success() {
            anyhow::bail!(
                "Cross-compilation linker '{}' not found. Please install the {} toolchain",
                self.config.linker,
                self.config.toolchain_prefix
            );
        }
        
        Ok(())
    }
    
    /// Get path to LLC (LLVM compiler)
    fn get_llc_path(&self) -> PathBuf {
        if let Some(llvm_path) = &self.llvm_path {
            llvm_path.join("bin").join("llc")
        } else {
            PathBuf::from("llc")
        }
    }
    
    /// Get path to LLD (LLVM linker)
    fn get_lld_path(&self) -> PathBuf {
        if let Some(llvm_path) = &self.llvm_path {
            llvm_path.join("bin").join("ld.lld")
        } else {
            PathBuf::from("ld.lld")
        }
    }
    
    /// Compile LLVM IR to target assembly
    pub fn compile_ir_to_assembly(&self, ir_path: &Path, asm_path: &Path) -> Result<()> {
        let mut cmd = Command::new(self.get_llc_path());
        
        // Set target triple
        cmd.arg("-mtriple").arg(self.config.target.to_llvm_triple());
        
        // Add architecture-specific features
        match self.config.target.arch {
            Architecture::RiscV32 | Architecture::RiscV64 => {
                // Add RISC-V specific attributes
                if self.config.compiler_flags.iter().any(|f| f.contains("v")) {
                    cmd.arg("-mattr=+v,+f,+d,+c,+m,+a");
                } else if self.config.compiler_flags.iter().any(|f| f.contains("d")) {
                    cmd.arg("-mattr=+f,+d,+c,+m,+a");
                } else if self.config.compiler_flags.iter().any(|f| f.contains("c")) {
                    cmd.arg("-mattr=+c,+m");
                }
            }
            _ => {}
        }
        
        // Set optimization level
        if self.config.compiler_flags.iter().any(|f| f.contains("-O3")) {
            cmd.arg("-O3");
        } else if self.config.compiler_flags.iter().any(|f| f.contains("-O2")) {
            cmd.arg("-O2");
        } else if self.config.compiler_flags.iter().any(|f| f.contains("-Os")) {
            cmd.arg("-Os");
        } else {
            cmd.arg("-O1");
        }
        
        // Input and output files
        cmd.arg("-o").arg(asm_path);
        cmd.arg(ir_path);
        
        // Run compilation
        let output = cmd.output()
            .context("Failed to run LLVM compiler")?;
            
        if !output.status.success() {
            anyhow::bail!(
                "LLVM compilation failed:\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        
        Ok(())
    }
    
    /// Assemble and link to create executable
    pub fn link_executable(&self, asm_path: &Path, exe_path: &Path) -> Result<()> {
        let mut cmd = if self.config.use_lld {
            Command::new(self.get_lld_path())
        } else {
            Command::new(&self.config.linker)
        };
        
        // Add sysroot if specified
        if let Some(sysroot) = &self.config.sysroot {
            cmd.arg(format!("--sysroot={}", sysroot.display()));
        }
        
        // Add linker flags
        for flag in &self.config.linker_flags {
            cmd.arg(flag);
        }
        
        // Add output file
        cmd.arg("-o").arg(exe_path);
        
        // Add input assembly file
        cmd.arg(asm_path);
        
        // Add runtime libraries for non-bare-metal targets
        match self.config.target.os {
            OperatingSystem::Linux => {
                if !self.config.linker_flags.contains(&"-nostdlib".to_string()) {
                    cmd.arg("-lc");
                    cmd.arg("-lm");
                }
            }
            _ => {}
        }
        
        // Run linker
        let output = cmd.output()
            .context("Failed to run linker")?;
            
        if !output.status.success() {
            anyhow::bail!(
                "Linking failed:\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        
        Ok(())
    }
    
    /// Complete cross-compilation pipeline
    pub fn compile(&self, ir_path: &Path, output_name: &str) -> Result<PathBuf> {
        // Check toolchain availability
        self.check_toolchain()?;
        
        // Create output directory if needed
        std::fs::create_dir_all(&self.output_dir)
            .context("Failed to create output directory")?;
        
        // Generate paths
        let asm_path = self.output_dir.join(format!("{}.s", output_name));
        let exe_path = self.output_dir.join(output_name);
        
        // Compile IR to assembly
        println!("Compiling LLVM IR to {} assembly...", self.config.target.to_llvm_triple());
        self.compile_ir_to_assembly(ir_path, &asm_path)?;
        
        // Link to executable
        println!("Linking {} executable...", self.config.target.to_llvm_triple());
        self.link_executable(&asm_path, &exe_path)?;
        
        // Clean up intermediate files
        let _ = std::fs::remove_file(&asm_path);
        
        println!("✓ Cross-compilation complete: {}", exe_path.display());
        Ok(exe_path)
    }
}

/// Helper to detect available RISC-V toolchains
pub fn detect_riscv_toolchains() -> Vec<String> {
    let mut toolchains = Vec::new();
    
    let prefixes = [
        "riscv32-unknown-linux-gnu",
        "riscv64-unknown-linux-gnu",
        "riscv32-unknown-elf",
        "riscv64-unknown-elf",
        "riscv32-linux-gnu",
        "riscv64-linux-gnu",
    ];
    
    for prefix in &prefixes {
        let gcc = format!("{}-gcc", prefix);
        if Command::new("which")
            .arg(&gcc)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            toolchains.push(prefix.to_string());
        }
    }
    
    toolchains
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_cross_compile_config_creation() {
        let config = CrossCompileConfig::riscv32_linux();
        assert_eq!(config.target.to_llvm_triple(), "riscv32-unknown-linux-gnu");
        assert!(config.compiler_flags.contains(&"-march=rv32imafdc".to_string()));
        
        let config = CrossCompileConfig::riscv64_bare_metal();
        assert_eq!(config.target.to_llvm_triple(), "riscv64-unknown-none");
        assert!(config.use_lld);
    }
    
    #[test]
    fn test_from_target_string() {
        let config = CrossCompileConfig::from_target_string("riscv64-linux").unwrap();
        assert_eq!(config.target.arch, Architecture::RiscV64);
        
        let config = CrossCompileConfig::from_target_string("riscv32-none").unwrap();
        assert_eq!(config.target.os, OperatingSystem::None);
        
        let result = CrossCompileConfig::from_target_string("invalid-target");
        assert!(result.is_err());
    }
    
    #[test]
    fn test_config_builders() {
        let config = CrossCompileConfig::riscv64_vector()
            .with_sysroot(PathBuf::from("/custom/sysroot"))
            .add_compiler_flags(vec!["-g".to_string()])
            .add_linker_flags(vec!["-Map=output.map".to_string()]);
            
        assert!(config.sysroot.is_some());
        assert!(config.compiler_flags.contains(&"-g".to_string()));
        assert!(config.linker_flags.contains(&"-Map=output.map".to_string()));
    }
}