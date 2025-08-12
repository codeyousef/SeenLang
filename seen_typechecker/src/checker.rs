//! Type checker implementation for the Seen programming language

use std::collections::HashMap;
use seen_parser::ast::*;
use seen_lexer::Position;
use crate::types::Type;
use crate::errors::*;
use crate::{TypeCheckResult, FunctionSignature, Parameter};

/// Type checking environment
#[derive(Debug, Clone)]
pub struct Environment {
    /// Variables in scope with their types
    variables: HashMap<String, Type>,
    /// Functions in scope with their signatures  
    functions: HashMap<String, FunctionSignature>,
    /// User-defined types in scope
    types: HashMap<String, Type>,
    /// Parent environment for nested scopes
    parent: Option<Box<Environment>>,
    /// Smart cast information - variables that are smart-cast to non-nullable
    smart_casts: HashMap<String, Type>,
}

impl Environment {
    /// Create a new empty environment
    fn new() -> Self {
        Self {
            variables: HashMap::new(),
            functions: HashMap::new(),
            types: HashMap::new(),
            parent: None,
            smart_casts: HashMap::new(),
        }
    }

    /// Create a new environment with a parent
    fn with_parent(parent: Environment) -> Self {
        Self {
            variables: HashMap::new(),
            functions: HashMap::new(),
            types: HashMap::new(),
            parent: Some(Box::new(parent)),
            smart_casts: HashMap::new(),
        }
    }

    /// Define a variable in this environment
    pub fn define_variable(&mut self, name: String, var_type: Type) {
        self.variables.insert(name, var_type);
    }

    /// Define a function in this environment
    pub fn define_function(&mut self, name: String, signature: FunctionSignature) {
        self.functions.insert(name, signature);
    }

    /// Look up a variable type, checking smart casts first, then parent environments
    pub fn get_variable(&self, name: &str) -> Option<&Type> {
        // Check smart casts first (they take precedence)
        self.smart_casts.get(name)
            .or_else(|| self.variables.get(name))
            .or_else(|| self.parent.as_ref().and_then(|p| p.get_variable(name)))
    }

    /// Look up a function signature, checking parent environments
    pub fn get_function(&self, name: &str) -> Option<&FunctionSignature> {
        self.functions.get(name)
            .or_else(|| self.parent.as_ref().and_then(|p| p.get_function(name)))
    }

    /// Check if a variable is defined in this scope only
    pub fn has_variable(&self, name: &str) -> bool {
        self.variables.contains_key(name)
    }

    /// Check if a function is defined in this scope only
    pub fn has_function(&self, name: &str) -> bool {
        self.functions.contains_key(name)
    }

    /// Define a type in this environment
    pub fn define_type(&mut self, name: String, type_def: Type) {
        self.types.insert(name, type_def);
    }

    /// Add a smart cast for a variable (makes nullable var non-nullable in this scope)
    pub fn add_smart_cast(&mut self, name: String, smart_cast_type: Type) {
        self.smart_casts.insert(name, smart_cast_type);
    }

    /// Remove a smart cast for a variable
    fn remove_smart_cast(&mut self, name: &str) {
        self.smart_casts.remove(name);
    }

    /// Create a child environment that inherits smart casts
    fn with_smart_casts(&self) -> Environment {
        let mut child = Environment::new();
        child.parent = Some(Box::new(self.clone()));
        // Inherit smart casts from parent
        child.smart_casts = self.smart_casts.clone();
        child
    }
}

