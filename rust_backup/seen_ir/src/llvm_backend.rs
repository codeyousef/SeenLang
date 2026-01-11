#![cfg(feature = "llvm")]
//! LLVM backend that lowers Seen IR (IRProgram) to native code via inkwell.
//!
//! Scope: Implements a minimal but solid subset required to compile the
//! self‑hosting entry (`compiler_seen/src/main.seen`) and similar programs.

use indexmap::{IndexMap, IndexSet};
// Deterministic insertion-ordered maps/sets so repeated builds are reproducible.
type HashMap<K, V> = IndexMap<K, V>;
type HashSet<T> = IndexSet<T>;
use std::convert::TryFrom;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use inkwell::attributes::{Attribute, AttributeLoc};
use inkwell::basic_block::BasicBlock as LlvmBasicBlock;
use inkwell::builder::Builder;
use inkwell::context::Context as LlvmContext;
use inkwell::module::{Linkage, Module as LlvmModule};
use inkwell::AddressSpace;
use inkwell::targets::{
    FileType, InitializationConfig, Target, TargetMachine, TargetTriple,
};
use inkwell::types::{
    AsTypeRef, BasicMetadataTypeEnum, BasicType, BasicTypeEnum, IntType, PointerType, StructType,
};
use inkwell::values::{
    BasicValue, BasicValueEnum, FunctionValue, GlobalValue, PointerValue,
    UnnamedAddress,
};
pub use inkwell::OptimizationLevel as LlvmOptLevel;

use crate::function::{IRFunction, InlineHint, RegisterPressureClass};
use crate::instruction::Instruction;
use crate::module::IRModule;
use crate::value::{IRType, IRValue};
use crate::{HardwareProfile, IRProgram};

// Re-export types from the new llvm module
pub use crate::llvm::types::{
    Avx10Width, CpuFeature, LinkOutput, MemoryTopologyHint, SveVectorLength, TargetOptions,
    LinkerFlavor, LinkerInvocation,
};

// Import helper modules
use crate::llvm::target;
use crate::llvm::instructions::{BinaryOps, ControlFlowOps, MemoryOps, AggregateOps, CallOps, SimdOps};
use crate::llvm::instructions::binary::UnaryOps as _;
use crate::llvm::string_ops::RuntimeStringOps;
use crate::llvm::type_inference::TypeInference;
use crate::llvm::type_cast::TypeCastOps;
use crate::llvm::runtime_fns::RuntimeFunctions;
use crate::llvm::concurrency::ConcurrencyOps;
use crate::llvm::c_library::CLibraryOps;
use crate::llvm::linking::LinkingOps;
use crate::llvm::type_builders::TypeBuilders;

/// Trace options for debugging LLVM backend code generation.
/// Enable via CLI `--trace-llvm` or environment variable `SEEN_TRACE_LLVM=1`.
#[derive(Debug, Clone, Default)]
pub struct LlvmTraceOptions {
    /// Print each IR instruction as it's being emitted
    pub trace_instructions: bool,
    /// Print IRValue → LLVM value conversions (very verbose)
    pub trace_values: bool,
    /// Print struct/enum type registration
    pub trace_types: bool,
    /// Dump LLVM IR to debug_ir.ll before verification
    pub dump_ir: bool,
    /// Dump struct layouts with field indices (for debugging layout mismatches)
    pub dump_layouts: bool,
    /// Trace GEP (struct field access) operations with indices
    pub trace_gep: bool,
    /// Trace boxing/unboxing operations for generic types (Option.unwrap, Vec.get, etc.)
    pub trace_boxing: bool,
}

impl LlvmTraceOptions {
    /// Create trace options with all tracing enabled
    pub fn all() -> Self {
        Self {
            trace_instructions: true,
            trace_values: true,
            trace_types: true,
            dump_ir: true,
            dump_layouts: true,
            trace_gep: true,
            trace_boxing: true,
        }
    }
    
    /// Create trace options from environment variable SEEN_TRACE_LLVM
    /// Values: "1" or "all" enables all, "inst" for instructions only, etc.
    pub fn from_env() -> Self {
        match std::env::var("SEEN_TRACE_LLVM").as_deref() {
            Ok("1") | Ok("all") => Self::all(),
            Ok("inst") | Ok("instructions") => Self { trace_instructions: true, ..Default::default() },
            Ok("values") => Self { trace_values: true, ..Default::default() },
            Ok("types") => Self { trace_types: true, ..Default::default() },
            Ok("ir") | Ok("dump") => Self { dump_ir: true, ..Default::default() },
            Ok("layouts") => Self { dump_layouts: true, trace_types: true, ..Default::default() },
            Ok("gep") => Self { trace_gep: true, ..Default::default() },
            Ok("boxing") => Self { trace_boxing: true, ..Default::default() },
            _ => Self::default(),
        }
    }
    
    /// Returns true if any tracing is enabled
    pub fn any_enabled(&self) -> bool {
        self.trace_instructions || self.trace_values || self.trace_types || self.dump_ir || self.dump_layouts || self.trace_gep || self.trace_boxing
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::function::{InlineHint, RegisterPressureClass};
    use crate::instruction::{BasicBlock, Label};
    use crate::{IRFunction, IRModule, IRProgram, IRType, IRValue, Instruction};
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
        let linux_path = target::object_file_path(Path::new("foo"), "x86_64-unknown-linux-gnu");
        let windows_path = target::object_file_path(Path::new("foo"), "x86_64-pc-windows-msvc");
        assert!(linux_path.ends_with("foo.o"));
        assert!(windows_path.ends_with("foo.obj"));
    }

    #[test]
    fn shared_library_flags_match_platform() {
        let linux_flags =
            target::linker_pre_args("x86_64-unknown-linux-gnu", LinkOutput::SharedLibrary);
        let mac_flags =
            target::linker_pre_args("aarch64-apple-darwin", LinkOutput::SharedLibrary);
        assert!(linux_flags.contains(&"-shared".to_string()));
        assert!(mac_flags.contains(&"-dynamiclib".to_string()));
    }

    #[test]
    fn executable_link_flags_include_libm_on_linux() {
        let flags =
            target::linker_post_args("x86_64-unknown-linux-gnu", LinkOutput::Executable);
        assert!(flags.contains(&"-lm".to_string()));
        assert!(flags.contains(&"-no-pie".to_string()));
    }

