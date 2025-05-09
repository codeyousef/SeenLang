use std::collections::HashMap;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module;
// Note: In inkwell 0.2.0, we use numeric address spaces
use inkwell::passes::PassManager;
use inkwell::types::{BasicType, BasicTypeEnum};
use inkwell::values::{BasicValueEnum, FunctionValue, PointerValue};
use inkwell::OptimizationLevel;
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
    pass_manager: PassManager<FunctionValue<'ctx>>,
}

impl<'ctx> CodeGenerator<'ctx> {
    /// Create a new code generator
    pub fn new(context: &'ctx Context, module_name: &str) -> Result<Self> {
        let module = context.create_module(module_name);
        let builder = context.create_builder();
        let type_system = TypeSystem::new(context);
        let environment = Environment::new();
        let pass_manager = PassManager::create(&module);

        // Add optimization passes
        pass_manager.add_instruction_combining_pass();
        pass_manager.add_reassociate_pass();
        pass_manager.add_gvn_pass();
        pass_manager.add_cfg_simplification_pass();
        pass_manager.add_basic_alias_analysis_pass();
        pass_manager.add_promote_memory_to_register_pass();
        pass_manager.add_instruction_combining_pass();
        pass_manager.add_reassociate_pass();

        pass_manager.initialize();

        Ok(Self {
            context,
            module,
            builder,
            type_system,
            environment,
            pass_manager,
        })
    }

    /// Generate LLVM IR for a program
    pub fn generate(&mut self, program: &Program) -> Result<&Module<'ctx>> {
        // Create a declaration for the C printf function
        self.declare_printf();

        // Process all declarations in the program
        for declaration in &program.declarations {
            self.generate_declaration(declaration)?;
        }

