#![cfg(feature = "llvm")]
//! LLVM backend that lowers Seen IR (IRProgram) to native code via inkwell.
//!
//! Scope: Implements a minimal but solid subset required to compile the
//! self‑hosting entry (`compiler_seen/src/main.seen`) and similar programs.

use std::collections::HashMap;
use std::convert::TryFrom;
use std::env;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use inkwell::attributes::{Attribute, AttributeLoc};
use inkwell::basic_block::BasicBlock as LlvmBasicBlock;
use inkwell::builder::Builder;
use inkwell::context::Context as LlvmContext;
use inkwell::module::{Linkage, Module as LlvmModule};
use inkwell::targets::{
    CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetMachine, TargetTriple,
};
use inkwell::types::{
    BasicMetadataTypeEnum, BasicType, BasicTypeEnum, IntType, PointerType, StructType, VectorType,
};
use inkwell::values::{
    BasicMetadataValueEnum, BasicValue, BasicValueEnum, FunctionValue, GlobalValue, PointerValue,
    UnnamedAddress, VectorValue,
};
pub use inkwell::OptimizationLevel as LlvmOptLevel;

use crate::function::{IRFunction, InlineHint, RegisterPressureClass};
use crate::instruction::{BasicBlock, IRSelectArm, Instruction};
use crate::module::IRModule;
use crate::value::{IRType, IRValue};
use crate::{HardwareProfile, IRProgram};

fn block_sort_key(name: &str) -> (i64, String, String) {
    if let Some(idx) = name.rfind('_') {
        if let Ok(num) = name[idx + 1..].parse::<i64>() {
            let prefix = name[..idx].to_string();
            return (num, prefix, name.to_string());
        }
    }
    (i64::MAX, name.to_string(), name.to_string())
}

