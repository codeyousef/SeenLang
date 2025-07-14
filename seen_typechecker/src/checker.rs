//! Type checker implementation for the Seen programming language

use std::collections::HashMap;
use seen_parser::ast::*;
use seen_lexer::token::Location;
use crate::types::Type;
use crate::errors::{TypeError, type_mismatch, undefined_variable, undefined_function};
use crate::{TypeCheckResult, FunctionSignature, Parameter};

/// Type checking environment
#[derive(Debug, Clone)]
struct Environment {
    /// Variables in scope with their types
    variables: HashMap<String, Type>,
    /// Functions in scope with their signatures
    functions: HashMap<String, FunctionSignature>,
    /// Struct definitions with their fields
    structs: HashMap<String, Vec<StructField>>,
    /// Enum definitions with their variants
    enums: HashMap<String, Vec<EnumVariant>>,
    /// Parent environment for nested scopes
    parent: Option<Box<Environment>>,
}

impl Environment {
    /// Create a new empty environment
    fn new() -> Self {
        Self {
            variables: HashMap::new(),
            functions: HashMap::new(),
            structs: HashMap::new(),
            enums: HashMap::new(),
            parent: None,
        }
    }

    /// Create a new environment with a parent
    fn with_parent(parent: Environment) -> Self {
        Self {
            variables: HashMap::new(),
            functions: HashMap::new(),
            structs: HashMap::new(),
            enums: HashMap::new(),
            parent: Some(Box::new(parent)),
        }
    }

    /// Define a variable in this environment
    fn define_variable(&mut self, name: String, var_type: Type) {
        self.variables.insert(name, var_type);
    }

    /// Define a function in this environment
    fn define_function(&mut self, name: String, signature: FunctionSignature) {
        self.functions.insert(name, signature);
    }

    /// Define a struct in this environment
    fn define_struct(&mut self, name: String, fields: Vec<StructField>) {
        self.structs.insert(name, fields);
    }

    /// Define an enum in this environment
    fn define_enum(&mut self, name: String, variants: Vec<EnumVariant>) {
        self.enums.insert(name, variants);
    }

    /// Look up a variable type, checking parent environments
    fn get_variable(&self, name: &str) -> Option<&Type> {
        self.variables.get(name)
            .or_else(|| self.parent.as_ref().and_then(|p| p.get_variable(name)))
    }

    /// Look up a function signature, checking parent environments
    fn get_function(&self, name: &str) -> Option<&FunctionSignature> {
        self.functions.get(name)
            .or_else(|| self.parent.as_ref().and_then(|p| p.get_function(name)))
    }

    /// Look up a struct definition, checking parent environments
    fn get_struct(&self, name: &str) -> Option<&Vec<StructField>> {
        self.structs.get(name)
            .or_else(|| self.parent.as_ref().and_then(|p| p.get_struct(name)))
    }

    /// Look up an enum definition, checking parent environments
    fn get_enum(&self, name: &str) -> Option<&Vec<EnumVariant>> {
        self.enums.get(name)
            .or_else(|| self.parent.as_ref().and_then(|p| p.get_enum(name)))
    }

    /// Check if a variable is defined in this scope only
    fn has_variable(&self, name: &str) -> bool {
        self.variables.contains_key(name)
    }

    /// Check if a function is defined in this scope only
    fn has_function(&self, name: &str) -> bool {
        self.functions.contains_key(name)
    }

    /// Check if a struct is defined in this scope only
    fn has_struct(&self, name: &str) -> bool {
        self.structs.contains_key(name)
    }

    /// Check if an enum is defined in this scope only
    fn has_enum(&self, name: &str) -> bool {
        self.enums.contains_key(name)
    }

    /// Push a new scope
    fn push_scope(&mut self) {
        let new_env = Environment {
            variables: HashMap::new(),
            functions: HashMap::new(),
            structs: HashMap::new(),
            enums: HashMap::new(),
            parent: Some(Box::new(self.clone())),
        };
        *self = new_env;
    }

    /// Pop the current scope
    fn pop_scope(&mut self) {
        if let Some(parent) = &self.parent {
            *self = parent.as_ref().clone();
        }
    }
}

/// Main type checker
pub struct TypeChecker {
    /// Current environment
    env: Environment,
    /// Type checking result
    result: TypeCheckResult,
    /// Current function return type (for return type checking)
    current_function_return_type: Option<Type>,
}

impl TypeChecker {
    /// Create a new type checker
    pub fn new() -> Self {
        let mut env = Environment::new();
        
        // Add built-in functions
        env.define_function("println".to_string(), FunctionSignature {
            name: "println".to_string(),
            parameters: vec![Parameter {
                name: "value".to_string(),
                param_type: Type::String,
            }],
            return_type: Some(Type::Unit),
        });

        Self {
            env,
            result: TypeCheckResult::new(),
            current_function_return_type: None,
        }
    }

