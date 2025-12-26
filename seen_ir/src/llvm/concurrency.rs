//! Concurrency operations for the LLVM backend.
//!
//! This module handles channel operations, spawn, scope, and await
//! for the Seen concurrency model.

use indexmap::IndexMap;
use anyhow::{anyhow, Result};
use inkwell::values::{BasicValue, BasicValueEnum, FunctionValue, PointerValue};
use inkwell::AddressSpace;

use crate::llvm_backend::LlvmBackend;
use crate::llvm::type_cast::TypeCastOps;
use crate::instruction::IRSelectArm;
use crate::value::IRValue;

// Use IndexMap like the main backend for deterministic iteration
type HashMap<K, V> = IndexMap<K, V>;

/// Trait for concurrency operations on the LLVM backend.
pub trait ConcurrencyOps<'ctx> {
    /// Ensure the channel send function is declared.
    fn ensure_channel_send_fn(&mut self) -> FunctionValue<'ctx>;
    
    /// Ensure the channel select function is declared.
    fn ensure_channel_select_fn(&mut self) -> FunctionValue<'ctx>;
    
    /// Ensure a scope function is declared.
    fn ensure_scope_fn(&mut self, name: &str) -> FunctionValue<'ctx>;
    
    /// Ensure a spawn function is declared.
    fn ensure_spawn_fn(&mut self, name: &str) -> FunctionValue<'ctx>;
    
    /// Ensure the task handle new function is declared.
    fn ensure_task_handle_new_fn(&mut self) -> FunctionValue<'ctx>;
    
    /// Ensure the await function is declared.
    fn ensure_await_fn(&mut self) -> FunctionValue<'ctx>;
    
    /// Cast a value to a task handle pointer.
    fn cast_handle_ptr(&mut self, value: BasicValueEnum<'ctx>, label: &str) -> Result<PointerValue<'ctx>>;
    
    /// Lower a channel select instruction.
    fn lower_channel_select(
        &mut self,
        cases: &[IRSelectArm],
        payload_result: &IRValue,
        index_result: &IRValue,
        status_result: &IRValue,
        fn_map: &HashMap<String, FunctionValue<'ctx>>,
    ) -> Result<()>;
}

impl<'ctx> ConcurrencyOps<'ctx> for LlvmBackend<'ctx> {
    fn ensure_channel_send_fn(&mut self) -> FunctionValue<'ctx> {
        let ty = self.i8_ptr_t.fn_type(&[self.i8_ptr_t.into(), self.i8_ptr_t.into()], false);
        self.declare_if_missing("seen_channel_send", ty)
    }

    fn ensure_channel_select_fn(&mut self) -> FunctionValue<'ctx> {
        let fn_ty = self.i8_ptr_t.fn_type(
            &[
                self.i8_ptr_t.into(),
                self.i8_ptr_t.into(),
                self.i64_t.into(),
            ],
            false,
        );
        self.declare_if_missing("seen_channel_select", fn_ty)
    }

    fn ensure_scope_fn(&mut self, name: &str) -> FunctionValue<'ctx> {
        let fn_ty = self.ctx.void_type().fn_type(&[self.ctx.i32_type().into()], false);
        self.declare_if_missing(name, fn_ty)
    }

    fn ensure_spawn_fn(&mut self, name: &str) -> FunctionValue<'ctx> {
        let ptr_ty = self.ctx.ptr_type(AddressSpace::default());
        let fn_ty = ptr_ty.fn_type(&[], false);
        self.declare_if_missing(name, fn_ty)
    }

    fn ensure_task_handle_new_fn(&mut self) -> FunctionValue<'ctx> {
        let ptr_ty = self.ctx.ptr_type(AddressSpace::default());
        let fn_ty = ptr_ty.fn_type(&[self.ctx.i32_type().into()], false);
        self.declare_if_missing("__task_handle_new", fn_ty)
    }

    fn ensure_await_fn(&mut self) -> FunctionValue<'ctx> {
        let ret_ty = self.ctx.i32_type();
        let arg_ty = self.ctx.ptr_type(AddressSpace::default());
        let fn_ty = ret_ty.fn_type(&[arg_ty.into()], false);
        self.declare_if_missing("__await", fn_ty)
    }

    fn cast_handle_ptr(
        &mut self,
        value: BasicValueEnum<'ctx>,
        label: &str,
    ) -> Result<PointerValue<'ctx>> {
        let handle_ptr_ty = self.ctx.ptr_type(AddressSpace::default());
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
}
