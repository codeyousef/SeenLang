//! Target machine configuration and linker invocation.
//!
//! This module handles LLVM target machine creation, object file generation,
//! and platform-specific linking logic.

use std::env;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use inkwell::targets::{
    CodeModel, RelocMode, Target, TargetMachine, TargetTriple,
};

use super::types::{
    LinkOutput, LinkerFlavor, LinkerInvocation, LlvmOptLevel, TargetOptions,
};

/// Create LLVM target machine from options.
pub fn target_machine_for(options: &TargetOptions<'_>) -> Result<(TargetTriple, TargetMachine)> {
    let triple = if let Some(triple) = options.triple {
        TargetTriple::create(triple)
    } else {
        TargetMachine::get_default_triple()
    };
    let target = Target::from_triple(&triple)
        .map_err(|e| anyhow!("Target from triple failed: {e:?}"))?;
    let cpu = options.cpu.unwrap_or("generic");
    let mut feature_parts = Vec::new();
    if let Some(explicit) = options.features.as_deref() {
        if !explicit.trim().is_empty() {
            feature_parts.push(explicit.to_string());
        }
    }
    let hardware_flags: Vec<&'static str> = options
        .hardware_features
        .iter()
        .flat_map(|feature| feature.llvm_feature_flags().iter().copied())
        .collect();
    if !hardware_flags.is_empty() {
        feature_parts.push(hardware_flags.join(","));
    }
    let feature_string = feature_parts.join(",");
    let features = if feature_string.is_empty() {
        ""
    } else {
        feature_string.as_str()
    };
    let machine = target
        .create_target_machine(
            &triple,
            cpu,
            features,
            options.opt_level.unwrap_or(LlvmOptLevel::Default),
            RelocMode::Default,
            CodeModel::Default,
        )
        .ok_or_else(|| anyhow!("Create target machine failed"))?;
    Ok((triple, machine))
}

/// Determine object file extension based on target triple.
pub fn object_file_path(out_path: &Path, triple: &str) -> PathBuf {
    let ext = if triple.contains("windows") {
        "obj"
    } else {
        "o"
    };
    out_path.with_extension(ext)
}

/// Select appropriate linker for the target.
pub fn select_linker(triple: &str) -> Result<LinkerInvocation> {
    if let Ok(explicit) = env::var("SEEN_LLVM_LINKER") {
        if !explicit.is_empty() {
            return Ok(LinkerInvocation {
                program: PathBuf::from(explicit),
                args: Vec::new(),
                flavor: LinkerFlavor::Custom,
            });
        }
    }

    if triple.contains("wasm32") {
        let program = which::which("wasm-ld")
            .ok()
            .unwrap_or_else(|| PathBuf::from("wasm-ld"));
        return Ok(LinkerInvocation {
            program,
            args: Vec::new(),
            flavor: LinkerFlavor::WasmLd,
        });
    }

    if triple.contains("android") {
        let clang = android_clang_path(triple)?;
        return Ok(LinkerInvocation {
            program: clang,
            args: Vec::new(),
            flavor: LinkerFlavor::AndroidClang,
        });
    }

    let program = which::which("clang").ok().unwrap_or_else(|| {
        which::which("cc")
            .ok()
            .unwrap_or_else(|| PathBuf::from("clang"))
    });

    let mut args = Vec::new();
    let program_is_clang = program
        .file_name()
        .map(|n| n.to_string_lossy().contains("clang"))
        .unwrap_or(false);
    if program_is_clang {
        args.push(format!("--target={}", triple));
    }

    Ok(LinkerInvocation {
        program,
        args,
        flavor: LinkerFlavor::ClangLike,
    })
}

/// Select archiver tool for static library creation.
pub fn select_archiver(triple: &str) -> Result<PathBuf> {
    if let Ok(explicit) = env::var("SEEN_LLVM_ARCHIVER") {
        if !explicit.is_empty() {
            return Ok(PathBuf::from(explicit));
        }
    }

    if triple.contains("android") {
        return android_tool_path("llvm-ar");
    }

    if triple.contains("windows") {
        Ok(which::which("llvm-ar")
            .ok()
            .unwrap_or_else(|| PathBuf::from("lib")))
    } else {
        Ok(which::which("llvm-ar")
            .ok()
            .unwrap_or_else(|| PathBuf::from("ar")))
    }
}

/// Select ranlib tool for library indexing.
pub fn select_ranlib(triple: &str) -> Result<PathBuf> {
    if let Ok(explicit) = env::var("SEEN_LLVM_RANLIB") {
        if !explicit.is_empty() {
            return Ok(PathBuf::from(explicit));
        }
    }
    if triple.contains("android") {
        return android_tool_path("llvm-ranlib");
    }
    Ok(which::which("llvm-ranlib")
        .ok()
        .unwrap_or_else(|| PathBuf::from("ranlib")))
}

