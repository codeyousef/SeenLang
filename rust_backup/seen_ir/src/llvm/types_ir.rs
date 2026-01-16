//! IR-to-LLVM type conversion and type builders.
//!
//! This module handles converting Seen IR types to LLVM types and provides
//! helper functions for building common LLVM type structures.

use indexmap::IndexMap;
use inkwell::context::Context as LlvmContext;
use inkwell::types::{BasicMetadataTypeEnum, BasicType, BasicTypeEnum, IntType, PointerType, StructType};
use inkwell::AddressSpace;

use crate::value::IRType;

// Type alias for deterministic maps
type HashMap<K, V> = IndexMap<K, V>;

/// Convert an IR type to an LLVM basic type.
///
/// # Arguments
/// * `ctx` - The LLVM context
/// * `t` - The IR type to convert
/// * `i64_t` - Cached i64 type
/// * `bool_t` - Cached bool (i1) type
/// * `i8_ptr_t` - Cached i8* type
/// * `struct_types` - Registry of known struct types
pub fn ir_type_to_llvm<'ctx>(
    ctx: &'ctx LlvmContext,
    t: &IRType,
    i64_t: IntType<'ctx>,
    bool_t: IntType<'ctx>,
    i8_ptr_t: PointerType<'ctx>,
    struct_types: &HashMap<String, (StructType<'ctx>, Vec<String>)>,
) -> BasicTypeEnum<'ctx> {
    match t {
        // Void is not a BasicType in LLVM; callers that need a function
        // type must handle void explicitly. Provide a placeholder type to
        // satisfy type requirements in contexts that should never see Void.
        IRType::Void => ctx.i8_type().into(),
        IRType::Integer => i64_t.into(),
        IRType::Float => ctx.f64_type().into(),
        IRType::Boolean => bool_t.into(),
        IRType::Char => ctx.i8_type().into(),
        IRType::String => ty_string(ctx, i64_t, i8_ptr_t).into(),
        IRType::Array(_) => {
            // Represent arrays as opaque pointer to match runtime ABI
            i8_ptr_t.into()
        }
        IRType::Function {
            parameters,
            return_type,
        } => {
            let _fn_ty = fn_type_from_ir(ctx, return_type, parameters, i64_t, bool_t, i8_ptr_t, struct_types);
            ctx.ptr_type(AddressSpace::from(0u16)).into()
        }
        IRType::Vector { lanes, lane_type } => {
            let lane = ir_type_to_llvm(ctx, lane_type, i64_t, bool_t, i8_ptr_t, struct_types);
            match lane {
                BasicTypeEnum::IntType(int_ty) => int_ty.vec_type(*lanes).into(),
                BasicTypeEnum::FloatType(float_ty) => float_ty.vec_type(*lanes).into(),
                BasicTypeEnum::PointerType(ptr_ty) => ptr_ty.vec_type(*lanes).into(),
                BasicTypeEnum::VectorType(vec_ty) => vec_ty.into(),
                BasicTypeEnum::ScalableVectorType(vec_ty) => vec_ty.into(),
                BasicTypeEnum::StructType(_) | BasicTypeEnum::ArrayType(_) => {
                    i64_t.vec_type(*lanes).into()
                }
            }
        }
        IRType::Struct { name, .. } => {
            if let Some((st, _)) = struct_types.get(name) {
                (*st).into()
            } else {
                // Use i8* as a placeholder pointer to struct if not found
                i8_ptr_t.into()
            }
        }
        IRType::Enum { .. } => i64_t.into(),
        IRType::Pointer(inner) | IRType::Reference(inner) => {
            ir_type_to_llvm(ctx, inner, i64_t, bool_t, i8_ptr_t, struct_types);
            ctx.ptr_type(AddressSpace::from(0u16)).into()
        }
        IRType::Optional(inner) => {
            // Use pointer to inner where practical
            ir_type_to_llvm(ctx, inner, i64_t, bool_t, i8_ptr_t, struct_types);
            ctx.ptr_type(AddressSpace::from(0u16)).into()
        }
        IRType::Generic(_) => i8_ptr_t.into(),
    }
}

/// Convert IR type to LLVM type for function parameters.
/// Struct types are passed as pointers (consistent with C ABI and call sites).
pub fn ir_type_to_llvm_param<'ctx>(
    ctx: &'ctx LlvmContext,
    t: &IRType,
    i64_t: IntType<'ctx>,
    bool_t: IntType<'ctx>,
    i8_ptr_t: PointerType<'ctx>,
    struct_types: &HashMap<String, (StructType<'ctx>, Vec<String>)>,
) -> BasicTypeEnum<'ctx> {
    match t {
        // Struct parameters are passed as pointers for ABI compatibility
        IRType::Struct { .. } => i8_ptr_t.into(),
        // All other types use the standard conversion
        _ => ir_type_to_llvm(ctx, t, i64_t, bool_t, i8_ptr_t, struct_types),
    }
}