    /// Type check a program
    pub fn check_program(&mut self, program: &Program) -> TypeCheckResult {
        for declaration in &program.declarations {
            self.check_declaration(declaration);
        }

        // Collect all variables and functions into the result
        self.collect_environment();
        
        std::mem::take(&mut self.result)
    }

    /// Collect environment data into the result
    fn collect_environment(&mut self) {
        for (name, var_type) in &self.env.variables {
            self.result.add_variable(name.clone(), var_type.clone());
        }
        for (name, signature) in &self.env.functions {
            self.result.add_function(name.clone(), signature.clone());
        }
    }

    /// Type check a declaration
    fn check_declaration(&mut self, declaration: &Declaration) {
        match declaration {
            Declaration::Function(func_decl) => {
                self.check_function_declaration(func_decl);
            }
            Declaration::Variable(var_decl) => {
                self.check_variable_declaration(var_decl);
            }
            Declaration::Struct(struct_decl) => {
                self.check_struct_declaration(struct_decl);
            }
            Declaration::Enum(enum_decl) => {
                self.check_enum_declaration(enum_decl);
            }
        }
    }

    /// Type check a variable declaration
    fn check_variable_declaration(&mut self, var_decl: &VariableDeclaration) {
        // Check for duplicate variable
        if self.env.has_variable(&var_decl.name) {
            self.result.add_error(TypeError::DuplicateVariable {
                name: var_decl.name.clone(),
                position: self.location_to_position(&var_decl.location),
            });
            return;
        }

        let var_type = match (&var_decl.var_type, &var_decl.initializer) {
            // Type declared and value provided
            (Some(declared), initializer) => {
                let value_type = self.check_expression(initializer);
                let declared_type = Type::from(declared);
                
                if !value_type.is_assignable_to(&declared_type) {
                    self.result.add_error(type_mismatch(
                        declared_type.clone(), 
                        value_type, 
                        self.location_to_position(&var_decl.location)
                    ));
                }
                declared_type
            }
            // Only value provided (type inference)
            (None, initializer) => {
                let value_type = self.check_expression(initializer);
                if matches!(value_type, Type::Unknown) {
                    self.result.add_error(TypeError::InferenceFailed { 
                        position: self.location_to_position(&var_decl.location) 
                    });
                    Type::Unknown
                } else {
                    value_type
                }
            }
        };

        self.env.define_variable(var_decl.name.clone(), var_type);
    }

    /// Type check a struct declaration
    fn check_struct_declaration(&mut self, struct_decl: &StructDeclaration) {
        // Check for duplicate struct
        if self.env.has_struct(&struct_decl.name) {
            self.result.add_error(TypeError::DuplicateFunction {
                name: struct_decl.name.clone(),
                position: self.location_to_position(&struct_decl.location),
            });
            return;
        }

        // Validate field types and register the struct
        self.env.define_struct(struct_decl.name.clone(), struct_decl.fields.clone());
    }

    /// Type check an enum declaration
    fn check_enum_declaration(&mut self, enum_decl: &EnumDeclaration) {
        // Check for duplicate enum
        if self.env.has_enum(&enum_decl.name) {
            self.result.add_error(TypeError::DuplicateFunction {
                name: enum_decl.name.clone(),
                position: self.location_to_position(&enum_decl.location),
            });
            return;
        }

        // For generic enums, we need to validate the type parameters are used correctly
        if !enum_decl.type_parameters.is_empty() {
            // Create a temporary type environment with the type parameters
            let mut temp_type_params = std::collections::HashSet::new();
            for param in &enum_decl.type_parameters {
                if !temp_type_params.insert(param.clone()) {
                    self.result.add_error(TypeError::DuplicateFunction {
                        name: format!("Duplicate type parameter '{}'", param),
                        position: self.location_to_position(&enum_decl.location),
                    });
                }
            }
            
            // Validate that variant data types use valid types (including type parameters)
            for variant in &enum_decl.variants {
                if let Some(data_types) = &variant.data {
                    for data_type in data_types {
                        self.validate_type_with_params(data_type, &enum_decl.type_parameters, &variant.location);
                    }
                }
            }
        }

        // Register the enum (generic enums are templates, concrete instances are created on use)
        self.env.define_enum(enum_decl.name.clone(), enum_decl.variants.clone());
    }

