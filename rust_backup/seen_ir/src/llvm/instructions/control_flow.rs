use inkwell::values::BasicValue;
use anyhow::{anyhow, Result};
use inkwell::values::FunctionValue;
use indexmap::IndexMap;

use crate::instruction::Label;
use crate::value::IRValue;
use crate::llvm_backend::LlvmBackend;
use crate::llvm::type_cast::TypeCastOps;

type HashMap<K, V> = IndexMap<K, V>;

pub trait ControlFlowOps<'ctx> {
    fn emit_label(&mut self, lbl: &Label) -> Result<()>;
    
    fn emit_jump(&mut self, target: &Label) -> Result<()>;
    
    fn emit_jump_if(
        &mut self,
        condition: &IRValue,
        target: &Label,
        fn_map: &HashMap<String, FunctionValue<'ctx>>,
    ) -> Result<()>;
    
    fn emit_jump_if_not(
        &mut self,
        condition: &IRValue,
        target: &Label,
        fn_map: &HashMap<String, FunctionValue<'ctx>>,
    ) -> Result<()>;
    
    fn emit_return(
        &mut self,
        val_opt: &Option<IRValue>,
        fn_map: &HashMap<String, FunctionValue<'ctx>>,
    ) -> Result<()>;
}

impl<'ctx> ControlFlowOps<'ctx> for LlvmBackend<'ctx> {
    fn emit_label(&mut self, lbl: &Label) -> Result<()> {
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
        Ok(())
    }

    fn emit_jump(&mut self, target: &Label) -> Result<()> {
        let dst = *self
            .blocks
            .get(&target.0)
            .ok_or_else(|| anyhow!("Unknown label {} in function {}", target.0, self.current_fn.map(|f| f.get_name().to_str().unwrap_or("?").to_string()).unwrap_or("?".to_string())))?;
        self.builder.build_unconditional_branch(dst)?;
        self.builder.clear_insertion_position();
        Ok(())
    }

    fn emit_jump_if(
        &mut self,
        condition: &IRValue,
        target: &Label,
        fn_map: &HashMap<String, FunctionValue<'ctx>>,
    ) -> Result<()> {
        let cond = self.eval_value(condition, fn_map)?;
        let i1 = self.as_bool(cond)?;
        let dst = *self
            .blocks
            .get(&target.0)
            .ok_or_else(|| anyhow!("Unknown label {} in function {}", target.0, self.current_fn.map(|f| f.get_name().to_str().unwrap_or("?").to_string()).unwrap_or("?".to_string())))?;
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
        Ok(())
    }

    fn emit_jump_if_not(
        &mut self,
        condition: &IRValue,
        target: &Label,
        fn_map: &HashMap<String, FunctionValue<'ctx>>,
    ) -> Result<()> {
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
        Ok(())
    }

    fn emit_return(
        &mut self,
        val_opt: &Option<IRValue>,
        fn_map: &HashMap<String, FunctionValue<'ctx>>,
    ) -> Result<()> {
        let current_fn = self
            .current_fn
            .ok_or_else(|| anyhow!("return outside of function"))?;
        let ret_ty_opt = current_fn.get_type().get_return_type();
        let fn_name = current_fn.get_name().to_str().unwrap_or("unknown");
        
        // Debug: Show what value we're trying to return
        if fn_name.contains("Vec") || fn_name.contains("new") {
            if let Some(v) = val_opt {
                eprintln!("DEBUG emit_return: fn={}, val={:?}", fn_name, v);
            }
        }
        
        // Runtime debug: pop frame before returning
        if self.runtime_debug_flag {
            let pop_frame_ty = self.ctx.void_type().fn_type(&[], false);
            let pop_frame_fn = if let Some(f) = self.module.get_function("__seen_pop_frame") {
                f
            } else {
                self.module.add_function("__seen_pop_frame", pop_frame_ty, None)
            };
            self.builder.build_call(pop_frame_fn, &[], "pop_frame")?;
        }
        
        match (val_opt, ret_ty_opt) {
            (Some(v), Some(ret_ty)) => {
                if fn_name == "createLexer" {
    //                             println!("DEBUG: Return in {} - evaluating {:?}", fn_name, v);
                }
                let mut bv = self.eval_value(v, fn_map)?;
                if bv.get_type() != ret_ty {
    //                             println!("DEBUG: Return cast in {} - value type: {:?}, expected ret_ty: {:?}", fn_name, bv.get_type(), ret_ty);
                    
                    // Special case: if we are returning a struct but have a pointer, load it
                    if ret_ty.is_struct_type() && bv.is_pointer_value() {
    //                                 println!("DEBUG:   Auto-loading struct from pointer for return");
                        bv = self.builder.build_load(ret_ty, bv.into_pointer_value(), "ret_load")?;
                    } else if bv.is_struct_value() && ret_ty.is_int_type() && ret_ty.into_int_type() == self.i64_t {
                        // Generics workaround: if we have a struct (like String) but the declared
                        // return type is i64 (due to unmonomorphized generics), return the struct
                        // value directly. This happens with Result<T,E>.unwrapErr() when E=String.
                        // The function signature is wrong, but the actual value is correct.
                        // We return the struct by spilling to stack and returning a pointer as i64.
    //                                 println!("DEBUG:   Generics workaround: struct value with i64 return type in {}", fn_name);
                        let struct_ty = bv.get_type();
                        let tmp = self.alloca_for_type(struct_ty, "generic_ret_spill")?;
                        self.builder.build_store(tmp, bv)?;
                        let ptr_as_int = self.builder.build_ptr_to_int(tmp, self.i64_t, "ptr_to_i64")?;
                        bv = ptr_as_int.as_basic_value_enum();
                    } else {
                        bv = self.cast_basic_to_type(bv, ret_ty)?;
                    }
                }
                self.builder.build_return(Some(&bv))?;
            }
            (Some(_), None) => {
                self.builder.build_return(None)?;
            }
            (None, None) => {
                self.builder.build_return(None)?;
            }
            (None, Some(ret_ty)) => {
                if ret_ty.is_int_type() {
                    self.builder.build_return(Some(&ret_ty.into_int_type().const_zero()))?;
                } else if ret_ty.is_pointer_type() {
                    // Return null pointer for pointer return types
                    let null_ptr = ret_ty.into_pointer_type().const_null();
                    self.builder.build_return(Some(&null_ptr.as_basic_value_enum()))?;
                } else if ret_ty.is_struct_type() {
                    // Return zeroed struct for struct return types
                    let zero_struct = ret_ty.into_struct_type().const_zero();
                    self.builder.build_return(Some(&zero_struct.as_basic_value_enum()))?;
                } else {
                    self.builder.build_return(None)?;
                }
            }
        }
        self.builder.clear_insertion_position();
        Ok(())
    }
}
