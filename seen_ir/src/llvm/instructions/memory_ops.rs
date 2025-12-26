use anyhow::Result;
use inkwell::values::FunctionValue;
use indexmap::IndexMap;

use crate::value::IRValue;
use crate::llvm_backend::LlvmBackend;

type HashMap<K, V> = IndexMap<K, V>;

pub trait MemoryOps<'ctx> {
    fn emit_move(
        &mut self,
        source: &IRValue,
        dest: &IRValue,
        fn_map: &HashMap<String, FunctionValue<'ctx>>,
    ) -> Result<()>;

    fn emit_store(
        &mut self,
        value: &IRValue,
        dest: &IRValue,
        fn_map: &HashMap<String, FunctionValue<'ctx>>,
    ) -> Result<()>;

    fn emit_load(
        &mut self,
        source: &IRValue,
        dest: &IRValue,
        fn_map: &HashMap<String, FunctionValue<'ctx>>,
    ) -> Result<()>;
}

impl<'ctx> MemoryOps<'ctx> for LlvmBackend<'ctx> {
    fn emit_move(
        &mut self,
        source: &IRValue,
        dest: &IRValue,
        fn_map: &HashMap<String, FunctionValue<'ctx>>,
    ) -> Result<()> {
        let v = self.eval_value(source, fn_map)?;
        self.assign_value(dest, v)?;
        Ok(())
    }

    fn emit_store(
        &mut self,
        value: &IRValue,
        dest: &IRValue,
        fn_map: &HashMap<String, FunctionValue<'ctx>>,
    ) -> Result<()> {
        // Propagate float type info BEFORE assign_value so it can use bitcast
        if let IRValue::Variable(var_name) = dest {
            match value {
                IRValue::Register(reg_id) => {
                    if self.reg_is_float.contains(reg_id) {
                        self.var_is_float.insert(var_name.clone());
                    }
                }
                IRValue::Variable(src_name) => {
                    if self.var_is_float.contains(src_name) {
                        self.var_is_float.insert(var_name.clone());
                    }
                }
                _ => {}
            }
        }
        
        let v = self.eval_value(value, fn_map)?;
        self.assign_value(dest, v)?;

        // Propagate struct type info
        if let IRValue::Variable(var_name) = dest {
            match value {
                IRValue::Register(reg_id) => {
                    if let Some(struct_name) = self.reg_struct_types.get(reg_id) {
                        if var_name == "location" {
                            eprintln!("DEBUG: emit_store overwriting var_struct_types['location'] from {:?} to {:?} (from reg {})", 
                                self.var_struct_types.get("location"), struct_name, reg_id);
                        }
                        self.var_struct_types.insert(var_name.clone(), struct_name.clone());
                    }
                    // Propagate array element struct type info
                    if let Some(elem_struct) = self.reg_array_element_struct.get(reg_id) {
                        self.var_array_element_struct.insert(var_name.clone(), elem_struct.clone());
                    }
                }
                IRValue::Variable(src_name) => {
                    if let Some(struct_name) = self.var_struct_types.get(src_name) {
                        self.var_struct_types.insert(var_name.clone(), struct_name.clone());
                    }
                    // Propagate array element struct type info
                    if let Some(elem_struct) = self.var_array_element_struct.get(src_name) {
                        self.var_array_element_struct.insert(var_name.clone(), elem_struct.clone());
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn emit_load(
        &mut self,
        source: &IRValue,
        dest: &IRValue,
        fn_map: &HashMap<String, FunctionValue<'ctx>>,
    ) -> Result<()> {
        let v = self.eval_value(source, fn_map)?;
        self.assign_value(dest, v)?;
        Ok(())
    }
}
