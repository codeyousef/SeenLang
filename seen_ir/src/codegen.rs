use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::passes::PassBuilderOptions;
use inkwell::targets::{CodeModel, InitializationConfig, RelocMode, Target, TargetMachine};
use inkwell::types::{AnyTypeEnum, BasicType, BasicTypeEnum};
use inkwell::values::{BasicValueEnum, FunctionValue, PointerValue};
use inkwell::Either;
use inkwell::OptimizationLevel;
use std::collections::HashMap;

use crate::error::{CodeGenError, Result};
use crate::mapping::{map_binary_operator, map_unary_operator};
use crate::types::TypeSystem;
use seen_parser::ast::{self, Declaration, Expression, Program, Statement};
use seen_lexer::token::{Location, Position};

/// Environment for storing variables during code generation
struct Environment<'ctx> {
    /// Variables in the current scope (pointer and type)
    variables: HashMap<String, (PointerValue<'ctx>, BasicTypeEnum<'ctx>)>,
    /// Parent environment (for nested scopes)
    parent: Option<Box<Environment<'ctx>>>,
}

impl<'ctx> Environment<'ctx> {
    /// Create a new environment
    fn new() -> Self {
        Self {
            variables: HashMap::new(),
            parent: None,
        }
    }

    /// Create a new nested environment with the given parent
    fn with_parent(parent: Environment<'ctx>) -> Self {
        Self {
            variables: HashMap::new(),
            parent: Some(Box::new(parent)),
        }
    }

    /// Define a variable in the current scope
    fn define(&mut self, name: &str, ptr: PointerValue<'ctx>, ty: BasicTypeEnum<'ctx>) {
        self.variables.insert(name.to_string(), (ptr, ty));
    }

    /// Get a variable pointer from any scope
    fn get(&self, name: &str) -> Option<PointerValue<'ctx>> {
        if let Some((ptr, _)) = self.variables.get(name) {
            Some(*ptr)
        } else if let Some(parent) = &self.parent {
            parent.get(name)
        } else {
            None
        }
    }

    /// Get a variable with its type from any scope
    fn get_with_type(&self, name: &str) -> Option<(PointerValue<'ctx>, BasicTypeEnum<'ctx>)> {
        if let Some((ptr, ty)) = self.variables.get(name) {
            Some((*ptr, *ty))
        } else if let Some(parent) = &self.parent {
            parent.get_with_type(name)
        } else {
            None
        }
    }
}

/// Code generator for translating Seen AST to LLVM IR
pub struct CodeGenerator<'ctx> {
    context: &'ctx Context,
    module: Module<'ctx>,
    builder: Builder<'ctx>,
    type_system: TypeSystem<'ctx>,
    environment: Environment<'ctx>,
}

impl<'ctx> CodeGenerator<'ctx> {
    /// Create a new code generator
    pub fn new(context: &'ctx Context, module_name: &str) -> Self {
        let builder = context.create_builder();
        let module = context.create_module(module_name);
        let environment = Environment::new();
        let type_system = TypeSystem::new(context);
        CodeGenerator {
            context,
            builder,
            module,
            environment,
            type_system,
        }
    }

    /// Generate LLVM IR for a program
    pub fn generate(&mut self, program: &Program) -> Result<&Module<'ctx>> {
        // Create a declaration for the C printf function
        self.declare_printf();

        // Create standard library functions
        self.create_stdlib_functions()?;

        // Process all declarations in the program
        for declaration in &program.declarations {
            self.generate_declaration(declaration)?;
        }

        // Run optimization passes after all IR is generated.
        self.run_optimization_passes()?;