    /// Type check a function declaration
    fn check_function_declaration(&mut self, func_decl: &FunctionDeclaration) {
        // Check for duplicate function
        if self.env.has_function(&func_decl.name) {
            self.result.add_error(TypeError::DuplicateFunction {
                name: func_decl.name.clone(),
                position: self.location_to_position(&func_decl.location),
            });
            return;
        }

        // Convert parameters
        let param_types: Vec<Parameter> = func_decl.parameters.iter().map(|p| Parameter {
            name: p.name.clone(),
            param_type: Type::from(&p.param_type),
        }).collect();

        let return_type = func_decl.return_type.as_ref()
            .map(Type::from);

        // Create function signature
        let signature = FunctionSignature {
            name: func_decl.name.clone(),
            parameters: param_types.clone(),
            return_type: return_type.clone(),
        };

        // Define function in current environment
        self.env.define_function(func_decl.name.clone(), signature);

        // Create new environment for function body
        let old_env = self.env.clone();
        self.env = Environment::with_parent(old_env);

        // Add parameters to function environment
        for param in &param_types {
            self.env.define_variable(param.name.clone(), param.param_type.clone());
        }

        // Set current function return type
        let old_return_type = std::mem::replace(&mut self.current_function_return_type, return_type);

        // Type check function body
        self.check_block(&func_decl.body);

        // Restore environment and return type
        self.env = self.env.parent.as_ref().unwrap().as_ref().clone();
        self.current_function_return_type = old_return_type;
    }

    /// Type check a statement
    fn check_statement(&mut self, statement: &Statement) {
        match statement {
            Statement::Expression(expr_stmt) => {
                self.check_expression(&expr_stmt.expression);
            }
            Statement::Block(block) => {
                self.check_block(block);
            }
            Statement::Return(return_stmt) => {
                self.check_return_statement(return_stmt);
            }
            Statement::If(if_stmt) => {
                self.check_if_statement(if_stmt);
            }
            Statement::While(while_stmt) => {
                self.check_while_statement(while_stmt);
            }
            Statement::Print(print_stmt) => {
                self.check_print_statement(print_stmt);
            }
            Statement::DeclarationStatement(decl) => {
                self.check_declaration(decl);
            }
            Statement::For(for_stmt) => {
                self.check_for_statement(for_stmt);
            }
            Statement::Match(match_stmt) => {
                self.check_match_statement(match_stmt);
            }
        }
    }

    /// Type check an if statement
    fn check_if_statement(&mut self, if_stmt: &IfStatement) {
        let condition_type = self.check_expression(&if_stmt.condition);
        if !condition_type.is_assignable_to(&Type::Bool) {
            self.result.add_error(type_mismatch(
                Type::Bool, 
                condition_type, 
                self.location_to_position(&if_stmt.location)
            ));
        }

        self.check_statement(&if_stmt.then_branch);
        if let Some(else_branch) = &if_stmt.else_branch {
            self.check_statement(else_branch);
        }
    }

    /// Type check a for statement
    fn check_for_statement(&mut self, for_stmt: &ForStatement) {
        // Check the iterable expression
        let iterable_type = self.check_expression(&for_stmt.iterable);
        
        // For now, only support arrays
        match iterable_type {
            Type::Array(element_type) => {
                // Create new scope for loop variable
                self.env.push_scope();
                self.env.define_variable(for_stmt.variable.clone(), *element_type);
                
                // Check the body
                self.check_statement(&for_stmt.body);
                
                self.env.pop_scope();
            }
            _ => {
                self.result.add_error(TypeError::TypeMismatch {
                    expected: Type::Array(Box::new(Type::Unknown)),
                    actual: iterable_type,
                    position: self.expression_location(&for_stmt.iterable),
                });
            }
        }
    }

    /// Type check a while statement
    fn check_while_statement(&mut self, while_stmt: &WhileStatement) {
        let condition_type = self.check_expression(&while_stmt.condition);
        if !condition_type.is_assignable_to(&Type::Bool) {
            self.result.add_error(type_mismatch(
                Type::Bool, 
                condition_type, 
                self.location_to_position(&while_stmt.location)
            ));
        }

        self.check_statement(&while_stmt.body);
    }

    /// Type check a return statement
    fn check_return_statement(&mut self, return_stmt: &ReturnStatement) {
        let return_type = if let Some(val) = &return_stmt.value {
            self.check_expression(val)
        } else {
            Type::Unit
        };

        if let Some(expected_type) = &self.current_function_return_type {
            if !return_type.is_assignable_to(expected_type) {
                self.result.add_error(TypeError::ReturnTypeMismatch {
                    expected: expected_type.clone(),
                    actual: return_type,
                    position: self.location_to_position(&return_stmt.location),
                });
            }
        }
    }

    /// Type check a print statement
    fn check_print_statement(&mut self, print_stmt: &PrintStatement) {
        for arg in &print_stmt.arguments {
            self.check_expression(arg);
        }
    }