/// Build an LLVM function type from IR types.
pub fn fn_type_from_ir<'ctx>(
    ctx: &'ctx LlvmContext,
    ret: &IRType,
    params: &[IRType],
    i64_t: IntType<'ctx>,
    bool_t: IntType<'ctx>,
    i8_ptr_t: PointerType<'ctx>,
    struct_types: &HashMap<String, (StructType<'ctx>, Vec<String>)>,
) -> inkwell::types::FunctionType<'ctx> {
    let params_ll: Vec<BasicMetadataTypeEnum> = params
        .iter()
        .map(|p| ir_type_to_llvm_param(ctx, p, i64_t, bool_t, i8_ptr_t, struct_types).into())
        .collect();
    match ret {
        IRType::Void => ctx.void_type().fn_type(&params_ll, false),
        _ => {
            let r: BasicTypeEnum = ir_type_to_llvm(ctx, ret, i64_t, bool_t, i8_ptr_t, struct_types);
            r.fn_type(&params_ll, false)
        }
    }
}

// ============================================================================
// Type Builders - Common LLVM type structures used by the backend
// ============================================================================

/// Build the Seen String type: `{ i64 len, i8* data }`
pub fn ty_string<'ctx>(
    ctx: &'ctx LlvmContext,
    i64_t: IntType<'ctx>,
    i8_ptr_t: PointerType<'ctx>,
) -> StructType<'ctx> {
    ctx.struct_type(&[i64_t.into(), i8_ptr_t.into()], false)
}

/// Build the Seen Array type: `{ i64 len, i64 cap, i64 element_size, T* data }`
pub fn ty_array<'ctx>(
    ctx: &'ctx LlvmContext,
    i64_t: IntType<'ctx>,
    _elem_ty: BasicTypeEnum<'ctx>,
) -> StructType<'ctx> {
    ctx.struct_type(
        &[
            i64_t.into(), // len (index 0)
            i64_t.into(), // cap (index 1)
            i64_t.into(), // element_size (index 2) - stored at creation for runtime use
            ctx.ptr_type(AddressSpace::default()).into(), // data (index 3)
        ],
        false,
    )
}

/// Build the string array type: `{ i64, i8** }`
pub fn ty_str_array<'ctx>(
    ctx: &'ctx LlvmContext,
    i64_t: IntType<'ctx>,
    _i8_ptr_t: PointerType<'ctx>,
) -> StructType<'ctx> {
    ctx.struct_type(
        &[
            i64_t.into(),
            ctx.ptr_type(AddressSpace::from(0u16)).into(),
        ],
        false,
    )
}

/// Build the command result type: `{ i8 success, String output }`
pub fn ty_cmd_result<'ctx>(
    ctx: &'ctx LlvmContext,
    i64_t: IntType<'ctx>,
    i8_ptr_t: PointerType<'ctx>,
) -> StructType<'ctx> {
    // Use i8 for bool to match Rust #[repr(C)] ABI
    ctx.struct_type(&[ctx.i8_type().into(), ty_string(ctx, i64_t, i8_ptr_t).into()], false)
}

/// Build the task handle type: `{ i32, i32 }`
pub fn ty_handle<'ctx>(ctx: &'ctx LlvmContext) -> StructType<'ctx> {
    ctx.struct_type(
        &[ctx.i32_type().into(), ctx.i32_type().into()],
        false,
    )
}

/// Build the channel select result type: `{ i8*, i64 payload, i64 index, i64 status }`
pub fn ty_select_result<'ctx>(
    ctx: &'ctx LlvmContext,
    i64_t: IntType<'ctx>,
    i8_ptr_t: PointerType<'ctx>,
) -> StructType<'ctx> {
    ctx.struct_type(
        &[i8_ptr_t.into(), i64_t.into(), i64_t.into()],
        false,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_type_layout() {
        let ctx = LlvmContext::create();
        let i64_t = ctx.i64_type();
        let i8_ptr_t = ctx.i8_type().ptr_type(AddressSpace::from(0u16));
        
        let str_ty = ty_string(&ctx, i64_t, i8_ptr_t);
        assert_eq!(str_ty.count_fields(), 2);
    }

    #[test]
    fn test_array_type_layout() {
        let ctx = LlvmContext::create();
        let i64_t = ctx.i64_type();

        let arr_ty = ty_array(&ctx, i64_t, i64_t.into());
        assert_eq!(arr_ty.count_fields(), 4);
    }

    #[test]
    fn test_handle_type_layout() {
        let ctx = LlvmContext::create();
        let handle_ty = ty_handle(&ctx);
        assert_eq!(handle_ty.count_fields(), 2);
    }
}
