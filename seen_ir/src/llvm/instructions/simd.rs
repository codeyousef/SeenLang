//! SIMD instruction handlers for the LLVM backend.
//!
//! This module handles SIMD (Single Instruction Multiple Data) operations
//! like splat and reduce_add for vector types.

use indexmap::IndexMap;
use anyhow::{anyhow, Result};
use inkwell::values::{BasicValueEnum, FunctionValue};

use crate::llvm_backend::LlvmBackend;
use crate::value::IRType;

// Use IndexMap like the main backend for deterministic iteration
type HashMap<K, V> = IndexMap<K, V>;

/// Trait for SIMD operations on the LLVM backend.
pub trait SimdOps<'ctx> {
    /// Splat a scalar value across all lanes of a vector.
    fn emit_simd_splat(
        &mut self,
        scalar: &crate::value::IRValue,
        lane_type: &IRType,
        lanes: u32,
        result: &crate::value::IRValue,
        fn_map: &HashMap<String, FunctionValue<'ctx>>,
    ) -> Result<()>;

    /// Reduce a vector by adding all lanes together.
    fn emit_simd_reduce_add(
        &mut self,
        vector: &crate::value::IRValue,
        lane_type: &IRType,
        result: &crate::value::IRValue,
        fn_map: &HashMap<String, FunctionValue<'ctx>>,
    ) -> Result<()>;
}

impl<'ctx> SimdOps<'ctx> for LlvmBackend<'ctx> {
    fn emit_simd_splat(
        &mut self,
        scalar: &crate::value::IRValue,
        lane_type: &IRType,
        lanes: u32,
        result: &crate::value::IRValue,
        fn_map: &HashMap<String, FunctionValue<'ctx>>,
    ) -> Result<()> {
        use inkwell::types::BasicTypeEnum;
        use inkwell::values::BasicValue;

        let mut scalar_val = self.eval_value(scalar, fn_map)?;
        let lane_basic = self.ir_type_to_llvm(lane_type);
        scalar_val = self.cast_basic_to_type(scalar_val, lane_basic)?;
        
        let vec_type = match self.ir_type_to_llvm(&IRType::vector(lanes, lane_type.clone())) {
            BasicTypeEnum::VectorType(vt) => vt,
            other => {
                return Err(anyhow!(
                    "simd.splat requires numeric lane type, found {other:?}"
                ))
            }
        };
        
        let mut acc = vec_type.get_undef();
        let index_ty = self.ctx.i32_type();
        
        for idx in 0..lanes {
            let lane_index = index_ty.const_int(idx as u64, false);
            acc = self
                .builder
                .build_insert_element(acc, scalar_val, lane_index, &format!("splat_lane_{idx}"))
                .map_err(|e| anyhow!("{e:?}"))?;
        }
        
        self.assign_value(result, acc.as_basic_value_enum())?;
        Ok(())
    }

    fn emit_simd_reduce_add(
        &mut self,
        vector: &crate::value::IRValue,
        lane_type: &IRType,
        result: &crate::value::IRValue,
        fn_map: &HashMap<String, FunctionValue<'ctx>>,
    ) -> Result<()> {
        use inkwell::values::BasicValue;

        let vec_basic = self.eval_value(vector, fn_map)?;
        let vec_value = match vec_basic {
            BasicValueEnum::VectorValue(vec) => vec,
            _ => {
                return Err(anyhow!("simd.reduce_add expects a vector operand"));
            }
        };
        
        let lanes = vec_value.get_type().get_size();
        let index_ty = self.ctx.i32_type();
        
        let mut acc = match lane_type {
            IRType::Float => self.ctx.f64_type().const_float(0.0).as_basic_value_enum(),
            IRType::Integer => self.i64_t.const_zero().as_basic_value_enum(),
            _ => {
                return Err(anyhow!(
                    "simd.reduce_add currently supports integer or float lanes"
                ))
            }
        };
        
        for idx in 0..lanes {
            let lane_index = index_ty.const_int(idx as u64, false);
            let lane = self
                .builder
                .build_extract_element(vec_value, lane_index, &format!("lane_extract_{idx}"))
                .map_err(|e| anyhow!("{e:?}"))?;
            
            acc = match lane_type {
                IRType::Float => {
                    let a = acc.into_float_value();
                    let b = lane.into_float_value();
                    self.builder
                        .build_float_add(a, b, "reduce_fadd")
                        .map_err(|e| anyhow!("{e:?}"))?
                        .as_basic_value_enum()
                }
                IRType::Integer => {
                    let a = acc.into_int_value();
                    let b = lane.into_int_value();
                    self.builder
                        .build_int_add(a, b, "reduce_iadd")
                        .map_err(|e| anyhow!("{e:?}"))?
                        .as_basic_value_enum()
                }
                _ => unreachable!(),
            };
        }
        
        self.assign_value(result, acc)?;
        Ok(())
    }
}
