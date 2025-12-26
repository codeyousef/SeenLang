//! C library function declarations and wrappers.
//!
//! This module provides functions to declare and call standard C library functions
//! (printf, malloc, strlen, etc.) from LLVM IR.

use anyhow::{anyhow, Result};
use inkwell::builder::Builder;
use inkwell::context::Context as LlvmContext;
use inkwell::module::Module as LlvmModule;
use inkwell::types::{BasicMetadataTypeEnum, BasicTypeEnum, IntType, PointerType};
use inkwell::values::{BasicMetadataValueEnum, FunctionValue, PointerValue};
use inkwell::AddressSpace;

/// Declare an external function if it doesn't already exist.
pub fn declare_if_missing<'ctx>(
    module: &LlvmModule<'ctx>,
    name: &str,
    fn_ty: inkwell::types::FunctionType<'ctx>,
) -> FunctionValue<'ctx> {
    if let Some(f) = module.get_function(name) {
        f
    } else {
        module.add_function(name, fn_ty, None)
    }
}

/// Declare a C function with a return type.
pub fn declare_c_fn<'ctx>(
    module: &LlvmModule<'ctx>,
    name: &str,
    ret: BasicTypeEnum<'ctx>,
    params: &[BasicMetadataTypeEnum<'ctx>],
    varargs: bool,
) -> FunctionValue<'ctx> {
    if let Some(f) = module.get_function(name) {
        return f;
    }
    let ty = match ret {
        BasicTypeEnum::IntType(it) => it.fn_type(params, varargs),
        BasicTypeEnum::FloatType(ft) => ft.fn_type(params, varargs),
        BasicTypeEnum::PointerType(pt) => pt.fn_type(params, varargs),
        BasicTypeEnum::StructType(st) => st.fn_type(params, varargs),
        BasicTypeEnum::ArrayType(at) => at.fn_type(params, varargs),
        BasicTypeEnum::VectorType(vt) => vt.fn_type(params, varargs),
        BasicTypeEnum::ScalableVectorType(svt) => svt.fn_type(params, varargs),
    };
    module.add_function(name, ty, None)
}

/// Declare a C function with void return type.
pub fn declare_c_void_fn<'ctx>(
    ctx: &'ctx LlvmContext,
    module: &LlvmModule<'ctx>,
    name: &str,
    params: &[BasicMetadataTypeEnum<'ctx>],
    varargs: bool,
) -> FunctionValue<'ctx> {
    if let Some(f) = module.get_function(name) {
        return f;
    }
    let ty = ctx.void_type().fn_type(params, varargs);
    module.add_function(name, ty, None)
}

/// Get the llvm.trap intrinsic, declaring it if needed.
pub fn get_trap<'ctx>(ctx: &'ctx LlvmContext, module: &LlvmModule<'ctx>) -> FunctionValue<'ctx> {
    let trap_ty = ctx.void_type().fn_type(&[], false);
    declare_if_missing(module, "llvm.trap", trap_ty)
}

/// Get or declare the printf function.
pub fn get_printf<'ctx>(
    module: &LlvmModule<'ctx>,
    i64_t: IntType<'ctx>,
    i8_ptr_t: PointerType<'ctx>,
) -> FunctionValue<'ctx> {
    if let Some(f) = module.get_function("printf") {
        return f;
    }
    let ty = i64_t.fn_type(&[i8_ptr_t.into()], true);
    module.add_function("printf", ty, None)
}

/// Get or declare the strlen function.
pub fn get_strlen<'ctx>(
    module: &LlvmModule<'ctx>,
    i64_t: IntType<'ctx>,
    i8_ptr_t: PointerType<'ctx>,
) -> FunctionValue<'ctx> {
    if let Some(f) = module.get_function("strlen") {
        return f;
    }
    let ty = i64_t.fn_type(&[i8_ptr_t.into()], false);
    module.add_function("strlen", ty, None)
}