/// Linker arguments to prepend (platform-specific).
pub fn linker_pre_args(triple: &str, kind: LinkOutput) -> Vec<String> {
    match kind {
        LinkOutput::SharedLibrary => {
            if triple.contains("apple") {
                vec!["-dynamiclib".to_string()]
            } else {
                vec!["-shared".to_string()]
            }
        }
        LinkOutput::Executable => {
            if triple.contains("windows") {
                vec![]
            } else {
                vec![]
            }
        }
        _ => vec![],
    }
}

/// Linker arguments to append (platform-specific).
pub fn linker_post_args(triple: &str, kind: LinkOutput) -> Vec<String> {
    match kind {
        LinkOutput::Executable => {
            if triple.contains("android") {
                vec!["-Wl,--build-id=none".to_string(), "-lm".to_string()]
            } else if triple.contains("linux") {
                // Force deterministic ELF: no build-id tag and no PIE randomization.
                vec![
                    "-Wl,--build-id=none".to_string(),
                    "-no-pie".to_string(),
                    "-lm".to_string(),
                ]
            } else if triple.contains("apple") {
                vec!["-lm".to_string()]
            } else {
                vec![]
            }
        }
        LinkOutput::SharedLibrary => {
            if triple.contains("android") || triple.contains("linux") {
                vec!["-Wl,--build-id=none".to_string(), "-lm".to_string()]
            } else {
                vec![]
            }
        }
        _ => vec![],
    }
}

/// Execute a shell command and check for success.
pub fn exec_command(mut cmd: std::process::Command, action: &str) -> Result<()> {
    let status = cmd
        .status()
        .with_context(|| format!("Spawning {}", action))?;
    if !status.success() {
        return Err(anyhow!(
            "Command to {} failed with status {}",
            action,
            status
        ));
    }
    Ok(())
}

/// Apply environment variables for deterministic builds.
pub fn apply_deterministic_env(cmd: &mut std::process::Command) {
    // Respect caller override but default to a zero epoch to strip timestamps/build IDs.
    if std::env::var_os("SOURCE_DATE_EPOCH").is_none() {
        cmd.env("SOURCE_DATE_EPOCH", "0");
    }
}

/// Find wasm-ld linker.
pub fn lookup_wasm_linker() -> Result<PathBuf> {
    if let Ok(explicit) = env::var("SEEN_LLVM_LINKER") {
        if !explicit.is_empty() {
            let path = PathBuf::from(&explicit);
            if path.components().count() > 1 || path.is_absolute() {
                if path.exists() {
                    return Ok(path);
                } else {
                    bail!(
                        "SEEN_LLVM_LINKER points to {}, but the file does not exist",
                        path.display()
                    );
                }
            } else if let Ok(found) = which::which(&path) {
                return Ok(found);
            } else {
                bail!("SEEN_LLVM_LINKER={} was not found on PATH", explicit);
            }
        }
    }

    which::which("wasm-ld").map_err(|_| {
        anyhow!(
            "wasm-ld linker not found; install LLVM 15 wasm-ld or set SEEN_LLVM_LINKER to an available linker"
        )
    })
}

// =============================================================================
// Android NDK helpers
// =============================================================================

/// Get path to Android NDK clang for a target triple.
pub fn android_clang_path(triple: &str) -> Result<PathBuf> {
    let bin_dir = android_bin_dir()?;
    let api = android_api_level();
    let prefix = if triple.contains("aarch64") {
        "aarch64-linux-android"
    } else if triple.contains("armv7") || triple.contains("armeabi") || triple.contains("arm") {
        "armv7a-linux-androideabi"
    } else if triple.contains("x86_64") {
        "x86_64-linux-android"
    } else if triple.contains("i686") || triple.contains("x86") {
        "i686-linux-android"
    } else {
        bail!(
            "Android target {} not yet supported; expected aarch64, armv7a, x86_64, or i686 variants",
            triple
        );
    };
    let tool_name = format!("{prefix}{api}-clang");
    let path = bin_dir.join(tool_name);
    if path.exists() {
        Ok(path)
    } else {
        bail!(
            "Expected Android NDK clang at {}, but it was not found",
            path.display()
        );
    }
}

/// Get path to an Android NDK tool.
pub fn android_tool_path(tool: &str) -> Result<PathBuf> {
    let bin_dir = android_bin_dir()?;
    let path = bin_dir.join(tool);
    if path.exists() {
        Ok(path)
    } else {
        bail!(
            "Expected Android NDK tool {} inside {}; ensure the NDK is installed",
            tool,
            bin_dir.display()
        );
    }
}

