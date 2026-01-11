//! String operations for LLVM code generation.
//!
//! This module provides high-level string operations such as concatenation,
//! substring extraction, and string comparison utilities.

use anyhow::Result;
use inkwell::values::{BasicValueEnum, IntValue, PointerValue};

use crate::llvm_backend::LlvmBackend;
use crate::llvm::c_library::CLibraryOps;

/// Trait for runtime string operations.
///
/// These operations handle the Seen language's String type which is represented
/// as a struct `{ i64 len, i8* data }` in LLVM IR.
pub trait RuntimeStringOps<'ctx> {
    /// Concatenate two strings and return a new String struct.
    fn runtime_concat(
        &mut self,
        left: BasicValueEnum<'ctx>,
        right: BasicValueEnum<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>>;

    /// Check if a string ends with a given suffix.
    fn runtime_endswith(
        &mut self,
        s: PointerValue<'ctx>,
        suffix: PointerValue<'ctx>,
    ) -> Result<IntValue<'ctx>>;

    /// Extract a substring from a string.
    fn runtime_substring(
        &mut self,
        s: PointerValue<'ctx>,
        start: IntValue<'ctx>,
        end: IntValue<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>>;

    /// Extract pointer and length from a String value.
    fn get_string_ptr_len(
        &mut self,
        val: BasicValueEnum<'ctx>,
    ) -> Result<(PointerValue<'ctx>, IntValue<'ctx>)>;

    /// Ensure a value is a valid string pointer.
    fn ensure_string_ptr(&mut self, val: BasicValueEnum<'ctx>) -> Result<PointerValue<'ctx>>;
}

impl<'ctx> RuntimeStringOps<'ctx> for LlvmBackend<'ctx> {
    fn runtime_concat(
        &mut self,
        left: BasicValueEnum<'ctx>,
        right: BasicValueEnum<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>> {
        let (l_ptr, l_len) = self.get_string_ptr_len(left)?;
        let (r_ptr, r_len) = self.get_string_ptr_len(right)?;

        let one = self.i64_t.const_int(1, false);
        let total_len = self.builder.build_int_add(l_len, r_len, "sum")?;
        let alloc_size = self.builder.build_int_add(total_len, one, "plus1")?;

        let malloc = self.get_malloc();
        let buf = self
            .builder
            .build_call(malloc, &[alloc_size.into()], "malloc")
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        let dest = buf
            .try_as_basic_value()
            .left()
            .unwrap()
            .into_pointer_value();

        // memcpy(dest, left, l_len)
        let memcpy = self.get_memcpy();
        self.builder
            .build_call(memcpy, &[dest.into(), l_ptr.into(), l_len.into()], "cpy1")
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        // memcpy(dest + l_len, right, r_len)
        let dest_off = unsafe {
            self.builder
                .build_gep(self.ctx.i8_type(), dest, &[l_len], "off")?
        };
        self.builder
            .build_call(
                memcpy,
                &[dest_off.into(), r_ptr.into(), r_len.into()],
                "cpy2",
            )
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        // null terminate at dest[l_len + r_len]
        let end_ptr = unsafe {
            self.builder
                .build_gep(self.ctx.i8_type(), dest, &[total_len], "end")?
        };
        self.builder
            .build_store(end_ptr, self.ctx.i8_type().const_int(0, false))?;

        // Allocate struct on heap to return pointer (consistent with IRValue::String)
        let str_ty = self.ty_string();
        let malloc = self.get_malloc();
        let struct_size = str_ty.size_of().unwrap();
        let struct_ptr = self.builder.build_call(malloc, &[struct_size.into()], "str_struct_alloc")?
            .try_as_basic_value().left().unwrap().into_pointer_value();
            
        // Store len
        let len_ptr = self.builder.build_struct_gep(str_ty, struct_ptr, 0, "len_ptr")?;
        self.builder.build_store(len_ptr, total_len)?;
        
        // Store data ptr
        let data_ptr = self.builder.build_struct_gep(str_ty, struct_ptr, 1, "data_ptr")?;
        self.builder.build_store(data_ptr, dest)?;

        Ok(struct_ptr.into())
    }

    fn runtime_endswith(
        &mut self,
        s: PointerValue<'ctx>,
        suffix: PointerValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let s_len = self.call_strlen(s)?;
        let suf_len = self.call_strlen(suffix)?;
        let suf_gt = self.builder.build_int_compare(
            inkwell::IntPredicate::UGT,
            suf_len,
            s_len,
            "suf_gt",
        )?;
        let len_ok = self
            .builder
            .build_not(suf_gt, "len_ok")
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        let start = self
            .builder
            .build_int_sub(s_len, suf_len, "start")
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        let off = unsafe {
            self.builder
                .build_gep(self.ctx.i8_type(), s, &[start], "s_off")
                .map_err(|e| anyhow::anyhow!("{e:?}"))?
        };
        let cmp = self.call_strcmp(off, suffix)?;
        let zero32 = self.ctx.i32_type().const_zero();
        let cmp_eq = self
            .builder
            .build_int_compare(inkwell::IntPredicate::EQ, cmp, zero32, "ends_eq")
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        let suf_zero = self
            .builder
            .build_int_compare(
                inkwell::IntPredicate::EQ,
                suf_len,
                self.i64_t.const_zero(),
                "suf_zero",
            )
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        let eq_or_zero = self
            .builder
            .build_or(cmp_eq, suf_zero, "ends_match")
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        let result = self
            .builder
            .build_and(len_ok, eq_or_zero, "ends_res")
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        Ok(result)
    }

    fn runtime_substring(
        &mut self,
        s: PointerValue<'ctx>,
        start: IntValue<'ctx>,
        end: IntValue<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>> {
        let len = self.builder.build_int_sub(end, start, "sub_len")?;
        let one = self.i64_t.const_int(1, false);
        let total = self.builder.build_int_add(len, one, "plus1")?;
        let malloc = self.get_malloc();
        let buf = self
            .builder
            .build_call(malloc, &[total.into()], "malloc")
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
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

        // Allocate struct on heap to return pointer
        let str_ty = self.ty_string();
        let struct_size = str_ty.size_of().unwrap();
        let struct_ptr = self.builder.build_call(malloc, &[struct_size.into()], "str_struct_alloc")?
            .try_as_basic_value().left().unwrap().into_pointer_value();
            
        // Store len
        let len_ptr = self.builder.build_struct_gep(str_ty, struct_ptr, 0, "len_ptr")?;
        self.builder.build_store(len_ptr, len)?;
        
        // Store data ptr
        let data_ptr = self.builder.build_struct_gep(str_ty, struct_ptr, 1, "data_ptr")?;
        self.builder.build_store(data_ptr, dest)?;
        
        Ok(struct_ptr.into())
    }

    fn get_string_ptr_len(
        &mut self,
        val: BasicValueEnum<'ctx>,
    ) -> Result<(PointerValue<'ctx>, IntValue<'ctx>)> {
        if val.is_struct_value() {
            let sv = val.into_struct_value();
            // println!("DEBUG: get_string_ptr_len val type: {:?}", sv.get_type());
            
            let field_count = sv.get_type().count_fields();
//                 .build_global_string_ptr("DEBUG: get_string_ptr_len fields=%d\n", "debug_fmt_gspl_fields")?;
//             let field_count_i32 = self.ctx.i32_type().const_int(field_count as u64, false);
// //             self.call_printf(&[fmt_fields.as_pointer_value().into(), field_count_i32.into()])?;
            
            // Handle { ptr, ptr, ptr, i64 } layout (likely Vec-based String or similar)
            if field_count == 4 {
                 let f0 = sv.get_type().get_field_type_at_index(0).unwrap();
                 let f3 = sv.get_type().get_field_type_at_index(3).unwrap();
                 if f0.is_pointer_type() && f3.is_int_type() {
                     let ptr = self.builder.build_extract_value(sv, 0, "vec_ptr").unwrap().into_pointer_value();
                     let len = self.builder.build_extract_value(sv, 3, "vec_len").unwrap().into_int_value();
//                         .build_global_string_ptr("DEBUG: get_string_ptr_len vec path len=%lld ptr=%p\n", "debug_fmt_gspl_vec")?;
//                      self.call_printf(&[fmt.as_pointer_value().into(), len.into(), ptr.into()])?;
                     return Ok((ptr, len));
                 }
            }
            
            // Handle single-field struct { ptr } - likely a class-as-pointer that got wrapped
            if field_count == 1 {
                let f0 = sv.get_type().get_field_type_at_index(0).unwrap();
                if f0.is_pointer_type() {
                    // Single pointer field - treat as C string pointer
                    let ptr = self.builder.build_extract_value(sv, 0, "cstr_ptr").unwrap().into_pointer_value();
                    let len = self.call_strlen(ptr)?;
//                         .build_global_string_ptr("DEBUG: get_string_ptr_len single-ptr len=%lld ptr=%p\n", "debug_fmt_gspl_ptr")?;
//                     self.call_printf(&[fmt.as_pointer_value().into(), len.into(), ptr.into()])?;
                    return Ok((ptr, len));
                } else if f0.is_int_type() {
                    // Single int field - might be class pointer as i64
                    let ptr_int = self.builder.build_extract_value(sv, 0, "ptr_int").unwrap().into_int_value();
                    let ptr = self.builder.build_int_to_ptr(ptr_int, self.i8_ptr_t, "i64_to_ptr")?;
                    let len = self.call_strlen(ptr)?;
//                         .build_global_string_ptr("DEBUG: get_string_ptr_len single-int len=%lld ptr=%p\n", "debug_fmt_gspl_int")?;
//                     self.call_printf(&[fmt.as_pointer_value().into(), len.into(), ptr.into()])?;
                    return Ok((ptr, len));
                }
            }

            let len = self
                .builder
                .build_extract_value(sv, 0, "str_len")
                .unwrap();
            
            if len.is_struct_value() {
                 // Try to extract from the inner struct if it's a wrapper
                 let inner = len.into_struct_value();
                 let inner_len = self.builder.build_extract_value(inner, 0, "inner_len").unwrap();
                 if inner_len.is_int_value() {
                     let ptr = self.builder.build_extract_value(inner, 1, "inner_ptr").unwrap().into_pointer_value();
                     return Ok((ptr, inner_len.into_int_value()));
                 }
            }

            let len = len.into_int_value();
            let ptr = self
                .builder
                .build_extract_value(sv, 1, "str_ptr")
                .unwrap()
                .into_pointer_value();
//                 .build_global_string_ptr("DEBUG: get_string_ptr_len string len=%lld ptr=%p\n", "debug_fmt_gspl_string")?;
//             self.call_printf(&[fmt.as_pointer_value().into(), len.into(), ptr.into()])?;
            Ok((ptr, len))
        } else if val.is_pointer_value() {
            let ptr = val.into_pointer_value();
            
            // Assume pointer to String struct {len, ptr}
            // This handles Strings that are boxed/heap-allocated (e.g. from literals or generic slots)
            let str_ty = self.ty_string();
            // We can't easily distinguish char* from String*, but SeenLang Strings are usually passed as pointers to structs
            // Try to load as struct. If it was char*, this load might be invalid or produce garbage, 
            // but since we changed IRValue::String to return pointer-to-struct, this is the correct path for Strings.
            let loaded = self.builder.build_load(str_ty, ptr, "load_str_struct")?;
            if loaded.is_struct_value() {
                return self.get_string_ptr_len(loaded);
            }

            let len = self.call_strlen(ptr)?;
//                 .build_global_string_ptr("DEBUG: get_string_ptr_len raw-ptr len=%lld ptr=%p\n", "debug_fmt_gspl_rawptr")?;
//             self.call_printf(&[fmt.as_pointer_value().into(), len.into(), ptr.into()])?;
            Ok((ptr, len))
        } else {
            anyhow::bail!("get_string_ptr_len: expected struct or pointer, got {:?}", val)
        }
    }

    fn ensure_string_ptr(&mut self, val: BasicValueEnum<'ctx>) -> Result<PointerValue<'ctx>> {
        if val.is_struct_value() {
            let sv = val.into_struct_value();
            let ptr = self
                .builder
                .build_extract_value(sv, 1, "str_ptr")
                .unwrap()
                .into_pointer_value();
            Ok(ptr)
        } else if val.is_pointer_value() {
            Ok(val.into_pointer_value())
        } else {
            anyhow::bail!("ensure_string_ptr: expected struct or pointer, got {:?}", val)
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_module_compiles() {
        assert!(true);
    }
}

