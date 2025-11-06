#![cfg(feature = "llvm")]
//! LLVM backend that lowers Seen IR (IRProgram) to native code via inkwell.
//!
//! Scope: Implements a minimal but solid subset required to compile the
//! self‑hosting entry (`compiler_seen/src/main.seen`) and similar programs.

use std::collections::HashMap;
use std::convert::TryFrom;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use inkwell::basic_block::BasicBlock as LlvmBasicBlock;
use inkwell::builder::Builder;
use inkwell::context::Context as LlvmContext;
use inkwell::module::{Linkage, Module as LlvmModule};
use inkwell::targets::{
    CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetMachine,
};
use inkwell::types::{
    BasicMetadataTypeEnum, BasicType, BasicTypeEnum, IntType, PointerType, StructType,
};
use inkwell::values::{
    BasicMetadataValueEnum, BasicValue, BasicValueEnum, FunctionValue, GlobalValue, PointerValue,
    UnnamedAddress,
};
use inkwell::OptimizationLevel as LlvmOptLevel;

use crate::function::IRFunction;
use crate::instruction::{BasicBlock, Instruction};
use crate::module::IRModule;
use crate::value::{IRType, IRValue};
use crate::IRProgram;

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

pub struct LlvmBackend<'ctx> {
    ctx: &'ctx LlvmContext,
    module: LlvmModule<'ctx>,
    builder: Builder<'ctx>,
    i64_t: IntType<'ctx>,
    bool_t: IntType<'ctx>,
    i8_ptr_t: PointerType<'ctx>,
    handle_ty: Option<StructType<'ctx>>,

    // Runtime/extern declarations
    printf: Option<FunctionValue<'ctx>>,
    strlen: Option<FunctionValue<'ctx>>,
    strcmp: Option<FunctionValue<'ctx>>,
    malloc: Option<FunctionValue<'ctx>>,
    free: Option<FunctionValue<'ctx>>,
    memcpy: Option<FunctionValue<'ctx>>,

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
    task_counter: Option<GlobalValue<'ctx>>,
    actor_counter: Option<GlobalValue<'ctx>>,
    byte_array_globals: HashMap<Vec<u8>, GlobalValue<'ctx>>,
}