/// Main type checker
pub struct TypeChecker {
    /// Current environment
    pub env: Environment,
    /// Type checking result
    pub result: TypeCheckResult,
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
        for expression in &program.expressions {
            self.check_expression(expression);
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

    /// Type check an expression and return its type
    pub fn check_expression(&mut self, expression: &Expression) -> Type {
        match expression {
            // Literals
            Expression::IntegerLiteral { .. } => Type::Int,
            Expression::FloatLiteral { .. } => Type::Float,
            Expression::StringLiteral { .. } => Type::String,
            Expression::BooleanLiteral { .. } => Type::Bool,
            Expression::NullLiteral { .. } => Type::Nullable(Box::new(Type::Unknown)),
            
            // Identifiers
            Expression::Identifier { name, pos, .. } => {
                if let Some(var_type) = self.env.get_variable(name) {
                    var_type.clone()
                } else {
                    self.result.add_error(undefined_variable(name.clone(), *pos));
                    Type::Unknown
                }
            }
            
            // Binary operations
            Expression::BinaryOp { left, op, right, pos } => {
                self.check_binary_operation(left, op, right, *pos)
            }
            
            // Unary operations
            Expression::UnaryOp { op, operand, pos } => {
                self.check_unary_operation(op, operand, *pos)
            }
            
            // Function calls
            Expression::Call { callee, args, pos } => {
                self.check_call_expression(callee, args, *pos)
            }
            
            // Member access
            Expression::MemberAccess { object, member, is_safe, pos } => {
                self.check_member_access(object, member, *is_safe, *pos)
            }
            
            // Nullable operators
            Expression::Elvis { nullable, default, pos } => {
                self.check_elvis_operator(nullable, default, *pos)
            }
            
            Expression::ForceUnwrap { nullable, pos } => {
                self.check_force_unwrap(nullable, *pos)
            }
            
            // Control flow
            Expression::If { condition, then_branch, else_branch, pos } => {
                self.check_if_expression(condition, then_branch, else_branch.as_deref(), *pos)
            }
            
            // Blocks
            Expression::Block { expressions, .. } => {
                self.check_block_expression(expressions)
            }
            
            // Variable binding
            Expression::Let { name, type_annotation, value, pos, .. } => {
                self.check_let_expression(name, type_annotation, value, *pos)
            }
            
            // Collections
            Expression::ArrayLiteral { elements, pos } => {
                self.check_array_literal(elements, *pos)
            }
            
            Expression::StructLiteral { name, fields, pos } => {
                self.check_struct_literal(name, fields, *pos)
            }
            
            Expression::IndexAccess { object, index, pos } => {
                self.check_index_access(object, index, *pos)
            }
            
            // For now, treat other expression types as unknown
            _ => Type::Unknown
        }
    }

    /// Type check a binary operation
    fn check_binary_operation(&mut self, left: &Expression, op: &BinaryOperator, right: &Expression, pos: Position) -> Type {
        let left_type = self.check_expression(left);
        let right_type = self.check_expression(right);

        // Convert operator to string for type system
        let op_str = match op {
            BinaryOperator::Add => "+",
            BinaryOperator::Subtract => "-", 
            BinaryOperator::Multiply => "*",
            BinaryOperator::Divide => "/",
            BinaryOperator::Modulo => "%",
            BinaryOperator::Equal => "==",
            BinaryOperator::NotEqual => "!=",
            BinaryOperator::Less => "<",
            BinaryOperator::Greater => ">",
            BinaryOperator::LessEqual => "<=",
            BinaryOperator::GreaterEqual => ">=",
            BinaryOperator::And => "and",
            BinaryOperator::Or => "or",
            BinaryOperator::InclusiveRange => "..",
            BinaryOperator::ExclusiveRange => "..<",
        };

        if let Some(result_type) = left_type.binary_operation_result(op_str, &right_type) {
            result_type
        } else {
            self.result.add_error(TypeError::InvalidOperation {
                operation: op_str.to_string(),
                left_type: left_type.clone(),
                right_type: right_type.clone(),
                position: pos,
            });
            Type::Unknown
        }
    }

    /// Type check a unary operation
    fn check_unary_operation(&mut self, op: &UnaryOperator, operand: &Expression, pos: Position) -> Type {
        let operand_type = self.check_expression(operand);

        match op {
            UnaryOperator::Negate => {
                if operand_type.is_numeric() {
                    operand_type
                } else {
                    self.result.add_error(TypeError::InvalidOperation {
                        operation: "unary minus".to_string(),
                        left_type: operand_type.clone(),
                        right_type: Type::Unit,
                        position: pos,
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
                        position: pos,
                    });
                    Type::Bool
                }
            }
        }
    }

    /// Type check a function call
    fn check_call_expression(&mut self, callee: &Expression, args: &[Expression], pos: Position) -> Type {
        // Complete call checking with full type resolution
        if let Expression::Identifier { name, .. } = callee {
            if let Some(signature) = self.env.get_function(name).cloned() {
                // Check argument count
                if args.len() != signature.parameters.len() {
                    self.result.add_error(TypeError::ArgumentCountMismatch {
                        name: name.clone(),
                        expected: signature.parameters.len(),
                        actual: args.len(),
                        position: pos,
                    });
                    return signature.return_type.clone().unwrap_or(Type::Unit);
                }

                // Check argument types
                for (arg, param) in args.iter().zip(&signature.parameters) {
                    let arg_type = self.check_expression(arg);
                    if !arg_type.is_assignable_to(&param.param_type) {
                        self.result.add_error(TypeError::TypeMismatch {
                            expected: param.param_type.clone(),
                            actual: arg_type,
                            position: pos,
                        });
                    }
                }

                signature.return_type.clone().unwrap_or(Type::Unit)
            } else {
                self.result.add_error(TypeError::UndefinedFunction {
                    name: name.clone(),
                    position: pos,
                });
                Type::Unknown
            }
        } else {
            // For complex callee expressions, just type check them and assume unknown return
            self.check_expression(callee);
            for arg in args {
                self.check_expression(arg);
            }
            Type::Unknown
        }
    }

    /// Type check member access
    fn check_member_access(&mut self, object: &Expression, member: &str, is_safe: bool, pos: Position) -> Type {
        let object_type = self.check_expression(object);
        
        match &object_type {
            Type::Struct { fields, .. } => {
                if let Some(field_type) = fields.get(member) {
                    if is_safe && object_type.is_nullable() {
                        field_type.clone().nullable()
                    } else {
                        field_type.clone()
                    }
                } else {
                    self.result.add_error(TypeError::UnknownType {
                        name: format!("field '{}' not found", member),
                        position: pos,
                    });
                    Type::Unknown
                }
            }
            Type::Nullable(inner) if is_safe => {
                // Safe navigation on nullable type
                if let Type::Struct { fields, .. } = inner.as_ref() {
                    if let Some(field_type) = fields.get(member) {
                        field_type.clone().nullable()
                    } else {
                        Type::Unknown
                    }
                } else {
                    Type::Unknown
                }
            }
            _ => {
                if !is_safe {
                    self.result.add_error(TypeError::InvalidOperation {
                        operation: "field access".to_string(),
                        left_type: object_type,
                        right_type: Type::String,
                        position: pos,
                    });
                }
                Type::Unknown
            }
        }
    }

    /// Type check Elvis operator
    fn check_elvis_operator(&mut self, nullable: &Expression, default: &Expression, pos: Position) -> Type {
        let nullable_type = self.check_expression(nullable);
        let default_type = self.check_expression(default);

        // Elvis operator unwraps nullable and provides default
        match nullable_type {
            Type::Nullable(inner) => {
                if default_type.is_assignable_to(&inner) {
                    *inner
                } else {
                    self.result.add_error(TypeError::TypeMismatch {
                        expected: *inner,
                        actual: default_type,
                        position: pos,
                    });
                    Type::Unknown
                }
            }
            _ => {
                // Not nullable, return the original type
                nullable_type
            }
        }
    }

    /// Type check force unwrap
    fn check_force_unwrap(&mut self, nullable: &Expression, _pos: Position) -> Type {
        let nullable_type = self.check_expression(nullable);

        match nullable_type {
            Type::Nullable(inner) => *inner,
            _ => {
                // Force unwrap on non-nullable is just the original type
                nullable_type
            }
        }
    }

    /// Type check if expression with smart casting support
    pub fn check_if_expression(&mut self, condition: &Expression, then_branch: &Expression, else_branch: Option<&Expression>, pos: Position) -> Type {
        let condition_type = self.check_expression(condition);
        if !condition_type.is_assignable_to(&Type::Bool) {
            self.result.add_error(TypeError::TypeMismatch {
                expected: Type::Bool,
                actual: condition_type,
                position: pos,
            });
        }

        // Analyze condition for smart casting opportunities
        let smart_casts = self.analyze_condition_for_smart_casts(condition);
        
        // Type check then branch with smart casts applied
        let then_type = {
            let old_env = self.env.clone();
            // Apply smart casts for then branch
            for (var_name, cast_type) in &smart_casts {
                self.env.add_smart_cast(var_name.clone(), cast_type.clone());
            }
            let then_type = self.check_expression(then_branch);
            // Restore original environment for else branch
            self.env = old_env;
            then_type
        };
        
        if let Some(else_expr) = else_branch {
            // Type check else branch without smart casts (original types)
            let else_type = self.check_expression(else_expr);
            if then_type.is_assignable_to(&else_type) {
                else_type
            } else if else_type.is_assignable_to(&then_type) {
                then_type
            } else {
                // Types don't match, return Union or Unknown
                Type::Unknown
            }
        } else {
            // If without else returns Unit if then branch also returns Unit
            if matches!(then_type, Type::Unit) {
                Type::Unit
            } else {
                // If with non-unit then branch but no else becomes Optional
                Type::Nullable(Box::new(then_type))
            }
        }
    }

    /// Analyze a condition expression for smart casting opportunities
    /// Returns a map of variable names to their smart-cast types
    fn analyze_condition_for_smart_casts(&mut self, condition: &Expression) -> HashMap<String, Type> {
        let mut smart_casts = HashMap::new();
        
        match condition {
            // Handle: if variable != null
            Expression::BinaryOp { left, op: BinaryOperator::NotEqual, right, .. } => {
                if let (Expression::Identifier { name, .. }, Expression::NullLiteral { .. }) = (left.as_ref(), right.as_ref()) {
                    if let Some(var_type) = self.env.get_variable(name).cloned() {
                        if let Type::Nullable(inner_type) = var_type {
                            smart_casts.insert(name.clone(), *inner_type);
                        }
                    }
                } else if let (Expression::NullLiteral { .. }, Expression::Identifier { name, .. }) = (left.as_ref(), right.as_ref()) {
                    if let Some(var_type) = self.env.get_variable(name).cloned() {
                        if let Type::Nullable(inner_type) = var_type {
                            smart_casts.insert(name.clone(), *inner_type);
                        }
                    }
                }
            }
            
            // Handle: if variable (implicit null check for Bool?)
            Expression::Identifier { name, .. } => {
                if let Some(var_type) = self.env.get_variable(name).cloned() {
                    if let Type::Nullable(inner_type) = var_type {
                        if matches!(inner_type.as_ref(), Type::Bool) {
                            smart_casts.insert(name.clone(), *inner_type);
                        }
                    }
                }
            }
            
            // Handle compound conditions with 'and': if x != null and y != null
            Expression::BinaryOp { left, op: BinaryOperator::And, right, .. } => {
                let left_casts = self.analyze_condition_for_smart_casts(left);
                let right_casts = self.analyze_condition_for_smart_casts(right);
                smart_casts.extend(left_casts);
                smart_casts.extend(right_casts);
            }
            
            _ => {
                // Other condition types don't provide smart casting opportunities
            }
        }
        
        smart_casts
    }

    /// Type check block expression
    fn check_block_expression(&mut self, expressions: &[Expression]) -> Type {
        if expressions.is_empty() {
            return Type::Unit;
        }

        let mut result_type = Type::Unit;
        for expr in expressions {
            result_type = self.check_expression(expr);
        }
        result_type
    }

    /// Type check let expression
    fn check_let_expression(&mut self, name: &str, type_annotation: &Option<seen_parser::ast::Type>, value: &Expression, pos: Position) -> Type {
        let value_type = self.check_expression(value);
        
        let declared_type = if let Some(type_ann) = type_annotation {
            let declared = Type::from(type_ann);
            if !value_type.is_assignable_to(&declared) {
                self.result.add_error(TypeError::TypeMismatch {
                    expected: declared.clone(),
                    actual: value_type,
                    position: pos,
                });
            }
            declared
        } else {
            value_type.clone()
        };

        // Check for duplicate variable
        if self.env.has_variable(name) {
            self.result.add_error(TypeError::DuplicateVariable {
                name: name.to_string(),
                position: pos,
            });
        } else {
            self.env.define_variable(name.to_string(), declared_type.clone());
        }

        // Let expressions return the bound value
        declared_type
    }

    /// Type check array literal
    fn check_array_literal(&mut self, elements: &[Expression], pos: Position) -> Type {
        if elements.is_empty() {
            return Type::Array(Box::new(Type::Unknown));
        }

        let element_type = self.check_expression(&elements[0]);
        for element in &elements[1..] {
            let elem_type = self.check_expression(element);
            if !elem_type.is_assignable_to(&element_type) {
                self.result.add_error(TypeError::TypeMismatch {
                    expected: element_type.clone(),
                    actual: elem_type,
                    position: pos,
                });
            }
        }

        Type::Array(Box::new(element_type))
    }

    /// Type check struct literal
    fn check_struct_literal(&mut self, name: &str, fields: &[(String, Expression)], pos: Position) -> Type {
        // Check if the struct type exists
        if let Some(struct_type) = self.env.types.get(name).cloned() {
            // Type check each field initialization
            for (_, value) in fields {
                self.check_expression(value);
            }
            struct_type
        } else {
            self.result.add_error(TypeError::UnknownType {
                name: name.to_string(),
                position: pos,
            });
            Type::Unknown
        }
    }

    /// Type check index access
    fn check_index_access(&mut self, object: &Expression, index: &Expression, pos: Position) -> Type {
        let array_type = self.check_expression(object);
        let index_type = self.check_expression(index);

        if !index_type.is_assignable_to(&Type::Int) {
            self.result.add_error(TypeError::InvalidIndexType {
                actual_type: index_type,
                position: pos,
            });
        }

        match array_type {
            Type::Array(element_type) => *element_type,
            _ => {
                self.result.add_error(TypeError::InvalidOperation {
                    operation: "indexing".to_string(),
                    left_type: array_type,
                    right_type: Type::Int,
                    position: pos,
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