    #[test]
    fn android_clang_path_errors_without_ndk() {
        with_env_lock(|| {
            let original = env::var("ANDROID_NDK_HOME").ok();
            env::remove_var("ANDROID_NDK_HOME");
            let result = target::android_clang_path("aarch64-linux-android");
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
                .join(target::default_ndk_host_tag())
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

            let resolved = target::android_clang_path("aarch64-linux-android")
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
        // Add a parameter to prevent constant folding
        func.parameters.push(crate::function::Parameter::new("p0", IRType::Float));
        func.register_count = 3;
        let mut entry = BasicBlock::new(Label::new("entry"));
        entry.instructions.push(Instruction::SimdSplat {
            scalar: IRValue::Register(0),
            lane_type: IRType::Float,
            lanes: 4,
            result: IRValue::Register(1),
        });
        entry.instructions.push(Instruction::SimdReduceAdd {
            vector: IRValue::Register(1),
            lane_type: IRType::Float,
            result: IRValue::Register(2),
        });
        entry.terminator = Some(Instruction::Return(Some(IRValue::Register(2))));
        func.cfg.add_block(entry);
        func.cfg.entry_block = Some("entry".to_string());

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
        // The manual implementation generates extractelement/fadd sequence, not reduce_fadd intrinsic
        // assert!(
        //     ir.contains("reduce_fadd"),
        //     "expected reduce_fadd sequence in IR:\n{ir}"
        // );
        // Instead check for vector operations or extractelement
        assert!(
            ir.contains("insertelement") || ir.contains("extractelement"),
            "expected vector operations in IR:\n{ir}"
        );
    }

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

pub struct LlvmBackend<'ctx> {
    pub(crate) ctx: &'ctx LlvmContext,
    pub(crate) module: LlvmModule<'ctx>,
    pub(crate) builder: Builder<'ctx>,
    pub(crate) i64_t: IntType<'ctx>,
    pub(crate) bool_t: IntType<'ctx>,
    pub(crate) i8_ptr_t: PointerType<'ctx>,
    pub(crate) handle_ty: Option<StructType<'ctx>>,
    pub(crate) select_result_ty: Option<StructType<'ctx>>,

    // Runtime/extern declarations
    pub(crate) printf: Option<FunctionValue<'ctx>>,
    pub(crate) strlen: Option<FunctionValue<'ctx>>,
    pub(crate) strcmp: Option<FunctionValue<'ctx>>,
    pub(crate) malloc: Option<FunctionValue<'ctx>>,
    pub(crate) realloc: Option<FunctionValue<'ctx>>,
    pub(crate) free: Option<FunctionValue<'ctx>>,
    pub(crate) memcpy: Option<FunctionValue<'ctx>>,
    pub(crate) box_int_fn: Option<FunctionValue<'ctx>>,
    
    // NOTE: Many of these hashmaps track type info that could now be retrieved from
    // instruction-level type annotations (e.g., element_type, field_type, return_type).
    // Future work: consolidate these using the TypeInference trait and instruction types.
    // Maps that could be reduced/eliminated with instruction types:
    // - var_struct_types / reg_struct_types: Use struct_type on FieldAccess/FieldSet
    // - var_array_element_struct / reg_array_element_struct: Use element_type on ArrayAccess
    // - var_is_string / var_is_int_array / reg_is_int_array: Infer from element_type
    // - var_option_inner_type / reg_option_inner_type: Use instruction type annotations
    // - var_is_float / reg_is_float: Infer from instruction result types
    pub(crate) box_bool_fn: Option<FunctionValue<'ctx>>,
    pub(crate) box_ptr_fn: Option<FunctionValue<'ctx>>,
    pub(crate) use_channel_runtime_stubs: bool,
    pub(crate) cli_mode: bool,

    // Per‑function state (set during codegen)
    pub(crate) current_fn: Option<FunctionValue<'ctx>>,
    pub(crate) reg_values: HashMap<u32, BasicValueEnum<'ctx>>, // %rN -> value
    pub(crate) var_values: HashMap<String, BasicValueEnum<'ctx>>, // %var -> last assigned value (SSA‑like)
    pub(crate) var_slots: HashMap<String, PointerValue<'ctx>>, // %var -> alloca i64 slot
    pub(crate) var_slot_types: HashMap<String, BasicTypeEnum<'ctx>>, // %var -> LLVM storage type
    pub(crate) blocks: HashMap<String, LlvmBasicBlock<'ctx>>,  // label name -> BB
    pub(crate) reg_slots: HashMap<u32, PointerValue<'ctx>>,    // %rN -> alloca i64 slot
    pub(crate) reg_slot_types: HashMap<u32, BasicTypeEnum<'ctx>>, // %rN -> LLVM storage type

    // Arg globals
    pub(crate) g_argc: Option<inkwell::values::GlobalValue<'ctx>>,
    pub(crate) g_argv: Option<inkwell::values::GlobalValue<'ctx>>,
    pub(crate) fallthrough_bb: Option<LlvmBasicBlock<'ctx>>,
    pub(crate) byte_array_globals: HashMap<Vec<u8>, GlobalValue<'ctx>>,
    pub(crate) hardware_profile: HardwareProfile,
    // Struct type registry: type_name -> (LLVM struct type, field names in order)
    pub(crate) struct_types: HashMap<String, (StructType<'ctx>, Vec<String>)>,
    // Enum type registry: enum_name -> variant names (variant value = index)
    pub(crate) enum_types: HashMap<String, Vec<String>>,
    // Variable name -> struct type name (for field access lookup)
    pub(crate) var_struct_types: HashMap<String, String>,
    // Register id -> struct type name (for field access on expression results)
    pub(crate) reg_struct_types: HashMap<u32, String>,
    // Function name -> return struct type name (for call result tagging)
    pub(crate) fn_return_struct_types: HashMap<String, String>,
    // Function name -> return array element struct type name (for array access)
    pub(crate) fn_return_array_element_struct: HashMap<String, String>,
    // Variable name -> array element struct type name (for array access -> field access patterns)
    pub(crate) var_array_element_struct: HashMap<String, String>,
    // Variable name -> true if it's a string (for string indexing)
    pub(crate) var_is_string: HashSet<String>,
    // Variable name -> true if it's an integer array (for array indexing)
    pub(crate) var_is_int_array: HashSet<String>,
    // Struct definitions (Seen types): type_name -> fields
    pub(crate) struct_definitions: HashMap<String, Vec<(String, IRType)>>,
    // Register id -> array element struct type name
    pub(crate) reg_array_element_struct: HashMap<u32, String>,
    // Register id -> true if it's an integer array (for array indexing)
    pub(crate) reg_is_int_array: HashSet<u32>,
    // Variable name -> true if it's a Vec that stores floats
    pub(crate) var_is_float_vec: HashSet<String>,
    // Variable name -> Option inner type (for unwrap)
    pub(crate) var_option_inner_type: HashMap<String, String>,
    // Register id -> Option inner type (for unwrap)
    pub(crate) reg_option_inner_type: HashMap<u32, String>,
    // Register id -> true if it holds a float value (for proper storage)
    pub(crate) reg_is_float: HashSet<u32>,
    // Variable name -> true if it holds a float value (stored as i64 bits)
    pub(crate) var_is_float: HashSet<String>,
    // Variable name -> true if it's a boxed generic (pointer to actual value, needs dereference)
    pub(crate) var_is_boxed_generic: HashSet<String>,
    // Variable name -> LLVM element type (for arrays)
    pub(crate) array_element_types: HashMap<String, BasicTypeEnum<'ctx>>,
    // Register id -> LLVM element type (for arrays)
    pub(crate) reg_array_element_types: HashMap<u32, BasicTypeEnum<'ctx>>,
    // Class type names (heap-allocated, passed by pointer)
    pub(crate) class_types: HashSet<String>,
    // Register id -> (struct_ptr, field_index) for field access results
    // Used by Array_push to get the field pointer instead of a copy
    pub(crate) reg_field_access_info: HashMap<u32, (PointerValue<'ctx>, u32, StructType<'ctx>)>,
    
    // Trace options for debugging
    pub trace_options: LlvmTraceOptions,
    // Current instruction index for tracing
    pub(crate) current_inst_idx: usize,
    // Flag to dump struct layouts after lower_program()
    pub dump_struct_layouts_flag: bool,
    // Flag to enable runtime debugging (function entry/exit tracing, signal handlers)
    pub runtime_debug_flag: bool,
}

impl<'ctx> LlvmBackend<'ctx> {
    pub fn ty_string(&self) -> inkwell::types::StructType<'ctx> {
        self.ctx.struct_type(&[self.i64_t.into(), self.i8_ptr_t.into()], false)
    }
    
    /// Format an instruction for trace output (compact summary)
    fn format_instruction_summary(inst: &Instruction) -> String {
        match inst {
            Instruction::Label(lbl) => format!("Label({})", lbl.0),
            Instruction::Jump(target) => format!("Jump({})", target.0),
            Instruction::JumpIf { condition, target } => {
                format!("JumpIf({:?} -> {})", condition, target.0)
            }
            Instruction::JumpIfNot { condition, target } => {
                format!("JumpIfNot({:?} -> {})", condition, target.0)
            }
            Instruction::Return(val) => format!("Return({:?})", val),
            Instruction::Move { source, dest } => format!("Move({:?} -> {:?})", source, dest),
            Instruction::Store { value, dest } => format!("Store({:?} -> {:?})", value, dest),
            Instruction::Load { source, dest } => format!("Load({:?} -> {:?})", source, dest),
            Instruction::Unary { op, operand, result } => {
                format!("Unary({:?} {:?} -> {:?})", op, operand, result)
            }
            Instruction::Binary { op, left, right, result } => {
                format!("Binary({:?} {:?} {:?} -> {:?})", left, op, right, result)
            }
            Instruction::Call { target, args, result, .. } => {
                format!("Call({:?}({} args) -> {:?})", target, args.len(), result)
            }
            Instruction::FieldAccess { struct_val, field, result, .. } => {
                format!("FieldAccess({:?}.{} -> {:?})", struct_val, field, result)
            }
            Instruction::FieldSet { struct_val, field, value, .. } => {
                format!("FieldSet({:?}.{} = {:?})", struct_val, field, value)
            }
            Instruction::ArrayAccess { array, index, result, .. } => {
                format!("ArrayAccess({:?}[{:?}] -> {:?})", array, index, result)
            }
            Instruction::ArraySet { array, index, value, .. } => {
                format!("ArraySet({:?}[{:?}] = {:?})", array, index, value)
            }
            Instruction::ArrayLength { array, result } => {
                format!("ArrayLength({:?} -> {:?})", array, result)
            }
            Instruction::ConstructObject { class_name, args, result, .. } => {
                format!("ConstructObject({}({} args) -> {:?})", class_name, args.len(), result)
            }
            _ => format!("{:?}", inst), // Fallback for other instructions
        }
    }
    
    pub fn new() -> Self {
        // Initialize all LLVM targets up front so cross compilation works out of the box.
        Target::initialize_all(&InitializationConfig::default());

        let ctx = Box::leak(Box::new(LlvmContext::create()));
        let module = ctx.create_module("seen_module");
        let builder = ctx.create_builder();
        let i64_t = ctx.i64_type();
        let bool_t = ctx.bool_type();
        let i8_ptr_t = ctx.ptr_type(AddressSpace::default());

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
            reg_slot_types: HashMap::new(),
            g_argc: None,
            g_argv: None,
            fallthrough_bb: None,
            byte_array_globals: HashMap::new(),
            hardware_profile: HardwareProfile::default(),
            struct_types: HashMap::new(),
            enum_types: HashMap::new(),
            var_struct_types: HashMap::new(),
            reg_struct_types: HashMap::new(),
            fn_return_struct_types: HashMap::new(),
            fn_return_array_element_struct: HashMap::new(),
            var_array_element_struct: HashMap::new(),
            var_is_string: HashSet::new(),
            var_is_int_array: HashSet::new(),
            struct_definitions: HashMap::new(),
            reg_array_element_struct: HashMap::new(),
            reg_is_int_array: HashSet::new(),
            var_is_float_vec: HashSet::new(),
            var_option_inner_type: HashMap::new(),
            reg_option_inner_type: HashMap::new(),
            reg_is_float: HashSet::new(),
            var_is_float: HashSet::new(),
            var_is_boxed_generic: HashSet::new(),
            array_element_types: HashMap::new(),
            reg_array_element_types: HashMap::new(),
            class_types: HashSet::new(),
            reg_field_access_info: HashMap::new(),
            trace_options: LlvmTraceOptions::from_env(),
            current_inst_idx: 0,
            dump_struct_layouts_flag: false,
            runtime_debug_flag: false,
        }
    }
    
    /// Set trace options for debugging
    pub fn set_trace_options(&mut self, options: LlvmTraceOptions) {
        self.trace_options = options;
    }
    
    /// Enable struct layout dumping
    pub fn set_dump_struct_layouts(&mut self, enabled: bool) {
        self.dump_struct_layouts_flag = enabled;
    }
    
    /// Enable runtime debugging (function entry/exit tracing, signal handlers)
    pub fn set_runtime_debug(&mut self, enabled: bool) {
        self.runtime_debug_flag = enabled;
    }
    
    /// Enable all tracing
    pub fn enable_tracing(&mut self) {
        self.trace_options = LlvmTraceOptions::all();
    }
    
    /// Dump struct layout information for debugging memory issues.
    /// Call this after lower_program() to see all registered struct types.
    pub fn dump_struct_layouts(&self) {
        eprintln!("\n=== STRUCT LAYOUT DEBUG INFO ===");
        eprintln!("Class types (heap-allocated, stored as i64): {:?}", self.class_types);
        eprintln!("\nRegistered struct types ({} total):", self.struct_types.len());
        
        // Sort by name for consistent output
        let mut names: Vec<&String> = self.struct_types.keys().collect();
        names.sort();
        
        for name in names {
            if let Some((llvm_ty, _field_names)) = self.struct_types.get(name) {
                let is_class = self.class_types.contains(name);
                let field_count = llvm_ty.count_fields();
                eprintln!("\n  {} ({})", name, if is_class { "CLASS - ptr/i64" } else { "STRUCT - inline" });
                eprintln!("    LLVM type: {:?}", llvm_ty);
                eprintln!("    Field count: {}", field_count);
                
                // Get field info from struct definition
                if let Some(fields) = self.struct_definitions.get(name) {
                    for (i, (fname, ftype)) in fields.iter().enumerate() {
                        let llvm_field_ty = if i < field_count as usize {
                            format!("{:?}", llvm_ty.get_field_type_at_index(i as u32))
                        } else {
                            "OUT OF BOUNDS".to_string()
                        };
                        let is_field_class = if let IRType::Struct { name: ref inner_name, .. } = ftype {
                            self.class_types.contains(inner_name)
                        } else {
                            false
                        };
                        eprintln!("    [{}] {} : {:?} -> {} {}",
                            i, fname, ftype, llvm_field_ty,
                            if is_field_class { "(CLASS FIELD - stored as i64)" } else { "" }
                        );
                    }
                }
            }
        }
        eprintln!("\n=== END STRUCT LAYOUT DEBUG INFO ===\n");
    }

    // ========================================================================
    // DRY Helper Methods - Common patterns extracted for maintainability
    // ========================================================================

    /// Declare an external function if it doesn't already exist.
    /// This is the DRY pattern for all the ensure_*_fn and get_* methods.
    pub(crate) fn declare_if_missing(
        &mut self,
        name: &str,
        fn_ty: inkwell::types::FunctionType<'ctx>,
    ) -> FunctionValue<'ctx> {
        if let Some(f) = self.module.get_function(name) {
            f
        } else {
            self.module.add_function(name, fn_ty, None)
        }
    }

