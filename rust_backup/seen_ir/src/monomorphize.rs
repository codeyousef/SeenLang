//! Monomorphization pass for the Seen IR
//!
//! This module specializes generic functions and types to their concrete instantiations.
//! After monomorphization, the IR contains no unresolved generic type parameters.
//!
//! ## How it works:
//! 1. Scan all Call instructions for calls to generic functions
//! 2. For each unique set of concrete type arguments, create a specialized copy
//! 3. Replace generic calls with calls to the specialized versions
//! 4. Remove the original generic function definitions (optional)
//!
//! ## Example:
//! ```text
//! fn identity<T>(x: T) -> T { x }
//! identity(42)      // becomes identity__i64(42)
//! identity("hello") // becomes identity__str("hello")
//! ```

use crate::{
    function::IRFunction,
    instruction::Instruction,
    module::IRModule,
    value::{IRType, IRValue},
    IRProgram, IRResult,
};
use indexmap::{IndexMap, IndexSet};

/// Monomorphization context tracking type substitutions and specialized functions
pub struct MonomorphizationPass {
    /// Maps (generic_fn_name, Vec<concrete_types>) -> specialized_fn_name
    specializations: IndexMap<(String, Vec<IRType>), String>,
    /// Generic functions that have been fully specialized (can be removed)
    fully_specialized: IndexSet<String>,
    /// Counter for generating unique specialized function names
    _specialization_counter: u32,
}

impl MonomorphizationPass {
    pub fn new() -> Self {
        Self {
            specializations: IndexMap::new(),
            fully_specialized: IndexSet::new(),
            _specialization_counter: 0,
        }
    }

    /// Run the monomorphization pass on an entire IR program
    pub fn run(&mut self, program: &mut IRProgram) -> IRResult<()> {
        for module in program.modules.iter_mut() {
            self.monomorphize_module(module)?;
        }
        Ok(())
    }

    /// Monomorphize a single module
    pub fn monomorphize_module(&mut self, module: &mut IRModule) -> IRResult<()> {
        // Phase 1: Collect all instantiation sites
        let _instantiations = self.collect_instantiations(module);
        
        // Phase 2: Generate specialized functions
        let mut new_functions = Vec::new();
        for ((fn_name, type_args), specialized_name) in &self.specializations {
            if let Some(generic_fn) = module.get_function(fn_name) {
                if let Some(specialized) = self.specialize_function(
                    generic_fn,
                    type_args,
                    specialized_name,
                ) {
                    new_functions.push(specialized);
                    self.fully_specialized.insert(fn_name.clone());
                }
            }
        }
        
        // Phase 3: Add specialized functions to module
        for func in new_functions {
            module.add_function(func);
        }
        
        // Phase 4: Rewrite call sites
        self.rewrite_calls(module);
        
        // Phase 5: Remove fully specialized generic functions (optional - keep for debug)
        // For now, keep them for debugging purposes
        
        Ok(())
    }

    /// Collect all instantiation sites of generic functions
    fn collect_instantiations(&mut self, module: &IRModule) -> Vec<(String, Vec<IRType>)> {
        let mut instantiations = Vec::new();
        
        for func in module.functions_iter() {
            for block_name in &func.cfg.block_order {
                if let Some(block) = func.cfg.get_block(block_name) {
                    for inst in &block.instructions {
                        if let Some((fn_name, type_args)) = self.extract_generic_call(inst) {
                            if !type_args.is_empty() && self.has_concrete_types(&type_args) {
                                let key = (fn_name.clone(), type_args.clone());
                                if !self.specializations.contains_key(&key) {
                                    let specialized_name = self.generate_specialized_name(&fn_name, &type_args);
                                    self.specializations.insert(key.clone(), specialized_name);
                                }
                                instantiations.push((fn_name, type_args));
                            }
                        }
                    }
                    
                    // Check terminator
                    if let Some(ref term) = block.terminator {
                        if let Some((fn_name, type_args)) = self.extract_generic_call(term) {
                            if !type_args.is_empty() && self.has_concrete_types(&type_args) {
                                let key = (fn_name.clone(), type_args.clone());
                                if !self.specializations.contains_key(&key) {
                                    let specialized_name = self.generate_specialized_name(&fn_name, &type_args);
                                    self.specializations.insert(key.clone(), specialized_name);
                                }
                                instantiations.push((fn_name, type_args));
                            }
                        }
                    }
                }
            }
        }
        
        instantiations
    }