/// Get or declare the strcmp function.
pub fn get_strcmp<'ctx>(
    ctx: &'ctx LlvmContext,
    module: &LlvmModule<'ctx>,
    i8_ptr_t: PointerType<'ctx>,
) -> FunctionValue<'ctx> {
    if let Some(f) = module.get_function("strcmp") {
        return f;
    }
    let i32_t = ctx.i32_type();
    let ty = i32_t.fn_type(&[i8_ptr_t.into(), i8_ptr_t.into()], false);
    module.add_function("strcmp", ty, None)
}

/// Get or declare the malloc function.
pub fn get_malloc<'ctx>(
    module: &LlvmModule<'ctx>,
    i64_t: IntType<'ctx>,
    i8_ptr_t: PointerType<'ctx>,
) -> FunctionValue<'ctx> {
    if let Some(f) = module.get_function("malloc") {
        return f;
    }
    let ty = i8_ptr_t.fn_type(&[i64_t.into()], false);
    module.add_function("malloc", ty, None)
}

/// Get or declare the realloc function.
pub fn get_realloc<'ctx>(
    module: &LlvmModule<'ctx>,
    i64_t: IntType<'ctx>,
    i8_ptr_t: PointerType<'ctx>,
) -> FunctionValue<'ctx> {
    if let Some(f) = module.get_function("realloc") {
        return f;
    }
    let ty = i8_ptr_t.fn_type(&[i8_ptr_t.into(), i64_t.into()], false);
    module.add_function("realloc", ty, None)
}

/// Get or declare the free function.
pub fn get_free<'ctx>(
    ctx: &'ctx LlvmContext,
    module: &LlvmModule<'ctx>,
    i8_ptr_t: PointerType<'ctx>,
) -> FunctionValue<'ctx> {
    if let Some(f) = module.get_function("free") {
        return f;
    }
    let ty = ctx.void_type().fn_type(&[i8_ptr_t.into()], false);
    module.add_function("free", ty, None)
}

/// Get or declare the memcpy function.
pub fn get_memcpy<'ctx>(
    module: &LlvmModule<'ctx>,
    i64_t: IntType<'ctx>,
    i8_ptr_t: PointerType<'ctx>,
) -> FunctionValue<'ctx> {
    if let Some(f) = module.get_function("memcpy") {
        return f;
    }
    let ty = i8_ptr_t.fn_type(
        &[
            i8_ptr_t.into(),
            i8_ptr_t.into(),
            i64_t.into(),
        ],
        false,
    );
    module.add_function("memcpy", ty, None)
}

/// Get or declare the fflush function.
pub fn get_fflush<'ctx>(
    ctx: &'ctx LlvmContext,
    module: &LlvmModule<'ctx>,
    i8_ptr_t: PointerType<'ctx>,
) -> FunctionValue<'ctx> {
    if let Some(func) = module.get_function("fflush") {
        return func;
    }
    let i32_t = ctx.i32_type();
    let fn_type = i32_t.fn_type(&[i8_ptr_t.into()], false);
    module.add_function("fflush", fn_type, None)
}

/// Get or declare the clock_gettime function.
pub fn get_clock_gettime<'ctx>(
    ctx: &'ctx LlvmContext,
    module: &LlvmModule<'ctx>,
    i64_t: IntType<'ctx>,
) -> FunctionValue<'ctx> {
    if let Some(f) = module.get_function("clock_gettime") {
        return f;
    }
    // int clock_gettime(clockid_t clk_id, struct timespec *tp);
    // timespec is { i64 tv_sec, i64 tv_nsec }
    let timespec_ty = ctx.struct_type(&[
        i64_t.into(),
        i64_t.into(),
    ], false);
    let ty = ctx.i32_type().fn_type(&[
        ctx.i32_type().into(),
        timespec_ty.ptr_type(AddressSpace::from(0u16)).into(),
    ], false);
    module.add_function("clock_gettime", ty, None)
}

// ============================================================================
// Call Helpers
// ============================================================================

/// Call printf with the given arguments.
pub fn call_printf<'ctx>(
    builder: &Builder<'ctx>,
    module: &LlvmModule<'ctx>,
    i64_t: IntType<'ctx>,
    i8_ptr_t: PointerType<'ctx>,
    args: &[BasicMetadataValueEnum<'ctx>],
) -> Result<()> {
    let printf = get_printf(module, i64_t, i8_ptr_t);
    builder
        .build_call(printf, args, "printf_call")
        .map(|_| ())
        .map_err(|e| anyhow!("{e:?}"))
}

