//! Type builders for the LLVM backend.
//!
//! This module provides methods for constructing LLVM types that represent
//! Seen language constructs (strings, arrays, handles, etc.).

use inkwell::types::{BasicMetadataTypeEnum, BasicType, BasicTypeEnum, StructType};
use inkwell::AddressSpace;

use crate::llvm_backend::LlvmBackend;
use crate::value::IRType;

/// Trait for building LLVM types from Seen type constructs.
pub trait TypeBuilders<'ctx> {
    /// Build the LLVM string type: { i64 len, ptr data }.
    fn ty_string(&self) -> StructType<'ctx>;

    /// Build an array type: { i64 len, i64 cap, i64 element_size, ptr data }.
    fn ty_array(&self, elem_ty: BasicTypeEnum<'ctx>) -> StructType<'ctx>;

    /// Build a string array type: { i64 len, ptr to ptr }.
    fn ty_str_array(&self) -> StructType<'ctx>;

    /// Build the command result type: { i8 success, String output }.
    fn ty_cmd_result(&self) -> StructType<'ctx>;

    /// Build or get the cached handle type: { i32, i32 }.
    fn ty_handle(&mut self) -> StructType<'ctx>;

    /// Build an LLVM function type from IR parameter and return types.
    fn fn_type_from_ir(
        &self,
        ret: &IRType,
        params: &[IRType],
    ) -> inkwell::types::FunctionType<'ctx>;

    /// Convert an IR type to an LLVM type for function parameters.
    /// Struct types are passed as pointers (consistent with C ABI).
    fn ir_type_to_llvm_param(&self, t: &IRType) -> BasicTypeEnum<'ctx>;
}

impl<'ctx> TypeBuilders<'ctx> for LlvmBackend<'ctx> {
    fn ty_string(&self) -> StructType<'ctx> {
        self.ctx.struct_type(&[self.i64_t.into(), self.i8_ptr_t.into()], false)
    }

    fn ty_array(&self, _elem_ty: BasicTypeEnum<'ctx>) -> StructType<'ctx> {
        self.ctx.struct_type(
            &[
                self.i64_t.into(), // len (index 0)
                self.i64_t.into(), // cap (index 1)
                self.i64_t.into(), // element_size (index 2) - stored at creation for runtime use
                self.ctx.ptr_type(AddressSpace::default()).into(), // data (index 3)
            ],
            false,
        )
    }

    fn ty_str_array(&self) -> StructType<'ctx> {
        self.ctx.struct_type(
            &[
                self.i64_t.into(),
                self.ctx
                    .ptr_type(AddressSpace::default())
                    .into(),
            ],
            false,
        )
    }

    fn ty_cmd_result(&self) -> StructType<'ctx> {
        // Use i8 for bool to match Rust #[repr(C)] ABI
        self.ctx
            .struct_type(&[self.ctx.i8_type().into(), self.ty_string().into()], false)
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

    fn fn_type_from_ir(
        &self,
        ret: &IRType,
        params: &[IRType],
    ) -> inkwell::types::FunctionType<'ctx> {
        let params_ll: Vec<BasicMetadataTypeEnum> = params
            .iter()
            .map(|t| self.ir_type_to_llvm_param(t).into())
            .collect();
        match ret {
            IRType::Void => self.ctx.void_type().fn_type(&params_ll, false),
            other => self.ir_type_to_llvm(other).fn_type(&params_ll, false),
        }
    }

    fn ir_type_to_llvm_param(&self, t: &IRType) -> BasicTypeEnum<'ctx> {
        match t {
            IRType::Struct { .. } => self.ctx.ptr_type(AddressSpace::default()).into(),
            other => self.ir_type_to_llvm(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_type_layout() {
        let backend = LlvmBackend::new();
        let str_ty = backend.ty_string();
        let fields = str_ty.get_field_types();
        assert_eq!(fields.len(), 2);
        // First field is i64 (len)
        assert!(fields[0].into_int_type().get_bit_width() == 64);
        // Second field is pointer (data)
        assert!(fields[1].is_pointer_type());
    }

    #[test]
    fn test_array_type_layout() {
        let backend = LlvmBackend::new();
        let arr_ty = backend.ty_array(backend.i64_t.into());
        let fields = arr_ty.get_field_types();
        assert_eq!(fields.len(), 4);
        // len, cap, element_size are i64
        assert!(fields[0].into_int_type().get_bit_width() == 64);
        assert!(fields[1].into_int_type().get_bit_width() == 64);
        assert!(fields[2].into_int_type().get_bit_width() == 64);
        // data is pointer
        assert!(fields[3].is_pointer_type());
    }

    #[test]
    fn test_handle_type_layout() {
        let mut backend = LlvmBackend::new();
        let ty = backend.ty_handle();
        let fields = ty.get_field_types();
        assert_eq!(fields.len(), 2);
        assert!(fields
            .iter()
            .all(|f| f.into_int_type().get_bit_width() == 32));
    }
}
