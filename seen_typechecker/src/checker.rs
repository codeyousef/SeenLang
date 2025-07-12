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
    /// Parent environment for nested scopes
    parent: Option<Box<Environment>>,
}

impl Environment {
    /// Create a new empty environment
    fn new() -> Self {
        Self {
            variables: HashMap::new(),
            functions: HashMap::new(),
            parent: None,
        }
    }

    /// Create a new environment with a parent
    fn with_parent(parent: Environment) -> Self {
        Self {
            variables: HashMap::new(),
            functions: HashMap::new(),
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

    /// Check if a variable is defined in this scope only
    fn has_variable(&self, name: &str) -> bool {
        self.variables.contains_key(name)
    }

    /// Check if a function is defined in this scope only
    fn has_function(&self, name: &str) -> bool {
        self.functions.contains_key(name)
    }

    /// Push a new scope
    fn push_scope(&mut self) {
        let new_env = Environment {
            variables: HashMap::new(),
            functions: HashMap::new(),
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
        // For now, just register the struct type
        // TODO: Implement proper struct field type checking
        let struct_type = Type::Struct(struct_decl.name.clone());
        // We would register this in a type environment if we had one
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
            .map(Type::from)
            .unwrap_or(Type::Unit);

        // Create function signature
        let signature = FunctionSignature {
            name: func_decl.name.clone(),
            parameters: param_types.clone(),
            return_type: Some(return_type.clone()),
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
        let old_return_type = std::mem::replace(&mut self.current_function_return_type, Some(return_type));

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
            Expression::StructLiteral(_) => {
                // TODO: Implement struct literal type checking
                Type::Unknown
            }
            Expression::FieldAccess(_) => {
                // TODO: Implement field access type checking
                Type::Unknown
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
                self.result.add_error(TypeError::ArgumentCountMismatch {
                    name: call.callee.clone(),
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
        };
        self.location_to_position(location)
    }

    /// Convert Location to Position (for compatibility)
    fn location_to_position(&self, location: &Location) -> seen_lexer::token::Position {
        seen_lexer::token::Position::new(location.start.line, location.start.column)
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
}

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
    }
}