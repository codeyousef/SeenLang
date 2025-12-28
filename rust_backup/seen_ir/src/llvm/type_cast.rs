//! Type casting operations for the LLVM backend.
//!
//! This module handles type conversions between LLVM basic values,
//! including as_bool, as_i64, as_f64, as_cstr_ptr, and to_i8_ptr.

use anyhow::{anyhow, Result};
use inkwell::types::BasicType;
use inkwell::values::{BasicValue, BasicValueEnum, PointerValue};

use crate::llvm_backend::LlvmBackend;

/// Trait for type casting operations on the LLVM backend.
pub trait TypeCastOps<'ctx> {
    /// Convert a value to a boolean (i1).
    fn as_bool(&self, v: BasicValueEnum<'ctx>) -> Result<inkwell::values::IntValue<'ctx>>;
    
    /// Convert a value to an i64.
    fn as_i64(&self, v: BasicValueEnum<'ctx>) -> Result<inkwell::values::IntValue<'ctx>>;
    
    /// Convert a value to an f64.
    fn as_f64(&self, v: BasicValueEnum<'ctx>) -> Result<inkwell::values::FloatValue<'ctx>>;
    
    /// Convert a value to a usize (u64).
    fn as_usize(&self, v: BasicValueEnum<'ctx>) -> Result<u64>;
    
    /// Convert a value to a C string pointer (i8*).
    fn as_cstr_ptr(&self, v: BasicValueEnum<'ctx>) -> Result<PointerValue<'ctx>>;
    
    /// Convert any value to an i8 pointer.
    fn to_i8_ptr(&mut self, value: BasicValueEnum<'ctx>, name: &str) -> Result<PointerValue<'ctx>>;
}

impl<'ctx> TypeCastOps<'ctx> for LlvmBackend<'ctx> {
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
        if v.is_struct_value() {
            // For struct values, try to extract first field as bool
            // This handles Result<T,E>, Option<T> and similar types
            let sv = v.into_struct_value();
            if sv.get_type().count_fields() > 0 {
                if let Some(first_field_ty) = sv.get_type().get_field_type_at_index(0) {
                    if first_field_ty.is_int_type() {
                        if let Ok(first_field) = self.builder.build_extract_value(sv, 0, "struct_first_field") {
                            if first_field.is_int_value() {
                                let iv = first_field.into_int_value();
                                if iv.get_type() == self.bool_t {
                                    return Ok(iv);
                                }
                                // For other int types, compare to zero
                                let zero = iv.get_type().const_zero();
                                return self.builder
                                    .build_int_compare(inkwell::IntPredicate::NE, iv, zero, "tobool")
                                    .map_err(|e| anyhow!("{e:?}"));
                            }
                        }
                    }
                }
            }
        }
        if v.is_pointer_value() {
            // Pointer to bool - compare to null
            let pv = v.into_pointer_value();
            let null = pv.get_type().const_null();
            return self.builder
                .build_int_compare(inkwell::IntPredicate::NE, pv, null, "ptr_tobool")
                .map_err(|e| anyhow!("{e:?}"));
        }
        if v.is_float_value() {
            let fv = v.into_float_value();
            let zero = fv.get_type().const_float(0.0);
            return self.builder
                .build_float_compare(inkwell::FloatPredicate::ONE, fv, zero, "float_tobool")
                .map_err(|e| anyhow!("{e:?}"));
        }
        eprintln!("DEBUG: as_bool failed! val type: {:?}", v.get_type());
        Err(anyhow!("Cannot convert value to bool"))
    }

    fn as_i64(&self, v: BasicValueEnum<'ctx>) -> Result<inkwell::values::IntValue<'ctx>> {
        if v.is_int_value() {
            let iv = v.into_int_value();
            if iv.get_type() == self.i64_t {
                Ok(iv)
            } else {
                // Use zero-extension for smaller types (char/byte/bool) to avoid sign issues
                self.builder
                    .build_int_z_extend(iv, self.i64_t, "zext")
                    .map_err(|e| anyhow!("{e:?}"))
            }
        } else if v.is_pointer_value() {
            // Handle pointer values - for generic returns (like Vec_get), this is a pointer 
            // to the actual value, so we need to dereference
            let ptr = v.into_pointer_value();
            
            // Try loading as i64 from the pointer - this handles generic return values
            // which are autoboxed pointers to the actual value
            let loaded = self.builder.build_load(self.i64_t, ptr, "deref_i64")?;
            if loaded.is_int_value() {
                let iv = loaded.into_int_value();
                if iv.get_type() == self.i64_t {
                    Ok(iv)
                } else {
                    self.builder
                        .build_int_z_extend(iv, self.i64_t, "zext")
                        .map_err(|e| anyhow!("{e:?}"))
                }
            } else {
                // Fall back to ptr-to-int if dereference fails
                self.builder
                    .build_ptr_to_int(ptr, self.i64_t, "ptr2i")
                    .map_err(|e| anyhow!("{e:?}"))
            }
        } else if v.is_float_value() {
            // Handle float values by converting to int
            self.builder
                .build_float_to_signed_int(v.into_float_value(), self.i64_t, "f2i")
                .map_err(|e| anyhow!("{e:?}"))
        } else if v.is_struct_value() {
            // Handle struct values - try to extract integer field
            // Common patterns: {i64, ptr} for Char, {ptr, i64} for StringBuilder, etc.
            let sv = v.into_struct_value();
            // Try field 0 first
            if let Ok(field0) = self.builder.build_extract_value(sv, 0, "struct_f0") {
                if field0.is_int_value() {
                    let iv = field0.into_int_value();
                    if iv.get_type() == self.i64_t {
                        return Ok(iv);
                    } else {
                        return self.builder
                            .build_int_z_extend(iv, self.i64_t, "zext")
                            .map_err(|e| anyhow!("{e:?}"));
                    }
                }
            }
            // Try field 1 if field 0 wasn't int
            if let Ok(field1) = self.builder.build_extract_value(sv, 1, "struct_f1") {
                if field1.is_int_value() {
                    let iv = field1.into_int_value();
                    if iv.get_type() == self.i64_t {
                        return Ok(iv);
                    } else {
                        return self.builder
                            .build_int_z_extend(iv, self.i64_t, "zext")
                            .map_err(|e| anyhow!("{e:?}"));
                    }
                }
            }
            Err(anyhow!("Expected integer value, got struct {:?}", sv))
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
            // SeenString layout is ALWAYS { i64 len, ptr data }
            // Field 0 = len (i64), Field 1 = data (ptr)
            // The data pointer is at field 1 - try this first as it's the canonical layout
            if let Ok(val) = self.builder.build_extract_value(sv, 1, "str_data_ptr") {
                if val.is_pointer_value() {
                    return Ok(val.into_pointer_value());
                }
            }
            // Fallback: try field 0 in case of legacy {ptr, i64} layout (should not happen)
            if let Ok(val) = self.builder.build_extract_value(sv, 0, "str_ptr_f0") {
                if val.is_pointer_value() {
                    return Ok(val.into_pointer_value());
                }
            }
            // If we get here with a struct, the ABI is wrong - don't silently convert int to ptr
            return Err(anyhow!("SeenString struct has no pointer field - ABI mismatch? struct type: {:?}", sv.get_type()));
        }
        if v.is_int_value() {
            return self
                .builder
                .build_int_to_ptr(v.into_int_value(), self.i8_ptr_t, "i2ptr")
                .map_err(|e| anyhow!("{e:?}"));
        }
        if v.is_float_value() {
            // Float is likely a mistyped pointer - bitcast to i64 first
            let as_i64 = self.builder
                .build_bit_cast(v.into_float_value(), self.i64_t, "f2i_ptr")
                .map_err(|e| anyhow!("{e:?}"))?
                .into_int_value();
            return self
                .builder
                .build_int_to_ptr(as_i64, self.i8_ptr_t, "i2ptr")
                .map_err(|e| anyhow!("{e:?}"));
        }
        Err(anyhow!("Expected pointer to cstr, got {:?}", v))
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
}