        Ok(&self.module)
    }

    /// Generate code for a declaration
    fn generate_declaration(&mut self, declaration: &Declaration) -> Result<()> {
        match declaration {
            Declaration::Function(func_decl) => {
                self.generate_function(func_decl)?;
            }
            Declaration::Variable(var_decl) => {
                // Check if we're inside a function or at global scope
                if self.builder.get_insert_block().is_some() {
                    // We're inside a function, create a local variable
                    self.generate_local_variable(var_decl)?;
                } else {
                    // We're at global scope
                    self.generate_global_variable(var_decl)?;
                }
            }
            Declaration::Struct(struct_decl) => {
                self.generate_struct_declaration(struct_decl)?;
            }
            Declaration::Enum(enum_decl) => {
                self.generate_enum_declaration(enum_decl)?;
            }
        }

        Ok(())
    }

    /// Generate code for a function declaration
    fn generate_function(
        &mut self,
        func_decl: &ast::FunctionDeclaration,
    ) -> Result<FunctionValue<'ctx>> {
        // Get the return type
        let return_type = if let Some(ret_type) = &func_decl.return_type {
            match self.type_system.convert_type(ret_type) {
                Ok(t) => Some(t),
                Err(_) if matches!(ret_type, ast::Type::Simple(name) if name == "void") => None,
                Err(e) => return Err(e),
            }
        } else {
            None
        };

        // Create function parameter types
        let param_types: Vec<inkwell::types::BasicMetadataTypeEnum<'ctx>> = func_decl
            .parameters
            .iter()
            .map(|param| {
                self.type_system
                    .convert_type(&param.param_type)
                    .map(|basic_type| basic_type.into())
                    .map_err(|e| CodeGenError::TypeMismatch {
                        expected: "valid type".to_string(),
                        actual: format!("{:?}", e),
                    })
            })
            .collect::<Result<Vec<_>>>()?;

        // Create the function type
        let fn_type = match return_type {
            Some(ret_type) => ret_type.fn_type(&param_types, false),
            None => self.type_system.void_type().fn_type(&param_types, false),
        };

        // Create the function
        let function = self.module.add_function(&func_decl.name, fn_type, None);

        // Create a basic block for the function
        let basic_block = self.context.append_basic_block(function, "entry");
        self.builder.position_at_end(basic_block);

        // Create a new environment for this function
        let parent_env = std::mem::replace(&mut self.environment, Environment::new());
        self.environment = Environment::with_parent(parent_env);

        // Set parameter names and add them to the environment
        for (i, param) in func_decl.parameters.iter().enumerate() {
            let param_value = function.get_nth_param(i as u32).unwrap();
            param_value.set_name(&param.name);

            // Allocate memory for the parameter on the stack
            let param_ptr = self.create_entry_block_alloca(&param.name, param_value.get_type());

            // Store the parameter value in the alloca
            self.builder.build_store(param_ptr, param_value)?;

            // Add the parameter to the environment
            self.environment.define(&param.name, param_ptr, param_value.get_type());
        }

        // Generate code for the function body
        self.generate_block(&func_decl.body)?;

        // Ensure the function has a terminator
        if self.builder.get_insert_block().unwrap().get_terminator().is_none() {
            // If there's no explicit return, add a default return
            if return_type.is_none() {
                self.builder.build_return(None)?;
            } else {
                // For non-void functions without explicit return, this is a semantic error
                // For now, we'll return a default value instead of None
                let default_value: BasicValueEnum = match return_type.unwrap() {
                    BasicTypeEnum::IntType(int_type) => int_type.const_zero().into(),
                    BasicTypeEnum::FloatType(float_type) => float_type.const_zero().into(),
                    BasicTypeEnum::PointerType(ptr_type) => ptr_type.const_null().into(),
                    _ => {
                        return Err(CodeGenError::CodeGeneration(
                            "Function missing return statement".to_string()
                        ));
                    }
                };
                self.builder.build_return(Some(&default_value))?;
            }
        }

        // Verify the function
        if function.verify(true) {
            Ok(function)
        } else {
            // Remove the invalid function
            unsafe {
                function.delete();
            }
            Err(CodeGenError::CodeGeneration(
                "Invalid function generated".to_string(),
            ))
        }
    }

    /// Generate code for a local variable
    fn generate_local_variable(
        &mut self,
        var_decl: &ast::VariableDeclaration,
    ) -> Result<()> {
        // Generate the initializer expression
        let init_value = self.generate_expression(&var_decl.initializer)?;
        let var_type = init_value.get_type();

        // If there's an explicit type annotation, verify it matches
        if let Some(type_ann) = &var_decl.var_type {
            let expected_type = self.type_system.convert_type(type_ann)?;
            // TODO: Add proper type checking here
            // For now, we'll just use the inferred type
        }

        // Create an alloca for the variable
        let var_ptr = self.create_entry_block_alloca(&var_decl.name, var_type);

        // Store the initial value
        self.builder.build_store(var_ptr, init_value)?;

        // Add to the environment
        self.environment.define(&var_decl.name, var_ptr, var_type);

        Ok(())
    }

    /// Generate code for a global variable
    fn generate_global_variable(
        &mut self,
        var_decl: &ast::VariableDeclaration,
    ) -> Result<PointerValue<'ctx>> {
        // Get the variable type
        let var_type = if let Some(type_ann) = &var_decl.var_type {
            self.type_system.convert_type(type_ann)?
        } else {
            // Infer the type from the initializer
            let init_val = self.generate_expression(&var_decl.initializer)?;
            init_val.get_type()
        };

        // Create the global variable
        let global = self.module.add_global(var_type, None, &var_decl.name);

        // Generate the initializer
        let initializer = self.generate_expression(&var_decl.initializer)?;

        // Set the initializer
        match initializer {
            BasicValueEnum::IntValue(v) => global.set_initializer(&v),
            BasicValueEnum::FloatValue(v) => global.set_initializer(&v),
            BasicValueEnum::PointerValue(v) => global.set_initializer(&v),
            BasicValueEnum::ArrayValue(v) => global.set_initializer(&v),
            BasicValueEnum::StructValue(v) => global.set_initializer(&v),
            BasicValueEnum::VectorValue(v) => global.set_initializer(&v),
            BasicValueEnum::ScalableVectorValue(_) => {
                todo!("Handle ScalableVectorValue initialization")
            }
        }

        // Set the linkage (internal for constants, external for variables)
        if !var_decl.is_mutable {
            global.set_constant(true);
        }

        Ok(global.as_pointer_value())
    }

    /// Generate code for a block
    fn generate_block(&mut self, block: &ast::Block) -> Result<()> {
        // Create a new environment for this block
        let parent_env = std::mem::replace(&mut self.environment, Environment::new());
        self.environment = Environment::with_parent(parent_env);

        // Generate code for each statement in the block
        for statement in &block.statements {
            self.generate_statement(statement)?;
        }

        // Restore the parent environment
        if let Some(parent) = self.environment.parent.take() {
            self.environment = *parent;
        }

        Ok(())
    }

    /// Generate code for a statement
    fn generate_statement(&mut self, statement: &Statement) -> Result<()> {
        match statement {
            Statement::Expression(expr_stmt) => {
                self.generate_expression(&expr_stmt.expression)?;
                Ok(())
            }
            Statement::Block(block) => self.generate_block(block),
            Statement::Return(ret_stmt) => {
                if let Some(expr) = &ret_stmt.value {
                    let return_value = self.generate_expression(expr)?;
                    self.builder.build_return(Some(&return_value))?;
                } else {
                    self.builder.build_return(None)?;
                }
                Ok(())
            }
            Statement::If(if_stmt) => {
                let condition_val = self.generate_expression(&if_stmt.condition)?;
                let condition_val = self.builder.build_int_compare(
                    inkwell::IntPredicate::NE,
                    condition_val.into_int_value(),
                    self.type_system.bool_type().const_int(0, false),
                    "ifcond",
                )?;

                let function = self
                    .builder
                    .get_insert_block()
                    .unwrap()
                    .get_parent()
                    .unwrap();

                let then_block = self.context.append_basic_block(function, "then");
                let else_block = self.context.append_basic_block(function, "else");
                let merge_block = self.context.append_basic_block(function, "ifcont");

                self.builder
                    .build_conditional_branch(condition_val, then_block, else_block)?;

                self.builder.position_at_end(then_block);
                self.generate_statement(&if_stmt.then_branch)?;

                // Branch to merge block if not terminated
                if self
                    .builder
                    .get_insert_block()
                    .unwrap()
                    .get_terminator()
                    .is_none()
                {
                    self.builder.build_unconditional_branch(merge_block)?;
                }

                self.builder.position_at_end(else_block);
                if let Some(else_branch) = &if_stmt.else_branch {
                    self.generate_statement(else_branch)?;
                }

                // Branch to merge block if not terminated
                if self
                    .builder
                    .get_insert_block()
                    .unwrap()
                    .get_terminator()
                    .is_none()
                {
                    self.builder.build_unconditional_branch(merge_block)?;
                }

                self.builder.position_at_end(merge_block);

                Ok(())
            }
            Statement::While(while_stmt) => {
                let function = self
                    .builder
                    .get_insert_block()
                    .unwrap()
                    .get_parent()
                    .unwrap();

                let cond_block = self.context.append_basic_block(function, "while.cond");
                let loop_block = self.context.append_basic_block(function, "while.body");
                let merge_block = self.context.append_basic_block(function, "while.end");

                self.builder.build_unconditional_branch(cond_block)?;

                self.builder.position_at_end(cond_block);
                let condition_val = self.generate_expression(&while_stmt.condition)?;
                let condition_val = self.builder.build_int_compare(
                    inkwell::IntPredicate::NE,
                    condition_val.into_int_value(),
                    self.type_system.bool_type().const_int(0, false),
                    "whilecond",
                )?;

                self.builder
                    .build_conditional_branch(condition_val, loop_block, merge_block)?;

                self.builder.position_at_end(loop_block);
                self.generate_statement(&while_stmt.body)?;

                // Branch back to the condition block if not terminated
                if self
                    .builder
                    .get_insert_block()
                    .unwrap()
                    .get_terminator()
                    .is_none()
                {
                    self.builder.build_unconditional_branch(cond_block)?;
                }

                self.builder.position_at_end(merge_block);

                Ok(())
            }
            Statement::Print(print_stmt) => {
                if print_stmt.arguments.is_empty() {
                    return Err(CodeGenError::InvalidASTNode {
                        location: format!("{:?}", print_stmt.location),
                        message: "Print statement has no arguments".to_string(),
                    });
                }

                let mut llvm_values = Vec::with_capacity(print_stmt.arguments.len());
                for arg_expr in &print_stmt.arguments {
                    llvm_values.push(self.generate_expression(arg_expr)?);
                }

                self.build_printf_call(&llvm_values)?;
                Ok(())
            }
            Statement::For(for_stmt) => {
                // For now, we only support range expressions as iterables
                match &for_stmt.iterable {
                    Expression::Range(range_expr) => {
                        self.generate_for_range_loop(for_stmt, range_expr)?;
                        Ok(())
                    }
                    _ => {
                        return Err(CodeGenError::UnsupportedOperation(
                            "For loops currently only support range expressions".to_string()
                        ));
                    }
                }
            }
            Statement::Match(match_stmt) => {
                self.generate_match_statement(match_stmt)
            }
            Statement::DeclarationStatement(decl) => self.generate_declaration(decl),
        }
    }

    /// Generate code for an expression
    fn generate_expression(&mut self, expression: &Expression) -> Result<BasicValueEnum<'ctx>> {
        match expression {
            Expression::Identifier(ident_expr) => {
                let (var_ptr, var_type) = self.environment.get_with_type(&ident_expr.name).ok_or_else(|| {
                    CodeGenError::UndefinedSymbol(format!(
                        "Undefined variable: {}",
                        ident_expr.name
                    ))
                })?;
                let loaded_value =
                    self.builder
                        .build_load(var_type, var_ptr, &ident_expr.name)?;
                Ok(loaded_value)
            }
            Expression::Literal(lit_expr) => match lit_expr {
                ast::LiteralExpression::Number(num_lit) => {
                    if num_lit.is_float {
                        let float_val = num_lit.value.parse::<f64>().map_err(|_| {
                            CodeGenError::CodeGeneration(format!(
                                "Invalid float literal: {}",
                                num_lit.value
                            ))
                        })?;
                        Ok(self.context.f64_type().const_float(float_val).into())
                    } else {
                        let int_val = num_lit.value.parse::<i64>().map_err(|_| {
                            CodeGenError::CodeGeneration(format!(
                                "Invalid integer literal: {}",
                                num_lit.value
                            ))
                        })?;
                        Ok(self
                            .context
                            .i64_type()
                            .const_int(int_val as u64, true)
                            .into())
                    }
                }
                ast::LiteralExpression::String(str_lit) => {
                    let string_value = self
                        .builder
                        .build_global_string_ptr(&str_lit.value, ".str")?;
                    Ok(string_value.as_pointer_value().into())
                }
                ast::LiteralExpression::Boolean(bool_lit) => Ok(self
                    .context
                    .bool_type()
                    .const_int(bool_lit.value as u64, false)
                    .into()),
                ast::LiteralExpression::Null(_null_lit) => {
                    Ok(self.context.ptr_type(0.into()).const_null().into())
                }
            },
            Expression::Unary(unary_expr) => {
                let operand_val = self.generate_expression(&unary_expr.operand)?;
                let value_result =
                    map_unary_operator(&unary_expr.operator, operand_val, &self.builder, |msg| {
                        CodeGenError::UnsupportedOperation(msg)
                    });
                Ok(value_result?)
            }
            Expression::Binary(binary_expr) => {
                let left_val = self.generate_expression(&binary_expr.left)?;
                let right_val = self.generate_expression(&binary_expr.right)?;
                let value_result = map_binary_operator(
                    &binary_expr.operator,
                    left_val,
                    right_val,
                    &self.builder,
                    |msg| CodeGenError::UnsupportedOperation(msg),
                );
                Ok(value_result?)
            }
            Expression::Assignment(assign_expr) => {
                // assign_expr is AssignmentExpression
                let var_ptr = self.environment.get(&assign_expr.name).ok_or_else(|| {
                    CodeGenError::UndefinedSymbol(format!(
                        "Undefined variable for assignment: {}",
                        assign_expr.name
                    ))
                })?;
                let val_to_assign = self.generate_expression(&assign_expr.value)?;
                self.builder.build_store(var_ptr, val_to_assign)?;
                Ok(val_to_assign)
            }
            Expression::Call(call_expr) => {
                // call_expr is CallExpression
                let function = self.module.get_function(&call_expr.callee).ok_or_else(|| {
                    CodeGenError::UndefinedSymbol(format!(
                        "Function '{}' not found",
                        call_expr.callee
                    ))
                })?;

                let mut args = Vec::new();
                for arg_expr in &call_expr.arguments {
                    args.push(self.generate_expression(arg_expr)?.into());
                }

                let call_site_value = self.builder.build_call(function, &args, "call")?;
                // Check if the function returns void or a value
                if function.get_type().get_return_type().is_some() {
                    match call_site_value.try_as_basic_value() {
                        Either::Left(basic_value) => Ok(basic_value),
                        Either::Right(_) => Err(CodeGenError::CodeGeneration(
                            "Call did not produce a basic value as expected".to_string(),
                        )),
                    }
                } else {
                    // If function is void, we can't return its value.
                    // For now, returning a dummy i32 zero. This might need adjustment
                    // depending on how void calls are handled in expressions.
                    Ok(self.context.i32_type().const_zero().into())
                }
            }
            Expression::Parenthesized(paren_expr) => {
                self.generate_expression(&paren_expr.expression)
            }
            Expression::StructLiteral(struct_lit) => {
                self.generate_struct_literal(struct_lit)
            }
            Expression::FieldAccess(field_access) => {
                self.generate_field_access(field_access)
            }
            Expression::ArrayLiteral(array_lit) => {
                self.generate_array_literal(array_lit)
            }
            Expression::Index(index_expr) => {
                self.generate_array_index(index_expr)
            }
            Expression::Range(range_expr) => {
                // For now, ranges are only used in for loops, so we'll return a dummy value
                // In a more complete implementation, we'd create a range struct
                let start_val = self.generate_expression(&range_expr.start)?;
                let _end_val = self.generate_expression(&range_expr.end)?;
                
                // For now, just return the start value as a placeholder
                // This is fine because ranges are only used in for loops which handle them specially
                Ok(start_val)
            }
            Expression::Match(match_expr) => {
                self.generate_match_expression(match_expr)
            }
            Expression::EnumLiteral(enum_lit) => {
                self.generate_enum_literal(enum_lit)
            }
            Expression::Try(try_expr) => {
                self.generate_try_expression(try_expr)
            }
        }
    }

    /// Generate code for a for loop over a range
    fn generate_for_range_loop(
        &mut self,
        for_stmt: &ast::ForStatement,
        range_expr: &ast::RangeExpression,
    ) -> Result<()> {
        // Create a new environment for the loop
        let parent_env = std::mem::replace(&mut self.environment, Environment::new());
        self.environment = Environment::with_parent(parent_env);

        // Evaluate range bounds
        let start_val = self.generate_expression(&range_expr.start)?;
        let end_val = self.generate_expression(&range_expr.end)?;

        // Ensure both are integers
        if !start_val.is_int_value() || !end_val.is_int_value() {
            return Err(CodeGenError::TypeMismatch {
                expected: "integer".to_string(),
                actual: "non-integer type in range".to_string(),
            });
        }

        let start_int = start_val.into_int_value();
        let end_int = end_val.into_int_value();

        // Create the loop variable
        let loop_var_ptr = self.create_entry_block_alloca(
            &for_stmt.variable,
            self.context.i64_type().into(),
        );
        self.builder.build_store(loop_var_ptr, start_int)?;
        self.environment.define(&for_stmt.variable, loop_var_ptr, self.context.i64_type().into());

        // Get the current function
        let function = self
            .builder
            .get_insert_block()
            .unwrap()
            .get_parent()
            .unwrap();

        // Create basic blocks
        let cond_block = self.context.append_basic_block(function, "for.cond");
        let body_block = self.context.append_basic_block(function, "for.body");
        let incr_block = self.context.append_basic_block(function, "for.incr");
        let exit_block = self.context.append_basic_block(function, "for.exit");

        // Jump to condition block
        self.builder.build_unconditional_branch(cond_block)?;

        // Condition block: check if loop_var < end
        self.builder.position_at_end(cond_block);
        let current_val = self.builder.build_load(
            self.context.i64_type(),
            loop_var_ptr,
            "current",
        )?.into_int_value();
        let cond = self.builder.build_int_compare(
            inkwell::IntPredicate::SLT,
            current_val,
            end_int,
            "loopcond",
        )?;
        self.builder.build_conditional_branch(cond, body_block, exit_block)?;

        // Body block: execute loop body
        self.builder.position_at_end(body_block);
        self.generate_statement(&for_stmt.body)?;
        
        // Jump to increment block if not terminated
        if self.builder.get_insert_block().unwrap().get_terminator().is_none() {
            self.builder.build_unconditional_branch(incr_block)?;
        }

        // Increment block: increment loop variable
        self.builder.position_at_end(incr_block);
        let current_val = self.builder.build_load(
            self.context.i64_type(),
            loop_var_ptr,
            "current",
        )?.into_int_value();
        let one = self.context.i64_type().const_int(1, false);
        let next_val = self.builder.build_int_add(current_val, one, "nextval")?;
        self.builder.build_store(loop_var_ptr, next_val)?;
        self.builder.build_unconditional_branch(cond_block)?;

        // Exit block: continue after loop
        self.builder.position_at_end(exit_block);

        // Restore the parent environment
        if let Some(parent) = self.environment.parent.take() {
            self.environment = *parent;
        }

        Ok(())
    }

    /// Create an alloca instruction at the entry point of the function
    fn create_entry_block_alloca(&self, name: &str, ty: BasicTypeEnum<'ctx>) -> PointerValue<'ctx> {
        let builder = self.context.create_builder();

        match &self.builder.get_insert_block() {
            Some(block) => match block.get_parent() {
                Some(function) => {
                    let entry = function.get_first_basic_block().unwrap();

                    match entry.get_first_instruction() {
                        Some(first_instr) => {
                            builder.position_before(&first_instr);
                        }
                        None => {
                            builder.position_at_end(entry);
                        }
                    }
                }
                None => {
                    panic!("Block has no parent function");
                }
            },
            None => {
                panic!("No current block to insert into");
            }
        }

        builder
            .build_alloca(ty, name)
            .expect("Failed to build alloca")
    }

    /// Declare the printf function for use in print statements
    fn declare_printf(&self) -> FunctionValue<'ctx> {
        if let Some(printf) = self.module.get_function("printf") {
            return printf;
        }

        let printf_type = self.context.i32_type().fn_type(
            &[self
                .context
                .ptr_type(inkwell::AddressSpace::default())
                .into()], // Format string (char*)
            true, // is_var_args
        );

        self.module.add_function("printf", printf_type, None)
    }

    /// Build a call to printf with the appropriate format string for multiple values
    fn build_printf_call(&self, values: &[BasicValueEnum<'ctx>]) -> Result<()> {
        let printf = self.declare_printf();

        if values.is_empty() {
            // Optionally, print just a newline or handle as an error/do nothing
            let format_string_ptr = self
                .builder
                .build_global_string_ptr("\n", "empty_print_format")?;
            self.builder.build_call(
                printf,
                &[format_string_ptr.as_pointer_value().into()],
                "printf_call_empty",
            )?;
            return Ok(());
        }

        let mut format_string = String::new();
        let mut call_args = Vec::with_capacity(values.len() + 1);

        for (i, value) in values.iter().enumerate() {
            match value {
                BasicValueEnum::IntValue(iv) => {
                    // Check if it's a boolean (i1). If so, print true/false string or 0/1.
                    // For simplicity here, we'll print 0/1 for i1, and decimal for others.
                    // A more robust solution would involve type information from the AST.
                    if iv.get_type().get_bit_width() == 1 {
                        format_string.push_str("%d"); // or handle as string "true"/"false"
                    } else {
                        format_string.push_str("%lld"); // long long for i64, standard for others
                    }
                }
                BasicValueEnum::FloatValue(_) => {
                    format_string.push_str("%f"); // %f for double, %lf is for scanf
                }
                BasicValueEnum::PointerValue(_pv) => {
                    // Assuming pointer is a C string (char*)
                    // A more robust system would check the actual pointed-to type.
                    // Especially if you have pointers to other things like structs/arrays.
                    format_string.push_str("%s");
                }
                _ => {
                    return Err(CodeGenError::UnsupportedOperation(format!(
                        "Cannot print value of type {:?} at argument {}",
                        value.get_type(),
                        i
                    )));
                }
            }
            if i < values.len() - 1 {
                format_string.push(' '); // Add a space between format specifiers
            }
        }
        format_string.push('\n'); // Add a newline at the end

        let format_string_ptr = self
            .builder
            .build_global_string_ptr(&format_string, "dynamic_format_str")?;
        call_args.push(format_string_ptr.as_pointer_value().into());

        for value in values {
            call_args.push((*value).into());
        }

        self.builder
            .build_call(printf, &call_args, "printf_call_multi")?;

        Ok(())
    }

    /// Compile the generated module to an object file
    pub fn compile_to_object(&self, filename: &str) -> Result<()> {
        let target_triple = inkwell::targets::TargetMachine::get_default_triple();

        inkwell::targets::Target::initialize_all(&inkwell::targets::InitializationConfig::default());

        let target = inkwell::targets::Target::from_triple(&target_triple)
            .map_err(|e| CodeGenError::CodeGeneration(format!("Failed to get target: {:?}", e)))?;

        let target_machine = target
            .create_target_machine(
                &target_triple,
                "generic",
                "",
                OptimizationLevel::Default,
                inkwell::targets::RelocMode::Default,
                inkwell::targets::CodeModel::Default,
            )
            .ok_or_else(|| {
                CodeGenError::CodeGeneration("Failed to create target machine".to_string())
            })?;

        target_machine
            .write_to_file(
                &self.module,
                inkwell::targets::FileType::Object,
                filename.as_ref(),
            )
            .map_err(|e| {
                CodeGenError::CodeGeneration(format!("Failed to write object file: {:?}", e))
            })?;

        Ok(())
    }

    /// Generate code for array literals
    fn generate_array_literal(&mut self, array_lit: &ast::ArrayLiteralExpression) -> Result<BasicValueEnum<'ctx>> {
        if array_lit.elements.is_empty() {
            return Err(CodeGenError::CodeGeneration(
                "Empty array literals not yet supported".to_string()
            ));
        }

        // Generate code for all elements and determine common type
        let mut element_values = Vec::new();
        let mut element_type: Option<BasicTypeEnum> = None;

        for element in &array_lit.elements {
            let value = self.generate_expression(element)?;
            element_values.push(value);

            // Use the first element's type as the array element type
            if element_type.is_none() {
                element_type = Some(value.get_type());
            }
        }

        let elem_type = element_type.unwrap();
        let array_length = array_lit.elements.len() as u32;

        // Create array type
        let array_type = elem_type.array_type(array_length);

        // Allocate space for the array on the stack
        let array_ptr = self.builder.build_alloca(array_type, "array_literal")?;

        // Store each element in the array
        for (i, value) in element_values.iter().enumerate() {
            let index = self.context.i32_type().const_int(i as u64, false);
            let element_ptr = unsafe {
                self.builder.build_gep(
                    array_type,
                    array_ptr,
                    &[self.context.i32_type().const_zero(), index],
                    &format!("array_elem_{}", i)
                )?
            };
            self.builder.build_store(element_ptr, *value)?;
        }

        // Return a pointer to the array (arrays are passed by reference)
        Ok(array_ptr.into())
    }

    /// Generate code for array indexing
    fn generate_array_index(&mut self, index_expr: &ast::IndexExpression) -> Result<BasicValueEnum<'ctx>> {
        // Generate the array expression (should be a pointer to array)
        let array_ptr = self.generate_expression(&index_expr.object)?;
        
        // Generate the index expression (should be an integer)
        let index_val = self.generate_expression(&index_expr.index)?;
        
        // Ensure index is an integer
        let index_int = match index_val {
            BasicValueEnum::IntValue(iv) => iv,
            _ => {
                return Err(CodeGenError::CodeGeneration(
                    "Array index must be an integer".to_string()
                ));
            }
        };

        // Ensure array is a pointer
        let array_pointer = match array_ptr {
            BasicValueEnum::PointerValue(pv) => pv,
            _ => {
                return Err(CodeGenError::CodeGeneration(
                    "Array indexing requires a pointer to array".to_string()
                ));
            }
        };

        // For array indexing, we need to know the array type
        // Since we're dealing with stack-allocated arrays from array literals,
        // we need to handle this differently in LLVM
        // For now, we'll assume i64 array type and improve this later
        let array_type = self.context.i64_type().array_type(10);
        let array_length = array_type.len();

        // Add bounds checking
        let zero = self.context.i64_type().const_zero();
        let length_const = self.context.i64_type().const_int(array_length as u64, false);
        
        // Check if index < 0 (if signed)
        let is_negative = self.builder.build_int_compare(
            inkwell::IntPredicate::SLT,
            index_int,
            zero,
            "is_negative"
        )?;
        
        // Check if index >= length
        let is_too_large = self.builder.build_int_compare(
            inkwell::IntPredicate::SGE,
            index_int,
            length_const,
            "is_too_large"
        )?;
        
        // Combine bounds checks
        let out_of_bounds = self.builder.build_or(is_negative, is_too_large, "out_of_bounds")?;
        
        // Create basic blocks for bounds check
        let current_block = self.builder.get_insert_block().unwrap();
        let function = current_block.get_parent().unwrap();
        
        let bounds_ok_block = self.context.append_basic_block(function, "bounds_ok");
        let bounds_fail_block = self.context.append_basic_block(function, "bounds_fail");
        let merge_block = self.context.append_basic_block(function, "bounds_merge");
        
        // Branch based on bounds check
        self.builder.build_conditional_branch(out_of_bounds, bounds_fail_block, bounds_ok_block)?;
        
        // Bounds failure block - return zero as error value
        self.builder.position_at_end(bounds_fail_block);
        let error_value = self.context.i64_type().const_zero();
        self.builder.build_unconditional_branch(merge_block)?;
        
        // Bounds OK block - do normal array access
        self.builder.position_at_end(bounds_ok_block);
        
        // Generate GEP instruction to get element pointer
        let element_ptr = unsafe {
            self.builder.build_gep(
                array_type,
                array_pointer,
                &[self.context.i32_type().const_zero(), index_int],
                "array_index"
            )?
        };

        // Load the value from the element pointer
        let loaded_value = self.builder.build_load(
            array_type.get_element_type(),
            element_ptr,
            "array_element"
        )?;
        
        // Branch to merge block
        self.builder.build_unconditional_branch(merge_block)?;
        
        // Create merge block with phi node
        self.builder.position_at_end(merge_block);
        let phi = self.builder.build_phi(array_type.get_element_type(), "bounds_result")?;
        phi.add_incoming(&[(&loaded_value, bounds_ok_block), (&error_value, bounds_fail_block)]);

        Ok(phi.as_basic_value())
    }

    /// Generate code for struct declarations
    fn generate_struct_declaration(&mut self, struct_decl: &ast::StructDeclaration) -> Result<()> {
        // Extract field names and types
        let mut field_names = Vec::new();
        let mut field_types = Vec::new();
        for field in &struct_decl.fields {
            field_names.push(field.name.clone());
            let field_type = self.type_system.convert_type(&field.field_type)?;
            field_types.push(field_type);
        }

        // Define the struct type in the type system
        self.type_system.define_struct_type(&struct_decl.name, field_names, field_types)?;

        Ok(())
    }

    /// Generate code for enum declarations
    fn generate_enum_declaration(&mut self, enum_decl: &ast::EnumDeclaration) -> Result<()> {
        // For generic enums, we don't generate LLVM types immediately
        // Instead, we store the template and generate concrete types when instantiated
        if !enum_decl.type_parameters.is_empty() {
            // This is a generic enum template - we'll handle it when instantiated with concrete types
            // For now, just skip the LLVM generation but we might want to store the template
            Ok(())
        } else {
            // Non-generic enum - generate the LLVM type immediately
            self.type_system.define_enum_type(&enum_decl.name, &enum_decl.variants)?;
            Ok(())
        }
    }

    /// Generate code for struct literals
    fn generate_struct_literal(&mut self, struct_lit: &ast::StructLiteralExpression) -> Result<BasicValueEnum<'ctx>> {
        // Handle generic struct types (like Vec<T>)
        let concrete_struct_name = if struct_lit.struct_name.contains('<') {
            // This is a generic struct instantiation - for now, we'll need the monomorphized name
            // TODO: Add proper generic struct parsing support
            struct_lit.struct_name.clone()
        } else {
            struct_lit.struct_name.clone()
        };
        
        // Get the struct type from the type system
        let struct_type = self.type_system.get_struct_type(&concrete_struct_name)
            .ok_or_else(|| CodeGenError::UnknownType(format!("Struct type '{}' not found", concrete_struct_name)))?;

        // Allocate space for the struct on the stack
        let struct_ptr = self.builder.build_alloca(struct_type, "struct_literal")?;

        // Initialize fields
        for (field_index, field_init) in struct_lit.fields.iter().enumerate() {
            // Generate the value for this field
            let field_value = self.generate_expression(&field_init.value)?;

            // Get a pointer to the field in the struct
            let field_ptr = self.builder.build_struct_gep(
                struct_type,
                struct_ptr,
                field_index as u32,
                &format!("field_{}", field_init.field_name)
            )?;

            // Store the value in the field
            self.builder.build_store(field_ptr, field_value)?;
        }

        // Return a pointer to the struct (structs are passed by reference)
        Ok(struct_ptr.into())
    }

    /// Generate code for field access
    fn generate_field_access(&mut self, field_access: &ast::FieldAccessExpression) -> Result<BasicValueEnum<'ctx>> {
        // Generate the object expression (should be a pointer to struct)
        let struct_ptr = self.generate_expression(&field_access.object)?;
        
        // Ensure object is a pointer
        let struct_pointer = match struct_ptr {
            BasicValueEnum::PointerValue(pv) => pv,
            _ => {
                return Err(CodeGenError::CodeGeneration(
                    "Field access requires a pointer to struct".to_string()
                ));
            }
        };

        // For this implementation, we need to determine the struct type from context
        // This is a limitation - in a more complete implementation, we'd track type information with values
        // For now, we'll attempt to find a matching struct type by trying all registered struct types
        
        let mut found_struct_info: Option<(String, &crate::types::StructInfo)> = None;
        for (struct_name, struct_info) in &self.type_system.struct_types {
            if struct_info.field_names.contains(&field_access.field) {
                found_struct_info = Some((struct_name.clone(), struct_info));
                break;
            }
        }

        let (struct_name, struct_info) = found_struct_info.ok_or_else(|| {
            CodeGenError::CodeGeneration(format!(
                "Could not find struct type containing field '{}'", 
                field_access.field
            ))
        })?;

        // Get the field index
        let field_index = struct_info.field_names.iter()
            .position(|name| name == &field_access.field)
            .ok_or_else(|| CodeGenError::CodeGeneration(
                format!("Field '{}' not found in struct '{}'", field_access.field, struct_name)
            ))?;

        // Generate GEP instruction to get field pointer
        let field_ptr = self.builder.build_struct_gep(
            struct_info.llvm_type,
            struct_pointer,
            field_index as u32,
            &format!("field_{}", field_access.field)
        )?;

        // Load the value from the field pointer
        let field_type = &struct_info.field_types[field_index];
        let loaded_value = self.builder.build_load(
            *field_type,
            field_ptr,
            &format!("field_{}_value", field_access.field)
        )?;

        Ok(loaded_value)
    }

    /// Generate code for enum literals
    fn generate_enum_literal(&mut self, enum_lit: &ast::EnumLiteralExpression) -> Result<BasicValueEnum<'ctx>> {
        // Determine the concrete enum type name (handling generics)
        let concrete_enum_name = if let Some(type_args) = &enum_lit.type_arguments {
            // This is a generic enum instantiation like Option<Int>
            self.monomorphize_enum_type(&enum_lit.enum_name, type_args)?
        } else {
            // Non-generic enum
            enum_lit.enum_name.clone()
        };

        // Get the enum type from the type system
        let enum_info = self.type_system.get_enum_info(&concrete_enum_name)
            .ok_or_else(|| CodeGenError::UnknownType(format!("Enum type '{}' not found", concrete_enum_name)))?;
        
        // Find the variant
        let variant_info = enum_info.variants.iter()
            .find(|v| v.name == enum_lit.variant_name)
            .ok_or_else(|| CodeGenError::CodeGeneration(format!(
                "Variant '{}' not found in enum '{}'", 
                enum_lit.variant_name, 
                enum_lit.enum_name
            )))?;
        
        // Allocate space for the enum on the stack
        let enum_alloca = self.builder.build_alloca(
            enum_info.llvm_type,
            &format!("{}_{}", enum_lit.enum_name, enum_lit.variant_name)
        )?;
        
        // Store the tag
        let tag_ptr = self.builder.build_struct_gep(
            enum_info.llvm_type,
            enum_alloca,
            0,
            "enum_tag_ptr"
        )?;
        let tag_value = self.context.i64_type().const_int(variant_info.tag, false);
        self.builder.build_store(tag_ptr, tag_value)?;
        
        // Store variant data if any
        if let Some(args) = &enum_lit.arguments {
            if let Some(data_types) = &variant_info.data_types {
                if args.len() != data_types.len() {
                    return Err(CodeGenError::CodeGeneration(format!(
                        "Variant '{}' expects {} arguments but got {}",
                        variant_info.name, data_types.len(), args.len()
                    )));
                }
                
                // Get pointer to the data union
                let data_ptr = self.builder.build_struct_gep(
                    enum_info.llvm_type,
                    enum_alloca,
                    1,
                    "enum_data_ptr"
                )?;
                
                // Cast data pointer to array of i64
                let data_array_type = self.context.i64_type().array_type(args.len() as u32);
                let data_array_ptr = self.builder.build_pointer_cast(
                    data_ptr,
                    self.context.ptr_type(0.into()),
                    "data_array_ptr"
                )?;
                
                // Store each argument in the data array
                for (i, arg) in args.iter().enumerate() {
                    let arg_value = self.generate_expression(arg)?;
                    
                    // Get pointer to the i-th element
                    let elem_ptr = unsafe {
                        self.builder.build_gep(
                            data_array_type,
                            data_array_ptr,
                            &[
                                self.context.i32_type().const_zero(),
                                self.context.i32_type().const_int(i as u64, false)
                            ],
                            &format!("data_elem_{}", i)
                        )?
                    };
                    
                    // Convert and store the value (simplified - assumes all values fit in i64)
                    let int_value = match arg_value {
                        BasicValueEnum::IntValue(iv) => iv,
                        BasicValueEnum::FloatValue(fv) => {
                            self.builder.build_float_to_signed_int(fv, self.context.i64_type(), "float_to_int")?
                        }
                        BasicValueEnum::PointerValue(pv) => {
                            self.builder.build_ptr_to_int(pv, self.context.i64_type(), "ptr_to_int")?
                        }
                        _ => {
                            return Err(CodeGenError::CodeGeneration(
                                "Unsupported value type in enum variant data".to_string()
                            ));
                        }
                    };
                    
                    self.builder.build_store(elem_ptr, int_value)?;
                }
            }
        }
        
        // Return a pointer to the enum
        Ok(enum_alloca.into())
    }

    /// Generate a concrete enum type from a generic template (monomorphization)
    fn monomorphize_enum_type(&mut self, generic_name: &str, type_args: &[ast::Type]) -> Result<String> {
        // Generate a concrete type name (e.g., "Option_Int", "Result_String_Int")
        let concrete_name = self.generate_monomorphized_name(generic_name, type_args);
        
        // Check if this concrete type already exists
        if self.type_system.get_enum_info(&concrete_name).is_some() {
            return Ok(concrete_name);
        }
        
        // Auto-generate common generic types
        match generic_name {
            "Option" => {
                if type_args.len() != 1 {
                    return Err(CodeGenError::UnsupportedOperation(
                        "Option must have exactly one type parameter".to_string()
                    ));
                }
                self.create_option_enum(&concrete_name, &type_args[0])?;
                Ok(concrete_name)
            }
            "Result" => {
                if type_args.len() != 2 {
                    return Err(CodeGenError::UnsupportedOperation(
                        "Result must have exactly two type parameters".to_string()
                    ));
                }
                self.create_result_enum(&concrete_name, &type_args[0], &type_args[1])?;
                Ok(concrete_name)
            }
            "Vec" => {
                if type_args.len() != 1 {
                    return Err(CodeGenError::UnsupportedOperation(
                        "Vec must have exactly one type parameter".to_string()
                    ));
                }
                self.create_vec_struct(&concrete_name, &type_args[0])?;
                Ok(concrete_name)
            }
            _ => {
                // For other generics, we need templates (not implemented yet)
                Err(CodeGenError::UnsupportedOperation(
                    format!("Monomorphization not yet implemented for generic type '{}'", generic_name)
                ))
            }
        }
    }

    /// Create standard library functions
    fn create_stdlib_functions(&mut self) -> Result<()> {
        self.create_string_functions()?;
        self.create_math_functions()?;
        self.create_error_handling_functions()?;
        Ok(())
    }

    /// Create string manipulation functions
    fn create_string_functions(&mut self) -> Result<()> {
        let i8_ptr_type = self.context.ptr_type(0.into());
        let i64_type = self.context.i64_type();

        // string_concat(str1: *i8, str2: *i8) -> *i8
        let concat_type = i8_ptr_type.fn_type(&[
            i8_ptr_type.into(),
            i8_ptr_type.into(),
        ], false);
        let _concat_fn = self.module.add_function("string_concat", concat_type, None);

        // string_length(str: *i8) -> i64
        let length_type = i64_type.fn_type(&[i8_ptr_type.into()], false);
        let _length_fn = self.module.add_function("string_length", length_type, None);

        // string_substring(str: *i8, start: i64, len: i64) -> *i8
        let substring_type = i8_ptr_type.fn_type(&[
            i8_ptr_type.into(),
            i64_type.into(),
            i64_type.into(),
        ], false);
        let _substring_fn = self.module.add_function("string_substring", substring_type, None);

        // For now, these are just function declarations
        // Full implementation would require malloc and C string operations
        Ok(())
    }

    /// Create math functions using LLVM intrinsics
    fn create_math_functions(&mut self) -> Result<()> {
        let i64_type = self.context.i64_type();
        let f64_type = self.context.f64_type();

        // abs_int(x: i64) -> i64 - implemented with conditional logic
        let abs_int_type = i64_type.fn_type(&[i64_type.into()], false);
        let abs_int_fn = self.module.add_function("abs_int", abs_int_type, None);
        self.implement_abs_int_function(abs_int_fn)?;

        // abs_float(x: f64) -> f64 - use LLVM fabs intrinsic
        let abs_float_type = f64_type.fn_type(&[f64_type.into()], false);
        let abs_float_fn = self.module.add_function("abs_float", abs_float_type, None);
        self.implement_abs_float_function(abs_float_fn)?;

        // min_int(a: i64, b: i64) -> i64
        let min_int_type = i64_type.fn_type(&[i64_type.into(), i64_type.into()], false);
        let min_int_fn = self.module.add_function("min_int", min_int_type, None);
        self.implement_min_int_function(min_int_fn)?;

        // max_int(a: i64, b: i64) -> i64
        let max_int_type = i64_type.fn_type(&[i64_type.into(), i64_type.into()], false);
        let max_int_fn = self.module.add_function("max_int", max_int_type, None);
        self.implement_max_int_function(max_int_fn)?;

        // pow_float(base: f64, exp: f64) -> f64 - use LLVM pow intrinsic
        let pow_type = f64_type.fn_type(&[f64_type.into(), f64_type.into()], false);
        let pow_fn = self.module.add_function("pow_float", pow_type, None);
        self.implement_pow_float_function(pow_fn)?;
        Ok(())
    }

    /// Create error handling and runtime functions
    fn create_error_handling_functions(&mut self) -> Result<()> {
        let i8_ptr_type = self.context.ptr_type(0.into());
        let void_type = self.context.void_type();

        // panic(message: *i8) -> !
        // Note: LLVM doesn't have a "never" type, so we use void and mark as noreturn
        let panic_type = void_type.fn_type(&[i8_ptr_type.into()], false);
        let panic_fn = self.module.add_function("panic", panic_type, None);
        self.implement_panic_function(panic_fn)?;

        // abort() -> !
        let abort_type = void_type.fn_type(&[], false);
        let abort_fn = self.module.add_function("abort", abort_type, None);
        self.implement_abort_function(abort_fn)?;

        Ok(())
    }

    /// Implement panic function - prints message and aborts
    fn implement_panic_function(&mut self, function: FunctionValue<'ctx>) -> Result<()> {
        let entry_block = self.context.append_basic_block(function, "entry");
        self.builder.position_at_end(entry_block);

        // Get the message parameter
        let message = function.get_nth_param(0).unwrap().into_pointer_value();
        
        // Print panic message using printf
        let printf_fn = self.module.get_function("printf").unwrap();
        let panic_format = self.builder.build_global_string_ptr("PANIC: %s\n", "panic_fmt")?;
        self.builder.build_call(printf_fn, &[panic_format.as_pointer_value().into(), message.into()], "panic_print")?;
        
        // Call abort
        let abort_fn = self.module.get_function("abort").unwrap();
        self.builder.build_call(abort_fn, &[], "abort_call")?;
        
        // Unreachable instruction (function never returns)
        self.builder.build_unreachable()?;
        Ok(())
    }

    /// Implement abort function - terminates program immediately
    fn implement_abort_function(&mut self, function: FunctionValue<'ctx>) -> Result<()> {
        let entry_block = self.context.append_basic_block(function, "entry");
        self.builder.position_at_end(entry_block);

        // Print abort message
        let printf_fn = self.module.get_function("printf").unwrap();
        let abort_msg = self.builder.build_global_string_ptr("Program aborted\n", "abort_msg")?;
        self.builder.build_call(printf_fn, &[abort_msg.as_pointer_value().into()], "abort_print")?;
        
        // Exit with code 1
        let exit_code = self.context.i32_type().const_int(1, false);
        
        // Declare exit function from C stdlib
        let i32_type = self.context.i32_type();
        let exit_type = self.context.void_type().fn_type(&[i32_type.into()], false);
        let exit_fn = self.module.add_function("exit", exit_type, None);
        
        self.builder.build_call(exit_fn, &[exit_code.into()], "exit_call")?;
        self.builder.build_unreachable()?;
        Ok(())
    }

    /// Implement abs_int function: return x if x >= 0, else return -x
    fn implement_abs_int_function(&mut self, function: FunctionValue<'ctx>) -> Result<()> {
        let entry_block = self.context.append_basic_block(function, "entry");
        let negative_block = self.context.append_basic_block(function, "negative");
        let positive_block = self.context.append_basic_block(function, "positive");
        let end_block = self.context.append_basic_block(function, "end");

        // Position builder at entry
        self.builder.position_at_end(entry_block);

        // Get the parameter
        let x = function.get_nth_param(0).unwrap().into_int_value();
        
        // Check if x < 0
        let zero = self.context.i64_type().const_zero();
        let is_negative = self.builder.build_int_compare(
            inkwell::IntPredicate::SLT, x, zero, "is_negative"
        )?;

        // Branch based on sign
        self.builder.build_conditional_branch(is_negative, negative_block, positive_block)?;

        // Negative case: return -x
        self.builder.position_at_end(negative_block);
        let negated = self.builder.build_int_neg(x, "negated")?;
        self.builder.build_unconditional_branch(end_block)?;

        // Positive case: return x
        self.builder.position_at_end(positive_block);
        self.builder.build_unconditional_branch(end_block)?;

        // End block: phi node to select result
        self.builder.position_at_end(end_block);
        let phi = self.builder.build_phi(self.context.i64_type(), "abs_result")?;
        phi.add_incoming(&[(&negated, negative_block), (&x, positive_block)]);
        self.builder.build_return(Some(&phi.as_basic_value()))?;

        Ok(())
    }

    /// Implement abs_float using LLVM fabs intrinsic
    fn implement_abs_float_function(&mut self, function: FunctionValue<'ctx>) -> Result<()> {
        let entry_block = self.context.append_basic_block(function, "entry");
        self.builder.position_at_end(entry_block);

        let x = function.get_nth_param(0).unwrap().into_float_value();
        
        // Use LLVM's fabs intrinsic
        let fabs_intrinsic = inkwell::intrinsics::Intrinsic::find("llvm.fabs").unwrap();
        let fabs_fn = fabs_intrinsic.get_declaration(&self.module, &[self.context.f64_type().into()]).unwrap();
        
        let result = self.builder.build_call(fabs_fn, &[x.into()], "fabs_result")?;
        let abs_value = result.try_as_basic_value().left().unwrap();
        
        self.builder.build_return(Some(&abs_value))?;
        Ok(())
    }

    /// Implement min_int function: return a if a <= b, else return b
    fn implement_min_int_function(&mut self, function: FunctionValue<'ctx>) -> Result<()> {
        let entry_block = self.context.append_basic_block(function, "entry");
        self.builder.position_at_end(entry_block);

        let a = function.get_nth_param(0).unwrap().into_int_value();
        let b = function.get_nth_param(1).unwrap().into_int_value();
        
        // Compare a <= b
        let a_le_b = self.builder.build_int_compare(
            inkwell::IntPredicate::SLE, a, b, "a_le_b"
        )?;

        // Select minimum value
        let min_val = self.builder.build_select(a_le_b, a, b, "min")?;
        self.builder.build_return(Some(&min_val))?;
        Ok(())
    }

    /// Implement max_int function: return a if a >= b, else return b
    fn implement_max_int_function(&mut self, function: FunctionValue<'ctx>) -> Result<()> {
        let entry_block = self.context.append_basic_block(function, "entry");
        self.builder.position_at_end(entry_block);

        let a = function.get_nth_param(0).unwrap().into_int_value();
        let b = function.get_nth_param(1).unwrap().into_int_value();
        
        // Compare a >= b
        let a_ge_b = self.builder.build_int_compare(
            inkwell::IntPredicate::SGE, a, b, "a_ge_b"
        )?;

        // Select maximum value
        let max_val = self.builder.build_select(a_ge_b, a, b, "max")?;
        self.builder.build_return(Some(&max_val))?;
        Ok(())
    }

    /// Implement pow_float using LLVM pow intrinsic
    fn implement_pow_float_function(&mut self, function: FunctionValue<'ctx>) -> Result<()> {
        let entry_block = self.context.append_basic_block(function, "entry");
        self.builder.position_at_end(entry_block);

        let base = function.get_nth_param(0).unwrap().into_float_value();
        let exp = function.get_nth_param(1).unwrap().into_float_value();
        
        // Use LLVM's pow intrinsic
        let pow_intrinsic = inkwell::intrinsics::Intrinsic::find("llvm.pow").unwrap();
        let pow_fn = pow_intrinsic.get_declaration(&self.module, &[self.context.f64_type().into()]).unwrap();
        
        let result = self.builder.build_call(pow_fn, &[base.into(), exp.into()], "pow_result")?;
        let pow_value = result.try_as_basic_value().left().unwrap();
        
        self.builder.build_return(Some(&pow_value))?;
        Ok(())
    }

    /// Generate code for try expression (? operator)
    fn generate_try_expression(&mut self, try_expr: &ast::TryExpression) -> Result<BasicValueEnum<'ctx>> {
        // Generate the Result<T, E> expression
        let result_value = self.generate_expression(&try_expr.expression)?;
        
        // Assume result_value is a pointer to a Result enum (tagged union)
        // The Result enum has structure: { i64 tag, [data_array] data }
        // tag = 0 for Ok(value), tag = 1 for Err(error)
        
        // Load the tag to check if it's Ok or Err
        let result_ptr = result_value.into_pointer_value();
        let tag_ptr = self.builder.build_struct_gep(
            self.type_system.get_enum_type("Result_Int_String").unwrap(), // This should be dynamically determined
            result_ptr,
            0,
            "result_tag_ptr"
        )?;
        let tag_value = self.builder.build_load(
            self.context.i64_type(),
            tag_ptr,
            "result_tag"
        )?;
        
        // Compare tag with 0 (Ok variant)
        let is_ok = self.builder.build_int_compare(
            inkwell::IntPredicate::EQ,
            tag_value.into_int_value(),
            self.context.i64_type().const_zero(),
            "is_ok"
        )?;
        
        // Create basic blocks for Ok and Err cases
        let current_function = self.builder.get_insert_block().unwrap().get_parent().unwrap();
        let ok_block = self.context.append_basic_block(current_function, "try_ok");
        let err_block = self.context.append_basic_block(current_function, "try_err");
        let after_block = self.context.append_basic_block(current_function, "try_after");
        
        // Branch based on tag
        self.builder.build_conditional_branch(is_ok, ok_block, err_block)?;
        
        // Handle Ok case: extract the value
        self.builder.position_at_end(ok_block);
        let data_ptr = self.builder.build_struct_gep(
            self.type_system.get_enum_type("Result_Int_String").unwrap(), // This should be dynamically determined
            result_ptr,
            1,
            "result_data_ptr"
        )?;
        // For simplicity, assume the Ok value is the first element in the data array
        let value_ptr = unsafe {
            self.builder.build_gep(
                self.context.i64_type().array_type(2), // Assuming 2-element array for Result<T,E>
                data_ptr,
                &[
                    self.context.i32_type().const_zero(),
                    self.context.i32_type().const_zero(),
                ],
                "ok_value_ptr"
            )?
        };
        let ok_value = self.builder.build_load(
            self.context.i64_type(),
            value_ptr,
            "ok_value"
        )?;
        self.builder.build_unconditional_branch(after_block)?;
        
        // Handle Err case: early return with error
        self.builder.position_at_end(err_block);
        // For now, we'll create a new Result with the same error
        // In a real implementation, this would propagate the error up the call stack
        let err_value_ptr = unsafe {
            self.builder.build_gep(
                self.context.i64_type().array_type(2),
                data_ptr,
                &[
                    self.context.i32_type().const_zero(),
                    self.context.i32_type().const_int(1, false), // Second element is the error
                ],
                "err_value_ptr"
            )?
        };
        let err_value = self.builder.build_load(
            self.context.i64_type(),
            err_value_ptr,
            "err_value"
        )?;
        
        // Create a new Result with Err variant and return it
        // For simplicity, we'll just return the error value for now
        // TODO: Implement proper early return mechanism
        self.builder.build_return(Some(&err_value))?;
        
        // Continue in after block with the Ok value
        self.builder.position_at_end(after_block);
        Ok(ok_value)
    }

    /// Generate a monomorphized type name from generic name and type arguments
    fn generate_monomorphized_name(&self, generic_name: &str, type_args: &[ast::Type]) -> String {
        let arg_names: Vec<String> = type_args.iter()
            .map(|arg| self.type_system.type_name_for_monomorphization(arg))
            .collect();
        format!("{}_{}", generic_name, arg_names.join("_"))
    }

    /// Create a concrete Option<T> enum type
    fn create_option_enum(&mut self, concrete_name: &str, value_type: &ast::Type) -> Result<()> {
        // Create enum variants: Some(T) and None
        let dummy_position = Position::new(0, 0);
        let location = Location::new(dummy_position, dummy_position);
        
        let some_variant = ast::EnumVariant {
            name: "Some".to_string(),
            data: Some(vec![value_type.clone()]),
            location: location.clone(),
        };
        
        let none_variant = ast::EnumVariant {
            name: "None".to_string(),
            data: None,
            location: location.clone(),
        };
        
        let variants = vec![some_variant, none_variant];
        
        // Define the concrete enum type in the type system
        self.type_system.define_enum_type(concrete_name, &variants)?;
        
        Ok(())
    }

    /// Create a concrete Result<T, E> enum type
    fn create_result_enum(&mut self, concrete_name: &str, value_type: &ast::Type, error_type: &ast::Type) -> Result<()> {
        // Create enum variants: Ok(T) and Err(E)
        let dummy_position = Position::new(0, 0);
        let location = Location::new(dummy_position, dummy_position);
        
        let ok_variant = ast::EnumVariant {
            name: "Ok".to_string(),
            data: Some(vec![value_type.clone()]),
            location: location.clone(),
        };
        
        let err_variant = ast::EnumVariant {
            name: "Err".to_string(),
            data: Some(vec![error_type.clone()]),
            location: location.clone(),
        };
        
        let variants = vec![ok_variant, err_variant];
        
        // Define the concrete enum type in the type system
        self.type_system.define_enum_type(concrete_name, &variants)?;
        
        Ok(())
    }

    /// Create a concrete Vec<T> struct type
    fn create_vec_struct(&mut self, concrete_name: &str, element_type: &ast::Type) -> Result<()> {
        // Convert element type to LLVM type
        let _element_llvm_type = self.type_system.convert_type(element_type)?;
        
        // Vec<T> structure: { T* data, i64 len, i64 capacity }
        let field_names = vec![
            "data".to_string(),
            "len".to_string(), 
            "capacity".to_string(),
        ];
        
        let field_types = vec![
            self.context.ptr_type(0.into()).into(), // T* data
            self.context.i64_type().into(),         // i64 len
            self.context.i64_type().into(),         // i64 capacity
        ];
        
        // Define the struct type in the type system
        self.type_system.define_struct_type(concrete_name, field_names, field_types)?;
        
        // Create builtin functions for this Vec<T> type
        self.create_vec_builtins(concrete_name, element_type)?;
        
        Ok(())
    }

    /// Create builtin functions for Vec<T> operations
    fn create_vec_builtins(&mut self, concrete_name: &str, element_type: &ast::Type) -> Result<()> {
        let element_llvm_type = self.type_system.convert_type(element_type)?;
        let vec_struct_type = self.type_system.get_struct_type(concrete_name).unwrap();
        
        // Create vec_new function: () -> Vec<T>
        let vec_new_type = self.context.void_type().fn_type(&[], false);
        let vec_new_fn = self.module.add_function(&format!("{}_new", concrete_name), vec_new_type, None);
        
        // Create vec_push function: (Vec<T>*, T) -> void
        let vec_push_type = self.context.void_type().fn_type(&[
            self.context.ptr_type(0.into()).into(), // Vec<T>*
            element_llvm_type.into(),                // T
        ], false);
        let vec_push_fn = self.module.add_function(&format!("{}_push", concrete_name), vec_push_type, None);
        
        // Create vec_get function: (Vec<T>*, i64) -> T*
        let vec_get_type = self.context.ptr_type(0.into()).fn_type(&[
            self.context.ptr_type(0.into()).into(), // Vec<T>*
            self.context.i64_type().into(),         // i64 index
        ], false);
        let vec_get_fn = self.module.add_function(&format!("{}_get", concrete_name), vec_get_type, None);
        
        // Create vec_len function: (Vec<T>*) -> i64
        let vec_len_type = self.context.i64_type().fn_type(&[
            self.context.ptr_type(0.into()).into(), // Vec<T>*
        ], false);
        let vec_len_fn = self.module.add_function(&format!("{}_len", concrete_name), vec_len_type, None);
        
        // For now, these are just function declarations
        // Full implementation would require malloc/free and proper memory management
        
        Ok(())
    }

    /// Generate code for match statements
    fn generate_match_statement(&mut self, match_stmt: &ast::MatchStatement) -> Result<()> {
        // Generate the value to match on
        let match_value = self.generate_expression(&match_stmt.value)?;
        
        // For enum matching, we need to extract the tag
        let tag_value = match match_value {
            BasicValueEnum::PointerValue(enum_ptr) => {
                // Load the tag from the enum
                let tag_ptr = self.builder.build_struct_gep(
                    self.context.struct_type(&[
                        self.context.i64_type().into(),
                        self.context.i64_type().array_type(1).into()
                    ], false),
                    enum_ptr,
                    0,
                    "tag_ptr"
                )?;
                self.builder.build_load(
                    self.context.i64_type(),
                    tag_ptr,
                    "tag_value"
                )?
            }
            BasicValueEnum::IntValue(iv) => iv.into(),
            _ => {
                return Err(CodeGenError::CodeGeneration(
                    "Match statement requires enum or integer value".to_string()
                ));
            }
        };
        
        // Create basic blocks for each arm and the end
        let current_fn = self.builder.get_insert_block().unwrap().get_parent().unwrap();
        let mut arm_blocks = Vec::new();
        let end_block = self.context.append_basic_block(current_fn, "match_end");
        
        // Create blocks for each arm
        for (i, _) in match_stmt.arms.iter().enumerate() {
            arm_blocks.push(self.context.append_basic_block(current_fn, &format!("match_arm_{}", i)));
        }
        
        // Build cases for switch
        let mut cases = Vec::new();
        for (i, _arm) in match_stmt.arms.iter().enumerate() {
            let case_value = self.context.i64_type().const_int(i as u64, false);
            cases.push((case_value, arm_blocks[i]));
        }
        
        // Generate switch instruction
        self.builder.build_switch(
            tag_value.into_int_value(),
            end_block,
            &cases
        )?;
        
        // Generate code for each arm
        for (i, arm) in match_stmt.arms.iter().enumerate() {
            // Generate arm code
            self.builder.position_at_end(arm_blocks[i]);
            
            // TODO: Bind pattern variables in environment
            
            // Generate arm expression
            self.generate_expression(&arm.expression)?;
            
            // Jump to end
            self.builder.build_unconditional_branch(end_block)?;
        }
        
        // Continue at end block
        self.builder.position_at_end(end_block);
        Ok(())
    }

    /// Generate code for match expressions
    fn generate_match_expression(&mut self, match_expr: &ast::MatchExpression) -> Result<BasicValueEnum<'ctx>> {
        // Similar to match statement but collects the result value
        // Generate the value to match on
        let match_value = self.generate_expression(&match_expr.value)?;
        
        // Create a phi node for the result
        let current_fn = self.builder.get_insert_block().unwrap().get_parent().unwrap();
        let end_block = self.context.append_basic_block(current_fn, "match_expr_end");
        
        // For now, return a placeholder value
        // Full implementation would handle all arms and create a phi node
        Err(CodeGenError::UnsupportedOperation(
            "Match expressions not fully implemented yet".to_string()
        ))
    }

    fn run_optimization_passes(&self) -> Result<()> {
        // Initialize all targets for the current platform.
        Target::initialize_native(&InitializationConfig::default()).map_err(|e| {
            CodeGenError::CodeGeneration(format!("Failed to initialize native target: {:?}", e))
        })?;

        let target_triple = TargetMachine::get_default_triple();
        let target = Target::from_triple(&target_triple).map_err(|e| {
            CodeGenError::CodeGeneration(format!("Failed to create target from triple: {:?}", e))
        })?;

        // Create a target machine for the host
        // TODO: Allow specifying target CPU and features, or use a more generic target for broader compatibility if needed.
        let target_machine = target
            .create_target_machine(
                &target_triple,
                "generic", // Use "native" for host CPU, or "generic" for general compatibility
                "", // CPU features. Use "+avx2" for example, or an empty string for no specific features.
                OptimizationLevel::Default, // This opt level is for the TM, passes are specified below
                RelocMode::Default,         // Or RelocMode::PIC for position-independent code
                CodeModel::Default,         // Or CodeModel::Small, Medium, Large
            )
            .ok_or_else(|| {
                CodeGenError::CodeGeneration("Failed to create target machine".to_string())
            })?;

        // Define the sequence of passes to run.
        // These are common and generally safe starting passes.
        let passes = [
            "instcombine", // Combine redundant instructions
            "reassociate", // Reassociate expressions
            "gvn",         // Global Value Numbering
            "simplifycfg", // Simplify control-flow graph
            "mem2reg",     // Promote memory to registers (SROA)
            // Add more passes as needed, e.g.:
            // "early-cse",         // Early Common Subexpression Elimination
            // "loop-simplify",     // Simplify loops
            // "loop-unroll",       // Unroll loops
            // "sccp",              // Sparse Conditional Constant Propagation
            // "adce",              // Aggressive Dead Code Elimination
            // "dce"                // Dead Code Elimination
        ]
            .join(",");

        let pass_builder_options = PassBuilderOptions::create();
        // Example: Set optimization level for the pass pipeline if desired
        // pass_builder_options.set_optimization_level(OptimizationLevel::Aggressive);
        // pass_builder_options.set_verify_each(true); // For debugging passes

        self.module
            .run_passes(&passes, &target_machine, pass_builder_options)
            .map_err(|e_str| {
                CodeGenError::CodeGeneration(format!(
                    "Failed to run optimization passes: {}",
                    e_str
                ))
            })?;

        Ok(())
    }
}
