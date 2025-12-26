//! Memory operations and slot management.
//!
//! This module handles stack allocation (alloca), loading from slots,
//! array bounds checking, and embedded byte array globals.

use std::convert::TryFrom;
use anyhow::{anyhow, Result};
use indexmap::IndexMap;
use inkwell::basic_block::BasicBlock as LlvmBasicBlock;
use inkwell::builder::Builder;
use inkwell::context::Context as LlvmContext;
use inkwell::module::{Linkage, Module as LlvmModule};
use inkwell::types::{BasicType, BasicTypeEnum, IntType, PointerType};
use inkwell::values::{BasicValueEnum, FunctionValue, GlobalValue, PointerValue, UnnamedAddress};

use super::c_library;

// Type alias for deterministic maps
type HashMap<K, V> = IndexMap<K, V>;

// ============================================================================
// Stack Allocation
// ============================================================================

/// Allocate stack space for a given type, placing the alloca in the entry block.
pub fn alloca_for_type<'ctx>(
    ctx: &'ctx LlvmContext,
    builder: &Builder<'ctx>,
    current_fn: Option<FunctionValue<'ctx>>,
    ty: BasicTypeEnum<'ctx>,
    name: &str,
) -> Result<PointerValue<'ctx>> {
    // Save current position
    let current_block = builder.get_insert_block();
    
    // Move to entry block
    if let Some(func) = current_fn {
        if let Some(entry) = func.get_first_basic_block() {
            if let Some(first_inst) = entry.get_first_instruction() {
                builder.position_before(&first_inst);
            } else {
                builder.position_at_end(entry);
            }
        }
    }

    let slot = match ty {
        BasicTypeEnum::ArrayType(at) => builder.build_alloca(at, name),
        BasicTypeEnum::FloatType(ft) => builder.build_alloca(ft, name),
        BasicTypeEnum::IntType(it) => builder.build_alloca(it, name),
        BasicTypeEnum::PointerType(pt) => builder.build_alloca(pt, name),
        BasicTypeEnum::StructType(st) => builder.build_alloca(st, name),
        BasicTypeEnum::VectorType(vt) => builder.build_alloca(vt, name),
        BasicTypeEnum::ScalableVectorType(svt) => builder.build_alloca(svt, name),
    }
    .map_err(|e| anyhow!("{e:?}"))?;

    // Restore position
    if let Some(block) = current_block {
        builder.position_at_end(block);
    }

    Ok(slot)
}

/// Load a value from a variable slot, using the slot type registry.
pub fn load_from_slot<'ctx>(
    builder: &Builder<'ctx>,
    name: &str,
    slot: PointerValue<'ctx>,
    var_slot_types: &HashMap<String, BasicTypeEnum<'ctx>>,
) -> Result<BasicValueEnum<'ctx>> {
    let elem_ty = *var_slot_types
        .get(name)
        .ok_or_else(|| anyhow!("Missing slot type for {}", name))?;
    let load_name = format!("load_{}", name);
    let loaded = match elem_ty {
        BasicTypeEnum::ArrayType(at) => builder.build_load(at, slot, &load_name),
        BasicTypeEnum::FloatType(ft) => builder.build_load(ft, slot, &load_name),
        BasicTypeEnum::IntType(it) => builder.build_load(it, slot, &load_name),
        BasicTypeEnum::PointerType(pt) => builder.build_load(pt, slot, &load_name),
        BasicTypeEnum::StructType(st) => builder.build_load(st, slot, &load_name),
        BasicTypeEnum::VectorType(vt) => builder.build_load(vt, slot, &load_name),
        BasicTypeEnum::ScalableVectorType(svt) => builder.build_load(svt, slot, &load_name),
    }
    .map_err(|e| anyhow!("{e:?}"))?;
    Ok(loaded)
}

/// Get the basic type from a value.
pub fn basic_type_from_value<'ctx>(value: &BasicValueEnum<'ctx>) -> Option<BasicTypeEnum<'ctx>> {
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

// ============================================================================
// Array Operations
// ============================================================================

/// Build array bounds check and branch to trap on failure.
/// Returns the continuation block where execution resumes if bounds check passes.
pub fn build_bounds_check<'ctx>(
    ctx: &'ctx LlvmContext,
    builder: &Builder<'ctx>,
    module: &LlvmModule<'ctx>,
    current_fn: FunctionValue<'ctx>,
    index: inkwell::values::IntValue<'ctx>,
    length: inkwell::values::IntValue<'ctx>,
) -> Result<LlvmBasicBlock<'ctx>> {
    let cmp = builder.build_int_compare(
        inkwell::IntPredicate::UGE,
        index,
        length,
        "bounds_check",
    )?;

    let fail_bb = ctx.append_basic_block(current_fn, "bounds_fail");
    let cont_bb = ctx.append_basic_block(current_fn, "bounds_ok");

    builder.build_conditional_branch(cmp, fail_bb, cont_bb)?;

    builder.position_at_end(fail_bb);
    let trap = c_library::get_trap(ctx, module);
    builder.build_call(trap, &[], "trap")?;
    builder.build_unreachable()?;

    builder.position_at_end(cont_bb);
    Ok(cont_bb)
}

/// Get the length field from an array pointer (field 0).
pub fn get_array_len<'ctx>(
    builder: &Builder<'ctx>,
    i64_t: IntType<'ctx>,
    arr_ptr: PointerValue<'ctx>,
) -> Result<inkwell::values::IntValue<'ctx>> {
    let len_ptr = builder.build_pointer_cast(
        arr_ptr,
        i64_t.ptr_type(inkwell::AddressSpace::from(0u16)),
        "len_ptr",
    )?;
    let len = builder
        .build_load(i64_t, len_ptr, "len")
        .map_err(|e| anyhow!("{e:?}"))?
        .into_int_value();
    Ok(len)
}

