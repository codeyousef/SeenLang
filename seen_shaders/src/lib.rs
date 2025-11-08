use naga::back::{msl, wgsl};
use naga::front::spv;
use naga::valid::{Capabilities, ValidationFlags, Validator};
use std::fs;
use std::path::{Path, PathBuf};
use naga::ShaderStage;
use thiserror::Error;

/// Supported shader output formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShaderTarget {
    Spirv,
    Wgsl,
    Msl,
}

impl ShaderTarget {
    pub fn extension(&self) -> &'static str {
        match self {
            ShaderTarget::Spirv => "spv",
            ShaderTarget::Wgsl => "wgsl",
            ShaderTarget::Msl => "metal",
        }
    }

    pub fn as_label(&self) -> &'static str {
        match self {
            ShaderTarget::Spirv => "spirv",
            ShaderTarget::Wgsl => "wgsl",
            ShaderTarget::Msl => "msl",
        }
    }
}

/// Shader stage represented by a compiled entry point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShaderStage {
    Vertex,
    Fragment,
    Compute,
}

impl ShaderStage {
    pub fn as_str(&self) -> &'static str {
        match self {
            ShaderStage::Vertex => "vertex",
            ShaderStage::Fragment => "fragment",
            ShaderStage::Compute => "compute",
        }
    }
}

impl From<naga::ShaderStage> for ShaderStage {
    fn from(stage: naga::ShaderStage) -> Self {
        match stage {
            naga::ShaderStage::Vertex => ShaderStage::Vertex,
            naga::ShaderStage::Fragment => ShaderStage::Fragment,
            naga::ShaderStage::Compute => ShaderStage::Compute,
        }
    }
}

/// Resulting artifact from shader compilation/emission.
#[derive(Debug, Clone)]
pub struct ShaderArtifact {
    pub target: ShaderTarget,
    pub path: PathBuf,
}

/// Summary of entry points discovered in an input module.
#[derive(Debug, Clone)]
pub struct ShaderEntryPoint {
    pub name: String,
    pub stage: ShaderStage,
}

/// Complete result of compiling a single shader input.
#[derive(Debug, Clone)]
pub struct ShaderCompileResult {
    pub entry_points: Vec<ShaderEntryPoint>,
    pub artifacts: Vec<ShaderArtifact>,
}

#[derive(Debug, Clone)]
pub struct ShaderCompileOptions<'a> {
    pub output_dir: &'a Path,
    pub targets: &'a [ShaderTarget],
    pub validate_only: bool,
}

#[derive(Default)]
pub struct ShaderCompiler;

impl ShaderCompiler {
    pub fn new() -> Self {
        Self
    }

    pub fn compile_file(
        &self,
        input: &Path,
        options: &ShaderCompileOptions,
    ) -> Result<ShaderCompileResult, ShaderError> {
        let data = fs::read(input).map_err(|source| ShaderError::Io {
            path: input.to_path_buf(),
            source,
        })?;
        let module = spv::parse_u8_slice(&data, &spv::Options::default()).map_err(|err| {
            ShaderError::Parse {
                path: input.to_path_buf(),
                message: err.to_string(),
            }
        })?;

        let mut validator = Validator::new(ValidationFlags::all(), Capabilities::all());
        let info = validator
            .validate(&module)
            .map_err(|err| ShaderError::Validation {
                path: input.to_path_buf(),
                message: format!("{err:?}"),
            })?;
        let entry_points = module
            .entry_points
            .iter()
            .map(|ep| ShaderEntryPoint {
                name: ep.name.clone(),
                stage: ShaderStage::from(ep.stage),
            })
            .collect::<Vec<_>>();

        if options.validate_only {
            return Ok(ShaderCompileResult {
                entry_points,
                artifacts: Vec::new(),
            });
        }

        if options.targets.is_empty() {
            return Err(ShaderError::NoTargets);
        }

        let base = input
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .ok_or_else(|| ShaderError::InvalidInputName {
                path: input.to_path_buf(),
            })?;

        let mut artifacts = Vec::new();
        for target in options.targets {
            let artifact = match target {
                ShaderTarget::Spirv => self.emit_spirv_copy(input, options.output_dir, &base)?,
                ShaderTarget::Wgsl => self.emit_wgsl(&module, &info, options.output_dir, &base)?,
                ShaderTarget::Msl => self.emit_msl(&module, &info, options.output_dir, &base)?,
            };
            artifacts.push(ShaderArtifact {
                target: *target,
                path: artifact,
            });
        }

        Ok(ShaderCompileResult {
            entry_points,
            artifacts,
        })
    }

