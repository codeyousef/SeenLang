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
    /// Generate IR for function definitions (module level)
    pub(crate) fn generate_function_definition(
        &mut self,
        name: &str,
        params: &[ASTParameter],
        return_type: &Option<seen_parser::ast::Type>,
        body: &Expression,
    ) -> IRResult<IRFunction> {
        // Determine return type
        let ir_return_type = if let Some(ret_type) = return_type {
            self.convert_ast_type_to_ir(ret_type)
        } else {
            IRType::Void
        };

        // Create the function
        let mut function = IRFunction::new(name, ir_return_type);

        // Add parameters
        for param in params {
            let param_type = if let Some(type_annotation) = &param.type_annotation {
                self.convert_ast_type_to_ir(type_annotation)
            } else {
                IRType::Integer // Default fallback
            };
            function.parameters.push(crate::function::Parameter::new(
                param.name.clone(),
                param_type,
            ));
        }

        // Save current context
        let saved_function = self.context.current_function.clone();
        let saved_block = self.context.current_block.clone();
        let saved_register_counter = self.context.register_counter;

        // Set up function context
        self.context.current_function = Some(name.to_string());
        self.context.register_counter = 0; // Reset for this function

        // Add parameters to context as variables
        for param in params {
            let param_type = if let Some(type_annotation) = &param.type_annotation {
                self.convert_ast_type_to_ir(type_annotation)
            } else {
                IRType::Integer // Default fallback
            };
            self.context
                .set_variable_type(param.name.clone(), param_type);
        }

        // Create entry block for function
        let entry_label = Label::new("entry");
        self.context.current_block = Some(entry_label.0.clone());

        // Generate function body
        let (result_value, mut instructions) = self.generate_expression(body)?;

        // Add entry label at the beginning
        instructions.insert(0, Instruction::Label(entry_label));

        // Add return instruction at the end
        instructions.push(Instruction::Return(Some(result_value)));

        // Update function register count
        function.register_count = self.context.register_counter;

        // Build proper CFG from instruction list
        let cfg = crate::cfg_builder::build_cfg_from_instructions(instructions);
        function.cfg = cfg;

        // Restore context
        self.context.current_function = saved_function;
        self.context.current_block = saved_block;
        self.context.register_counter = saved_register_counter;

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
        // Methods optionally include an explicit receiver as the first parameter.
        // If absent, treat as a static method (no receiver) for bootstrap resilience.
        let _receiver_type_opt = if !params.is_empty() {
            if let Some(type_ann) = &params[0].type_annotation {
                Some(self.convert_ast_type_to_ir(type_ann))
            } else {
                Some(IRType::Generic("Self".to_string()))
            }
        } else {
            None
        };

        // Build IR parameters including explicit receiver when present
        let mut ir_params = Vec::new();
        for (i, param) in params.iter().enumerate() {
            let param_type = if let Some(type_ann) = &param.type_annotation {
                self.convert_ast_type_to_ir(type_ann)
            } else {
                IRType::Generic(format!("T{}", i))
            };

            ir_params.push(crate::function::Parameter {
                name: param.name.clone(),
                param_type,
                is_mutable: false,
            });
        }

        // Determine return type
        let ir_return_type = if let Some(ret_type) = return_type {
            self.convert_ast_type_to_ir(ret_type)
        } else {
            IRType::Void
        };

        // Generate method body with receiver context
        let (body_value, body_instructions) = self.generate_expression(body)?;

        // Create IR function with method semantics
        let mut ir_function = crate::function::IRFunction::new(name, ir_return_type);

        // Add parameters
        for param in ir_params {
            ir_function.add_parameter(param);
        }

        // Create an entry block if missing and append instructions + return
        let entry_label = crate::instruction::Label::new("entry");
        let mut entry_block = crate::instruction::BasicBlock::new(entry_label.clone());
        entry_block.instructions.extend(body_instructions);
        entry_block.terminator = Some(crate::instruction::Instruction::Return(Some(
            body_value.clone(),
        )));
        ir_function.add_block(entry_block);
        ir_function.register_count = self.context.register_counter;

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
        // Determine return type
        let ir_return_type = if let Some(ret_type) = return_type {
            self.convert_ast_type_to_ir(ret_type)
        } else {
            IRType::Void
        };

        // Create the function signature (no body for interfaces)
        let mut function = IRFunction::new(name, ir_return_type);

        // Add parameters
        for param in params {
            let param_type = if let Some(type_annotation) = &param.type_annotation {
                self.convert_ast_type_to_ir(type_annotation)
            } else {
                IRType::Integer // Default fallback
            };
            function.parameters.push(crate::function::Parameter::new(
                param.name.clone(),
                param_type,
            ));
        }

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