        Ok(&self.module)
    }

    /// Generate code for a declaration
    fn generate_declaration(&mut self, declaration: &Declaration) -> Result<()> {
        match declaration {
            Declaration::Function(func_decl) => { self.generate_function(func_decl)?; },
            Declaration::Variable(var_decl) => { self.generate_global_variable(var_decl)?; },
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
            self.builder.build_store(param_ptr, param_value);

            // Add the parameter to the environment
            self.environment.define(&param.name, param_ptr);
        }

        // Generate code for the function body
        self.generate_block(&func_decl.body)?;

        // Verify the function
        if function.verify(true) {
            // Run optimization passes on the function
            self.pass_manager.run_on(&function);
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
            BasicValueEnum::StructValue(v) => global.set_initializer(&v),
            BasicValueEnum::ArrayValue(v) => global.set_initializer(&v),
            BasicValueEnum::VectorValue(v) => global.set_initializer(&v),
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
                    self.builder.build_return(Some(&return_value));
                } else {
                    self.builder.build_return(None);
                }
                Ok(())
            },
            Statement::If(if_stmt) => {
                // Generate condition
                let condition = self.generate_expression(&if_stmt.condition)?;
                
                // Convert condition to boolean (0 = false, 1 = true)
                let condition_val = if condition.is_int_value() {
                    let int_val = condition.into_int_value();
                    self.builder.build_int_compare(
                        inkwell::IntPredicate::NE,
                        int_val,
                        self.context.bool_type().const_zero(),
                        "ifcond",
                    )
                } else {
                    return Err(CodeGenError::TypeMismatch {
                        expected: "boolean".to_string(),
                        actual: "non-boolean".to_string(),
                    });
                };

                // Get the current function
                let function = self.builder
                    .get_insert_block()
                    .and_then(|block| block.get_parent())
                    .ok_or_else(|| CodeGenError::CodeGeneration("Failed to get current function".to_string()))?;

                // Create basic blocks for the then, else, and merge
                let then_block = self.context.append_basic_block(function, "then");
                let else_block = self.context.append_basic_block(function, "else");
                let merge_block = self.context.append_basic_block(function, "ifcont");

                // Create the conditional branch
                self.builder.build_conditional_branch(condition_val, then_block, else_block);

                // Generate code for the then block
                self.builder.position_at_end(then_block);
                self.generate_statement(&if_stmt.then_branch)?;
                
                // Branch to merge block if not terminated
                if self.builder.get_insert_block().unwrap().get_terminator().is_none() {
                    self.builder.build_unconditional_branch(merge_block);
                }

                // Generate code for the else block
                self.builder.position_at_end(else_block);
                if let Some(else_branch) = &if_stmt.else_branch {
                    self.generate_statement(else_branch)?;
                }

                // Branch to merge block if not terminated
                if self.builder.get_insert_block().unwrap().get_terminator().is_none() {
                    self.builder.build_unconditional_branch(merge_block);
                }

                // Continue from the merge block
                self.builder.position_at_end(merge_block);

                Ok(())
            },
            Statement::While(while_stmt) => {
                // Get the current function
                let function = self.builder
                    .get_insert_block()
                    .and_then(|block| block.get_parent())
                    .ok_or_else(|| CodeGenError::CodeGeneration("Failed to get current function".to_string()))?;

                // Create basic blocks for the condition, loop, and merge
                let cond_block = self.context.append_basic_block(function, "while.cond");
                let loop_block = self.context.append_basic_block(function, "while.body");
                let merge_block = self.context.append_basic_block(function, "while.end");

                // Branch to the condition block
                self.builder.build_unconditional_branch(cond_block);
                self.builder.position_at_end(cond_block);

                // Generate condition
                let condition = self.generate_expression(&while_stmt.condition)?;
                
                // Convert condition to boolean (0 = false, 1 = true)
                let condition_val = if condition.is_int_value() {
                    let int_val = condition.into_int_value();
                    self.builder.build_int_compare(
                        inkwell::IntPredicate::NE,
                        int_val,
                        self.context.bool_type().const_zero(),
                        "whilecond",
                    )
                } else {
                    return Err(CodeGenError::TypeMismatch {
                        expected: "boolean".to_string(),
                        actual: "non-boolean".to_string(),
                    });
                };

                // Create the conditional branch
                self.builder.build_conditional_branch(condition_val, loop_block, merge_block);

                // Generate code for the loop block
                self.builder.position_at_end(loop_block);
                self.generate_statement(&while_stmt.body)?;
                
                // Branch back to the condition block if not terminated
                if self.builder.get_insert_block().unwrap().get_terminator().is_none() {
                    self.builder.build_unconditional_branch(cond_block);
                }

                // Continue from the merge block
                self.builder.position_at_end(merge_block);

                Ok(())
            },
            Statement::Print(print_stmt) => {
                // Generate the expression to print
                let value = self.generate_expression(&print_stmt.expression)?;
                
                // Call the appropriate printf function based on the value type
                self.build_printf_call(value)?;
                
                Ok(())
            },
        }
    }

    /// Generate code for an expression
    fn generate_expression(&mut self, expression: &Expression) -> Result<BasicValueEnum<'ctx>> {
        match expression {
            Expression::Assignment(assign_expr) => {
                // Generate the value to assign
                let value = self.generate_expression(&assign_expr.value)?;
                
                // Get the variable
                let var_ptr = self.environment.get(&assign_expr.name)
                    .ok_or_else(|| CodeGenError::UndefinedSymbol(assign_expr.name.clone()))?;
                
                // Store the value in the variable
                self.builder.build_store(var_ptr, value);
                
                // The value of an assignment expression is the assigned value
                Ok(value)
            },
            Expression::Binary(binary_expr) => {
                // Generate code for the left and right operands
                let left = self.generate_expression(&binary_expr.left)?;
                let right = self.generate_expression(&binary_expr.right)?;
                
                // Map the operator to LLVM instructions
                map_binary_operator(
                    &binary_expr.operator,
                    left,
                    right,
                    &self.builder,
                    |msg| CodeGenError::UnsupportedOperation(msg),
                )
            },
            Expression::Unary(unary_expr) => {
                // Generate code for the operand
                let operand = self.generate_expression(&unary_expr.operand)?;
                
                // Map the operator to LLVM instructions
                map_unary_operator(
                    &unary_expr.operator,
                    operand,
                    &self.builder,
                    |msg| CodeGenError::UnsupportedOperation(msg),
                )
            },
            Expression::Literal(literal) => {
                match literal {
                    ast::LiteralExpression::Number(num) => {
                        if num.is_float {
                            // Parse the floating-point value
                            let float_val = num.value.parse::<f64>()
                                .map_err(|_| CodeGenError::CodeGeneration(format!("Invalid float literal: {}", num.value)))?;
                            
                            // Create an LLVM float constant
                            Ok(self.context.f64_type().const_float(float_val).into())
                        } else {
                            // Parse the integer value
                            let int_val = num.value.parse::<i64>()
                                .map_err(|_| CodeGenError::CodeGeneration(format!("Invalid integer literal: {}", num.value)))?;
                            
                            // Create an LLVM integer constant
                            Ok(self.context.i64_type().const_int(int_val as u64, true).into())
                        }
                    },
                    ast::LiteralExpression::String(str_lit) => {
                        // Create a global string constant
                        let string_value = self.builder.build_global_string_ptr(&str_lit.value, "str");
                        
                        // Return the pointer to the string
                        Ok(string_value.as_pointer_value().into())
                    },
                    ast::LiteralExpression::Boolean(bool_lit) => {
                        // Create an LLVM boolean constant
                        Ok(self.context.bool_type().const_int(bool_lit.value as u64, false).into())
                    },
                    ast::LiteralExpression::Null(_) => {
                        // Create a null pointer
                        Ok(self.context.i8_type().ptr_type(0.into()).const_null().into())
                    },
                }
            },
            Expression::Identifier(ident) => {
                // Get the variable from the environment
                let var_ptr = self.environment.get(&ident.name)
                    .ok_or_else(|| CodeGenError::UndefinedSymbol(ident.name.clone()))?;
                
                // Load the value from the variable
                // In inkwell 0.2.0, we need a different approach to get the element type
                // Just use i32 as a placeholder - this would need proper type tracking in production
                let value = self.builder.build_load(self.context.i32_type(), var_ptr, &ident.name);
                
                Ok(value)
            },
            Expression::Call(call_expr) => {
                // Look up the function in the module
                let function = self.module.get_function(&call_expr.callee)
                    .ok_or_else(|| CodeGenError::UndefinedSymbol(call_expr.callee.clone()))?;
                
                // Generate code for the arguments
                let mut args: Vec<inkwell::values::BasicMetadataValueEnum<'ctx>> = Vec::new();
                for arg in &call_expr.arguments {
                    args.push(self.generate_expression(arg)?.into());
                }
                
                // Call the function
                let call_site_value = self.builder.build_call(function, &args, "call")
                    .try_as_basic_value()
                    .left();
                
                match call_site_value {
                    Some(value) => Ok(value),
                    None => {
                        // If the function returns void, return a dummy value
                        Ok(self.context.i32_type().const_int(0, false).into())
                    },
                }
            },
            Expression::Parenthesized(paren_expr) => {
                // Simply generate code for the inner expression
                self.generate_expression(&paren_expr.expression)
            },
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
        
        builder.build_alloca(ty, name)
    }

    /// Declare the printf function for use in print statements
    fn declare_printf(&self) -> FunctionValue<'ctx> {
        // Check if printf is already declared
        if let Some(printf) = self.module.get_function("printf") {
            return printf;
        }

        // Create the printf function type:
        // int printf(const char *format, ...);
        let printf_type = self.context.i32_type().fn_type(
            &[self.context.i8_type().ptr_type(0.into()).into()],
            true,
        );

        // Declare the printf function
        self.module.add_function("printf", printf_type, None)
    }

    /// Build a call to printf with the appropriate format string
    fn build_printf_call(&self, value: BasicValueEnum<'ctx>) -> Result<()> {
        let printf = self.declare_printf();
        
        let format_string = match value {
            BasicValueEnum::IntValue(_) => {
                self.builder.build_global_string_ptr("%lld\n", "int_format")
            },
            BasicValueEnum::FloatValue(_) => {
                self.builder.build_global_string_ptr("%lf\n", "float_format")
            },
            BasicValueEnum::PointerValue(_) => {
                // Assume pointer to string
                self.builder.build_global_string_ptr("%s\n", "str_format")
            },
            _ => {
                return Err(CodeGenError::UnsupportedOperation(
                    "Cannot print this type of value".to_string()
                ));
            },
        };
        
        self.builder.build_call(
            printf,
            &[format_string.as_pointer_value().into(), value.into()],
            "printf_call",
        );
        
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
}