    /// Extract function name and type arguments from a Call instruction
    fn extract_generic_call(&self, inst: &Instruction) -> Option<(String, Vec<IRType>)> {
        match inst {
            Instruction::Call { target, arg_types, .. } => {
                let fn_name = match target {
                    IRValue::Function { name, .. } => name.clone(),
                    IRValue::Variable(name) => name.clone(),
                    _ => return None,
                };
                
                // Extract concrete type arguments from arg_types if available
                if let Some(types) = arg_types {
                    if !types.is_empty() {
                        return Some((fn_name, types.clone()));
                    }
                }
                
                None
            }
            _ => None,
        }
    }

    /// Check if all types in the list are concrete (no unresolved generics)
    fn has_concrete_types(&self, types: &[IRType]) -> bool {
        types.iter().all(|t| !self.is_generic_type(t))
    }

    /// Check if a type is an unresolved generic
    fn is_generic_type(&self, ty: &IRType) -> bool {
        match ty {
            IRType::Generic(name) => {
                // Single uppercase letter = unresolved generic
                name.len() == 1 && name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
            }
            IRType::Array(elem) => self.is_generic_type(elem),
            IRType::Optional(inner) => self.is_generic_type(inner),
            IRType::Pointer(inner) => self.is_generic_type(inner),
            IRType::Reference(inner) => self.is_generic_type(inner),
            IRType::Function { parameters, return_type } => {
                parameters.iter().any(|p| self.is_generic_type(p)) 
                    || self.is_generic_type(return_type)
            }
            IRType::Struct { fields, .. } => {
                fields.iter().any(|(_, ty)| self.is_generic_type(ty))
            }
            IRType::Enum { variants, .. } => {
                variants.iter().any(|(_, opt_fields)| {
                    opt_fields.as_ref().map(|fields| {
                        fields.iter().any(|ty| self.is_generic_type(ty))
                    }).unwrap_or(false)
                })
            }
            IRType::Vector { lane_type, .. } => self.is_generic_type(lane_type),
            _ => false,
        }
    }

    /// Generate a mangled name for a specialized function
    fn generate_specialized_name(&mut self, base_name: &str, type_args: &[IRType]) -> String {
        let type_suffix: Vec<String> = type_args.iter().map(|t| self.mangle_type(t)).collect();
        let suffix = type_suffix.join("_");
        format!("{}__{}", base_name, suffix)
    }

    /// Mangle a type into a string suitable for function names
    fn mangle_type(&self, ty: &IRType) -> String {
        match ty {
            IRType::Void => "void".to_string(),
            IRType::Integer => "i64".to_string(),
            IRType::Float => "f64".to_string(),
            IRType::Boolean => "bool".to_string(),
            IRType::Char => "char".to_string(),
            IRType::String => "str".to_string(),
            IRType::Array(elem) => format!("arr_{}", self.mangle_type(elem)),
            IRType::Optional(inner) => format!("opt_{}", self.mangle_type(inner)),
            IRType::Pointer(inner) => format!("ptr_{}", self.mangle_type(inner)),
            IRType::Reference(inner) => format!("ref_{}", self.mangle_type(inner)),
            IRType::Struct { name, .. } => name.clone(),
            IRType::Enum { name, .. } => name.clone(),
            IRType::Generic(name) => name.clone(),
            IRType::Function { .. } => "fn".to_string(),
            IRType::Vector { lanes, lane_type } => {
                format!("vec{}x{}", lanes, self.mangle_type(lane_type))
            }
        }
    }