fn is_string_literal_ir(value: &IRValue) -> bool {
    matches!(value, IRValue::String(_) | IRValue::StringConstant(_))
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum LinkOutput {
    Executable,
    ObjectOnly,
    SharedLibrary,
    StaticLibrary,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Avx10Width {
    Bits256,
    Bits512,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SveVectorLength {
    Bits128,
    Bits256,
    Bits512,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CpuFeature {
    IntelApx,
    IntelAvx10(Avx10Width),
    ArmSve(SveVectorLength),
}

impl CpuFeature {
    fn llvm_feature_flags(&self) -> &'static [&'static str] {
        match self {
            CpuFeature::IntelApx => &[],
            CpuFeature::IntelAvx10(Avx10Width::Bits256) => &["+avx2"],
            CpuFeature::IntelAvx10(Avx10Width::Bits512) => &["+avx512f", "+avx512vl"],
            CpuFeature::ArmSve(_) => &["+sve"],
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub enum MemoryTopologyHint {
    #[default]
    Default,
    CxlNear,
    CxlFar,
}

#[derive(Clone, Debug)]
pub struct TargetOptions<'a> {
    pub triple: Option<&'a str>,
    pub cpu: Option<&'a str>,
    pub features: Option<String>,
    pub static_libraries: Vec<PathBuf>,
    pub hardware_features: Vec<CpuFeature>,
    pub memory_topology: MemoryTopologyHint,
    pub opt_level: LlvmOptLevel,
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
            opt_level: LlvmOptLevel::Default,
        }
    }
}

#[derive(Debug)]
struct LinkerInvocation {
    program: PathBuf,
    args: Vec<String>,
    flavor: LinkerFlavor,
}

#[derive(Debug)]
enum LinkerFlavor {
    ClangLike,
    WasmLd,
    Custom,
    AndroidClang,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::function::{InlineHint, RegisterPressureClass};
    use crate::{IRFunction, IRModule, IRProgram, IRType, IRValue, Instruction};
    use inkwell::targets::TargetTriple;
    use std::env;
    use std::fs;
    use std::path::Path;
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
        let triple_linux = TargetTriple::create("x86_64-unknown-linux-gnu");
        let triple_windows = TargetTriple::create("x86_64-pc-windows-msvc");
        let linux_path = LlvmBackend::object_file_path(Path::new("foo"), &triple_linux);
        let windows_path = LlvmBackend::object_file_path(Path::new("foo"), &triple_windows);
        assert!(linux_path.ends_with("foo.o"));
        assert!(windows_path.ends_with("foo.obj"));
    }

    #[test]
    fn shared_library_flags_match_platform() {
        let linux_flags =
            LlvmBackend::linker_pre_args("x86_64-unknown-linux-gnu", LinkOutput::SharedLibrary);
        let mac_flags =
            LlvmBackend::linker_pre_args("aarch64-apple-darwin", LinkOutput::SharedLibrary);
        assert!(linux_flags.contains(&"-shared".to_string()));
        assert!(mac_flags.contains(&"-dynamiclib".to_string()));
    }

    #[test]
    fn executable_link_flags_include_libm_on_linux() {
        let flags =
            LlvmBackend::linker_post_args("x86_64-unknown-linux-gnu", LinkOutput::Executable);
        assert!(flags.contains(&"-lm".to_string()));
        assert!(flags.contains(&"-no-pie".to_string()));
    }

    #[test]
    fn android_clang_path_errors_without_ndk() {
        with_env_lock(|| {
            let original = env::var("ANDROID_NDK_HOME").ok();
            env::remove_var("ANDROID_NDK_HOME");
            let result = LlvmBackend::android_clang_path("aarch64-linux-android");
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
                .join(LlvmBackend::default_ndk_host_tag())
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

            let resolved = LlvmBackend::android_clang_path("aarch64-linux-android")
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

    #[test]
    fn declare_function_applies_inline_and_pressure_attributes() {
        let mut backend = LlvmBackend::new();
        let mut func = IRFunction::new("inline_me", IRType::Void);
        func.inline_hint = InlineHint::AlwaysInline;
        func.register_pressure = RegisterPressureClass::High;
        backend.declare_function(&func).expect("declare");
        let ir = backend.module.print_to_string().to_string();
        assert!(
            ir.contains("alwaysinline"),
            "expected alwaysinline attribute in {ir}"
        );
        assert!(
            ir.contains("seen-register-pressure"),
            "expected register pressure attribute in {ir}"
        );
    }

    #[test]
    fn simd_splat_and_reduce_lower_to_vector_ir() {
        let mut backend = LlvmBackend::new();
        let mut func = IRFunction::new("simd_reduce", IRType::Float);
        func.register_count = 2;
        let mut entry = BasicBlock::new(Label::new("entry"));
        entry.instructions.push(Instruction::SimdSplat {
            scalar: IRValue::Float(1.0),
            lane_type: IRType::Float,
            lanes: 4,
            result: IRValue::Register(0),
        });
        entry.instructions.push(Instruction::SimdReduceAdd {
            vector: IRValue::Register(0),
            lane_type: IRType::Float,
            result: IRValue::Register(1),
        });
        entry.terminator = Some(Instruction::Return(Some(IRValue::Register(1))));
        func.cfg.add_block(entry);
        func.cfg.set_entry_block("entry".to_string());

        let mut module = IRModule::new("simd_mod");
        module.add_function(func);
        let mut program = IRProgram::new();
        program.add_module(module);

        backend.lower_program(&program).expect("lower program");
        let ir = backend.module.print_to_string().to_string();
        assert!(
            ir.contains("<4 x double>"),
            "vector width missing in IR:\n{ir}"
        );
        assert!(
            ir.contains("reduce_fadd"),
            "expected reduce_fadd sequence in IR:\n{ir}"
        );
    }
}

pub struct LlvmBackend<'ctx> {
    ctx: &'ctx LlvmContext,
    module: LlvmModule<'ctx>,
    builder: Builder<'ctx>,
    i64_t: IntType<'ctx>,
    bool_t: IntType<'ctx>,
    i8_ptr_t: PointerType<'ctx>,
    handle_ty: Option<StructType<'ctx>>,
    select_result_ty: Option<StructType<'ctx>>,

    // Runtime/extern declarations
    printf: Option<FunctionValue<'ctx>>,
    strlen: Option<FunctionValue<'ctx>>,
    strcmp: Option<FunctionValue<'ctx>>,
    malloc: Option<FunctionValue<'ctx>>,
    free: Option<FunctionValue<'ctx>>,
    memcpy: Option<FunctionValue<'ctx>>,
    box_int_fn: Option<FunctionValue<'ctx>>,
    box_bool_fn: Option<FunctionValue<'ctx>>,
    box_ptr_fn: Option<FunctionValue<'ctx>>,
    use_channel_runtime_stubs: bool,
    cli_mode: bool,

    // Per‑function state (set during codegen)
    current_fn: Option<FunctionValue<'ctx>>,
    reg_values: HashMap<u32, BasicValueEnum<'ctx>>, // %rN -> value
    var_values: HashMap<String, BasicValueEnum<'ctx>>, // %var -> last assigned value (SSA‑like)
    var_slots: HashMap<String, PointerValue<'ctx>>, // %var -> alloca i64 slot
    var_slot_types: HashMap<String, BasicTypeEnum<'ctx>>, // %var -> LLVM storage type
    blocks: HashMap<String, LlvmBasicBlock<'ctx>>,  // label name -> BB
    reg_slots: HashMap<u32, PointerValue<'ctx>>,    // %rN -> alloca i64 slot

    // Arg globals
    g_argc: Option<inkwell::values::GlobalValue<'ctx>>,
    g_argv: Option<inkwell::values::GlobalValue<'ctx>>,
    fallthrough_bb: Option<LlvmBasicBlock<'ctx>>,
    byte_array_globals: HashMap<Vec<u8>, GlobalValue<'ctx>>,
    hardware_profile: HardwareProfile,
}

impl<'ctx> LlvmBackend<'ctx> {
    pub fn new() -> Self {
        // Initialize all LLVM targets up front so cross compilation works out of the box.
        Target::initialize_all(&InitializationConfig::default());

        let ctx = Box::leak(Box::new(LlvmContext::create()));
        let module = ctx.create_module("seen_module");
        let builder = ctx.create_builder();
        let i64_t = ctx.i64_type();
        let bool_t = ctx.bool_type();
        let i8_ptr_t = ctx.i8_type().ptr_type(inkwell::AddressSpace::from(0u16));

        Self {
            ctx,
            module,
            builder,
            i64_t,
            bool_t,
            i8_ptr_t,
            handle_ty: None,
            select_result_ty: None,
            printf: None,
            strlen: None,
            strcmp: None,
            malloc: None,
            free: None,
            memcpy: None,
            box_int_fn: None,
            box_bool_fn: None,
            box_ptr_fn: None,
            use_channel_runtime_stubs: true,
            cli_mode: false,
            current_fn: None,
            reg_values: HashMap::new(),
            var_values: HashMap::new(),
            var_slots: HashMap::new(),
            var_slot_types: HashMap::new(),
            blocks: HashMap::new(),
            reg_slots: HashMap::new(),
            g_argc: None,
            g_argv: None,
            fallthrough_bb: None,
            byte_array_globals: HashMap::new(),
            hardware_profile: HardwareProfile::default(),
        }
    }

    pub fn set_cli_mode(&mut self, enabled: bool) {
        self.cli_mode = enabled;
    }

    pub fn emit_llvm_ir(
        &mut self,
        prog: &IRProgram,
        out_path: &Path,
        options: TargetOptions<'_>,
    ) -> Result<()> {
        let (triple, target_machine) = Self::target_machine_for(&options)?;
        self.configure_module_target(&triple, &target_machine);
        self.hardware_profile = prog.hardware_profile.clone();

        self.lower_program(prog)
            .context("Lowering IR to LLVM failed")?;
        self.module
            .print_to_file(out_path)
            .map_err(|e| anyhow!("Failed to write .ll: {e:?}"))
    }

    pub fn emit_executable(
        &mut self,
        prog: &IRProgram,
        out_path: &Path,
        kind: LinkOutput,
        options: TargetOptions<'_>,
    ) -> Result<()> {
        self.use_channel_runtime_stubs = false; // Always link real runtime; stubs disabled for production
        self.hardware_profile = prog.hardware_profile.clone();
        self.lower_program(prog)
            .context("Lowering IR to LLVM failed")?;

        // Build object
        let static_libs = options.static_libraries.clone();
        let (target_triple, target_machine) = Self::target_machine_for(&options)?;
        self.configure_module_target(&target_triple, &target_machine);
        let obj_path = Self::object_file_path(out_path, &target_triple);
        eprintln!("LLVM backend: writing object file {:?}", obj_path);
        target_machine
            .write_to_file(&self.module, FileType::Object, &obj_path)
            .map_err(|e| anyhow!("Write object failed: {e:?}"))?;

        if matches!(kind, LinkOutput::ObjectOnly) {
            return Ok(());
        }

        let triple_string = target_triple
            .as_str()
            .to_str()
            .unwrap_or_else(|_| "")
            .to_string();
        self.link_artifact(kind, &obj_path, out_path, &triple_string, &static_libs)
    }

    fn lower_program(&mut self, prog: &IRProgram) -> Result<()> {
        // Predeclare all functions
        let mut fn_map: HashMap<String, FunctionValue<'ctx>> = HashMap::new();
        let mut modules: Vec<&IRModule> = prog.modules.iter().collect();
        modules.sort_by(|a, b| a.name.cmp(&b.name));
        for module in &modules {
            let mut funcs: Vec<&IRFunction> = module.functions_iter().collect();
            funcs.sort_by(|a, b| a.name.cmp(&b.name));
            for func in funcs {
                let f = self.declare_function(func)?;
                fn_map.insert(func.name.clone(), f);
            }
        }

        // Define each function
        for module in &modules {
            let mut funcs: Vec<&IRFunction> = module.functions_iter().collect();
            funcs.sort_by(|a, b| a.name.cmp(&b.name));
            for func in funcs {
                let f = *fn_map.get(&func.name).expect("declared");
                self.define_function(func, f, &fn_map)?;
            }
        }

        // Ensure we have a `seen_main` entry point; if only `main` exists, rename it so we can
        // inject a wrapper that initializes argc/argv for runtime helpers.
        if self.module.get_function("seen_main").is_none() {
            if let Some(orig_main) = self.module.get_function("main") {
                orig_main.as_global_value().set_name("seen_main");
            }
        }

        if self.module.get_function("seen_main").is_some()
            && self.module.get_function("main").is_none()
        {
            self.declare_main_wrapper();
        }

        // Do not inject any runtime stubs in production builds; rely on real runtime symbols.
        // If a symbol is genuinely missing, the linker should fail to reveal the gap.
        Ok(())
    }

    fn target_machine_for(options: &TargetOptions<'_>) -> Result<(TargetTriple, TargetMachine)> {
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
                LlvmOptLevel::Default,
                RelocMode::Default,
                CodeModel::Default,
            )
            .ok_or_else(|| anyhow!("Create target machine failed"))?;
        Ok((triple, machine))
    }

    fn configure_module_target(&mut self, triple: &TargetTriple, machine: &TargetMachine) {
        self.module.set_triple(triple);
        let layout = machine.get_target_data().get_data_layout();
        self.module.set_data_layout(&layout);
    }

    fn object_file_path(out_path: &Path, triple: &TargetTriple) -> PathBuf {
        let triple_str = triple.to_string();
        let ext = if triple_str.contains("windows") {
            "obj"
        } else {
            "o"
        };
        out_path.with_extension(ext)
    }

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
        let invocation = self.select_linker(triple)?;
        eprintln!(
            "LLVM backend: invoking linker {:?} (flavor {:?})",
            invocation.program, invocation.flavor
        );
        let mut cmd = std::process::Command::new(&invocation.program);
        cmd.args(&invocation.args);
        cmd.args(Self::linker_pre_args(triple, LinkOutput::Executable));
        cmd.arg(obj_path);
        cmd.arg("-o");
        cmd.arg(out_path);
        for lib in extra_libs {
            cmd.arg(lib);
        }
        cmd.args(Self::linker_post_args(triple, LinkOutput::Executable));
        Self::exec_command(cmd, "link executable")
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
        let invocation = self.select_linker(triple)?;
        eprintln!(
            "LLVM backend: invoking linker {:?} (flavor {:?})",
            invocation.program, invocation.flavor
        );
        let mut cmd = std::process::Command::new(&invocation.program);
        cmd.args(&invocation.args);
        cmd.args(Self::linker_pre_args(triple, LinkOutput::SharedLibrary));
        cmd.arg(obj_path);
        cmd.arg("-o");
        cmd.arg(out_path);
        for lib in extra_libs {
            cmd.arg(lib);
        }
        cmd.args(Self::linker_post_args(triple, LinkOutput::SharedLibrary));
        Self::exec_command(cmd, "link shared library")
    }

    fn link_static(&self, obj_path: &Path, out_path: &Path, triple: &str) -> Result<()> {
        let tool = Self::select_archiver(triple)?;
        eprintln!("LLVM backend: invoking archiver {}", tool.display());
        let mut cmd = std::process::Command::new(&tool);
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
        Self::exec_command(cmd, "archive static library")?;

        if triple.contains("apple") {
            let ranlib = Self::select_ranlib(triple)?;
            let mut cmd = std::process::Command::new(&ranlib);
            cmd.arg(out_path);
            Self::exec_command(cmd, "ranlib static library")?;
        }

        Ok(())
    }

    fn link_wasm(&self, obj_path: &Path, out_path: &Path, kind: LinkOutput) -> Result<()> {
        let program = Self::lookup_wasm_linker()?;
        eprintln!("LLVM backend: invoking wasm linker {:?}", program);
        let mut cmd = std::process::Command::new(&program);
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
        Self::exec_command(cmd, "link wasm artifact")
    }

    fn select_linker(&self, triple: &str) -> Result<LinkerInvocation> {
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
            let clang = Self::android_clang_path(triple)?;
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

    fn select_archiver(triple: &str) -> Result<PathBuf> {
        if let Ok(explicit) = env::var("SEEN_LLVM_ARCHIVER") {
            if !explicit.is_empty() {
                return Ok(PathBuf::from(explicit));
            }
        }

        if triple.contains("android") {
            return Self::android_tool_path("llvm-ar");
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

    fn select_ranlib(triple: &str) -> Result<PathBuf> {
        if let Ok(explicit) = env::var("SEEN_LLVM_RANLIB") {
            if !explicit.is_empty() {
                return Ok(PathBuf::from(explicit));
            }
        }
        if triple.contains("android") {
            return Self::android_tool_path("llvm-ranlib");
        }
        Ok(which::which("llvm-ranlib")
            .ok()
            .unwrap_or_else(|| PathBuf::from("ranlib")))
    }

    fn linker_pre_args(triple: &str, kind: LinkOutput) -> Vec<String> {
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

    fn linker_post_args(triple: &str, kind: LinkOutput) -> Vec<String> {
        match kind {
            LinkOutput::Executable => {
                if triple.contains("android") {
                    vec!["-lm".to_string()]
                } else if triple.contains("linux") {
                    vec!["-no-pie".to_string(), "-lm".to_string()]
                } else if triple.contains("apple") {
                    vec!["-lm".to_string()]
                } else {
                    vec![]
                }
            }
            LinkOutput::SharedLibrary => {
                if triple.contains("android") || triple.contains("linux") {
                    vec!["-lm".to_string()]
                } else {
                    vec![]
                }
            }
            _ => vec![],
        }
    }

    fn exec_command(mut cmd: std::process::Command, action: &str) -> Result<()> {
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

    fn lookup_wasm_linker() -> Result<PathBuf> {
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

    fn android_clang_path(triple: &str) -> Result<PathBuf> {
        let bin_dir = Self::android_bin_dir()?;
        let api = Self::android_api_level();
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

    fn android_tool_path(tool: &str) -> Result<PathBuf> {
        let bin_dir = Self::android_bin_dir()?;
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

    fn android_bin_dir() -> Result<PathBuf> {
        let ndk_home = env::var("ANDROID_NDK_HOME")
            .map_err(|_| anyhow!("ANDROID_NDK_HOME must be set to cross-compile for Android"))?;
        let host_tag = env::var("ANDROID_NDK_HOST")
            .ok()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| Self::default_ndk_host_tag());
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

    fn default_ndk_host_tag() -> String {
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

    fn android_api_level() -> String {
        env::var("ANDROID_API_LEVEL")
            .ok()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "21".to_string())
    }

    fn inject_runtime_stubs(&mut self, include_channel: bool) -> Result<()> {
        let saved_block = self.builder.get_insert_block();
        let saved_fn = self.current_fn;

        let i64_t = self.i64_t;
        let i32_t = self.ctx.i32_type();
        let ptr_t = self.i8_ptr_t;

        let mut stub_specs: Vec<(
            &str,
            inkwell::types::FunctionType<'ctx>,
            Box<dyn Fn(&mut Self, FunctionValue<'ctx>) -> Result<()> + '_>,
        )> = vec![
            (
                "seen_channel_new",
                ptr_t.fn_type(&[i64_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    let null_ptr = backend.i8_ptr_t.const_zero();
                    backend
                        .builder
                        .build_return(Some(&null_ptr.as_basic_value_enum()))
                        .map(|_| ())
                        .map_err(|e| anyhow!("{e:?}"))
                }),
            ),
            (
                "seen_channel_send",
                ptr_t.fn_type(&[ptr_t.into(), ptr_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    let ok = backend.i8_ptr_t.const_zero();
                    backend
                        .builder
                        .build_return(Some(&ok.as_basic_value_enum()))
                        .map(|_| ())
                        .map_err(|e| anyhow!("{e:?}"))
                }),
            ),
            (
                "seen_channel_recv",
                ptr_t.fn_type(&[ptr_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    let none = backend.i8_ptr_t.const_zero();
                    backend
                        .builder
                        .build_return(Some(&none.as_basic_value_enum()))
                        .map(|_| ())
                        .map_err(|e| anyhow!("{e:?}"))
                }),
            ),
            (
                "seen_channel_select",
                ptr_t.fn_type(&[ptr_t.into(), ptr_t.into(), i64_t.into()], false),
                Box::new(|backend: &mut Self, func| {
                    let _cases = func
                        .get_nth_param(0)
                        .ok_or_else(|| anyhow!("missing cases param"))?
                        .into_pointer_value();
                    let out_ptr = func
                        .get_nth_param(1)
                        .ok_or_else(|| anyhow!("missing out param"))?
                        .into_pointer_value();
                    let _count = func
                        .get_nth_param(2)
                        .ok_or_else(|| anyhow!("missing count param"))?;

                    let result_ty = backend.ty_select_result();
                    let typed_out = backend
                        .builder
                        .build_pointer_cast(
                            out_ptr,
                            result_ty.ptr_type(inkwell::AddressSpace::from(0u16)),
                            "select_stub_out",
                        )
                        .map_err(|e| anyhow!("{e:?}"))?;
                    let payload_ptr = backend
                        .builder
                        .build_struct_gep(result_ty, typed_out, 0, "select_stub_payload")
                        .map_err(|e| anyhow!("{e:?}"))?;
                    backend
                        .builder
                        .build_store(
                            payload_ptr,
                            backend.i8_ptr_t.const_zero().as_basic_value_enum(),
                        )
                        .map_err(|e| anyhow!("{e:?}"))?;
                    let index_ptr = backend
                        .builder
                        .build_struct_gep(result_ty, typed_out, 1, "select_stub_index")
                        .map_err(|e| anyhow!("{e:?}"))?;
                    backend
                        .builder
                        .build_store(index_ptr, backend.i64_t.const_zero().as_basic_value_enum())
                        .map_err(|e| anyhow!("{e:?}"))?;
                    let status_ptr = backend
                        .builder
                        .build_struct_gep(result_ty, typed_out, 2, "select_stub_status")
                        .map_err(|e| anyhow!("{e:?}"))?;
                    backend
                        .builder
                        .build_store(status_ptr, backend.i64_t.const_zero().as_basic_value_enum())
                        .map_err(|e| anyhow!("{e:?}"))?;

                    backend
                        .builder
                        .build_return(Some(&out_ptr.as_basic_value_enum()));
                    Ok(())
                }),
            ),
            (
                "seen_scope_push",
                self.ctx.void_type().fn_type(&[i32_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    backend.builder.build_return(None);
                    Ok(())
                }),
            ),
            (
                "seen_scope_pop",
                self.ctx.void_type().fn_type(&[i32_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    backend.builder.build_return(None);
                    Ok(())
                }),
            ),
            (
                "seen_spawn",
                ptr_t.fn_type(&[ptr_t.into(), i32_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    let handle = backend.i8_ptr_t.const_zero();
                    backend
                        .builder
                        .build_return(Some(&handle.as_basic_value_enum()));
                    Ok(())
                }),
            ),
            (
                "seen_box_int",
                ptr_t.fn_type(&[i64_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    backend
                        .builder
                        .build_return(Some(&backend.i8_ptr_t.const_zero().as_basic_value_enum()));
                    Ok(())
                }),
            ),
            (
                "seen_box_bool",
                ptr_t.fn_type(&[i32_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    backend
                        .builder
                        .build_return(Some(&backend.i8_ptr_t.const_zero().as_basic_value_enum()));
                    Ok(())
                }),
            ),
            (
                "seen_box_ptr",
                ptr_t.fn_type(&[ptr_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    backend
                        .builder
                        .build_return(Some(&backend.i8_ptr_t.const_zero().as_basic_value_enum()));
                    Ok(())
                }),
            ),
        ];

        if !include_channel {
            stub_specs.retain(|(name, _, _)| {
                !matches!(
                    *name,
                    "seen_channel_new"
                        | "seen_channel_send"
                        | "seen_channel_recv"
                        | "seen_channel_select"
                )
            });
        }

        for (name, ty, body) in stub_specs {
            let is_channel = matches!(
                name,
                "seen_channel_new"
                    | "seen_channel_send"
                    | "seen_channel_recv"
                    | "seen_channel_select"
            );
            if !include_channel && is_channel {
                if self.module.get_function(name).is_none() {
                    self.module.add_function(name, ty, None);
                }
                continue;
            }
            if self.module.get_function(name).is_some() {
                continue;
            }
            let func = self.module.add_function(name, ty, None);
            let entry = self.ctx.append_basic_block(func, "entry");
            self.builder.position_at_end(entry);
            let prev_fn = self.current_fn;
            self.current_fn = Some(func);
            body(self, func)?;
            self.current_fn = prev_fn;
        }

        match saved_block {
            Some(block) => self.builder.position_at_end(block),
            None => self.builder.clear_insertion_position(),
        }
        self.current_fn = saved_fn;
        Ok(())
    }

    fn target_machine_for(options: &TargetOptions<'_>) -> Result<(TargetTriple, TargetMachine)> {
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
                LlvmOptLevel::Default,
                RelocMode::Default,
                CodeModel::Default,
            )
            .ok_or_else(|| anyhow!("Create target machine failed"))?;
        Ok((triple, machine))
    }

    fn configure_module_target(&mut self, triple: &TargetTriple, machine: &TargetMachine) {
        self.module.set_triple(triple);
        let layout = machine.get_target_data().get_data_layout();
        self.module.set_data_layout(&layout);
    }

    fn object_file_path(out_path: &Path, triple: &TargetTriple) -> PathBuf {
        let triple_str = triple.to_string();
        let ext = if triple_str.contains("windows") {
            "obj"
        } else {
            "o"
        };
        out_path.with_extension(ext)
    }

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
        let invocation = self.select_linker(triple)?;
        eprintln!(
            "LLVM backend: invoking linker {:?} (flavor {:?})",
            invocation.program, invocation.flavor
        );
        let mut cmd = std::process::Command::new(&invocation.program);
        cmd.args(&invocation.args);
        cmd.args(Self::linker_pre_args(triple, LinkOutput::Executable));
        cmd.arg(obj_path);
        cmd.arg("-o");
        cmd.arg(out_path);
        for lib in extra_libs {
            cmd.arg(lib);
        }
        cmd.args(Self::linker_post_args(triple, LinkOutput::Executable));
        Self::exec_command(cmd, "link executable")
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
        let invocation = self.select_linker(triple)?;
        eprintln!(
            "LLVM backend: invoking linker {:?} (flavor {:?})",
            invocation.program, invocation.flavor
        );
        let mut cmd = std::process::Command::new(&invocation.program);
        cmd.args(&invocation.args);
        cmd.args(Self::linker_pre_args(triple, LinkOutput::SharedLibrary));
        cmd.arg(obj_path);
        cmd.arg("-o");
        cmd.arg(out_path);
        for lib in extra_libs {
            cmd.arg(lib);
        }
        cmd.args(Self::linker_post_args(triple, LinkOutput::SharedLibrary));
        Self::exec_command(cmd, "link shared library")
    }

    fn link_static(&self, obj_path: &Path, out_path: &Path, triple: &str) -> Result<()> {
        let tool = Self::select_archiver(triple)?;
        eprintln!("LLVM backend: invoking archiver {}", tool.display());
        let mut cmd = std::process::Command::new(&tool);
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
        Self::exec_command(cmd, "archive static library")?;

        if triple.contains("apple") {
            let ranlib = Self::select_ranlib(triple)?;
            let mut cmd = std::process::Command::new(&ranlib);
            cmd.arg(out_path);
            Self::exec_command(cmd, "ranlib static library")?;
        }

        Ok(())
    }

    fn link_wasm(&self, obj_path: &Path, out_path: &Path, kind: LinkOutput) -> Result<()> {
        let program = Self::lookup_wasm_linker()?;
        eprintln!("LLVM backend: invoking wasm linker {:?}", program);
        let mut cmd = std::process::Command::new(&program);
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
        Self::exec_command(cmd, "link wasm artifact")
    }

    fn select_linker(&self, triple: &str) -> Result<LinkerInvocation> {
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
            let clang = Self::android_clang_path(triple)?;
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

    fn select_archiver(triple: &str) -> Result<PathBuf> {
        if let Ok(explicit) = env::var("SEEN_LLVM_ARCHIVER") {
            if !explicit.is_empty() {
                return Ok(PathBuf::from(explicit));
            }
        }

        if triple.contains("android") {
            return Self::android_tool_path("llvm-ar");
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

    fn select_ranlib(triple: &str) -> Result<PathBuf> {
        if let Ok(explicit) = env::var("SEEN_LLVM_RANLIB") {
            if !explicit.is_empty() {
                return Ok(PathBuf::from(explicit));
            }
        }
        if triple.contains("android") {
            return Self::android_tool_path("llvm-ranlib");
        }
        Ok(which::which("llvm-ranlib")
            .ok()
            .unwrap_or_else(|| PathBuf::from("ranlib")))
    }

    fn linker_pre_args(triple: &str, kind: LinkOutput) -> Vec<String> {
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

    fn linker_post_args(triple: &str, kind: LinkOutput) -> Vec<String> {
        match kind {
            LinkOutput::Executable => {
                if triple.contains("android") {
                    vec!["-lm".to_string()]
                } else if triple.contains("linux") {
                    vec!["-no-pie".to_string(), "-lm".to_string()]
                } else if triple.contains("apple") {
                    vec!["-lm".to_string()]
                } else {
                    vec![]
                }
            }
            LinkOutput::SharedLibrary => {
                if triple.contains("android") || triple.contains("linux") {
                    vec!["-lm".to_string()]
                } else {
                    vec![]
                }
            }
            _ => vec![],
        }
    }

    fn exec_command(mut cmd: std::process::Command, action: &str) -> Result<()> {
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

    fn lookup_wasm_linker() -> Result<PathBuf> {
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

    fn android_clang_path(triple: &str) -> Result<PathBuf> {
        let bin_dir = Self::android_bin_dir()?;
        let api = Self::android_api_level();
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

    fn android_tool_path(tool: &str) -> Result<PathBuf> {
        let bin_dir = Self::android_bin_dir()?;
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

    fn android_bin_dir() -> Result<PathBuf> {
        let ndk_home = env::var("ANDROID_NDK_HOME")
            .map_err(|_| anyhow!("ANDROID_NDK_HOME must be set to cross-compile for Android"))?;
        let host_tag = env::var("ANDROID_NDK_HOST")
            .ok()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| Self::default_ndk_host_tag());
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

    fn default_ndk_host_tag() -> String {
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

    fn android_api_level() -> String {
        env::var("ANDROID_API_LEVEL")
            .ok()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "21".to_string())
    }

    fn inject_runtime_stubs(&mut self, include_channel: bool) -> Result<()> {
        let saved_block = self.builder.get_insert_block();
        let saved_fn = self.current_fn;

        let i64_t = self.i64_t;
        let i32_t = self.ctx.i32_type();
        let ptr_t = self.i8_ptr_t;

        let mut stub_specs: Vec<(
            &str,
            inkwell::types::FunctionType<'ctx>,
            Box<dyn Fn(&mut Self, FunctionValue<'ctx>) -> Result<()> + '_>,
        )> = vec![
            (
                "seen_channel_new",
                ptr_t.fn_type(&[i64_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    let null_ptr = backend.i8_ptr_t.const_zero();
                    backend
                        .builder
                        .build_return(Some(&null_ptr.as_basic_value_enum()))
                        .map(|_| ())
                        .map_err(|e| anyhow!("{e:?}"))
                }),
            ),
            (
                "seen_channel_send",
                ptr_t.fn_type(&[ptr_t.into(), ptr_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    let ok = backend.i8_ptr_t.const_zero();
                    backend
                        .builder
                        .build_return(Some(&ok.as_basic_value_enum()))
                        .map(|_| ())
                        .map_err(|e| anyhow!("{e:?}"))
                }),
            ),
            (
                "seen_channel_recv",
                ptr_t.fn_type(&[ptr_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    let none = backend.i8_ptr_t.const_zero();
                    backend
                        .builder
                        .build_return(Some(&none.as_basic_value_enum()))
                        .map(|_| ())
                        .map_err(|e| anyhow!("{e:?}"))
                }),
            ),
            (
                "seen_channel_select",
                ptr_t.fn_type(&[ptr_t.into(), ptr_t.into(), i64_t.into()], false),
                Box::new(|backend: &mut Self, func| {
                    let _cases = func
                        .get_nth_param(0)
                        .ok_or_else(|| anyhow!("missing cases param"))?
                        .into_pointer_value();
                    let out_ptr = func
                        .get_nth_param(1)
                        .ok_or_else(|| anyhow!("missing out param"))?
                        .into_pointer_value();
                    let _count = func
                        .get_nth_param(2)
                        .ok_or_else(|| anyhow!("missing count param"))?;

                    let result_ty = backend.ty_select_result();
                    let typed_out = backend
                        .builder
                        .build_pointer_cast(
                            out_ptr,
                            result_ty.ptr_type(inkwell::AddressSpace::from(0u16)),
                            "select_stub_out",
                        )
                        .map_err(|e| anyhow!("{e:?}"))?;
                    let payload_ptr = backend
                        .builder
                        .build_struct_gep(result_ty, typed_out, 0, "select_stub_payload")
                        .map_err(|e| anyhow!("{e:?}"))?;
                    backend
                        .builder
                        .build_store(
                            payload_ptr,
                            backend.i8_ptr_t.const_zero().as_basic_value_enum(),
                        )
                        .map_err(|e| anyhow!("{e:?}"))?;
                    let index_ptr = backend
                        .builder
                        .build_struct_gep(result_ty, typed_out, 1, "select_stub_index")
                        .map_err(|e| anyhow!("{e:?}"))?;
                    backend
                        .builder
                        .build_store(index_ptr, backend.i64_t.const_zero().as_basic_value_enum())
                        .map_err(|e| anyhow!("{e:?}"))?;
                    let status_ptr = backend
                        .builder
                        .build_struct_gep(result_ty, typed_out, 2, "select_stub_status")
                        .map_err(|e| anyhow!("{e:?}"))?;
                    backend
                        .builder
                        .build_store(status_ptr, backend.i64_t.const_zero().as_basic_value_enum())
                        .map_err(|e| anyhow!("{e:?}"))?;

                    backend
                        .builder
                        .build_return(Some(&out_ptr.as_basic_value_enum()));
                    Ok(())
                }),
            ),
            (
                "seen_scope_push",
                self.ctx.void_type().fn_type(&[i32_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    backend.builder.build_return(None);
                    Ok(())
                }),
            ),
            (
                "seen_scope_pop",
                self.ctx.void_type().fn_type(&[i32_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    backend.builder.build_return(None);
                    Ok(())
                }),
            ),
            (
                "seen_spawn",
                ptr_t.fn_type(&[ptr_t.into(), i32_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    let handle = backend.i8_ptr_t.const_zero();
                    backend
                        .builder
                        .build_return(Some(&handle.as_basic_value_enum()));
                    Ok(())
                }),
            ),
            (
                "seen_box_int",
                ptr_t.fn_type(&[i64_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    backend
                        .builder
                        .build_return(Some(&backend.i8_ptr_t.const_zero().as_basic_value_enum()));
                    Ok(())
                }),
            ),
            (
                "seen_box_bool",
                ptr_t.fn_type(&[i32_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    backend
                        .builder
                        .build_return(Some(&backend.i8_ptr_t.const_zero().as_basic_value_enum()));
                    Ok(())
                }),
            ),
            (
                "seen_box_ptr",
                ptr_t.fn_type(&[ptr_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    backend
                        .builder
                        .build_return(Some(&backend.i8_ptr_t.const_zero().as_basic_value_enum()));
                    Ok(())
                }),
            ),
        ];

        if !include_channel {
            stub_specs.retain(|(name, _, _)| {
                !matches!(
                    *name,
                    "seen_channel_new"
                        | "seen_channel_send"
                        | "seen_channel_recv"
                        | "seen_channel_select"
                )
            });
        }

        for (name, ty, body) in stub_specs {
            let is_channel = matches!(
                name,
                "seen_channel_new"
                    | "seen_channel_send"
                    | "seen_channel_recv"
                    | "seen_channel_select"
            );
            if !include_channel && is_channel {
                if self.module.get_function(name).is_none() {
                    self.module.add_function(name, ty, None);
                }
                continue;
            }
            if self.module.get_function(name).is_some() {
                continue;
            }
            let func = self.module.add_function(name, ty, None);
            let entry = self.ctx.append_basic_block(func, "entry");
            self.builder.position_at_end(entry);
            let prev_fn = self.current_fn;
            self.current_fn = Some(func);
            body(self, func)?;
            self.current_fn = prev_fn;
        }

        match saved_block {
            Some(block) => self.builder.position_at_end(block),
            None => self.builder.clear_insertion_position(),
        }
        self.current_fn = saved_fn;
        Ok(())
    }

    fn target_machine_for(options: &TargetOptions<'_>) -> Result<(TargetTriple, TargetMachine)> {
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
                LlvmOptLevel::Default,
                RelocMode::Default,
                CodeModel::Default,
            )
            .ok_or_else(|| anyhow!("Create target machine failed"))?;
        Ok((triple, machine))
    }

    fn configure_module_target(&mut self, triple: &TargetTriple, machine: &TargetMachine) {
        self.module.set_triple(triple);
        let layout = machine.get_target_data().get_data_layout();
        self.module.set_data_layout(&layout);
    }

    fn object_file_path(out_path: &Path, triple: &TargetTriple) -> PathBuf {
        let triple_str = triple.to_string();
        let ext = if triple_str.contains("windows") {
            "obj"
        } else {
            "o"
        };
        out_path.with_extension(ext)
    }

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
        let invocation = self.select_linker(triple)?;
        eprintln!(
            "LLVM backend: invoking linker {:?} (flavor {:?})",
            invocation.program, invocation.flavor
        );
        let mut cmd = std::process::Command::new(&invocation.program);
        cmd.args(&invocation.args);
        cmd.args(Self::linker_pre_args(triple, LinkOutput::Executable));
        cmd.arg(obj_path);
        cmd.arg("-o");
        cmd.arg(out_path);
        for lib in extra_libs {
            cmd.arg(lib);
        }
        cmd.args(Self::linker_post_args(triple, LinkOutput::Executable));
        Self::exec_command(cmd, "link executable")
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
        let invocation = self.select_linker(triple)?;
        eprintln!(
            "LLVM backend: invoking linker {:?} (flavor {:?})",
            invocation.program, invocation.flavor
        );
        let mut cmd = std::process::Command::new(&invocation.program);
        cmd.args(&invocation.args);
        cmd.args(Self::linker_pre_args(triple, LinkOutput::SharedLibrary));
        cmd.arg(obj_path);
        cmd.arg("-o");
        cmd.arg(out_path);
        for lib in extra_libs {
            cmd.arg(lib);
        }
        cmd.args(Self::linker_post_args(triple, LinkOutput::SharedLibrary));
        Self::exec_command(cmd, "link shared library")
    }

    fn link_static(&self, obj_path: &Path, out_path: &Path, triple: &str) -> Result<()> {
        let tool = Self::select_archiver(triple)?;
        eprintln!("LLVM backend: invoking archiver {}", tool.display());
        let mut cmd = std::process::Command::new(&tool);
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
        Self::exec_command(cmd, "archive static library")?;

        if triple.contains("apple") {
            let ranlib = Self::select_ranlib(triple)?;
            let mut cmd = std::process::Command::new(&ranlib);
            cmd.arg(out_path);
            Self::exec_command(cmd, "ranlib static library")?;
        }

        Ok(())
    }

    fn link_wasm(&self, obj_path: &Path, out_path: &Path, kind: LinkOutput) -> Result<()> {
        let program = Self::lookup_wasm_linker()?;
        eprintln!("LLVM backend: invoking wasm linker {:?}", program);
        let mut cmd = std::process::Command::new(&program);
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
        Self::exec_command(cmd, "link wasm artifact")
    }

    fn select_linker(&self, triple: &str) -> Result<LinkerInvocation> {
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
            let clang = Self::android_clang_path(triple)?;
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

    fn select_archiver(triple: &str) -> Result<PathBuf> {
        if let Ok(explicit) = env::var("SEEN_LLVM_ARCHIVER") {
            if !explicit.is_empty() {
                return Ok(PathBuf::from(explicit));
            }
        }

        if triple.contains("android") {
            return Self::android_tool_path("llvm-ar");
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

    fn select_ranlib(triple: &str) -> Result<PathBuf> {
        if let Ok(explicit) = env::var("SEEN_LLVM_RANLIB") {
            if !explicit.is_empty() {
                return Ok(PathBuf::from(explicit));
            }
        }
        if triple.contains("android") {
            return Self::android_tool_path("llvm-ranlib");
        }
        Ok(which::which("llvm-ranlib")
            .ok()
            .unwrap_or_else(|| PathBuf::from("ranlib")))
    }

    fn linker_pre_args(triple: &str, kind: LinkOutput) -> Vec<String> {
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

    fn linker_post_args(triple: &str, kind: LinkOutput) -> Vec<String> {
        match kind {
            LinkOutput::Executable => {
                if triple.contains("android") {
                    vec!["-lm".to_string()]
                } else if triple.contains("linux") {
                    vec!["-no-pie".to_string(), "-lm".to_string()]
                } else if triple.contains("apple") {
                    vec!["-lm".to_string()]
                } else {
                    vec![]
                }
            }
            LinkOutput::SharedLibrary => {
                if triple.contains("android") || triple.contains("linux") {
                    vec!["-lm".to_string()]
                } else {
                    vec![]
                }
            }
            _ => vec![],
        }
    }

    fn exec_command(mut cmd: std::process::Command, action: &str) -> Result<()> {
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

    fn lookup_wasm_linker() -> Result<PathBuf> {
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

    fn android_clang_path(triple: &str) -> Result<PathBuf> {
        let bin_dir = Self::android_bin_dir()?;
        let api = Self::android_api_level();
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

    fn android_tool_path(tool: &str) -> Result<PathBuf> {
        let bin_dir = Self::android_bin_dir()?;
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

    fn android_bin_dir() -> Result<PathBuf> {
        let ndk_home = env::var("ANDROID_NDK_HOME")
            .map_err(|_| anyhow!("ANDROID_NDK_HOME must be set to cross-compile for Android"))?;
        let host_tag = env::var("ANDROID_NDK_HOST")
            .ok()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| Self::default_ndk_host_tag());
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

    fn default_ndk_host_tag() -> String {
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

    fn android_api_level() -> String {
        env::var("ANDROID_API_LEVEL")
            .ok()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "21".to_string())
    }

    fn inject_runtime_stubs(&mut self, include_channel: bool) -> Result<()> {
        let saved_block = self.builder.get_insert_block();
        let saved_fn = self.current_fn;

        let i64_t = self.i64_t;
        let i32_t = self.ctx.i32_type();
        let ptr_t = self.i8_ptr_t;

        let mut stub_specs: Vec<(
            &str,
            inkwell::types::FunctionType<'ctx>,
            Box<dyn Fn(&mut Self, FunctionValue<'ctx>) -> Result<()> + '_>,
        )> = vec![
            (
                "seen_channel_new",
                ptr_t.fn_type(&[i64_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    let null_ptr = backend.i8_ptr_t.const_zero();
                    backend
                        .builder
                        .build_return(Some(&null_ptr.as_basic_value_enum()))
                        .map(|_| ())
                        .map_err(|e| anyhow!("{e:?}"))
                }),
            ),
            (
                "seen_channel_send",
                ptr_t.fn_type(&[ptr_t.into(), ptr_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    let ok = backend.i8_ptr_t.const_zero();
                    backend
                        .builder
                        .build_return(Some(&ok.as_basic_value_enum()))
                        .map(|_| ())
                        .map_err(|e| anyhow!("{e:?}"))
                }),
            ),
            (
                "seen_channel_recv",
                ptr_t.fn_type(&[ptr_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    let none = backend.i8_ptr_t.const_zero();
                    backend
                        .builder
                        .build_return(Some(&none.as_basic_value_enum()))
                        .map(|_| ())
                        .map_err(|e| anyhow!("{e:?}"))
                }),
            ),
            (
                "seen_channel_select",
                ptr_t.fn_type(&[ptr_t.into(), ptr_t.into(), i64_t.into()], false),
                Box::new(|backend: &mut Self, func| {
                    let _cases = func
                        .get_nth_param(0)
                        .ok_or_else(|| anyhow!("missing cases param"))?
                        .into_pointer_value();
                    let out_ptr = func
                        .get_nth_param(1)
                        .ok_or_else(|| anyhow!("missing out param"))?
                        .into_pointer_value();
                    let _count = func
                        .get_nth_param(2)
                        .ok_or_else(|| anyhow!("missing count param"))?;

                    let result_ty = backend.ty_select_result();
                    let typed_out = backend
                        .builder
                        .build_pointer_cast(
                            out_ptr,
                            result_ty.ptr_type(inkwell::AddressSpace::from(0u16)),
                            "select_stub_out",
                        )
                        .map_err(|e| anyhow!("{e:?}"))?;
                    let payload_ptr = backend
                        .builder
                        .build_struct_gep(result_ty, typed_out, 0, "select_stub_payload")
                        .map_err(|e| anyhow!("{e:?}"))?;
                    backend
                        .builder
                        .build_store(
                            payload_ptr,
                            backend.i8_ptr_t.const_zero().as_basic_value_enum(),
                        )
                        .map_err(|e| anyhow!("{e:?}"))?;
                    let index_ptr = backend
                        .builder
                        .build_struct_gep(result_ty, typed_out, 1, "select_stub_index")
                        .map_err(|e| anyhow!("{e:?}"))?;
                    backend
                        .builder
                        .build_store(index_ptr, backend.i64_t.const_zero().as_basic_value_enum())
                        .map_err(|e| anyhow!("{e:?}"))?;
                    let status_ptr = backend
                        .builder
                        .build_struct_gep(result_ty, typed_out, 2, "select_stub_status")
                        .map_err(|e| anyhow!("{e:?}"))?;
                    backend
                        .builder
                        .build_store(status_ptr, backend.i64_t.const_zero().as_basic_value_enum())
                        .map_err(|e| anyhow!("{e:?}"))?;

                    backend
                        .builder
                        .build_return(Some(&out_ptr.as_basic_value_enum()));
                    Ok(())
                }),
            ),
            (
                "seen_scope_push",
                self.ctx.void_type().fn_type(&[i32_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    backend.builder.build_return(None);
                    Ok(())
                }),
            ),
            (
                "seen_scope_pop",
                self.ctx.void_type().fn_type(&[i32_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    backend.builder.build_return(None);
                    Ok(())
                }),
            ),
            (
                "seen_spawn",
                ptr_t.fn_type(&[ptr_t.into(), i32_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    let handle = backend.i8_ptr_t.const_zero();
                    backend
                        .builder
                        .build_return(Some(&handle.as_basic_value_enum()));
                    Ok(())
                }),
            ),
            (
                "seen_box_int",
                ptr_t.fn_type(&[i64_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    backend
                        .builder
                        .build_return(Some(&backend.i8_ptr_t.const_zero().as_basic_value_enum()));
                    Ok(())
                }),
            ),
            (
                "seen_box_bool",
                ptr_t.fn_type(&[i32_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    backend
                        .builder
                        .build_return(Some(&backend.i8_ptr_t.const_zero().as_basic_value_enum()));
                    Ok(())
                }),
            ),
            (
                "seen_box_ptr",
                ptr_t.fn_type(&[ptr_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    backend
                        .builder
                        .build_return(Some(&backend.i8_ptr_t.const_zero().as_basic_value_enum()));
                    Ok(())
                }),
            ),
        ];

        if !include_channel {
            stub_specs.retain(|(name, _, _)| {
                !matches!(
                    *name,
                    "seen_channel_new"
                        | "seen_channel_send"
                        | "seen_channel_recv"
                        | "seen_channel_select"
                )
            });
        }

        for (name, ty, body) in stub_specs {
            let is_channel = matches!(
                name,
                "seen_channel_new"
                    | "seen_channel_send"
                    | "seen_channel_recv"
                    | "seen_channel_select"
            );
            if !include_channel && is_channel {
                if self.module.get_function(name).is_none() {
                    self.module.add_function(name, ty, None);
                }
                continue;
            }
            if self.module.get_function(name).is_some() {
                continue;
            }
            let func = self.module.add_function(name, ty, None);
            let entry = self.ctx.append_basic_block(func, "entry");
            self.builder.position_at_end(entry);
            let prev_fn = self.current_fn;
            self.current_fn = Some(func);
            body(self, func)?;
            self.current_fn = prev_fn;
        }

        match saved_block {
            Some(block) => self.builder.position_at_end(block),
            None => self.builder.clear_insertion_position(),
        }
        self.current_fn = saved_fn;
        Ok(())
    }

    fn target_machine_for(options: &TargetOptions<'_>) -> Result<(TargetTriple, TargetMachine)> {
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
                LlvmOptLevel::Default,
                RelocMode::Default,
                CodeModel::Default,
            )
            .ok_or_else(|| anyhow!("Create target machine failed"))?;
        Ok((triple, machine))
    }

    fn configure_module_target(&mut self, triple: &TargetTriple, machine: &TargetMachine) {
        self.module.set_triple(triple);
        let layout = machine.get_target_data().get_data_layout();
        self.module.set_data_layout(&layout);
    }

    fn object_file_path(out_path: &Path, triple: &TargetTriple) -> PathBuf {
        let triple_str = triple.to_string();
        let ext = if triple_str.contains("windows") {
            "obj"
        } else {
            "o"
        };
        out_path.with_extension(ext)
    }

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
        let invocation = self.select_linker(triple)?;
        eprintln!(
            "LLVM backend: invoking linker {:?} (flavor {:?})",
            invocation.program, invocation.flavor
        );
        let mut cmd = std::process::Command::new(&invocation.program);
        cmd.args(&invocation.args);
        cmd.args(Self::linker_pre_args(triple, LinkOutput::Executable));
        cmd.arg(obj_path);
        cmd.arg("-o");
        cmd.arg(out_path);
        for lib in extra_libs {
            cmd.arg(lib);
        }
        cmd.args(Self::linker_post_args(triple, LinkOutput::Executable));
        Self::exec_command(cmd, "link executable")
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
        let invocation = self.select_linker(triple)?;
        eprintln!(
            "LLVM backend: invoking linker {:?} (flavor {:?})",
            invocation.program, invocation.flavor
        );
        let mut cmd = std::process::Command::new(&invocation.program);
        cmd.args(&invocation.args);
        cmd.args(Self::linker_pre_args(triple, LinkOutput::SharedLibrary));
        cmd.arg(obj_path);
        cmd.arg("-o");
        cmd.arg(out_path);
        for lib in extra_libs {
            cmd.arg(lib);
        }
        cmd.args(Self::linker_post_args(triple, LinkOutput::SharedLibrary));
        Self::exec_command(cmd, "link shared library")
    }

    fn link_static(&self, obj_path: &Path, out_path: &Path, triple: &str) -> Result<()> {
        let tool = Self::select_archiver(triple)?;
        eprintln!("LLVM backend: invoking archiver {}", tool.display());
        let mut cmd = std::process::Command::new(&tool);
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
        Self::exec_command(cmd, "archive static library")?;

        if triple.contains("apple") {
            let ranlib = Self::select_ranlib(triple)?;
            let mut cmd = std::process::Command::new(&ranlib);
            cmd.arg(out_path);
            Self::exec_command(cmd, "ranlib static library")?;
        }

        Ok(())
    }

    fn link_wasm(&self, obj_path: &Path, out_path: &Path, kind: LinkOutput) -> Result<()> {
        let program = Self::lookup_wasm_linker()?;
        eprintln!("LLVM backend: invoking wasm linker {:?}", program);
        let mut cmd = std::process::Command::new(&program);
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
        Self::exec_command(cmd, "link wasm artifact")
    }

    fn select_linker(&self, triple: &str) -> Result<LinkerInvocation> {
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
            let clang = Self::android_clang_path(triple)?;
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

    fn select_archiver(triple: &str) -> Result<PathBuf> {
        if let Ok(explicit) = env::var("SEEN_LLVM_ARCHIVER") {
            if !explicit.is_empty() {
                return Ok(PathBuf::from(explicit));
            }
        }

        if triple.contains("android") {
            return Self::android_tool_path("llvm-ar");
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

    fn select_ranlib(triple: &str) -> Result<PathBuf> {
        if let Ok(explicit) = env::var("SEEN_LLVM_RANLIB") {
            if !explicit.is_empty() {
                return Ok(PathBuf::from(explicit));
            }
        }
        if triple.contains("android") {
            return Self::android_tool_path("llvm-ranlib");
        }
        Ok(which::which("llvm-ranlib")
            .ok()
            .unwrap_or_else(|| PathBuf::from("ranlib")))
    }

    fn linker_pre_args(triple: &str, kind: LinkOutput) -> Vec<String> {
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

    fn linker_post_args(triple: &str, kind: LinkOutput) -> Vec<String> {
        match kind {
            LinkOutput::Executable => {
                if triple.contains("android") {
                    vec!["-lm".to_string()]
                } else if triple.contains("linux") {
                    vec!["-no-pie".to_string(), "-lm".to_string()]
                } else if triple.contains("apple") {
                    vec!["-lm".to_string()]
                } else {
                    vec![]
                }
            }
            LinkOutput::SharedLibrary => {
                if triple.contains("android") || triple.contains("linux") {
                    vec!["-lm".to_string()]
                } else {
                    vec![]
                }
            }
            _ => vec![],
        }
    }

    fn exec_command(mut cmd: std::process::Command, action: &str) -> Result<()> {
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

    fn lookup_wasm_linker() -> Result<PathBuf> {
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

    fn android_clang_path(triple: &str) -> Result<PathBuf> {
        let bin_dir = Self::android_bin_dir()?;
        let api = Self::android_api_level();
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

    fn android_tool_path(tool: &str) -> Result<PathBuf> {
        let bin_dir = Self::android_bin_dir()?;
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

    fn android_bin_dir() -> Result<PathBuf> {
        let ndk_home = env::var("ANDROID_NDK_HOME")
            .map_err(|_| anyhow!("ANDROID_NDK_HOME must be set to cross-compile for Android"))?;
        let host_tag = env::var("ANDROID_NDK_HOST")
            .ok()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| Self::default_ndk_host_tag());
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

    fn default_ndk_host_tag() -> String {
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

    fn android_api_level() -> String {
        env::var("ANDROID_API_LEVEL")
            .ok()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "21".to_string())
    }

    fn inject_runtime_stubs(&mut self, include_channel: bool) -> Result<()> {
        let saved_block = self.builder.get_insert_block();
        let saved_fn = self.current_fn;

        let i64_t = self.i64_t;
        let i32_t = self.ctx.i32_type();
        let ptr_t = self.i8_ptr_t;

        let mut stub_specs: Vec<(
            &str,
            inkwell::types::FunctionType<'ctx>,
            Box<dyn Fn(&mut Self, FunctionValue<'ctx>) -> Result<()> + '_>,
        )> = vec![
            (
                "seen_channel_new",
                ptr_t.fn_type(&[i64_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    let null_ptr = backend.i8_ptr_t.const_zero();
                    backend
                        .builder
                        .build_return(Some(&null_ptr.as_basic_value_enum()))
                        .map(|_| ())
                        .map_err(|e| anyhow!("{e:?}"))
                }),
            ),
            (
                "seen_channel_send",
                ptr_t.fn_type(&[ptr_t.into(), ptr_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    let ok = backend.i8_ptr_t.const_zero();
                    backend
                        .builder
                        .build_return(Some(&ok.as_basic_value_enum()))
                        .map(|_| ())
                        .map_err(|e| anyhow!("{e:?}"))
                }),
            ),
            (
                "seen_channel_recv",
                ptr_t.fn_type(&[ptr_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    let none = backend.i8_ptr_t.const_zero();
                    backend
                        .builder
                        .build_return(Some(&none.as_basic_value_enum()))
                        .map(|_| ())
                        .map_err(|e| anyhow!("{e:?}"))
                }),
            ),
            (
                "seen_channel_select",
                ptr_t.fn_type(&[ptr_t.into(), ptr_t.into(), i64_t.into()], false),
                Box::new(|backend: &mut Self, func| {
                    let _cases = func
                        .get_nth_param(0)
                        .ok_or_else(|| anyhow!("missing cases param"))?
                        .into_pointer_value();
                    let out_ptr = func
                        .get_nth_param(1)
                        .ok_or_else(|| anyhow!("missing out param"))?
                        .into_pointer_value();
                    let _count = func
                        .get_nth_param(2)
                        .ok_or_else(|| anyhow!("missing count param"))?;

                    let result_ty = backend.ty_select_result();
                    let typed_out = backend
                        .builder
                        .build_pointer_cast(
                            out_ptr,
                            result_ty.ptr_type(inkwell::AddressSpace::from(0u16)),
                            "select_stub_out",
                        )
                        .map_err(|e| anyhow!("{e:?}"))?;
                    let payload_ptr = backend
                        .builder
                        .build_struct_gep(result_ty, typed_out, 0, "select_stub_payload")
                        .map_err(|e| anyhow!("{e:?}"))?;
                    backend
                        .builder
                        .build_store(
                            payload_ptr,
                            backend.i8_ptr_t.const_zero().as_basic_value_enum(),
                        )
                        .map_err(|e| anyhow!("{e:?}"))?;
                    let index_ptr = backend
                        .builder
                        .build_struct_gep(result_ty, typed_out, 1, "select_stub_index")
                        .map_err(|e| anyhow!("{e:?}"))?;
                    backend
                        .builder
                        .build_store(index_ptr, backend.i64_t.const_zero().as_basic_value_enum())
                        .map_err(|e| anyhow!("{e:?}"))?;
                    let status_ptr = backend
                        .builder
                        .build_struct_gep(result_ty, typed_out, 2, "select_stub_status")
                        .map_err(|e| anyhow!("{e:?}"))?;
                    backend
                        .builder
                        .build_store(status_ptr, backend.i64_t.const_zero().as_basic_value_enum())
                        .map_err(|e| anyhow!("{e:?}"))?;

                    backend
                        .builder
                        .build_return(Some(&out_ptr.as_basic_value_enum()));
                    Ok(())
                }),
            ),
            (
                "seen_scope_push",
                self.ctx.void_type().fn_type(&[i32_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    backend.builder.build_return(None);
                    Ok(())
                }),
            ),
            (
                "seen_scope_pop",
                self.ctx.void_type().fn_type(&[i32_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    backend.builder.build_return(None);
                    Ok(())
                }),
            ),
            (
                "seen_spawn",
                ptr_t.fn_type(&[ptr_t.into(), i32_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    let handle = backend.i8_ptr_t.const_zero();
                    backend
                        .builder
                        .build_return(Some(&handle.as_basic_value_enum()));
                    Ok(())
                }),
            ),
            (
                "seen_box_int",
                ptr_t.fn_type(&[i64_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    backend
                        .builder
                        .build_return(Some(&backend.i8_ptr_t.const_zero().as_basic_value_enum()));
                    Ok(())
                }),
            ),
            (
                "seen_box_bool",
                ptr_t.fn_type(&[i32_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    backend
                        .builder
                        .build_return(Some(&backend.i8_ptr_t.const_zero().as_basic_value_enum()));
                    Ok(())
                }),
            ),
            (
                "seen_box_ptr",
                ptr_t.fn_type(&[ptr_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    backend
                        .builder
                        .build_return(Some(&backend.i8_ptr_t.const_zero().as_basic_value_enum()));
                    Ok(())
                }),
            ),
        ];

        if !include_channel {
            stub_specs.retain(|(name, _, _)| {
                !matches!(
                    *name,
                    "seen_channel_new"
                        | "seen_channel_send"
                        | "seen_channel_recv"
                        | "seen_channel_select"
                )
            });
        }

        for (name, ty, body) in stub_specs {
            let is_channel = matches!(
                name,
                "seen_channel_new"
                    | "seen_channel_send"
                    | "seen_channel_recv"
                    | "seen_channel_select"
            );
            if !include_channel && is_channel {
                if self.module.get_function(name).is_none() {
                    self.module.add_function(name, ty, None);
                }
                continue;
            }
            if self.module.get_function(name).is_some() {
                continue;
            }
            let func = self.module.add_function(name, ty, None);
            let entry = self.ctx.append_basic_block(func, "entry");
            self.builder.position_at_end(entry);
            let prev_fn = self.current_fn;
            self.current_fn = Some(func);
            body(self, func)?;
            self.current_fn = prev_fn;
        }

        match saved_block {
            Some(block) => self.builder.position_at_end(block),
            None => self.builder.clear_insertion_position(),
        }
        self.current_fn = saved_fn;
        Ok(())
    }

    fn target_machine_for(options: &TargetOptions<'_>) -> Result<(TargetTriple, TargetMachine)> {
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
                LlvmOptLevel::Default,
                RelocMode::Default,
                CodeModel::Default,
            )
            .ok_or_else(|| anyhow!("Create target machine failed"))?;
        Ok((triple, machine))
    }

    fn configure_module_target(&mut self, triple: &TargetTriple, machine: &TargetMachine) {
        self.module.set_triple(triple);
        let layout = machine.get_target_data().get_data_layout();
        self.module.set_data_layout(&layout);
    }

    fn object_file_path(out_path: &Path, triple: &TargetTriple) -> PathBuf {
        let triple_str = triple.to_string();
        let ext = if triple_str.contains("windows") {
            "obj"
        } else {
            "o"
        };
        out_path.with_extension(ext)
    }

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
        let invocation = self.select_linker(triple)?;
        eprintln!(
            "LLVM backend: invoking linker {:?} (flavor {:?})",
            invocation.program, invocation.flavor
        );
        let mut cmd = std::process::Command::new(&invocation.program);
        cmd.args(&invocation.args);
        cmd.args(Self::linker_pre_args(triple, LinkOutput::Executable));
        cmd.arg(obj_path);
        cmd.arg("-o");
        cmd.arg(out_path);
        for lib in extra_libs {
            cmd.arg(lib);
        }
        cmd.args(Self::linker_post_args(triple, LinkOutput::Executable));
        Self::exec_command(cmd, "link executable")
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
        let invocation = self.select_linker(triple)?;
        eprintln!(
            "LLVM backend: invoking linker {:?} (flavor {:?})",
            invocation.program, invocation.flavor
        );
        let mut cmd = std::process::Command::new(&invocation.program);
        cmd.args(&invocation.args);
        cmd.args(Self::linker_pre_args(triple, LinkOutput::SharedLibrary));
        cmd.arg(obj_path);
        cmd.arg("-o");
        cmd.arg(out_path);
        for lib in extra_libs {
            cmd.arg(lib);
        }
        cmd.args(Self::linker_post_args(triple, LinkOutput::SharedLibrary));
        Self::exec_command(cmd, "link shared library")
    }

    fn link_static(&self, obj_path: &Path, out_path: &Path, triple: &str) -> Result<()> {
        let tool = Self::select_archiver(triple)?;
        eprintln!("LLVM backend: invoking archiver {}", tool.display());
        let mut cmd = std::process::Command::new(&tool);
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
        Self::exec_command(cmd, "archive static library")?;

        if triple.contains("apple") {
            let ranlib = Self::select_ranlib(triple)?;
            let mut cmd = std::process::Command::new(&ranlib);
            cmd.arg(out_path);
            Self::exec_command(cmd, "ranlib static library")?;
        }

        Ok(())
    }

    fn link_wasm(&self, obj_path: &Path, out_path: &Path, kind: LinkOutput) -> Result<()> {
        let program = Self::lookup_wasm_linker()?;
        eprintln!("LLVM backend: invoking wasm linker {:?}", program);
        let mut cmd = std::process::Command::new(&program);
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
        Self::exec_command(cmd, "link wasm artifact")
    }

    fn select_linker(&self, triple: &str) -> Result<LinkerInvocation> {
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
            let clang = Self::android_clang_path(triple)?;
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

    fn select_archiver(triple: &str) -> Result<PathBuf> {
        if let Ok(explicit) = env::var("SEEN_LLVM_ARCHIVER") {
            if !explicit.is_empty() {
                return Ok(PathBuf::from(explicit));
            }
        }

        if triple.contains("android") {
            return Self::android_tool_path("llvm-ar");
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

    fn select_ranlib(triple: &str) -> Result<PathBuf> {
        if let Ok(explicit) = env::var("SEEN_LLVM_RANLIB") {
            if !explicit.is_empty() {
                return Ok(PathBuf::from(explicit));
            }
        }
        if triple.contains("android") {
            return Self::android_tool_path("llvm-ranlib");
        }
        Ok(which::which("llvm-ranlib")
            .ok()
            .unwrap_or_else(|| PathBuf::from("ranlib")))
    }

    fn linker_pre_args(triple: &str, kind: LinkOutput) -> Vec<String> {
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

    fn linker_post_args(triple: &str, kind: LinkOutput) -> Vec<String> {
        match kind {
            LinkOutput::Executable => {
                if triple.contains("android") {
                    vec!["-lm".to_string()]
                } else if triple.contains("linux") {
                    vec!["-no-pie".to_string(), "-lm".to_string()]
                } else if triple.contains("apple") {
                    vec!["-lm".to_string()]
                } else {
                    vec![]
                }
            }
            LinkOutput::SharedLibrary => {
                if triple.contains("android") || triple.contains("linux") {
                    vec!["-lm".to_string()]
                } else {
                    vec![]
                }
            }
            _ => vec![],
        }
    }

    fn exec_command(mut cmd: std::process::Command, action: &str) -> Result<()> {
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

    fn lookup_wasm_linker() -> Result<PathBuf> {
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

    fn android_clang_path(triple: &str) -> Result<PathBuf> {
        let bin_dir = Self::android_bin_dir()?;
        let api = Self::android_api_level();
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

    fn android_tool_path(tool: &str) -> Result<PathBuf> {
        let bin_dir = Self::android_bin_dir()?;
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

    fn android_bin_dir() -> Result<PathBuf> {
        let ndk_home = env::var("ANDROID_NDK_HOME")
            .map_err(|_| anyhow!("ANDROID_NDK_HOME must be set to cross-compile for Android"))?;
        let host_tag = env::var("ANDROID_NDK_HOST")
            .ok()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| Self::default_ndk_host_tag());
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

    fn default_ndk_host_tag() -> String {
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

    fn android_api_level() -> String {
        env::var("ANDROID_API_LEVEL")
            .ok()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "21".to_string())
    }

    fn inject_runtime_stubs(&mut self, include_channel: bool) -> Result<()> {
        let saved_block = self.builder.get_insert_block();
        let saved_fn = self.current_fn;

        let i64_t = self.i64_t;
        let i32_t = self.ctx.i32_type();
        let ptr_t = self.i8_ptr_t;

        let mut stub_specs: Vec<(
            &str,
            inkwell::types::FunctionType<'ctx>,
            Box<dyn Fn(&mut Self, FunctionValue<'ctx>) -> Result<()> + '_>,
        )> = vec![
            (
                "seen_channel_new",
                ptr_t.fn_type(&[i64_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    let null_ptr = backend.i8_ptr_t.const_zero();
                    backend
                        .builder
                        .build_return(Some(&null_ptr.as_basic_value_enum()))
                        .map(|_| ())
                        .map_err(|e| anyhow!("{e:?}"))
                }),
            ),
            (
                "seen_channel_send",
                ptr_t.fn_type(&[ptr_t.into(), ptr_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    let ok = backend.i8_ptr_t.const_zero();
                    backend
                        .builder
                        .build_return(Some(&ok.as_basic_value_enum()))
                        .map(|_| ())
                        .map_err(|e| anyhow!("{e:?}"))
                }),
            ),
            (
                "seen_channel_recv",
                ptr_t.fn_type(&[ptr_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    let none = backend.i8_ptr_t.const_zero();
                    backend
                        .builder
                        .build_return(Some(&none.as_basic_value_enum()))
                        .map(|_| ())
                        .map_err(|e| anyhow!("{e:?}"))
                }),
            ),
            (
                "seen_channel_select",
                ptr_t.fn_type(&[ptr_t.into(), ptr_t.into(), i64_t.into()], false),
                Box::new(|backend: &mut Self, func| {
                    let _cases = func
                        .get_nth_param(0)
                        .ok_or_else(|| anyhow!("missing cases param"))?
                        .into_pointer_value();
                    let out_ptr = func
                        .get_nth_param(1)
                        .ok_or_else(|| anyhow!("missing out param"))?
                        .into_pointer_value();
                    let _count = func
                        .get_nth_param(2)
                        .ok_or_else(|| anyhow!("missing count param"))?;

                    let result_ty = backend.ty_select_result();
                    let typed_out = backend
                        .builder
                        .build_pointer_cast(
                            out_ptr,
                            result_ty.ptr_type(inkwell::AddressSpace::from(0u16)),
                            "select_stub_out",
                        )
                        .map_err(|e| anyhow!("{e:?}"))?;
                    let payload_ptr = backend
                        .builder
                        .build_struct_gep(result_ty, typed_out, 0, "select_stub_payload")
                        .map_err(|e| anyhow!("{e:?}"))?;
                    backend
                        .builder
                        .build_store(
                            payload_ptr,
                            backend.i8_ptr_t.const_zero().as_basic_value_enum(),
                        )
                        .map_err(|e| anyhow!("{e:?}"))?;
                    let index_ptr = backend
                        .builder
                        .build_struct_gep(result_ty, typed_out, 1, "select_stub_index")
                        .map_err(|e| anyhow!("{e:?}"))?;
                    backend
                        .builder
                        .build_store(index_ptr, backend.i64_t.const_zero().as_basic_value_enum())
                        .map_err(|e| anyhow!("{e:?}"))?;
                    let status_ptr = backend
                        .builder
                        .build_struct_gep(result_ty, typed_out, 2, "select_stub_status")
                        .map_err(|e| anyhow!("{e:?}"))?;
                    backend
                        .builder
                        .build_store(status_ptr, backend.i64_t.const_zero().as_basic_value_enum())
                        .map_err(|e| anyhow!("{e:?}"))?;

                    backend
                        .builder
                        .build_return(Some(&out_ptr.as_basic_value_enum()));
                    Ok(())
                }),
            ),
            (
                "seen_scope_push",
                self.ctx.void_type().fn_type(&[i32_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    backend.builder.build_return(None);
                    Ok(())
                }),
            ),
            (
                "seen_scope_pop",
                self.ctx.void_type().fn_type(&[i32_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    backend.builder.build_return(None);
                    Ok(())
                }),
            ),
            (
                "seen_spawn",
                ptr_t.fn_type(&[ptr_t.into(), i32_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    let handle = backend.i8_ptr_t.const_zero();
                    backend
                        .builder
                        .build_return(Some(&handle.as_basic_value_enum()));
                    Ok(())
                }),
            ),
            (
                "seen_box_int",
                ptr_t.fn_type(&[i64_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    backend
                        .builder
                        .build_return(Some(&backend.i8_ptr_t.const_zero().as_basic_value_enum()));
                    Ok(())
                }),
            ),
            (
                "seen_box_bool",
                ptr_t.fn_type(&[i32_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    backend
                        .builder
                        .build_return(Some(&backend.i8_ptr_t.const_zero().as_basic_value_enum()));
                    Ok(())
                }),
            ),
            (
                "seen_box_ptr",
                ptr_t.fn_type(&[ptr_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    backend
                        .builder
                        .build_return(Some(&backend.i8_ptr_t.const_zero().as_basic_value_enum()));
                    Ok(())
                }),
            ),
        ];

        if !include_channel {
            stub_specs.retain(|(name, _, _)| {
                !matches!(
                    *name,
                    "seen_channel_new"
                        | "seen_channel_send"
                        | "seen_channel_recv"
                        | "seen_channel_select"
                )
            });
        }

        for (name, ty, body) in stub_specs {
            let is_channel = matches!(
                name,
                "seen_channel_new"
                    | "seen_channel_send"
                    | "seen_channel_recv"
                    | "seen_channel_select"
            );
            if !include_channel && is_channel {
                if self.module.get_function(name).is_none() {
                    self.module.add_function(name, ty, None);
                }
                continue;
            }
            if self.module.get_function(name).is_some() {
                continue;
            }
            let func = self.module.add_function(name, ty, None);
            let entry = self.ctx.append_basic_block(func, "entry");
            self.builder.position_at_end(entry);
            let prev_fn = self.current_fn;
            self.current_fn = Some(func);
            body(self, func)?;
            self.current_fn = prev_fn;
        }

        match saved_block {
            Some(block) => self.builder.position_at_end(block),
            None => self.builder.clear_insertion_position(),
        }
        self.current_fn = saved_fn;
        Ok(())
    }

    fn target_machine_for(options: &TargetOptions<'_>) -> Result<(TargetTriple, TargetMachine)> {
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
                LlvmOptLevel::Default,
                RelocMode::Default,
                CodeModel::Default,
            )
            .ok_or_else(|| anyhow!("Create target machine failed"))?;
        Ok((triple, machine))
    }

    fn configure_module_target(&mut self, triple: &TargetTriple, machine: &TargetMachine) {
        self.module.set_triple(triple);
        let layout = machine.get_target_data().get_data_layout();
        self.module.set_data_layout(&layout);
    }

    fn object_file_path(out_path: &Path, triple: &TargetTriple) -> PathBuf {
        let triple_str = triple.to_string();
        let ext = if triple_str.contains("windows") {
            "obj"
        } else {
            "o"
        };
        out_path.with_extension(ext)
    }

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
        let invocation = self.select_linker(triple)?;
        eprintln!(
            "LLVM backend: invoking linker {:?} (flavor {:?})",
            invocation.program, invocation.flavor
        );
        let mut cmd = std::process::Command::new(&invocation.program);
        cmd.args(&invocation.args);
        cmd.args(Self::linker_pre_args(triple, LinkOutput::Executable));
        cmd.arg(obj_path);
        cmd.arg("-o");
        cmd.arg(out_path);
        for lib in extra_libs {
            cmd.arg(lib);
        }
        cmd.args(Self::linker_post_args(triple, LinkOutput::Executable));
        Self::exec_command(cmd, "link executable")
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
        let invocation = self.select_linker(triple)?;
        eprintln!(
            "LLVM backend: invoking linker {:?} (flavor {:?})",
            invocation.program, invocation.flavor
        );
        let mut cmd = std::process::Command::new(&invocation.program);
        cmd.args(&invocation.args);
        cmd.args(Self::linker_pre_args(triple, LinkOutput::SharedLibrary));
        cmd.arg(obj_path);
        cmd.arg("-o");
        cmd.arg(out_path);
        for lib in extra_libs {
            cmd.arg(lib);
        }
        cmd.args(Self::linker_post_args(triple, LinkOutput::SharedLibrary));
        Self::exec_command(cmd, "link shared library")
    }

    fn link_static(&self, obj_path: &Path, out_path: &Path, triple: &str) -> Result<()> {
        let tool = Self::select_archiver(triple)?;
        eprintln!("LLVM backend: invoking archiver {}", tool.display());
        let mut cmd = std::process::Command::new(&tool);
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
        Self::exec_command(cmd, "archive static library")?;

        if triple.contains("apple") {
            let ranlib = Self::select_ranlib(triple)?;
            let mut cmd = std::process::Command::new(&ranlib);
            cmd.arg(out_path);
            Self::exec_command(cmd, "ranlib static library")?;
        }

        Ok(())
    }

    fn link_wasm(&self, obj_path: &Path, out_path: &Path, kind: LinkOutput) -> Result<()> {
        let program = Self::lookup_wasm_linker()?;
        eprintln!("LLVM backend: invoking wasm linker {:?}", program);
        let mut cmd = std::process::Command::new(&program);
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
        Self::exec_command(cmd, "link wasm artifact")
    }

    fn select_linker(&self, triple: &str) -> Result<LinkerInvocation> {
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
            let clang = Self::android_clang_path(triple)?;
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

    fn select_archiver(triple: &str) -> Result<PathBuf> {
        if let Ok(explicit) = env::var("SEEN_LLVM_ARCHIVER") {
            if !explicit.is_empty() {
                return Ok(PathBuf::from(explicit));
            }
        }

        if triple.contains("android") {
            return Self::android_tool_path("llvm-ar");
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

    fn select_ranlib(triple: &str) -> Result<PathBuf> {
        if let Ok(explicit) = env::var("SEEN_LLVM_RANLIB") {
            if !explicit.is_empty() {
                return Ok(PathBuf::from(explicit));
            }
        }
        if triple.contains("android") {
            return Self::android_tool_path("llvm-ranlib");
        }
        Ok(which::which("llvm-ranlib")
            .ok()
            .unwrap_or_else(|| PathBuf::from("ranlib")))
    }

    fn linker_pre_args(triple: &str, kind: LinkOutput) -> Vec<String> {
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

    fn linker_post_args(triple: &str, kind: LinkOutput) -> Vec<String> {
        match kind {
            LinkOutput::Executable => {
                if triple.contains("android") {
                    vec!["-lm".to_string()]
                } else if triple.contains("linux") {
                    vec!["-no-pie".to_string(), "-lm".to_string()]
                } else if triple.contains("apple") {
                    vec!["-lm".to_string()]
                } else {
                    vec![]
                }
            }
            LinkOutput::SharedLibrary => {
                if triple.contains("android") || triple.contains("linux") {
                    vec!["-lm".to_string()]
                } else {
                    vec![]
                }
            }
            _ => vec![],
        }
    }

    fn exec_command(mut cmd: std::process::Command, action: &str) -> Result<()> {
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

    fn lookup_wasm_linker() -> Result<PathBuf> {
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

    fn android_clang_path(triple: &str) -> Result<PathBuf> {
        let bin_dir = Self::android_bin_dir()?;
        let api = Self::android_api_level();
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

    fn android_tool_path(tool: &str) -> Result<PathBuf> {
        let bin_dir = Self::android_bin_dir()?;
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

    fn android_bin_dir() -> Result<PathBuf> {
        let ndk_home = env::var("ANDROID_NDK_HOME")
            .map_err(|_| anyhow!("ANDROID_NDK_HOME must be set to cross-compile for Android"))?;
        let host_tag = env::var("ANDROID_NDK_HOST")
            .ok()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| Self::default_ndk_host_tag());
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

    fn default_ndk_host_tag() -> String {
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

    fn android_api_level() -> String {
        env::var("ANDROID_API_LEVEL")
            .ok()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "21".to_string())
    }

    fn inject_runtime_stubs(&mut self, include_channel: bool) -> Result<()> {
        let saved_block = self.builder.get_insert_block();
        let saved_fn = self.current_fn;

        let i64_t = self.i64_t;
        let i32_t = self.ctx.i32_type();
        let ptr_t = self.i8_ptr_t;

        let mut stub_specs: Vec<(
            &str,
            inkwell::types::FunctionType<'ctx>,
            Box<dyn Fn(&mut Self, FunctionValue<'ctx>) -> Result<()> + '_>,
        )> = vec![
            (
                "seen_channel_new",
                ptr_t.fn_type(&[i64_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    let null_ptr = backend.i8_ptr_t.const_zero();
                    backend
                        .builder
                        .build_return(Some(&null_ptr.as_basic_value_enum()))
                        .map(|_| ())
                        .map_err(|e| anyhow!("{e:?}"))
                }),
            ),
            (
                "seen_channel_send",
                ptr_t.fn_type(&[ptr_t.into(), ptr_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    let ok = backend.i8_ptr_t.const_zero();
                    backend
                        .builder
                        .build_return(Some(&ok.as_basic_value_enum()))
                        .map(|_| ())
                        .map_err(|e| anyhow!("{e:?}"))
                }),
            ),
            (
                "seen_channel_recv",
                ptr_t.fn_type(&[ptr_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    let none = backend.i8_ptr_t.const_zero();
                    backend
                        .builder
                        .build_return(Some(&none.as_basic_value_enum()))
                        .map(|_| ())
                        .map_err(|e| anyhow!("{e:?}"))
                }),
            ),
            (
                "seen_channel_select",
                ptr_t.fn_type(&[ptr_t.into(), ptr_t.into(), i64_t.into()], false),
                Box::new(|backend: &mut Self, func| {
                    let _cases = func
                        .get_nth_param(0)
                        .ok_or_else(|| anyhow!("missing cases param"))?
                        .into_pointer_value();
                    let out_ptr = func
                        .get_nth_param(1)
                        .ok_or_else(|| anyhow!("missing out param"))?
                        .into_pointer_value();
                    let _count = func
                        .get_nth_param(2)
                        .ok_or_else(|| anyhow!("missing count param"))?;

                    let result_ty = backend.ty_select_result();
                    let typed_out = backend
                        .builder
                        .build_pointer_cast(
                            out_ptr,
                            result_ty.ptr_type(inkwell::AddressSpace::from(0u16)),
                            "select_stub_out",
                        )
                        .map_err(|e| anyhow!("{e:?}"))?;
                    let payload_ptr = backend
                        .builder
                        .build_struct_gep(result_ty, typed_out, 0, "select_stub_payload")
                        .map_err(|e| anyhow!("{e:?}"))?;
                    backend
                        .builder
                        .build_store(
                            payload_ptr,
                            backend.i8_ptr_t.const_zero().as_basic_value_enum(),
                        )
                        .map_err(|e| anyhow!("{e:?}"))?;
                    let index_ptr = backend
                        .builder
                        .build_struct_gep(result_ty, typed_out, 1, "select_stub_index")
                        .map_err(|e| anyhow!("{e:?}"))?;
                    backend
                        .builder
                        .build_store(index_ptr, backend.i64_t.const_zero().as_basic_value_enum())
                        .map_err(|e| anyhow!("{e:?}"))?;
                    let status_ptr = backend
                        .builder
                        .build_struct_gep(result_ty, typed_out, 2, "select_stub_status")
                        .map_err(|e| anyhow!("{e:?}"))?;
                    backend
                        .builder
                        .build_store(status_ptr, backend.i64_t.const_zero().as_basic_value_enum())
                        .map_err(|e| anyhow!("{e:?}"))?;

                    backend
                        .builder
                        .build_return(Some(&out_ptr.as_basic_value_enum()));
                    Ok(())
                }),
            ),
            (
                "seen_scope_push",
                self.ctx.void_type().fn_type(&[i32_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    backend.builder.build_return(None);
                    Ok(())
                }),
            ),
            (
                "seen_scope_pop",
                self.ctx.void_type().fn_type(&[i32_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    backend.builder.build_return(None);
                    Ok(())
                }),
            ),
            (
                "seen_spawn",
                ptr_t.fn_type(&[ptr_t.into(), i32_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    let handle = backend.i8_ptr_t.const_zero();
                    backend
                        .builder
                        .build_return(Some(&handle.as_basic_value_enum()));
                    Ok(())
                }),
            ),
            (
                "seen_box_int",
                ptr_t.fn_type(&[i64_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    backend
                        .builder
                        .build_return(Some(&backend.i8_ptr_t.const_zero().as_basic_value_enum()));
                    Ok(())
                }),
            ),
            (
                "seen_box_bool",
                ptr_t.fn_type(&[i32_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    backend
                        .builder
                        .build_return(Some(&backend.i8_ptr_t.const_zero().as_basic_value_enum()));
                    Ok(())
                }),
            ),
            (
                "seen_box_ptr",
                ptr_t.fn_type(&[ptr_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    backend
                        .builder
                        .build_return(Some(&backend.i8_ptr_t.const_zero().as_basic_value_enum()));
                    Ok(())
                }),
            ),
        ];

        if !include_channel {
            stub_specs.retain(|(name, _, _)| {
                !matches!(
                    *name,
                    "seen_channel_new"
                        | "seen_channel_send"
                        | "seen_channel_recv"
                        | "seen_channel_select"
                )
            });
        }

        for (name, ty, body) in stub_specs {
            let is_channel = matches!(
                name,
                "seen_channel_new"
                    | "seen_channel_send"
                    | "seen_channel_recv"
                    | "seen_channel_select"
            );
            if !include_channel && is_channel {
                if self.module.get_function(name).is_none() {
                    self.module.add_function(name, ty, None);
                }
                continue;
            }
            if self.module.get_function(name).is_some() {
                continue;
            }
            let func = self.module.add_function(name, ty, None);
            let entry = self.ctx.append_basic_block(func, "entry");
            self.builder.position_at_end(entry);
            let prev_fn = self.current_fn;
            self.current_fn = Some(func);
            body(self, func)?;
            self.current_fn = prev_fn;
        }

        match saved_block {
            Some(block) => self.builder.position_at_end(block),
            None => self.builder.clear_insertion_position(),
        }
        self.current_fn = saved_fn;
        Ok(())
    }

    fn target_machine_for(options: &TargetOptions<'_>) -> Result<(TargetTriple, TargetMachine)> {
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
                LlvmOptLevel::Default,
                RelocMode::Default,
                CodeModel::Default,
            )
            .ok_or_else(|| anyhow!("Create target machine failed"))?;
        Ok((triple, machine))
    }

    fn configure_module_target(&mut self, triple: &TargetTriple, machine: &TargetMachine) {
        self.module.set_triple(triple);
        let layout = machine.get_target_data().get_data_layout();
        self.module.set_data_layout(&layout);
    }

    fn object_file_path(out_path: &Path, triple: &TargetTriple) -> PathBuf {
        let triple_str = triple.to_string();
        let ext = if triple_str.contains("windows") {
            "obj"
        } else {
            "o"
        };
        out_path.with_extension(ext)
    }

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
        let invocation = self.select_linker(triple)?;
        eprintln!(
            "LLVM backend: invoking linker {:?} (flavor {:?})",
            invocation.program, invocation.flavor
        );
        let mut cmd = std::process::Command::new(&invocation.program);
        cmd.args(&invocation.args);
        cmd.args(Self::linker_pre_args(triple, LinkOutput::Executable));
        cmd.arg(obj_path);
        cmd.arg("-o");
        cmd.arg(out_path);
        for lib in extra_libs {
            cmd.arg(lib);
        }
        cmd.args(Self::linker_post_args(triple, LinkOutput::Executable));
        Self::exec_command(cmd, "link executable")
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
        let invocation = self.select_linker(triple)?;
        eprintln!(
            "LLVM backend: invoking linker {:?} (flavor {:?})",
            invocation.program, invocation.flavor
        );
        let mut cmd = std::process::Command::new(&invocation.program);
        cmd.args(&invocation.args);
        cmd.args(Self::linker_pre_args(triple, LinkOutput::SharedLibrary));
        cmd.arg(obj_path);
        cmd.arg("-o");
        cmd.arg(out_path);
        for lib in extra_libs {
            cmd.arg(lib);
        }
        cmd.args(Self::linker_post_args(triple, LinkOutput::SharedLibrary));
        Self::exec_command(cmd, "link shared library")
    }

    fn link_static(&self, obj_path: &Path, out_path: &Path, triple: &str) -> Result<()> {
        let tool = Self::select_archiver(triple)?;
        eprintln!("LLVM backend: invoking archiver {}", tool.display());
        let mut cmd = std::process::Command::new(&tool);
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
        Self::exec_command(cmd, "archive static library")?;

        if triple.contains("apple") {
            let ranlib = Self::select_ranlib(triple)?;
            let mut cmd = std::process::Command::new(&ranlib);
            cmd.arg(out_path);
            Self::exec_command(cmd, "ranlib static library")?;
        }

        Ok(())
    }

    fn link_wasm(&self, obj_path: &Path, out_path: &Path, kind: LinkOutput) -> Result<()> {
        let program = Self::lookup_wasm_linker()?;
        eprintln!("LLVM backend: invoking wasm linker {:?}", program);
        let mut cmd = std::process::Command::new(&program);
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
        Self::exec_command(cmd, "link wasm artifact")
    }

    fn select_linker(&self, triple: &str) -> Result<LinkerInvocation> {
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
            let clang = Self::android_clang_path(triple)?;
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

    fn select_archiver(triple: &str) -> Result<PathBuf> {
        if let Ok(explicit) = env::var("SEEN_LLVM_ARCHIVER") {
            if !explicit.is_empty() {
                return Ok(PathBuf::from(explicit));
            }
        }

        if triple.contains("android") {
            return Self::android_tool_path("llvm-ar");
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

    fn select_ranlib(triple: &str) -> Result<PathBuf> {
        if let Ok(explicit) = env::var("SEEN_LLVM_RANLIB") {
            if !explicit.is_empty() {
                return Ok(PathBuf::from(explicit));
            }
        }
        if triple.contains("android") {
            return Self::android_tool_path("llvm-ranlib");
        }
        Ok(which::which("llvm-ranlib")
            .ok()
            .unwrap_or_else(|| PathBuf::from("ranlib")))
    }

    fn linker_pre_args(triple: &str, kind: LinkOutput) -> Vec<String> {
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

    fn linker_post_args(triple: &str, kind: LinkOutput) -> Vec<String> {
        match kind {
            LinkOutput::Executable => {
                if triple.contains("android") {
                    vec!["-lm".to_string()]
                } else if triple.contains("linux") {
                    vec!["-no-pie".to_string(), "-lm".to_string()]
                } else if triple.contains("apple") {
                    vec!["-lm".to_string()]
                } else {
                    vec![]
                }
            }
            LinkOutput::SharedLibrary => {
                if triple.contains("android") || triple.contains("linux") {
                    vec!["-lm".to_string()]
                } else {
                    vec![]
                }
            }
            _ => vec![],
        }
    }

    fn exec_command(mut cmd: std::process::Command, action: &str) -> Result<()> {
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

    fn lookup_wasm_linker() -> Result<PathBuf> {
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

    fn android_clang_path(triple: &str) -> Result<PathBuf> {
        let bin_dir = Self::android_bin_dir()?;
        let api = Self::android_api_level();
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

    fn android_tool_path(tool: &str) -> Result<PathBuf> {
        let bin_dir = Self::android_bin_dir()?;
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

    fn android_bin_dir() -> Result<PathBuf> {
        let ndk_home = env::var("ANDROID_NDK_HOME")
            .map_err(|_| anyhow!("ANDROID_NDK_HOME must be set to cross-compile for Android"))?;
        let host_tag = env::var("ANDROID_NDK_HOST")
            .ok()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| Self::default_ndk_host_tag());
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

    fn default_ndk_host_tag() -> String {
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

    fn android_api_level() -> String {
        env::var("ANDROID_API_LEVEL")
            .ok()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "21".to_string())
    }

    fn inject_runtime_stubs(&mut self, include_channel: bool) -> Result<()> {
        let saved_block = self.builder.get_insert_block();
        let saved_fn = self.current_fn;

        let i64_t = self.i64_t;
        let i32_t = self.ctx.i32_type();
        let ptr_t = self.i8_ptr_t;

        let mut stub_specs: Vec<(
            &str,
            inkwell::types::FunctionType<'ctx>,
            Box<dyn Fn(&mut Self, FunctionValue<'ctx>) -> Result<()> + '_>,
        )> = vec![
            (
                "seen_channel_new",
                ptr_t.fn_type(&[i64_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    let null_ptr = backend.i8_ptr_t.const_zero();
                    backend
                        .builder
                        .build_return(Some(&null_ptr.as_basic_value_enum()))
                        .map(|_| ())
                        .map_err(|e| anyhow!("{e:?}"))
                }),
            ),
            (
                "seen_channel_send",
                ptr_t.fn_type(&[ptr_t.into(), ptr_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    let ok = backend.i8_ptr_t.const_zero();
                    backend
                        .builder
                        .build_return(Some(&ok.as_basic_value_enum()))
                        .map(|_| ())
                        .map_err(|e| anyhow!("{e:?}"))
                }),
            ),
            (
                "seen_channel_recv",
                ptr_t.fn_type(&[ptr_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    let none = backend.i8_ptr_t.const_zero();
                    backend
                        .builder
                        .build_return(Some(&none.as_basic_value_enum()))
                        .map(|_| ())
                        .map_err(|e| anyhow!("{e:?}"))
                }),
            ),
            (
                "seen_channel_select",
                ptr_t.fn_type(&[ptr_t.into(), ptr_t.into(), i64_t.into()], false),
                Box::new(|backend: &mut Self, func| {
                    let _cases = func
                        .get_nth_param(0)
                        .ok_or_else(|| anyhow!("missing cases param"))?
                        .into_pointer_value();
                    let out_ptr = func
                        .get_nth_param(1)
                        .ok_or_else(|| anyhow!("missing out param"))?
                        .into_pointer_value();
                    let _count = func
                        .get_nth_param(2)
                        .ok_or_else(|| anyhow!("missing count param"))?;

                    let result_ty = backend.ty_select_result();
                    let typed_out = backend
                        .builder
                        .build_pointer_cast(
                            out_ptr,
                            result_ty.ptr_type(inkwell::AddressSpace::from(0u16)),
                            "select_stub_out",
                        )
                        .map_err(|e| anyhow!("{e:?}"))?;
                    let payload_ptr = backend
                        .builder
                        .build_struct_gep(result_ty, typed_out, 0, "select_stub_payload")
                        .map_err(|e| anyhow!("{e:?}"))?;
                    backend
                        .builder
                        .build_store(
                            payload_ptr,
                            backend.i8_ptr_t.const_zero().as_basic_value_enum(),
                        )
                        .map_err(|e| anyhow!("{e:?}"))?;
                    let index_ptr = backend
                        .builder
                        .build_struct_gep(result_ty, typed_out, 1, "select_stub_index")
                        .map_err(|e| anyhow!("{e:?}"))?;
                    backend
                        .builder
                        .build_store(index_ptr, backend.i64_t.const_zero().as_basic_value_enum())
                        .map_err(|e| anyhow!("{e:?}"))?;
                    let status_ptr = backend
                        .builder
                        .build_struct_gep(result_ty, typed_out, 2, "select_stub_status")
                        .map_err(|e| anyhow!("{e:?}"))?;
                    backend
                        .builder
                        .build_store(status_ptr, backend.i64_t.const_zero().as_basic_value_enum())
                        .map_err(|e| anyhow!("{e:?}"))?;

                    backend
                        .builder
                        .build_return(Some(&out_ptr.as_basic_value_enum()));
                    Ok(())
                }),
            ),
            (
                "seen_scope_push",
                self.ctx.void_type().fn_type(&[i32_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    backend.builder.build_return(None);
                    Ok(())
                }),
            ),
            (
                "seen_scope_pop",
                self.ctx.void_type().fn_type(&[i32_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    backend.builder.build_return(None);
                    Ok(())
                }),
            ),
            (
                "seen_spawn",
                ptr_t.fn_type(&[ptr_t.into(), i32_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    let handle = backend.i8_ptr_t.const_zero();
                    backend
                        .builder
                        .build_return(Some(&handle.as_basic_value_enum()));
                    Ok(())
                }),
            ),
            (
                "seen_box_int",
                ptr_t.fn_type(&[i64_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    backend
                        .builder
                        .build_return(Some(&backend.i8_ptr_t.const_zero().as_basic_value_enum()));
                    Ok(())
                }),
            ),
            (
                "seen_box_bool",
                ptr_t.fn_type(&[i32_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    backend
                        .builder
                        .build_return(Some(&backend.i8_ptr_t.const_zero().as_basic_value_enum()));
                    Ok(())
                }),
            ),
            (
                "seen_box_ptr",
                ptr_t.fn_type(&[ptr_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    backend
                        .builder
                        .build_return(Some(&backend.i8_ptr_t.const_zero().as_basic_value_enum()));
                    Ok(())
                }),
            ),
        ];

        if !include_channel {
            stub_specs.retain(|(name, _, _)| {
                !matches!(
                    *name,
                    "seen_channel_new"
                        | "seen_channel_send"
                        | "seen_channel_recv"
                        | "seen_channel_select"
                )
            });
        }

        for (name, ty, body) in stub_specs {
            let is_channel = matches!(
                name,
                "seen_channel_new"
                    | "seen_channel_send"
                    | "seen_channel_recv"
                    | "seen_channel_select"
            );
            if !include_channel && is_channel {
                if self.module.get_function(name).is_none() {
                    self.module.add_function(name, ty, None);
                }
                continue;
            }
            if self.module.get_function(name).is_some() {
                continue;
            }
            let func = self.module.add_function(name, ty, None);
            let entry = self.ctx.append_basic_block(func, "entry");
            self.builder.position_at_end(entry);
            let prev_fn = self.current_fn;
            self.current_fn = Some(func);
            body(self, func)?;
            self.current_fn = prev_fn;
        }

        match saved_block {
            Some(block) => self.builder.position_at_end(block),
            None => self.builder.clear_insertion_position(),
        }
        self.current_fn = saved_fn;
        Ok(())
    }

    fn target_machine_for(options: &TargetOptions<'_>) -> Result<(TargetTriple, TargetMachine)> {
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
                LlvmOptLevel::Default,
                RelocMode::Default,
                CodeModel::Default,
            )
            .ok_or_else(|| anyhow!("Create target machine failed"))?;
        Ok((triple, machine))
    }

    fn configure_module_target(&mut self, triple: &TargetTriple, machine: &TargetMachine) {
        self.module.set_triple(triple);
        let layout = machine.get_target_data().get_data_layout();
        self.module.set_data_layout(&layout);
    }

    fn object_file_path(out_path: &Path, triple: &TargetTriple) -> PathBuf {
        let triple_str = triple.to_string();
        let ext = if triple_str.contains("windows") {
            "obj"
        } else {
            "o"
        };
        out_path.with_extension(ext)
    }

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
        let invocation = self.select_linker(triple)?;
        eprintln!(
            "LLVM backend: invoking linker {:?} (flavor {:?})",
            invocation.program, invocation.flavor
        );
        let mut cmd = std::process::Command::new(&invocation.program);
        cmd.args(&invocation.args);
        cmd.args(Self::linker_pre_args(triple, LinkOutput::Executable));
        cmd.arg(obj_path);
        cmd.arg("-o");
        cmd.arg(out_path);
        for lib in extra_libs {
            cmd.arg(lib);
        }
        cmd.args(Self::linker_post_args(triple, LinkOutput::Executable));
        Self::exec_command(cmd, "link executable")
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
        let invocation = self.select_linker(triple)?;
        eprintln!(
            "LLVM backend: invoking linker {:?} (flavor {:?})",
            invocation.program, invocation.flavor
        );
        let mut cmd = std::process::Command::new(&invocation.program);
        cmd.args(&invocation.args);
        cmd.args(Self::linker_pre_args(triple, LinkOutput::SharedLibrary));
        cmd.arg(obj_path);
        cmd.arg("-o");
        cmd.arg(out_path);
        for lib in extra_libs {
            cmd.arg(lib);
        }
        cmd.args(Self::linker_post_args(triple, LinkOutput::SharedLibrary));
        Self::exec_command(cmd, "link shared library")
    }

    fn link_static(&self, obj_path: &Path, out_path: &Path, triple: &str) -> Result<()> {
        let tool = Self::select_archiver(triple)?;
        eprintln!("LLVM backend: invoking archiver {}", tool.display());
        let mut cmd = std::process::Command::new(&tool);
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
        Self::exec_command(cmd, "archive static library")?;

        if triple.contains("apple") {
            let ranlib = Self::select_ranlib(triple)?;
            let mut cmd = std::process::Command::new(&ranlib);
            cmd.arg(out_path);
            Self::exec_command(cmd, "ranlib static library")?;
        }

        Ok(())
    }

    fn link_wasm(&self, obj_path: &Path, out_path: &Path, kind: LinkOutput) -> Result<()> {
        let program = Self::lookup_wasm_linker()?;
        eprintln!("LLVM backend: invoking wasm linker {:?}", program);
        let mut cmd = std::process::Command::new(&program);
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
        Self::exec_command(cmd, "link wasm artifact")
    }

    fn select_linker(&self, triple: &str) -> Result<LinkerInvocation> {
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
            let clang = Self::android_clang_path(triple)?;
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

    fn select_archiver(triple: &str) -> Result<PathBuf> {
        if let Ok(explicit) = env::var("SEEN_LLVM_ARCHIVER") {
            if !explicit.is_empty() {
                return Ok(PathBuf::from(explicit));
            }
        }

        if triple.contains("android") {
            return Self::android_tool_path("llvm-ar");
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

    fn select_ranlib(triple: &str) -> Result<PathBuf> {
        if let Ok(explicit) = env::var("SEEN_LLVM_RANLIB") {
            if !explicit.is_empty() {
                return Ok(PathBuf::from(explicit));
            }
        }
        if triple.contains("android") {
            return Self::android_tool_path("llvm-ranlib");
        }
        Ok(which::which("llvm-ranlib")
            .ok()
            .unwrap_or_else(|| PathBuf::from("ranlib")))
    }

    fn linker_pre_args(triple: &str, kind: LinkOutput) -> Vec<String> {
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

    fn linker_post_args(triple: &str, kind: LinkOutput) -> Vec<String> {
        match kind {
            LinkOutput::Executable => {
                if triple.contains("android") {
                    vec!["-lm".to_string()]
                } else if triple.contains("linux") {
                    vec!["-no-pie".to_string(), "-lm".to_string()]
                } else if triple.contains("apple") {
                    vec!["-lm".to_string()]
                } else {
                    vec![]
                }
            }
            LinkOutput::SharedLibrary => {
                if triple.contains("android") || triple.contains("linux") {
                    vec!["-lm".to_string()]
                } else {
                    vec![]
                }
            }
            _ => vec![],
        }
    }

    fn exec_command(mut cmd: std::process::Command, action: &str) -> Result<()> {
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

    fn lookup_wasm_linker() -> Result<PathBuf> {
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

    fn android_clang_path(triple: &str) -> Result<PathBuf> {
        let bin_dir = Self::android_bin_dir()?;
        let api = Self::android_api_level();
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

    fn android_tool_path(tool: &str) -> Result<PathBuf> {
        let bin_dir = Self::android_bin_dir()?;
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

    fn android_bin_dir() -> Result<PathBuf> {
        let ndk_home = env::var("ANDROID_NDK_HOME")
            .map_err(|_| anyhow!("ANDROID_NDK_HOME must be set to cross-compile for Android"))?;
        let host_tag = env::var("ANDROID_NDK_HOST")
            .ok()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| Self::default_ndk_host_tag());
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

    fn default_ndk_host_tag() -> String {
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

    fn android_api_level() -> String {
        env::var("ANDROID_API_LEVEL")
            .ok()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "21".to_string())
    }

    fn inject_runtime_stubs(&mut self, include_channel: bool) -> Result<()> {
        let saved_block = self.builder.get_insert_block();
        let saved_fn = self.current_fn;

        let i64_t = self.i64_t;
        let i32_t = self.ctx.i32_type();
        let ptr_t = self.i8_ptr_t;

        let mut stub_specs: Vec<(
            &str,
            inkwell::types::FunctionType<'ctx>,
            Box<dyn Fn(&mut Self, FunctionValue<'ctx>) -> Result<()> + '_>,
        )> = vec![
            (
                "seen_channel_new",
                ptr_t.fn_type(&[i64_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    let null_ptr = backend.i8_ptr_t.const_zero();
                    backend
                        .builder
                        .build_return(Some(&null_ptr.as_basic_value_enum()))
                        .map(|_| ())
                        .map_err(|e| anyhow!("{e:?}"))
                }),
            ),
            (
                "seen_channel_send",
                ptr_t.fn_type(&[ptr_t.into(), ptr_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    let ok = backend.i8_ptr_t.const_zero();
                    backend
                        .builder
                        .build_return(Some(&ok.as_basic_value_enum()))
                        .map(|_| ())
                        .map_err(|e| anyhow!("{e:?}"))
                }),
            ),
            (
                "seen_channel_recv",
                ptr_t.fn_type(&[ptr_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    let none = backend.i8_ptr_t.const_zero();
                    backend
                        .builder
                        .build_return(Some(&none.as_basic_value_enum()))
                        .map(|_| ())
                        .map_err(|e| anyhow!("{e:?}"))
                }),
            ),
            (
                "seen_channel_select",
                ptr_t.fn_type(&[ptr_t.into(), ptr_t.into(), i64_t.into()], false),
                Box::new(|backend: &mut Self, func| {
                    let _cases = func
                        .get_nth_param(0)
                        .ok_or_else(|| anyhow!("missing cases param"))?
                        .into_pointer_value();
                    let out_ptr = func
                        .get_nth_param(1)
                        .ok_or_else(|| anyhow!("missing out param"))?
                        .into_pointer_value();
                    let _count = func
                        .get_nth_param(2)
                        .ok_or_else(|| anyhow!("missing count param"))?;

                    let result_ty = backend.ty_select_result();
                    let typed_out = backend
                        .builder
                        .build_pointer_cast(
                            out_ptr,
                            result_ty.ptr_type(inkwell::AddressSpace::from(0u16)),
                            "select_stub_out",
                        )
                        .map_err(|e| anyhow!("{e:?}"))?;
                    let payload_ptr = backend
                        .builder
                        .build_struct_gep(result_ty, typed_out, 0, "select_stub_payload")
                        .map_err(|e| anyhow!("{e:?}"))?;
                    backend
                        .builder
                        .build_store(
                            payload_ptr,
                            backend.i8_ptr_t.const_zero().as_basic_value_enum(),
                        )
                        .map_err(|e| anyhow!("{e:?}"))?;
                    let index_ptr = backend
                        .builder
                        .build_struct_gep(result_ty, typed_out, 1, "select_stub_index")
                        .map_err(|e| anyhow!("{e:?}"))?;
                    backend
                        .builder
                        .build_store(index_ptr, backend.i64_t.const_zero().as_basic_value_enum())
                        .map_err(|e| anyhow!("{e:?}"))?;
                    let status_ptr = backend
                        .builder
                        .build_struct_gep(result_ty, typed_out, 2, "select_stub_status")
                        .map_err(|e| anyhow!("{e:?}"))?;
                    backend
                        .builder
                        .build_store(status_ptr, backend.i64_t.const_zero().as_basic_value_enum())
                        .map_err(|e| anyhow!("{e:?}"))?;

                    backend
                        .builder
                        .build_return(Some(&out_ptr.as_basic_value_enum()));
                    Ok(())
                }),
            ),
            (
                "seen_scope_push",
                self.ctx.void_type().fn_type(&[i32_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    backend.builder.build_return(None);
                    Ok(())
                }),
            ),
            (
                "seen_scope_pop",
                self.ctx.void_type().fn_type(&[i32_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    backend.builder.build_return(None);
                    Ok(())
                }),
            ),
            (
                "seen_spawn",
                ptr_t.fn_type(&[ptr_t.into(), i32_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    let handle = backend.i8_ptr_t.const_zero();
                    backend
                        .builder
                        .build_return(Some(&handle.as_basic_value_enum()));
                    Ok(())
                }),
            ),
            (
                "seen_box_int",
                ptr_t.fn_type(&[i64_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    backend
                        .builder
                        .build_return(Some(&backend.i8_ptr_t.const_zero().as_basic_value_enum()));
                    Ok(())
                }),
            ),
            (
                "seen_box_bool",
                ptr_t.fn_type(&[i32_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    backend
                        .builder
                        .build_return(Some(&backend.i8_ptr_t.const_zero().as_basic_value_enum()));
                    Ok(())
                }),
            ),
            (
                "seen_box_ptr",
                ptr_t.fn_type(&[ptr_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    backend
                        .builder
                        .build_return(Some(&backend.i8_ptr_t.const_zero().as_basic_value_enum()));
                    Ok(())
                }),
            ),
        ];

        if !include_channel {
            stub_specs.retain(|(name, _, _)| {
                !matches!(
                    *name,
                    "seen_channel_new"
                        | "seen_channel_send"
                        | "seen_channel_recv"
                        | "seen_channel_select"
                )
            });
        }

        for (name, ty, body) in stub_specs {
            let is_channel = matches!(
                name,
                "seen_channel_new"
                    | "seen_channel_send"
                    | "seen_channel_recv"
                    | "seen_channel_select"
            );
            if !include_channel && is_channel {
                if self.module.get_function(name).is_none() {
                    self.module.add_function(name, ty, None);
                }
                continue;
            }
            if self.module.get_function(name).is_some() {
                continue;
            }
            let func = self.module.add_function(name, ty, None);
            let entry = self.ctx.append_basic_block(func, "entry");
            self.builder.position_at_end(entry);
            let prev_fn = self.current_fn;
            self.current_fn = Some(func);
            body(self, func)?;
            self.current_fn = prev_fn;
        }

        match saved_block {
            Some(block) => self.builder.position_at_end(block),
            None => self.builder.clear_insertion_position(),
        }
        self.current_fn = saved_fn;
        Ok(())
    }

    fn target_machine_for(options: &TargetOptions<'_>) -> Result<(TargetTriple, TargetMachine)> {
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
                LlvmOptLevel::Default,
                RelocMode::Default,
                CodeModel::Default,
            )
            .ok_or_else(|| anyhow!("Create target machine failed"))?;
        Ok((triple, machine))
    }

    fn configure_module_target(&mut self, triple: &TargetTriple, machine: &TargetMachine) {
        self.module.set_triple(triple);
        let layout = machine.get_target_data().get_data_layout();
        self.module.set_data_layout(&layout);
    }

    fn object_file_path(out_path: &Path, triple: &TargetTriple) -> PathBuf {
        let triple_str = triple.to_string();
        let ext = if triple_str.contains("windows") {
            "obj"
        } else {
            "o"
        };
        out_path.with_extension(ext)
    }

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
        let invocation = self.select_linker(triple)?;
        eprintln!(
            "LLVM backend: invoking linker {:?} (flavor {:?})",
            invocation.program, invocation.flavor
        );
        let mut cmd = std::process::Command::new(&invocation.program);
        cmd.args(&invocation.args);
        cmd.args(Self::linker_pre_args(triple, LinkOutput::Executable));
        cmd.arg(obj_path);
        cmd.arg("-o");
        cmd.arg(out_path);
        for lib in extra_libs {
            cmd.arg(lib);
        }
        cmd.args(Self::linker_post_args(triple, LinkOutput::Executable));
        Self::exec_command(cmd, "link executable")
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
        let invocation = self.select_linker(triple)?;
        eprintln!(
            "LLVM backend: invoking linker {:?} (flavor {:?})",
            invocation.program, invocation.flavor
        );
        let mut cmd = std::process::Command::new(&invocation.program);
        cmd.args(&invocation.args);
        cmd.args(Self::linker_pre_args(triple, LinkOutput::SharedLibrary));
        cmd.arg(obj_path);
        cmd.arg("-o");
        cmd.arg(out_path);
        for lib in extra_libs {
            cmd.arg(lib);
        }
        cmd.args(Self::linker_post_args(triple, LinkOutput::SharedLibrary));
        Self::exec_command(cmd, "link shared library")
    }

    fn link_static(&self, obj_path: &Path, out_path: &Path, triple: &str) -> Result<()> {
        let tool = Self::select_archiver(triple)?;
        eprintln!("LLVM backend: invoking archiver {}", tool.display());
        let mut cmd = std::process::Command::new(&tool);
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
        Self::exec_command(cmd, "archive static library")?;

        if triple.contains("apple") {
            let ranlib = Self::select_ranlib(triple)?;
            let mut cmd = std::process::Command::new(&ranlib);
            cmd.arg(out_path);
            Self::exec_command(cmd, "ranlib static library")?;
        }

        Ok(())
    }

    fn link_wasm(&self, obj_path: &Path, out_path: &Path, kind: LinkOutput) -> Result<()> {
        let program = Self::lookup_wasm_linker()?;
        eprintln!("LLVM backend: invoking wasm linker {:?}", program);
        let mut cmd = std::process::Command::new(&program);
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
        Self::exec_command(cmd, "link wasm artifact")
    }

    fn select_linker(&self, triple: &str) -> Result<LinkerInvocation> {
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
            let clang = Self::android_clang_path(triple)?;
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

    fn select_archiver(triple: &str) -> Result<PathBuf> {
        if let Ok(explicit) = env::var("SEEN_LLVM_ARCHIVER") {
            if !explicit.is_empty() {
                return Ok(PathBuf::from(explicit));
            }
        }

        if triple.contains("android") {
            return Self::android_tool_path("llvm-ar");
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

    fn select_ranlib(triple: &str) -> Result<PathBuf> {
        if let Ok(explicit) = env::var("SEEN_LLVM_RANLIB") {
            if !explicit.is_empty() {
                return Ok(PathBuf::from(explicit));
            }
        }
        if triple.contains("android") {
            return Self::android_tool_path("llvm-ranlib");
        }
        Ok(which::which("llvm-ranlib")
            .ok()
            .unwrap_or_else(|| PathBuf::from("ranlib")))
    }

    fn linker_pre_args(triple: &str, kind: LinkOutput) -> Vec<String> {
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

    fn linker_post_args(triple: &str, kind: LinkOutput) -> Vec<String> {
        match kind {
            LinkOutput::Executable => {
                if triple.contains("android") {
                    vec!["-lm".to_string()]
                } else if triple.contains("linux") {
                    vec!["-no-pie".to_string(), "-lm".to_string()]
                } else if triple.contains("apple") {
                    vec!["-lm".to_string()]
                } else {
                    vec![]
                }
            }
            LinkOutput::SharedLibrary => {
                if triple.contains("android") || triple.contains("linux") {
                    vec!["-lm".to_string()]
                } else {
                    vec![]
                }
            }
            _ => vec![],
        }
    }

    fn exec_command(mut cmd: std::process::Command, action: &str) -> Result<()> {
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

    fn lookup_wasm_linker() -> Result<PathBuf> {
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

    fn android_clang_path(triple: &str) -> Result<PathBuf> {
        let bin_dir = Self::android_bin_dir()?;
        let api = Self::android_api_level();
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

    fn android_tool_path(tool: &str) -> Result<PathBuf> {
        let bin_dir = Self::android_bin_dir()?;
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

    fn android_bin_dir() -> Result<PathBuf> {
        let ndk_home = env::var("ANDROID_NDK_HOME")
            .map_err(|_| anyhow!("ANDROID_NDK_HOME must be set to cross-compile for Android"))?;
        let host_tag = env::var("ANDROID_NDK_HOST")
            .ok()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| Self::default_ndk_host_tag());
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

    fn default_ndk_host_tag() -> String {
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

    fn android_api_level() -> String {
        env::var("ANDROID_API_LEVEL")
            .ok()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "21".to_string())
    }

    fn inject_runtime_stubs(&mut self, include_channel: bool) -> Result<()> {
        let saved_block = self.builder.get_insert_block();
        let saved_fn = self.current_fn;

        let i64_t = self.i64_t;
        let i32_t = self.ctx.i32_type();
        let ptr_t = self.i8_ptr_t;

        let mut stub_specs: Vec<(
            &str,
            inkwell::types::FunctionType<'ctx>,
            Box<dyn Fn(&mut Self, FunctionValue<'ctx>) -> Result<()> + '_>,
        )> = vec![
            (
                "seen_channel_new",
                ptr_t.fn_type(&[i64_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    let null_ptr = backend.i8_ptr_t.const_zero();
                    backend
                        .builder
                        .build_return(Some(&null_ptr.as_basic_value_enum()))
                        .map(|_| ())
                        .map_err(|e| anyhow!("{e:?}"))
                }),
            ),
            (
                "seen_channel_send",
                ptr_t.fn_type(&[ptr_t.into(), ptr_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    let ok = backend.i8_ptr_t.const_zero();
                    backend
                        .builder
                        .build_return(Some(&ok.as_basic_value_enum()))
                        .map(|_| ())
                        .map_err(|e| anyhow!("{e:?}"))
                }),
            ),
            (
                "seen_channel_recv",
                ptr_t.fn_type(&[ptr_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    let none = backend.i8_ptr_t.const_zero();
                    backend
                        .builder
                        .build_return(Some(&none.as_basic_value_enum()))
                        .map(|_| ())
                        .map_err(|e| anyhow!("{e:?}"))
                }),
            ),
            (
                "seen_channel_select",
                ptr_t.fn_type(&[ptr_t.into(), ptr_t.into(), i64_t.into()], false),
                Box::new(|backend: &mut Self, func| {
                    let _cases = func
                        .get_nth_param(0)
                        .ok_or_else(|| anyhow!("missing cases param"))?
                        .into_pointer_value();
                    let out_ptr = func
                        .get_nth_param(1)
                        .ok_or_else(|| anyhow!("missing out param"))?
                        .into_pointer_value();
                    let _count = func
                        .get_nth_param(2)
                        .ok_or_else(|| anyhow!("missing count param"))?;

                    let result_ty = backend.ty_select_result();
                    let typed_out = backend
                        .builder
                        .build_pointer_cast(
                            out_ptr,
                            result_ty.ptr_type(inkwell::AddressSpace::from(0u16)),
                            "select_stub_out",
                        )
                        .map_err(|e| anyhow!("{e:?}"))?;
                    let payload_ptr = backend
                        .builder
                        .build_struct_gep(result_ty, typed_out, 0, "select_stub_payload")
                        .map_err(|e| anyhow!("{e:?}"))?;
                    backend
                        .builder
                        .build_store(
                            payload_ptr,
                            backend.i8_ptr_t.const_zero().as_basic_value_enum(),
                        )
                        .map_err(|e| anyhow!("{e:?}"))?;
                    let index_ptr = backend
                        .builder
                        .build_struct_gep(result_ty, typed_out, 1, "select_stub_index")
                        .map_err(|e| anyhow!("{e:?}"))?;
                    backend
                        .builder
                        .build_store(index_ptr, backend.i64_t.const_zero().as_basic_value_enum())
                        .map_err(|e| anyhow!("{e:?}"))?;
                    let status_ptr = backend
                        .builder
                        .build_struct_gep(result_ty, typed_out, 2, "select_stub_status")
                        .map_err(|e| anyhow!("{e:?}"))?;
                    backend
                        .builder
                        .build_store(status_ptr, backend.i64_t.const_zero().as_basic_value_enum())
                        .map_err(|e| anyhow!("{e:?}"))?;

                    backend
                        .builder
                        .build_return(Some(&out_ptr.as_basic_value_enum()));
                    Ok(())
                }),
            ),
            (
                "seen_scope_push",
                self.ctx.void_type().fn_type(&[i32_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    backend.builder.build_return(None);
                    Ok(())
                }),
            ),
            (
                "seen_scope_pop",
                self.ctx.void_type().fn_type(&[i32_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    backend.builder.build_return(None);
                    Ok(())
                }),
            ),
            (
                "seen_spawn",
                ptr_t.fn_type(&[ptr_t.into(), i32_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    let handle = backend.i8_ptr_t.const_zero();
                    backend
                        .builder
                        .build_return(Some(&handle.as_basic_value_enum()));
                    Ok(())
                }),
            ),
            (
                "seen_box_int",
                ptr_t.fn_type(&[i64_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    backend
                        .builder
                        .build_return(Some(&backend.i8_ptr_t.const_zero().as_basic_value_enum()));
                    Ok(())
                }),
            ),
            (
                "seen_box_bool",
                ptr_t.fn_type(&[i32_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    backend
                        .builder
                        .build_return(Some(&backend.i8_ptr_t.const_zero().as_basic_value_enum()));
                    Ok(())
                }),
            ),
            (
                "seen_box_ptr",
                ptr_t.fn_type(&[ptr_t.into()], false),
                Box::new(|backend: &mut Self, _func| {
                    backend
                        .builder
                        .build_return(Some(&backend.i8_ptr_t.const_zero().as_basic_value_enum()));
                    Ok(())
                }),
            ),
        ];

        if !include_channel {
            stub_specs.retain(|(name, _, _)| {
                !matches!(
                    *name,
                    "seen_channel_new"
                        | "seen_channel_send"
                        | "seen_channel_recv"
                        | "seen_channel_select"
                )
            });
        }

        for (name, ty, body) in stub_specs {
            let is_channel = matches!(
                name,
                "seen_channel_new"
                    | "seen_channel_send"
                    | "seen_channel_recv"
                    | "seen_channel_select"
            );
            if !include_channel && is_channel {
                if self.module.get_function(name).is_none() {
                    self.module.add_function(name, ty, None);
                }
                continue;
            }
            if self.module.get_function(name).is_some() {
                continue;
            }
            let func = self.module.add_function(name, ty, None);
            let entry = self.ctx.append_basic_block(func, "entry");
            self.builder.position_at_end(entry);
            let prev_fn = self.current_fn;
            self.current_fn = Some(func);
            body(self, func)?;
            self.current_fn = prev_fn;
        }

        match saved_block {
            Some(block) => self.builder.position_at_end(block),
            None => self.builder.clear_insertion_position(),
        }
        self.current_fn = saved_fn;
        Ok(())
    }

    fn target_machine_for(options: &TargetOptions<'_>) -> Result<(TargetTriple, TargetMachine)> {
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
                LlvmOptLevel::Default,
                RelocMode::Default,
                CodeModel::Default,
            )
            .ok_or_else(|| anyhow!("Create target machine failed"))?;
        Ok((triple, machine))
    }

    fn configure_module_target(&mut self, triple: &TargetTriple, machine: &TargetMachine) {
        self.module.set_triple(triple);
        let layout = machine.get_target_data().get_data_layout();
        self.module.set_data_layout(&layout);
    }

    fn object_file_path(out_path: &Path, triple: &TargetTriple) -> PathBuf {
        let triple_str = triple.to_string();
        let ext = if triple_str.contains("windows") {
            "obj"
        } else {
            "o"
        };
        out_path.with_extension(ext)
    }

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
        let invocation = self.select_linker(triple)?;
        eprintln!(
            "LLVM backend: invoking linker {:?} (flavor {:?})",
            invocation.program, invocation.flavor
        );
        let mut cmd = std::process::Command::new(&invocation.program);
        cmd.args(&invocation.args);
        cmd.args(Self::linker_pre_args(triple, LinkOutput::Executable));
        cmd.arg(obj_path);
        cmd.arg("-o");
        cmd.arg(out_path);
        for lib in extra_libs {
            cmd.arg(lib);
        }
        cmd.args(Self::linker_post_args(triple, LinkOutput::Executable));
        Self::exec_command(cmd, "link executable")
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
        let invocation = self.select_linker(triple)?;
        eprintln!(
            "LLVM backend: invoking linker {:?} (flavor {:?})",
            invocation.program, invocation.flavor
        );
        let mut cmd = std::process::Command::new(&invocation.program);
        cmd.args(&invocation.args);
        cmd.args(Self::linker_pre_args(triple, LinkOutput::SharedLibrary));
        cmd.arg(obj_path);
        cmd.arg("-o");
        cmd.arg(out_path);
        for lib in extra_libs {
            cmd.arg(lib);
        }
        cmd.args(Self::linker_post_args(triple, LinkOutput::SharedLibrary));
        Self::exec_command(cmd, "link shared library")
    }

    fn link_static(&self, obj_path: &Path, out_path: &Path, triple: &str) -> Result<()> {
        let tool = Self::select_archiver(triple)?;
        eprintln!("LLVM