    /// Type check a match statement
    fn check_match_statement(&mut self, match_stmt: &MatchStatement) {
        let value_type = self.check_expression(&match_stmt.value);
        
        // Check each match arm
        for arm in &match_stmt.arms {
            // Type check the pattern and bind any variables
            self.env.push_scope();
            self.check_pattern(&arm.pattern, &value_type);
            
            // Type check the expression
            self.check_expression(&arm.expression);
            
            self.env.pop_scope();
        }
    }

    /// Type check a pattern and bind variables
    fn check_pattern(&mut self, pattern: &Pattern, expected_type: &Type) {
        match pattern {
            Pattern::Literal(lit_pattern) => {
                let literal_type = self.check_literal_expression(&lit_pattern.value);
                if !literal_type.is_assignable_to(expected_type) {
                    self.result.add_error(type_mismatch(
                        expected_type.clone(),
                        literal_type,
                        self.location_to_position(&lit_pattern.location),
                    ));
                }
            }
            Pattern::Identifier(id_pattern) => {
                // Bind the identifier to the expected type
                self.env.define_variable(id_pattern.name.clone(), expected_type.clone());
            }
            Pattern::EnumVariant(enum_pattern) => {
                // Check that the enum exists and has this variant
                if let Some(variants) = self.env.get_enum(&enum_pattern.enum_name) {
                    let variant = variants.iter().find(|v| v.name == enum_pattern.variant_name);
                    if let Some(enum_variant) = variant {
                        // Check that the pattern matches the variant structure
                        match (&enum_pattern.patterns, &enum_variant.data) {
                            (Some(patterns), Some(data_types)) => {
                                if patterns.len() != data_types.len() {
                                    // Pattern arity mismatch
                                    return;
                                }
                                // Clone data types to avoid borrowing issues
                                let data_types_clone = data_types.clone();
                                // Check each sub-pattern against the corresponding data type
                                for (sub_pattern, data_type) in patterns.iter().zip(data_types_clone.iter()) {
                                    let converted_type = Type::from(data_type);
                                    self.check_pattern(sub_pattern, &converted_type);
                                }
                            }
                            (None, None) => {
                                // Unit variant - no data
                            }
                            _ => {
                                // Mismatch between expected data and actual pattern
                                self.result.add_error(TypeError::TypeMismatch {
                                    expected: expected_type.clone(),
                                    actual: Type::Enum(enum_pattern.enum_name.clone()),
                                    position: self.location_to_position(&enum_pattern.location),
                                });
                            }
                        }
                    } else {
                        // Variant not found
                        self.result.add_error(TypeError::UndefinedVariable {
                            name: format!("{}::{}", enum_pattern.enum_name, enum_pattern.variant_name),
                            position: self.location_to_position(&enum_pattern.location),
                        });
                    }
                } else {
                    // Enum not found
                    self.result.add_error(TypeError::UndefinedVariable {
                        name: enum_pattern.enum_name.clone(),
                        position: self.location_to_position(&enum_pattern.location),
                    });
                }
            }
            Pattern::Wildcard(_) => {
                // Wildcard matches anything - no type checking needed
            }
        }
    }

    /// Type check a block
    fn check_block(&mut self, block: &Block) {
        // Create new scope
        let old_env = self.env.clone();
        self.env = Environment::with_parent(old_env);

        for statement in &block.statements {
            self.check_statement(statement);
        }

        // Restore environment
        self.env = self.env.parent.as_ref().unwrap().as_ref().clone();
    }

    /// Type check an expression and return its type
    fn check_expression(&mut self, expression: &Expression) -> Type {
        match expression {
            Expression::Literal(literal) => {
                self.check_literal_expression(literal)
            }
            Expression::Identifier(ident) => {
                if let Some(var_type) = self.env.get_variable(&ident.name) {
                    var_type.clone()
                } else {
                    self.result.add_error(undefined_variable(
                        ident.name.clone(), 
                        self.location_to_position(&ident.location)
                    ));
                    Type::Unknown
                }
            }
            Expression::Binary(binary) => {
                self.check_binary_expression(binary)
            }
            Expression::Unary(unary) => {
                self.check_unary_expression(unary)
            }
            Expression::Call(call) => {
                self.check_call_expression(call)
            }
            Expression::Assignment(assignment) => {
                self.check_assignment_expression(assignment)
            }
            Expression::Parenthesized(paren) => {
                self.check_expression(&paren.expression)
            }
            Expression::StructLiteral(struct_lit) => {
                self.check_struct_literal(struct_lit)
            }
            Expression::FieldAccess(field_access) => {
                self.check_field_access(field_access)
            }
            Expression::ArrayLiteral(array) => {
                self.check_array_literal(array)
            }
            Expression::Index(index) => {
                self.check_index_expression(index)
            }
            Expression::Range(_) => {
                // Range expressions return arrays of integers
                Type::Array(Box::new(Type::Int))
            }
            Expression::Match(match_expr) => {
                self.check_match_expression(match_expr)
            }
            Expression::EnumLiteral(enum_literal) => {
                self.check_enum_literal(enum_literal)
            }
            Expression::Try(try_expr) => {
                self.check_try_expression(try_expr)
            }
        }
    }