/// Get Android NDK toolchain bin directory.
pub fn android_bin_dir() -> Result<PathBuf> {
    let ndk_home = env::var("ANDROID_NDK_HOME")
        .map_err(|_| anyhow!("ANDROID_NDK_HOME must be set to cross-compile for Android"))?;
    let host_tag = env::var("ANDROID_NDK_HOST")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(default_ndk_host_tag);
    let bin_path = Path::new(&ndk_home)
        .join("toolchains")
        .join("llvm")
        .join("prebuilt")
        .join(&host_tag)
        .join("bin");
    if bin_path.exists() {
        Ok(bin_path)
    } else {
        bail!(
            "Android NDK toolchain bin directory {} does not exist; check ANDROID_NDK_HOME and ANDROID_NDK_HOST (currently using {})",
            bin_path.display(),
            host_tag
        );
    }
}

/// Get default NDK host tag based on current OS.
pub fn default_ndk_host_tag() -> String {
    if cfg!(target_os = "linux") {
        "linux-x86_64".to_string()
    } else if cfg!(target_os = "macos") {
        "darwin-x86_64".to_string()
    } else if cfg!(target_os = "windows") {
        "windows-x86_64".to_string()
    } else {
        "linux-x86_64".to_string()
    }
}

/// Get Android API level from environment or default.
pub fn android_api_level() -> String {
    env::var("ANDROID_API_LEVEL")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "21".to_string())
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::{Mutex, OnceLock};
    use tempfile::tempdir;

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn with_env_lock<F, R>(f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let guard = env_lock().lock().expect("lock env mutex");
        let result = f();
        drop(guard);
        result
    }

    #[test]
    fn object_file_extension_matches_target() {
        let linux_path = object_file_path(Path::new("foo"), "x86_64-unknown-linux-gnu");
        let windows_path = object_file_path(Path::new("foo"), "x86_64-pc-windows-msvc");
        assert!(linux_path.ends_with("foo.o"));
        assert!(windows_path.ends_with("foo.obj"));
    }

    #[test]
    fn shared_library_flags_match_platform() {
        let linux_flags = linker_pre_args("x86_64-unknown-linux-gnu", LinkOutput::SharedLibrary);
        let mac_flags = linker_pre_args("aarch64-apple-darwin", LinkOutput::SharedLibrary);
        assert!(linux_flags.contains(&"-shared".to_string()));
        assert!(mac_flags.contains(&"-dynamiclib".to_string()));
    }

    #[test]
    fn executable_link_flags_include_libm_on_linux() {
        let flags = linker_post_args("x86_64-unknown-linux-gnu", LinkOutput::Executable);
        assert!(flags.contains(&"-lm".to_string()));
        assert!(flags.contains(&"-no-pie".to_string()));
    }

    #[test]
    fn android_clang_path_errors_without_ndk() {
        with_env_lock(|| {
            let original = env::var("ANDROID_NDK_HOME").ok();
            env::remove_var("ANDROID_NDK_HOME");
            let result = android_clang_path("aarch64-linux-android");
            assert!(result.is_err());
            match original {
                Some(val) => env::set_var("ANDROID_NDK_HOME", val),
                None => env::remove_var("ANDROID_NDK_HOME"),
            }
        });
    }

    #[test]
    fn android_clang_path_resolves_with_mock_ndk() {
        with_env_lock(|| {
            let tmp = tempdir().expect("temp dir");
            let ndk_home = tmp.path();
            let bin_dir = ndk_home
                .join("toolchains")
                .join("llvm")
                .join("prebuilt")
                .join(default_ndk_host_tag())
                .join("bin");
            fs::create_dir_all(&bin_dir).expect("create bin dir");
            let clang_path = bin_dir.join("aarch64-linux-android21-clang");
            fs::write(&clang_path, b"").expect("write mock clang");

            let original_home = env::var("ANDROID_NDK_HOME").ok();
            let original_host = env::var("ANDROID_NDK_HOST").ok();
            let original_api = env::var("ANDROID_API_LEVEL").ok();

            env::set_var("ANDROID_NDK_HOME", ndk_home);
            env::remove_var("ANDROID_NDK_HOST");
            env::remove_var("ANDROID_API_LEVEL");

            let resolved = android_clang_path("aarch64-linux-android")
                .expect("resolve clang path");
            assert_eq!(resolved, clang_path);

            match original_home {
                Some(val) => env::set_var("ANDROID_NDK_HOME", val),
                None => env::remove_var("ANDROID_NDK_HOME"),
            }
            match original_host {
                Some(val) => env::set_var("ANDROID_NDK_HOST", val),
                None => env::remove_var("ANDROID_NDK_HOST"),
            }
            match original_api {
                Some(val) => env::set_var("ANDROID_API_LEVEL", val),
                None => env::remove_var("ANDROID_API_LEVEL"),
            }
        });
    }
}