/// Call fflush(NULL) to flush stdout.
pub fn call_fflush<'ctx>(
    ctx: &'ctx LlvmContext,
    builder: &Builder<'ctx>,
    module: &LlvmModule<'ctx>,
    i8_ptr_t: PointerType<'ctx>,
) -> Result<()> {
    let fflush = get_fflush(ctx, module, i8_ptr_t);
    let null = i8_ptr_t.const_zero();
    builder
        .build_call(fflush, &[null.into()], "fflush_call")
        .map(|_| ())
        .map_err(|e| anyhow!("{e:?}"))
}

/// Call strlen and return the length.
pub fn call_strlen<'ctx>(
    builder: &Builder<'ctx>,
    module: &LlvmModule<'ctx>,
    i64_t: IntType<'ctx>,
    i8_ptr_t: PointerType<'ctx>,
    s: PointerValue<'ctx>,
) -> Result<inkwell::values::IntValue<'ctx>> {
    let strlen = get_strlen(module, i64_t, i8_ptr_t);
    let call = builder
        .build_call(strlen, &[s.into()], "strlen")
        .map_err(|e| anyhow!("{e:?}"))?;
    Ok(call.try_as_basic_value().left().unwrap().into_int_value())
}

/// Call strcmp and return the comparison result.
pub fn call_strcmp<'ctx>(
    ctx: &'ctx LlvmContext,
    builder: &Builder<'ctx>,
    module: &LlvmModule<'ctx>,
    i8_ptr_t: PointerType<'ctx>,
    a: PointerValue<'ctx>,
    b: PointerValue<'ctx>,
) -> Result<inkwell::values::IntValue<'ctx>> {
    let strcmp = get_strcmp(ctx, module, i8_ptr_t);
    let call = builder
        .build_call(strcmp, &[a.into(), b.into()], "strcmp")
        .map_err(|e| anyhow!("{e:?}"))?;
    Ok(call.try_as_basic_value().left().unwrap().into_int_value())
}

// ============================================================================
// CLibraryOps Trait - Cached C library function access for LlvmBackend
// ============================================================================

use crate::llvm_backend::LlvmBackend;
use inkwell::values::BasicValue;