    /// Type check a literal expression
    fn check_literal_expression(&self, literal: &LiteralExpression) -> Type {
        match literal {
            LiteralExpression::Number(num) => {
                if num.is_float {
                    Type::Float
                } else {
                    Type::Int
                }
            }
            LiteralExpression::String(_) => Type::String,
            LiteralExpression::Boolean(_) => Type::Bool,
            LiteralExpression::Null(_) => Type::Optional(Box::new(Type::Unknown)),
        }
    }

    /// Type check a binary expression
    fn check_binary_expression(&mut self, binary: &BinaryExpression) -> Type {
        let left_type = self.check_expression(&binary.left);
        let right_type = self.check_expression(&binary.right);

        match binary.operator {
            BinaryOperator::Add | BinaryOperator::Subtract | 
            BinaryOperator::Multiply | BinaryOperator::Divide | BinaryOperator::Modulo => {
                if left_type.is_numeric() && right_type.is_numeric() {
                    // Promote to Float if either operand is Float
                    if matches!(left_type, Type::Float) || matches!(right_type, Type::Float) {
                        Type::Float
                    } else {
                        Type::Int
                    }
                } else {
                    self.result.add_error(TypeError::InvalidOperation {
                        operation: format!("{:?}", binary.operator),
                        left_type: left_type.clone(),
                        right_type: right_type.clone(),
                        position: self.location_to_position(&binary.location),
                    });
                    Type::Unknown
                }
            }
            BinaryOperator::Equal | BinaryOperator::NotEqual => {
                if left_type.is_assignable_to(&right_type) || right_type.is_assignable_to(&left_type) {
                    Type::Bool
                } else {
                    self.result.add_error(TypeError::InvalidOperation {
                        operation: format!("{:?}", binary.operator),
                        left_type: left_type.clone(),
                        right_type: right_type.clone(),
                        position: self.location_to_position(&binary.location),
                    });
                    Type::Bool
                }
            }
            BinaryOperator::LessThan | BinaryOperator::GreaterThan |
            BinaryOperator::LessEqual | BinaryOperator::GreaterEqual => {
                if left_type.is_numeric() && right_type.is_numeric() {
                    Type::Bool
                } else {
                    self.result.add_error(TypeError::InvalidOperation {
                        operation: format!("{:?}", binary.operator),
                        left_type: left_type.clone(),
                        right_type: right_type.clone(),
                        position: self.location_to_position(&binary.location),
                    });
                    Type::Bool
                }
            }
            BinaryOperator::And | BinaryOperator::Or => {
                if left_type.is_assignable_to(&Type::Bool) && right_type.is_assignable_to(&Type::Bool) {
                    Type::Bool
                } else {
                    self.result.add_error(TypeError::InvalidOperation {
                        operation: format!("{:?}", binary.operator),
                        left_type: left_type.clone(),
                        right_type: right_type.clone(),
                        position: self.location_to_position(&binary.location),
                    });
                    Type::Bool
                }
            }
        }
    }

    /// Type check a unary expression
    fn check_unary_expression(&mut self, unary: &UnaryExpression) -> Type {
        let operand_type = self.check_expression(&unary.operand);

        match unary.operator {
            UnaryOperator::Negate => {
                if operand_type.is_numeric() {
                    operand_type
                } else {
                    self.result.add_error(TypeError::InvalidOperation {
                        operation: "unary minus".to_string(),
                        left_type: operand_type.clone(),
                        right_type: Type::Unit,
                        position: self.location_to_position(&unary.location),
                    });
                    Type::Unknown
                }
            }
            UnaryOperator::Not => {
                if operand_type.is_assignable_to(&Type::Bool) {
                    Type::Bool
                } else {
                    self.result.add_error(TypeError::InvalidOperation {
                        operation: "logical not".to_string(),
                        left_type: operand_type.clone(),
                        right_type: Type::Unit,
                        position: self.location_to_position(&unary.location),
                    });
                    Type::Bool
                }
            }
            UnaryOperator::Plus => {
                if operand_type.is_numeric() {
                    operand_type
                } else {
                    self.result.add_error(TypeError::InvalidOperation {
                        operation: "unary plus".to_string(),
                        left_type: operand_type.clone(),
                        right_type: Type::Unit,
                        position: self.location_to_position(&unary.location),
                    });
                    Type::Unknown
                }
            }
        }
    }

