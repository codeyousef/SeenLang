//! Linking operations for the LLVM backend.
//!
//! This module handles artifact linking: executables, shared libraries,
//! static libraries, and WebAssembly modules.

use std::path::{Path, PathBuf};
use anyhow::{anyhow, Result};

use crate::llvm_backend::LlvmBackend;
use crate::llvm::target;
use crate::llvm::types::LinkOutput;

/// Trait for linking operations on the LLVM backend.
pub trait LinkingOps<'ctx> {
    /// Link an artifact (executable, shared lib, static lib) from an object file.
    fn link_artifact(
        &self,
        kind: LinkOutput,
        obj_path: &Path,
        out_path: &Path,
        triple: &str,
        extra_libs: &[PathBuf],
    ) -> Result<()>;

    /// Link an executable from an object file.
    fn link_executable(
        &self,
        obj_path: &Path,
        out_path: &Path,
        triple: &str,
        extra_libs: &[PathBuf],
    ) -> Result<()>;

    /// Link a shared library from an object file.
    fn link_shared(
        &self,
        obj_path: &Path,
        out_path: &Path,
        triple: &str,
        extra_libs: &[PathBuf],
    ) -> Result<()>;

    /// Create a static library (archive) from an object file.
    fn link_static(&self, obj_path: &Path, out_path: &Path, triple: &str) -> Result<()>;

    /// Link a WebAssembly module from an object file.
    fn link_wasm(&self, obj_path: &Path, out_path: &Path, kind: LinkOutput) -> Result<()>;
}

impl<'ctx> LinkingOps<'ctx> for LlvmBackend<'ctx> {
    fn link_artifact(
        &self,
        kind: LinkOutput,
        obj_path: &Path,
        out_path: &Path,
        triple: &str,
        extra_libs: &[PathBuf],
    ) -> Result<()> {
        match kind {
            LinkOutput::Executable => self.link_executable(obj_path, out_path, triple, extra_libs),
            LinkOutput::SharedLibrary => self.link_shared(obj_path, out_path, triple, extra_libs),
            LinkOutput::StaticLibrary => self.link_static(obj_path, out_path, triple),
            LinkOutput::ObjectOnly => unreachable!("handled earlier"),
        }
    }

    fn link_executable(
        &self,
        obj_path: &Path,
        out_path: &Path,
        triple: &str,
        extra_libs: &[PathBuf],
    ) -> Result<()> {
        if triple.contains("wasm32") {
            return self.link_wasm(obj_path, out_path, LinkOutput::Executable);
        }
        self.invoke_linker(obj_path, out_path, triple, extra_libs, LinkOutput::Executable)
    }

    fn link_shared(
        &self,
        obj_path: &Path,
        out_path: &Path,
        triple: &str,
        extra_libs: &[PathBuf],
    ) -> Result<()> {
        if triple.contains("wasm32") {
            return self.link_wasm(obj_path, out_path, LinkOutput::SharedLibrary);
        }
        self.invoke_linker(obj_path, out_path, triple, extra_libs, LinkOutput::SharedLibrary)
    }

    fn link_static(&self, obj_path: &Path, out_path: &Path, triple: &str) -> Result<()> {
        let tool = target::select_archiver(triple)?;
        eprintln!("LLVM backend: invoking archiver {}", tool.display());
        let mut cmd = std::process::Command::new(&tool);
        target::apply_deterministic_env(&mut cmd);
        if triple.contains("windows")
            && tool
                .file_name()
                .map(|name| name.to_string_lossy().to_ascii_lowercase().contains("lib"))
                .unwrap_or(false)
        {
            cmd.arg("/nologo")
                .arg(format!("/OUT:{}", out_path.display()))
                .arg(obj_path);
        } else {
            cmd.arg("rcs").arg(out_path).arg(obj_path);
        }
        target::exec_command(cmd, "archive static library")?;

        if triple.contains("apple") {
            let ranlib = target::select_ranlib(triple)?;
            let mut cmd = std::process::Command::new(&ranlib);
            cmd.arg(out_path);
            target::exec_command(cmd, "ranlib static library")?;
        }

        Ok(())
    }

    fn link_wasm(&self, obj_path: &Path, out_path: &Path, kind: LinkOutput) -> Result<()> {
        let program = target::lookup_wasm_linker()?;
        eprintln!("LLVM backend: invoking wasm linker {:?}", program);
        let mut cmd = std::process::Command::new(&program);
        target::apply_deterministic_env(&mut cmd);
        cmd.arg(obj_path);
        cmd.arg("-o");
        cmd.arg(out_path);
        cmd.arg("--no-entry");
        cmd.arg("--allow-undefined");
        cmd.arg("--export-all");
        cmd.arg("--export=seen_main");
        cmd.arg("--export=memory");
        cmd.arg("--gc-sections");
        if matches!(kind, LinkOutput::SharedLibrary) {
            cmd.arg("--shared");
        }
        target::exec_command(cmd, "link wasm artifact")
    }
}

// Private helper - DRY pattern for link_executable and link_shared
impl<'ctx> LlvmBackend<'ctx> {
    fn invoke_linker(
        &self,
        obj_path: &Path,
        out_path: &Path,
        triple: &str,
        extra_libs: &[PathBuf],
        kind: LinkOutput,
    ) -> Result<()> {
        let invocation = target::select_linker(triple)?;
        eprintln!(
            "LLVM backend: invoking linker {:?} (flavor {:?})",
            invocation.program, invocation.flavor
        );
        let mut cmd = std::process::Command::new(&invocation.program);
        target::apply_deterministic_env(&mut cmd);
        cmd.args(&invocation.args);
        cmd.args(target::linker_pre_args(triple, kind));
        cmd.arg(obj_path);
        cmd.arg("-o");
        cmd.arg(out_path);
        for lib in extra_libs {
            cmd.arg(lib);
        }
        cmd.args(target::linker_post_args(triple, kind));
        let desc = match kind {
            LinkOutput::Executable => "link executable",
            LinkOutput::SharedLibrary => "link shared library",
            _ => "link artifact",
        };
        target::exec_command(cmd, desc)
    }
}