impl<'ctx> LlvmBackend<'ctx> {
    pub fn new() -> Self {
        // Initialize native targets once
        Target::initialize_native(&InitializationConfig::default())
            .expect("Failed to initialize LLVM native target");

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
            printf: None,
            strlen: None,
            strcmp: None,
            malloc: None,
            free: None,
            memcpy: None,
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
            task_counter: None,
            actor_counter: None,
            byte_array_globals: HashMap::new(),
        }
    }

    pub fn emit_llvm_ir(&mut self, prog: &IRProgram, out_path: &Path) -> Result<()> {
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
    ) -> Result<()> {
        self.lower_program(prog)
            .context("Lowering IR to LLVM failed")?;

        // Build object
        let target_triple = TargetMachine::get_default_triple();
        let target = Target::from_triple(&target_triple)
            .map_err(|e| anyhow!("Target from triple failed: {e:?}"))?;
        let tm = target
            .create_target_machine(
                &target_triple,
                "generic",
                "",
                LlvmOptLevel::Default,
                RelocMode::Default,
                CodeModel::Default,
            )
            .ok_or_else(|| anyhow!("Create target machine failed"))?;

        let obj_path = out_path.with_extension("o");
        eprintln!("LLVM backend: writing object file {:?}", obj_path);
        tm.write_to_file(&self.module, FileType::Object, &obj_path)
            .map_err(|e| anyhow!("Write object failed: {e:?}"))?;

        if matches!(kind, LinkOutput::ObjectOnly) {
            return Ok(());
        }

        // Link to executable via system linker (honor SEEN_LLVM_LINKER or fallback to cc/clang)
        let linker = std::env::var("SEEN_LLVM_LINKER")
            .ok()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| {
                if which::which("cc").is_ok() {
                    "cc".to_string()
                } else {
                    "clang".to_string()
                }
            });
        eprintln!("LLVM backend: invoking linker {}", linker);
        match kind {
            LinkOutput::Executable => {
                let status = std::process::Command::new(&linker)
                    .arg(&obj_path)
                    .arg("-o")
                    .arg(out_path)
                    .arg("-no-pie")
                    .arg("-lm")
                    .status()
                    .with_context(|| format!("Spawning linker ({})", linker))?;
                if !status.success() {
                    return Err(anyhow!("Linking failed with status {status}"));
                }
                Ok(())
            }
            LinkOutput::SharedLibrary => {
                let mut cmd = std::process::Command::new(&linker);
                cmd.arg(&obj_path).arg("-o").arg(out_path);
                if cfg!(target_os = "macos") {
                    cmd.arg("-dynamiclib");
                } else if cfg!(target_os = "windows") {
                    cmd.arg("-shared");
                } else {
                    cmd.arg("-shared");
                }
                cmd.arg("-lm");
                let status = cmd
                    .status()
                    .with_context(|| format!("Spawning linker ({})", linker))?;
                if !status.success() {
                    return Err(anyhow!(
                        "Shared library linking failed with status {status}"
                    ));
                }
                Ok(())
            }
            LinkOutput::StaticLibrary => {
                #[cfg(target_os = "windows")]
                let tool = std::env::var("SEEN_LLVM_ARCHIVER")
                    .ok()
                    .filter(|v| !v.is_empty())
                    .unwrap_or_else(|| "lib".to_string());
                #[cfg(not(target_os = "windows"))]
                let tool = std::env::var("SEEN_LLVM_ARCHIVER")
                    .ok()
                    .filter(|v| !v.is_empty())
                    .unwrap_or_else(|| "ar".to_string());

                #[cfg(target_os = "windows")]
                let status = std::process::Command::new(&tool)
                    .arg("/nologo")
                    .arg(format!("/OUT:{}", out_path.display()))
                    .arg(&obj_path)
                    .status()
                    .with_context(|| format!("Spawning archiver ({})", tool))?;
                #[cfg(not(target_os = "windows"))]
                let status = std::process::Command::new(&tool)
                    .arg("crus")
                    .arg(out_path)
                    .arg(&obj_path)
                    .status()
                    .with_context(|| format!("Spawning archiver ({})", tool))?;

                if !status.success() {
                    return Err(anyhow!("Archiving failed with status {status}"));
                }

                #[cfg(target_os = "macos")]
                {
                    let ranlib = std::env::var("SEEN_LLVM_RANLIB")
                        .ok()
                        .filter(|v| !v.is_empty())
                        .unwrap_or_else(|| "ranlib".to_string());
                    let status = std::process::Command::new(&ranlib)
                        .arg(out_path)
                        .status()
                        .with_context(|| format!("Spawning ranlib ({})", ranlib))?;
                    if !status.success() {
                        return Err(anyhow!("ranlib failed with status {status}"));
                    }
                }

                Ok(())
            }
            LinkOutput::ObjectOnly => unreachable!("handled earlier"),
        }
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
                // Represent arrays as opaque pointer for now
                self.i8_ptr_t.into()
            }
            IRType::Function {
                parameters,
                return_type,
            } => {
                let fn_ty = self.fn_type_from_ir(return_type, parameters);
                fn_ty.ptr_type(inkwell::AddressSpace::from(0u16)).into()
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
        Ok(f)
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

    fn define_function(
        &mut self,
        func: &IRFunction,
        f: FunctionValue<'ctx>,
        fn_map: &HashMap<String, FunctionValue<'ctx>>,
    ) -> Result<()> {
        self.current_fn = Some(f);
        self.reg_values.clear();
        self.var_values.clear();
        self.var_slots.clear();
        self.var_slot_types.clear();
        self.blocks.clear();
        self.reg_slots.clear();

        // Create all basic blocks first
        let block_names: Vec<_> = if !func.cfg.block_order.is_empty() {
            func.cfg.block_order.clone()
        } else {
            let mut names: Vec<_> = func.cfg.blocks.keys().cloned().collect();
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
        }
        for local in func.locals.values() {
            let ty = self.ir_type_to_llvm(&local.var_type);
            self.var_slot_types.insert(local.name.clone(), ty);
            let slot = self.alloca_for_type(ty, &format!("local_slot_{}", local.name))?;
            self.var_slots.insert(local.name.clone(), slot);
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

            let b = func.cfg.blocks.get(name).unwrap();
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
                    (Some(_), None) | (None, _) => {
                        self.builder.build_return(None)?;
                    }
                }
                self.builder.clear_insertion_position();
            }
            Instruction::Move { source, dest } => {
                let v = self.eval_value(source, fn_map)?;
                self.assign_value(dest, v)?;
            }
            Instruction::Store { value, dest } => {
                let v = self.eval_value(value, fn_map)?;
                self.assign_value(dest, v)?;
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
                let res = match op {
                    crate::instruction::BinaryOp::Add => {
                        let li = self.as_i64(l.clone())?;
                        let ri = self.as_i64(r.clone())?;
                        self.builder
                            .build_int_add(li, ri, "add")?
                            .as_basic_value_enum()
                    }
                    crate::instruction::BinaryOp::Subtract => {
                        let li = self.as_i64(l.clone())?;
                        let ri = self.as_i64(r.clone())?;
                        self.builder
                            .build_int_sub(li, ri, "sub")?
                            .as_basic_value_enum()
                    }
                    crate::instruction::BinaryOp::Multiply => {
                        let li = self.as_i64(l.clone())?;
                        let ri = self.as_i64(r.clone())?;
                        self.builder
                            .build_int_mul(li, ri, "mul")?
                            .as_basic_value_enum()
                    }
                    crate::instruction::BinaryOp::Divide => {
                        let li = self.as_i64(l.clone())?;
                        let ri = self.as_i64(r.clone())?;
                        self.builder
                            .build_int_signed_div(li, ri, "div")?
                            .as_basic_value_enum()
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
                let l = self.as_cstr_ptr(lval)?;
                let r = self.as_cstr_ptr(rval)?;
                let out = self.runtime_concat(l, r)?;
                self.assign_value(result, out.as_basic_value_enum())?;
            }
            Instruction::ArrayLength { array, result } => {
                // Constant arrays or runtime StrArray* with layout { i64 len; i8** data }
                let arr_v = self.eval_value(array, fn_map)?;
                let res = if let IRValue::Array(values) = array {
                    self.i64_t
                        .const_int(values.len() as u64, false)
                        .as_basic_value_enum()
                } else if arr_v.is_pointer_value() || arr_v.is_int_value() {
                    let ty = self.ty_str_array();
                    let arr_ptr = if arr_v.is_pointer_value() {
                        arr_v.into_pointer_value()
                    } else {
                        self.builder
                            .build_int_to_ptr(
                                arr_v.into_int_value(),
                                ty.ptr_type(inkwell::AddressSpace::from(0u16)),
                                "arr_len_ptr",
                            )
                            .map_err(|e| anyhow!("{e:?}"))?
                    };
                    let len_ptr = self.builder.build_struct_gep(ty, arr_ptr, 0, "len_ptr")?;
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
                if let IRValue::Array(vs) = array {
                    let idx_bv = self.eval_value(index, fn_map)?;
                    let idx_val = self.as_usize(idx_bv)? as usize;
                    if idx_val >= vs.len() {
                        return Err(anyhow!("Array index OOB"));
                    }
                    let elem = self.eval_value(&vs[idx_val], fn_map)?;
                    self.assign_value(result, elem)?;
                } else if arr_v.is_pointer_value() || arr_v.is_int_value() {
                    // Treat as StrArray*
                    let ty = self.ty_str_array();
                    let arr_ptr = if arr_v.is_pointer_value() {
                        arr_v.into_pointer_value()
                    } else {
                        self.builder
                            .build_int_to_ptr(
                                arr_v.into_int_value(),
                                ty.ptr_type(inkwell::AddressSpace::from(0u16)),
                                "arr_int_to_ptr",
                            )
                            .map_err(|e| anyhow!("{e:?}"))?
                    };
                    let data_ptr_ptr = self.builder.build_struct_gep(ty, arr_ptr, 1, "data_ptr")?;
                    let data_pp = self.builder.build_load(
                        self.i8_ptr_t.ptr_type(inkwell::AddressSpace::from(0u16)),
                        data_ptr_ptr,
                        "data",
                    )?;
                    let idx_bv = self.eval_value(index, fn_map)?;
                    let idx_iv = self.as_i64(idx_bv)?;
                    let elem_ptr_ptr = unsafe {
                        self.builder.build_gep(
                            self.i8_ptr_t,
                            data_pp.into_pointer_value(),
                            &[idx_iv],
                            "elempp",
                        )?
                    };
                    let elem_ptr = self
                        .builder
                        .build_load(self.i8_ptr_t, elem_ptr_ptr, "elem")?;
                    self.assign_value(result, elem_ptr.as_basic_value_enum())?;
                } else {
                    return Err(anyhow!("Unsupported array access value"));
                }
            }
            Instruction::Call {
                target,
                args,
                result,
            } => {
                // Handle known intrinsics
                if let IRValue::Variable(name) = target {
                    match name.as_str() {
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
                            self.builder.position_at_end(then_bb);
                            let empty = self.builder.build_global_string_ptr("", "empty")?;
                            if let Some(r) = result {
                                self.assign_value(
                                    r,
                                    empty.as_pointer_value().as_basic_value_enum(),
                                )?;
                            }
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
                            if let Some(r) = result {
                                self.assign_value(r, bufp.as_basic_value_enum())?;
                            }
                            self.builder.build_unconditional_branch(done_bb)?;
                            self.builder.position_at_end(done_bb);
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
                            if let Some(arg0) = args.get(0) {
                                let _ = self.eval_value(arg0, fn_map)?;
                            }
                            if let Some(arg1) = args.get(1) {
                                let _ = self.eval_value(arg1, fn_map)?;
                            }
                            let counter = self.ensure_task_counter();
                            let handle = self.runtime_spawn_handle(counter)?;
                            if let Some(r) = result {
                                self.assign_value(r, handle)?;
                            }
                            return Ok(());
                        }
                        "__spawn_task" => {
                            if let Some(arg0) = args.get(0) {
                                let _ = self.eval_value(arg0, fn_map)?;
                            }
                            let counter = self.ensure_task_counter();
                            let handle = self.runtime_spawn_handle(counter)?;
                            if let Some(r) = result {
                                self.assign_value(r, handle)?;
                            }
                            return Ok(());
                        }
                        "__spawn_actor" => {
                            if let Some(arg0) = args.get(0) {
                                let _ = self.eval_value(arg0, fn_map)?;
                            }
                            let counter = self.ensure_actor_counter();
                            let handle = self.runtime_spawn_handle(counter)?;
                            if let Some(r) = result {
                                self.assign_value(r, handle)?;
                            }
                            return Ok(());
                        }
                        "__await" => {
                            if let Some(arg0) = args.get(0) {
                                let handle_val = self.eval_value(arg0, fn_map)?;
                                let handle_ptr = self.as_cstr_ptr(handle_val)?;
                                let handle_ty = self.ty_handle();
                                let struct_ptr = self
                                    .builder
                                    .build_pointer_cast(
                                        handle_ptr,
                                        handle_ty.ptr_type(inkwell::AddressSpace::from(0u16)),
                                        "await_handle",
                                    )
                                    .map_err(|e| anyhow!("{e:?}"))?;
                                let gen_gep = self
                                    .builder
                                    .build_struct_gep(handle_ty, struct_ptr, 1, "await_gen")
                                    .map_err(|e| anyhow!("{e:?}"))?;
                                let gen_val = self
                                    .builder
                                    .build_load(self.ctx.i32_type(), gen_gep, "await_gen_val")
                                    .map_err(|e| anyhow!("{e:?}"))?
                                    .into_int_value();
                                let next = self
                                    .builder
                                    .build_int_add(
                                        gen_val,
                                        self.ctx.i32_type().const_int(1, false),
                                        "await_gen_next",
                                    )
                                    .map_err(|e| anyhow!("{e:?}"))?;
                                self.builder
                                    .build_store(gen_gep, next.as_basic_value_enum())
                                    .map_err(|e| anyhow!("{e:?}"))?;
                            }
                            if let Some(r) = result {
                                let success = self.bool_t.const_int(1, false);
                                self.assign_value(r, success.as_basic_value_enum())?;
                            }
                            return Ok(());
                        }
                        _ => {}
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
                for a in args {
                    let v = self.eval_value(a, fn_map)?;
                    call_args.push(v.into());
                }
                let call = self.builder.build_call(f, &call_args, "call")?;
                if let Some(r) = result {
                    if let Some(ret) = call.try_as_basic_value().left() {
                        self.assign_value(r, ret)?;
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
                // Support CommandResult{ success: i1, output: i8* }
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
            _ => {
                // Many IR ops are not required for bootstrap subset; ignore nops etc.
            }
        }
        Ok(())
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
                if let Some(slot) = self.reg_slots.get(r).copied() {
                    let loaded =
                        self.builder
                            .build_load(self.i64_t, slot, &format!("load_r{}", r))?;
                    return Ok(loaded.as_basic_value_enum());
                }
                if let Some(val) = self.reg_values.get(r).cloned() {
                    return Ok(val);
                }
                Err(anyhow!("Unknown register %r{r}"))
            }
            IRValue::Variable(name) => {
                if let Some(slot) = self.var_slots.get(name).copied() {
                    let loaded = self.load_from_slot(name, slot)?;
                    return Ok(loaded);
                }
                if let Some(v) = self.var_values.get(name).cloned() {
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
            _ => Err(anyhow!("Unsupported IRValue in LLVM backend: {v:?}")),
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
                let stored = self.cast_basic_to_type(v, slot_ty)?;
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
        } else {
            Err(anyhow!("Expected integer value"))
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
            (BasicValueEnum::FloatValue(fv), BasicTypeEnum::FloatType(ft)) => {
                if fv.get_type() == ft {
                    Ok(fv.as_basic_value_enum())
                } else {
                    Err(anyhow!("Mismatched float store type"))
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

    fn call_printf(&mut self, args: &[BasicMetadataValueEnum<'ctx>]) -> Result<()> {
        let printf = self.get_printf();
        self.builder
            .build_call(printf, args, "printf_call")
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
        if let Some(seen) = self.module.get_function("seen_main") {
            let call = self.builder.build_call(seen, &[], "call_main").unwrap();
            if let Some(ret) = call.try_as_basic_value().left() {
                let i64v = ret.into_int_value();
                let i32v = self.builder.build_int_truncate(i64v, i32_t, "tr").unwrap();
                self.builder.build_return(Some(&i32v)).unwrap();
            } else {
                self.builder
                    .build_return(Some(&i32_t.const_zero()))
                    .unwrap();
            }
        } else {
            self.builder
                .build_return(Some(&i32_t.const_zero()))
                .unwrap();
        }
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

    fn ensure_task_counter(&mut self) -> GlobalValue<'ctx> {
        if let Some(counter) = self.task_counter {
            counter
        } else {
            let counter = self
                .module
                .add_global(self.ctx.i32_type(), None, "__task_slot_counter");
            counter.set_initializer(&self.ctx.i32_type().const_zero());
            self.task_counter = Some(counter);
            counter
        }
    }

    fn ensure_actor_counter(&mut self) -> GlobalValue<'ctx> {
        if let Some(counter) = self.actor_counter {
            counter
        } else {
            let counter = self
                .module
                .add_global(self.ctx.i32_type(), None, "__actor_slot_counter");
            counter.set_initializer(&self.ctx.i32_type().const_zero());
            self.actor_counter = Some(counter);
            counter
        }
    }

    fn runtime_spawn_handle(&mut self, counter: GlobalValue<'ctx>) -> Result<BasicValueEnum<'ctx>> {
        let i32_t = self.ctx.i32_type();
        let slot_ptr = counter.as_pointer_value();
        let cur_val = self
            .builder
            .build_load(i32_t, slot_ptr, "slot_cur")
            .map_err(|e| anyhow!("{e:?}"))?;
        let cur = cur_val.into_int_value();
        let next = self
            .builder
            .build_int_add(cur, i32_t.const_int(1, false), "slot_next")
            .map_err(|e| anyhow!("{e:?}"))?;
        self.builder
            .build_store(slot_ptr, next.as_basic_value_enum())
            .map_err(|e| anyhow!("{e:?}"))?;

        let handle_ty = self.ty_handle();
        let malloc = self.get_malloc();
        let size = self.i64_t.const_int(8, false); // two i32 fields
        let raw = self
            .builder
            .build_call(malloc, &[size.into()], "malloc_handle")?;
        let raw_ptr = raw
            .try_as_basic_value()
            .left()
            .ok_or_else(|| anyhow!("malloc returned void"))?
            .into_pointer_value();
        let handle_ptr = self
            .builder
            .build_pointer_cast(
                raw_ptr,
                handle_ty.ptr_type(inkwell::AddressSpace::from(0u16)),
                "handle_cast",
            )
            .map_err(|e| anyhow!("{e:?}"))?;
        let slot_gep = self
            .builder
            .build_struct_gep(handle_ty, handle_ptr, 0, "slot_gep")
            .map_err(|e| anyhow!("{e:?}"))?;
        self.builder
            .build_store(slot_gep, cur.as_basic_value_enum())
            .map_err(|e| anyhow!("{e:?}"))?;
        let gen_gep = self
            .builder
            .build_struct_gep(handle_ty, handle_ptr, 1, "gen_gep")
            .map_err(|e| anyhow!("{e:?}"))?;
        self.builder
            .build_store(gen_gep, i32_t.const_zero().as_basic_value_enum())
            .map_err(|e| anyhow!("{e:?}"))?;
        let raw_i8 = self
            .builder
            .build_pointer_cast(handle_ptr, self.i8_ptr_t, "handle_ret")
            .map_err(|e| anyhow!("{e:?}"))?;
        Ok(raw_i8.as_basic_value_enum())
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