    /// Type check a function call
    fn check_call_expression(&mut self, call: &CallExpression) -> Type {
        let signature = self.env.get_function(&call.callee).cloned();
        if let Some(signature) = signature {
            // Check argument count
            if call.arguments.len() != signature.parameters.len() {
                self.result.add_error(TypeError::WrongArgumentCount {
                    expected: signature.parameters.len(),
                    actual: call.arguments.len(),
                    position: self.location_to_position(&call.location),
                });
                return signature.return_type.clone().unwrap_or(Type::Unit);
            }

            // Check argument types
            for (arg, param) in call.arguments.iter().zip(&signature.parameters) {
                let arg_type = self.check_expression(arg);
                if !arg_type.is_assignable_to(&param.param_type) {
                    self.result.add_error(type_mismatch(
                        param.param_type.clone(), 
                        arg_type, 
                        self.expression_location(arg)
                    ));
                }
            }

            signature.return_type.clone().unwrap_or(Type::Unit)
        } else {
            self.result.add_error(undefined_function(
                call.callee.clone(), 
                self.location_to_position(&call.location)
            ));
            Type::Unknown
        }
    }

    /// Type check an assignment expression
    fn check_assignment_expression(&mut self, assignment: &AssignmentExpression) -> Type {
        // Check if variable exists
        let target_type = self.env.get_variable(&assignment.name).cloned();
        if let Some(target_type) = target_type {
            let value_type = self.check_expression(&assignment.value);
            if !value_type.is_assignable_to(&target_type) {
                self.result.add_error(type_mismatch(
                    target_type.clone(), 
                    value_type, 
                    self.location_to_position(&assignment.location)
                ));
            }
            target_type
        } else {
            self.result.add_error(undefined_variable(
                assignment.name.clone(), 
                self.location_to_position(&assignment.location)
            ));
            Type::Unknown
        }
    }

    /// Get the location of an expression
    fn expression_location(&self, expr: &Expression) -> seen_lexer::token::Position {
        let location = match expr {
            Expression::Assignment(a) => &a.location,
            Expression::Binary(b) => &b.location,
            Expression::Unary(u) => &u.location,
            Expression::Literal(l) => match l {
                LiteralExpression::Number(n) => &n.location,
                LiteralExpression::String(s) => &s.location,
                LiteralExpression::Boolean(b) => &b.location,
                LiteralExpression::Null(n) => &n.location,
            },
            Expression::Identifier(i) => &i.location,
            Expression::Call(c) => &c.location,
            Expression::Parenthesized(p) => &p.location,
            Expression::StructLiteral(s) => &s.location,
            Expression::FieldAccess(f) => &f.location,
            Expression::ArrayLiteral(a) => &a.location,
            Expression::Index(i) => &i.location,
            Expression::Range(r) => &r.location,
            Expression::Match(m) => &m.location,
            Expression::EnumLiteral(e) => &e.location,
            Expression::Try(t) => &t.location,
        };
        self.location_to_position(location)
    }

    /// Convert Location to Position (for compatibility)
    fn location_to_position(&self, location: &Location) -> seen_lexer::token::Position {
        seen_lexer::token::Position::new(location.start.line, location.start.column)
    }

    /// Validate that a type is valid within a generic context
    fn validate_type_with_params(&mut self, type_ref: &seen_parser::ast::Type, type_params: &[String], location: &Location) {
        match type_ref {
            seen_parser::ast::Type::Simple(name) => {
                // Check if it's a type parameter or a known type
                if !type_params.contains(name) && !self.is_known_type(name) {
                    self.result.add_error(TypeError::UndefinedType {
                        name: name.clone(),
                        position: self.location_to_position(location),
                    });
                }
            }
            seen_parser::ast::Type::Array(inner) => {
                self.validate_type_with_params(inner, type_params, location);
            }
            seen_parser::ast::Type::Struct(name) => {
                if !self.env.has_struct(name) {
                    self.result.add_error(TypeError::UndefinedType {
                        name: name.clone(),
                        position: self.location_to_position(location),
                    });
                }
            }
            seen_parser::ast::Type::Enum(name) => {
                if !self.env.has_enum(name) {
                    self.result.add_error(TypeError::UndefinedType {
                        name: name.clone(),
                        position: self.location_to_position(location),
                    });
                }
            }
            seen_parser::ast::Type::Generic(name, args) => {
                // Validate the base type and all arguments
                if !type_params.contains(name) && !self.is_known_type(name) && !self.env.has_enum(name) {
                    self.result.add_error(TypeError::UndefinedType {
                        name: name.clone(),
                        position: self.location_to_position(location),
                    });
                }
                for arg in args {
                    self.validate_type_with_params(arg, type_params, location);
                }
            }
            seen_parser::ast::Type::Pointer(inner) => {
                // Validate the pointed-to type
                self.validate_type_with_params(inner, type_params, location);
            }
        }
    }