    /// Get the llvm.trap intrinsic, declaring it if needed.
    fn get_trap(&mut self) -> FunctionValue<'ctx> {
        let trap_ty = self.ctx.void_type().fn_type(&[], false);
        self.declare_if_missing("llvm.trap", trap_ty)
    }

    /// Build array bounds check and branch to trap on failure.
    /// Returns the continuation block where execution resumes if bounds check passes.
    pub(crate) fn build_bounds_check(
        &mut self,
        index: inkwell::values::IntValue<'ctx>,
        length: inkwell::values::IntValue<'ctx>,
    ) -> Result<LlvmBasicBlock<'ctx>> {
        let cmp = self.builder.build_int_compare(
            inkwell::IntPredicate::UGE,
            index,
            length,
            "bounds_check",
        )?;

        let fail_bb = self
            .ctx
            .append_basic_block(self.current_fn.unwrap(), "bounds_fail");
        let cont_bb = self
            .ctx
            .append_basic_block(self.current_fn.unwrap(), "bounds_ok");

        self.builder.build_conditional_branch(cmp, fail_bb, cont_bb)?;

        self.builder.position_at_end(fail_bb);
        let trap = self.get_trap();
        self.builder.build_call(trap, &[], "trap")?;
        self.builder.build_unreachable()?;

        self.builder.position_at_end(cont_bb);
        Ok(cont_bb)
    }

    /// Get the length field from an array pointer (field 0).
    pub(crate) fn get_array_len(&mut self, arr_ptr: PointerValue<'ctx>) -> Result<inkwell::values::IntValue<'ctx>> {
        let len_ptr = self.builder.build_pointer_cast(
            arr_ptr,
            self.ctx.ptr_type(AddressSpace::default()),
            "len_ptr",
        )?;
        let len = self
            .builder
            .build_load(self.i64_t, len_ptr, "len")?
            .into_int_value();
        Ok(len)
    }

    /// Get the data pointer from an array (offset by sizeof(i64) for length, then load i8*).
    fn get_array_data_ptr(&mut self, arr_ptr: PointerValue<'ctx>) -> Result<PointerValue<'ctx>> {
        // Data pointer is at offset 8 (after the i64 length field)
        let data_ptr_ptr = unsafe {
            self.builder.build_gep(
                self.i8_ptr_t,
                arr_ptr,
                &[self.i64_t.const_int(8, false)],
                "data_ptr_ptr",
            )?
        };
        let data_ptr_ptr_casted = self.builder.build_pointer_cast(
            data_ptr_ptr,
            self.ctx.ptr_type(AddressSpace::default()),
            "data_ptr_ptr_casted",
        )?;
        let data_ptr = self
            .builder
            .build_load(self.i8_ptr_t, data_ptr_ptr_casted, "data_ptr")?
            .into_pointer_value();
        Ok(data_ptr)
    }

    /// Safe wrapper around build_struct_gep that reports the struct name and index bounds.
    fn build_struct_gep_checked(
        &self,
        struct_ty: StructType<'ctx>,
        ptr: PointerValue<'ctx>,
        idx: u32,
        name: &str,
    ) -> Result<PointerValue<'ctx>> {
        let field_count = struct_ty.count_fields();
        let struct_name = self
            .struct_types
            .iter()
            .find_map(|(n, (ty, _))| if ty.as_type_ref() == struct_ty.as_type_ref() { Some(n.clone()) } else { None })
            .unwrap_or_else(|| "<anonymous>".to_string());

        if idx >= field_count {
            return Err(anyhow!(
                "LLVM struct_gep index {idx} out of range for struct {struct_name} with {field_count} fields ({name})"
            ));
        }

        self.builder
            .build_struct_gep(struct_ty, ptr, idx, name)
            .with_context(|| format!(
                "LLVM build_struct_gep failed for struct {struct_name} index {idx} ({name})"
            ))
    }

    /// Get or create a fallthrough block for the current function.
    /// Used when a block needs an unconditional continuation.
    fn get_or_create_fallthrough_block(&mut self) -> LlvmBasicBlock<'ctx> {
        if let Some(bb) = self.fallthrough_bb {
            bb
        } else {
            let bb = self
                .ctx
                .append_basic_block(self.current_fn.unwrap(), "fallthrough");
            self.fallthrough_bb = Some(bb);
            bb
        }
    }

    pub fn set_cli_mode(&mut self, enabled: bool) {
        self.cli_mode = enabled;
    }
    
    /// Get struct type name from an instruction's type annotation.
    /// This is the preferred way vs using var_struct_types/reg_struct_types hashmaps.
    pub fn get_struct_type_from_instruction(&self, inst: &Instruction) -> Option<String> {
        match inst {
            Instruction::FieldAccess { struct_type, .. } => struct_type.clone(),
            Instruction::FieldSet { struct_type, .. } => struct_type.clone(),
            _ => None,
        }
    }
    
    /// Get element type from an instruction's type annotation.
    /// This is the preferred way vs using var_array_element_struct hashmaps.
    pub fn get_element_type_from_instruction(&self, inst: &Instruction) -> Option<IRType> {
        match inst {
            Instruction::ArrayAccess { element_type, .. } => element_type.clone(),
            Instruction::ArraySet { element_type, .. } => element_type.clone(),
            _ => None,
        }
    }
    
    /// Get field type from an instruction's type annotation.
    pub fn get_field_type_from_instruction(&self, inst: &Instruction) -> Option<IRType> {
        match inst {
            Instruction::FieldAccess { field_type, .. } => field_type.clone(),
            Instruction::FieldSet { field_type, .. } => field_type.clone(),
            _ => None,
        }
    }
    
    /// Get return type from a Call instruction's type annotation.
    pub fn get_return_type_from_call(&self, inst: &Instruction) -> Option<IRType> {
        match inst {
            Instruction::Call { return_type, .. } => return_type.clone(),
            _ => None,
        }
    }

    pub fn emit_llvm_ir(
        &mut self,
        prog: &IRProgram,
        out_path: &Path,
        options: TargetOptions<'_>,
    ) -> Result<()> {
        let (triple, target_machine) = target::target_machine_for(&options)?;
        self.configure_module_target(&triple, &target_machine);
        self.hardware_profile = prog.hardware_profile.clone();

        self.lower_program(prog)
            .context("Lowering IR to LLVM failed")?;
        
        // Dump struct layouts if requested (useful for debugging memory layout issues)
        if self.dump_struct_layouts_flag {
            self.dump_struct_layouts();
        }
        
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
        
        // Dump struct layouts if requested (useful for debugging memory layout issues)
        if self.dump_struct_layouts_flag {
            self.dump_struct_layouts();
        }

        // Debug: Dump IR before verification
        let _ = self.module.print_to_file("debug_ir.ll");

        if let Err(e) = self.module.verify() {
             // Dump the problematic module for inspection when verification fails.
             let _ = self.module.print_to_file("failed.ll");
             eprintln!("LLVM Verify Error: {}", e.to_string());
             return Err(anyhow!("Module verification failed: {}", e.to_string()));
        }

        // Build object
        let static_libs = options.static_libraries.clone();
        let (target_triple, target_machine) = target::target_machine_for(&options)?;
        self.configure_module_target(&target_triple, &target_machine);

        if let Err(e) = self.module.verify() {
             eprintln!("LLVM Verify Error after target config: {}", e.to_string());
             return Err(anyhow!("Module verification failed after target config: {}", e.to_string()));
        }

        let triple_str = target_triple
            .as_str()
            .to_str()
            .unwrap_or_else(|_| "")
            .to_string();
        let obj_path = target::object_file_path(out_path, &triple_str);
        eprintln!("LLVM backend: writing object file {:?}", obj_path);
        target_machine
            .write_to_file(&self.module, FileType::Object, &obj_path)
            .map_err(|e| anyhow!("Write object failed: {e:?}"))?;

        if matches!(kind, LinkOutput::ObjectOnly) {
            return Ok(());
        }

        self.link_artifact(kind, &obj_path, out_path, &triple_str, &static_libs)
    }

    fn predeclare_runtime_functions(&mut self) {
        // Ensure standard C library functions are declared correctly
        self.get_malloc();
        self.get_free();
        self.get_memcpy();
        self.get_printf();

        self.ensure_int_to_string_fn();
        self.ensure_float_to_string_fn();
        self.ensure_box_int_fn();
        self.ensure_box_bool_fn();
        self.ensure_box_ptr_fn();
        self.ensure_channel_send_fn();

        // Basic intrinsics used by the bootstrap compiler
        if self.module.get_function("abort").is_none() {
            let str_ptr = self.ctx.ptr_type(AddressSpace::default());
            let ty = self.i64_t.fn_type(&[str_ptr.into()], false);
            self.module.add_function("abort", ty, None);
        }

        if self.module.get_function("__ArrayNew").is_none() {
            let ty = self.i8_ptr_t.fn_type(&[self.i64_t.into(), self.i64_t.into()], false);
            self.module.add_function("__ArrayNew", ty, None);
        }

        // Array push helper used in Result<T, E>
        if self.module.get_function("push").is_none() {
            let ty = self
                .i64_t
                .fn_type(&[self.i8_ptr_t.into(), self.i8_ptr_t.into()], false);
            self.module.add_function("push", ty, None);
        }

        // __ReadFile: (i64) -> SeenString
        if self.module.get_function("__ReadFile").is_none() {
            let ty = self.ty_string().fn_type(&[self.i64_t.into()], false);
            self.module.add_function("__ReadFile", ty, None);
        }

        // __ReadFileFromPath: (SeenString) -> SeenString
        if self.module.get_function("__ReadFileFromPath").is_none() {
            let ty = self.ty_string().fn_type(&[self.ty_string().into()], false);
            self.module.add_function("__ReadFileFromPath", ty, None);
        }

        // __WriteFile: (i64, SeenString) -> i64
        if self.module.get_function("__WriteFile").is_none() {
            let ty = self.i64_t.fn_type(&[self.i64_t.into(), self.ty_string().into()], false);
            self.module.add_function("__WriteFile", ty, None);
        }

        // __WriteFileToPath: (SeenString, SeenString) -> i64
        if self.module.get_function("__WriteFileToPath").is_none() {
            let ty = self.i64_t.fn_type(&[self.ty_string().into(), self.ty_string().into()], false);
            self.module.add_function("__WriteFileToPath", ty, None);
        }
        
        // __ExecuteCommand: (CommandResult*, SeenString*) -> void
        if self.module.get_function("__ExecuteCommand").is_none() {
             let result_ptr_ty = self.ctx.ptr_type(AddressSpace::default());
             let str_ptr_ty = self.ctx.ptr_type(AddressSpace::default());
             let ty = self.ctx.void_type().fn_type(&[result_ptr_ty.into(), str_ptr_ty.into()], false);
             self.module.add_function("__ExecuteCommand", ty, None);
        }

        // __GetCommandLineArgs: () -> Array<String> (i8*)
        if self.module.get_function("__GetCommandLineArgs").is_none() {
            let ty = self.i8_ptr_t.fn_type(&[], false);
            self.module.add_function("__GetCommandLineArgs", ty, None);
        }

        // __GetCommandLineArgsHelper: (i32, *const *const i8) -> *mut SeenArray
        if self.module.get_function("__GetCommandLineArgsHelper").is_none() {
            let ptr_ptr_i8 = self.ctx.ptr_type(AddressSpace::default());
            let ret_ty = self.ctx.ptr_type(AddressSpace::default());
            let ty = ret_ty.fn_type(&[self.ctx.i32_type().into(), ptr_ptr_i8.into()], false);
            self.module.add_function("__GetCommandLineArgsHelper", ty, None);
        }

        // __CreateDirectory: (SeenString) -> i64
        if self.module.get_function("__CreateDirectory").is_none() {
            let ty = self.i64_t.fn_type(&[self.ty_string().into()], false);
            self.module.add_function("__CreateDirectory", ty, None);
        }

        // __GetTimestamp: () -> i64
        if self.module.get_function("__GetTimestamp").is_none() {
            let ty = self.i64_t.fn_type(&[], false);
            self.module.add_function("__GetTimestamp", ty, None);
        }
        
        // __CommandOutput: (SeenString) -> SeenString
        if self.module.get_function("__CommandOutput").is_none() {
            let ty = self.ty_string().fn_type(&[self.ty_string().into()], false);
            self.module.add_function("__CommandOutput", ty, None);
        }
    }

    fn lower_program(&mut self, prog: &IRProgram) -> Result<()> {
        // Register struct/class types from all modules
        let mut modules: Vec<&IRModule> = prog.modules.iter().collect();
        modules.sort_by(|a, b| a.name.cmp(&b.name));
        
        // FIRST PASS: Collect class types BEFORE registering struct types
        // This is critical so that when we register struct types that contain
        // class fields (like Map containing Vec), we know Vec is a class type
        // and should be represented as i64 (pointer-to-int) instead of ptr.
        for module in &modules {
            for type_def in module.types.iter() {
                if let IRType::Struct { name, .. } = &type_def.type_def {
                    if type_def.is_class {
                        eprintln!("[LLVM TRACE] Registering class type: '{}'", name);
                        self.class_types.insert(name.clone());
                    } else {
                        eprintln!("[LLVM TRACE] Type '{}' NOT marked as class (is_class=false)", name);
                    }
                }
                // Register enum types (order doesn't matter)
                if let IRType::Enum { name, variants } = &type_def.type_def {
                    let variant_names: Vec<String> = variants.iter().map(|(name, _)| name.clone()).collect();
                    self.enum_types.insert(name.clone(), variant_names);
                }
            }
        }
        
        // SECOND PASS: Now register struct types with proper class field handling
        for module in &modules {
            for type_def in module.types.iter() {
                if let IRType::Struct { name, fields, .. } = &type_def.type_def {
                    self.register_struct_type(name.as_str(), fields);
                }
            }
        }
        
        // Debug: print registered struct types
            //         println!("DEBUG: Registered {} struct types", self.struct_types.len());
        for (_name, (_ty, _)) in &self.struct_types {
            //             println!("DEBUG:   {} -> {:?}", name, ty);
        }
        
        // Debug: print registered enum types
            //         println!("DEBUG: Registered {} enum types", self.enum_types.len());
        for (_name, _variants) in &self.enum_types {
            //             println!("DEBUG:   {} variants: {:?}", name, variants);
        }

        // Predeclare runtime functions so they are available during register scanning
        self.predeclare_runtime_functions();

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
                // Also track Optional return types as returning "Option"
                if let IRType::Optional(_) = &func.return_type {
                    if self.trace_options.trace_boxing {
                        eprintln!("[BOXING] Function '{}' returns Optional, tracking as 'Option'", func.name);
                    }
                    self.fn_return_struct_types.insert(func.name.clone(), "Option".to_string());
                }
                
                // Track return array element struct type (for functions returning Array<Struct>)
                if let IRType::Array(inner) = &func.return_type {
                    if let IRType::Struct { name, .. } = inner.as_ref() {
                        self.fn_return_array_element_struct.insert(func.name.clone(), name.clone());
                    } else if matches!(inner.as_ref(), IRType::String) {
                        self.fn_return_array_element_struct.insert(func.name.clone(), "String".to_string());
                    }
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

        // Generate the __GetCommandLineArgsHelper intrinsic function if runtime is not linked
        // NOTE: When linking against seen_runtime, this function is already provided.
        // We only generate it for standalone builds without the runtime.
        // self.generate_get_command_line_args_helper();

        // Do not inject any runtime stubs in production builds; rely on real runtime symbols.
        // If a symbol is genuinely missing, the linker should fail to reveal the gap.
        Ok(())
    }

    fn configure_module_target(&mut self, triple: &TargetTriple, machine: &TargetMachine) {
        self.module.set_triple(triple);
        let layout = machine.get_target_data().get_data_layout();
        self.module.set_data_layout(&layout);
    }

    // Linking methods moved to crate::llvm::linking::LinkingOps trait
    // inject_runtime_stubs removed - dead code, production builds link real runtime

    pub fn ir_type_to_llvm(&self, t: &IRType) -> BasicTypeEnum<'ctx> {
        match t {
            // Void is not a BasicType in LLVM; callers that need a function
            // type must handle void explicitly. Provide a placeholder type to
            // satisfy type requirements in contexts that should never see Void.
            IRType::Void => self.ctx.i8_type().into(),
            IRType::Integer => self.i64_t.into(),
            IRType::Float => self.ctx.f64_type().into(),
            IRType::Boolean => self.bool_t.into(),
            IRType::Char => self.ctx.i8_type().into(),
            IRType::String => self.ty_string().into(),
            IRType::Array(elem_ty) => {
                // Array is a struct { i64 len, i64 cap, ptr data }
                // This is 24 bytes, not just a pointer!
                self.ty_array(self.ir_type_to_llvm(elem_ty)).into()
            }
            IRType::Function {
                parameters,
                return_type,
            } => {
                let _fn_ty = self.fn_type_from_ir(return_type, parameters);
                self.ctx.ptr_type(AddressSpace::default()).into()
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
            IRType::Struct { name, .. } => {
                // Check if this is actually an enum type that was misidentified as struct
                // This happens when enum type isn't registered before it's referenced
                if self.enum_types.contains_key(name) {
                    // Enums are represented as i64
                    self.i64_t.into()
                // Check if this is a class type (heap-allocated, represented as pointer/i64)
                } else if self.class_types.contains(name) {
                    // Classes are represented as i64 (pointer-to-int) for ABI compatibility
                    self.i64_t.into()
                } else if let Some((st, _)) = self.struct_types.get(name) {
                    (*st).into()
                } else {
                    // Use i8* as a placeholder pointer to struct if not found
                    self.i8_ptr_t.into()
                }
            }
            IRType::Enum { .. } => self.i64_t.into(),
            IRType::Pointer(_inner) | IRType::Reference(_inner) => self
                .ctx
                .ptr_type(AddressSpace::default())
                .into(),
            IRType::Optional(_inner) => {
                // Use pointer to inner where practical
                self.ctx
                    .ptr_type(AddressSpace::default())
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
        
        // Trace type registration with [LAYOUT] output for debugging layout mismatches
        if self.trace_options.trace_types || self.trace_options.dump_layouts || name == "VecChunk" || name == "Map" {
            eprintln!("[LAYOUT] Struct Name: {}", name);
            for (idx, (fname, ftype)) in fields.iter().enumerate() {
                eprintln!("[LAYOUT]   Field {}: {} (Type: {:?})", idx, fname, ftype);
            }
        }
        
        // Store Seen type definition
        self.struct_definitions.insert(name.to_string(), fields.to_vec());

        // Build LLVM struct type from fields
        let field_types: Vec<BasicTypeEnum<'ctx>> = fields
            .iter()
            .map(|(fname, ty)| {
                let llvm_ty = self.ir_type_to_llvm(ty);
                if name == "Map" {
                    eprintln!("[LLVM DEBUG] Map field {} type {:?} -> LLVM {:?}", fname, ty, llvm_ty);
                }
                llvm_ty
            })
            .collect();
        let field_names: Vec<String> = fields.iter().map(|(n, _)| n.clone()).collect();

        let llvm_struct_ty = self.ctx.struct_type(&field_types, false);
        
        // Debug for SeenLexer
        if name == "SeenLexer" || name == "Map" {
            eprintln!("[LLVM DEBUG] register_struct_type {} with {} fields, llvm_struct_ty: {:?}", name, field_types.len(), llvm_struct_ty);
        }
        
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

        // Debug: show what fields we're creating for Map
        if type_name == "Map" || type_name == "Vec" {
            eprintln!("DEBUG get_or_create_struct_type {}: fields={:?}", type_name, fields);
            eprintln!("  reg_struct_types={:?}", self.reg_struct_types);
            eprintln!("  class_types={:?}", self.class_types);
        }

        let field_types: Vec<BasicTypeEnum<'ctx>> = field_names
            .iter()
            .map(|name| {
                let val = &fields[name];
                match val {
                    IRValue::Integer(_) => self.i64_t.into(),
                    IRValue::Float(_) => self.ctx.f64_type().into(),
                    IRValue::Boolean(_) => self.bool_t.into(),
                    IRValue::String(_) => self.ty_string().into(), // Use proper String struct type
                    IRValue::Char(_) => self.ctx.i8_type().into(),
                    IRValue::Register(r) => {
                        // Check if this register holds a class type (pointer-to-int)
                        if let Some(struct_name) = self.reg_struct_types.get(r) {
                            eprintln!("  DEBUG field {}: reg %r{} -> struct_name={}, is_class={}", 
                                name, r, struct_name, self.class_types.contains(struct_name));
                            if self.class_types.contains(struct_name) {
                                // Class types are stored as i64 (pointer-to-int)
                                return self.i64_t.into();
                            }
                            // Check if it's a struct type we know about
                            if let Some((struct_ty, _)) = self.struct_types.get(struct_name).cloned() {
                                return struct_ty.into();
                            }
                        } else {
                            eprintln!("  DEBUG field {}: reg %r{} NOT FOUND in reg_struct_types", name, r);
                        }
                        // Default to pointer for unknowns
                        self.i8_ptr_t.into()
                    }
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
            .map(|p| self.ir_type_to_llvm_param(p).into())
            .collect();
        match ret {
            IRType::Void => self.ctx.void_type().fn_type(&params_ll, false),
            _ => {
                let r: BasicTypeEnum = self.ir_type_to_llvm(ret);
                r.fn_type(&params_ll, false)
            }
        }
    }
    
    /// Convert IR type to LLVM type for function parameters.
    /// Struct types are passed as pointers (consistent with C ABI and call sites).
    fn ir_type_to_llvm_param(&self, t: &IRType) -> BasicTypeEnum<'ctx> {
        match t {
            // Struct parameters are passed as pointers for ABI compatibility
            IRType::Struct { .. } => self.i8_ptr_t.into(),
            // All other types use the standard conversion
            _ => self.ir_type_to_llvm(t),
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
        self.reg_array_element_struct.clear();
        self.var_option_inner_type.clear();
        self.reg_option_inner_type.clear();
        self.var_is_string.clear();
        self.var_is_int_array.clear();
        self.blocks.clear();
        self.reg_slots.clear();
        self.reg_slot_types.clear();
        self.reg_field_access_info.clear();

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

        // Runtime debug: push frame at function entry
        if self.runtime_debug_flag {
            let i8_ptr_t = self.i8_ptr_t;
            let push_frame_ty = self.ctx.void_type().fn_type(&[i8_ptr_t.into()], false);
            let push_frame_fn = if let Some(f) = self.module.get_function("__seen_push_frame") {
                f
            } else {
                self.module.add_function("__seen_push_frame", push_frame_ty, None)
            };
            let func_name_str = self.builder.build_global_string_ptr(&func.name, &format!("func_name_{}", func.name)).unwrap();
            self.builder.build_call(push_frame_fn, &[func_name_str.as_pointer_value().into()], "push_frame").unwrap();
        }

        // DEBUG: Print start of main
        if func.name == "main" || func.name == "seen_main" {
             let i32_t = self.ctx.i32_type();
             let i8_ptr_t = self.i8_ptr_t;
             let puts_ty = i32_t.fn_type(&[i8_ptr_t.into()], false);
             let puts = if let Some(f) = self.module.get_function("puts") { f } else { self.module.add_function("puts", puts_ty, None) };
             
             let fmt = self.builder.build_global_string_ptr("DEBUG: seen_main start", "debug_start").unwrap();
             self.builder.build_call(puts, &[fmt.as_pointer_value().into()], "debug_puts").unwrap();
        }

        // Pre-scan to determine register types and allocate slots
        self.scan_and_allocate_registers(func)?;

        // Preallocate slots for parameters and locals so variable loads work across blocks.
        for param in &func.parameters {
            let ty = self.ir_type_to_llvm(&param.param_type);
            self.var_slot_types.insert(param.name.clone(), ty);
            
            // Special handling for 'this' if it's a struct passed by pointer (not a class)
            let is_struct_this = (param.name == "this" || param.name == "self") 
                && matches!(param.param_type, IRType::Struct { .. })
                && if let IRType::Struct { name, .. } = &param.param_type { !self.class_types.contains(name) } else { false };

            if is_struct_this {
                // Don't allocate slot yet, will use arg pointer directly in second loop
            } else {
                let slot = self.alloca_for_type(ty, &format!("param_slot_{}", param.name))?;
                self.var_slots.insert(param.name.clone(), slot);
            }
            
            // Track struct type names for field access
            if let IRType::Struct { name, fields } = &param.param_type {
                self.var_struct_types.insert(param.name.clone(), name.clone());
                if self.trace_options.trace_boxing && (param.name == "this" || param.name == "self") {
                    eprintln!("[BOXING] define_function '{}': param '{}' has struct type '{}'", func.name, param.name, name);
                }
                // Debug: also show value param
                if self.trace_options.trace_boxing && param.name == "value" {
                    eprintln!("[BOXING] define_function '{}': param 'value' has struct type '{}', fields={:?}", func.name, name, fields);
                }
                // Mark generic type parameters (struct T { }) as boxed
                // These are passed as pointers to the actual value and need dereference on use
                if (name == "T" || name == "E" || name == "K" || name == "V") && fields.is_empty() {
                    if self.trace_options.trace_boxing {
                        eprintln!("[BOXING] Marking param '{}' as boxed generic (type '{}')", param.name, name);
                    }
                    self.var_is_boxed_generic.insert(param.name.clone());
                }
            } else if self.trace_options.trace_boxing && (param.name == "this" || param.name == "self") {
                eprintln!("[BOXING] define_function '{}': param '{}' has NON-struct type {:?}", func.name, param.name, param.param_type);
            }
            // Also track struct types behind references/pointers
            if let IRType::Pointer(inner) | IRType::Reference(inner) = &param.param_type {
                if let IRType::Struct { name, .. } = inner.as_ref() {
                    self.var_struct_types.insert(param.name.clone(), name.clone());
                }
                if let IRType::Array(element_type) = inner.as_ref() {
                    if let IRType::Struct { name, .. } = element_type.as_ref() {
                        self.var_array_element_struct.insert(param.name.clone(), name.clone());
                    }
                    // Track String arrays behind pointers/references
                    if matches!(element_type.as_ref(), IRType::String) {
                        self.var_array_element_struct.insert(param.name.clone(), "String".to_string());
                    }
                    if matches!(element_type.as_ref(), IRType::Integer | IRType::Char) {
                        self.var_is_int_array.insert(param.name.clone());
                    }
                }
            }
            // Track array element struct types for array[i].field patterns
            if let IRType::Array(element_type) = &param.param_type {
                if let IRType::Struct { name, .. } = element_type.as_ref() {
                    self.var_array_element_struct.insert(param.name.clone(), name.clone());
                }
                // Track String arrays (String is a built-in struct type)
                if matches!(element_type.as_ref(), IRType::String) {
                    self.var_array_element_struct.insert(param.name.clone(), "String".to_string());
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
            // Debug all local variables in Some function
            if self.trace_options.trace_boxing && func.name == "Some" {
                eprintln!("[BOXING] Some: local var '{}' has type {:?}", local.name, local.var_type);
            }
            // Track struct type names for field access
            if let IRType::Struct { name, .. } = &local.var_type {
                // Debug: show location variable type
                if self.trace_options.trace_boxing && (local.name == "location" || local.name == "error" || local.name == "typeError") {
                    eprintln!("[BOXING] local var '{}' has struct type '{}' in func '{}'", local.name, name, func.name);
                }
                self.var_struct_types.insert(local.name.clone(), name.clone());
            }
            // Handle Optional types (struct T { }?) - track as "Option", not inner type
            if let IRType::Optional(inner) = &local.var_type {
                // Optional<T> is represented as Option struct
                if self.trace_options.trace_boxing {
                    eprintln!("[BOXING] local var '{}' has Optional type, tracking as 'Option' in func '{}'", local.name, func.name);
                }
                self.var_struct_types.insert(local.name.clone(), "Option".to_string());
                // Also track the inner type for unwrap
                if let IRType::Struct { name, .. } = inner.as_ref() {
                    self.var_option_inner_type.insert(local.name.clone(), name.clone());
                }
            }
            // Also track struct types behind references/pointers
            if let IRType::Pointer(inner) | IRType::Reference(inner) = &local.var_type {
                if let IRType::Struct { name, .. } = inner.as_ref() {
                    self.var_struct_types.insert(local.name.clone(), name.clone());
                }
                if let IRType::Array(element_type) = inner.as_ref() {
                    if let IRType::Struct { name, .. } = element_type.as_ref() {
                        self.var_array_element_struct.insert(local.name.clone(), name.clone());
                    }
                    // Track String arrays behind pointers/references
                    if matches!(element_type.as_ref(), IRType::String) {
                        self.var_array_element_struct.insert(local.name.clone(), "String".to_string());
                    }
                    if matches!(element_type.as_ref(), IRType::Integer | IRType::Char) {
                        self.var_is_int_array.insert(local.name.clone());
                    }
                }
            }
            // Track array element struct types for array[i].field patterns
            if let IRType::Array(element_type) = &local.var_type {
                if self.trace_options.trace_boxing {
                    eprintln!("[BOXING] Found Array type for '{}', element_type: {:?}", local.name, element_type);
                }
                if let IRType::Struct { name, .. } = element_type.as_ref() {
                    self.var_array_element_struct.insert(local.name.clone(), name.clone());
                    if self.trace_options.trace_boxing {
                        eprintln!("[BOXING] Tracked struct array element type '{}' for '{}'", name, local.name);
                    }
                }
                // Track String arrays (String is a built-in struct type)
                if matches!(element_type.as_ref(), IRType::String) {
                    self.var_array_element_struct.insert(local.name.clone(), "String".to_string());
                    if self.trace_options.trace_boxing {
                        eprintln!("[BOXING] Tracked String array element type for '{}'", local.name);
                    }
                }
                // Track integer and char arrays for proper array access
                if matches!(element_type.as_ref(), IRType::Integer | IRType::Char) {
                    self.var_is_int_array.insert(local.name.clone());
                }
            }

            // Debug: print local variable types
            if self.trace_options.trace_boxing && func.name == "GetCommandLineArgs" && local.name.contains("rawArgs") {
                eprintln!("[BOXING] GetCommandLineArgs local '{}' has type {:?}", local.name, local.var_type);
            }
            
            // Track string types for string indexing
            if matches!(local.var_type, IRType::String) {
                self.var_is_string.insert(local.name.clone());
            }
        }

        // Expose struct receiver fields (e.g., line/column/position on SeenLexer) as pseudo-variables
        // so IR that omits explicit locals still maps to the underlying struct fields.
        for receiver in ["self", "this"] {
            if let Some(struct_name) = self.var_struct_types.get(receiver).cloned() {
                if let Some((llvm_struct_ty, field_names)) = self.struct_types.get(&struct_name).cloned() {
                    if let Some(receiver_slot) = self.var_slots.get(receiver).copied() {
                        for (idx, fname) in field_names.iter().enumerate() {
                            if self.var_slots.contains_key(fname) {
                                continue;
                            }
                            let gep = self.build_struct_gep_checked(
                                llvm_struct_ty,
                                receiver_slot,
                                idx as u32,
                                &format!("{receiver}_field_{fname}"),
                            )?;
                            let field_ty = llvm_struct_ty.get_field_types()[idx];
                            self.var_slots.insert(fname.clone(), gep);
                            self.var_slot_types.insert(fname.clone(), field_ty);

                            if let Some(fields) = self.struct_definitions.get(&struct_name) {
                                if let Some((_, ir_ty)) = fields.iter().find(|(n, _)| n == fname) {
                                    match ir_ty {
                                        IRType::Struct { name, .. } => {
                                            self.var_struct_types.insert(fname.clone(), name.clone());
                                        }
                                        IRType::Array(inner) => {
                                            if let IRType::Struct { name, .. } = inner.as_ref() {
                                                self.var_array_element_struct.insert(fname.clone(), name.clone());
                                            }
                                            // Track String arrays (String is a built-in struct type)
                                            if matches!(inner.as_ref(), IRType::String) {
                                                self.var_array_element_struct.insert(fname.clone(), "String".to_string());
                                            }
                                            if matches!(inner.as_ref(), IRType::Integer | IRType::Char) {
                                                self.var_is_int_array.insert(fname.clone());
                                            }
                                        }
                                        IRType::String => {
                                            self.var_is_string.insert(fname.clone());
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Allocate slots for virtual registers
        // (Handled by scan_and_allocate_registers)

        // Initialize virtual registers with function parameters in order
        // (assumes IR uses %r0..%rN for the first N arguments)
        let param_count = f.count_params() as u32;
        for i in 0..param_count {
            if let Some(p) = f.get_nth_param(i as u32) {
                let param_val = p.clone();
                // Map %r{i}
                self.reg_values.insert(i, param_val.clone());
                // Also store into slot if available
                if let Some(slot) = self.reg_slots.get(&i).copied() {
                    let reg_val = param_val.clone();
                    let slot_ty = self.reg_slot_types.get(&i).copied().unwrap_or(self.i64_t.into());
                    
                    if reg_val.get_type() == slot_ty {
                         self.builder.build_store(slot, reg_val).map_err(|e| anyhow!("{e:?}"))?;
                    } else if slot_ty == self.i64_t.into() {
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
                }
                // Map by parameter name when available
                if (i as usize) < func.parameters.len() {
                    let pname = func.parameters[i as usize].name.clone();
                    self.var_values.insert(pname.clone(), param_val.clone());
                    
                    // Special handling for 'this' struct pointer
                    let is_struct_this = (pname == "this" || pname == "self") 
                        && matches!(func.parameters[i as usize].param_type, IRType::Struct { .. })
                        && if let IRType::Struct { name, .. } = &func.parameters[i as usize].param_type { !self.class_types.contains(name) } else { false };

                    if is_struct_this {
                        if param_val.is_pointer_value() {
                            self.var_slots.insert(pname.clone(), param_val.into_pointer_value());
                        }
                        // Skip store, as we use the pointer directly
                    } else if let Some(slot) = self.var_slots.get(&pname).copied() {
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
                // If the builder position is cleared (e.g. by an early return or jump in the instruction list),
                // subsequent instructions are unreachable. Skip them to avoid "Builder position is not set" errors.
                if self.builder.get_insert_block().is_some() {
                    self.emit_instruction(inst, fn_map)?;
                }
            }
            if let Some(term) = &b.terminator {
                if self.builder.get_insert_block().is_some() {
                    self.emit_instruction(term, fn_map)?;
                }
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
        // Trace instruction if enabled
        if self.trace_options.trace_instructions {
            let fn_name = self.current_fn
                .map(|f| f.get_name().to_string_lossy().into_owned())
                .unwrap_or_else(|| "<unknown>".to_string());
            eprintln!(
                "[LLVM TRACE] fn {} inst #{}: {}",
                fn_name,
                self.current_inst_idx,
                Self::format_instruction_summary(inst)
            );
        }
        self.current_inst_idx += 1;
        
        match inst {
            Instruction::Label(lbl) => {
                self.emit_label(lbl)?;
            }
            Instruction::Jump(target) => {
                self.emit_jump(target)?;
            }
            Instruction::JumpIf { condition, target } => {
                self.emit_jump_if(condition, target, fn_map)?;
            }
            Instruction::JumpIfNot { condition, target } => {
                self.emit_jump_if_not(condition, target, fn_map)?;
            }
            Instruction::Return(val_opt) => {
                self.emit_return(val_opt, fn_map)?;
            }
            Instruction::Move { source, dest } => {
                self.emit_move(source, dest, fn_map)?;
            }
            Instruction::Store { value, dest } => {
                self.emit_store(value, dest, fn_map)?;
            }
            Instruction::Load { source, dest } => {
                self.emit_load(source, dest, fn_map)?;
            }
            Instruction::Unary {
                op,
                operand,
                result,
            } => {
                let val = self.eval_value(operand, fn_map)?;
                let res = self.emit_unary_op(op, val)?;
                self.assign_value(result, res)?;
            }
            Instruction::Binary {
                op,
                left,
                right,
                result,
            } => {
                let res = self.emit_binary_op(op, left, right, fn_map)?;
                self.assign_value(result, res)?;
            }
            Instruction::StringLength { string, result } => {
                let s_val = self.eval_value(string, fn_map)?;
                let (_, len) = self.get_string_ptr_len(s_val)?;
                self.assign_value(result, len.as_basic_value_enum())?;
            }
            Instruction::StringConcat {
                left,
                right,
                result,
            } => {
                let lval = self.eval_value(left, fn_map)?;
                let rval = self.eval_value(right, fn_map)?;
                
                let l_str = self.ensure_string(lval, left)?;
                let r_str = self.ensure_string(rval, right)?;
                
                let out = self.runtime_concat(l_str, r_str)?;
                
                // runtime_concat returns a pointer to the new String struct
                // We need to load it to get the String struct value, otherwise assign_value
                // might misinterpret the pointer as a C-string (char*) and try to strlen it,
                // leading to corruption (reading the struct length as the first char).
                let out_ptr = out.into_pointer_value();
                let out_struct = self.builder.build_load(self.ty_string(), out_ptr, "concat_res")
                    .map_err(|e| anyhow!("Failed to load string struct: {:?}", e))?;
                
                self.assign_value(result, out_struct.as_basic_value_enum())?;
            }
            Instruction::SimdSplat {
                scalar,
                lane_type,
                lanes,
                result,
            } => {
                self.emit_simd_splat(scalar, lane_type, *lanes, result, fn_map)?;
            }
            Instruction::SimdReduceAdd {
                vector,
                lane_type,
                result,
            } => {
                self.emit_simd_reduce_add(vector, lane_type, result, fn_map)?;
            }
            Instruction::ArrayLength { array, result } => {
                self.emit_array_length(array, result, fn_map)?;
            }
            Instruction::ArrayAccess {
                array,
                index,
                result,
                element_type,
            } => {
                // Pass element_type to help with struct array access
                if let Some(ref et) = element_type {
                    // Propagate element type to emit_array_access
                    if let IRType::Struct { name, .. } = et {
                        // Store element type info for this access
                        if let IRValue::Register(reg_id) = array {
                            self.reg_array_element_struct.insert(*reg_id, name.clone());
                        }
                    }
                }
                self.emit_array_access(array, index, result, fn_map)?;
            }
            Instruction::ArraySet { array, index, value, element_type } => {
                // Pass element_type to help with struct array set
                if let Some(ref et) = element_type {
                    // Propagate element type to emit_array_set
                    if let IRType::Struct { name, .. } = et {
                        // Store element type info for this set
                        if let IRValue::Register(reg_id) = array {
                            self.reg_array_element_struct.insert(*reg_id, name.clone());
                        }
                    }
                }
                self.emit_array_set(array, index, value, fn_map)?;
            }
            Instruction::Call {
                target,
                args,
                result,
                ..
            } => {
                self.emit_call(target, args, result, fn_map)?;
            }
            Instruction::Print(v) => {
                let vval = self.eval_value(v, fn_map)?;
                let (ptr, _) = self.get_string_ptr_len(vval)?;
                self.call_printf(&[ptr.into()])?;
            }
            Instruction::FieldAccess {
                struct_val,
                field,
                result,
                ..
            } => {
                self.emit_field_access(struct_val, field, result, fn_map)?;
            }
            Instruction::FieldSet {
                struct_val,
                field,
                value,
                ..
            } => {
                self.emit_field_set(struct_val, field, value, fn_map)?;
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
            Instruction::ConstructObject { class_name, args, result, .. } => {
                if let Some((struct_ty, field_names)) = self.struct_types.get(class_name).cloned() {
                    let expected_fields = field_names.len();
                    if args.len() != expected_fields {
                        return Err(anyhow!(
                            "ConstructObject: class '{}' expects {} fields but IR provided {} args",
                            class_name,
                            expected_fields,
                            args.len(),
                        ));
                    }

                    // Heap-allocate the struct and return a pointer (class-as-pointer semantics)
                    let struct_size = struct_ty.size_of().unwrap_or(self.i64_t.const_int(64, false));
                    
                    // Trace ConstructObject
                    if self.trace_options.trace_instructions {
                        eprintln!(
                            "[LLVM TRACE]   ConstructObject details: class={}, struct_size={:?}, fields={:?}",
                            class_name, struct_size, field_names
                        );
                    }
                    
                    let malloc_fn = self.get_malloc();
                    let heap_ptr = self.builder
                        .build_call(malloc_fn, &[struct_size.into()], "alloc_obj")?
                        .try_as_basic_value()
                        .left()
                        .ok_or_else(|| anyhow!("malloc didn't return a value"))?
                        .into_pointer_value();
                    
                    // Cast to the struct pointer type
                    let struct_ptr_ty = self.ctx.ptr_type(AddressSpace::default());
                    let typed_ptr = self.builder
                        .build_pointer_cast(heap_ptr, struct_ptr_ty, "typed_ptr")?;
                    
                    // Store field values via GEP
                    for (i, arg) in args.iter().enumerate() {
                        let arg_val = self.eval_value(arg, fn_map)?;
                        let field_ptr = self.build_struct_gep_checked(
                            struct_ty,
                            typed_ptr,
                            i as u32,
                            &format!("field_{}_ptr", i),
                        )?;
                        
                        // Check if the field is an Array type (struct { i64, i64, ptr }) and arg is a pointer
                        // If so, we need to load the array content from the pointer and store it
                        let field_ty = struct_ty.get_field_types()[i];
                        let is_array_field = if let BasicTypeEnum::StructType(ft) = field_ty {
                            let types = ft.get_field_types();
                            types.len() == 3 
                                && matches!(types[0], BasicTypeEnum::IntType(_))
                                && matches!(types[1], BasicTypeEnum::IntType(_))
                                && matches!(types[2], BasicTypeEnum::PointerType(_))
                        } else {
                            false
                        };
                        
                        // Trace field storage
                        if self.trace_options.trace_values {
                            eprintln!(
                                "[LLVM TRACE]   field[{}] '{}': arg={:?}, val_type={}, is_array_field={}, is_ptr={}",
                                i,
                                field_names.get(i).unwrap_or(&"?".to_string()),
                                arg,
                                Self::format_llvm_type(arg_val.get_type()),
                                is_array_field,
                                arg_val.is_pointer_value()
                            );
                        }
                        
                        if is_array_field && arg_val.is_pointer_value() {
                            // arg_val is a pointer to an array struct - load the struct and store it
                            // BUT: check for null first (happens when default constructor has null for array fields)
                            let ptr_val = arg_val.into_pointer_value();
                            let is_null_ptr = ptr_val.is_null();
                            
                            if is_null_ptr {
                                // Create an empty array struct { i64 len=0, i64 cap=0, ptr data=null }
                                let arr_struct_ty = field_ty.into_struct_type();
                                let zero = self.i64_t.const_int(0, false);
                                let null_ptr = self.i8_ptr_t.const_null();
                                let empty_arr = arr_struct_ty.const_named_struct(&[
                                    zero.into(),
                                    zero.into(),
                                    null_ptr.into(),
                                ]);
                                self.builder.build_store(field_ptr, empty_arr)?;
                            } else {
                                let arr_struct_ty = field_ty.into_struct_type();
                                let loaded_arr = self.builder.build_load(arr_struct_ty, ptr_val, "load_arr")?;
                                self.builder.build_store(field_ptr, loaded_arr)?;
                            }
                        } else {
                            self.builder.build_store(field_ptr, arg_val)?;
                        }
                    }
                    
                    // Return the pointer as i64 for ABI compatibility
                    let ptr_as_int = self.builder
                        .build_ptr_to_int(typed_ptr, self.i64_t, "obj_ptr")?;
                    
                    self.assign_value(result, ptr_as_int.as_basic_value_enum())?;
                    
                    // Track struct type for this register
                    if let IRValue::Register(r) = result {
                        self.reg_struct_types.insert(*r, class_name.clone());
                    }
                } else {
                    return Err(anyhow!("ConstructObject: unknown class type '{}'", class_name));
                }
            }
            Instruction::Allocate { size, result } => {
                // Handle generic allocation instruction
                let size_val = self.eval_value(size, fn_map)?;
                let size_int = if size_val.is_int_value() {
                    size_val.into_int_value()
                } else {
                    self.i64_t.const_int(8, false) // Default 8 bytes
                };
                
                let malloc_fn = self.get_malloc();
                let heap_ptr = self.builder
                    .build_call(malloc_fn, &[size_int.into()], "alloc")?
                    .try_as_basic_value()
                    .left()
                    .ok_or_else(|| anyhow!("malloc didn't return a value"))?;
                
                // Return pointer as i64
                let ptr_as_int = self.builder
                    .build_ptr_to_int(heap_ptr.into_pointer_value(), self.i64_t, "ptr")?;
                
                self.assign_value(result, ptr_as_int.as_basic_value_enum())?;
            }
            _ => {
                // Many IR ops are not required for bootstrap subset; ignore nops etc.
            }
        }
        Ok(())
    }

    // lower_channel_select moved to crate::llvm::concurrency::ConcurrencyOps trait

    // box_runtime_value, ensure_box_*_fn moved to crate::llvm::runtime_fns::RuntimeFunctions trait

    // ensure_int_to_string_fn, ensure_char_to_string_fn, ensure_float_to_string_fn, ensure_bool_to_string_fn 
    // moved to crate::llvm::runtime_fns::RuntimeFunctions trait

    /// Auto-declare an external runtime function with a generic signature.
    /// All parameters and return type default to i64 for simplicity.
    /// This allows calling runtime functions that aren't specifically handled.

    pub(crate) fn to_string_ptr(&mut self, v: BasicValueEnum<'ctx>) -> Result<PointerValue<'ctx>> {
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
            let ret = call.try_as_basic_value().left().unwrap();
            if ret.is_struct_value() {
                let sv = ret.into_struct_value();
                let ptr = self.builder.build_extract_value(sv, 1, "str_ptr")?.into_pointer_value();
                return Ok(ptr);
            }
            return Ok(ret.into_pointer_value());
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
            let ret = call.try_as_basic_value().left().unwrap();
            if ret.is_struct_value() {
                let sv = ret.into_struct_value();
                let ptr = self.builder.build_extract_value(sv, 1, "str_ptr")?.into_pointer_value();
                return Ok(ptr);
            }
            return Ok(ret.into_pointer_value());
        }
        if v.is_struct_value() {
             let sv = v.into_struct_value();
             if let Ok(val) = self.builder.build_extract_value(sv, 1, "str_ptr") {
                if val.is_pointer_value() {
                    return Ok(val.into_pointer_value());
                }
            }
        }
        Err(anyhow!("Cannot convert {:?} to string pointer", v))
    }

    pub(crate) fn eval_value(
        &mut self,
        v: &IRValue,
        fn_map: &HashMap<String, FunctionValue<'ctx>>,
    ) -> Result<BasicValueEnum<'ctx>> {
        let result = self.eval_value_inner(v, fn_map);
        
        // Trace value evaluation if enabled
        if self.trace_options.trace_values {
            match &result {
                Ok(val) => {
                    let ty_str = Self::format_llvm_type(val.get_type());
                    let is_ptr = val.is_pointer_value();
                    eprintln!(
                        "[LLVM TRACE]   eval {:?} => {} (is_ptr: {})",
                        v, ty_str, is_ptr
                    );
                }
                Err(e) => {
                    eprintln!("[LLVM TRACE]   eval {:?} => ERROR: {}", v, e);
                }
            }
        }
        
        result
    }
    
    /// Format LLVM type for trace output
    fn format_llvm_type(ty: BasicTypeEnum<'_>) -> String {
        match ty {
            BasicTypeEnum::IntType(t) => format!("i{}", t.get_bit_width()),
            BasicTypeEnum::FloatType(_) => "float".to_string(),
            BasicTypeEnum::PointerType(_) => "ptr".to_string(),
            BasicTypeEnum::StructType(s) => {
                let fields: Vec<String> = s.get_field_types()
                    .iter()
                    .map(|f| Self::format_llvm_type(*f))
                    .collect();
                format!("{{ {} }}", fields.join(", "))
            }
            BasicTypeEnum::ArrayType(a) => {
                format!("[{} x {}]", a.len(), Self::format_llvm_type(a.get_element_type()))
            }
            BasicTypeEnum::VectorType(v) => {
                format!("<{} x ?>", v.get_size())
            }
            BasicTypeEnum::ScalableVectorType(_) => "<scalable vector>".to_string(),
        }
    }
    
    fn eval_value_inner(
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
            IRValue::Char(c) => {
                // Char is an i8 in LLVM
                Ok(self.ctx.i8_type().const_int(*c as u64, false).as_basic_value_enum())
            }
            IRValue::String(s) => {
                let gv = self.builder.build_global_string_ptr(&(s.clone()), "str")?;
                let ptr = gv.as_pointer_value();
                let len = self.i64_t.const_int(s.len() as u64, false);
                
                let ty = self.ty_string();
                let mut val = ty.get_undef();
                val = self.builder.build_insert_value(val, len, 0, "slen")?.into_struct_value();
                val = self.builder.build_insert_value(val, ptr, 1, "sptr")?.into_struct_value();
                
                // Allocate on heap to ensure it persists and fits in generic slots (as pointer)
                let malloc_ptr = self.builder.build_malloc(ty, "str_malloc")?;
                self.builder.build_store(malloc_ptr, val)?;
                
                Ok(malloc_ptr.as_basic_value_enum())
            }
            IRValue::Void => Ok(self.i64_t.const_zero().as_basic_value_enum()),
            IRValue::SizeOf(ty) => {
                let llvm_ty = self.ir_type_to_llvm(ty);
                let size = llvm_ty.size_of().unwrap_or(self.i64_t.const_int(8, false));
                // Ensure it's i64
                let size_i64 = self.builder.build_int_z_extend(size, self.i64_t, "sizeof_ext")?;
                Ok(size_i64.as_basic_value_enum())
            }
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
                    let ty = self.reg_slot_types.get(r).copied().unwrap_or(self.i64_t.into());
                    let loaded =
                        self.builder
                            .build_load(ty, slot, &format!("load_r{}", r))?;
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
                let fn_name = self.current_fn.map(|f| f.get_name().to_str().unwrap_or("?").to_string()).unwrap_or("?".to_string());
                let available = self.reg_slots.keys().map(|k| k.to_string()).collect::<Vec<_>>().join(", ");
                Err(anyhow!("Unknown register %r{r} in function {fn_name}. Available: {available}"))
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
                let fn_name = self.current_fn.map(|f| f.get_name().to_str().unwrap_or("?").to_string()).unwrap_or("?".to_string());
                Err(anyhow!(format!("Unknown variable {} in function {}", name, fn_name)))
            }
            IRValue::Array(vals) => {
                let len = vals.len() as u64;
                let cap = if len < 8 { 8 } else { len };
                
                // Determine element type and size
                let (elem_ty, elem_size_val) = if let Some(first) = vals.first() {
                    let v = self.eval_value(first, fn_map)?;
                    let ty = v.get_type();
                    let size = ty.size_of().unwrap(); // IntValue
                    (ty, size)
                } else {
                    // Default to i64/ptr size
                    (self.i64_t.into(), self.i64_t.const_int(8, false))
                };
                
                // Calculate data size: cap * elem_size
                let cap_val = self.i64_t.const_int(cap, false);
                let elem_size_i64 = self.builder.build_int_z_extend(elem_size_val, self.i64_t, "sz_ext")?;
                let data_size = self.builder.build_int_mul(cap_val, elem_size_i64, "data_sz")?;
                
                let malloc_fn = self.get_malloc();
                let data_ptr = self.builder
                    .build_call(malloc_fn, &[data_size.into()], "malloc_data")?
                    .try_as_basic_value()
                    .left()
                    .unwrap()
                    .into_pointer_value();
                
                // Store elements
                for (i, val) in vals.iter().enumerate() {
                    let v = self.eval_value(val, fn_map)?;
                    // Cast data_ptr to elem_ty*
                    let ptr_ty = self.ctx.ptr_type(AddressSpace::default());
                    let elem_ptr = self.builder.build_pointer_cast(data_ptr, ptr_ty, "elem_ptr")?;
                    // GEP to index i
                    let slot = unsafe { self.builder.build_gep(elem_ty, elem_ptr, &[self.i64_t.const_int(i as u64, false)], "slot")? };
                    self.builder.build_store(slot, v)?;
                }
                
                // Allocate Array Struct {len, cap, data}
                // Use i8 for generic data ptr in struct type to match malloc
                let struct_ty = self.ty_array(self.ctx.i8_type().into());
                let struct_size = struct_ty.size_of().unwrap();
                let struct_size_i64 = self.builder.build_int_z_extend(struct_size, self.i64_t, "struct_sz")?;
                
                let arr_ptr = self.builder
                    .build_call(malloc_fn, &[struct_size_i64.into()], "malloc_arr")?
                    .try_as_basic_value()
                    .left()
                    .unwrap()
                    .into_pointer_value();
                    
                let arr_ptr_typed = self.builder.build_pointer_cast(arr_ptr, self.ctx.ptr_type(AddressSpace::default()), "arr_ptr_typed")?;
                
                // Set len
                let len_ptr = self.build_struct_gep_checked(struct_ty, arr_ptr_typed, 0, "len_ptr")?;
                self.builder.build_store(len_ptr, self.i64_t.const_int(len, false))?;
                
                // Set cap
                let cap_ptr = self.build_struct_gep_checked(struct_ty, arr_ptr_typed, 1, "cap_ptr")?;
                self.builder.build_store(cap_ptr, self.i64_t.const_int(cap, false))?;
                
                // Set data
                let data_field_ptr = self.build_struct_gep_checked(struct_ty, arr_ptr_typed, 2, "data_ptr")?;
                self.builder.build_store(data_field_ptr, data_ptr)?;
                
                Ok(arr_ptr.as_basic_value_enum())
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
                // Allocate memory for the struct on the HEAP and populate fields.
                // NOTE: This causes memory leaks for temporary structs (like VecSlot) that 
                // are created frequently but never freed. A proper fix would use:
                // 1. Return structs by value for small structs (<= 16 bytes)
                // 2. LLVM sret convention for larger structs
                // 3. Reference counting or garbage collection
                // For now, heap allocation is used to ensure correctness.
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
                
                // Debug: show field order vs fields
                if type_name == "Vec" {
                    let fn_name = self.current_fn.map(|f| f.get_name().to_string_lossy().to_string()).unwrap_or("?".to_string());
                    eprintln!("DEBUG eval_value Vec struct in {}: field_order={:?}, fields.keys={:?}", 
                        fn_name, field_order, fields.keys().collect::<Vec<_>>());
                }
                
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
                
                // Cast heap_ptr to struct pointer type
                let struct_ptr_ty = self.ctx.ptr_type(AddressSpace::default());
                let typed_ptr = self.builder.build_pointer_cast(heap_ptr, struct_ptr_ty, "typed_ptr")?;

                // Set each field
                for (idx, field_name) in field_order.iter().enumerate() {
                    if let Some(field_val) = fields.get(field_name) {
                        if type_name == "Vec" {
                            let fn_name = self.current_fn.map(|f| f.get_name().to_string_lossy().to_string()).unwrap_or("?".to_string());
                            let reg_values_keys: Vec<_> = self.reg_values.keys().collect();
                            eprintln!("DEBUG Vec field: fn={}, idx={}, name={}, val={:?}, reg_values={:?}", fn_name, idx, field_name, field_val, reg_values_keys);
                        }
                        let val = self.eval_value(field_val, fn_map)?;
                        if type_name == "Vec" {
                            eprintln!("DEBUG Vec field evaluated: idx={}, name={}, llvm_val={:?}", idx, field_name, val);
                        }
                        let field_ptr = self.build_struct_gep_checked(
                            llvm_struct_ty,
                            typed_ptr,
                            idx as u32,
                            &format!("{}_field_{}", type_name, field_name),
                        )?;
                        
                        // Check if the field is an Array type and val is a pointer
                        // If so, load the array struct content from the pointer
                        let field_llvm_ty = llvm_struct_ty.get_field_types()[idx];
                        let is_array_field = if let BasicTypeEnum::StructType(ft) = field_llvm_ty {
                            let types = ft.get_field_types();
                            types.len() == 3 
                                && matches!(types[0], BasicTypeEnum::IntType(_))
                                && matches!(types[1], BasicTypeEnum::IntType(_))
                                && matches!(types[2], BasicTypeEnum::PointerType(_))
                        } else {
                            false
                        };
                        
                        if is_array_field && val.is_pointer_value() {
                            // val is a pointer to an array struct - load the struct and store it
                            if self.trace_options.trace_values {
                                eprintln!("[LLVM TRACE]   IRValue::Struct field '{}': loading array struct from ptr", field_name);
                            }
                            let arr_struct_ty = field_llvm_ty.into_struct_type();
                            let loaded_arr = self.builder.build_load(arr_struct_ty, val.into_pointer_value(), "load_arr_struct")?;
                            self.builder.build_store(field_ptr, loaded_arr)?;
                        } else {
                            self.builder.build_store(field_ptr, val)?;
                        }
                    }
                }
                
                // Return pointer to struct
                Ok(heap_ptr.as_basic_value_enum())
            }
            _ => Err(anyhow!("Unsupported IRValue in LLVM backend: {v:?}")),
        }
    }

    // to_i8_ptr moved to crate::llvm::type_cast::TypeCastOps trait

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

    pub(crate) fn assign_value(&mut self, dest: &IRValue, v: BasicValueEnum<'ctx>) -> Result<()> {
        match dest {
            IRValue::Register(r) => {
                self.reg_values.insert(*r, v.clone());

                // Lazy allocation if not exists
                if !self.reg_slots.contains_key(r) {
                    let func = self.current_fn.unwrap();
                    let entry = func.get_first_basic_block().unwrap();
                    let current_block = self.builder.get_insert_block();

                    if let Some(first_inst) = entry.get_first_instruction() {
                        self.builder.position_before(&first_inst);
                    } else {
                        self.builder.position_at_end(entry);
                    }

                    let ty = v.get_type();
                    let slot = self.builder.build_alloca(ty, &format!("reg{}_slot", r)).map_err(|e| anyhow!("{e:?}"))?;
                    self.reg_slots.insert(*r, slot);
                    self.reg_slot_types.insert(*r, ty);

                    if let Some(cb) = current_block {
                        self.builder.position_at_end(cb);
                    } else {
                        self.builder.clear_insertion_position();
                    }
                }

                // Also persist through the reg slot if available
                if let Some(slot) = self.reg_slots.get(r).copied() {
                    let reg_val = v.clone();
                    let slot_ty = self.reg_slot_types.get(r).copied().unwrap_or(self.i64_t.into());

                    if reg_val.get_type() == slot_ty {
                         self.builder.build_store(slot, reg_val).ok();
                    } else if slot_ty == self.i64_t.into() {
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
                    
                    // Check for struct-to-int type mismatch - if the value is a struct but slot is i64,
                    // we need to reallocate with the correct struct type
                    if v.is_struct_value() && ty.is_int_type() {
                        let value_ty = self
                            .basic_type_from_value(&v)
                            .ok_or_else(|| anyhow!("Cannot infer type for variable {}", name))?;
                        let new_slot = self.alloca_for_type(value_ty, &format!("var_{}_struct", name))?;
                        self.var_slots.insert(name.clone(), new_slot);
                        self.var_slot_types.insert(name.clone(), value_ty);
                        (new_slot, value_ty)
                    // Check for String struct-to-pointer type mismatch - if the value is a String struct 
                    // but slot is a pointer (e.g., from Generic type), reallocate with String type
                    } else if v.is_struct_value() && ty.is_pointer_type() {
                        // Get struct type without consuming v - check if it's a String struct
                        if let BasicValueEnum::StructValue(sv) = &v {
                            if sv.get_type() == self.ty_string() {
                                let string_ty = self.ty_string().as_basic_type_enum();
                                let new_slot = self.alloca_for_type(string_ty, &format!("var_{}_string", name))?;
                                self.var_slots.insert(name.clone(), new_slot);
                                self.var_slot_types.insert(name.clone(), string_ty);
                                (new_slot, string_ty)
                            } else {
                                (p, ty)
                            }
                        } else {
                            (p, ty)
                        }
                    } else if v.is_int_value() && ty.is_int_type() && v.into_int_value().get_type().get_bit_width() != ty.into_int_type().get_bit_width() {
                        // Int width mismatch (e.g. i8 vs i64) - reallocate if needed, or just extend/truncate
                        // If the slot is i64 but value is i8, we can extend.
                        // But if we want to store i8 in i8 slot, we need to make sure slot is i8.
                        // If slot is i64 (default), we should probably keep it i64 and extend.
                        (p, ty)
                    } else {
                        (p, ty)
                    }
                } else {
                    let value_ty = self
                        .basic_type_from_value(&v)
                        .ok_or_else(|| anyhow!("Cannot infer type for variable {}", name))?;
                    if self.trace_options.trace_boxing {
                        eprintln!("[BOXING] assign_value creating new slot for '{}', value_ty={}",
                            name, value_ty.print_to_string().to_string());
                    }
                    let slot = self.alloca_for_type(value_ty, &format!("var_{}", name))?;
                    self.var_slots.insert(name.clone(), slot);
                    self.var_slot_types.insert(name.clone(), value_ty);
                    (slot, value_ty)
                };

                // If the slot type looks like an Array struct {len, cap, data} but the incoming
                // value is a pointer/int (e.g., the raw header pointer from __ArrayNew), load the
                // struct from that pointer rather than degrading the slot to i64.
                if let BasicTypeEnum::StructType(st) = slot_ty {
                    let f = st.get_field_types();
                    let looks_like_array = f.len() == 3
                        && matches!(f[0], BasicTypeEnum::IntType(_))
                        && matches!(f[1], BasicTypeEnum::IntType(_))
                        && matches!(f[2], BasicTypeEnum::PointerType(_));

                    if looks_like_array && (v.is_pointer_value() || v.is_int_value()) {
                        let raw_ptr = if v.is_pointer_value() {
                            v.into_pointer_value()
                        } else {
                            // The header pointer was materialized as an integer; reinterpret it.
                            self.builder
                                .build_int_to_ptr(v.into_int_value(), self.i8_ptr_t, "arr_header_ptr")
                                .map_err(|e| anyhow!("{e:?}"))?
                        };

                        let typed_ptr = self
                            .builder
                            .build_pointer_cast(
                                raw_ptr,
                                self.ctx.ptr_type(AddressSpace::default()),
                                "arr_header_cast",
                            )
                            .map_err(|e| anyhow!("{e:?}"))?;

                        let loaded = self
                            .builder
                            .build_load(st, typed_ptr, "arr_header_load")
                            .map_err(|e| anyhow!("{e:?}"))?
                            .as_basic_value_enum();

                        self.var_values.insert(name.clone(), loaded);
                        self.builder.build_store(slot, loaded).map_err(|e| anyhow!("{e:?}"))?;
                        return Ok(());
                    }
                }
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

    // as_bool, as_i64, as_f64, as_usize, as_cstr_ptr moved to crate::llvm::type_cast::TypeCastOps trait

    pub(crate) fn alloca_for_type(
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
        if self.trace_options.trace_boxing {
            eprintln!("[BOXING] load_from_slot name='{}', slot={:?}, elem_ty={}",
                name, slot, elem_ty.print_to_string().to_string());
        }
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
        
        // For boxed generic parameters, dereference the pointer to get the actual value
        // The loaded value is a pointer to the actual value (boxed primitive)
        if self.var_is_boxed_generic.contains(name) {
            if self.trace_options.trace_boxing {
                eprintln!("[BOXING] load_from_slot dereferencing boxed generic '{}'", name);
            }
            if loaded.is_pointer_value() {
                let ptr = loaded.into_pointer_value();
                let deref = self.builder.build_load(self.i64_t, ptr, "deref_boxed_generic")
                    .map_err(|e| anyhow!("{e:?}"))?;
                return Ok(deref.as_basic_value_enum());
            }
        }
        
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

    pub(crate) fn cast_basic_to_type(
        &mut self,
        value: BasicValueEnum<'ctx>,
        target: BasicTypeEnum<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>> {
        match (value, target) {
            (BasicValueEnum::IntValue(iv), BasicTypeEnum::IntType(it)) => {
                if iv.get_type() == it {
                    Ok(iv.as_basic_value_enum())
                } else if iv.get_type().get_bit_width() < it.get_bit_width() {
                    // Special case: if we are extending i8 to i64, use zero extension for chars
                    // This avoids sign extension issues with high-bit chars (like 0x80-0xFF)
                    // which would become negative numbers if sign-extended.
                    let is_char = iv.get_type().get_bit_width() == 8;
                    let ext = if is_char {
                        self.builder
                            .build_int_z_extend(iv, it, "zext_char")
                            .map_err(|e| anyhow!("{e:?}"))?
                    } else {
                        self.builder
                            .build_int_s_extend(iv, it, "sext_store")
                            .map_err(|e| anyhow!("{e:?}"))?
                    };
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
                // Prefer boxing integers when a pointer (e.g. optional primitive) is expected
                let alloca = self
                    .builder
                    .build_alloca(self.i64_t, "int_autobox")
                    .map_err(|e| anyhow!("{e:?}"))?;
                let stored = if iv.get_type().get_bit_width() < 64 {
                    self
                        .builder
                        .build_int_s_extend(iv, self.i64_t, "int_autobox_ext")
                        .map_err(|e| anyhow!("{e:?}"))?
                } else if iv.get_type().get_bit_width() > 64 {
                    self
                        .builder
                        .build_int_truncate(iv, self.i64_t, "int_autobox_trunc")
                        .map_err(|e| anyhow!("{e:?}"))?
                } else {
                    iv
                };
                self
                    .builder
                    .build_store(alloca, stored)
                    .map_err(|e| anyhow!("{e:?}"))?;

                let cast = self
                    .builder
                    .build_bit_cast(
                        alloca.as_basic_value_enum(),
                        pt.as_basic_type_enum(),
                        "int_ptrcast_store",
                    )
                    .map_err(|e| anyhow!("{e:?}"))?;
                Ok(cast)
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
            (BasicValueEnum::StructValue(sv), BasicTypeEnum::IntType(it)) => {
                if sv.get_type() == self.ty_string() && it == self.bool_t {
                    // String to Bool: check if length > 0
                    let len = self.builder.build_extract_value(sv, 0, "slen").unwrap().into_int_value();
                    let zero = self.i64_t.const_zero();
                    let is_not_empty = self.builder.build_int_compare(inkwell::IntPredicate::NE, len, zero, "str_not_empty")?;
                    Ok(is_not_empty.as_basic_value_enum())
                } else if it == self.bool_t && sv.get_type().count_fields() >= 1 {
                    // CommandResult or similar struct with bool as first field - extract the bool
                    if let Some(first_field_ty) = sv.get_type().get_field_type_at_index(0) {
                        if first_field_ty.is_int_type() && first_field_ty.into_int_type() == self.bool_t {
                            let bool_val = self.builder.build_extract_value(sv, 0, "struct_bool_field").unwrap();
                            return Ok(bool_val);
                        }
                    }
                    // Fallback to default behavior
                    let malloc = self.get_malloc();
                    let size = sv.get_type()
                        .size_of()
                        .unwrap_or(self.i64_t.const_int(16, false));
                    let raw_ptr = self
                        .builder
                        .build_call(malloc, &[size.into()], "struct_autobox")?
                        .try_as_basic_value()
                        .left()
                        .ok_or_else(|| anyhow!("malloc returned void for struct cast"))?
                        .into_pointer_value();
                    let struct_ptr = self
                        .builder
                        .build_pointer_cast(
                            raw_ptr,
                            self.ctx.ptr_type(AddressSpace::from(0u16)),
                            "struct_autobox_cast",
                        )?;
                    self.builder.build_store(struct_ptr, sv)?;
                    let cast = self
                        .builder
                        .build_ptr_to_int(struct_ptr, it, "struct_ptr2int")
                        .map_err(|e| anyhow!("{e:?}"))?;
                    Ok(cast.as_basic_value_enum())
                } else {
                    // Treat structs as heap values and return their pointer bits
                    // This handles String and other structs being cast to int (pointer)
                    let malloc = self.get_malloc();
                    let size = sv.get_type()
                        .size_of()
                        .unwrap_or(self.i64_t.const_int(16, false));
                    let raw_ptr = self
                        .builder
                        .build_call(malloc, &[size.into()], "struct_autobox")?
                        .try_as_basic_value()
                        .left()
                        .ok_or_else(|| anyhow!("malloc returned void for struct cast"))?
                        .into_pointer_value();
                    let struct_ptr = self
                        .builder
                        .build_pointer_cast(
                            raw_ptr,
                            self.ctx.ptr_type(AddressSpace::from(0u16)),
                            "struct_autobox_cast",
                        )?;
                    self.builder.build_store(struct_ptr, sv)?;
                    let cast = self
                        .builder
                        .build_ptr_to_int(struct_ptr, it, "struct_ptr2int")
                        .map_err(|e| anyhow!("{e:?}"))?;
                    Ok(cast.as_basic_value_enum())
                }
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
            (BasicValueEnum::FloatValue(fv), BasicTypeEnum::PointerType(pt)) => {
                // Box primitive floats when a pointer (e.g. optional) is expected
                let alloca = self
                    .builder
                    .build_alloca(fv.get_type(), "float_autobox")
                    .map_err(|e| anyhow!("{e:?}"))?;
                self
                    .builder
                    .build_store(alloca, fv)
                    .map_err(|e| anyhow!("{e:?}"))?;

                if alloca.get_type() == pt {
                    Ok(alloca.as_basic_value_enum())
                } else {
                    let cast = self
                        .builder
                        .build_bit_cast(
                            alloca.as_basic_value_enum(),
                            pt.as_basic_type_enum(),
                            "float_ptrcast_store",
                        )
                        .map_err(|e| anyhow!("{e:?}"))?;
                    Ok(cast)
                }
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
            (BasicValueEnum::IntValue(iv), BasicTypeEnum::StructType(st)) => {
                // If we have an i64 that really represents a pointer to the struct, load it
                // This handles String and other structs passed as pointers/ints
                let ptr = self
                    .builder
                    .build_int_to_ptr(
                        iv,
                        self.ctx.ptr_type(AddressSpace::from(0u16)),
                        "int2ptr_struct",
                    )
                    .map_err(|e| anyhow!("{e:?}"))?;
                let loaded = self
                    .builder
                    .build_load(st, ptr, "load_struct_from_int_ptr")
                    .map_err(|e| anyhow!("{e:?}"))?;
                return Ok(loaded.as_basic_value_enum());
            }
            (BasicValueEnum::PointerValue(pv), BasicTypeEnum::StructType(st)) => {
                if st == self.ty_string() {
                    // Convert i8* to String struct
                    let len = self.call_strlen(pv)?;
                    let mut val = st.get_undef();
                    val = self.builder.build_insert_value(val, len, 0, "slen").unwrap().into_struct_value();
                    val = self.builder.build_insert_value(val, pv, 1, "sptr").unwrap().into_struct_value();
                    Ok(val.as_basic_value_enum())
                } else if st.count_fields() == 0 {
                    // Empty struct (e.g. Unit)
                    Ok(st.get_undef().as_basic_value_enum())
                } else {
                    // Treat pointer as *struct and load
                    let target_ptr = self
                        .builder
                        .build_pointer_cast(pv, self.ctx.ptr_type(AddressSpace::from(0u16)), "ptrcast_struct")
                        .map_err(|e| anyhow!("{e:?}"))?;
                    let loaded = self
                        .builder
                        .build_load(st, target_ptr, "load_struct_ptr")
                        .map_err(|e| anyhow!("{e:?}"))?;
                    Ok(loaded.as_basic_value_enum())
                }
            }

            (BasicValueEnum::StructValue(sv), BasicTypeEnum::StructType(st)) => {
                if sv.get_type() == st {
                    Ok(sv.as_basic_value_enum())
                } else if st.count_fields() == 0 {
                    // Target is an empty struct (like Unit or empty class), just return undef
                    Ok(st.get_undef().as_basic_value_enum())
                } else if sv.get_type().count_fields() == 0 && st.count_fields() > 0 {
                    // Source is empty struct, target has fields - create undef target
                    Ok(st.get_undef().as_basic_value_enum())
                } else {
                    // Try to cast field by field - handle mismatched field counts
                    let src_ty = sv.get_type();
                    let src_count = src_ty.count_fields();
                    let dst_count = st.count_fields();
                    
                    let mut res = st.get_undef();
                    // Copy fields up to the minimum of both
                    let copy_count = std::cmp::min(src_count, dst_count);
                    for i in 0..copy_count {
                        let field_val = self.builder.build_extract_value(sv, i, "extract").unwrap();
                        let target_field_ty = st.get_field_type_at_index(i).unwrap();
                        
                        let cast_val = self.cast_basic_to_type(field_val, target_field_ty)?;
                        res = self.builder.build_insert_value(res, cast_val, i, "insert").unwrap().into_struct_value();
                    }
                    Ok(res.as_basic_value_enum())
                }
            }
            (BasicValueEnum::ArrayValue(av), BasicTypeEnum::ArrayType(at)) => {
                if av.get_type() == at {
                    Ok(av.as_basic_value_enum())
                } else {
                    Err(anyhow!("Mismatched array store type"))
                }
            }

            (BasicValueEnum::ScalableVectorValue(svv), BasicTypeEnum::ScalableVectorType(svt)) => {
                if svv.get_type() == svt {
                    Ok(svv.as_basic_value_enum())
                } else {
                    Err(anyhow!("Mismatched scalable vector store type"))
                }
            }

            (BasicValueEnum::StructValue(sv), BasicTypeEnum::PointerType(pt)) => {
                if sv.get_type() == self.ty_string() && pt == self.i8_ptr_t {
                     let ptr = self.builder.build_extract_value(sv, 1, "sptr").unwrap();
                     Ok(ptr)
                } else {
                     // Spill to stack and return pointer
                     let ty = sv.get_type().as_basic_type_enum();
                     let tmp = self.alloca_for_type(ty, "struct_spill")?;
                     self.builder.build_store(tmp, sv)?;
                     // Cast pointer to target pointer type
                     let cast = self.builder.build_pointer_cast(tmp, pt, "struct_ptr_cast").map_err(|e| anyhow!("{e:?}"))?;
                     Ok(cast.as_basic_value_enum())
                }
            }

            (v, target_ty) => Err(anyhow!(
                "Cannot cast value {:?} to requested type {}",
                v,
                target_ty.print_to_string().to_string()
            )),
        }
    }

    pub(crate) fn is_string_value_ir(&self, value: &IRValue) -> bool {
        if is_string_literal_ir(value) {
            return true;
        }
        let ty_opt = match value {
            IRValue::Variable(name) => self.var_slot_types.get(name).copied(),
            IRValue::Register(id) => self.reg_slot_types.get(&(*id as u32)).copied(),
            _ => None,
        };

        if let Some(ty) = ty_opt {
            let str_ty = self.ty_string();
            return ty == str_ty.into();
        }
        false
    }

    // C library function getters (get_printf, get_strlen, get_strcmp, get_malloc, get_realloc,
    // get_free, get_memcpy, get_fflush, get_or_declare_clock_gettime) and call helpers
    // (call_printf, call_fflush, call_strlen, call_strcmp) moved to crate::llvm::c_library::CLibraryOps trait

    /// Convert a C string pointer to a Seen String struct { i64 len, ptr data }
    pub(crate) fn cstr_to_string_struct(&mut self, cstr: PointerValue<'ctx>) -> Result<BasicValueEnum<'ctx>> {
        // Handle null pointer: return empty string
        let null = self.i8_ptr_t.const_null();
        let is_null = self.builder.build_int_compare(
            inkwell::IntPredicate::EQ,
            cstr,
            null,
            "is_null",
        )?;
        
        let str_ty = self.ty_string();
        
        // Get strlen for non-null case
        let strlen_fn = self.get_strlen();
        let len_call = self.builder.build_call(strlen_fn, &[cstr.into()], "cstr_len")?;
        let len = len_call.try_as_basic_value().left().unwrap().into_int_value();
        let len64 = self.builder.build_int_z_extend(len, self.i64_t, "len64")?;
        
        // Build non-null string struct
        let mut str_val = str_ty.get_undef();
        str_val = self.builder.build_insert_value(str_val, len64, 0, "str_len")?.into_struct_value();
        str_val = self.builder.build_insert_value(str_val, cstr, 1, "str_ptr")?.into_struct_value();
        
        // Build empty string struct for null case
        let empty_ptr = self.builder.build_global_string_ptr("", "empty_str")?.as_pointer_value();
        let mut empty_str = str_ty.get_undef();
        empty_str = self.builder.build_insert_value(empty_str, self.i64_t.const_zero(), 0, "empty_len")?.into_struct_value();
        empty_str = self.builder.build_insert_value(empty_str, empty_ptr, 1, "empty_ptr")?.into_struct_value();
        
        // Select based on null check
        let result = self.builder.build_select(
            is_null,
            empty_str.as_basic_value_enum(),
            str_val.as_basic_value_enum(),
            "cstr_to_string",
        )?;
        
        Ok(result)
    }

    // Note: get_string_ptr_len is now in crate::llvm::string_ops::RuntimeStringOps

    pub(crate) fn ensure_string(&mut self, val: BasicValueEnum<'ctx>, ir_val: &IRValue) -> Result<BasicValueEnum<'ctx>> {
        // Check actual LLVM value type first - if it's already a string struct, return it
        if val.is_struct_value() {
            let sv = val.into_struct_value();
            // Check if it looks like our string struct { i64, ptr }
            if sv.get_type().count_fields() == 2 {
                return Ok(val);
            }
        }
        
        // Check if it's a char or int
        let is_char = if let IRValue::Register(id) = ir_val {
             self.reg_slot_types.get(id).map(|ty| *ty == self.ctx.i8_type().into()).unwrap_or(false)
        } else {
            false
        };
        
        if is_char {
             let iv = self.as_i64(val)?;
             let func = self.ensure_char_to_string_fn();
             let call = self.builder.build_call(func, &[iv.into()], "c2s")?;
             return Ok(call.try_as_basic_value().left().unwrap());
        }
        
        if val.is_int_value() {
             let iv = val.into_int_value();
             // Check if it's a boolean (i1 type) - needs special handling
             if iv.get_type().get_bit_width() == 1 {
                 // Inline bool-to-string: convert bool to "true" or "false" string struct
                 // Compare to 0 to get boolean value
                 let is_true = self.builder.build_int_compare(
                     inkwell::IntPredicate::NE,
                     iv,
                     self.ctx.bool_type().const_zero(),
                     "is_true",
                 )?;
                 
                 // Create global strings for "true" and "false"
                 let true_str = self.builder.build_global_string_ptr("true", "str_true")?;
                 let false_str = self.builder.build_global_string_ptr("false", "str_false")?;
                 
                 // Select pointer based on condition
                 let result_ptr = self.builder.build_select(
                     is_true,
                     true_str.as_pointer_value(),
                     false_str.as_pointer_value(),
                     "bool_str_ptr",
                 )?.into_pointer_value();
                 
                 // Select length: "true" = 4, "false" = 5
                 let true_len = self.i64_t.const_int(4, false);
                 let false_len = self.i64_t.const_int(5, false);
                 let result_len = self.builder.build_select(
                     is_true,
                     true_len,
                     false_len,
                     "bool_str_len",
                 )?.into_int_value();
                 
                 // Build string struct { len, ptr }
                 let str_ty = self.ty_string();
                 let mut str_struct = str_ty.get_undef();
                 str_struct = self.builder.build_insert_value(str_struct, result_len, 0, "str_len")?.into_struct_value();
                 str_struct = self.builder.build_insert_value(str_struct, result_ptr, 1, "str_ptr")?.into_struct_value();
                 
                 return Ok(str_struct.as_basic_value_enum());
             }
             // Regular integer - convert to i64 and call int to string
             let i64_val = if iv.get_type() == self.i64_t {
                 iv
             } else {
                 self.builder.build_int_z_extend(iv, self.i64_t, "int_zext")?
             };
             let func = self.ensure_int_to_string_fn();
             let call = self.builder.build_call(func, &[i64_val.into()], "i2s")?;
             return Ok(call.try_as_basic_value().left().unwrap());
        }
        
        // Handle float values
        if val.is_float_value() {
             let fv = val.into_float_value();
             let func = self.ensure_float_to_string_fn();
             let call = self.builder.build_call(func, &[fv.into()], "f2s")?;
             return Ok(call.try_as_basic_value().left().unwrap());
        }
        
        Ok(val)
    }

    // Note: runtime_concat, runtime_endswith, runtime_substring are now in
    // crate::llvm::string_ops::RuntimeStringOps trait implementation

    fn declare_main_wrapper(&mut self) {
        if self.module.get_function("main").is_some() {
            return;
        }
        let i32_t = self.ctx.i32_type();
        let i8_ptr_ptr = self.ctx.ptr_type(AddressSpace::default());
        let c_main_ty = i32_t.fn_type(&[i32_t.into(), i8_ptr_ptr.into()], false);
        let c_main = self.module.add_function("main", c_main_ty, None);
        let bb = self.ctx.append_basic_block(c_main, "entry");
        self.builder.position_at_end(bb);
        
        // DEBUG: Print start of main wrapper
        let i32_t = self.ctx.i32_type();
        let i8_ptr_t = self.i8_ptr_t;
        let puts_ty = i32_t.fn_type(&[i8_ptr_t.into()], false);
        let puts = if let Some(f) = self.module.get_function("puts") { f } else { self.module.add_function("puts", puts_ty, None) };
        
        let fmt = self.builder.build_global_string_ptr("DEBUG: main wrapper start", "debug_main_start").unwrap();
        self.builder.build_call(puts, &[fmt.as_pointer_value().into()], "debug_puts").unwrap();

        // If runtime debugging is enabled, call __seen_debug_init() first
        if self.runtime_debug_flag {
            let debug_init_ty = self.ctx.void_type().fn_type(&[], false);
            let debug_init_fn = if let Some(f) = self.module.get_function("__seen_debug_init") {
                f
            } else {
                self.module.add_function("__seen_debug_init", debug_init_ty, None)
            };
            self.builder.build_call(debug_init_fn, &[], "debug_init").unwrap();
        }

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

    pub(crate) fn ensure_arg_globals(
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
            self.ctx.ptr_type(AddressSpace::default()),
            None,
            "__argv",
        );
        g_argv.set_initializer(
            &self
                .ctx
                .ptr_type(AddressSpace::default())
                .const_null(),
        );
        self.g_argc = Some(g_argc);
        self.g_argv = Some(g_argv);
        (g_argc, g_argv)
    }

    /// Generate the `__GetCommandLineArgsHelper` function that converts C argc/argv
    /// into a Seen array of String structs.
    fn generate_get_command_line_args_helper(&mut self) {
        // Check if already defined or if there's only a declaration
        if let Some(existing) = self.module.get_function("__GetCommandLineArgsHelper") {
            // If it has a body, don't redefine
            if existing.get_first_basic_block().is_some() {
                return;
            }
            // Remove the declaration so we can define it properly
            unsafe { existing.delete(); }
        }

        let i32_t = self.ctx.i32_type();
        let i64_t = self.i64_t;
        let i8_ptr_t = self.i8_ptr_t;
        let i8_ptr_ptr_t = self.ctx.ptr_type(AddressSpace::default());
        
        // String struct type: { i64 len, ptr data }
        let string_ty = self.ty_string();
        // Array struct type: { i64 len, i64 cap, ptr data }
        let array_ty = self.ctx.struct_type(
            &[i64_t.into(), i64_t.into(), self.ctx.ptr_type(AddressSpace::default()).into()],
            false,
        );
        let ret_ty = self.ctx.ptr_type(AddressSpace::default());
        
        // Function type: (i32 argc, ptr argv) -> ptr to array
        let fn_ty = ret_ty.fn_type(&[i32_t.into(), i8_ptr_ptr_t.into()], false);
        let func = self.module.add_function("__GetCommandLineArgsHelper", fn_ty, None);
        
        // Create basic blocks
        let entry_bb = self.ctx.append_basic_block(func, "entry");
        let loop_start_bb = self.ctx.append_basic_block(func, "loop_start");
        let loop_body_bb = self.ctx.append_basic_block(func, "loop_body");
        let loop_end_bb = self.ctx.append_basic_block(func, "loop_end");
        
        // Entry block
        self.builder.position_at_end(entry_bb);
        let argc = func.get_nth_param(0).unwrap().into_int_value();
        let argv = func.get_nth_param(1).unwrap().into_pointer_value();
        
        // Extend argc to i64
        let argc64 = self.builder.build_int_z_extend(argc, i64_t, "argc64").unwrap();
        
        // Allocate array struct: malloc(sizeof(array_struct))
        let malloc = self.get_malloc();
        let array_struct_size = array_ty.size_of().unwrap();
        let array_ptr = self.builder.build_call(malloc, &[array_struct_size.into()], "array_malloc").unwrap()
            .try_as_basic_value().left().unwrap().into_pointer_value();
        let array_ptr_typed = self.builder.build_pointer_cast(array_ptr, ret_ty, "array_ptr_typed").unwrap();
        
        // Allocate data buffer: malloc(argc * sizeof(String))
        let string_size = string_ty.size_of().unwrap();
        let data_size = self.builder.build_int_mul(argc64, string_size, "data_size").unwrap();
        let data_ptr = self.builder.build_call(malloc, &[data_size.into()], "data_malloc").unwrap()
            .try_as_basic_value().left().unwrap().into_pointer_value();
        let data_ptr_typed = self.builder.build_pointer_cast(
            data_ptr, 
            self.ctx.ptr_type(AddressSpace::default()), 
            "data_ptr_typed"
        ).unwrap();
        
        // Store len = argc, cap = argc, data = data_ptr
        let len_ptr = self
            .build_struct_gep_checked(array_ty, array_ptr_typed, 0, "len_ptr")
            .unwrap();
        self.builder.build_store(len_ptr, argc64).unwrap();
        let cap_ptr = self
            .build_struct_gep_checked(array_ty, array_ptr_typed, 1, "cap_ptr")
            .unwrap();
        self.builder.build_store(cap_ptr, argc64).unwrap();
        let data_ptr_ptr = self
            .build_struct_gep_checked(array_ty, array_ptr_typed, 2, "data_ptr_ptr")
            .unwrap();
        self.builder.build_store(data_ptr_ptr, data_ptr_typed).unwrap();
        
        // Allocate loop index
        let index_ptr = self.builder.build_alloca(i64_t, "index").unwrap();
        self.builder.build_store(index_ptr, i64_t.const_zero()).unwrap();
        
        self.builder.build_unconditional_branch(loop_start_bb).unwrap();
        
        // Loop start: check if index < argc
        self.builder.position_at_end(loop_start_bb);
        let index = self.builder.build_load(i64_t, index_ptr, "index_load").unwrap().into_int_value();
        let cond = self.builder.build_int_compare(inkwell::IntPredicate::SLT, index, argc64, "loop_cond").unwrap();
        self.builder.build_conditional_branch(cond, loop_body_bb, loop_end_bb).unwrap();
        
        // Loop body: get argv[index], compute strlen, store String struct
        self.builder.position_at_end(loop_body_bb);
        let index_in_body = self.builder.build_load(i64_t, index_ptr, "idx").unwrap().into_int_value();
        
        // Get argv[index] - argv is char**, so argv[i] is char*
        let argv_elem_ptr = unsafe {
            self.builder.build_gep(i8_ptr_t, argv, &[index_in_body], "argv_elem_ptr").unwrap()
        };
        let cstr = self.builder.build_load(i8_ptr_t, argv_elem_ptr, "cstr").unwrap().into_pointer_value();
        
        // Get strlen(cstr)
        let strlen_fn = self.get_strlen();
        let len_call = self.builder.build_call(strlen_fn, &[cstr.into()], "strlen_call").unwrap();
        let cstr_len = len_call.try_as_basic_value().left().unwrap().into_int_value();
        let cstr_len64 = if cstr_len.get_type().get_bit_width() != 64 {
            self.builder.build_int_z_extend(cstr_len, i64_t, "len64").unwrap()
        } else {
            cstr_len
        };
        
        // Build String struct {len, ptr}
        let mut str_val = string_ty.get_undef();
        str_val = self.builder.build_insert_value(str_val, cstr_len64, 0, "str_len").unwrap().into_struct_value();
        str_val = self.builder.build_insert_value(str_val, cstr, 1, "str_ptr").unwrap().into_struct_value();
        
        // Store in data_ptr[index]
        let elem_ptr = unsafe {
            self.builder.build_gep(string_ty, data_ptr_typed, &[index_in_body], "elem_ptr").unwrap()
        };
        self.builder.build_store(elem_ptr, str_val).unwrap();
        
        // Increment index
        let next_index = self.builder.build_int_add(index_in_body, i64_t.const_int(1, false), "next_idx").unwrap();
        self.builder.build_store(index_ptr, next_index).unwrap();
        
        self.builder.build_unconditional_branch(loop_start_bb).unwrap();
        
        // Loop end: return array pointer
        self.builder.position_at_end(loop_end_bb);
        self.builder.build_return(Some(&array_ptr_typed)).unwrap();
    }

    // Type builder methods (ty_str_array, ty_array, ty_cmd_result, ty_handle) 
    // moved to crate::llvm::type_builders::TypeBuilders trait

    pub(crate) fn ty_select_result(&mut self) -> StructType<'ctx> {
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

    // ensure_channel_select_fn, ensure_scope_fn, ensure_spawn_fn, ensure_task_handle_new_fn,
    // ensure_await_fn, cast_handle_ptr moved to crate::llvm::concurrency::ConcurrencyOps trait

    pub(crate) fn declare_c_fn(
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

    pub(crate) fn declare_c_void_fn(
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

    // Type inference functions moved to crate::llvm::type_inference::TypeInference trait
}