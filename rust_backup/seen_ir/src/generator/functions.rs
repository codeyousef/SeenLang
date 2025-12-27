//! Function and method definition generation for the IR generator.
//!
//! Handles function definitions, method generation, and interface method signatures.

use crate::{
    function::IRFunction,
    instruction::{Instruction, Label},
    value::{IRType, IRValue},
    IRResult,
};
use seen_parser::{Expression, Parameter as ASTParameter};

use super::IRGenerator;

impl IRGenerator {
    // ==================== DRY Helpers ====================

    /// Convert an optional return type to IRType (defaults to Void).
    fn resolve_return_type(&self, return_type: &Option<seen_parser::ast::Type>) -> IRType {
        return_type
            .as_ref()
            .map(|t| self.convert_ast_type_to_ir(t))
            .unwrap_or(IRType::Void)
    }

    /// Convert AST parameters to IR parameters.
    fn convert_parameters(&self, params: &[ASTParameter]) -> Vec<crate::function::Parameter> {
        params
            .iter()
            .map(|p| {
                let param_type = p
                    .type_annotation
                    .as_ref()
                    .map(|t| self.convert_ast_type_to_ir(t))
                    .unwrap_or(IRType::Integer);
                crate::function::Parameter::new(p.name.clone(), param_type)
            })
            .collect()
    }