    /// Check if a type name is a known primitive type
    fn is_known_type(&self, name: &str) -> bool {
        matches!(name, "Int" | "Float" | "Bool" | "String" | "Char" | "()")
    }

    /// Type check an array literal
    fn check_array_literal(&mut self, array: &ArrayLiteralExpression) -> Type {
        if array.elements.is_empty() {
            // Empty array - we can't infer the type
            Type::Array(Box::new(Type::Unknown))
        } else {
            // Check the first element to determine the array type
            let first_type = self.check_expression(&array.elements[0]);
            
            // Check that all elements have the same type
            for element in &array.elements[1..] {
                let element_type = self.check_expression(element);
                if !element_type.is_assignable_to(&first_type) {
                    self.result.add_error(TypeError::TypeMismatch {
                        expected: first_type.clone(),
                        actual: element_type,
                        position: self.expression_location(element),
                    });
                }
            }
            
            Type::Array(Box::new(first_type))
        }
    }

    /// Type check an index expression
    fn check_index_expression(&mut self, index: &IndexExpression) -> Type {
        let object_type = self.check_expression(&index.object);
        let index_type = self.check_expression(&index.index);
        
        // Index must be an integer
        if !index_type.is_assignable_to(&Type::Int) {
            self.result.add_error(TypeError::TypeMismatch {
                expected: Type::Int,
                actual: index_type,
                position: self.expression_location(&index.index),
            });
        }
        
        // Object must be an array
        match object_type {
            Type::Array(element_type) => *element_type,
            _ => {
                self.result.add_error(TypeError::TypeMismatch {
                    expected: Type::Array(Box::new(Type::Unknown)),
                    actual: object_type,
                    position: self.expression_location(&index.object),
                });
                Type::Unknown
            }
        }
    }

    /// Type check a struct literal
    fn check_struct_literal(&mut self, struct_lit: &StructLiteralExpression) -> Type {
        // Look up the struct definition
        let struct_fields = match self.env.get_struct(&struct_lit.struct_name) {
            Some(fields) => fields.clone(),
            None => {
                self.result.add_error(TypeError::UndefinedVariable {
                    name: struct_lit.struct_name.clone(),
                    position: self.location_to_position(&struct_lit.location),
                });
                return Type::Unknown;
            }
        };

        // Check that all required fields are provided
        let mut provided_fields: std::collections::HashSet<String> = std::collections::HashSet::new();
        
        for field_init in &struct_lit.fields {
            provided_fields.insert(field_init.field_name.clone());
            
            // Find the field definition
            let field_def = struct_fields.iter().find(|f| f.name == field_init.field_name);
            
            match field_def {
                Some(def) => {
                    // Type check the field value
                    let actual_type = self.check_expression(&field_init.value);
                    let expected_type = Type::from(&def.field_type);
                    
                    if !actual_type.is_assignable_to(&expected_type) {
                        self.result.add_error(TypeError::TypeMismatch {
                            expected: expected_type,
                            actual: actual_type,
                            position: self.expression_location(&field_init.value),
                        });
                    }
                }
                None => {
                    // Unknown field
                    self.result.add_error(TypeError::UnknownField {
                        struct_name: struct_lit.struct_name.clone(),
                        field: field_init.field_name.clone(),
                        position: self.location_to_position(&field_init.location),
                    });
                }
            }
        }

        // Check for missing fields
        for field_def in &struct_fields {
            if !provided_fields.contains(&field_def.name) {
                self.result.add_error(TypeError::MissingField {
                    struct_name: struct_lit.struct_name.clone(),
                    field: field_def.name.clone(),
                    position: self.location_to_position(&struct_lit.location),
                });
            }
        }

        Type::Struct(struct_lit.struct_name.clone())
    }

    /// Type check a field access expression
    fn check_field_access(&mut self, field_access: &FieldAccessExpression) -> Type {
        let object_type = self.check_expression(&field_access.object);
        
        match object_type {
            Type::Struct(struct_name) => {
                // Look up the struct definition
                let struct_fields = match self.env.get_struct(&struct_name) {
                    Some(fields) => fields.clone(),
                    None => {
                        self.result.add_error(TypeError::UndefinedVariable {
                            name: struct_name.clone(),
                            position: self.expression_location(&field_access.object),
                        });
                        return Type::Unknown;
                    }
                };

                // Find the field
                let field_def = struct_fields.iter().find(|f| f.name == field_access.field);
                
                match field_def {
                    Some(def) => Type::from(&def.field_type),
                    None => {
                        self.result.add_error(TypeError::UnknownField {
                            struct_name: struct_name.clone(),
                            field: field_access.field.clone(),
                            position: self.location_to_position(&field_access.location),
                        });
                        Type::Unknown
                    }
                }
            }
            _ => {
                self.result.add_error(TypeError::TypeMismatch {
                    expected: Type::Struct("any".to_string()),
                    actual: object_type,
                    position: self.expression_location(&field_access.object),
                });
                Type::Unknown
            }
        }
    }