    fn emit_spirv_copy(
        &self,
        input: &Path,
        output_dir: &Path,
        base: &str,
    ) -> Result<PathBuf, ShaderError> {
        fs::create_dir_all(output_dir).map_err(|source| ShaderError::IoDir {
            path: output_dir.to_path_buf(),
            source,
        })?;
        let dest = output_dir.join(format!("{base}.spv"));
        fs::copy(input, &dest).map_err(|source| ShaderError::IoCopy {
            from: input.to_path_buf(),
            to: dest.clone(),
            source,
        })?;
        Ok(dest)
    }

    fn emit_wgsl(
        &self,
        module: &naga::Module,
        info: &naga::valid::ModuleInfo,
        output_dir: &Path,
        base: &str,
    ) -> Result<PathBuf, ShaderError> {
        fs::create_dir_all(output_dir).map_err(|source| ShaderError::IoDir {
            path: output_dir.to_path_buf(),
            source,
        })?;
        let wgsl = wgsl::write_string(module, info, wgsl::WriterFlags::empty()).map_err(|err| {
            ShaderError::Emit {
                target: ShaderTarget::Wgsl,
                message: err.to_string(),
            }
        })?;
        let dest = output_dir.join(format!("{base}.wgsl"));
        fs::write(&dest, wgsl).map_err(|source| ShaderError::IoWrite {
            path: dest.clone(),
            source,
        })?;
        Ok(dest)
    }

    fn emit_msl(
        &self,
        module: &naga::Module,
        info: &naga::valid::ModuleInfo,
        output_dir: &Path,
        base: &str,
    ) -> Result<PathBuf, ShaderError> {
        fs::create_dir_all(output_dir).map_err(|source| ShaderError::IoDir {
            path: output_dir.to_path_buf(),
            source,
        })?;
        let options = msl::Options::default();
        let pipeline = msl::PipelineOptions {
            allow_and_force_point_size: false,
        };
        let (msl, _) = msl::write_string(module, info, &options, &pipeline).map_err(|err| {
            ShaderError::Emit {
                target: ShaderTarget::Msl,
                message: err.to_string(),
            }
        })?;
        let dest = output_dir.join(format!("{base}.metal"));
        fs::write(&dest, msl).map_err(|source| ShaderError::IoWrite {
            path: dest.clone(),
            source,
        })?;
        Ok(dest)
    }
}

#[derive(Error, Debug)]
pub enum ShaderError {
    #[error("failed to read shader {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to read or create directory {path}: {source}")]
    IoDir {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to copy shader from {from} to {to}: {source}")]
    IoCopy {
        from: PathBuf,
        to: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to write shader artifact {path}: {source}")]
    IoWrite {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to parse SPIR-V {path}: {message}")]
    Parse { path: PathBuf, message: String },
    #[error("validation error in {path}: {message}")]
    Validation { path: PathBuf, message: String },
    #[error("{target:?} backend failed: {message}")]
    Emit {
        target: ShaderTarget,
        message: String,
    },
    #[error("input path {path} is missing a valid file name")]
    InvalidInputName { path: PathBuf },
    #[error("no shader targets were requested")]
    NoTargets,
}