    /// Convert seen_parser::Parameter to IR parameters with index-based fallback types.
    fn convert_method_parameters(
        &self,
        params: &[seen_parser::Parameter],
    ) -> Vec<crate::function::Parameter> {
        params
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let param_type = p
                    .type_annotation
                    .as_ref()
                    .map(|t| self.convert_ast_type_to_ir(t))
                    .unwrap_or_else(|| IRType::Generic(format!("T{}", i)));
                crate::function::Parameter {
                    name: p.name.clone(),
                    param_type,
                    is_mutable: false,
                }
            })
            .collect()
    }

    // ==================== Function Generation ====================

    /// Generate IR for function definitions (module level)
    pub(crate) fn generate_function_definition(
        &mut self,
        name: &str,
        params: &[ASTParameter],
        return_type: &Option<seen_parser::ast::Type>,
        body: &Expression,
    ) -> IRResult<IRFunction> {
        // Determine return type and create function
        let ir_return_type = self.resolve_return_type(return_type);
        let mut function = IRFunction::new(name, ir_return_type);

        // Add parameters using helper
        function.parameters = self.convert_parameters(params);

        // Save current context
        let saved_function = self.context.current_function.clone();
        let saved_block = self.context.current_block.clone();
        let saved_register_counter = self.context.register_counter;
        let saved_local_variables = std::mem::take(&mut self.context.local_variables);
        let saved_variable_types = std::mem::take(&mut self.context.variable_types);
        let saved_register_types = std::mem::take(&mut self.context.register_types);
        let saved_result_inner_types = std::mem::take(&mut self.context.result_inner_types);

        // Set up function context
        self.context.current_function = Some(name.to_string());
        self.context.register_counter = 0; // Reset for this function

        // Add parameters to context as variables
        for param in params {
            let param_type = param
                .type_annotation
                .as_ref()
                .map(|t| self.convert_ast_type_to_ir(t))
                .unwrap_or(IRType::Integer);
            self.context
                .set_variable_type(param.name.clone(), param_type);
        }

        // Create entry block for function
        let entry_label = Label::new("entry");
        self.context.current_block = Some(entry_label.0.clone());

        // Generate function body
        let (result_value, mut instructions) = self.generate_expression(body)?;

        if name.contains("compareExecutables") {
             println!("DEBUG: generate_function_definition for {}, instructions len: {}", name, instructions.len());
             for inst in &instructions {
                if format!("{:?}", inst).contains("then_596") {
                    println!("DEBUG: generate_function_definition: Found then_596");
                }
            }
        }

        // Add entry label at the beginning
        instructions.insert(0, Instruction::Label(entry_label));

        // Add return instruction at the end
        instructions.push(Instruction::Return(Some(result_value)));

        // Update function register count
        function.register_count = self.context.register_counter;
        
        // Add locals to function
        function.locals = self.context.local_variables.clone();

        // Build proper CFG from instruction list
        let cfg = crate::cfg_builder::build_cfg_from_instructions(instructions);
        function.cfg = cfg;

        // Restore context
        self.context.current_function = saved_function;
        self.context.current_block = saved_block;
        self.context.register_counter = saved_register_counter;
        self.context.local_variables = saved_local_variables;
        self.context.variable_types = saved_variable_types;
        self.context.register_types = saved_register_types;
        self.context.result_inner_types = saved_result_inner_types;

        Ok(function)
    }

    /// Generate IR for method definitions (similar to function definitions but for class methods)
    pub(crate) fn generate_method_function(
        &mut self,
        name: &str,
        params: &[seen_parser::Parameter],
        return_type: &Option<seen_parser::Type>,
        body: &Expression,
    ) -> IRResult<IRFunction> {
        // Save current context
        let saved_function = self.context.current_function.clone();
        let saved_block = self.context.current_block.clone();
        let saved_register_counter = self.context.register_counter;
        let saved_local_variables = std::mem::take(&mut self.context.local_variables);
        let saved_variable_types = std::mem::take(&mut self.context.variable_types);
        let saved_register_types = std::mem::take(&mut self.context.register_types);
        let saved_result_inner_types = std::mem::take(&mut self.context.result_inner_types);

        // Set up function context
        self.context.current_function = Some(name.to_string());
        self.context.register_counter = 0;

        // Methods optionally include an explicit receiver as the first parameter.
        // If absent, treat as a static method (no receiver) for bootstrap resilience.
        let receiver_type_opt = if !params.is_empty() {
            params[0]
                .type_annotation
                .as_ref()
                .map(|t| self.convert_ast_type_to_ir(t))
                .or_else(|| Some(IRType::Generic("Self".to_string())))
        } else {
            None
        };

        // Build IR parameters using helper
        let ir_params = self.convert_method_parameters(params);

        // Add parameters to context as variables
        for param in &ir_params {
            self.context.set_variable_type(param.name.clone(), param.param_type.clone());
            
            // Alias 'self' -> 'this' if 'this' is present
            if param.name == "this" {
                self.context.set_variable_type("self".to_string(), param.param_type.clone());
            }
        }

        // Determine return type using helper
        let ir_return_type = return_type
            .as_ref()
            .map(|t| self.convert_ast_type_to_ir(t))
            .unwrap_or(IRType::Void);

        // Set receiver context
        let old_receiver_type = self.context._current_receiver_type.clone();
        self.context._current_receiver_type = receiver_type_opt;

        // Generate method body with receiver context
        let (body_value, body_instructions) = self.generate_expression(body)?;

        // Restore receiver context
        self.context._current_receiver_type = old_receiver_type;

        // Create IR function with method semantics
        let mut ir_function = crate::function::IRFunction::new(name, ir_return_type);

        // Add parameters
        for param in ir_params {
            ir_function.add_parameter(param);
        }

        // Create an entry block if missing and append instructions + return
        let entry_label = crate::instruction::Label::new("entry");
        
        // Prepare instructions for CFG builder
        let mut instructions = body_instructions;
        instructions.insert(0, crate::instruction::Instruction::Label(entry_label));
        instructions.push(crate::instruction::Instruction::Return(Some(body_value)));

        // Build proper CFG from instruction list
        let cfg = crate::cfg_builder::build_cfg_from_instructions(instructions);
        ir_function.cfg = cfg;
        
        ir_function.register_count = self.context.register_counter;
        
        // Add locals to function
        ir_function.locals = self.context.local_variables.clone();

        // Restore context
        self.context.current_function = saved_function;
        self.context.current_block = saved_block;
        self.context.register_counter = saved_register_counter;
        self.context.local_variables = saved_local_variables;
        self.context.variable_types = saved_variable_types;
        self.context.register_types = saved_register_types;
        self.context.result_inner_types = saved_result_inner_types;

        Ok(ir_function)
    }

    /// Generate IR for interface method signatures (abstract functions)
    #[allow(dead_code)]
    pub(crate) fn generate_interface_method(
        &mut self,
        name: &str,
        params: &[seen_parser::Parameter],
        return_type: &Option<seen_parser::Type>,
    ) -> IRResult<IRFunction> {
        // Determine return type using consistent pattern
        let ir_return_type = return_type
            .as_ref()
            .map(|t| self.convert_ast_type_to_ir(t))
            .unwrap_or(IRType::Void);

        // Create the function signature (no body for interfaces)
        let mut function = IRFunction::new(name, ir_return_type);

        // Add parameters - use convert_parameters pattern for consistency
        function.parameters = params
            .iter()
            .map(|p| {
                let param_type = p
                    .type_annotation
                    .as_ref()
                    .map(|t| self.convert_ast_type_to_ir(t))
                    .unwrap_or(IRType::Integer);
                crate::function::Parameter::new(p.name.clone(), param_type)
            })
            .collect();

        // Interface methods are abstract - no body implementation
        function.is_public = true;

        Ok(function)
    }

    /// Generate IR for function expressions (now deprecated - use generate_function_definition)
    pub(crate) fn generate_function_expression(
        &mut self,
        name: &str,
        _params: &[ASTParameter],
        _body: &Expression,
    ) -> IRResult<(IRValue, Vec<Instruction>)> {
        // Function expressions should not occur in the main generation flow anymore
        // They're handled at the module level
        Ok((IRValue::Variable(format!("function_{}", name)), Vec::new()))
    }
}
