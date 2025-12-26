//! Public types and configuration structures for the LLVM backend.

use std::path::PathBuf;

pub use inkwell::OptimizationLevel as LlvmOptLevel;

/// Output artifact type for linking.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum LinkOutput {
    Executable,
    ObjectOnly,
    SharedLibrary,
    StaticLibrary,
}

/// AVX-10 vector width configuration.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Avx10Width {
    Bits256,
    Bits512,
}

/// ARM SVE vector length configuration.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SveVectorLength {
    Bits128,
    Bits256,
    Bits512,
}

/// CPU feature flags for target-specific optimization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CpuFeature {
    IntelApx,
    IntelAvx10(Avx10Width),
    ArmSve(SveVectorLength),
}

impl CpuFeature {
    pub fn llvm_feature_flags(&self) -> &'static [&'static str] {
        match self {
            CpuFeature::IntelApx => &[],
            CpuFeature::IntelAvx10(Avx10Width::Bits256) => &["+avx2"],
            CpuFeature::IntelAvx10(Avx10Width::Bits512) => &["+avx512f", "+avx512vl"],
            CpuFeature::ArmSve(_) => &["+sve"],
        }
    }
}

/// Memory topology hint for CXL-aware optimization.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub enum MemoryTopologyHint {
    #[default]
    Default,
    CxlNear,
    CxlFar,
}

/// Target-specific compilation options.
#[derive(Clone, Debug)]
pub struct TargetOptions<'a> {
    pub triple: Option<&'a str>,
    pub cpu: Option<&'a str>,
    pub features: Option<String>,
    pub static_libraries: Vec<PathBuf>,
    pub hardware_features: Vec<CpuFeature>,
    pub memory_topology: MemoryTopologyHint,
    pub opt_level: Option<LlvmOptLevel>,
}

impl<'a> Default for TargetOptions<'a> {
    fn default() -> Self {
        Self {
            triple: None,
            cpu: None,
            features: None,
            static_libraries: Vec::new(),
            hardware_features: Vec::new(),
            memory_topology: MemoryTopologyHint::Default,
            opt_level: None,
        }
    }
}

/// Internal linker invocation configuration.
#[derive(Debug)]
pub struct LinkerInvocation {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub flavor: LinkerFlavor,
}

/// Linker flavor for platform-specific linking.
#[derive(Debug)]
pub enum LinkerFlavor {
    ClangLike,
    WasmLd,
    Custom,
    AndroidClang,
}