/// Trait for cached C library function access on the LLVM backend.
/// These methods use the backend's cached function pointers for efficiency.
pub trait CLibraryOps<'ctx> {
    /// Get or declare printf (cached).
    fn get_printf(&mut self) -> FunctionValue<'ctx>;
    
    /// Get or declare strlen (cached).
    fn get_strlen(&mut self) -> FunctionValue<'ctx>;
    
    /// Get or declare strcmp (cached).
    fn get_strcmp(&mut self) -> FunctionValue<'ctx>;
    
    /// Get or declare malloc (cached).
    fn get_malloc(&mut self) -> FunctionValue<'ctx>;
    
    /// Get or declare realloc (cached).
    fn get_realloc(&mut self) -> FunctionValue<'ctx>;
    
    /// Get or declare free (cached).
    fn get_free(&mut self) -> FunctionValue<'ctx>;
    
    /// Get or declare memcpy (cached).
    fn get_memcpy(&mut self) -> FunctionValue<'ctx>;
    
    /// Get or declare fflush.
    fn get_fflush(&self) -> FunctionValue<'ctx>;
    
    /// Get or declare clock_gettime.
    fn get_or_declare_clock_gettime(&mut self) -> FunctionValue<'ctx>;
    
    /// Call printf with the given arguments.
    fn call_printf(&mut self, args: &[BasicMetadataValueEnum<'ctx>]) -> Result<()>;
    
    /// Call fflush(NULL) to flush stdout.
    fn call_fflush(&mut self) -> Result<()>;
    
    /// Call strlen and return the length.
    fn call_strlen(&mut self, s: PointerValue<'ctx>) -> Result<inkwell::values::IntValue<'ctx>>;
    
    /// Call strcmp and return the comparison result.
    fn call_strcmp(
        &mut self,
        a: PointerValue<'ctx>,
        b: PointerValue<'ctx>,
    ) -> Result<inkwell::values::IntValue<'ctx>>;
}

impl<'ctx> CLibraryOps<'ctx> for LlvmBackend<'ctx> {
    fn get_printf(&mut self) -> FunctionValue<'ctx> {
        if let Some(f) = self.printf {
            return f;
        }
        let f = get_printf(&self.module, self.i64_t, self.i8_ptr_t);
        self.printf = Some(f);
        f
    }

    fn get_strlen(&mut self) -> FunctionValue<'ctx> {
        if let Some(f) = self.strlen {
            return f;
        }
        let f = get_strlen(&self.module, self.i64_t, self.i8_ptr_t);
        self.strlen = Some(f);
        f
    }

    fn get_strcmp(&mut self) -> FunctionValue<'ctx> {
        if let Some(f) = self.strcmp {
            return f;
        }
        let f = get_strcmp(self.ctx, &self.module, self.i8_ptr_t);
        self.strcmp = Some(f);
        f
    }

    fn get_malloc(&mut self) -> FunctionValue<'ctx> {
        if let Some(f) = self.malloc {
            return f;
        }
        let f = get_malloc(&self.module, self.i64_t, self.i8_ptr_t);
        self.malloc = Some(f);
        f
    }

    fn get_realloc(&mut self) -> FunctionValue<'ctx> {
        if let Some(f) = self.realloc {
            return f;
        }
        let f = get_realloc(&self.module, self.i64_t, self.i8_ptr_t);
        self.realloc = Some(f);
        f
    }

    fn get_free(&mut self) -> FunctionValue<'ctx> {
        if let Some(f) = self.free {
            return f;
        }
        let f = get_free(self.ctx, &self.module, self.i8_ptr_t);
        self.free = Some(f);
        f
    }

    fn get_memcpy(&mut self) -> FunctionValue<'ctx> {
        if let Some(f) = self.memcpy {
            return f;
        }
        let f = get_memcpy(&self.module, self.i64_t, self.i8_ptr_t);
        self.memcpy = Some(f);
        f
    }

    fn get_fflush(&self) -> FunctionValue<'ctx> {
        get_fflush(self.ctx, &self.module, self.i8_ptr_t)
    }

    fn get_or_declare_clock_gettime(&mut self) -> FunctionValue<'ctx> {
        get_clock_gettime(self.ctx, &self.module, self.i64_t)
    }

    fn call_printf(&mut self, args: &[BasicMetadataValueEnum<'ctx>]) -> Result<()> {
        let printf = self.get_printf();
        self.builder
            .build_call(printf, args, "printf_call")
            .map(|_| ())
            .map_err(|e| anyhow!("{e:?}"))
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use inkwell::context::Context;

    #[test]
    fn test_declare_printf() {
        let ctx = Context::create();
        let module = ctx.create_module("test");
        let i64_t = ctx.i64_type();
        let i8_ptr_t = ctx.i8_type().ptr_type(AddressSpace::from(0u16));
        
        let printf = get_printf(&module, i64_t, i8_ptr_t);
        assert_eq!(printf.get_name().to_str().unwrap(), "printf");
        
        // Second call should return same function
        let printf2 = get_printf(&module, i64_t, i8_ptr_t);
        assert_eq!(printf, printf2);
    }

    #[test]
    fn test_declare_memory_functions() {
        let ctx = Context::create();
        let module = ctx.create_module("test");
        let i64_t = ctx.i64_type();
        let i8_ptr_t = ctx.i8_type().ptr_type(AddressSpace::from(0u16));
        
        let malloc = get_malloc(&module, i64_t, i8_ptr_t);
        assert_eq!(malloc.get_name().to_str().unwrap(), "malloc");
        
        let realloc = get_realloc(&module, i64_t, i8_ptr_t);
        assert_eq!(realloc.get_name().to_str().unwrap(), "realloc");
        
        let free = get_free(&ctx, &module, i8_ptr_t);
        assert_eq!(free.get_name().to_str().unwrap(), "free");
        
        let memcpy = get_memcpy(&module, i64_t, i8_ptr_t);
        assert_eq!(memcpy.get_name().to_str().unwrap(), "memcpy");
    }
}
