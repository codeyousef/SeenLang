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
        // Track struct type info BEFORE eval/assign so we preserve the type name
        if let IRValue::Struct { type_name, .. } = source {
            if let IRValue::Variable(var_name) = dest {
                self.var_struct_types.insert(var_name.clone(), type_name.clone());
            }
        }
        
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
        
        // Debug: trace char stores
        if self.trace_options.trace_boxing && v.is_int_value() && v.into_int_value().get_type().get_bit_width() == 8 {
            eprintln!("[BOXING] emit_store storing i8 value to {:?}, val={:?}", dest, v);
        }
        
        self.assign_value(dest, v)?;

        // Propagate struct type info
        if let IRValue::Variable(var_name) = dest {
            match value {
                IRValue::Register(reg_id) => {
                    if self.trace_options.trace_boxing {
                        eprintln!("[BOXING] emit_store reg {} -> var '{}', reg_struct_types={:?}, reg_array_element_struct={:?}",
                            reg_id, var_name, self.reg_struct_types.get(reg_id), self.reg_array_element_struct.get(reg_id));
                    }
                    if let Some(struct_name) = self.reg_struct_types.get(reg_id) {
                        if self.trace_options.trace_boxing && (var_name == "location" || var_name == "entry") {
                            eprintln!("[BOXING] emit_store propagating struct type '{}' from reg {} to var '{}'",
                                struct_name, reg_id, var_name);
                        }
                        self.var_struct_types.insert(var_name.clone(), struct_name.clone());
                    }
                    // Propagate array element struct type info
                    if let Some(elem_struct) = self.reg_array_element_struct.get(reg_id) {
                        if self.trace_options.trace_boxing {
                            eprintln!("[BOXING] emit_store propagating array element type '{}' from reg {} to var '{}'", elem_struct, reg_id, var_name);
                        }
                        self.var_array_element_struct.insert(var_name.clone(), elem_struct.clone());
                    }
                    // Propagate Option inner type info
                    // This applies in two cases:
                    // 1. The source reg IS an Option with an inner type
                    // 2. The source reg is a Vec<Option<T>> and we're tracking T
                    if let Some(inner_type) = self.reg_option_inner_type.get(reg_id) {
                        let reg_is_option = self.reg_struct_types.get(reg_id)
                            .map(|t| t == "Option")
                            .unwrap_or(false);
                        let reg_is_vec_of_option = self.reg_struct_types.get(reg_id)
                            .map(|t| t == "Vec")
                            .unwrap_or(false)
                            && self.reg_array_element_struct.get(reg_id)
                            .map(|t| t == "Option")
                            .unwrap_or(false);

                        if self.trace_options.trace_boxing {
                            eprintln!("[BOXING] emit_store reg {} has Option inner type '{}', reg_is_option={}, reg_is_vec_of_option={}, var_struct_types['{}']='{:?}'",
                                reg_id, inner_type, reg_is_option, reg_is_vec_of_option, var_name, self.var_struct_types.get(var_name));
                        }
                        if reg_is_option {
                            if self.trace_options.trace_boxing {
                                eprintln!("[BOXING] emit_store propagating Option inner type '{}' to var '{}'", inner_type, var_name);
                            }
                            self.var_option_inner_type.insert(var_name.clone(), inner_type.clone());
                        } else if reg_is_vec_of_option {
                            // For Vec<Option<T>>, propagate the inner type so Vec_get can use it
                            if self.trace_options.trace_boxing {
                                eprintln!("[BOXING] emit_store propagating Vec<Option<{}>> inner type to var '{}'", inner_type, var_name);
                            }
                            self.var_option_inner_type.insert(var_name.clone(), inner_type.clone());
                        }
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
                    // Propagate Option inner type info - only if the dest var is an Option type
                    if let Some(inner_type) = self.var_option_inner_type.get(src_name) {
                        let is_option = self.var_struct_types.get(var_name)
                            .map(|t| t == "Option")
                            .unwrap_or(false)
                            || self.var_struct_types.get(src_name)
                            .map(|t| t == "Option")
                            .unwrap_or(false);
                        if is_option {
                            self.var_option_inner_type.insert(var_name.clone(), inner_type.clone());
                        }
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
