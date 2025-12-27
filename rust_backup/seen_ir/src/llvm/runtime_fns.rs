//! Runtime function declarations for the LLVM backend.
//!
//! This module centralizes all the `ensure_*_fn` methods that declare
//! external runtime functions used by the Seen compiler.

use anyhow::{anyhow, Result};
use inkwell::values::{BasicValue, BasicValueEnum, FunctionValue, PointerValue};

use crate::llvm_backend::LlvmBackend;
use crate::llvm::type_cast::TypeCastOps;

/// Trait for runtime function declarations.
pub trait RuntimeFunctions<'ctx> {
    // Boxing functions
    fn ensure_box_int_fn(&mut self) -> FunctionValue<'ctx>;
    fn ensure_box_bool_fn(&mut self) -> FunctionValue<'ctx>;
    fn ensure_box_ptr_fn(&mut self) -> FunctionValue<'ctx>;
    fn box_runtime_value(&mut self, value: BasicValueEnum<'ctx>) -> Result<PointerValue<'ctx>>;

    // String conversion functions
    fn ensure_int_to_string_fn(&mut self) -> FunctionValue<'ctx>;
    fn ensure_char_to_string_fn(&mut self) -> FunctionValue<'ctx>;
    fn ensure_float_to_string_fn(&mut self) -> FunctionValue<'ctx>;
    fn ensure_bool_to_string_fn(&mut self) -> FunctionValue<'ctx>;
}

impl<'ctx> RuntimeFunctions<'ctx> for LlvmBackend<'ctx> {
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

    fn ensure_int_to_string_fn(&mut self) -> FunctionValue<'ctx> {
        let ty = self.ty_string().fn_type(&[self.i64_t.into()], false);
        self.declare_if_missing("__IntToString", ty)
    }

    fn ensure_char_to_string_fn(&mut self) -> FunctionValue<'ctx> {
        let ty = self.ty_string().fn_type(&[self.i64_t.into()], false);
        self.declare_if_missing("__CharToString", ty)
    }

    fn ensure_float_to_string_fn(&mut self) -> FunctionValue<'ctx> {
        let ty = self.ty_string().fn_type(&[self.ctx.f64_type().into()], false);
        self.declare_if_missing("__FloatToString", ty)
    }

    fn ensure_bool_to_string_fn(&mut self) -> FunctionValue<'ctx> {
        // Takes i64 (0 or 1), returns string struct
        let ty = self.ty_string().fn_type(&[self.i64_t.into()], false);
        self.declare_if_missing("__BoolToString", ty)
    }
}
