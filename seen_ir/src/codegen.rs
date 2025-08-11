use std::collections::HashMap;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::values::{BasicValueEnum, FunctionValue, PointerValue};
use inkwell::types::{BasicTypeEnum, BasicType};
use inkwell::OptimizationLevel;
use inkwell::passes::PassBuilderOptions;
use inkwell::targets::{CodeModel, InitializationConfig, RelocMode, Target, TargetMachine};
use inkwell::Either;

use seen_parser::ast::{self, Expression, Statement, Declaration, Program};
use crate::error::{CodeGenError, Result};
use crate::mapping::{map_binary_operator, map_unary_operator};
use crate::types::TypeSystem;

/// Environment for storing variables during code generation
struct Environment<'ctx> {
    /// Variables in the current scope
    variables: HashMap<String, PointerValue<'ctx>>,
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
    fn define(&mut self, name: &str, value: PointerValue<'ctx>) {
        self.variables.insert(name.to_string(), value);
    }

    /// Get a variable from any scope
    fn get(&self, name: &str) -> Option<PointerValue<'ctx>> {
        if let Some(value) = self.variables.get(name) {
            Some(*value)
        } else if let Some(parent) = &self.parent {
            parent.get(name)
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
            Declaration::Function(func_decl) => { self.generate_function(func_decl)?; },
            Declaration::Variable(var_decl) => { self.generate_global_variable(var_decl)?; },
            Declaration::Struct(struct_decl) => {
                // Create struct type with fields
                let field_types: Vec<BasicTypeEnum<'ctx>> = struct_decl
                    .fields
                    .iter()
                    .map(|field| self.type_system.convert_type(&field.field_type))
                    .collect::<Result<Vec<_>>>()?;

                let _struct_type = self.context.struct_type(&field_types, false);
                
                // Register the struct type for later use
                // We create an opaque struct first and then set its body
                let named_struct = self.context.opaque_struct_type(&struct_decl.name);
                named_struct.set_body(&field_types, false);
            },
        }

        Ok(())
    }

    /// Generate code for a function declaration
    fn generate_function(&mut self, func_decl: &ast::FunctionDeclaration) -> Result<FunctionValue<'ctx>> {
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
            Some(ret_type) => {
                ret_type.fn_type(&param_types, false)
            },
            None => {
                self.type_system.void_type().fn_type(&param_types, false)
            },
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
            self.environment.define(&param.name, param_ptr);
        }

        // Generate code for the function body
        self.generate_block(&func_decl.body)?;

        // Verify the function
        if function.verify(true) {
            Ok(function)
        } else {
            // Remove the invalid function
            unsafe {
                function.delete();
            }
            Err(CodeGenError::CodeGeneration("Invalid function generated".to_string()))
        }
    }

    /// Generate code for a global variable
    fn generate_global_variable(&mut self, var_decl: &ast::VariableDeclaration) -> Result<PointerValue<'ctx>> {
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
            BasicValueEnum::ScalableVectorValue(_) => todo!("Handle ScalableVectorValue initialization"),
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
            },
            Statement::Block(block) => self.generate_block(block),
            Statement::Return(ret_stmt) => {
                if let Some(expr) = &ret_stmt.value {
                    let return_value = self.generate_expression(expr)?;
                    self.builder.build_return(Some(&return_value))?;
                } else {
                    self.builder.build_return(None)?;
                }
                Ok(())
            },
            Statement::If(if_stmt) => {
                let condition_val = self.generate_expression(&if_stmt.condition)?;
                let condition_val = self.builder.build_int_compare(
                    inkwell::IntPredicate::NE,
                    condition_val.into_int_value(),
                    self.type_system.bool_type().const_int(0, false),
                    "ifcond",
                )?;

                let function = self.builder.get_insert_block().unwrap().get_parent().unwrap();

                let then_block = self.context.append_basic_block(function, "then");
                let else_block = self.context.append_basic_block(function, "else");
                let merge_block = self.context.append_basic_block(function, "ifcont");

                self.builder.build_conditional_branch(condition_val, then_block, else_block)?;

                self.builder.position_at_end(then_block);
                self.generate_statement(&if_stmt.then_branch)?;

                // Branch to merge block if not terminated
                if self.builder.get_insert_block().unwrap().get_terminator().is_none() {
                    self.builder.build_unconditional_branch(merge_block)?;
                }

                self.builder.position_at_end(else_block);
                if let Some(else_branch) = &if_stmt.else_branch {
                    self.generate_statement(else_branch)?;
                }

                // Branch to merge block if not terminated
                if self.builder.get_insert_block().unwrap().get_terminator().is_none() {
                    self.builder.build_unconditional_branch(merge_block)?;
                }

                self.builder.position_at_end(merge_block);

                Ok(())
            },
            Statement::While(while_stmt) => {
                let function = self.builder.get_insert_block().unwrap().get_parent().unwrap();

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

                self.builder.build_conditional_branch(condition_val, loop_block, merge_block)?;

                self.builder.position_at_end(loop_block);
                self.generate_statement(&while_stmt.body)?;

                // Branch back to the condition block if not terminated
                if self.builder.get_insert_block().unwrap().get_terminator().is_none() {
                    self.builder.build_unconditional_branch(cond_block)?;
                }

                self.builder.position_at_end(merge_block);

                Ok(())
            },
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
            },
            Statement::DeclarationStatement(decl) => self.generate_declaration(decl),
            Statement::For(for_stmt) => {
                // Generate for-in loop: for variable in iterable { body }
                let function = self.builder.get_insert_block().unwrap().get_parent().unwrap();

                // For now, generate a simple loop over the iterable
                let loop_block = self.context.append_basic_block(function, "for.body");
                let end_block = self.context.append_basic_block(function, "for.end");

                // Generate the iterable expression (but don't use it for now - just a simple implementation)
                self.generate_expression(&for_stmt.iterable)?;
                
                // Jump to loop body (simplified for MVP)
                self.builder.build_unconditional_branch(loop_block)?;
                
                // Generate loop body
                self.builder.position_at_end(loop_block);
                self.generate_statement(&for_stmt.body)?;
                
                // Jump to end (simplified - real implementation would iterate)
                if self.builder.get_insert_block().unwrap().get_terminator().is_none() {
                    self.builder.build_unconditional_branch(end_block)?;
                }

                // Position at end block for next statement
                self.builder.position_at_end(end_block);

                Ok(())
            },
        }
    }

    /// Generate code for an expression
    fn generate_expression(&mut self, expression: &Expression) -> Result<BasicValueEnum<'ctx>> {
        match expression {
            Expression::Identifier(ident_expr) => {
                let var_ptr = self.environment.get(&ident_expr.name).ok_or_else(|| {
                    CodeGenError::UndefinedSymbol(format!("Undefined variable: {}", ident_expr.name))
                })?;
                // TODO: Retrieve actual type of var_ptr for build_load.
                let loaded_value = self.builder.build_load(self.context.i64_type(), var_ptr, &ident_expr.name)?;
                Ok(loaded_value)
            }
            Expression::Literal(lit_expr) => {
                match lit_expr {
                    ast::LiteralExpression::Number(num_lit) => {
                        if num_lit.is_float {
                            let float_val = num_lit.value.parse::<f64>()
                                .map_err(|_| CodeGenError::CodeGeneration(format!("Invalid float literal: {}", num_lit.value)))?;
                            Ok(self.context.f64_type().const_float(float_val).into())
                        } else {
                            let int_val = num_lit.value.parse::<i64>()
                                .map_err(|_| CodeGenError::CodeGeneration(format!("Invalid integer literal: {}", num_lit.value)))?;
                            Ok(self.context.i64_type().const_int(int_val as u64, true).into())
                        }
                    }
                    ast::LiteralExpression::String(str_lit) => {
                        let string_value = self.builder.build_global_string_ptr(&str_lit.value, ".str")?;
                        Ok(string_value.as_pointer_value().into())
                    }
                    ast::LiteralExpression::Boolean(bool_lit) => {
                        Ok(self.context.bool_type().const_int(bool_lit.value as u64, false).into())
                    }
                    ast::LiteralExpression::Null(_null_lit) => {
                        Ok(self.context.ptr_type(0.into()).const_null().into())
                    }
                }
            }
            Expression::Unary(unary_expr) => {
                let operand_val = self.generate_expression(&unary_expr.operand)?;
                let value_result = map_unary_operator(&unary_expr.operator, operand_val, &self.builder, |msg| {
                    CodeGenError::UnsupportedOperation(msg)
                });
                Ok(value_result?)
            }
            Expression::Binary(binary_expr) => {
                let left_val = self.generate_expression(&binary_expr.left)?;
                let right_val = self.generate_expression(&binary_expr.right)?;
                let value_result = map_binary_operator(&binary_expr.operator, left_val, right_val, &self.builder, |msg| {
                    CodeGenError::UnsupportedOperation(msg)
                });
                Ok(value_result?)
            }
            Expression::Assignment(assign_expr) => { // assign_expr is AssignmentExpression
                let var_ptr = self.environment.get(&assign_expr.name).ok_or_else(|| {
                    CodeGenError::UndefinedSymbol(format!("Undefined variable for assignment: {}", assign_expr.name))
                })?;
                let val_to_assign = self.generate_expression(&assign_expr.value)?;
                self.builder.build_store(var_ptr, val_to_assign)?;
                Ok(val_to_assign)
            }
            Expression::Call(call_expr) => { // call_expr is CallExpression
                let function = self.module.get_function(&call_expr.callee).ok_or_else(|| {
                    CodeGenError::UndefinedSymbol(format!("Function '{}' not found", call_expr.callee))
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
            Expression::StructLiteral(struct_literal) => {
                // Create struct instance and initialize all fields
                let struct_name = &struct_literal.struct_name;
                let struct_type = self.context.opaque_struct_type(struct_name);
                
                // Allocate struct on stack
                let struct_alloca = self.create_entry_block_alloca(
                    &format!("{}_struct", struct_name), 
                    struct_type.into()
                );
                
                // Initialize each field
                for (field_index, field_init) in struct_literal.fields.iter().enumerate() {
                    let field_value = self.generate_expression(&field_init.value)?;
                    let field_ptr = self.builder.build_struct_gep(
                        struct_type,
                        struct_alloca,
                        field_index as u32,
                        &format!("{}_field", field_init.field_name)
                    )?;
                    self.builder.build_store(field_ptr, field_value)?;
                }
                
                // Load the complete struct
                let loaded_struct = self.builder.build_load(
                    struct_type,
                    struct_alloca,
                    &format!("{}_value", struct_name)
                )?;
                Ok(loaded_struct)
            }
            Expression::FieldAccess(field_access) => {
                // Access field from struct
                let object_value = self.generate_expression(&field_access.object)?;
                let object_ptr = if object_value.is_pointer_value() {
                    object_value.into_pointer_value()
                } else {
                    // If not a pointer, create temporary storage
                    let temp_alloca = self.create_entry_block_alloca(
                        "temp_struct", 
                        object_value.get_type()
                    );
                    self.builder.build_store(temp_alloca, object_value)?;
                    temp_alloca
                };
                
                // For struct access, we need to know the struct type
                // In a complete implementation, we'd maintain type information
                // For now, we'll create a dummy struct type
                let struct_type = self.context.struct_type(&[self.context.i64_type().into()], false);
                
                // Access the field (assuming field index 0 for now - real implementation would map field names to indices)
                let field_ptr = self.builder.build_struct_gep(
                    struct_type,
                    object_ptr,
                    0, // Field index - real implementation would look up by field name
                    &format!("{}_field_access", field_access.field)
                )?;
                
                let field_value = self.builder.build_load(
                    struct_type.get_field_type_at_index(0).unwrap(),
                    field_ptr,
                    &format!("{}_field_value", field_access.field)
                )?;
                
                Ok(field_value)
            }
            Expression::ArrayLiteral(array_literal) => {
                // Create array with all elements
                if array_literal.elements.is_empty() {
                    return Ok(self.context.i32_type().const_zero().into());
                }
                
                // Generate first element to determine array type
                let first_element = self.generate_expression(&array_literal.elements[0])?;
                let element_type = first_element.get_type();
                let array_type = element_type.array_type(array_literal.elements.len() as u32);
                
                // Allocate array on stack
                let array_alloca = self.create_entry_block_alloca("array", array_type.into());
                
                // Initialize each element
                for (index, element_expr) in array_literal.elements.iter().enumerate() {
                    let element_value = self.generate_expression(element_expr)?;
                    let element_ptr = unsafe {
                        self.builder.build_gep(
                            array_type,
                            array_alloca,
                            &[
                                self.context.i32_type().const_int(0, false),
                                self.context.i32_type().const_int(index as u64, false)
                            ],
                            &format!("array_element_{}", index)
                        )?
                    };
                    self.builder.build_store(element_ptr, element_value)?;
                }
                
                // Return pointer to array
                Ok(array_alloca.into())
            }
            Expression::Index(index_expr) => {
                // Index into array
                let array_value = self.generate_expression(&index_expr.object)?;
                let index_value = self.generate_expression(&index_expr.index)?;
                
                let array_ptr = if array_value.is_pointer_value() {
                    array_value.into_pointer_value()
                } else {
                    return Err(CodeGenError::InvalidASTNode {
                        location: "index expression".to_string(),
                        message: "Cannot index non-array type".to_string(),
                    });
                };
                
                // For array indexing, we need to know the array type
                // In a complete implementation, we'd maintain type information
                // For now, we'll assume i64 elements
                let element_type = self.context.i64_type();
                
                // Build GEP to access array element
                // For a complete implementation, we'd get the actual array type
                let array_type = self.context.i64_type().array_type(10);
                let element_ptr = unsafe {
                    self.builder.build_gep(
                        array_type,
                        array_ptr,
                        &[
                            self.context.i32_type().const_int(0, false),
                            index_value.into_int_value()
                        ],
                        "array_index"
                    )?
                };
                
                // Load the element value
                let element_value = self.builder.build_load(element_type, element_ptr, "indexed_value")?;
                Ok(element_value)
            }
            Expression::Range(range_expr) => {
                // Create range struct with start and end values
                let start_value = self.generate_expression(&range_expr.start)?;
                let end_value = self.generate_expression(&range_expr.end)?;
                
                // Create range struct type (start: i64, end: i64)
                let i64_type = self.context.i64_type();
                let range_struct_type = self.context.struct_type(&[i64_type.into(), i64_type.into()], false);
                
                // Allocate range struct
                let range_alloca = self.create_entry_block_alloca("range", range_struct_type.into());
                
                // Set start field
                let start_ptr = self.builder.build_struct_gep(
                    range_struct_type,
                    range_alloca,
                    0,
                    "range_start_ptr"
                )?;
                self.builder.build_store(start_ptr, start_value)?;
                
                // Set end field  
                let end_ptr = self.builder.build_struct_gep(
                    range_struct_type,
                    range_alloca,
                    1,
                    "range_end_ptr"
                )?;
                self.builder.build_store(end_ptr, end_value)?;
                
                // Load complete range struct
                let range_value = self.builder.build_load(
                    range_struct_type,
                    range_alloca,
                    "range_value"
                )?;
                
                Ok(range_value)
            }
        }
    }

    /// Create an alloca instruction at the entry point of the function
    fn create_entry_block_alloca(&self, name: &str, ty: BasicTypeEnum<'ctx>) -> PointerValue<'ctx> {
        let builder = self.context.create_builder();
        
        match &self.builder.get_insert_block() {
            Some(block) => {
                match block.get_parent() {
                    Some(function) => {
                        let entry = function.get_first_basic_block().unwrap();
                        
                        match entry.get_first_instruction() {
                            Some(first_instr) => {
                                builder.position_before(&first_instr);
                            },
                            None => {
                                builder.position_at_end(entry);
                            },
                        }
                    },
                    None => {
                        panic!("Block has no parent function");
                    },
                }
            },
            None => {
                panic!("No current block to insert into");
            },
        }
        
        builder.build_alloca(ty, name).expect("Failed to build alloca")
    }

    /// Declare the printf function for use in print statements
    fn declare_printf(&self) -> FunctionValue<'ctx> {
        if let Some(printf) = self.module.get_function("printf") {
            return printf;
        }

        let printf_type = self.context.i32_type().fn_type(
            &[self.context.ptr_type(inkwell::AddressSpace::default()).into()], // Format string (char*)
            true, // is_var_args
        );

        self.module.add_function("printf", printf_type, None)
    }

    /// Build a call to printf with the appropriate format string for multiple values
    fn build_printf_call(&self, values: &[BasicValueEnum<'ctx>]) -> Result<()> {
        let printf = self.declare_printf();
        
        if values.is_empty() {
            // Optionally, print just a newline or handle as an error/do nothing
            let format_string_ptr = self.builder.build_global_string_ptr("\n", "empty_print_format")?;
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
                },
                BasicValueEnum::FloatValue(_) => {
                    format_string.push_str("%f"); // %f for double, %lf is for scanf
                },
                BasicValueEnum::PointerValue(_pv) => {
                    // Assuming pointer is a C string (char*)
                    // A more robust system would check the actual pointed-to type.
                    // Especially if you have pointers to other things like structs/arrays.
                    format_string.push_str("%s");
                },
                _ => {
                    return Err(CodeGenError::UnsupportedOperation(
                        format!("Cannot print value of type {:?} at argument {}", value.get_type(), i)
                    ));
                },
            }
            if i < values.len() - 1 {
                format_string.push(' '); // Add a space between format specifiers
            }
        }
        format_string.push('\n'); // Add a newline at the end

        let format_string_ptr = self.builder.build_global_string_ptr(&format_string, "dynamic_format_str")?;
        call_args.push(format_string_ptr.as_pointer_value().into());

        for value in values {
            call_args.push((*value).into());
        }
        
        self.builder.build_call(printf, &call_args, "printf_call_multi")?;
        
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
            .ok_or_else(|| CodeGenError::CodeGeneration("Failed to create target machine".to_string()))?;
        
        target_machine
            .write_to_file(&self.module, inkwell::targets::FileType::Object, filename.as_ref())
            .map_err(|e| CodeGenError::CodeGeneration(format!("Failed to write object file: {:?}", e)))?;
        
        Ok(())
    }

    fn run_optimization_passes(&self) -> Result<()> {
        // Initialize all targets for the current platform.
        Target::initialize_native(&InitializationConfig::default())
            .map_err(|e| CodeGenError::CodeGeneration(format!("Failed to initialize native target: {:?}", e)))?;

        let target_triple = TargetMachine::get_default_triple();
        let target = Target::from_triple(&target_triple)
            .map_err(|e| CodeGenError::CodeGeneration(format!("Failed to create target from triple: {:?}", e)))?;
        
        // Create a target machine for the host
        // TODO: Allow specifying target CPU and features, or use a more generic target for broader compatibility if needed.
        let target_machine = target.create_target_machine(
                &target_triple, 
                "generic", // Use "native" for host CPU, or "generic" for general compatibility
                "", // CPU features. Use "+avx2" for example, or an empty string for no specific features.
                OptimizationLevel::Default, // This opt level is for the TM, passes are specified below
                RelocMode::Default, // Or RelocMode::PIC for position-independent code
                CodeModel::Default, // Or CodeModel::Small, Medium, Large
            ).ok_or_else(|| CodeGenError::CodeGeneration("Failed to create target machine".to_string()))?;

        // Define the sequence of passes to run.
        // These are common and generally safe starting passes.
        let passes = [
            "instcombine",       // Combine redundant instructions
            "reassociate",       // Reassociate expressions
            "gvn",               // Global Value Numbering
            "simplifycfg",       // Simplify control-flow graph
            "mem2reg",           // Promote memory to registers (SROA)
            // Add more passes as needed, e.g.:
            // "early-cse",         // Early Common Subexpression Elimination
            // "loop-simplify",     // Simplify loops
            // "loop-unroll",       // Unroll loops
            // "sccp",              // Sparse Conditional Constant Propagation
            // "adce",              // Aggressive Dead Code Elimination
            // "dce"                // Dead Code Elimination
        ].join(",");

        let pass_builder_options = PassBuilderOptions::create();
        // Example: Set optimization level for the pass pipeline if desired
        // pass_builder_options.set_optimization_level(OptimizationLevel::Aggressive);
        // pass_builder_options.set_verify_each(true); // For debugging passes

        self.module.run_passes(&passes, &target_machine, pass_builder_options)
            .map_err(|e_str| CodeGenError::CodeGeneration(format!("Failed to run optimization passes: {}", e_str)))?;
        
        Ok(())
    }
}