    /// Type check an enum literal expression
    fn check_enum_literal(&mut self, enum_literal: &EnumLiteralExpression) -> Type {
        // Check that the enum exists
        if let Some(variants) = self.env.get_enum(&enum_literal.enum_name) {
            // Check that the variant exists
            let variant = variants.iter().find(|v| v.name == enum_literal.variant_name);
            if let Some(enum_variant) = variant {
                // Check that the arguments match the variant structure
                match (&enum_literal.arguments, &enum_variant.data) {
                    (Some(args), Some(data_types)) => {
                        if args.len() != data_types.len() {
                            self.result.add_error(TypeError::WrongArgumentCount {
                                expected: data_types.len(),
                                actual: args.len(),
                                position: self.location_to_position(&enum_literal.location),
                            });
                            return Type::Unknown;
                        }
                        // Clone data types to avoid borrowing issues
                        let data_types_clone = data_types.clone();
                        // Check each argument against the corresponding data type
                        for (arg, data_type) in args.iter().zip(data_types_clone.iter()) {
                            let arg_type = self.check_expression(arg);
                            let expected_type = Type::from(data_type);
                            if !arg_type.is_assignable_to(&expected_type) {
                                self.result.add_error(TypeError::TypeMismatch {
                                    expected: expected_type,
                                    actual: arg_type,
                                    position: self.expression_location(arg),
                                });
                            }
                        }
                    }
                    (None, None) => {
                        // Unit variant - no arguments expected
                    }
                    (Some(_), None) => {
                        // Variant expects no arguments but got some
                        self.result.add_error(TypeError::WrongArgumentCount {
                            expected: 0,
                            actual: enum_literal.arguments.as_ref().unwrap().len(),
                            position: self.location_to_position(&enum_literal.location),
                        });
                        return Type::Unknown;
                    }
                    (None, Some(data_types)) => {
                        // Variant expects arguments but got none
                        self.result.add_error(TypeError::WrongArgumentCount {
                            expected: data_types.len(),
                            actual: 0,
                            position: self.location_to_position(&enum_literal.location),
                        });
                        return Type::Unknown;
                    }
                }
                Type::Enum(enum_literal.enum_name.clone())
            } else {
                // Variant not found
                self.result.add_error(TypeError::UndefinedVariable {
                    name: format!("{}::{}", enum_literal.enum_name, enum_literal.variant_name),
                    position: self.location_to_position(&enum_literal.location),
                });
                Type::Unknown
            }
        } else {
            // Enum not found
            self.result.add_error(TypeError::UndefinedVariable {
                name: enum_literal.enum_name.clone(),
                position: self.location_to_position(&enum_literal.location),
            });
            Type::Unknown
        }
    }

    /// Type check a try expression (? operator)
    fn check_try_expression(&mut self, try_expr: &TryExpression) -> Type {
        let expr_type = self.check_expression(&try_expr.expression);
        
        // The ? operator should only be used on Result<T, E> types
        match &expr_type {
            Type::ParameterizedGeneric(name, args) if name == "Result" && args.len() == 2 => {
                // Extract the T type from Result<T, E>
                args[0].clone()
            }
            _ => {
                self.result.add_error(TypeError::InvalidOperation {
                    operation: "? operator".to_string(),
                    left_type: expr_type.clone(),
                    right_type: Type::Unit, // Not used for unary operations
                    position: self.location_to_position(&try_expr.location),
                });
                Type::Unknown
            }
        }
    }

    /// Type check a match expression
    fn check_match_expression(&mut self, match_expr: &MatchExpression) -> Type {
        let value_type = self.check_expression(&match_expr.value);
        let mut return_type: Option<Type> = None;
        
        // Check each match arm
        for arm in &match_expr.arms {
            // Type check the pattern and bind any variables
            self.env.push_scope();
            self.check_pattern(&arm.pattern, &value_type);
            
            // Type check the expression and ensure all arms have compatible types
            let arm_type = self.check_expression(&arm.expression);
            match &return_type {
                None => return_type = Some(arm_type),
                Some(expected) => {
                    if !arm_type.is_assignable_to(expected) {
                        self.result.add_error(TypeError::TypeMismatch {
                            expected: expected.clone(),
                            actual: arm_type,
                            position: self.expression_location(&arm.expression),
                        });
                    }
                }
            }
            
            self.env.pop_scope();
        }
        
        return_type.unwrap_or(Type::Unit)
    }
}

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
    }
}