    /// Create a specialized copy of a generic function
    fn specialize_function(
        &self,
        generic_fn: &IRFunction,
        type_args: &[IRType],
        specialized_name: &str,
    ) -> Option<IRFunction> {
        // Build substitution map from generic params to concrete types
        let substitutions = self.build_substitution_map(generic_fn, type_args)?;
        
        // Clone the function and apply substitutions
        let mut specialized = generic_fn.clone();
        specialized.name = specialized_name.to_string();
        
        // Substitute parameter types
        for param in &mut specialized.parameters {
            param.param_type = self.substitute_type(&param.param_type, &substitutions);
        }
        
        // Substitute return type
        specialized.return_type = self.substitute_type(&specialized.return_type, &substitutions);
        
        // Substitute types in all instructions
        for block_name in &specialized.cfg.block_order.clone() {
            if let Some(block) = specialized.cfg.get_block_mut(block_name) {
                for inst in &mut block.instructions {
                    self.substitute_in_instruction(inst, &substitutions);
                }
                if let Some(ref mut term) = block.terminator {
                    self.substitute_in_instruction(term, &substitutions);
                }
            }
        }
        
        Some(specialized)
    }

    /// Build a map from generic type parameter names to concrete types
    fn build_substitution_map(
        &self,
        func: &IRFunction,
        type_args: &[IRType],
    ) -> Option<IndexMap<String, IRType>> {
        let mut map = IndexMap::new();
        
        // Infer from parameter types - match generic types with concrete types
        for (param, concrete) in func.parameters.iter().zip(type_args.iter()) {
            if let IRType::Generic(name) = &param.param_type {
                map.insert(name.clone(), concrete.clone());
            }
        }
        
        Some(map)
    }

    /// Substitute generic types with concrete types
    fn substitute_type(&self, ty: &IRType, subs: &IndexMap<String, IRType>) -> IRType {
        match ty {
            IRType::Generic(name) => {
                subs.get(name).cloned().unwrap_or_else(|| ty.clone())
            }
            IRType::Array(elem) => {
                IRType::Array(Box::new(self.substitute_type(elem, subs)))
            }
            IRType::Optional(inner) => {
                IRType::Optional(Box::new(self.substitute_type(inner, subs)))
            }
            IRType::Pointer(inner) => {
                IRType::Pointer(Box::new(self.substitute_type(inner, subs)))
            }
            IRType::Reference(inner) => {
                IRType::Reference(Box::new(self.substitute_type(inner, subs)))
            }
            IRType::Function { parameters, return_type } => {
                IRType::Function {
                    parameters: parameters.iter().map(|p| self.substitute_type(p, subs)).collect(),
                    return_type: Box::new(self.substitute_type(return_type, subs)),
                }
            }
            IRType::Struct { name, fields } => {
                IRType::Struct {
                    name: name.clone(),
                    fields: fields.iter()
                        .map(|(n, t)| (n.clone(), self.substitute_type(t, subs)))
                        .collect(),
                }
            }
            IRType::Enum { name, variants } => {
                IRType::Enum {
                    name: name.clone(),
                    variants: variants.iter()
                        .map(|(vn, opt_fields)| {
                            (vn.clone(), opt_fields.as_ref().map(|fields| {
                                fields.iter()
                                    .map(|ty| self.substitute_type(ty, subs))
                                    .collect()
                            }))
                        })
                        .collect(),
                }
            }
            IRType::Vector { lanes, lane_type } => {
                IRType::Vector {
                    lanes: *lanes,
                    lane_type: Box::new(self.substitute_type(lane_type, subs)),
                }
            }
            _ => ty.clone(),
        }
    }

