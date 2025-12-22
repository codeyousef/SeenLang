#![cfg(feature = "llvm")]
//! LLVM backend that lowers Seen IR (IRProgram) to native code via inkwell.
//!
//! Scope: Implements a minimal but solid subset required to compile the
//! self‑hosting entry (`compiler_seen/src/main.seen`) and similar programs.

use std::collections::{HashMap, HashSet};
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
    realloc: Option<FunctionValue<'ctx>>,
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
    // Struct type registry: type_name -> (LLVM struct type, field names in order)
    struct_types: HashMap<String, (StructType<'ctx>, Vec<String>)>,
    // Variable name -> struct type name (for field access lookup)
    var_struct_types: HashMap<String, String>,
    // Register id -> struct type name (for field access on expression results)
    reg_struct_types: HashMap<u32, String>,
    // Function name -> return struct type name (for call result tagging)
    fn_return_struct_types: HashMap<String, String>,
    // Variable name -> array element struct type name (for array access -> field access patterns)
    var_array_element_struct: HashMap<String, String>,
    // Variable name -> true if it's a string (for string indexing)
    var_is_string: HashSet<String>,
    // Variable name -> true if it's an integer array (for array indexing)
    var_is_int_array: HashSet<String>,
    // Struct definitions (Seen types): type_name -> fields
    struct_definitions: HashMap<String, Vec<(String, IRType)>>,
    // Register id -> array element struct type name
    reg_array_element_struct: HashMap<u32, String>,
    // Register id -> true if it's an integer array (for array indexing)
    reg_is_int_array: HashSet<u32>,
    // Variable name -> true if it's a Vec that stores floats
    var_is_float_vec: HashSet<String>,
    // Register id -> true if it holds a float value (for proper storage)
    reg_is_float: HashSet<u32>,
    // Variable name -> true if it holds a float value (stored as i64 bits)
    var_is_float: HashSet<String>,
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
            realloc: None,
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
            struct_types: HashMap::new(),
            var_struct_types: HashMap::new(),
            reg_struct_types: HashMap::new(),
            fn_return_struct_types: HashMap::new(),
            var_array_element_struct: HashMap::new(),
            var_is_string: HashSet::new(),
            var_is_int_array: HashSet::new(),
            struct_definitions: HashMap::new(),
            reg_array_element_struct: HashMap::new(),
            reg_is_int_array: HashSet::new(),
            var_is_float_vec: HashSet::new(),
            reg_is_float: HashSet::new(),
            var_is_float: HashSet::new(),
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

        if let Err(e) = self.module.verify() {
             eprintln!("LLVM Verify Error: {}", e.to_string());
             return Err(anyhow!("Module verification failed: {}", e.to_string()));
        }

        // Build object
        let static_libs = options.static_libraries.clone();
        let (target_triple, target_machine) = Self::target_machine_for(&options)?;
        self.configure_module_target(&target_triple, &target_machine);

        if let Err(e) = self.module.verify() {
             eprintln!("LLVM Verify Error after target config: {}", e.to_string());
             return Err(anyhow!("Module verification failed after target config: {}", e.to_string()));
        }

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
        // Register struct/class types from all modules
        let mut modules: Vec<&IRModule> = prog.modules.iter().collect();
        modules.sort_by(|a, b| a.name.cmp(&b.name));
        for module in &modules {
            for type_def in module.types.iter() {
                if let IRType::Struct { name, fields, .. } = &type_def.type_def {
                    self.register_struct_type(name.as_str(), fields);
                }
            }
        }

        // Predeclare all functions
        let mut fn_map: HashMap<String, FunctionValue<'ctx>> = HashMap::new();
        for module in &modules {
            let mut funcs: Vec<&IRFunction> = module.functions_iter().collect();
            funcs.sort_by(|a, b| a.name.cmp(&b.name));
            for func in funcs {
                let f = self.declare_function(func)?;
                fn_map.insert(func.name.clone(), f);
                
                // Track return struct type
                if let IRType::Struct { name, .. } = &func.return_type {
                    self.fn_return_struct_types.insert(func.name.clone(), name.clone());
                }
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
                options.opt_level.unwrap_or(LlvmOptLevel::Default),
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

    fn ir_type_to_llvm(&self, t: &IRType) -> BasicTypeEnum<'ctx> {
        match t {
            // Void is not a BasicType in LLVM; callers that need a function
            // type must handle void explicitly. Provide a placeholder type to
            // satisfy type requirements in contexts that should never see Void.
            IRType::Void => self.ctx.i8_type().into(),
            IRType::Integer => self.i64_t.into(),
            IRType::Float => self.ctx.f64_type().into(),
            IRType::Boolean => self.bool_t.into(),
            IRType::Char => self.ctx.i8_type().into(),
            IRType::String => self.i8_ptr_t.into(),
            IRType::Array(_) => {
                // Represent arrays as opaque pointer to match runtime ABI
                self.i8_ptr_t.into()
            }
            IRType::Function {
                parameters,
                return_type,
            } => {
                let fn_ty = self.fn_type_from_ir(return_type, parameters);
                fn_ty.ptr_type(inkwell::AddressSpace::from(0u16)).into()
            }
            IRType::Vector { lanes, lane_type } => {
                let lane = self.ir_type_to_llvm(lane_type);
                match lane {
                    BasicTypeEnum::IntType(int_ty) => int_ty.vec_type(*lanes).into(),
                    BasicTypeEnum::FloatType(float_ty) => float_ty.vec_type(*lanes).into(),
                    BasicTypeEnum::PointerType(ptr_ty) => ptr_ty.vec_type(*lanes).into(),
                    BasicTypeEnum::VectorType(vec_ty) => vec_ty.into(),
                    BasicTypeEnum::ScalableVectorType(vec_ty) => vec_ty.into(),
                    BasicTypeEnum::StructType(_) | BasicTypeEnum::ArrayType(_) => {
                        self.i64_t.vec_type(*lanes).into()
                    }
                }
            }
            IRType::Struct { .. } => {
                // Use i8* as a placeholder pointer to struct
                self.i8_ptr_t.into()
            }
            IRType::Enum { .. } => self.i64_t.into(),
            IRType::Pointer(inner) | IRType::Reference(inner) => self
                .ir_type_to_llvm(inner)
                .ptr_type(inkwell::AddressSpace::from(0u16))
                .into(),
            IRType::Optional(inner) => {
                // Use pointer to inner where practical
                self.ir_type_to_llvm(inner)
                    .ptr_type(inkwell::AddressSpace::from(0u16))
                    .into()
            }
            IRType::Generic(_) => self.i8_ptr_t.into(),
        }
    }

    /// Register a struct type with its LLVM representation
    fn register_struct_type(&mut self, name: &str, fields: &[(String, IRType)]) {
        if self.struct_types.contains_key(name) {
            return; // Already registered
        }
        
        // Store Seen type definition
        self.struct_definitions.insert(name.to_string(), fields.to_vec());

        // Build LLVM struct type from fields
        let field_types: Vec<BasicTypeEnum<'ctx>> = fields
            .iter()
            .map(|(_, ty)| self.ir_type_to_llvm(ty))
            .collect();
        let field_names: Vec<String> = fields.iter().map(|(n, _)| n.clone()).collect();

        let llvm_struct_ty = self.ctx.struct_type(&field_types, false);
        self.struct_types.insert(name.to_string(), (llvm_struct_ty, field_names));
    }

    /// Get or create a struct type by name (used when struct is referenced before definition)
    fn get_or_create_struct_type(&mut self, type_name: &str, fields: &HashMap<String, IRValue>) -> StructType<'ctx> {
        if let Some((ty, _)) = self.struct_types.get(type_name) {
            return *ty;
        }

        // Infer struct layout from the values we have
        // Sort fields alphabetically for consistent ordering
        let mut field_names: Vec<String> = fields.keys().cloned().collect();
        field_names.sort();

        let field_types: Vec<BasicTypeEnum<'ctx>> = field_names
            .iter()
            .map(|name| {
                let val = &fields[name];
                match val {
                    IRValue::Integer(_) => self.i64_t.into(),
                    IRValue::Float(_) => self.ctx.f64_type().into(),
                    IRValue::Boolean(_) => self.bool_t.into(),
                    IRValue::String(_) => self.i8_ptr_t.into(),
                    _ => self.i8_ptr_t.into(), // Default to pointer for unknowns
                }
            })
            .collect();

        let llvm_struct_ty = self.ctx.struct_type(&field_types, false);
        self.struct_types.insert(type_name.to_string(), (llvm_struct_ty, field_names));
        llvm_struct_ty
    }

    fn declare_function(&self, func: &IRFunction) -> Result<FunctionValue<'ctx>> {
        let name = &func.name;
        if let Some(existing) = self.module.get_function(name) {
            return Ok(existing);
        }

        // Build param list
        let params_ir: Vec<IRType> = func
            .parameters
            .iter()
            .map(|p| p.param_type.clone())
            .collect();
        let fn_ty = self.fn_type_from_ir(&func.return_type, &params_ir);
        let f = self.module.add_function(name, fn_ty, None);
        self.apply_ir_function_attributes(func, f);
        self.apply_hardware_attributes(f);
        Ok(f)
    }

    fn apply_ir_function_attributes(&self, func: &IRFunction, llvm_fn: FunctionValue<'ctx>) {
        match func.inline_hint {
            InlineHint::AlwaysInline => self.add_enum_attribute(llvm_fn, "alwaysinline"),
            InlineHint::NeverInline => self.add_enum_attribute(llvm_fn, "noinline"),
            InlineHint::Auto => {}
        }

        if !matches!(func.register_pressure, RegisterPressureClass::Unknown) {
            let attr = self
                .ctx
                .create_string_attribute("seen-register-pressure", func.register_pressure.as_str());
            llvm_fn.add_attribute(AttributeLoc::Function, attr);
        }
    }

    fn add_enum_attribute(&self, llvm_fn: FunctionValue<'ctx>, name: &str) {
        let kind_id = Attribute::get_named_enum_kind_id(name);
        if kind_id == 0 {
            return;
        }
        let attr = self.ctx.create_enum_attribute(kind_id, 0);
        llvm_fn.add_attribute(AttributeLoc::Function, attr);
    }

    fn fn_type_from_ir(
        &self,
        ret: &IRType,
        params: &[IRType],
    ) -> inkwell::types::FunctionType<'ctx> {
        let params_ll: Vec<BasicMetadataTypeEnum> = params
            .iter()
            .map(|p| self.ir_type_to_llvm(p).into())
            .collect();
        match ret {
            IRType::Void => self.ctx.void_type().fn_type(&params_ll, false),
            _ => {
                let r: BasicTypeEnum = self.ir_type_to_llvm(ret);
                r.fn_type(&params_ll, false)
            }
        }
    }

    fn apply_hardware_attributes(&self, f: FunctionValue<'ctx>) {
        if let Some(bits) = self.hardware_profile.max_vector_bits {
            let width_str = bits.to_string();
            let prefer = self
                .ctx
                .create_string_attribute("prefer-vector-width", &width_str);
            f.add_attribute(AttributeLoc::Function, prefer);
            let min_attr = self
                .ctx
                .create_string_attribute("min-legal-vector-width", &width_str);
            f.add_attribute(AttributeLoc::Function, min_attr);
        }
        if !self.hardware_profile.cpu_features.is_empty() {
            let joined = self.hardware_profile.cpu_features.join(",");
            let attr = self.ctx.create_string_attribute("target-features", &joined);
            f.add_attribute(AttributeLoc::Function, attr);
        }
        let budget_attr = self.ctx.create_string_attribute(
            "seen-register-budget",
            &self.hardware_profile.register_budget_hint().to_string(),
        );
        f.add_attribute(AttributeLoc::Function, budget_attr);
        let scheduler_attr = self.ctx.create_string_attribute(
            "seen-scheduler-hint",
            self.hardware_profile.scheduler_hint().as_str(),
        );
        f.add_attribute(AttributeLoc::Function, scheduler_attr);
    }

    fn define_function(
        &mut self,
        func: &IRFunction,
        f: FunctionValue<'ctx>,
        fn_map: &HashMap<String, FunctionValue<'ctx>>,
    ) -> Result<()> {
        // Skip extern functions - they are just declarations without bodies
        if func.is_extern {
            return Ok(());
        }

        if func.name == "__PrintFloat" {
             let entry = self.ctx.append_basic_block(f, "entry");
             self.builder.position_at_end(entry);
             if let Some(arg) = f.get_nth_param(0) {
                 let float_val = arg.into_float_value();
                 let fmt = self.builder.build_global_string_ptr("%f\n", "fmt_float")?;
                 self.call_printf(&[fmt.as_pointer_value().into(), float_val.into()])?;
             }
             
             if let Some(ret_ty) = f.get_type().get_return_type() {
                 if ret_ty.is_int_type() {
                     self.builder.build_return(Some(&self.i64_t.const_zero()))?;
                 } else {
                     self.builder.build_return(None)?;
                 }
             } else {
                 self.builder.build_return(None)?;
             }
             return Ok(());
        }
        if func.name == "__IntToFloat" {
             let entry = self.ctx.append_basic_block(f, "entry");
             self.builder.position_at_end(entry);
             if let Some(arg) = f.get_nth_param(0) {
                 let int_val = arg.into_int_value();
                 let float_val = self.builder.build_signed_int_to_float(int_val, self.ctx.f64_type(), "i2f")?;
                 self.builder.build_return(Some(&float_val))?;
             } else {
                 self.builder.build_return(Some(&self.ctx.f64_type().const_zero()))?;
             }
             return Ok(());
        }

        self.current_fn = Some(f);
        self.apply_hardware_attributes(f);
        self.reg_values.clear();
        self.var_values.clear();
        self.var_slots.clear();
        self.var_slot_types.clear();
        self.var_struct_types.clear();
        self.reg_struct_types.clear();
        self.var_array_element_struct.clear();
        self.var_is_string.clear();
        self.var_is_int_array.clear();
        self.blocks.clear();
        self.reg_slots.clear();

        // Create all basic blocks first
        let block_names: Vec<_> = if !func.cfg.block_order.is_empty() {
            func.cfg.block_order.clone()
        } else {
            let mut names: Vec<_> = func
                .cfg
                .blocks_iter()
                .map(|block| block.label.0.clone())
                .collect();
            names.sort_by(|a, b| block_sort_key(a).cmp(&block_sort_key(b)));
            names
        };

        // LLVM starts execution at the first basic block appended to the function.
        // Ensure that the semantic entry block is created first so execution
        // begins where the Seen IR expects.
        if let Some(entry_name) = &func.cfg.entry_block {
            let bb = self.ctx.append_basic_block(f, entry_name);
            self.blocks.insert(entry_name.clone(), bb);
        }
        for name in &block_names {
            if Some(name) == func.cfg.entry_block.as_ref() {
                continue;
            }
            let bb = self.ctx.append_basic_block(f, name);
            self.blocks.insert(name.clone(), bb);
        }

        // Position at entry (create a synthetic one if the IR forgot to mark it).
        if let Some(entry_name) = &func.cfg.entry_block {
            let bb = self.blocks.get(entry_name).cloned().unwrap();
            self.builder.position_at_end(bb);
        } else {
            let bb = self.ctx.append_basic_block(f, "entry");
            self.blocks.insert("entry".to_string(), bb);
            self.builder.position_at_end(bb);
        }

        // Preallocate slots for parameters and locals so variable loads work across blocks.
        for param in &func.parameters {
            let ty = self.ir_type_to_llvm(&param.param_type);
            self.var_slot_types.insert(param.name.clone(), ty);
            let slot = self.alloca_for_type(ty, &format!("param_slot_{}", param.name))?;
            self.var_slots.insert(param.name.clone(), slot);
            // Track struct type names for field access
            if let IRType::Struct { name, .. } = &param.param_type {
                self.var_struct_types.insert(param.name.clone(), name.clone());
            }
            // Track array element struct types for array[i].field patterns
            if let IRType::Array(element_type) = &param.param_type {
                if let IRType::Struct { name, .. } = element_type.as_ref() {
                    self.var_array_element_struct.insert(param.name.clone(), name.clone());
                }
                // Track integer and char arrays for proper array access
                if matches!(element_type.as_ref(), IRType::Integer | IRType::Char) {
                    self.var_is_int_array.insert(param.name.clone());
                }
            }
            // Track string types for string indexing
            if matches!(param.param_type, IRType::String) {
                self.var_is_string.insert(param.name.clone());
            }
        }
        for local in func.locals_iter() {
            let ty = self.ir_type_to_llvm(&local.var_type);
            self.var_slot_types.insert(local.name.clone(), ty);
            let slot = self.alloca_for_type(ty, &format!("local_slot_{}", local.name))?;
            self.var_slots.insert(local.name.clone(), slot);
            // Track struct type names for field access
            if let IRType::Struct { name, .. } = &local.var_type {
                self.var_struct_types.insert(local.name.clone(), name.clone());
            }
            // Track array element struct types for array[i].field patterns
            if let IRType::Array(element_type) = &local.var_type {
                if let IRType::Struct { name, .. } = element_type.as_ref() {
                    self.var_array_element_struct.insert(local.name.clone(), name.clone());
                }
                // Track integer and char arrays for proper array access
                if matches!(element_type.as_ref(), IRType::Integer | IRType::Char) {
                    self.var_is_int_array.insert(local.name.clone());
                }
            }
            // Track string types for string indexing
            if matches!(local.var_type, IRType::String) {
                self.var_is_string.insert(local.name.clone());
            }
        }

        // Allocate slots for virtual registers as i64 cells
        for r in 0..func.register_count {
            let slot = self
                .builder
                .build_alloca(self.i64_t, &format!("reg{}_slot", r))
                .map_err(|e| anyhow!("{e:?}"))?;
            self.reg_slots.insert(r, slot);
        }

        // Initialize virtual registers with function parameters in order
        // (assumes IR uses %r0..%rN for the first N arguments)
        let param_count = f.count_params() as u32;
        for i in 0..param_count {
            if let Some(p) = f.get_nth_param(i as u32) {
                let param_val = p.clone();
                // Map %r{i}
                self.reg_values.insert(i, param_val.clone());
                // Also store into slot as i64 if available
                if let Some(slot) = self.reg_slots.get(&i).copied() {
                    let reg_val = param_val.clone();
                    let ival = if reg_val.is_int_value() {
                        let iv = reg_val.into_int_value();
                        if iv.get_type() == self.i64_t {
                            iv
                        } else {
                            self.builder
                                .build_int_s_extend(iv, self.i64_t, "sext")
                                .map_err(|e| anyhow!("{e:?}"))?
                        }
                    } else if reg_val.is_pointer_value() {
                        self.builder
                            .build_ptr_to_int(reg_val.into_pointer_value(), self.i64_t, "ptr2i")
                            .map_err(|e| anyhow!("{e:?}"))?
                    } else if reg_val.is_float_value() {
                        let fv = reg_val.into_float_value();
                        self.builder
                            .build_float_to_signed_int(fv, self.i64_t, "ftoi")
                            .map_err(|e| anyhow!("{e:?}"))?
                    } else {
                        self.i64_t.const_zero()
                    };
                    self.builder
                        .build_store(slot, ival)
                        .map_err(|e| anyhow!("{e:?}"))?;
                }
                // Map by parameter name when available
                if (i as usize) < func.parameters.len() {
                    let pname = func.parameters[i as usize].name.clone();
                    self.var_values.insert(pname.clone(), param_val.clone());
                    if let Some(slot) = self.var_slots.get(&pname).copied() {
                        let elem_ty = *self
                            .var_slot_types
                            .get(&pname)
                            .ok_or_else(|| anyhow!("Missing slot type for {}", pname))?;
                        let stored = self.cast_basic_to_type(param_val.clone(), elem_ty)?;
                        self.builder
                            .build_store(slot, stored)
                            .map_err(|e| anyhow!("{e:?}"))?;
                    }
                }
            }
        }

        // Emit entry block first, then the rest (stable order)
        let mut emit_order: Vec<String> = Vec::new();
        if let Some(entry_name) = &func.cfg.entry_block {
            emit_order.push(entry_name.clone());
        }
        for name in &block_names {
            if Some(name) != func.cfg.entry_block.as_ref() {
                emit_order.push(name.clone());
            }
        }

        #[cfg(debug_assertions)]
        if func.name == "main" {
            eprintln!("LLVM emit order: {:?}", emit_order);
        }

        for (idx, name) in emit_order.iter().enumerate() {
            let bb = self.blocks.get(name).cloned().unwrap();
            if self
                .builder
                .get_insert_block()
                .map(|b| b != bb)
                .unwrap_or(true)
            {
                self.builder.position_at_end(bb);
            }

            self.fallthrough_bb = emit_order
                .get(idx + 1)
                .and_then(|next| self.blocks.get(next).cloned());

            let b = func
                .cfg
                .get_block(name)
                .expect("basic block must exist in CFG");
            for inst in &b.instructions {
                self.emit_instruction(inst, fn_map)?;
            }
            if let Some(term) = &b.terminator {
                self.emit_instruction(term, fn_map)?;
            }
            self.fallthrough_bb = None;
            self.reg_values.clear();
            self.var_values.clear();
        }

        Ok(())
    }

    fn emit_instruction(
        &mut self,
        inst: &Instruction,
        fn_map: &HashMap<String, FunctionValue<'ctx>>,
    ) -> Result<()> {
        if let Instruction::Call { target, .. } = inst {
        }
        match inst {
            Instruction::Label(lbl) => {
                if let Some(bb) = self.blocks.get(&lbl.0) {
                    if self
                        .builder
                        .get_insert_block()
                        .map(|b| b != *bb)
                        .unwrap_or(true)
                    {
                        self.builder.position_at_end(*bb);
                    }
                }
            }
            Instruction::Jump(target) => {
                let dst = *self
                    .blocks
                    .get(&target.0)
                    .ok_or_else(|| anyhow!("Unknown label {}", target.0))?;
                self.builder.build_unconditional_branch(dst)?;
                self.builder.clear_insertion_position();
            }
            Instruction::JumpIf { condition, target } => {
                let cond = self.eval_value(condition, fn_map)?;
                let i1 = self.as_bool(cond)?;
                let dst = *self
                    .blocks
                    .get(&target.0)
                    .ok_or_else(|| anyhow!("Unknown label {}", target.0))?;
                let false_bb = match self.fallthrough_bb {
                    Some(block) => block,
                    None => {
                        let fb = self
                            .ctx
                            .append_basic_block(self.current_fn.unwrap(), "fallthrough");
                        self.builder.position_at_end(fb);
                        self.builder.build_unreachable()?;
                        self.builder.clear_insertion_position();
                        fb
                    }
                };
                self.builder.build_conditional_branch(i1, dst, false_bb)?;
                self.builder.clear_insertion_position();
            }
            Instruction::JumpIfNot { condition, target } => {
                let cond = self.eval_value(condition, fn_map)?;
                let i1 = self.as_bool(cond)?;
                let dst = *self
                    .blocks
                    .get(&target.0)
                    .ok_or_else(|| anyhow!("Unknown label {}", target.0))?;
                let true_bb = match self.fallthrough_bb {
                    Some(block) => block,
                    None => {
                        let fb = self
                            .ctx
                            .append_basic_block(self.current_fn.unwrap(), "fallthrough");
                        self.builder.position_at_end(fb);
                        self.builder.build_unreachable()?;
                        self.builder.clear_insertion_position();
                        fb
                    }
                };
                let not = self.builder.build_not(i1, "not")?;
                self.builder.build_conditional_branch(not, dst, true_bb)?;
                self.builder.clear_insertion_position();
            }
            Instruction::Return(val_opt) => {
                let current_fn = self
                    .current_fn
                    .ok_or_else(|| anyhow!("return outside of function"))?;
                let ret_ty_opt = current_fn.get_type().get_return_type();
                match (val_opt, ret_ty_opt) {
                    (Some(v), Some(ret_ty)) => {
                        let mut bv = self.eval_value(v, fn_map)?;
                        if bv.get_type() != ret_ty {
                            bv = self.cast_basic_to_type(bv, ret_ty)?;
                        }
                        self.builder.build_return(Some(&bv))?;
                    }
                    (Some(_), None) => {
                        self.builder.build_return(None)?;
                    }
                    (None, None) => {
                        self.builder.build_return(None)?;
                    }
                    (None, Some(ret_ty)) => {
                        if ret_ty.is_int_type() {
                            self.builder.build_return(Some(&ret_ty.into_int_type().const_zero()))?;
                        } else {
                            self.builder.build_return(None)?;
                        }
                    }
                }
                self.builder.clear_insertion_position();
            }
            Instruction::Move { source, dest } => {
                let v = self.eval_value(source, fn_map)?;
                self.assign_value(dest, v)?;
            }
            Instruction::Store { value, dest } => {
                // Propagate float type info BEFORE assign_value so it can use bitcast
                if let IRValue::Variable(var_name) = dest {
                    match value {
                        IRValue::Register(reg_id) => {
                            if self.reg_is_float.contains(reg_id) {
                                self.var_is_float.insert(var_name.clone());
                            }
                        }
                        IRValue::Variable(src_name) => {
                            if self.var_is_float.contains(src_name) {
                                self.var_is_float.insert(var_name.clone());
                            }
                        }
                        _ => {}
                    }
                }
                
                let v = self.eval_value(value, fn_map)?;
                self.assign_value(dest, v)?;

                // Propagate struct type info
                if let IRValue::Variable(var_name) = dest {
                    match value {
                        IRValue::Register(reg_id) => {
                            if let Some(struct_name) = self.reg_struct_types.get(reg_id) {
                                self.var_struct_types.insert(var_name.clone(), struct_name.clone());
                            }
                        }
                        IRValue::Variable(src_name) => {
                            if let Some(struct_name) = self.var_struct_types.get(src_name) {
                                self.var_struct_types.insert(var_name.clone(), struct_name.clone());
                            }
                        }
                        _ => {}
                    }
                }
            }
            Instruction::Load { source, dest } => {
                let v = self.eval_value(source, fn_map)?;
                self.assign_value(dest, v)?;
            }
            Instruction::Unary {
                op,
                operand,
                result,
            } => {
                let val = self.eval_value(operand, fn_map)?;
                let res = match op {
                    crate::instruction::UnaryOp::Not => {
                        let bool_val = self.as_bool(val)?;
                        self.builder
                            .build_not(bool_val, "not")
                            .map_err(|e| anyhow!("{e:?}"))?
                            .as_basic_value_enum()
                    }
                    crate::instruction::UnaryOp::Negate => {
                        if let BasicValueEnum::FloatValue(fv) = val {
                            self.builder
                                .build_float_neg(fv, "fneg")
                                .map_err(|e| anyhow!("{e:?}"))?
                                .as_basic_value_enum()
                        } else {
                            let int_val = self.as_i64(val)?;
                            self.builder
                                .build_int_neg(int_val, "ineg")
                                .map_err(|e| anyhow!("{e:?}"))?
                                .as_basic_value_enum()
                        }
                    }
                    crate::instruction::UnaryOp::BitwiseNot => {
                        let int_val = self.as_i64(val)?;
                        self.builder
                            .build_not(int_val, "bnot")
                            .map_err(|e| anyhow!("{e:?}"))?
                            .as_basic_value_enum()
                    }
                    crate::instruction::UnaryOp::Reference
                    | crate::instruction::UnaryOp::Dereference => {
                        return Err(anyhow!(
                            "Reference/dereference unary ops are not yet supported in LLVM backend"
                        ));
                    }
                };
                self.assign_value(result, res)?;
            }
            Instruction::Binary {
                op,
                left,
                right,
                result,
            } => {
                let l = self.eval_value(left, fn_map)?;
                let r = self.eval_value(right, fn_map)?;
                // Check if either operand is a float for arithmetic operations
                let is_float_op = l.is_float_value() || r.is_float_value();
                let res = match op {
                    crate::instruction::BinaryOp::Add => {
                        if is_float_op {
                            let lf = self.as_f64(l.clone())?;
                            let rf = self.as_f64(r.clone())?;
                            self.builder
                                .build_float_add(lf, rf, "fadd")?
                                .as_basic_value_enum()
                        } else {
                            let li = self.as_i64(l.clone())?;
                            let ri = self.as_i64(r.clone())?;
                            self.builder
                                .build_int_add(li, ri, "add")?
                                .as_basic_value_enum()
                        }
                    }
                    crate::instruction::BinaryOp::Subtract => {
                        if is_float_op {
                            let lf = self.as_f64(l.clone())?;
                            let rf = self.as_f64(r.clone())?;
                            self.builder
                                .build_float_sub(lf, rf, "fsub")?
                                .as_basic_value_enum()
                        } else {
                            let li = self.as_i64(l.clone())?;
                            let ri = self.as_i64(r.clone())?;
                            self.builder
                                .build_int_sub(li, ri, "sub")?
                                .as_basic_value_enum()
                        }
                    }
                    crate::instruction::BinaryOp::Multiply => {
                        if is_float_op {
                            let lf = self.as_f64(l.clone())?;
                            let rf = self.as_f64(r.clone())?;
                            self.builder
                                .build_float_mul(lf, rf, "fmul")?
                                .as_basic_value_enum()
                        } else {
                            let li = self.as_i64(l.clone())?;
                            let ri = self.as_i64(r.clone())?;
                            self.builder
                                .build_int_mul(li, ri, "mul")?
                                .as_basic_value_enum()
                        }
                    }
                    crate::instruction::BinaryOp::Divide => {
                        if is_float_op {
                            let lf = self.as_f64(l.clone())?;
                            let rf = self.as_f64(r.clone())?;
                            self.builder
                                .build_float_div(lf, rf, "fdiv")?
                                .as_basic_value_enum()
                        } else {
                            let li = self.as_i64(l.clone())?;
                            let ri = self.as_i64(r.clone())?;
                            self.builder
                                .build_int_signed_div(li, ri, "div")?
                                .as_basic_value_enum()
                        }
                    }
                    crate::instruction::BinaryOp::Modulo => {
                        let li = self.as_i64(l.clone())?;
                        let ri = self.as_i64(r.clone())?;
                        self.builder
                            .build_int_signed_rem(li, ri, "mod")?
                            .as_basic_value_enum()
                    }
                    crate::instruction::BinaryOp::Equal
                    | crate::instruction::BinaryOp::NotEqual => {
                        let pred = match op {
                            crate::instruction::BinaryOp::Equal => inkwell::IntPredicate::EQ,
                            _ => inkwell::IntPredicate::NE,
                        };
                        if self.is_string_value_ir(left) || self.is_string_value_ir(right) {
                            let lp = self.as_cstr_ptr(l.clone())?;
                            let rp = self.as_cstr_ptr(r.clone())?;
                            let cmp = self.call_strcmp(lp, rp)?;
                            let zero = self.ctx.i32_type().const_zero();
                            self.builder
                                .build_int_compare(pred, cmp, zero, "strcmp")?
                                .as_basic_value_enum()
                        } else {
                            let li = self.as_i64(l.clone())?;
                            let ri = self.as_i64(r.clone())?;
                            self.builder
                                .build_int_compare(pred, li, ri, "icmp")?
                                .as_basic_value_enum()
                        }
                    }
                    crate::instruction::BinaryOp::LessThan => {
                        let li = self.as_i64(l.clone())?;
                        let ri = self.as_i64(r.clone())?;
                        self.builder
                            .build_int_compare(inkwell::IntPredicate::SLT, li, ri, "lt")?
                            .as_basic_value_enum()
                    }
                    crate::instruction::BinaryOp::LessEqual => {
                        let li = self.as_i64(l.clone())?;
                        let ri = self.as_i64(r.clone())?;
                        self.builder
                            .build_int_compare(inkwell::IntPredicate::SLE, li, ri, "le")?
                            .as_basic_value_enum()
                    }
                    crate::instruction::BinaryOp::GreaterThan => {
                        let li = self.as_i64(l.clone())?;
                        let ri = self.as_i64(r.clone())?;
                        self.builder
                            .build_int_compare(inkwell::IntPredicate::SGT, li, ri, "gt")?
                            .as_basic_value_enum()
                    }
                    crate::instruction::BinaryOp::GreaterEqual => {
                        let li = self.as_i64(l.clone())?;
                        let ri = self.as_i64(r.clone())?;
                        self.builder
                            .build_int_compare(inkwell::IntPredicate::SGE, li, ri, "ge")?
                            .as_basic_value_enum()
                    }
                    crate::instruction::BinaryOp::And => {
                        let li = self.as_bool(l.clone())?;
                        let ri = self.as_bool(r.clone())?;
                        self.builder.build_and(li, ri, "and")?.as_basic_value_enum()
                    }
                    crate::instruction::BinaryOp::Or => {
                        let li = self.as_bool(l.clone())?;
                        let ri = self.as_bool(r.clone())?;
                        self.builder.build_or(li, ri, "or")?.as_basic_value_enum()
                    }
                    crate::instruction::BinaryOp::BitwiseAnd => {
                        let li = self.as_i64(l.clone())?;
                        let ri = self.as_i64(r.clone())?;
                        self.builder
                            .build_and(li, ri, "band")?
                            .as_basic_value_enum()
                    }
                    crate::instruction::BinaryOp::BitwiseOr => {
                        let li = self.as_i64(l.clone())?;
                        let ri = self.as_i64(r.clone())?;
                        self.builder.build_or(li, ri, "bor")?.as_basic_value_enum()
                    }
                    crate::instruction::BinaryOp::BitwiseXor => {
                        let li = self.as_i64(l.clone())?;
                        let ri = self.as_i64(r.clone())?;
                        self.builder
                            .build_xor(li, ri, "bxor")?
                            .as_basic_value_enum()
                    }
                    crate::instruction::BinaryOp::LeftShift => {
                        let li = self.as_i64(l.clone())?;
                        let ri = self.as_i64(r.clone())?;
                        self.builder
                            .build_left_shift(li, ri, "shl")?
                            .as_basic_value_enum()
                    }
                    crate::instruction::BinaryOp::RightShift => {
                        let li = self.as_i64(l.clone())?;
                        let ri = self.as_i64(r.clone())?;
                        self.builder
                            .build_right_shift(li, ri, true, "shr")?
                            .as_basic_value_enum()
                    }
                };
                self.assign_value(result, res)?;
            }
            Instruction::StringLength { string, result } => {
                let s_val = self.eval_value(string, fn_map)?;
                let s_ptr = self.as_cstr_ptr(s_val)?;
                let slen = self.call_strlen(s_ptr)?;
                self.assign_value(result, slen.as_basic_value_enum())?;
            }
            Instruction::StringConcat {
                left,
                right,
                result,
            } => {
                let lval = self.eval_value(left, fn_map)?;
                let rval = self.eval_value(right, fn_map)?;
                let l = self.to_string_ptr(lval)?;
                let r = self.to_string_ptr(rval)?;
                let out = self.runtime_concat(l, r)?;
                self.assign_value(result, out.as_basic_value_enum())?;
            }
            Instruction::SimdSplat {
                scalar,
                lane_type,
                lanes,
                result,
            } => {
                let mut scalar_val = self.eval_value(scalar, fn_map)?;
                let lane_basic = self.ir_type_to_llvm(lane_type);
                scalar_val = self.cast_basic_to_type(scalar_val, lane_basic)?;
                let vec_type =
                    match self.ir_type_to_llvm(&IRType::vector(*lanes, lane_type.clone())) {
                        BasicTypeEnum::VectorType(vt) => vt,
                        other => {
                            return Err(anyhow!(
                                "simd.splat requires numeric lane type, found {other:?}"
                            ))
                        }
                    };
                let mut acc = vec_type.get_undef();
                let index_ty = self.ctx.i32_type();
                for idx in 0..*lanes {
                    let lane_index = index_ty.const_int(idx as u64, false);
                    acc = self
                        .builder
                        .build_insert_element(
                            acc,
                            scalar_val,
                            lane_index,
                            &format!("splat_lane_{idx}"),
                        )
                        .map_err(|e| anyhow!("{e:?}"))?;
                }
                self.assign_value(result, acc.as_basic_value_enum())?;
            }
            Instruction::SimdReduceAdd {
                vector,
                lane_type,
                result,
            } => {
                let vec_basic = self.eval_value(vector, fn_map)?;
                let vec_value = match vec_basic {
                    BasicValueEnum::VectorValue(vec) => vec,
                    _ => {
                        return Err(anyhow!("simd.reduce_add expects a vector operand"));
                    }
                };
                let lanes = vec_value.get_type().get_size();
                let index_ty = self.ctx.i32_type();
                let mut acc = match lane_type {
                    IRType::Float => self.ctx.f64_type().const_float(0.0).as_basic_value_enum(),
                    IRType::Integer => self.i64_t.const_zero().as_basic_value_enum(),
                    _ => {
                        return Err(anyhow!(
                            "simd.reduce_add currently supports integer or float lanes"
                        ))
                    }
                };
                for idx in 0..lanes {
                    let lane_index = index_ty.const_int(idx as u64, false);
                    let lane = self
                        .builder
                        .build_extract_element(
                            vec_value,
                            lane_index,
                            &format!("lane_extract_{idx}"),
                        )
                        .map_err(|e| anyhow!("{e:?}"))?;
                    acc = match lane_type {
                        IRType::Float => {
                            let a = acc.into_float_value();
                            let b = lane.into_float_value();
                            self.builder
                                .build_float_add(a, b, "reduce_fadd")
                                .map_err(|e| anyhow!("{e:?}"))?
                                .as_basic_value_enum()
                        }
                        IRType::Integer => {
                            let a = acc.into_int_value();
                            let b = lane.into_int_value();
                            self.builder
                                .build_int_add(a, b, "reduce_iadd")
                                .map_err(|e| anyhow!("{e:?}"))?
                                .as_basic_value_enum()
                        }
                        _ => unreachable!(),
                    };
                }
                self.assign_value(result, acc)?;
            }
            Instruction::ArrayLength { array, result } => {
                // Dynamic arrays with layout { i64 len, i64 capacity, data... }
                // Length is always at offset 0
                let arr_v = self.eval_value(array, fn_map)?;
                let res = if let IRValue::Array(values) = array {
                    self.i64_t
                        .const_int(values.len() as u64, false)
                        .as_basic_value_enum()
                } else if arr_v.is_pointer_value() || arr_v.is_int_value() {
                    let arr_ptr = if arr_v.is_pointer_value() {
                        arr_v.into_pointer_value()
                    } else {
                        self.builder
                            .build_int_to_ptr(
                                arr_v.into_int_value(),
                                self.i64_t.ptr_type(inkwell::AddressSpace::from(0u16)),
                                "arr_len_ptr",
                            )
                            .map_err(|e| anyhow!("{e:?}"))?
                    };
                    // Cast to i64* and load the first i64 (length)
                    let len_ptr = self.builder.build_pointer_cast(
                        arr_ptr,
                        self.i64_t.ptr_type(inkwell::AddressSpace::from(0u16)),
                        "len_ptr"
                    )?;
                    let len = self.builder.build_load(self.i64_t, len_ptr, "len")?;
                    len.as_basic_value_enum()
                } else {
                    self.i64_t.const_int(0, false).as_basic_value_enum()
                };
                self.assign_value(result, res)?;
            }
            Instruction::ArrayAccess {
                array,
                index,
                result,
            } => {
                let arr_v = self.eval_value(array, fn_map)?;
                
                // Check if this is a string variable (for string indexing)
                let is_string = if let IRValue::Variable(var_name) = array {
                    self.var_is_string.contains(var_name)
                } else {
                    false
                };
                
                // Check if this is an integer array
                let is_int_array = match array {
                    IRValue::Variable(var_name) => self.var_is_int_array.contains(var_name),
                    IRValue::Register(reg_id) => self.reg_is_int_array.contains(reg_id),
                    _ => false,
                };
                
                // Check if this is an array of structs
                let element_struct_type = match array {
                    IRValue::Variable(var_name) => self.var_array_element_struct.get(var_name).cloned(),
                    IRValue::Register(reg_id) => self.reg_array_element_struct.get(reg_id).cloned(),
                    _ => None
                };
                
                if let IRValue::Array(vs) = array {
                    let idx_bv = self.eval_value(index, fn_map)?;
                    let idx_val = self.as_usize(idx_bv)? as usize;
                    if idx_val >= vs.len() {
                        return Err(anyhow!("Array index OOB"));
                    }
                    let elem = self.eval_value(&vs[idx_val], fn_map)?;
                    self.assign_value(result, elem)?;
                } else if is_string {
                    // String indexing: string is an i8* (pointer to characters)
                    // We need to index into the character array and return the char code as i64
                    let str_ptr = if arr_v.is_pointer_value() {
                        arr_v.into_pointer_value()
                    } else {
                        self.builder
                            .build_int_to_ptr(
                                arr_v.into_int_value(),
                                self.i8_ptr_t,
                                "str_int_to_ptr",
                            )
                            .map_err(|e| anyhow!("{e:?}"))?
                    };
                    
                    let idx_bv = self.eval_value(index, fn_map)?;
                    let idx_iv = self.as_i64(idx_bv)?;
                    
                    // GEP to the character at index
                    let char_ptr = unsafe {
                        self.builder.build_gep(
                            self.ctx.i8_type(),
                            str_ptr,
                            &[idx_iv],
                            "char_ptr",
                        )?
                    };
                    
                    // Load the character as i8 and zero-extend to i64
                    let char_val = self.builder.build_load(self.ctx.i8_type(), char_ptr, "char_val")?.into_int_value();
                    let char_i64 = self.builder.build_int_z_extend(char_val, self.i64_t, "char_i64")?;
                    
                    self.assign_value(result, char_i64.as_basic_value_enum())?;
                } else if arr_v.is_pointer_value() || arr_v.is_int_value() {
                    // Dynamic array with layout { i64 len, i64 cap, i8* data_ptr }
                    // Data pointer is at offset 16
                    let arr_ptr = if arr_v.is_pointer_value() {
                        arr_v.into_pointer_value()
                    } else {
                        self.builder
                            .build_int_to_ptr(
                                arr_v.into_int_value(),
                                self.i8_ptr_t,
                                "arr_int_to_ptr",
                            )
                            .map_err(|e| anyhow!("{e:?}"))?
                    };
                    
                    // Load data pointer from offset 16
                    let data_ptr_ptr = unsafe {
                        self.builder.build_gep(
                            self.i64_t,
                            self.builder.build_pointer_cast(arr_ptr, self.i64_t.ptr_type(inkwell::AddressSpace::from(0u16)), "cast")?,
                            &[self.i64_t.const_int(2, false)],
                            "data_ptr_ptr"
                        )?
                    };
                    let data_ptr_ptr_casted = self.builder.build_pointer_cast(
                        data_ptr_ptr,
                        self.i8_ptr_t.ptr_type(inkwell::AddressSpace::from(0u16)),
                        "data_ptr_ptr_casted"
                    )?;
                    let data_ptr = self.builder.build_load(self.i8_ptr_t, data_ptr_ptr_casted, "data_ptr")?.into_pointer_value();
                    
                    let idx_bv = self.eval_value(index, fn_map)?;
                    let idx_iv = self.as_i64(idx_bv)?;

                    // BOUNDS CHECK
                    let len_ptr = self.builder.build_pointer_cast(
                        arr_ptr,
                        self.i64_t.ptr_type(inkwell::AddressSpace::from(0u16)),
                        "len_ptr"
                    )?;
                    let len = self.builder.build_load(self.i64_t, len_ptr, "len")?.into_int_value();

                    let cmp = self.builder.build_int_compare(
                        inkwell::IntPredicate::UGE,
                        idx_iv,
                        len,
                        "bounds_check"
                    )?;
                    
                    let fail_bb = self.ctx.append_basic_block(self.current_fn.unwrap(), "bounds_fail");
                    let cont_bb = self.ctx.append_basic_block(self.current_fn.unwrap(), "bounds_ok");
                    
                    self.builder.build_conditional_branch(cmp, fail_bb, cont_bb)?;
                    
                    self.builder.position_at_end(fail_bb);
                    // Trap
                    if let Some(trap) = self.module.get_function("llvm.trap") {
                        self.builder.build_call(trap, &[], "trap")?;
                    } else {
                        let trap_ty = self.ctx.void_type().fn_type(&[], false);
                        let trap = self.module.add_function("llvm.trap", trap_ty, None);
                        self.builder.build_call(trap, &[], "trap")?;
                    }
                    self.builder.build_unreachable()?;
                    
                    self.builder.position_at_end(cont_bb);
                    
                    // Check if we're accessing a struct array
                    if let Some(ref struct_type_name) = element_struct_type {
                        if self.struct_types.contains_key(struct_type_name) {
                            // Struct arrays store pointers to heap-allocated structs
                            // Each element is a pointer (8 bytes)
                            let i64_ptr_ty = self.i64_t.ptr_type(inkwell::AddressSpace::from(0u16));
                            let data_i64_ptr = self.builder.build_pointer_cast(data_ptr, i64_ptr_ty, "data_ptr_ptr")?;
                            
                            // GEP to the element (pointer at index i)
                            let elem_ptr = unsafe {
                                self.builder.build_gep(
                                    self.i64_t,
                                    data_i64_ptr,
                                    &[idx_iv],
                                    "struct_elem_ptr_ptr",
                                )?
                            };
                            
                            // Load the struct pointer (stored as i64)
                            let struct_ptr_i64 = self.builder.build_load(self.i64_t, elem_ptr, "struct_ptr_load")?.into_int_value();
                            
                            // Assign to result
                            self.assign_value(result, struct_ptr_i64.as_basic_value_enum())?;
                            
                            // Track struct type for subsequent field access
                            if let IRValue::Register(reg_id) = result {
                                self.reg_struct_types.insert(*reg_id, struct_type_name.clone());
                            }
                            return Ok(());
                        }
                    }
                    
                    // Check if we're accessing an integer array
                    if is_int_array {
                        let i64_ptr_ty = self.i64_t.ptr_type(inkwell::AddressSpace::from(0u16));
                        let data_i64_ptr = self.builder.build_pointer_cast(data_ptr, i64_ptr_ty, "data_i64_ptr")?;
                        
                        let elem_ptr = unsafe {
                            self.builder.build_gep(
                                self.i64_t,
                                data_i64_ptr,
                                &[idx_iv],
                                "int_elem_ptr",
                            )?
                        };
                        let elem = self.builder.build_load(self.i64_t, elem_ptr, "int_elem")?;
                        self.assign_value(result, elem.as_basic_value_enum())?;
                        return Ok(());
                    }
                    
                    // Default: treat as f64 array
                    let f64_ptr_ty = self.ctx.f64_type().ptr_type(inkwell::AddressSpace::from(0u16));
                    let data_f64_ptr = self.builder.build_pointer_cast(data_ptr, f64_ptr_ty, "data_f64_ptr")?;
                    
                    let elem_ptr = unsafe {
                        self.builder.build_gep(
                            self.ctx.f64_type(),
                            data_f64_ptr,
                            &[idx_iv],
                            "elem_ptr",
                        )?
                    };
                    let elem = self.builder.build_load(self.ctx.f64_type(), elem_ptr, "elem")?;
                    self.assign_value(result, elem.as_basic_value_enum())?;
                } else {
                    return Err(anyhow!("Unsupported array access value"));
                }
            }
            Instruction::ArraySet { array, index, value } => {
                // Dynamic array with layout { i64 len, i64 cap, data[...] }
                // Data starts at offset 16 (2 * sizeof(i64))
                let arr_v = self.eval_value(array, fn_map)?;
                let val_v = self.eval_value(value, fn_map)?;
                
                // Check if this is an integer array
                let is_int_array = match array {
                    IRValue::Variable(var_name) => self.var_is_int_array.contains(var_name),
                    IRValue::Register(reg_id) => self.reg_is_int_array.contains(reg_id),
                    _ => false,
                };
                
                // Check if this is a struct array
                let element_struct_type = if let IRValue::Variable(var_name) = array {
                    self.var_array_element_struct.get(var_name).cloned()
                } else {
                    None
                };
                
                let arr_ptr = if arr_v.is_pointer_value() {
                    arr_v.into_pointer_value()
                } else {
                    self.builder
                        .build_int_to_ptr(
                            arr_v.into_int_value(),
                            self.i8_ptr_t,
                            "arr_set_ptr",
                        )
                        .map_err(|e| anyhow!("{e:?}"))?
                };
                
                // Get pointer to data section (offset 16)
                let data_ptr_ptr = unsafe {
                    self.builder.build_gep(
                        self.i64_t,
                        self.builder.build_pointer_cast(arr_ptr, self.i64_t.ptr_type(inkwell::AddressSpace::from(0u16)), "cast")?,
                        &[self.i64_t.const_int(2, false)],
                        "data_ptr_ptr"
                    )?
                };
                let data_ptr_ptr_casted = self.builder.build_pointer_cast(
                    data_ptr_ptr,
                    self.i8_ptr_t.ptr_type(inkwell::AddressSpace::from(0u16)),
                    "data_ptr_ptr_casted"
                )?;
                let data_ptr = self.builder.build_load(self.i8_ptr_t, data_ptr_ptr_casted, "data_ptr")?.into_pointer_value();
                
                let idx_bv = self.eval_value(index, fn_map)?;
                let idx_iv = self.as_i64(idx_bv)?;

                // BOUNDS CHECK
                let len_ptr = self.builder.build_pointer_cast(
                    arr_ptr,
                    self.i64_t.ptr_type(inkwell::AddressSpace::from(0u16)),
                    "len_ptr"
                )?;
                let len = self.builder.build_load(self.i64_t, len_ptr, "len")?.into_int_value();

                let cmp = self.builder.build_int_compare(
                    inkwell::IntPredicate::UGE,
                    idx_iv,
                    len,
                    "bounds_check"
                )?;
                
                let fail_bb = self.ctx.append_basic_block(self.current_fn.unwrap(), "bounds_fail");
                let cont_bb = self.ctx.append_basic_block(self.current_fn.unwrap(), "bounds_ok");
                
                self.builder.build_conditional_branch(cmp, fail_bb, cont_bb)?;
                
                self.builder.position_at_end(fail_bb);
                // Trap
                if let Some(trap) = self.module.get_function("llvm.trap") {
                    self.builder.build_call(trap, &[], "trap")?;
                } else {
                    let trap_ty = self.ctx.void_type().fn_type(&[], false);
                    let trap = self.module.add_function("llvm.trap", trap_ty, None);
                    self.builder.build_call(trap, &[], "trap")?;
                }
                self.builder.build_unreachable()?;
                
                self.builder.position_at_end(cont_bb);
                
                // Check if we're setting a struct array element
                if let Some(ref struct_type_name) = element_struct_type {
                    if self.struct_types.contains_key(struct_type_name) {
                        // Struct arrays store pointers to heap-allocated structs
                        let i64_ptr_ty = self.i64_t.ptr_type(inkwell::AddressSpace::from(0u16));
                        let data_i64_ptr = self.builder.build_pointer_cast(data_ptr, i64_ptr_ty, "data_ptr_ptr")?;
                        
                        let elem_ptr = unsafe {
                            self.builder.build_gep(
                                self.i64_t,
                                data_i64_ptr,
                                &[idx_iv],
                                "struct_elem_ptr_ptr",
                            )?
                        };
                        
                        // Store the struct pointer (as i64)
                        let ptr_as_i64 = if val_v.is_int_value() {
                            val_v.into_int_value()
                        } else if val_v.is_pointer_value() {
                            self.builder.build_ptr_to_int(
                                val_v.into_pointer_value(),
                                self.i64_t,
                                "ptr2i_struct"
                            )?
                        } else {
                            return Err(anyhow!("ArraySet struct: unsupported value type"));
                        };
                        self.builder.build_store(elem_ptr, ptr_as_i64)?;
                        return Ok(());
                    }
                }
                
                // Check if we're setting an integer array element
                if is_int_array {
                    let i64_ptr_ty = self.i64_t.ptr_type(inkwell::AddressSpace::from(0u16));
                    let data_i64_ptr = self.builder.build_pointer_cast(data_ptr, i64_ptr_ty, "data_i64_ptr")?;
                    
                    let elem_ptr = unsafe {
                        self.builder.build_gep(
                            self.i64_t,
                            data_i64_ptr,
                            &[idx_iv],
                            "int_elem_ptr",
                        )?
                    };
                    
                    // Store value (convert to i64 if needed)
                    let i64_val = if val_v.is_int_value() {
                        val_v.into_int_value()
                    } else if val_v.is_float_value() {
                        self.builder.build_float_to_signed_int(
                            val_v.into_float_value(),
                            self.i64_t,
                            "f2i"
                        )?
                    } else if val_v.is_pointer_value() {
                        self.builder.build_ptr_to_int(
                            val_v.into_pointer_value(),
                            self.i64_t,
                            "ptr2i"
                        )?
                    } else {
                        return Err(anyhow!("ArraySet: unsupported value type"));
                    };
                    self.builder.build_store(elem_ptr, i64_val)?;
                    return Ok(());
                }
                
                // Default: treat as f64 array
                let f64_ptr_ty = self.ctx.f64_type().ptr_type(inkwell::AddressSpace::from(0u16));
                let data_f64_ptr = self.builder.build_pointer_cast(data_ptr, f64_ptr_ty, "data_f64_ptr")?;
                
                let elem_ptr = unsafe {
                    self.builder.build_gep(
                        self.ctx.f64_type(),
                        data_f64_ptr,
                        &[idx_iv],
                        "elem_ptr",
                    )?
                };
                
                // Store value (convert to f64 if needed)
                let f64_val = if val_v.is_float_value() {
                    val_v.into_float_value()
                } else if val_v.is_int_value() {
                    self.builder.build_signed_int_to_float(
                        val_v.into_int_value(),
                        self.ctx.f64_type(),
                        "i2f"
                    )?
                } else if val_v.is_pointer_value() {
                    let int_val = self.builder.build_ptr_to_int(
                        val_v.into_pointer_value(),
                        self.i64_t,
                        "ptr2i"
                    )?;
                    self.builder.build_signed_int_to_float(
                        int_val,
                        self.ctx.f64_type(),
                        "i2f"
                    )?
                } else {
                    return Err(anyhow!("ArraySet: unsupported value type"));
                };
                self.builder.build_store(elem_ptr, f64_val)?;
            }
            Instruction::Call {
                target,
                args,
                result,
            } => {
                // Handle known intrinsics
                if let IRValue::Variable(name) = target {
                    match name.as_str() {
                        "toFloat" => {
                            // Convert integer to float
                            if let Some(arg) = args.get(0) {
                                let val = self.eval_value(arg, fn_map)?;
                                let float_val = if val.is_float_value() {
                                    val.into_float_value()
                                } else if val.is_int_value() {
                                    self.builder.build_signed_int_to_float(
                                        val.into_int_value(),
                                        self.ctx.f64_type(),
                                        "toFloat"
                                    )?
                                } else {
                                    self.ctx.f64_type().const_zero()
                                };
                                if let Some(r) = result {
                                    self.assign_value(r, float_val.as_basic_value_enum())?;
                                }
                            }
                            return Ok(());
                        }
                        "__default" => {
                            // Return 0 (i64) as default value
                            if let Some(r) = result {
                                self.assign_value(r, self.i64_t.const_zero().as_basic_value_enum())?;
                            }
                            return Ok(());
                        }
                        "abort" => {
                            // Print message and exit
                            if let Some(arg) = args.get(0) {
                                let msg_val = self.eval_value(arg, fn_map)?;
                                let msg_ptr = self.as_cstr_ptr(msg_val)?;
                                self.call_printf(&[msg_ptr.into()])?;
                                // Print newline
                                let newline = self.builder.build_global_string_ptr("\n", "newline")?;
                                self.call_printf(&[newline.as_pointer_value().into()])?;
                            }
                            
                            let exit_fn = self.declare_c_void_fn(
                                "exit",
                                &[self.ctx.i32_type().into()],
                                false,
                            );
                            self.builder.build_call(exit_fn, &[self.ctx.i32_type().const_int(1, false).into()], "exit")?;
                            // self.builder.build_unreachable()?;
                            return Ok(());
                        }
                        "__ArrayNew" => {
                            // Create a new dynamic array with given capacity
                            // Array layout: { i64 len, i64 capacity, i8* data_ptr }
                            // args: [element_size, capacity]
                            
                            // Use capacity from second argument if available, otherwise first (legacy/fallback)
                            let cap_arg = if args.len() >= 2 {
                                &args[1]
                            } else {
                                args.get(0).ok_or_else(|| anyhow!("__ArrayNew requires arguments"))?
                            };

                            let capacity = self.eval_value(cap_arg, fn_map)?;
                            let cap_i64 = self.as_i64(capacity)?;

                                // Allocate header (24 bytes)
                                let header_size = self.i64_t.const_int(24, false);
                                let malloc = self.get_malloc();
                                let header_ptr = self.builder.build_call(malloc, &[header_size.into()], "arr_header_alloc")?
                                    .try_as_basic_value().left()
                                    .ok_or_else(|| anyhow!("malloc returned void"))?
                                    .into_pointer_value();
                                
                                // Allocate data buffer
                                let elem_size = self.i64_t.const_int(8, false);
                                let data_size = self.builder.build_int_mul(cap_i64, elem_size, "data_size")?;
                                let data_ptr = self.builder.build_call(malloc, &[data_size.into()], "arr_data_alloc")?
                                    .try_as_basic_value().left()
                                    .ok_or_else(|| anyhow!("malloc returned void"))?
                                    .into_pointer_value();

                                // Store length = 0
                                let len_ptr = self.builder.build_pointer_cast(
                                    header_ptr,
                                    self.i64_t.ptr_type(inkwell::AddressSpace::from(0u16)),
                                    "len_ptr"
                                )?;
                                self.builder.build_store(len_ptr, self.i64_t.const_zero())?;
                                
                                // Store capacity
                                let cap_ptr = unsafe {
                                    self.builder.build_gep(
                                        self.i64_t,
                                        len_ptr,
                                        &[self.i64_t.const_int(1, false)],
                                        "cap_ptr"
                                    )?
                                };
                                self.builder.build_store(cap_ptr, cap_i64)?;

                                // Store data pointer
                                let data_ptr_ptr = unsafe {
                                    self.builder.build_gep(
                                        self.i64_t,
                                        len_ptr,
                                        &[self.i64_t.const_int(2, false)],
                                        "data_ptr_ptr"
                                    )?
                                };
                                let data_ptr_ptr_casted = self.builder.build_pointer_cast(
                                    data_ptr_ptr,
                                    self.i8_ptr_t.ptr_type(inkwell::AddressSpace::from(0u16)),
                                    "data_ptr_ptr_casted"
                                )?;
                                self.builder.build_store(data_ptr_ptr_casted, data_ptr)?;
                                
                                if let Some(r) = result {
                                    self.assign_value(r, header_ptr.as_basic_value_enum())?;
                                }
                                return Ok(());
                        }
                        "push" => {
                            // push(array, value) - append value to dynamic array
                            if args.len() == 2 {
                                let arr_val = self.eval_value(&args[0], fn_map)?;
                                let value = self.eval_value(&args[1], fn_map)?;
                                
                                let arr_ptr = if arr_val.is_pointer_value() {
                                    arr_val.into_pointer_value()
                                } else {
                                    self.builder.build_int_to_ptr(
                                        arr_val.into_int_value(),
                                        self.i8_ptr_t,
                                        "arr_ptr"
                                    )?
                                };
                                
                                // Load current length
                                let len_ptr = self.builder.build_pointer_cast(
                                    arr_ptr,
                                    self.i64_t.ptr_type(inkwell::AddressSpace::from(0u16)),
                                    "len_ptr"
                                )?;
                                let len = self.builder.build_load(self.i64_t, len_ptr, "len")?.into_int_value();

                                // Load capacity
                                let cap_ptr = unsafe {
                                    self.builder.build_gep(
                                        self.i64_t,
                                        len_ptr,
                                        &[self.i64_t.const_int(1, false)],
                                        "cap_ptr"
                                    )?
                                };
                                let cap = self.builder.build_load(self.i64_t, cap_ptr, "cap")?.into_int_value();

                                // Load data pointer
                                let data_ptr_ptr = unsafe {
                                    self.builder.build_gep(
                                        self.i64_t,
                                        len_ptr,
                                        &[self.i64_t.const_int(2, false)],
                                        "data_ptr_ptr"
                                    )?
                                };
                                let data_ptr_ptr_casted = self.builder.build_pointer_cast(
                                    data_ptr_ptr,
                                    self.i8_ptr_t.ptr_type(inkwell::AddressSpace::from(0u16)),
                                    "data_ptr_ptr_casted"
                                )?;
                                let mut data_ptr = self.builder.build_load(self.i8_ptr_t, data_ptr_ptr_casted, "data_ptr")?.into_pointer_value();

                                // Check if resize needed
                                let needs_resize = self.builder.build_int_compare(inkwell::IntPredicate::EQ, len, cap, "needs_resize")?;
                                
                                let current_bb = self.builder.get_insert_block().unwrap();
                                let resize_bb = self.ctx.append_basic_block(self.current_fn.unwrap(), "resize");
                                let cont_bb = self.ctx.append_basic_block(self.current_fn.unwrap(), "push_cont");
                                
                                self.builder.build_conditional_branch(needs_resize, resize_bb, cont_bb)?;
                                
                                // Resize block
                                self.builder.position_at_end(resize_bb);
                                
                                // Handle cap=0 case
                                let cap_is_zero = self.builder.build_int_compare(inkwell::IntPredicate::EQ, cap, self.i64_t.const_zero(), "cap_is_zero")?;
                                let new_cap = self.builder.build_select(
                                    cap_is_zero,
                                    self.i64_t.const_int(4, false),
                                    self.builder.build_int_mul(cap, self.i64_t.const_int(2, false), "mul")?,
                                    "new_cap"
                                )?.into_int_value();

                                let elem_size = self.i64_t.const_int(8, false);
                                let new_size = self.builder.build_int_mul(new_cap, elem_size, "new_size")?;
                                
                                let realloc = self.get_realloc();
                                let new_data_ptr = self.builder.build_call(realloc, &[data_ptr.into(), new_size.into()], "realloc")?
                                    .try_as_basic_value().left()
                                    .ok_or_else(|| anyhow!("realloc returned void"))?
                                    .into_pointer_value();
                                
                                self.builder.build_store(cap_ptr, new_cap)?;
                                self.builder.build_store(data_ptr_ptr_casted, new_data_ptr)?;
                                self.builder.build_unconditional_branch(cont_bb)?;
                                
                                // Continue block
                                self.builder.position_at_end(cont_bb);
                                let phi = self.builder.build_phi(self.i8_ptr_t, "data_ptr_phi")?;
                                phi.add_incoming(&[(&data_ptr, current_bb), (&new_data_ptr, resize_bb)]);
                                data_ptr = phi.as_basic_value().into_pointer_value();
                                
                                // Handle different value types
                                if value.is_float_value() {
                                    // Float array
                                    let f64_ptr_ty = self.ctx.f64_type().ptr_type(inkwell::AddressSpace::from(0u16));
                                    let data_f64_ptr = self.builder.build_pointer_cast(data_ptr, f64_ptr_ty, "data_f64_ptr")?;
                                    let elem_ptr = unsafe {
                                        self.builder.build_gep(
                                            self.ctx.f64_type(),
                                            data_f64_ptr,
                                            &[len],
                                            "elem_ptr"
                                        )?
                                    };
                                    self.builder.build_store(elem_ptr, value.into_float_value())?;
                                } else if value.is_int_value() {
                                    // Could be integer or pointer-as-int (struct pointer)
                                    // Treat as pointer array (for struct arrays)
                                    let i64_ptr_ty = self.i64_t.ptr_type(inkwell::AddressSpace::from(0u16));
                                    let data_i64_ptr = self.builder.build_pointer_cast(data_ptr, i64_ptr_ty, "data_i64_ptr")?;
                                    let elem_ptr = unsafe {
                                        self.builder.build_gep(
                                            self.i64_t,
                                            data_i64_ptr,
                                            &[len],
                                            "elem_ptr"
                                        )?
                                    };
                                    let val_to_store = if value.into_int_value().get_type().get_bit_width() < 64 {
                                        self.builder.build_int_z_extend(value.into_int_value(), self.i64_t, "zext")?
                                    } else {
                                        value.into_int_value()
                                    };
                                    self.builder.build_store(elem_ptr, val_to_store)?;
                                } else if value.is_pointer_value() {
                                    // Pointer value (struct pointer)
                                    let ptr_ptr_ty = self.i8_ptr_t.ptr_type(inkwell::AddressSpace::from(0u16));
                                    let data_ptr_ptr = self.builder.build_pointer_cast(data_ptr, ptr_ptr_ty, "data_ptr_ptr")?;
                                    let elem_ptr = unsafe {
                                        self.builder.build_gep(
                                            self.i8_ptr_t,
                                            data_ptr_ptr,
                                            &[len],
                                            "elem_ptr"
                                        )?
                                    };
                                    self.builder.build_store(elem_ptr, value.into_pointer_value())?;
                                } else {
                                    return Err(anyhow!("push: unsupported value type"));
                                }
                                
                                // Increment length
                                let new_len = self.builder.build_int_add(len, self.i64_t.const_int(1, false), "new_len")?;
                                self.builder.build_store(len_ptr, new_len)?;
                                
                                if let Some(r) = result {
                                    self.assign_value(r, self.i64_t.const_zero().as_basic_value_enum())?;
                                }
                                return Ok(());
                            }
                        }
                        "__Print" => {
                            // Print a string
                            if let Some(arg0) = args.get(0) {
                                let val = self.eval_value(arg0, fn_map)?;
                                let s = self.as_cstr_ptr(val)?;
                                // Use printf with %s format
                                let fmt = self.builder.build_global_string_ptr("%s", "fmt_str")?;
                                self.call_printf(&[fmt.as_pointer_value().into(), s.into()])?;
                                if let Some(r) = result {
                                    self.assign_value(r, self.i64_t.const_zero().as_basic_value_enum())?;
                                }
                                return Ok(());
                            }
                        }
                        "__PrintInt" => {
                            // Print an integer
                            if let Some(arg0) = args.get(0) {
                                let val = self.eval_value(arg0, fn_map)?;
                                let int_val = self.as_i64(val)?;
                                let fmt = self.builder.build_global_string_ptr("%ld", "fmt_int")?;
                                self.call_printf(&[fmt.as_pointer_value().into(), int_val.into()])?;
                                if let Some(r) = result {
                                    self.assign_value(r, self.i64_t.const_zero().as_basic_value_enum())?;
                                }
                                return Ok(());
                            }
                        }
                        "__GetTime" => {
                            // Get current time in seconds as float (high-resolution timer)
                            // Use clock_gettime(CLOCK_MONOTONIC) for precise timing
                            let clock_gettime = self.get_or_declare_clock_gettime();
                            
                            // Allocate timespec struct on stack: { i64 tv_sec, i64 tv_nsec }
                            let timespec_ty = self.ctx.struct_type(&[
                                self.i64_t.into(),
                                self.i64_t.into(),
                            ], false);
                            let timespec_ptr = self.builder.build_alloca(timespec_ty, "timespec")?;
                            
                            // CLOCK_MONOTONIC = 1
                            let clock_id = self.ctx.i32_type().const_int(1, false);
                            self.builder.build_call(
                                clock_gettime,
                                &[clock_id.into(), timespec_ptr.into()],
                                "gettime_call"
                            )?;
                            
                            // Load tv_sec and tv_nsec
                            let sec_ptr = self.builder.build_struct_gep(timespec_ty, timespec_ptr, 0, "sec_ptr")?;
                            let nsec_ptr = self.builder.build_struct_gep(timespec_ty, timespec_ptr, 1, "nsec_ptr")?;
                            let tv_sec = self.builder.build_load(self.i64_t, sec_ptr, "tv_sec")?.into_int_value();
                            let tv_nsec = self.builder.build_load(self.i64_t, nsec_ptr, "tv_nsec")?.into_int_value();
                            
                            // Convert to float: sec + nsec * 1e-9
                            let sec_f = self.builder.build_signed_int_to_float(tv_sec, self.ctx.f64_type(), "sec_f")?;
                            let nsec_f = self.builder.build_signed_int_to_float(tv_nsec, self.ctx.f64_type(), "nsec_f")?;
                            let nano = self.ctx.f64_type().const_float(1e-9);
                            let nsec_sec = self.builder.build_float_mul(nsec_f, nano, "nsec_sec")?;
                            let time_f = self.builder.build_float_add(sec_f, nsec_sec, "time_f")?;
                            
                            if let Some(r) = result {
                                self.assign_value(r, time_f.as_basic_value_enum())?;
                            }
                            return Ok(());
                        }
                        "__PrintFloat" => {
                            if let Some(arg0) = args.get(0) {
                                let val = self.eval_value(arg0, fn_map)?;
                                let float_val = if val.is_float_value() {
                                    val.into_float_value()
                                } else if val.is_int_value() {
                                    self.builder.build_signed_int_to_float(
                                        val.into_int_value(),
                                        self.ctx.f64_type(),
                                        "i2f"
                                    )?
                                } else if val.is_pointer_value() {
                                    let int_val = self.builder.build_ptr_to_int(
                                        val.into_pointer_value(),
                                        self.i64_t,
                                        "ptr2i"
                                    )?;
                                    self.builder.build_signed_int_to_float(
                                        int_val,
                                        self.ctx.f64_type(),
                                        "i2f"
                                    )?
                                } else {
                                    return Err(anyhow!("__PrintFloat: unsupported value type"));
                                };
                                let fmt = self.builder.build_global_string_ptr("%f\n", "fmt_float")?;
                                self.call_printf(&[fmt.as_pointer_value().into(), float_val.into()])?;
                                if let Some(r) = result {
                                    self.assign_value(
                                        r,
                                        self.i64_t.const_zero().as_basic_value_enum(),
                                    )?;
                                }
                                return Ok(());
                            }
                        }
                        "println" => {
                            if let Some(arg0) = args.get(0) {
                                let a0 = self.eval_value(arg0, fn_map)?;
                                let s = self.as_cstr_ptr(a0)?;
                                self.call_printf(&[s.into()])?;
                                if let Some(r) = result {
                                    self.assign_value(
                                        r,
                                        self.i64_t.const_zero().as_basic_value_enum(),
                                    )?;
                                }
                                return Ok(());
                            }
                        }
                        "endsWith" => {
                            // endsWith(string, suffix) -> bool
                            if args.len() == 2 {
                                let s_val = self.eval_value(&args[0], fn_map)?;
                                let suf_val = self.eval_value(&args[1], fn_map)?;
                                let s = self.as_cstr_ptr(s_val)?;
                                let suf = self.as_cstr_ptr(suf_val)?;
                                let res = self.runtime_endswith(s, suf)?;
                                if let Some(r) = result {
                                    self.assign_value(r, res.as_basic_value_enum())?;
                                }
                                return Ok(());
                            }
                        }
                        "substring" => {
                            // substring(string, start, end) -> string
                            if args.len() == 3 {
                                let s_val = self.eval_value(&args[0], fn_map)?;
                                let s = self.as_cstr_ptr(s_val)?;
                                let start_v = self.eval_value(&args[1], fn_map)?;
                                let start = self.as_i64(start_v)?;
                                let end_v = self.eval_value(&args[2], fn_map)?;
                                let end = self.as_i64(end_v)?;
                                let res = self.runtime_substring(s, start, end)?;
                                if let Some(r) = result {
                                    self.assign_value(r, res.as_basic_value_enum())?;
                                }
                                return Ok(());
                            }
                        }
                        "__GetCommandLineArgs" => {
                            // Build StrArray* from @__argc/@__argv
                            let (_g_argc, _g_argv) = self.ensure_arg_globals();
                            let argc = self.builder.build_load(
                                self.ctx.i32_type(),
                                self.g_argc.unwrap().as_pointer_value(),
                                "argc",
                            )?;
                            let argv = self.builder.build_load(
                                self.i8_ptr_t.ptr_type(inkwell::AddressSpace::from(0u16)),
                                self.g_argv.unwrap().as_pointer_value(),
                                "argv",
                            )?;
                            let ty = self.ty_str_array();
                            let sizeof = ty
                                .size_of()
                                .ok_or_else(|| anyhow!("Failed to compute StrArray size"))?;
                            let size64 = if sizeof.get_type() == self.i64_t {
                                sizeof
                            } else {
                                self.builder
                                    .build_int_z_extend(sizeof, self.i64_t, "strarray_size")
                                    .map_err(|e| anyhow!("{e:?}"))?
                            };
                            let malloc = self.get_malloc();
                            let arr_call = self.builder.build_call(
                                malloc,
                                &[size64.into()],
                                "malloc_strarray",
                            )?;
                            let raw_ptr = arr_call
                                .try_as_basic_value()
                                .left()
                                .ok_or_else(|| anyhow!("malloc returned void"))?
                                .into_pointer_value();
                            let arr_ptr = self
                                .builder
                                .build_pointer_cast(
                                    raw_ptr,
                                    ty.ptr_type(inkwell::AddressSpace::from(0u16)),
                                    "strarray_cast",
                                )
                                .map_err(|e| anyhow!("{e:?}"))?;
                            // store len and data
                            let len64 = self.builder.build_int_z_extend(
                                argc.into_int_value(),
                                self.i64_t,
                                "argc64",
                            )?;
                            let len_ptr = self.builder.build_struct_gep(ty, arr_ptr, 0, "lenp")?;
                            self.builder.build_store(len_ptr, len64)?;
                            let data_ptr =
                                self.builder.build_struct_gep(ty, arr_ptr, 1, "datap")?;
                            self.builder.build_store(data_ptr, argv)?;
                            if let Some(r) = result {
                                let arr_int = self
                                    .builder
                                    .build_ptr_to_int(arr_ptr, self.i64_t, "strarray_int")
                                    .map_err(|e| anyhow!("{e:?}"))?
                                    .as_basic_value_enum();
                                self.assign_value(r, arr_int)?;
                            }
                            return Ok(());
                        }
                        "__GetTimestamp" => {
                            // time(NULL) -> seconds -> sprintf into buffer and return char*
                            let time_t = self.i64_t; // treat as i64
                            let time_fn = self.declare_c_fn(
                                "time",
                                time_t.into(),
                                &[self.i8_ptr_t.into()],
                                false,
                            );
                            let null = self.i8_ptr_t.const_null();
                            let secs = self.builder.build_call(time_fn, &[null.into()], "time")?;
                            let secs_val = secs.try_as_basic_value().left().unwrap();
                            // allocate buffer 32 bytes
                            let malloc = self.get_malloc();
                            let sz = self.i64_t.const_int(32, false);
                            let buf = self.builder.build_call(malloc, &[sz.into()], "malloc_ts")?;
                            let bufp = buf
                                .try_as_basic_value()
                                .left()
                                .unwrap()
                                .into_pointer_value();
                            // sprintf(buf, "%lld", secs)
                            let sprintf = self.declare_c_fn(
                                "sprintf",
                                self.i64_t.into(),
                                &[
                                    self.i8_ptr_t.into(),
                                    self.i8_ptr_t.into(),
                                    self.i64_t.into(),
                                ],
                                true,
                            );
                            let fmt = self.builder.build_global_string_ptr("%lld", "fmt_ts")?;
                            let _ = self.builder.build_call(
                                sprintf,
                                &[bufp.into(), fmt.as_pointer_value().into(), secs_val.into()],
                                "sprintf_ts",
                            );
                            if let Some(r) = result {
                                self.assign_value(r, bufp.as_basic_value_enum())?;
                            }
                            return Ok(());
                        }
                        "__IntToString" => {
                            // Convert integer to string using sprintf
                            if let Some(arg) = args.get(0) {
                                let val = self.eval_value(arg, fn_map)?;
                                let int_val = self.as_i64(val)?;
                                
                                // Allocate buffer (32 bytes for largest i64)
                                let malloc = self.get_malloc();
                                let sz = self.i64_t.const_int(32, false);
                                let buf = self.builder.build_call(malloc, &[sz.into()], "malloc_inttostr")?;
                                let bufp = buf
                                    .try_as_basic_value()
                                    .left()
                                    .unwrap()
                                    .into_pointer_value();
                                
                                // sprintf(buf, "%lld", int_val)
                                let sprintf = self.declare_c_fn(
                                    "sprintf",
                                    self.ctx.i32_type().into(),
                                    &[
                                        self.i8_ptr_t.into(),
                                        self.i8_ptr_t.into(),
                                    ],
                                    true,
                                );
                                let fmt = self.builder.build_global_string_ptr("%lld", "fmt_inttostr")?;
                                let _ = self.builder.build_call(
                                    sprintf,
                                    &[bufp.into(), fmt.as_pointer_value().into(), int_val.into()],
                                    "sprintf_inttostr",
                                );
                                
                                if let Some(r) = result {
                                    self.assign_value(r, bufp.as_basic_value_enum())?;
                                }
                            }
                            return Ok(());
                        }
                        "__FloatToString" => {
                            // Convert float to string using sprintf
                            if let Some(arg) = args.get(0) {
                                let val = self.eval_value(arg, fn_map)?;
                                let float_val = if val.is_float_value() {
                                    val.into_float_value()
                                } else if val.is_int_value() {
                                    self.builder.build_signed_int_to_float(
                                        val.into_int_value(),
                                        self.ctx.f64_type(),
                                        "i2f_fts",
                                    )?
                                } else {
                                    return Err(anyhow!("__FloatToString requires numeric argument"));
                                };
                                
                                // Allocate buffer (64 bytes for float formatting)
                                let malloc = self.get_malloc();
                                let sz = self.i64_t.const_int(64, false);
                                let buf = self.builder.build_call(malloc, &[sz.into()], "malloc_floattostr")?;
                                let bufp = buf
                                    .try_as_basic_value()
                                    .left()
                                    .unwrap()
                                    .into_pointer_value();
                                
                                // sprintf(buf, "%g", float_val)
                                let sprintf = self.declare_c_fn(
                                    "sprintf",
                                    self.ctx.i32_type().into(),
                                    &[
                                        self.i8_ptr_t.into(),
                                        self.i8_ptr_t.into(),
                                    ],
                                    true,
                                );
                                let fmt = self.builder.build_global_string_ptr("%g", "fmt_floattostr")?;
                                let _ = self.builder.build_call(
                                    sprintf,
                                    &[bufp.into(), fmt.as_pointer_value().into(), float_val.into()],
                                    "sprintf_floattostr",
                                );
                                
                                if let Some(r) = result {
                                    self.assign_value(r, bufp.as_basic_value_enum())?;
                                }
                            }
                            return Ok(());
                        }
                        "__BoolToString" => {
                            // Convert bool to "true" or "false"
                            if let Some(arg) = args.get(0) {
                                let val = self.eval_value(arg, fn_map)?;
                                let bool_val = self.as_i64(val)?;
                                
                                // Compare to 0
                                let is_true = self.builder.build_int_compare(
                                    inkwell::IntPredicate::NE,
                                    bool_val,
                                    self.i64_t.const_zero(),
                                    "is_true",
                                )?;
                                
                                // Create global strings for "true" and "false"
                                let true_str = self.builder.build_global_string_ptr("true", "str_true")?;
                                let false_str = self.builder.build_global_string_ptr("false", "str_false")?;
                                
                                // Select based on condition
                                let result_ptr = self.builder.build_select(
                                    is_true,
                                    true_str.as_pointer_value(),
                                    false_str.as_pointer_value(),
                                    "bool_str",
                                )?;
                                
                                if let Some(r) = result {
                                    self.assign_value(r, result_ptr)?;
                                }
                            }
                            return Ok(());
                        }
                        "__Sqrt" => {
                            // Call llvm.sqrt.f64 intrinsic
                            if let Some(arg) = args.get(0) {
                                let val = self.eval_value(arg, fn_map)?;
                                let f64_val = if val.is_float_value() {
                                    val.into_float_value()
                                } else if val.is_int_value() {
                                    self.builder.build_signed_int_to_float(
                                        val.into_int_value(),
                                        self.ctx.f64_type(),
                                        "i2f_sqrt",
                                    )?
                                } else {
                                    return Err(anyhow!("__Sqrt requires a numeric argument"));
                                };
                                
                                // Use LLVM sqrt intrinsic for optimal codegen
                                let sqrt_intrinsic = inkwell::intrinsics::Intrinsic::find("llvm.sqrt.f64")
                                    .ok_or_else(|| anyhow!("Failed to find llvm.sqrt.f64 intrinsic"))?;
                                let sqrt_fn = sqrt_intrinsic.get_declaration(&self.module, &[self.ctx.f64_type().into()])
                                    .ok_or_else(|| anyhow!("Failed to get sqrt declaration"))?;
                                
                                let sqrt_result = self.builder.build_call(
                                    sqrt_fn,
                                    &[f64_val.into()],
                                    "sqrt_result",
                                )?;
                                
                                if let Some(r) = result {
                                    if let Some(ret) = sqrt_result.try_as_basic_value().left() {
                                        self.assign_value(r, ret)?;
                                    }
                                }
                            }
                            return Ok(());
                        }
                        "toFloat" | "__toFloat" => {
                            // Convert Int to Float
                            if let Some(arg) = args.get(0) {
                                let val = self.eval_value(arg, fn_map)?;
                                let f64_val = if val.is_float_value() {
                                    val.into_float_value()
                                } else if val.is_int_value() {
                                    self.builder.build_signed_int_to_float(
                                        val.into_int_value(),
                                        self.ctx.f64_type(),
                                        "i2f_toFloat",
                                    )?
                                } else if val.is_pointer_value() {
                                    let int_val = self.builder.build_ptr_to_int(
                                        val.into_pointer_value(),
                                        self.i64_t,
                                        "ptr2i_toFloat",
                                    )?;
                                    self.builder.build_signed_int_to_float(
                                        int_val,
                                        self.ctx.f64_type(),
                                        "i2f_toFloat",
                                    )?
                                } else {
                                    return Err(anyhow!("toFloat requires a numeric argument"));
                                };
                                
                                if let Some(r) = result {
                                    self.assign_value(r, f64_val.as_basic_value_enum())?;
                                }
                            }
                            return Ok(());
                        }
                        "__ReadFile" => {
                            // FILE* f = fopen(path, "rb"); if !f return ""
                            let fnty = self.i8_ptr_t; // FILE* opaque as i8*
                            let fopen = self.declare_c_fn(
                                "fopen",
                                fnty.into(),
                                &[self.i8_ptr_t.into(), self.i8_ptr_t.into()],
                                false,
                            );
                            let fseek = self.declare_c_fn(
                                "fseek",
                                self.ctx.i32_type().into(),
                                &[fnty.into(), self.i64_t.into(), self.ctx.i32_type().into()],
                                false,
                            );
                            let ftell = self.declare_c_fn(
                                "ftell",
                                self.i64_t.into(),
                                &[fnty.into()],
                                false,
                            );
                            let rewindf = self.declare_c_void_fn("rewind", &[fnty.into()], false);
                            let fread = self.declare_c_fn(
                                "fread",
                                self.i64_t.into(),
                                &[
                                    self.i8_ptr_t.into(),
                                    self.i64_t.into(),
                                    self.i64_t.into(),
                                    fnty.into(),
                                ],
                                false,
                            );
                            let fclose = self.declare_c_fn(
                                "fclose",
                                self.ctx.i32_type().into(),
                                &[fnty.into()],
                                false,
                            );
                            let path_v = self.eval_value(&args[0], fn_map)?;
                            let path = self.as_cstr_ptr(path_v)?;
                            let mode = self.builder.build_global_string_ptr("rb", "rb")?;
                            let f = self.builder.build_call(
                                fopen,
                                &[path.into(), mode.as_pointer_value().into()],
                                "fopen",
                            )?;
                            let fval = f.try_as_basic_value().left().unwrap();
                            let is_null = self
                                .builder
                                .build_is_null(fval.into_pointer_value(), "isnull")?;
                            let fnv = self.current_fn.unwrap();
                            let then_bb = self.ctx.append_basic_block(fnv, "rf_null");
                            let cont_bb = self.ctx.append_basic_block(fnv, "rf_cont");
                            let done_bb = self.ctx.append_basic_block(fnv, "rf_done");
                            self.builder
                                .build_conditional_branch(is_null, then_bb, cont_bb)?;
                            let mut rf_then_val: Option<(
                                BasicValueEnum<'ctx>,
                                LlvmBasicBlock<'ctx>,
                            )> = None;
                            let mut rf_cont_val: Option<(
                                BasicValueEnum<'ctx>,
                                LlvmBasicBlock<'ctx>,
                            )> = None;
                            self.builder.position_at_end(then_bb);
                            let empty = self.builder.build_global_string_ptr("", "empty")?;
                            let empty_val = empty.as_pointer_value().as_basic_value_enum();
                            rf_then_val = Some((empty_val, then_bb));
                            self.builder.build_unconditional_branch(done_bb)?;
                            self.builder.position_at_end(cont_bb);
                            // size
                            let seek_end = self.ctx.i32_type().const_int(2, false);
                            self.builder.build_call(
                                fseek,
                                &[fval.into(), self.i64_t.const_zero().into(), seek_end.into()],
                                "fseek_end",
                            )?;
                            let sz = self.builder.build_call(ftell, &[fval.into()], "ftell")?;
                            let szv = sz.try_as_basic_value().left().unwrap().into_int_value();
                            let _ = self.builder.build_call(rewindf, &[fval.into()], "rewind")?;
                            let malloc = self.get_malloc();
                            let one = self.i64_t.const_int(1, false);
                            let total = self.builder.build_int_add(szv, one, "tot")?;
                            let buf =
                                self.builder
                                    .build_call(malloc, &[total.into()], "malloc_rf")?;
                            let bufp = buf
                                .try_as_basic_value()
                                .left()
                                .unwrap()
                                .into_pointer_value();
                            let _rd = self.builder.build_call(
                                fread,
                                &[bufp.into(), one.into(), szv.into(), fval.into()],
                                "fread",
                            )?;
                            let endp = unsafe {
                                self.builder
                                    .build_gep(self.ctx.i8_type(), bufp, &[szv], "end")?
                            };
                            self.builder
                                .build_store(endp, self.ctx.i8_type().const_zero())?;
                            let _ = self.builder.build_call(fclose, &[fval.into()], "fclose")?;
                            let buf_val = bufp.as_basic_value_enum();
                            rf_cont_val = Some((buf_val, cont_bb));
                            self.builder.build_unconditional_branch(done_bb)?;
                            self.builder.position_at_end(done_bb);
                            if let Some(r) = result {
                                let phi = self.builder.build_phi(self.i8_ptr_t, "rf_value")?;
                                if let Some((val, bb)) = rf_then_val {
                                    phi.add_incoming(&[(&val, bb)]);
                                }
                                if let Some((val, bb)) = rf_cont_val {
                                    phi.add_incoming(&[(&val, bb)]);
                                }
                                self.assign_value(r, phi.as_basic_value())?;
                            }
                            return Ok(());
                        }
                        "__WriteFile" => {
                            let fnty = self.i8_ptr_t;
                            let fopen = self.declare_c_fn(
                                "fopen",
                                fnty.into(),
                                &[self.i8_ptr_t.into(), self.i8_ptr_t.into()],
                                false,
                            );
                            let fwrite = self.declare_c_fn(
                                "fwrite",
                                self.i64_t.into(),
                                &[
                                    self.i8_ptr_t.into(),
                                    self.i64_t.into(),
                                    self.i64_t.into(),
                                    fnty.into(),
                                ],
                                false,
                            );
                            let fclose = self.declare_c_fn(
                                "fclose",
                                self.ctx.i32_type().into(),
                                &[fnty.into()],
                                false,
                            );
                            let strlen = self.get_strlen();
                            let path_v = self.eval_value(&args[0], fn_map)?;
                            let path = self.as_cstr_ptr(path_v)?;
                            let content_v = self.eval_value(&args[1], fn_map)?;
                            let content = self.as_cstr_ptr(content_v)?;
                            let mode = self.builder.build_global_string_ptr("wb", "wb")?;
                            let f = self.builder.build_call(
                                fopen,
                                &[path.into(), mode.as_pointer_value().into()],
                                "fopen_w",
                            )?;
                            let fval = f.try_as_basic_value().left().unwrap();
                            let is_null = self
                                .builder
                                .build_is_null(fval.into_pointer_value(), "isnullw")?;
                            let len =
                                self.builder
                                    .build_call(strlen, &[content.into()], "strlen")?;
                            let lenv = len.try_as_basic_value().left().unwrap().into_int_value();
                            let one = self.i64_t.const_int(1, false);
                            let _wr = self.builder.build_call(
                                fwrite,
                                &[content.into(), one.into(), lenv.into(), fval.into()],
                                "fwrite",
                            )?;
                            let _ = self
                                .builder
                                .build_call(fclose, &[fval.into()], "fclose_w")?;
                            if let Some(r) = result {
                                self.assign_value(
                                    r,
                                    self.bool_t.const_int(1, false).as_basic_value_enum(),
                                )?;
                            }
                            return Ok(());
                        }
                        "__CreateDirectory" => {
                            let mkdir = self.declare_c_fn(
                                "mkdir",
                                self.ctx.i32_type().into(),
                                &[self.i8_ptr_t.into(), self.ctx.i32_type().into()],
                                false,
                            );
                            let path_v = self.eval_value(&args[0], fn_map)?;
                            let path = self.as_cstr_ptr(path_v)?;
                            let mode = self.ctx.i32_type().const_int(0o755, false);
                            let rc = self.builder.build_call(
                                mkdir,
                                &[path.into(), mode.into()],
                                "mkdir",
                            )?;
                            let ok = self.builder.build_int_compare(
                                inkwell::IntPredicate::EQ,
                                rc.try_as_basic_value().left().unwrap().into_int_value(),
                                self.ctx.i32_type().const_zero(),
                                "ok",
                            )?;
                            if let Some(r) = result {
                                self.assign_value(r, ok.as_basic_value_enum())?;
                            }
                            return Ok(());
                        }
                        "__DeleteFile" => {
                            let rm = self.declare_c_fn(
                                "remove",
                                self.ctx.i32_type().into(),
                                &[self.i8_ptr_t.into()],
                                false,
                            );
                            let path_v = self.eval_value(&args[0], fn_map)?;
                            let path = self.as_cstr_ptr(path_v)?;
                            let rc = self.builder.build_call(rm, &[path.into()], "rm")?;
                            let ok = self.builder.build_int_compare(
                                inkwell::IntPredicate::EQ,
                                rc.try_as_basic_value().left().unwrap().into_int_value(),
                                self.ctx.i32_type().const_zero(),
                                "ok",
                            )?;
                            if let Some(r) = result {
                                self.assign_value(r, ok.as_basic_value_enum())?;
                            }
                            return Ok(());
                        }
                        "__ExecuteProgram" => {
                            let system = self.declare_c_fn(
                                "system",
                                self.ctx.i32_type().into(),
                                &[self.i8_ptr_t.into()],
                                false,
                            );
                            let path_v = self.eval_value(&args[0], fn_map)?;
                            let path = self.as_cstr_ptr(path_v)?;
                            let rc = self.builder.build_call(system, &[path.into()], "system")?;
                            let rcv = rc.try_as_basic_value().left().unwrap().into_int_value();
                            let r64 = self.builder.build_int_s_extend(rcv, self.i64_t, "rc64")?;
                            if let Some(r) = result {
                                self.assign_value(r, r64.as_basic_value_enum())?;
                            }
                            return Ok(());
                        }
                        "__ExecuteCommand" => {
                            // Use system() to execute, cannot capture output: set success = rc==0, output = ""
                            let system = self.declare_c_fn(
                                "system",
                                self.ctx.i32_type().into(),
                                &[self.i8_ptr_t.into()],
                                false,
                            );
                            let cmd_v = self.eval_value(&args[0], fn_map)?;
                            let cmd = self.as_cstr_ptr(cmd_v)?;
                            let rc = self.builder.build_call(system, &[cmd.into()], "system")?;
                            let rcv = rc.try_as_basic_value().left().unwrap().into_int_value();
                            let ok = self.builder.build_int_compare(
                                inkwell::IntPredicate::EQ,
                                rcv,
                                self.ctx.i32_type().const_zero(),
                                "ok",
                            )?;
                            let ty = self.ty_cmd_result();
                            let malloc = self.get_malloc();
                            let bytes = self.i64_t.const_int(16, false);
                            let buf = self.builder.build_call(
                                malloc,
                                &[bytes.into()],
                                "malloc_cmdres",
                            )?;
                            let p = buf
                                .try_as_basic_value()
                                .left()
                                .unwrap()
                                .into_pointer_value();
                            let cast = self.builder.build_pointer_cast(
                                p,
                                ty.ptr_type(inkwell::AddressSpace::from(0u16)),
                                "cmdres",
                            )?;
                            let sp = self.builder.build_struct_gep(ty, cast, 0, "succp")?;
                            self.builder.build_store(sp, ok)?;
                            let op = self.builder.build_struct_gep(ty, cast, 1, "outp")?;
                            let empty = self.builder.build_global_string_ptr("", "empty_out")?;
                            self.builder.build_store(op, empty.as_pointer_value())?;
                            if let Some(r) = result {
                                self.assign_value(r, cast.as_basic_value_enum())?;
                            }
                            return Ok(());
                        }
                        "__FormatSeenCode" => {
                            // Identity: return input
                            if let Some(arg0) = args.get(0) {
                                let s = self.eval_value(arg0, fn_map)?;
                                if let Some(r) = result {
                                    self.assign_value(r, s)?;
                                }
                            }
                            return Ok(());
                        }
                        _ => {}
                    }
                }

                if let IRValue::Function { name, .. } = target {
                    match name.as_str() {
                        "__channel_send_future" => {
                            if args.len() >= 2 {
                                let chan_val = self.eval_value(&args[0], fn_map)?;
                                let chan_ptr = self.to_i8_ptr(chan_val, "send_chan")?;
                                let msg_val = self.eval_value(&args[1], fn_map)?;
                                let boxed = self.box_runtime_value(msg_val)?;
                                let send_fn = self.ensure_channel_send_fn();
                                self.builder
                                    .build_call(
                                        send_fn,
                                        &[
                                            chan_ptr.as_basic_value_enum().into(),
                                            boxed.as_basic_value_enum().into(),
                                        ],
                                        "channel_send",
                                    )
                                    .map_err(|e| anyhow!("{e:?}"))?;
                            }
                            let handle_fn = self.ensure_task_handle_new_fn();
                            let kind = self.ctx.i32_type().const_int(3, false);
                            let call = self.builder.build_call(
                                handle_fn,
                                &[kind.into()],
                                "channel_future_handle",
                            )?;
                            if let Some(r) = result {
                                if let Some(val) = call.try_as_basic_value().left() {
                                    self.assign_value(r, val)?;
                                }
                            }
                            return Ok(());
                        }
                        "__spawn_task" | "__spawn_detached" | "__spawn_actor" => {
                            if let Some(arg0) = args.get(0) {
                                let _ = self.eval_value(arg0, fn_map)?;
                            }
                            let spawn_fn = self.ensure_spawn_fn(name.as_str());
                            let call =
                                self.builder
                                    .build_call(spawn_fn, &[], "spawn_handle_call")?;
                            if let Some(r) = result {
                                if let Some(val) = call.try_as_basic_value().left() {
                                    self.assign_value(r, val)?;
                                }
                            }
                            return Ok(());
                        }
                        "__await" => {
                            if let Some(arg0) = args.get(0) {
                                let handle_val = self.eval_value(arg0, fn_map)?;
                                let handle_ptr =
                                    self.cast_handle_ptr(handle_val, "await_handle_ptr")?;
                                let await_fn = self.ensure_await_fn();
                                let call = self.builder.build_call(
                                    await_fn,
                                    &[handle_ptr.as_basic_value_enum().into()],
                                    "await_call",
                                )?;
                                if let Some(r) = result {
                                    if let Some(val) = call.try_as_basic_value().left() {
                                        let ok = self
                                            .builder
                                            .build_int_compare(
                                                inkwell::IntPredicate::NE,
                                                val.into_int_value(),
                                                self.ctx.i32_type().const_zero(),
                                                "await_ok",
                                            )
                                            .map_err(|e| anyhow!("{e:?}"))?;
                                        self.assign_value(r, ok.as_basic_value_enum())?;
                                    }
                                }
                            }
                            return Ok(());
                        }
                        "__scope_push" | "__scope_pop" => {
                            let scope_fn = self.ensure_scope_fn(name.as_str());
                            let kind = args
                                .get(0)
                                .and_then(|v| {
                                    if let IRValue::Integer(k) = v {
                                        Some(*k as u64)
                                    } else {
                                        None
                                    }
                                })
                                .unwrap_or(0);
                            let kind_const = self.ctx.i32_type().const_int(kind, false);
                            self.builder.build_call(
                                scope_fn,
                                &[kind_const.into()],
                                "scope_call",
                            )?;
                            return Ok(());
                        }
                        _ => {}
                    }
                }

                // Handle Vec methods specially to convert float<->i64
                let func_name = match target {
                    IRValue::Variable(name) => Some(name.clone()),
                    IRValue::Function { name, .. } => Some(name.clone()),
                    _ => None,
                };
                
                let is_vec_push = func_name.as_deref() == Some("Vec_push");
                let is_vec_get = func_name.as_deref() == Some("Vec_get");
                let is_vec_set = func_name.as_deref() == Some("Vec_set");
                
                // Track which Vec variables store floats (from push calls)
                let mut is_float_vec_call = false;
                if is_vec_push || is_vec_set {
                    // Check if the value being pushed/set is a float
                    let value_arg = if is_vec_push { args.get(1) } else { args.get(2) };
                    if let Some(v) = value_arg {
                        let val = self.eval_value(v, fn_map)?;
                        if val.is_float_value() {
                            is_float_vec_call = true;
                            // Track the Vec variable as storing floats
                            if let Some(IRValue::Variable(vec_var)) = args.get(0) {
                                self.var_is_float_vec.insert(vec_var.clone());
                            }
                        }
                    }
                }
                
                // Check if this is a get from a float Vec
                if is_vec_get {
                    if let Some(IRValue::Variable(vec_var)) = args.get(0) {
                        if self.var_is_float_vec.contains(vec_var) {
                            is_float_vec_call = true;
                        }
                    }
                }

                // Normal call by name
                let f_opt = match target {
                    IRValue::Variable(name) => fn_map.get(name).cloned(),
                    IRValue::Function { name, .. } => fn_map.get(name).cloned(),
                    _ => None,
                };
                let f = match f_opt {
                    Some(func) => func,
                    None => return Err(anyhow!("Unknown call target {:?}", target)),
                };

                let mut call_args: Vec<BasicMetadataValueEnum> = Vec::new();
                for (i, a) in args.iter().enumerate() {
                    let v = self.eval_value(a, fn_map)?;
                    // For Vec_push and Vec_set, convert non-i64 values to i64
                    let should_convert = (is_vec_push && i == 1) || (is_vec_set && i == 2);
                    if should_convert {
                        if v.is_float_value() {
                            // Float: bitcast to i64 to preserve bits
                            let f64_val = v.into_float_value();
                            let as_i64 = self.builder.build_bit_cast(f64_val, self.i64_t, "f2i_bitcast")?.into_int_value();
                            call_args.push(as_i64.into());
                        } else if v.is_pointer_value() {
                            // Pointer: convert to i64
                            let ptr_val = v.into_pointer_value();
                            let as_i64 = self.builder.build_ptr_to_int(ptr_val, self.i64_t, "ptr2i_vec")?;
                            call_args.push(as_i64.into());
                        } else {
                            call_args.push(v.into());
                        }
                    } else {
                        call_args.push(v.into());
                    }
                }
                let call = self.builder.build_call(f, &call_args, "call")?;
                if let Some(r) = result {
                    if let Some(ret) = call.try_as_basic_value().left() {
                        // For Vec_get from float Vec, DON'T bitcast here - keep as i64
                        // but mark the register as containing float bits
                        if is_vec_get && is_float_vec_call {
                            if let IRValue::Register(reg_id) = r {
                                self.reg_is_float.insert(*reg_id);
                            }
                        }
                        self.assign_value(r, ret)?;

                        // Propagate return struct type info
                        let func_name = match target {
                            IRValue::Variable(name) => Some(name),
                            IRValue::Function { name, .. } => Some(name),
                            _ => None,
                        };
                        
                        if let Some(name) = func_name {
                            if let Some(struct_name) = self.fn_return_struct_types.get(name) {
                                if let IRValue::Register(reg_id) = r {
                                    self.reg_struct_types.insert(*reg_id, struct_name.clone());
                                }
                            }
                        }
                    }
                }
            }
            Instruction::Print(v) => {
                let vval = self.eval_value(v, fn_map)?;
                let s = self.as_cstr_ptr(vval)?;
                self.call_printf(&[s.into()])?;
            }
            Instruction::FieldAccess {
                struct_val,
                field,
                result,
            } => {
                let sv = self.eval_value(struct_val, fn_map)?;
                
                // Try to determine the struct type from the variable name or register
                let struct_type_name = match struct_val {
                    IRValue::Variable(var_name) => {
                        self.var_struct_types.get(var_name).cloned()
                    }
                    IRValue::Register(reg_id) => {
                        self.reg_struct_types.get(reg_id).cloned()
                    }
                    _ => None
                };
                
                // Check if we have a registered struct type
                if let Some(type_name) = struct_type_name {
                    if let Some((llvm_struct_ty, field_names)) = self.struct_types.get(&type_name).cloned() {
                        // Find field index
                        let field_idx = field_names.iter().position(|n| n == field)
                            .ok_or_else(|| anyhow!("Field '{}' not found in struct '{}'", field, type_name))?;
                        
                        let ptr = if sv.is_pointer_value() {
                            sv.into_pointer_value()
                        } else if sv.is_int_value() {
                            self.builder
                                .build_int_to_ptr(
                                    sv.into_int_value(),
                                    llvm_struct_ty.ptr_type(inkwell::AddressSpace::from(0u16)),
                                    "struct_field_ptr",
                                )
                                .map_err(|e| anyhow!("{e:?}"))?
                        } else if sv.is_struct_value() {
                            let tmp = self.alloca_for_type(
                                sv.into_struct_value().get_type().as_basic_type_enum(),
                                "struct_field_stack",
                            )?;
                            self.builder.build_store(tmp, sv)?;
                            tmp
                        } else {
                            return Err(anyhow!("Unsupported field access value for {:?}", struct_val));
                        };
                        
                        // Cast to struct pointer to ensure GEP works correctly
                        let struct_ptr = self.builder.build_pointer_cast(
                            ptr,
                            llvm_struct_ty.ptr_type(inkwell::AddressSpace::from(0u16)),
                            "struct_ptr_cast"
                        ).map_err(|e| anyhow!("{e:?}"))?;

                        let gep = self.builder.build_struct_gep(llvm_struct_ty, struct_ptr, field_idx as u32, &format!("field_{}", field))?;
                        let field_ty = llvm_struct_ty.get_field_types()[field_idx];
                        let loaded = self.builder.build_load(field_ty, gep, &format!("load_{}", field))?;
                        self.assign_value(result, loaded.as_basic_value_enum())?;
                        
                        // Check if the field is an array of structs and record it for the result register
                        if let Some(fields) = self.struct_definitions.get(&type_name) {
                            if let Some((_, field_type)) = fields.iter().find(|(n, _)| n == field) {
                                if let IRType::Array(inner) = field_type {
                                    if let IRType::Struct { name: inner_struct_name, .. } = &**inner {
                                        if let IRValue::Register(reg_id) = result {
                                            self.reg_array_element_struct.insert(*reg_id, inner_struct_name.clone());
                                        }
                                    }
                                    // Track integer and char arrays for proper array access
                                    if matches!(inner.as_ref(), IRType::Integer | IRType::Char) {
                                        if let IRValue::Register(reg_id) = result {
                                            self.reg_is_int_array.insert(*reg_id);
                                        }
                                    }
                                }
                            }
                        }

                        return Ok(());
                    }
                }
                
                // Fallback: Support CommandResult{ success: i1, output: i8* }
                let ty = self.ty_cmd_result();
                let ptr = if sv.is_pointer_value() {
                    sv.into_pointer_value()
                } else if sv.is_int_value() {
                    self.builder
                        .build_int_to_ptr(
                            sv.into_int_value(),
                            ty.ptr_type(inkwell::AddressSpace::from(0u16)),
                            "cmd_field_ptr",
                        )
                        .map_err(|e| anyhow!("{e:?}"))?
                } else if sv.is_struct_value() {
                    let tmp = self.alloca_for_type(
                        sv.into_struct_value().get_type().as_basic_type_enum(),
                        "cmd_field_stack",
                    )?;
                    self.builder.build_store(tmp, sv)?;
                    self.builder
                        .build_pointer_cast(
                            tmp,
                            ty.ptr_type(inkwell::AddressSpace::from(0u16)),
                            "cmd_field_ptr_stack",
                        )
                        .map_err(|e| anyhow!("{e:?}"))?
                } else {
                    return Err(anyhow!(format!(
                        "Unsupported field access value for {:?}",
                        struct_val
                    )));
                };
                let idx = match field.as_str() {
                    "success" => 0u32,
                    "output" => 1u32,
                    _ => 0u32,
                };
                let gep = self.builder.build_struct_gep(ty, ptr, idx, "fld")?;
                let loaded = if idx == 0 {
                    self.builder.build_load(self.bool_t, gep, "succ")?
                } else {
                    self.builder.build_load(self.i8_ptr_t, gep, "out")?
                };
                self.assign_value(result, loaded.as_basic_value_enum())?;
            }
            Instruction::FieldSet {
                struct_val,
                field,
                value,
            } => {
                let sv = self.eval_value(struct_val, fn_map)?;
                let val = self.eval_value(value, fn_map)?;
                
                // Try to determine the struct type from the variable name or register
                let struct_type_name = match struct_val {
                    IRValue::Variable(var_name) => {
                        self.var_struct_types.get(var_name).cloned()
                    }
                    IRValue::Register(reg_id) => {
                        self.reg_struct_types.get(reg_id).cloned()
                    }
                    _ => None
                };
                
                // Check if we have a registered struct type
                if let Some(type_name) = struct_type_name {
                    if let Some((llvm_struct_ty, field_names)) = self.struct_types.get(&type_name).cloned() {
                        // Find field index
                        let field_idx = field_names.iter().position(|n| n == field)
                            .ok_or_else(|| anyhow!("Field '{}' not found in struct '{}'", field, type_name))?;
                        
                        let ptr = if sv.is_pointer_value() {
                            sv.into_pointer_value()
                        } else if sv.is_int_value() {
                            self.builder
                                .build_int_to_ptr(
                                    sv.into_int_value(),
                                    llvm_struct_ty.ptr_type(inkwell::AddressSpace::from(0u16)),
                                    "struct_field_set_ptr",
                                )
                                .map_err(|e| anyhow!("{e:?}"))?
                        } else {
                            return Err(anyhow!("Unsupported field set value for {:?}", struct_val));
                        };
                        
                        let gep = self.builder.build_struct_gep(llvm_struct_ty, ptr, field_idx as u32, &format!("field_set_{}", field))?;
                        
                        // Convert value to the correct type if needed
                        let field_ty = llvm_struct_ty.get_field_types()[field_idx];
                        let store_val = if field_ty.is_int_type() {
                            if val.is_int_value() {
                                val
                            } else if val.is_float_value() {
                                self.builder.build_float_to_signed_int(
                                    val.into_float_value(),
                                    field_ty.into_int_type(),
                                    "f2i_field"
                                )?.as_basic_value_enum()
                            } else if val.is_pointer_value() {
                                self.builder.build_ptr_to_int(
                                    val.into_pointer_value(),
                                    field_ty.into_int_type(),
                                    "ptr2i_field"
                                )?.as_basic_value_enum()
                            } else {
                                val
                            }
                        } else if field_ty.is_float_type() {
                            if val.is_float_value() {
                                val
                            } else if val.is_int_value() {
                                self.builder.build_signed_int_to_float(
                                    val.into_int_value(),
                                    field_ty.into_float_type(),
                                    "i2f_field"
                                )?.as_basic_value_enum()
                            } else {
                                val
                            }
                        } else {
                            val
                        };
                        
                        self.builder.build_store(gep, store_val)?;
                        return Ok(());
                    }
                }
                
                // Fallback: no-op for unknown struct types
                // This shouldn't happen if types are properly tracked
            }
            Instruction::ChannelSelect {
                cases,
                payload_result,
                index_result,
                status_result,
            } => {
                self.lower_channel_select(
                    cases,
                    payload_result,
                    index_result,
                    status_result,
                    fn_map,
                )?;
            }
            Instruction::Scoped { .. } | Instruction::Spawn { .. } => {
                return Err(anyhow!(
                    "LLVM backend does not yet lower scoped/spawn concurrency instructions"
                ));
            }
            Instruction::Cast { value, target_type, result } => {
                let val = self.eval_value(value, fn_map)?;
                let target_llvm_ty = self.ir_type_to_llvm(target_type);
                
                let casted = match target_type {
                    IRType::Float => {
                        // Cast to float
                        if val.is_int_value() {
                            self.builder
                                .build_signed_int_to_float(val.into_int_value(), self.ctx.f64_type(), "i2f")?
                                .as_basic_value_enum()
                        } else if val.is_float_value() {
                            val // Already a float
                        } else if val.is_pointer_value() {
                            // Pointer to int, then to float
                            let int_val = self.builder
                                .build_ptr_to_int(val.into_pointer_value(), self.i64_t, "ptr2i")?;
                            self.builder
                                .build_signed_int_to_float(int_val, self.ctx.f64_type(), "i2f")?
                                .as_basic_value_enum()
                        } else {
                            return Err(anyhow!("Cannot cast {:?} to Float", val));
                        }
                    }
                    IRType::Integer => {
                        // Cast to integer
                        if val.is_float_value() {
                            self.builder
                                .build_float_to_signed_int(val.into_float_value(), self.i64_t, "f2i")?
                                .as_basic_value_enum()
                        } else if val.is_int_value() {
                            let iv = val.into_int_value();
                            if iv.get_type() == self.i64_t {
                                iv.as_basic_value_enum()
                            } else {
                                self.builder
                                    .build_int_s_extend(iv, self.i64_t, "sext")?
                                    .as_basic_value_enum()
                            }
                        } else if val.is_pointer_value() {
                            self.builder
                                .build_ptr_to_int(val.into_pointer_value(), self.i64_t, "ptr2i")?
                                .as_basic_value_enum()
                        } else {
                            return Err(anyhow!("Cannot cast {:?} to Integer", val));
                        }
                    }
                    IRType::Boolean => {
                        // Cast to boolean (non-zero = true)
                        if val.is_int_value() {
                            let iv = val.into_int_value();
                            let zero = iv.get_type().const_zero();
                            self.builder
                                .build_int_compare(inkwell::IntPredicate::NE, iv, zero, "tobool")?
                                .as_basic_value_enum()
                        } else if val.is_float_value() {
                            let fv = val.into_float_value();
                            let zero = fv.get_type().const_zero();
                            self.builder
                                .build_float_compare(inkwell::FloatPredicate::ONE, fv, zero, "ftobool")?
                                .as_basic_value_enum()
                        } else {
                            return Err(anyhow!("Cannot cast {:?} to Boolean", val));
                        }
                    }
                    _ => {
                        // Try generic cast
                        self.cast_basic_to_type(val, target_llvm_ty)?
                    }
                };
                
                self.assign_value(result, casted)?;
            }
            _ => {
                // Many IR ops are not required for bootstrap subset; ignore nops etc.
            }
        }
        Ok(())
    }

    fn lower_channel_select(
        &mut self,
        cases: &[IRSelectArm],
        payload_result: &IRValue,
        index_result: &IRValue,
        status_result: &IRValue,
        fn_map: &HashMap<String, FunctionValue<'ctx>>,
    ) -> Result<()> {
        if cases.is_empty() {
            return Err(anyhow!("ChannelSelect emitted without any cases"));
        }

        let count = cases.len() as u64;
        let count_i32 = self.ctx.i32_type().const_int(count, false);
        let case_buffer = self
            .builder
            .build_array_alloca(self.i8_ptr_t, count_i32, "select_cases")
            .map_err(|e| anyhow!("{e:?}"))?;

        for (idx, case) in cases.iter().enumerate() {
            let channel_val = self.eval_value(&case.channel, fn_map)?;
            let channel_ptr = self.to_i8_ptr(channel_val, &format!("select_case_ptr_{idx}"))?;
            let slot = unsafe {
                self.builder.build_gep(
                    self.i8_ptr_t,
                    case_buffer,
                    &[self.ctx.i32_type().const_int(idx as u64, false)],
                    &format!("select_case_slot_{idx}"),
                )
            }
            .map_err(|e| anyhow!("{e:?}"))?;
            self.builder
                .build_store(slot, channel_ptr.as_basic_value_enum())
                .map_err(|e| anyhow!("{e:?}"))?;
        }

        let result_ty = self.ty_select_result();
        let result_alloca = self
            .builder
            .build_alloca(result_ty, "select_result")
            .map_err(|e| anyhow!("{e:?}"))?;
        self.builder
            .build_store(result_alloca, result_ty.const_zero().as_basic_value_enum())
            .map_err(|e| anyhow!("{e:?}"))?;

        let select_fn = self.ensure_channel_select_fn();
        let case_buffer_raw = self
            .builder
            .build_pointer_cast(case_buffer, self.i8_ptr_t, "select_cases_raw")
            .map_err(|e| anyhow!("{e:?}"))?;
        let result_raw = self
            .builder
            .build_pointer_cast(result_alloca, self.i8_ptr_t, "select_result_raw")
            .map_err(|e| anyhow!("{e:?}"))?;
        let count_i64 = self.i64_t.const_int(count, false);
        let args = &[
            case_buffer_raw.as_basic_value_enum().into(),
            result_raw.as_basic_value_enum().into(),
            count_i64.into(),
        ];
        self.builder.build_call(select_fn, args, "select_call")?;

        let payload_ptr = self
            .builder
            .build_struct_gep(result_ty, result_alloca, 0, "select_payload_ptr")
            .map_err(|e| anyhow!("{e:?}"))?;
        let payload_val = self
            .builder
            .build_load(self.i8_ptr_t, payload_ptr, "select_payload")
            .map_err(|e| anyhow!("{e:?}"))?;
        self.assign_value(payload_result, payload_val.as_basic_value_enum())?;

        let index_ptr = self
            .builder
            .build_struct_gep(result_ty, result_alloca, 1, "select_index_ptr")
            .map_err(|e| anyhow!("{e:?}"))?;
        let index_val = self
            .builder
            .build_load(self.i64_t, index_ptr, "select_index")
            .map_err(|e| anyhow!("{e:?}"))?;
        self.assign_value(index_result, index_val.as_basic_value_enum())?;

        let status_ptr = self
            .builder
            .build_struct_gep(result_ty, result_alloca, 2, "select_status_ptr")
            .map_err(|e| anyhow!("{e:?}"))?;
        let status_val = self
            .builder
            .build_load(self.i64_t, status_ptr, "select_status")
            .map_err(|e| anyhow!("{e:?}"))?;
        self.assign_value(status_result, status_val.as_basic_value_enum())?;

        Ok(())
    }

    fn box_runtime_value(&mut self, value: BasicValueEnum<'ctx>) -> Result<PointerValue<'ctx>> {
        if value.is_int_value() {
            let int_val = value.into_int_value();
            let width = int_val.get_type().get_bit_width();
            if width == 1 {
                let bool_val = self
                    .builder
                    .build_int_z_extend(int_val, self.ctx.i32_type(), "box_bool_zext")
                    .map_err(|e| anyhow!("{e:?}"))?;
                let func = self.ensure_box_bool_fn();
                let call = self
                    .builder
                    .build_call(func, &[bool_val.into()], "box_bool")
                    .map_err(|e| anyhow!("{e:?}"))?;
                return call
                    .try_as_basic_value()
                    .left()
                    .map(|v| v.into_pointer_value())
                    .ok_or_else(|| anyhow!("box_bool returned void"));
            } else {
                let i64_val = if width == 64 {
                    int_val
                } else {
                    self.builder
                        .build_int_s_extend(int_val, self.i64_t, "box_int_sext")
                        .map_err(|e| anyhow!("{e:?}"))?
                };
                let func = self.ensure_box_int_fn();
                let call = self
                    .builder
                    .build_call(func, &[i64_val.into()], "box_int")
                    .map_err(|e| anyhow!("{e:?}"))?;
                return call
                    .try_as_basic_value()
                    .left()
                    .map(|v| v.into_pointer_value())
                    .ok_or_else(|| anyhow!("box_int returned void"));
            }
        }

        if value.is_pointer_value() {
            let ptr_val = self
                .builder
                .build_pointer_cast(value.into_pointer_value(), self.i8_ptr_t, "box_ptr_cast")
                .map_err(|e| anyhow!("{e:?}"))?;
            let func = self.ensure_box_ptr_fn();
            let call = self
                .builder
                .build_call(func, &[ptr_val.as_basic_value_enum().into()], "box_ptr")
                .map_err(|e| anyhow!("{e:?}"))?;
            return call
                .try_as_basic_value()
                .left()
                .map(|v| v.into_pointer_value())
                .ok_or_else(|| anyhow!("box_ptr returned void"));
        }

        if value.is_float_value() {
            let float_val = value.into_float_value();
            let as_int = self
                .builder
                .build_bit_cast(float_val, self.ctx.f64_type(), "box_float_cast")
                .map_err(|e| anyhow!("{e:?}"))?
                .into_float_value();
            let bits = self
                .builder
                .build_float_to_signed_int(as_int, self.i64_t, "box_float_bits")
                .map_err(|e| anyhow!("{e:?}"))?;
            let func = self.ensure_box_int_fn();
            let call = self
                .builder
                .build_call(func, &[bits.into()], "box_float")
                .map_err(|e| anyhow!("{e:?}"))?;
            return call
                .try_as_basic_value()
                .left()
                .map(|v| v.into_pointer_value())
                .ok_or_else(|| anyhow!("box_float returned void"));
        }

        // Fallback: treat as pointer by copying to heap.
        let ptr = self.to_i8_ptr(value, "box_fallback")?;
        let func = self.ensure_box_ptr_fn();
        let call = self
            .builder
            .build_call(
                func,
                &[ptr.as_basic_value_enum().into()],
                "box_fallback_ptr",
            )
            .map_err(|e| anyhow!("{e:?}"))?;
        call.try_as_basic_value()
            .left()
            .map(|v| v.into_pointer_value())
            .ok_or_else(|| anyhow!("box_ptr fallback returned void"))
    }

    fn ensure_box_int_fn(&mut self) -> FunctionValue<'ctx> {
        if let Some(f) = self.box_int_fn {
            f
        } else {
            let ty = self.i8_ptr_t.fn_type(&[self.i64_t.into()], false);
            let func = self.module.add_function("seen_box_int", ty, None);
            self.box_int_fn = Some(func);
            func
        }
    }

    fn ensure_box_bool_fn(&mut self) -> FunctionValue<'ctx> {
        if let Some(f) = self.box_bool_fn {
            f
        } else {
            let ty = self.i8_ptr_t.fn_type(&[self.ctx.i32_type().into()], false);
            let func = self.module.add_function("seen_box_bool", ty, None);
            self.box_bool_fn = Some(func);
            func
        }
    }

    fn ensure_box_ptr_fn(&mut self) -> FunctionValue<'ctx> {
        if let Some(f) = self.box_ptr_fn {
            f
        } else {
            let ty = self.i8_ptr_t.fn_type(&[self.i8_ptr_t.into()], false);
            let func = self.module.add_function("seen_box_ptr", ty, None);
            self.box_ptr_fn = Some(func);
            func
        }
    }

    fn ensure_channel_send_fn(&mut self) -> FunctionValue<'ctx> {
        if let Some(f) = self.module.get_function("seen_channel_send") {
            f
        } else {
            let ty = self
                .i8_ptr_t
                .fn_type(&[self.i8_ptr_t.into(), self.i8_ptr_t.into()], false);
            self.module.add_function("seen_channel_send", ty, None)
        }
    }

    fn ensure_int_to_string_fn(&mut self) -> FunctionValue<'ctx> {
        if let Some(f) = self.module.get_function("__IntToString") {
            f
        } else {
            let ty = self.i8_ptr_t.fn_type(&[self.i64_t.into()], false);
            self.module.add_function("__IntToString", ty, None)
        }
    }

    fn ensure_float_to_string_fn(&mut self) -> FunctionValue<'ctx> {
        if let Some(f) = self.module.get_function("__FloatToString") {
            f
        } else {
            let ty = self.i8_ptr_t.fn_type(&[self.ctx.f64_type().into()], false);
            self.module.add_function("__FloatToString", ty, None)
        }
    }

    fn to_string_ptr(&mut self, v: BasicValueEnum<'ctx>) -> Result<PointerValue<'ctx>> {
        if v.is_pointer_value() {
            return Ok(v.into_pointer_value());
        }
        if v.is_int_value() {
            let func = self.ensure_int_to_string_fn();
            let iv = v.into_int_value();
            let i64_val = if iv.get_type() == self.i64_t {
                iv
            } else {
                self.builder.build_int_s_extend(iv, self.i64_t, "sext")?
            };
            let call = self.builder.build_call(func, &[i64_val.into()], "i2s")?;
            return Ok(call.try_as_basic_value().left().unwrap().into_pointer_value());
        }
        if v.is_float_value() {
            let func = self.ensure_float_to_string_fn();
            let fv = v.into_float_value();
            let f64_val = if fv.get_type() == self.ctx.f64_type() {
                fv
            } else {
                self.builder.build_float_cast(fv, self.ctx.f64_type(), "f2d")?
            };
            let call = self.builder.build_call(func, &[f64_val.into()], "f2s")?;
            return Ok(call.try_as_basic_value().left().unwrap().into_pointer_value());
        }
        if v.is_struct_value() {
             let sv = v.into_struct_value();
             if let Ok(val) = self.builder.build_extract_value(sv, 0, "str_ptr") {
                if val.is_pointer_value() {
                    return Ok(val.into_pointer_value());
                }
            }
        }
        Err(anyhow!("Cannot convert {:?} to string pointer", v))
    }

    fn eval_value(
        &mut self,
        v: &IRValue,
        fn_map: &HashMap<String, FunctionValue<'ctx>>,
    ) -> Result<BasicValueEnum<'ctx>> {
        match v {
            IRValue::Integer(i) => Ok(self.i64_t.const_int(*i as u64, true).as_basic_value_enum()),
            IRValue::Boolean(b) => Ok(self
                .bool_t
                .const_int(if *b { 1 } else { 0 }, false)
                .as_basic_value_enum()),
            IRValue::String(s) => {
                let gv = self.builder.build_global_string_ptr(&(s.clone()), "str")?;
                Ok(gv.as_pointer_value().as_basic_value_enum())
            }
            IRValue::Void => Ok(self.i64_t.const_zero().as_basic_value_enum()),
            IRValue::Register(r) => {
                if let Some(val) = self.reg_values.get(r).cloned() {
                    // If this register contains a float (from Vec<Float>.get), bitcast to f64
                    if self.reg_is_float.contains(r) && val.is_int_value() {
                        let as_f64 = self.builder.build_bit_cast(
                            val.into_int_value(),
                            self.ctx.f64_type(),
                            "i2f_bitcast"
                        )?;
                        return Ok(as_f64);
                    }
                    return Ok(val);
                }
                if let Some(slot) = self.reg_slots.get(r).copied() {
                    let loaded =
                        self.builder
                            .build_load(self.i64_t, slot, &format!("load_r{}", r))?;
                    // If this register contains a float, bitcast to f64
                    if self.reg_is_float.contains(r) {
                        let as_f64 = self.builder.build_bit_cast(
                            loaded.into_int_value(),
                            self.ctx.f64_type(),
                            "i2f_bitcast"
                        )?;
                        return Ok(as_f64);
                    }
                    return Ok(loaded.as_basic_value_enum());
                }
                Err(anyhow!("Unknown register %r{r}"))
            }
            IRValue::Variable(name) => {
                if let Some(slot) = self.var_slots.get(name).copied() {
                    let loaded = self.load_from_slot(name, slot)?;
                    // If this variable contains a float (from Vec<Float>.get), bitcast to f64
                    if self.var_is_float.contains(name) && loaded.is_int_value() {
                        let as_f64 = self.builder.build_bit_cast(
                            loaded.into_int_value(),
                            self.ctx.f64_type(),
                            "i2f_bitcast"
                        )?;
                        return Ok(as_f64);
                    }
                    return Ok(loaded);
                }
                if let Some(v) = self.var_values.get(name).cloned() {
                    // Check if this is a float variable
                    if self.var_is_float.contains(name) && v.is_int_value() {
                        let as_f64 = self.builder.build_bit_cast(
                            v.into_int_value(),
                            self.ctx.f64_type(),
                            "i2f_bitcast"
                        )?;
                        return Ok(as_f64);
                    }
                    return Ok(v);
                }
                Err(anyhow!(format!("Unknown variable {}", name)))
            }
            IRValue::Array(_vals) => {
                // Materialize arrays on demand in consumers; here return opaque null as placeholder
                Ok(self.i8_ptr_t.const_null().as_basic_value_enum())
            }
            IRValue::ByteArray(data) => self.byte_array_ptr(data),
            IRValue::Null => Ok(self.i8_ptr_t.const_null().as_basic_value_enum()),
            IRValue::Function { name, .. } => {
                let f = fn_map
                    .get(name)
                    .ok_or_else(|| anyhow!("Unknown function {name}"))?;
                Ok(f.as_global_value().as_pointer_value().as_basic_value_enum())
            }
            IRValue::Float(fv) => Ok(self.ctx.f64_type().const_float(*fv).as_basic_value_enum()),
            IRValue::Struct { type_name, fields } => {
                // Allocate memory for the struct on the HEAP and populate fields
                // (heap allocation is required because structs may be returned from functions)
                let llvm_struct_ty = self.get_or_create_struct_type(type_name, fields);
                
                // Get field order from registry
                let field_order = if let Some((_, names)) = self.struct_types.get(type_name) {
                    names.clone()
                } else {
                    // Fallback to sorted keys
                    let mut names: Vec<String> = fields.keys().cloned().collect();
                    names.sort();
                    names
                };
                
                // Allocate struct on heap using malloc
                let struct_size = llvm_struct_ty.size_of()
                    .ok_or_else(|| anyhow!("Cannot get size of struct {}", type_name))?;
                let size_i64 = if struct_size.get_type() == self.i64_t {
                    struct_size
                } else {
                    self.builder
                        .build_int_z_extend(struct_size, self.i64_t, "struct_size_ext")
                        .map_err(|e| anyhow!("{e:?}"))?
                };
                let malloc = self.get_malloc();
                let heap_ptr = self.builder
                    .build_call(malloc, &[size_i64.into()], &format!("{}_malloc", type_name))
                    .map_err(|e| anyhow!("{e:?}"))?
                    .try_as_basic_value()
                    .left()
                    .ok_or_else(|| anyhow!("malloc returned void"))?
                    .into_pointer_value();
                
                // Cast to struct pointer type
                let struct_ptr = self.builder
                    .build_pointer_cast(
                        heap_ptr,
                        llvm_struct_ty.ptr_type(inkwell::AddressSpace::from(0u16)),
                        &format!("{}_ptr", type_name)
                    )
                    .map_err(|e| anyhow!("{e:?}"))?;
                
                // Set each field
                for (idx, field_name) in field_order.iter().enumerate() {
                    if let Some(field_val) = fields.get(field_name) {
                        let val = self.eval_value(field_val, fn_map)?;
                        let field_ptr = self.builder.build_struct_gep(
                            llvm_struct_ty,
                            struct_ptr,
                            idx as u32,
                            &format!("{}_field_{}", type_name, field_name)
                        )?;
                        self.builder.build_store(field_ptr, val)?;
                    }
                }
                
                // Return pointer to struct (cast to i8* for uniform handling)
                Ok(self.builder
                    .build_pointer_cast(struct_ptr, self.i8_ptr_t, &format!("{}_heap_ptr", type_name))?
                    .as_basic_value_enum())
            }
            _ => Err(anyhow!("Unsupported IRValue in LLVM backend: {v:?}")),
        }
    }

    fn to_i8_ptr(&mut self, value: BasicValueEnum<'ctx>, name: &str) -> Result<PointerValue<'ctx>> {
        match value {
            BasicValueEnum::PointerValue(ptr) => self
                .builder
                .build_pointer_cast(ptr, self.i8_ptr_t, name)
                .map_err(|e| anyhow!("{e:?}")),
            BasicValueEnum::IntValue(int_val) => self
                .builder
                .build_int_to_ptr(int_val, self.i8_ptr_t, name)
                .map_err(|e| anyhow!("{e:?}")),
            BasicValueEnum::StructValue(struct_val) => {
                let ty = struct_val.get_type().as_basic_type_enum();
                let tmp = self.alloca_for_type(ty, &format!("{name}_stack"))?;
                self.builder.build_store(tmp, struct_val)?;
                self.builder
                    .build_pointer_cast(tmp, self.i8_ptr_t, &format!("{name}_stack_ptr"))
                    .map_err(|e| anyhow!("{e:?}"))
            }
            other => Err(anyhow!(
                "select requires pointer compatible value, got {:?}",
                other
            )),
        }
    }

    fn byte_array_ptr(&mut self, data: &[u8]) -> Result<BasicValueEnum<'ctx>> {
        let global = self.byte_array_global(data)?;
        let cast = self
            .builder
            .build_pointer_cast(global.as_pointer_value(), self.i8_ptr_t, "embed_ptr")
            .map_err(|e| anyhow!("{e:?}"))?;
        Ok(cast.as_basic_value_enum())
    }

    fn byte_array_global(&mut self, data: &[u8]) -> Result<GlobalValue<'ctx>> {
        if let Some(global) = self.byte_array_globals.get(data) {
            return Ok(*global);
        }

        let byte_ty = self.ctx.i8_type();
        let (array_ty, initializer) = if data.is_empty() {
            let arr_ty = byte_ty.array_type(1);
            let init = byte_ty.const_array(&[byte_ty.const_zero()]);
            (arr_ty, init)
        } else {
            let len = u32::try_from(data.len())
                .map_err(|_| anyhow!("Embedded blob exceeds maximum supported size"))?;
            let arr_ty = byte_ty.array_type(len);
            let const_vals: Vec<_> = data
                .iter()
                .map(|b| byte_ty.const_int(*b as u64, false))
                .collect();
            let init = byte_ty.const_array(&const_vals);
            (arr_ty, init)
        };

        let symbol = format!("__seen_embed_{}", self.byte_array_globals.len());
        let global = self.module.add_global(array_ty, None, &symbol);
        global.set_initializer(&initializer);
        global.set_constant(true);
        global.set_linkage(Linkage::Private);
        global.set_unnamed_address(UnnamedAddress::Global);
        global.set_alignment(1);
        self.byte_array_globals.insert(data.to_vec(), global);
        Ok(global)
    }

    fn assign_value(&mut self, dest: &IRValue, v: BasicValueEnum<'ctx>) -> Result<()> {
        match dest {
            IRValue::Register(r) => {
                self.reg_values.insert(*r, v.clone());
                // Also persist through the reg slot if available
                if let Some(slot) = self.reg_slots.get(r).copied() {
                    let reg_val = v.clone();
                    let ival = if reg_val.is_int_value() {
                        let iv = reg_val.into_int_value();
                        if iv.get_type() == self.i64_t {
                            iv
                        } else {
                            self.builder
                                .build_int_s_extend(iv, self.i64_t, "sext")
                                .ok()
                                .unwrap_or(self.i64_t.const_zero())
                        }
                    } else if reg_val.is_pointer_value() {
                        self.builder
                            .build_ptr_to_int(reg_val.into_pointer_value(), self.i64_t, "ptr2i")
                            .ok()
                            .unwrap_or(self.i64_t.const_zero())
                    } else if reg_val.is_float_value() {
                        self.builder
                            .build_float_to_signed_int(
                                reg_val.into_float_value(),
                                self.i64_t,
                                "ftoi",
                            )
                            .ok()
                            .unwrap_or(self.i64_t.const_zero())
                    } else if v.is_vector_value() {
                        self.i64_t.const_zero()
                    } else {
                        self.i64_t.const_zero()
                    };
                    self.builder
                        .build_store(slot, ival)
                        .map_err(|e| anyhow!("{e:?}"))?;
                }
                Ok(())
            }
            IRValue::Variable(name) => {
                // Update immediate map
                self.var_values.insert(name.clone(), v.clone());
                // Persist to slot (create lazily if needed)
                let (slot, slot_ty) = if let Some(p) = self.var_slots.get(name).copied() {
                    let ty = *self
                        .var_slot_types
                        .get(name)
                        .ok_or_else(|| anyhow!("Missing slot type for {}", name))?;
                    (p, ty)
                } else {
                    let value_ty = self
                        .basic_type_from_value(&v)
                        .ok_or_else(|| anyhow!("Cannot infer type for variable {}", name))?;
                    let slot = self.alloca_for_type(value_ty, &format!("var_{}", name))?;
                    self.var_slots.insert(name.clone(), slot);
                    self.var_slot_types.insert(name.clone(), value_ty);
                    (slot, value_ty)
                };
                // For float variables (e.g., from Vec<Float>.get), use bitcast to preserve bits
                let stored = if self.var_is_float.contains(name) && v.is_float_value() && slot_ty.is_int_type() {
                    self.builder.build_bit_cast(
                        v.into_float_value(),
                        slot_ty,
                        "f2i_bitcast"
                    )?
                } else {
                    self.cast_basic_to_type(v, slot_ty)?
                };
                self.builder
                    .build_store(slot, stored)
                    .map_err(|e| anyhow!("{e:?}"))?;
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn as_bool(&self, v: BasicValueEnum<'ctx>) -> Result<inkwell::values::IntValue<'ctx>> {
        if v.is_int_value() && v.into_int_value().get_type() == self.bool_t {
            return Ok(v.into_int_value());
        }
        if v.is_int_value() {
            let zero = v.into_int_value().get_type().const_zero();
            return self
                .builder
                .build_int_compare(
                    inkwell::IntPredicate::NE,
                    v.into_int_value(),
                    zero,
                    "tobool",
                )
                .map_err(|e| anyhow!("{e:?}"));
        }
        Err(anyhow!("Cannot convert value to bool"))
    }

    fn as_i64(&self, v: BasicValueEnum<'ctx>) -> Result<inkwell::values::IntValue<'ctx>> {
        if v.is_int_value() {
            let iv = v.into_int_value();
            if iv.get_type() == self.i64_t {
                Ok(iv)
            } else {
                self.builder
                    .build_int_s_extend(iv, self.i64_t, "sext")
                    .map_err(|e| anyhow!("{e:?}"))
            }
        } else if v.is_pointer_value() {
            // Handle pointer values by converting to int
            self.builder
                .build_ptr_to_int(v.into_pointer_value(), self.i64_t, "ptr2i")
                .map_err(|e| anyhow!("{e:?}"))
        } else if v.is_float_value() {
            // Handle float values by converting to int
            self.builder
                .build_float_to_signed_int(v.into_float_value(), self.i64_t, "f2i")
                .map_err(|e| anyhow!("{e:?}"))
        } else {
            Err(anyhow!("Expected integer value, got {:?}", v))
        }
    }

    fn as_f64(&self, v: BasicValueEnum<'ctx>) -> Result<inkwell::values::FloatValue<'ctx>> {
        if v.is_float_value() {
            Ok(v.into_float_value())
        } else if v.is_int_value() {
            // Convert int to float
            self.builder
                .build_signed_int_to_float(v.into_int_value(), self.ctx.f64_type(), "i2f")
                .map_err(|e| anyhow!("{e:?}"))
        } else if v.is_pointer_value() {
            // Convert pointer to int, then to float
            let int_val = self.builder
                .build_ptr_to_int(v.into_pointer_value(), self.i64_t, "ptr2i")
                .map_err(|e| anyhow!("{e:?}"))?;
            self.builder
                .build_signed_int_to_float(int_val, self.ctx.f64_type(), "i2f")
                .map_err(|e| anyhow!("{e:?}"))
        } else {
            Err(anyhow!("Cannot convert {:?} to float", v))
        }
    }

    fn as_usize(&self, v: BasicValueEnum<'ctx>) -> Result<u64> {
        let iv = self.as_i64(v)?;
        Ok(iv.get_zero_extended_constant().unwrap_or(0))
    }

    fn as_cstr_ptr(&self, v: BasicValueEnum<'ctx>) -> Result<PointerValue<'ctx>> {
        if v.is_pointer_value() {
            return Ok(v.into_pointer_value());
        }
        if v.is_struct_value() {
            let sv = v.into_struct_value();
            // Assume String struct wrapper, extract first field
            if let Ok(val) = self.builder.build_extract_value(sv, 0, "str_ptr") {
                if val.is_pointer_value() {
                    return Ok(val.into_pointer_value());
                }
            }
        }
        if v.is_int_value() {
            return self
                .builder
                .build_int_to_ptr(v.into_int_value(), self.i8_ptr_t, "i2ptr")
                .map_err(|e| anyhow!("{e:?}"));
        }
        Err(anyhow!("Expected pointer to cstr"))
    }

    fn alloca_for_type(
        &mut self,
        ty: BasicTypeEnum<'ctx>,
        name: &str,
    ) -> Result<PointerValue<'ctx>> {
        // Save current position
        let current_block = self.builder.get_insert_block();
        
        // Move to entry block
        if let Some(func) = self.current_fn {
            if let Some(entry) = func.get_first_basic_block() {
                if let Some(first_inst) = entry.get_first_instruction() {
                    self.builder.position_before(&first_inst);
                } else {
                    self.builder.position_at_end(entry);
                }
            }
        }

        let slot = match ty {
            BasicTypeEnum::ArrayType(at) => self.builder.build_alloca(at, name),
            BasicTypeEnum::FloatType(ft) => self.builder.build_alloca(ft, name),
            BasicTypeEnum::IntType(it) => self.builder.build_alloca(it, name),
            BasicTypeEnum::PointerType(pt) => self.builder.build_alloca(pt, name),
            BasicTypeEnum::StructType(st) => self.builder.build_alloca(st, name),
            BasicTypeEnum::VectorType(vt) => self.builder.build_alloca(vt, name),
            BasicTypeEnum::ScalableVectorType(svt) => self.builder.build_alloca(svt, name),
        }
        .map_err(|e| anyhow!("{e:?}"))?;

        // Restore position
        if let Some(block) = current_block {
            self.builder.position_at_end(block);
        }

        Ok(slot)
    }

    fn load_from_slot(
        &mut self,
        name: &str,
        slot: PointerValue<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>> {
        let elem_ty = *self
            .var_slot_types
            .get(name)
            .ok_or_else(|| anyhow!("Missing slot type for {}", name))?;
        let load_name = format!("load_{}", name);
        let loaded = match elem_ty {
            BasicTypeEnum::ArrayType(at) => self.builder.build_load(at, slot, &load_name),
            BasicTypeEnum::FloatType(ft) => self.builder.build_load(ft, slot, &load_name),
            BasicTypeEnum::IntType(it) => self.builder.build_load(it, slot, &load_name),
            BasicTypeEnum::PointerType(pt) => self.builder.build_load(pt, slot, &load_name),
            BasicTypeEnum::StructType(st) => self.builder.build_load(st, slot, &load_name),
            BasicTypeEnum::VectorType(vt) => self.builder.build_load(vt, slot, &load_name),
            BasicTypeEnum::ScalableVectorType(svt) => {
                self.builder.build_load(svt, slot, &load_name)
            }
        }
        .map_err(|e| anyhow!("{e:?}"))?;
        Ok(loaded.as_basic_value_enum())
    }

    fn basic_type_from_value(&self, value: &BasicValueEnum<'ctx>) -> Option<BasicTypeEnum<'ctx>> {
        Some(match value {
            BasicValueEnum::ArrayValue(av) => av.get_type().as_basic_type_enum(),
            BasicValueEnum::FloatValue(fv) => fv.get_type().as_basic_type_enum(),
            BasicValueEnum::IntValue(iv) => iv.get_type().as_basic_type_enum(),
            BasicValueEnum::PointerValue(pv) => pv.get_type().as_basic_type_enum(),
            BasicValueEnum::StructValue(sv) => sv.get_type().as_basic_type_enum(),
            BasicValueEnum::VectorValue(vv) => vv.get_type().as_basic_type_enum(),
            BasicValueEnum::ScalableVectorValue(svv) => svv.get_type().as_basic_type_enum(),
        })
    }

    fn cast_basic_to_type(
        &mut self,
        value: BasicValueEnum<'ctx>,
        target: BasicTypeEnum<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>> {
        match (value, target) {
            (BasicValueEnum::IntValue(iv), BasicTypeEnum::IntType(it)) => {
                if iv.get_type() == it {
                    Ok(iv.as_basic_value_enum())
                } else if iv.get_type().get_bit_width() < it.get_bit_width() {
                    let ext = self
                        .builder
                        .build_int_s_extend(iv, it, "sext_store")
                        .map_err(|e| anyhow!("{e:?}"))?;
                    Ok(ext.as_basic_value_enum())
                } else {
                    let trunc = self
                        .builder
                        .build_int_truncate(iv, it, "trunc_store")
                        .map_err(|e| anyhow!("{e:?}"))?;
                    Ok(trunc.as_basic_value_enum())
                }
            }
            (BasicValueEnum::IntValue(iv), BasicTypeEnum::FloatType(ft)) => {
                // Convert integer to float
                let cast = self
                    .builder
                    .build_signed_int_to_float(iv, ft, "i2f_store")
                    .map_err(|e| anyhow!("{e:?}"))?;
                Ok(cast.as_basic_value_enum())
            }
            (BasicValueEnum::IntValue(iv), BasicTypeEnum::PointerType(pt)) => {
                let cast = self
                    .builder
                    .build_int_to_ptr(iv, pt, "int2ptr_store")
                    .map_err(|e| anyhow!("{e:?}"))?;
                Ok(cast.as_basic_value_enum())
            }
            (BasicValueEnum::PointerValue(pv), BasicTypeEnum::PointerType(pt)) => {
                if pv.get_type() == pt {
                    Ok(pv.as_basic_value_enum())
                } else {
                    let cast = self
                        .builder
                        .build_bit_cast(
                            pv.as_basic_value_enum(),
                            pt.as_basic_type_enum(),
                            "ptrcast_store",
                        )
                        .map_err(|e| anyhow!("{e:?}"))?;
                    Ok(cast)
                }
            }
            (BasicValueEnum::PointerValue(pv), BasicTypeEnum::IntType(it)) => {
                let cast = self
                    .builder
                    .build_ptr_to_int(pv, it, "ptr2int_store")
                    .map_err(|e| anyhow!("{e:?}"))?;
                Ok(cast.as_basic_value_enum())
            }
            (BasicValueEnum::PointerValue(pv), BasicTypeEnum::FloatType(ft)) => {
                // Pointer to int, then int to float
                let int_val = self
                    .builder
                    .build_ptr_to_int(pv, self.i64_t, "ptr2int_store")
                    .map_err(|e| anyhow!("{e:?}"))?;
                let cast = self
                    .builder
                    .build_signed_int_to_float(int_val, ft, "i2f_store")
                    .map_err(|e| anyhow!("{e:?}"))?;
                Ok(cast.as_basic_value_enum())
            }
            (BasicValueEnum::FloatValue(fv), BasicTypeEnum::FloatType(ft)) => {
                if fv.get_type() == ft {
                    Ok(fv.as_basic_value_enum())
                } else {
                    Err(anyhow!("Mismatched float store type"))
                }
            }
            (BasicValueEnum::VectorValue(vv), BasicTypeEnum::VectorType(vt)) => {
                if vv.get_type() == vt {
                    Ok(vv.as_basic_value_enum())
                } else {
                    Err(anyhow!("Vector lane mismatch"))
                }
            }
            (BasicValueEnum::FloatValue(fv), BasicTypeEnum::IntType(it)) => {
                let cast = self
                    .builder
                    .build_float_to_signed_int(fv, it, "ftosi_store")
                    .map_err(|e| anyhow!("{e:?}"))?;
                Ok(cast.as_basic_value_enum())
            }
            (BasicValueEnum::IntValue(iv), BasicTypeEnum::FloatType(ft)) => {
                let cast = self
                    .builder
                    .build_signed_int_to_float(iv, ft, "sitofp_store")
                    .map_err(|e| anyhow!("{e:?}"))?;
                Ok(cast.as_basic_value_enum())
            }
            (BasicValueEnum::StructValue(sv), BasicTypeEnum::StructType(st)) => {
                if sv.get_type() == st {
                    Ok(sv.as_basic_value_enum())
                } else {
                    Err(anyhow!("Mismatched struct store type"))
                }
            }
            (BasicValueEnum::ArrayValue(av), BasicTypeEnum::ArrayType(at)) => {
                if av.get_type() == at {
                    Ok(av.as_basic_value_enum())
                } else {
                    Err(anyhow!("Mismatched array store type"))
                }
            }
            (BasicValueEnum::VectorValue(vv), BasicTypeEnum::VectorType(vt)) => {
                if vv.get_type() == vt {
                    Ok(vv.as_basic_value_enum())
                } else {
                    Err(anyhow!("Mismatched vector store type"))
                }
            }
            (BasicValueEnum::ScalableVectorValue(svv), BasicTypeEnum::ScalableVectorType(svt)) => {
                if svv.get_type() == svt {
                    Ok(svv.as_basic_value_enum())
                } else {
                    Err(anyhow!("Mismatched scalable vector store type"))
                }
            }
            (_, target_ty) => Err(anyhow!(
                "Cannot cast value to requested type {}",
                target_ty.print_to_string().to_string()
            )),
        }
    }

    fn is_string_value_ir(&self, value: &IRValue) -> bool {
        if is_string_literal_ir(value) {
            return true;
        }
        if let IRValue::Variable(name) = value {
            if let Some(BasicTypeEnum::PointerType(pt)) = self.var_slot_types.get(name) {
                return *pt == self.i8_ptr_t;
            }
        }
        false
    }

    fn get_printf(&mut self) -> FunctionValue<'ctx> {
        if let Some(f) = self.printf {
            return f;
        }
        let i8ptr = self.i8_ptr_t;
        let ty = self.i64_t.fn_type(&[i8ptr.into()], true);
        let f = self.module.add_function("printf", ty, None);
        self.printf = Some(f);
        f
    }

    fn get_strlen(&mut self) -> FunctionValue<'ctx> {
        if let Some(f) = self.strlen {
            return f;
        }
        let ty = self.i64_t.fn_type(&[self.i8_ptr_t.into()], false);
        let f = self.module.add_function("strlen", ty, None);
        self.strlen = Some(f);
        f
    }

    fn get_strcmp(&mut self) -> FunctionValue<'ctx> {
        if let Some(f) = self.strcmp {
            return f;
        }
        let i32_t = self.ctx.i32_type();
        let ty = i32_t.fn_type(&[self.i8_ptr_t.into(), self.i8_ptr_t.into()], false);
        let f = self.module.add_function("strcmp", ty, None);
        self.strcmp = Some(f);
        f
    }

    fn get_malloc(&mut self) -> FunctionValue<'ctx> {
        if let Some(f) = self.malloc {
            return f;
        }
        let ty = self.i8_ptr_t.fn_type(&[self.i64_t.into()], false);
        let f = self.module.add_function("malloc", ty, None);
        self.malloc = Some(f);
        f
    }

    fn get_realloc(&mut self) -> FunctionValue<'ctx> {
        if let Some(f) = self.realloc {
            return f;
        }
        let ty = self.i8_ptr_t.fn_type(&[self.i8_ptr_t.into(), self.i64_t.into()], false);
        let f = self.module.add_function("realloc", ty, None);
        self.realloc = Some(f);
        f
    }

    fn get_free(&mut self) -> FunctionValue<'ctx> {
        if let Some(f) = self.free {
            return f;
        }
        let ty = self.ctx.void_type().fn_type(&[self.i8_ptr_t.into()], false);
        let f = self.module.add_function("free", ty, None);
        self.free = Some(f);
        f
    }

    fn get_memcpy(&mut self) -> FunctionValue<'ctx> {
        if let Some(f) = self.memcpy {
            return f;
        }
        // declare void *memcpy(void *dest, const void *src, size_t n);
        let ty = self.i8_ptr_t.fn_type(
            &[
                self.i8_ptr_t.into(),
                self.i8_ptr_t.into(),
                self.i64_t.into(),
            ],
            false,
        );
        let f = self.module.add_function("memcpy", ty, None);
        self.memcpy = Some(f);
        f
    }

    fn get_or_declare_clock_gettime(&mut self) -> FunctionValue<'ctx> {
        if let Some(f) = self.module.get_function("clock_gettime") {
            return f;
        }
        // int clock_gettime(clockid_t clk_id, struct timespec *tp);
        // timespec is { i64 tv_sec, i64 tv_nsec }
        let timespec_ty = self.ctx.struct_type(&[
            self.i64_t.into(),
            self.i64_t.into(),
        ], false);
        let ty = self.ctx.i32_type().fn_type(&[
            self.ctx.i32_type().into(),
            timespec_ty.ptr_type(inkwell::AddressSpace::from(0u16)).into(),
        ], false);
        self.module.add_function("clock_gettime", ty, None)
    }

    fn call_printf(&mut self, args: &[BasicMetadataValueEnum<'ctx>]) -> Result<()> {
        let printf = self.get_printf();
        self.builder
            .build_call(printf, args, "printf_call")
            .map(|_| ())
            .map_err(|e| anyhow!("{e:?}"))
    }

    fn get_fflush(&self) -> FunctionValue<'ctx> {
        if let Some(func) = self.module.get_function("fflush") {
            return func;
        }
        let i32_t = self.ctx.i32_type();
        let ptr_t = self.i8_ptr_t;
        let fn_type = i32_t.fn_type(&[ptr_t.into()], false);
        self.module.add_function("fflush", fn_type, None)
    }

    fn call_fflush(&mut self) -> Result<()> {
        let fflush = self.get_fflush();
        let null = self.i8_ptr_t.const_zero();
        self.builder
            .build_call(fflush, &[null.into()], "fflush_call")
            .map(|_| ())
            .map_err(|e| anyhow!("{e:?}"))
    }

    fn call_strlen(&mut self, s: PointerValue<'ctx>) -> Result<inkwell::values::IntValue<'ctx>> {
        let strlen = self.get_strlen();
        let call = self
            .builder
            .build_call(strlen, &[s.into()], "strlen")
            .map_err(|e| anyhow!("{e:?}"))?;
        Ok(call.try_as_basic_value().left().unwrap().into_int_value())
    }

    fn call_strcmp(
        &mut self,
        a: PointerValue<'ctx>,
        b: PointerValue<'ctx>,
    ) -> Result<inkwell::values::IntValue<'ctx>> {
        let strcmp = self.get_strcmp();
        let call = self
            .builder
            .build_call(strcmp, &[a.into(), b.into()], "strcmp")
            .map_err(|e| anyhow!("{e:?}"))?;
        Ok(call.try_as_basic_value().left().unwrap().into_int_value())
    }

    fn runtime_concat(
        &mut self,
        left: PointerValue<'ctx>,
        right: PointerValue<'ctx>,
    ) -> Result<PointerValue<'ctx>> {
        let l_len = self.call_strlen(left)?;
        let r_len = self.call_strlen(right)?;
        let one = self.i64_t.const_int(1, false);
        let total = self.builder.build_int_add(
            self.builder.build_int_add(l_len, r_len, "sum")?,
            one,
            "plus1",
        )?;
        let malloc = self.get_malloc();
        let buf = self
            .builder
            .build_call(malloc, &[total.into()], "malloc")
            .map_err(|e| anyhow!("{e:?}"))?;
        let dest = buf
            .try_as_basic_value()
            .left()
            .unwrap()
            .into_pointer_value();

        // memcpy(dest, left, l_len)
        let memcpy = self.get_memcpy();
        self.builder
            .build_call(memcpy, &[dest.into(), left.into(), l_len.into()], "cpy1")
            .map_err(|e| anyhow!("{e:?}"))?;
        // memcpy(dest + l_len, right, r_len)
        let dest_off = unsafe {
            self.builder
                .build_gep(self.ctx.i8_type(), dest, &[l_len], "off")?
        };
        self.builder
            .build_call(
                memcpy,
                &[dest_off.into(), right.into(), r_len.into()],
                "cpy2",
            )
            .map_err(|e| anyhow!("{e:?}"))?;
        // null terminate at dest[l_len + r_len]
        let total_minus_one = self
            .builder
            .build_int_sub(total, one, "last_index")
            .map_err(|e| anyhow!("{e:?}"))?;
        let end_ptr = unsafe {
            self.builder
                .build_gep(self.ctx.i8_type(), dest, &[total_minus_one], "end")?
        };
        let zero = self.ctx.i8_type().const_int(0, false);
        self.builder.build_store(end_ptr, zero)?;
        Ok(dest)
    }

    fn runtime_endswith(
        &mut self,
        s: PointerValue<'ctx>,
        suffix: PointerValue<'ctx>,
    ) -> Result<inkwell::values::IntValue<'ctx>> {
        let s_len = self.call_strlen(s)?;
        let suf_len = self.call_strlen(suffix)?;
        let suf_gt =
            self.builder
                .build_int_compare(inkwell::IntPredicate::UGT, suf_len, s_len, "suf_gt")?;
        let len_ok = self
            .builder
            .build_not(suf_gt, "len_ok")
            .map_err(|e| anyhow!("{e:?}"))?;
        let start = self
            .builder
            .build_int_sub(s_len, suf_len, "start")
            .map_err(|e| anyhow!("{e:?}"))?;
        let off = unsafe {
            self.builder
                .build_gep(self.ctx.i8_type(), s, &[start], "s_off")
                .map_err(|e| anyhow!("{e:?}"))?
        };
        let cmp = self.call_strcmp(off, suffix)?;
        let zero32 = self.ctx.i32_type().const_zero();
        let cmp_eq = self
            .builder
            .build_int_compare(inkwell::IntPredicate::EQ, cmp, zero32, "ends_eq")
            .map_err(|e| anyhow!("{e:?}"))?;
        let suf_zero = self
            .builder
            .build_int_compare(
                inkwell::IntPredicate::EQ,
                suf_len,
                self.i64_t.const_zero(),
                "suf_zero",
            )
            .map_err(|e| anyhow!("{e:?}"))?;
        let eq_or_zero = self
            .builder
            .build_or(cmp_eq, suf_zero, "ends_match")
            .map_err(|e| anyhow!("{e:?}"))?;
        let result = self
            .builder
            .build_and(len_ok, eq_or_zero, "ends_res")
            .map_err(|e| anyhow!("{e:?}"))?;
        Ok(result)
    }

    fn runtime_substring(
        &mut self,
        s: PointerValue<'ctx>,
        start: inkwell::values::IntValue<'ctx>,
        end: inkwell::values::IntValue<'ctx>,
    ) -> Result<PointerValue<'ctx>> {
        let len = self.builder.build_int_sub(end, start, "sub_len")?;
        let one = self.i64_t.const_int(1, false);
        let total = self.builder.build_int_add(len, one, "plus1")?;
        let malloc = self.get_malloc();
        let buf = self
            .builder
            .build_call(malloc, &[total.into()], "malloc")
            .map_err(|e| anyhow!("{e:?}"))?;
        let dest = buf
            .try_as_basic_value()
            .left()
            .unwrap()
            .into_pointer_value();
        let src = unsafe {
            self.builder
                .build_gep(self.ctx.i8_type(), s, &[start], "src_off")?
        };
        let memcpy = self.get_memcpy();
        self.builder
            .build_call(memcpy, &[dest.into(), src.into(), len.into()], "cpy")?;
        let end_ptr = unsafe {
            self.builder
                .build_gep(self.ctx.i8_type(), dest, &[len], "end")?
        };
        self.builder
            .build_store(end_ptr, self.ctx.i8_type().const_zero())?;
        Ok(dest)
    }

    fn declare_main_wrapper(&mut self) {
        if self.module.get_function("main").is_some() {
            return;
        }
        let i32_t = self.ctx.i32_type();
        let i8_ptr_ptr = self.i8_ptr_t.ptr_type(inkwell::AddressSpace::from(0u16));
        let c_main_ty = i32_t.fn_type(&[i32_t.into(), i8_ptr_ptr.into()], false);
        let c_main = self.module.add_function("main", c_main_ty, None);
        let bb = self.ctx.append_basic_block(c_main, "entry");
        self.builder.position_at_end(bb);
        // Initialize argc/argv globals
        let (g_argc, g_argv) = self.ensure_arg_globals();
        let argc = c_main.get_nth_param(0).unwrap().into_int_value();
        let argv = c_main.get_nth_param(1).unwrap().into_pointer_value();
        self.builder
            .build_store(g_argc.as_pointer_value(), argc)
            .unwrap();
        self.builder
            .build_store(g_argv.as_pointer_value(), argv)
            .unwrap();
        let mut exit_code = i32_t.const_zero();
        if let Some(seen) = self.module.get_function("seen_main") {
            let call = self.builder.build_call(seen, &[], "call_main").unwrap();
            if let Some(ret) = call.try_as_basic_value().left() {
                if self.cli_mode {
                    let printf = self.get_printf();
                    let fmt = self
                        .builder
                        .build_global_string_ptr("%lld\n", "seen_cli_result_fmt")
                        .expect("create CLI printf format literal");
                    let int_val = ret.into_int_value();
                    let widened = if int_val.get_type().get_bit_width() != 64 {
                        self.builder
                            .build_int_s_extend_or_bit_cast(int_val, self.i64_t, "seen_cli_cast")
                            .unwrap()
                    } else {
                        int_val
                    };
                    self.builder
                        .build_call(
                            printf,
                            &[fmt.as_pointer_value().into(), widened.into()],
                            "seen_cli_print_result",
                        )
                        .unwrap();
                } else {
                    let i64v = ret.into_int_value();
                    exit_code = self.builder.build_int_truncate(i64v, i32_t, "tr").unwrap();
                }
            }
        }
        self.builder.build_return(Some(&exit_code)).unwrap();
    }

    fn ensure_arg_globals(
        &mut self,
    ) -> (
        inkwell::values::GlobalValue<'ctx>,
        inkwell::values::GlobalValue<'ctx>,
    ) {
        if let (Some(a), Some(v)) = (self.g_argc, self.g_argv) {
            return (a, v);
        }
        let g_argc = self.module.add_global(self.ctx.i32_type(), None, "__argc");
        g_argc.set_initializer(&self.ctx.i32_type().const_int(0, false));
        let g_argv = self.module.add_global(
            self.i8_ptr_t.ptr_type(inkwell::AddressSpace::from(0u16)),
            None,
            "__argv",
        );
        g_argv.set_initializer(
            &self
                .i8_ptr_t
                .ptr_type(inkwell::AddressSpace::from(0u16))
                .const_null(),
        );
        self.g_argc = Some(g_argc);
        self.g_argv = Some(g_argv);
        (g_argc, g_argv)
    }

    fn ty_str_array(&self) -> inkwell::types::StructType<'ctx> {
        self.ctx.struct_type(
            &[
                self.i64_t.into(),
                self.i8_ptr_t
                    .ptr_type(inkwell::AddressSpace::from(0u16))
                    .into(),
            ],
            false,
        )
    }

    fn ty_cmd_result(&self) -> inkwell::types::StructType<'ctx> {
        self.ctx
            .struct_type(&[self.bool_t.into(), self.i8_ptr_t.into()], false)
    }

    fn ty_handle(&mut self) -> StructType<'ctx> {
        if let Some(ty) = self.handle_ty {
            ty
        } else {
            let ty = self.ctx.struct_type(
                &[self.ctx.i32_type().into(), self.ctx.i32_type().into()],
                false,
            );
            self.handle_ty = Some(ty);
            ty
        }
    }

    fn ty_select_result(&mut self) -> StructType<'ctx> {
        if let Some(ty) = self.select_result_ty {
            ty
        } else {
            let ty = self.ctx.struct_type(
                &[self.i8_ptr_t.into(), self.i64_t.into(), self.i64_t.into()],
                false,
            );
            self.select_result_ty = Some(ty);
            ty
        }
    }

    fn ensure_channel_select_fn(&mut self) -> FunctionValue<'ctx> {
        if let Some(func) = self.module.get_function("seen_channel_select") {
            func
        } else {
            let fn_ty = self.i8_ptr_t.fn_type(
                &[
                    self.i8_ptr_t.into(),
                    self.i8_ptr_t.into(),
                    self.i64_t.into(),
                ],
                false,
            );
            self.module.add_function("seen_channel_select", fn_ty, None)
        }
    }

    fn ensure_scope_fn(&mut self, name: &str) -> FunctionValue<'ctx> {
        if let Some(func) = self.module.get_function(name) {
            func
        } else {
            let fn_ty = self
                .ctx
                .void_type()
                .fn_type(&[self.ctx.i32_type().into()], false);
            self.module.add_function(name, fn_ty, None)
        }
    }

    fn ensure_spawn_fn(&mut self, name: &str) -> FunctionValue<'ctx> {
        if let Some(func) = self.module.get_function(name) {
            func
        } else {
            let ptr_ty = self.ty_handle().ptr_type(inkwell::AddressSpace::from(0u16));
            let fn_ty = ptr_ty.fn_type(&[], false);
            self.module.add_function(name, fn_ty, None)
        }
    }

    fn ensure_task_handle_new_fn(&mut self) -> FunctionValue<'ctx> {
        if let Some(func) = self.module.get_function("__task_handle_new") {
            func
        } else {
            let ptr_ty = self.ty_handle().ptr_type(inkwell::AddressSpace::from(0u16));
            let fn_ty = ptr_ty.fn_type(&[self.ctx.i32_type().into()], false);
            self.module.add_function("__task_handle_new", fn_ty, None)
        }
    }

    fn ensure_await_fn(&mut self) -> FunctionValue<'ctx> {
        if let Some(func) = self.module.get_function("__await") {
            func
        } else {
            let ret_ty = self.ctx.i32_type();
            let arg_ty = self.ty_handle().ptr_type(inkwell::AddressSpace::from(0u16));
            let fn_ty = ret_ty.fn_type(&[arg_ty.into()], false);
            self.module.add_function("__await", fn_ty, None)
        }
    }

    fn cast_handle_ptr(
        &mut self,
        value: BasicValueEnum<'ctx>,
        label: &str,
    ) -> Result<PointerValue<'ctx>> {
        let handle_ptr_ty = self.ty_handle().ptr_type(inkwell::AddressSpace::from(0u16));
        if value.is_pointer_value() {
            self.builder
                .build_pointer_cast(value.into_pointer_value(), handle_ptr_ty, label)
                .map_err(|e| anyhow!("{e:?}"))
        } else if value.is_int_value() {
            self.builder
                .build_int_to_ptr(value.into_int_value(), handle_ptr_ty, label)
                .map_err(|e| anyhow!("{e:?}"))
        } else {
            Err(anyhow!("expected task handle pointer"))
        }
    }

    fn declare_c_fn(
        &self,
        name: &str,
        ret: inkwell::types::BasicTypeEnum<'ctx>,
        params: &[BasicMetadataTypeEnum<'ctx>],
        varargs: bool,
    ) -> FunctionValue<'ctx> {
        if let Some(f) = self.module.get_function(name) {
            return f;
        }
        let ty = ret.fn_type(params, varargs);
        self.module.add_function(name, ty, None)
    }

    fn declare_c_void_fn(
        &self,
        name: &str,
        params: &[BasicMetadataTypeEnum<'ctx>],
        varargs: bool,
    ) -> FunctionValue<'ctx> {
        if let Some(f) = self.module.get_function(name) {
            return f;
        }
        let ty = self.ctx.void_type().fn_type(params, varargs);
        self.module.add_function(name, ty, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handle_struct_is_i32_pair() {
        let mut backend = LlvmBackend::new();
        let ty = backend.ty_handle();
        let fields = ty.get_field_types();
        assert_eq!(fields.len(), 2);
        assert!(fields
            .iter()
            .all(|f| f.into_int_type().get_bit_width() == 32));
    }
}