/// Get the data pointer from an array (offset by sizeof(i64) for length, then load i8*).
pub fn get_array_data_ptr<'ctx>(
    ctx: &'ctx LlvmContext,
    builder: &Builder<'ctx>,
    i64_t: IntType<'ctx>,
    i8_ptr_t: PointerType<'ctx>,
    arr_ptr: PointerValue<'ctx>,
) -> Result<PointerValue<'ctx>> {
    // Data pointer is at offset 8 (after the i64 length field)
    let data_ptr_ptr = unsafe {
        builder
            .build_gep(
                ctx.i8_type(),
                arr_ptr,
                &[i64_t.const_int(8, false)],
                "data_ptr_ptr",
            )
            .map_err(|e| anyhow!("{e:?}"))?
    };
    let data_ptr_ptr_casted = builder.build_pointer_cast(
        data_ptr_ptr,
        i8_ptr_t.ptr_type(inkwell::AddressSpace::from(0u16)),
        "data_ptr_ptr_casted",
    )?;
    let data_ptr = builder
        .build_load(i8_ptr_t, data_ptr_ptr_casted, "data_ptr")
        .map_err(|e| anyhow!("{e:?}"))?
        .into_pointer_value();
    Ok(data_ptr)
}

// ============================================================================
// Byte Array Globals
// ============================================================================

/// Get or create a global byte array constant.
pub fn byte_array_global<'ctx>(
    ctx: &'ctx LlvmContext,
    module: &LlvmModule<'ctx>,
    data: &[u8],
    byte_array_globals: &mut HashMap<Vec<u8>, GlobalValue<'ctx>>,
) -> Result<GlobalValue<'ctx>> {
    if let Some(global) = byte_array_globals.get(data) {
        return Ok(*global);
    }

    let byte_ty = ctx.i8_type();
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

    let symbol = format!("__seen_embed_{}", byte_array_globals.len());
    let global = module.add_global(array_ty, None, &symbol);
    global.set_initializer(&initializer);
    global.set_constant(true);
    global.set_linkage(Linkage::Private);
    global.set_unnamed_address(UnnamedAddress::Global);
    global.set_alignment(1);
    byte_array_globals.insert(data.to_vec(), global);
    Ok(global)
}

/// Get a pointer to embedded byte data.
pub fn byte_array_ptr<'ctx>(
    ctx: &'ctx LlvmContext,
    builder: &Builder<'ctx>,
    module: &LlvmModule<'ctx>,
    i8_ptr_t: PointerType<'ctx>,
    data: &[u8],
    byte_array_globals: &mut HashMap<Vec<u8>, GlobalValue<'ctx>>,
) -> Result<PointerValue<'ctx>> {
    let global = byte_array_global(ctx, module, data, byte_array_globals)?;
    let cast = builder
        .build_pointer_cast(global.as_pointer_value(), i8_ptr_t, "embed_ptr")
        .map_err(|e| anyhow!("{e:?}"))?;
    Ok(cast)
}

/// Convert a pointer value to i8*.
pub fn to_i8_ptr<'ctx>(
    ctx: &'ctx LlvmContext,
    builder: &Builder<'ctx>,
    current_fn: Option<FunctionValue<'ctx>>,
    i8_ptr_t: PointerType<'ctx>,
    value: BasicValueEnum<'ctx>,
    name: &str,
) -> Result<PointerValue<'ctx>> {
    match value {
        BasicValueEnum::PointerValue(ptr) => builder
            .build_pointer_cast(ptr, i8_ptr_t, name)
            .map_err(|e| anyhow!("{e:?}")),
        BasicValueEnum::IntValue(int_val) => builder
            .build_int_to_ptr(int_val, i8_ptr_t, name)
            .map_err(|e| anyhow!("{e:?}")),
        BasicValueEnum::StructValue(struct_val) => {
            let ty = struct_val.get_type().as_basic_type_enum();
            let tmp = alloca_for_type(ctx, builder, current_fn, ty, &format!("{name}_stack"))?;
            builder.build_store(tmp, struct_val)?;
            builder
                .build_pointer_cast(tmp, i8_ptr_t, &format!("{name}_stack_ptr"))
                .map_err(|e| anyhow!("{e:?}"))
        }
        other => Err(anyhow!(
            "select requires pointer compatible value, got {:?}",
            other
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use inkwell::context::Context;
    use inkwell::values::BasicValue;

    #[test]
    fn test_basic_type_from_value() {
        let ctx = Context::create();
        let i64_t = ctx.i64_type();
        
        let val = i64_t.const_int(42, false);
        let ty = basic_type_from_value(&val.as_basic_value_enum());
        assert!(ty.is_some());
        assert!(ty.unwrap().is_int_type());
    }

    #[test]
    fn test_byte_array_global() {
        let ctx = Context::create();
        let module = ctx.create_module("test");
        let mut globals = HashMap::new();
        
        let data = b"hello";
        let global = byte_array_global(&ctx, &module, data, &mut globals).unwrap();
        
        // Second call should return same global
        let global2 = byte_array_global(&ctx, &module, data, &mut globals).unwrap();
        assert_eq!(global.as_pointer_value(), global2.as_pointer_value());
        
        // Different data should create different global
        let data2 = b"world";
        let global3 = byte_array_global(&ctx, &module, data2, &mut globals).unwrap();
        assert_ne!(global.as_pointer_value(), global3.as_pointer_value());
    }
}