    /// Substitute types within an instruction
    fn substitute_in_instruction(&self, inst: &mut Instruction, subs: &IndexMap<String, IRType>) {
        match inst {
            Instruction::Call { arg_types, return_type, .. } => {
                if let Some(ref mut types) = arg_types {
                    for ty in types.iter_mut() {
                        *ty = self.substitute_type(ty, subs);
                    }
                }
                if let Some(ref mut ret_ty) = return_type {
                    *ret_ty = self.substitute_type(ret_ty, subs);
                }
            }
            Instruction::ArrayAccess { element_type, .. } => {
                if let Some(ref mut ty) = element_type {
                    *ty = self.substitute_type(ty, subs);
                }
            }
            Instruction::ArraySet { element_type, .. } => {
                if let Some(ref mut ty) = element_type {
                    *ty = self.substitute_type(ty, subs);
                }
            }
            Instruction::FieldAccess { field_type, .. } => {
                if let Some(ref mut ty) = field_type {
                    *ty = self.substitute_type(ty, subs);
                }
            }
            Instruction::FieldSet { field_type, .. } => {
                if let Some(ref mut ty) = field_type {
                    *ty = self.substitute_type(ty, subs);
                }
            }
            Instruction::Cast { target_type, .. } => {
                *target_type = self.substitute_type(target_type, subs);
            }
            Instruction::TypeCheck { target_type, .. } => {
                *target_type = self.substitute_type(target_type, subs);
            }
            Instruction::SimdSplat { lane_type, .. } => {
                *lane_type = self.substitute_type(lane_type, subs);
            }
            Instruction::SimdReduceAdd { lane_type, .. } => {
                *lane_type = self.substitute_type(lane_type, subs);
            }
            _ => {}
        }
    }

    /// Rewrite call sites to use specialized function names
    fn rewrite_calls(&self, module: &mut IRModule) {
        for func in module.functions_iter_mut() {
            for block_name in &func.cfg.block_order.clone() {
                if let Some(block) = func.cfg.get_block_mut(block_name) {
                    for inst in &mut block.instructions {
                        self.rewrite_call_instruction(inst);
                    }
                    if let Some(ref mut term) = block.terminator {
                        self.rewrite_call_instruction(term);
                    }
                }
            }
        }
    }

    /// Rewrite a single call instruction if it's a generic call
    fn rewrite_call_instruction(&self, inst: &mut Instruction) {
        if let Instruction::Call { target, arg_types, .. } = inst {
            let fn_name = match target {
                IRValue::Function { name, .. } => name.clone(),
                IRValue::Variable(name) => name.clone(),
                _ => return,
            };
            
            if let Some(types) = arg_types {
                let key = (fn_name.clone(), types.clone());
                if let Some(specialized_name) = self.specializations.get(&key) {
                    // Rewrite the target to use the specialized function
                    match target {
                        IRValue::Function { name, .. } => {
                            *name = specialized_name.clone();
                        }
                        IRValue::Variable(name) => {
                            *name = specialized_name.clone();
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

impl Default for MonomorphizationPass {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_mangling() {
        let pass = MonomorphizationPass::new();
        assert_eq!(pass.mangle_type(&IRType::Integer), "i64");
        assert_eq!(pass.mangle_type(&IRType::String), "str");
        assert_eq!(
            pass.mangle_type(&IRType::Array(Box::new(IRType::Integer))),
            "arr_i64"
        );
    }

    #[test]
    fn test_is_generic_type() {
        let pass = MonomorphizationPass::new();
        assert!(pass.is_generic_type(&IRType::Generic("T".to_string())));
        assert!(pass.is_generic_type(&IRType::Generic("U".to_string())));
        assert!(!pass.is_generic_type(&IRType::Generic("Int".to_string())));
        assert!(!pass.is_generic_type(&IRType::Integer));
    }

    #[test]
    fn test_substitution() {
        let pass = MonomorphizationPass::new();
        let mut subs = IndexMap::new();
        subs.insert("T".to_string(), IRType::Integer);
        
        let result = pass.substitute_type(&IRType::Generic("T".to_string()), &subs);
        assert_eq!(result, IRType::Integer);
        
        let array_t = IRType::Array(Box::new(IRType::Generic("T".to_string())));
        let result = pass.substitute_type(&array_t, &subs);
        assert_eq!(result, IRType::Array(Box::new(IRType::Integer)));
    }
}

