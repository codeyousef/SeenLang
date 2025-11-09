//! Type checker implementation for the Seen programming language

use crate::errors::*;
use crate::types::Type;
use crate::{FunctionSignature, Parameter, TypeCheckResult};
use seen_lexer::Position;
use seen_parser::ast::*;
use std::collections::HashMap;

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

    /// Define a type in this environment
    pub fn define_type(&mut self, name: String, type_def: Type) {
        self.types.insert(name, type_def);
    }

    /// Look up a type definition
    pub fn get_type(&self, name: &str) -> Option<&Type> {
        self.types
            .get(name)
            .or_else(|| self.parent.as_ref().and_then(|p| p.get_type(name)))
    }

    /// Look up a variable type, checking smart casts first, then parent environments
    pub fn get_variable(&self, name: &str) -> Option<&Type> {
        // Check smart casts first (they take precedence)
        self.smart_casts
            .get(name)
            .or_else(|| self.variables.get(name))
            .or_else(|| self.parent.as_ref().and_then(|p| p.get_variable(name)))
    }

    /// Look up a function signature, checking parent environments
    pub fn get_function(&self, name: &str) -> Option<&FunctionSignature> {
        self.functions
            .get(name)
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
    /// Stack of in-scope generic parameter names
    generic_stack: Vec<Vec<String>>,
    /// Depth of structured concurrency scopes
    scope_depth: usize,
}

impl TypeChecker {
    /// Create a new type checker
    pub fn new() -> Self {
        let mut env = Environment::new();

        // Add built-in functions
        env.define_function(
            "println".to_string(),
            FunctionSignature {
                name: "println".to_string(),
                parameters: vec![Parameter {
                    name: "value".to_string(),
                    param_type: Type::String,
                }],
                return_type: Some(Type::Unit),
            },
        );

        // Built-ins used by bootstrap/self-host sources
        env.define_function(
            "CompileSeenProgram".to_string(),
            FunctionSignature {
                name: "CompileSeenProgram".to_string(),
                parameters: vec![
                    Parameter {
                        name: "source".to_string(),
                        param_type: Type::String,
                    },
                    Parameter {
                        name: "output".to_string(),
                        param_type: Type::String,
                    },
                ],
                return_type: Some(Type::Bool),
            },
        );

        // System/IO builtins used by self-host sources (double-underscore forms)
        env.define_function(
            "__GetCommandLineArgs".to_string(),
            FunctionSignature {
                name: "__GetCommandLineArgs".to_string(),
                parameters: vec![],
                return_type: Some(Type::Array(Box::new(Type::String))),
            },
        );
        env.define_function(
            "__GetTimestamp".to_string(),
            FunctionSignature {
                name: "__GetTimestamp".to_string(),
                parameters: vec![],
                return_type: Some(Type::String),
            },
        );
        env.define_function(
            "__ReadFile".to_string(),
            FunctionSignature {
                name: "__ReadFile".to_string(),
                parameters: vec![Parameter {
                    name: "path".to_string(),
                    param_type: Type::String,
                }],
                return_type: Some(Type::String),
            },
        );
        env.define_function(
            "__WriteFile".to_string(),
            FunctionSignature {
                name: "__WriteFile".to_string(),
                parameters: vec![
                    Parameter {
                        name: "path".to_string(),
                        param_type: Type::String,
                    },
                    Parameter {
                        name: "content".to_string(),
                        param_type: Type::String,
                    },
                ],
                return_type: Some(Type::Bool),
            },
        );
        env.define_function(
            "__CreateDirectory".to_string(),
            FunctionSignature {
                name: "__CreateDirectory".to_string(),
                parameters: vec![Parameter {
                    name: "path".to_string(),
                    param_type: Type::String,
                }],
                return_type: Some(Type::Bool),
            },
        );
        env.define_function(
            "__DeleteFile".to_string(),
            FunctionSignature {
                name: "__DeleteFile".to_string(),
                parameters: vec![Parameter {
                    name: "path".to_string(),
                    param_type: Type::String,
                }],
                return_type: Some(Type::Bool),
            },
        );
        env.define_function(
            "__ExecuteProgram".to_string(),
            FunctionSignature {
                name: "__ExecuteProgram".to_string(),
                parameters: vec![Parameter {
                    name: "path".to_string(),
                    param_type: Type::String,
                }],
                return_type: Some(Type::Int),
            },
        );
        // Return type models CommandResult { success: Bool, output: String }
        env.define_function(
            "__ExecuteCommand".to_string(),
            FunctionSignature {
                name: "__ExecuteCommand".to_string(),
                parameters: vec![Parameter {
                    name: "command".to_string(),
                    param_type: Type::String,
                }],
                return_type: Some(Type::Struct {
                    name: "CommandResult".to_string(),
                    fields: {
                        let mut m = std::collections::HashMap::new();
                        m.insert("success".to_string(), Type::Bool);
                        m.insert("output".to_string(), Type::String);
                        m
                    },
                    generics: Vec::new(),
                }),
            },
        );
        env.define_function(
            "__FormatSeenCode".to_string(),
            FunctionSignature {
                name: "__FormatSeenCode".to_string(),
                parameters: vec![Parameter {
                    name: "source".to_string(),
                    param_type: Type::String,
                }],
                return_type: Some(Type::String),
            },
        );
        env.define_function(
            "__Abort".to_string(),
            FunctionSignature {
                name: "__Abort".to_string(),
                parameters: vec![Parameter {
                    name: "message".to_string(),
                    param_type: Type::String,
                }],
                return_type: Some(Type::Unit),
            },
        );
        let channel_generic_type = Type::Struct {
            name: "Channel".to_string(),
            fields: HashMap::new(),
            generics: vec![Type::Generic("T".to_string())],
        };

        let mut channel_endpoints_fields_generic = HashMap::new();
        channel_endpoints_fields_generic.insert("Sender".to_string(), channel_generic_type.clone());
        channel_endpoints_fields_generic
            .insert("Receiver".to_string(), channel_generic_type.clone());

        let channel_endpoints_generic = Type::Struct {
            name: "ChannelEndpoints".to_string(),
            fields: channel_endpoints_fields_generic,
            generics: vec![Type::Generic("T".to_string())],
        };

        let mut channel_endpoints_fields_unknown = HashMap::new();
        channel_endpoints_fields_unknown.insert(
            "Sender".to_string(),
            Type::Struct {
                name: "Channel".to_string(),
                fields: HashMap::new(),
                generics: vec![Type::Unknown],
            },
        );
        channel_endpoints_fields_unknown.insert(
            "Receiver".to_string(),
            Type::Struct {
                name: "Channel".to_string(),
                fields: HashMap::new(),
                generics: vec![Type::Unknown],
            },
        );

        let channel_endpoints_return = Type::Struct {
            name: "ChannelEndpoints".to_string(),
            fields: channel_endpoints_fields_unknown,
            generics: vec![Type::Unknown],
        };

        env.define_function(
            "Channel".to_string(),
            FunctionSignature {
                name: "Channel".to_string(),
                parameters: Vec::new(),
                return_type: Some(channel_endpoints_return),
            },
        );

        // Built-in Phantom type for typestate modeling
        let phantom_type = Type::Struct {
            name: "Phantom".to_string(),
            fields: HashMap::new(),
            generics: vec![Type::Generic("T".to_string())],
        };
        env.define_type("Phantom".to_string(), phantom_type);

        env.define_type("Channel".to_string(), channel_generic_type);
        env.define_type("ChannelEndpoints".to_string(), channel_endpoints_generic);

        Self {
            env,
            result: TypeCheckResult::new(),
            current_function_return_type: None,
            generic_stack: Vec::new(),
            scope_depth: 0,
        }
    }

    /// Predeclare all top-level function signatures for forward references
    fn predeclare_signatures(&mut self, program: &Program) {
        for expr in &program.expressions {
            if let Expression::Function {
                name,
                params,
                return_type,
                ..
            } = expr
            {
                // Build parameter types
                let mut checker_params = Vec::new();
                for p in params {
                    let pty = if let Some(ta) = &p.type_annotation {
                        self.resolve_ast_type(ta, Position::start())
                    } else {
                        Type::Unknown
                    };
                    checker_params.push(crate::Parameter {
                        name: p.name.clone(),
                        param_type: pty,
                    });
                }
                // Return type (default Unit)
                let ret = return_type
                    .as_ref()
                    .map(|t| self.resolve_ast_type(t, Position::start()))
                    .or(Some(Type::Unit));
                let sig = FunctionSignature {
                    name: name.clone(),
                    parameters: checker_params,
                    return_type: ret,
                };
                if !self.env.has_function(name) {
                    self.env.define_function(name.clone(), sig);
                }
            }
        }
    }

    fn with_generics<F, R>(&mut self, generics: &[String], f: F) -> R
    where
        F: FnOnce(&mut Self) -> R,
    {
        if !generics.is_empty() {
            self.generic_stack.push(generics.to_vec());
            let result = f(self);
            self.generic_stack.pop();
            result
        } else {
            f(self)
        }
    }

    fn is_generic_name(&self, name: &str) -> bool {
        self.generic_stack
            .iter()
            .rev()
            .any(|scope| scope.iter().any(|g| g == name))
    }

    fn resolve_ast_type(&mut self, ast_type: &seen_parser::Type, pos: Position) -> Type {
        if self.is_generic_name(&ast_type.name) && ast_type.generics.is_empty() {
            let base = Type::Generic(ast_type.name.clone());
            return if ast_type.is_nullable {
                Type::Nullable(Box::new(base))
            } else {
                base
            };
        }

        let resolved_args: Vec<Type> = ast_type
            .generics
            .iter()
            .map(|g| self.resolve_ast_type(g, pos))
            .collect();

        let mut base = match ast_type.name.as_str() {
            "Int" => Type::Int,
            "UInt" => Type::UInt,
            "Float" => Type::Float,
            "Bool" => Type::Bool,
            "String" => Type::String,
            "Char" => Type::Char,
            "()" => Type::Unit,
            "Array" | "List" | "Vec" => {
                if resolved_args.len() == 1 {
                    Type::Array(Box::new(resolved_args[0].clone()))
                } else {
                    self.result.add_error(TypeError::GenericArityMismatch {
                        type_name: ast_type.name.clone(),
                        expected: 1,
                        actual: resolved_args.len(),
                        position: pos,
                    });
                    Type::Unknown
                }
            }
            _ => {
                if let Some(def) = self.env.get_type(&ast_type.name).cloned() {
                    return self.instantiate_type(def, &resolved_args, pos);
                }

                Type::Struct {
                    name: ast_type.name.clone(),
                    fields: HashMap::new(),
                    generics: resolved_args.clone(),
                }
            }
        };

        if ast_type.is_nullable {
            base = Type::Nullable(Box::new(base));
        }

        base
    }

    fn instantiate_type(&mut self, definition: Type, args: &[Type], pos: Position) -> Type {
        match definition {
            Type::Struct {
                name,
                fields,
                generics,
            } => {
                if generics.len() != args.len() {
                    self.result.add_error(TypeError::GenericArityMismatch {
                        type_name: name.clone(),
                        expected: generics.len(),
                        actual: args.len(),
                        position: pos,
                    });
                    return Type::Unknown;
                }

                let mut mapping = HashMap::new();
                for (param, arg) in generics.iter().zip(args.iter()) {
                    if let Type::Generic(param_name) = param {
                        mapping.insert(param_name.clone(), arg.clone());
                    }
                }

                let substituted_fields = fields
                    .into_iter()
                    .map(|(field, ty)| (field, self.substitute_generics(&ty, &mapping)))
                    .collect();

                Type::Struct {
                    name,
                    fields: substituted_fields,
                    generics: args.to_vec(),
                }
            }
            Type::Enum {
                name,
                variants,
                generics,
            } => {
                if generics.len() != args.len() {
                    self.result.add_error(TypeError::GenericArityMismatch {
                        type_name: name.clone(),
                        expected: generics.len(),
                        actual: args.len(),
                        position: pos,
                    });
                    return Type::Unknown;
                }

                Type::Enum {
                    name,
                    variants,
                    generics: args.to_vec(),
                }
            }
            Type::Interface {
                name,
                methods,
                generics,
                is_sealed,
            } => {
                if generics.len() != args.len() {
                    self.result.add_error(TypeError::GenericArityMismatch {
                        type_name: name.clone(),
                        expected: generics.len(),
                        actual: args.len(),
                        position: pos,
                    });
                    return Type::Unknown;
                }

                Type::Interface {
                    name,
                    methods,
                    generics: args.to_vec(),
                    is_sealed,
                }
            }
            other => other,
        }
    }

    fn substitute_generics(&self, ty: &Type, mapping: &HashMap<String, Type>) -> Type {
        match ty {
            Type::Generic(name) => mapping.get(name).cloned().unwrap_or_else(|| ty.clone()),
            Type::Array(inner) => Type::Array(Box::new(self.substitute_generics(inner, mapping))),
            Type::Nullable(inner) => {
                Type::Nullable(Box::new(self.substitute_generics(inner, mapping)))
            }
            Type::Struct {
                name,
                fields,
                generics,
            } => {
                let new_fields = fields
                    .iter()
                    .map(|(field, ty)| (field.clone(), self.substitute_generics(ty, mapping)))
                    .collect();
                let new_generics = generics
                    .iter()
                    .map(|g| self.substitute_generics(g, mapping))
                    .collect();
                Type::Struct {
                    name: name.clone(),
                    fields: new_fields,
                    generics: new_generics,
                }
            }
            Type::Enum {
                name,
                variants,
                generics,
            } => {
                let new_generics = generics
                    .iter()
                    .map(|g| self.substitute_generics(g, mapping))
                    .collect();
                Type::Enum {
                    name: name.clone(),
                    variants: variants.clone(),
                    generics: new_generics,
                }
            }
            Type::Interface {
                name,
                methods,
                generics,
                is_sealed,
            } => {
                let new_generics = generics
                    .iter()
                    .map(|g| self.substitute_generics(g, mapping))
                    .collect();
                Type::Interface {
                    name: name.clone(),
                    methods: methods.clone(),
                    generics: new_generics,
                    is_sealed: *is_sealed,
                }
            }
            Type::Function {
                params,
                return_type,
                is_async,
            } => {
                let new_params: Vec<Type> = params
                    .iter()
                    .map(|p| self.substitute_generics(p, mapping))
                    .collect();
                let new_return = self.substitute_generics(return_type, mapping);
                Type::Function {
                    params: new_params,
                    return_type: Box::new(new_return),
                    is_async: *is_async,
                }
            }
            _ => ty.clone(),
        }
    }

    /// Type check a program
    pub fn check_program(&mut self, program: &Program) -> TypeCheckResult {
        // Predeclare function signatures so that order of definitions doesn't matter
        self.predeclare_signatures(program);
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
            // Import declarations are compile-time only; no runtime type
            Expression::Import { .. } => Type::Unit,
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
                    self.result
                        .add_error(undefined_variable(name.clone(), *pos));
                    Type::Unknown
                }
            }

            // Binary operations
            Expression::BinaryOp {
                left,
                op,
                right,
                pos,
            } => self.check_binary_operation(left, op, right, *pos),

            // Unary operations
            Expression::UnaryOp { op, operand, pos } => {
                self.check_unary_operation(op, operand, *pos)
            }

            // Function calls
            Expression::Call { callee, args, pos } => {
                self.check_call_expression(callee, args, *pos)
            }

            // Member access
            Expression::MemberAccess {
                object,
                member,
                is_safe,
                pos,
            } => self.check_member_access(object, member, *is_safe, *pos),

            // Nullable operators
            Expression::Elvis {
                nullable,
                default,
                pos,
            } => self.check_elvis_operator(nullable, default, *pos),

            Expression::ForceUnwrap { nullable, pos } => self.check_force_unwrap(nullable, *pos),

            // Struct definition
            Expression::StructDefinition {
                name,
                generics,
                fields,
                pos,
                ..
            } => self.check_struct_definition(name, generics, fields, *pos),

            // Struct literal
            Expression::StructLiteral { name, fields, pos } => {
                self.check_struct_literal(name, fields, *pos)
            }

            // Control flow
            Expression::If {
                condition,
                then_branch,
                else_branch,
                pos,
            } => self.check_if_expression(condition, then_branch, else_branch.as_deref(), *pos),

            // Structured concurrency primitives
            Expression::Await { expr, pos } => self.check_await_expression(expr, *pos),

            Expression::Spawn {
                expr,
                detached,
                pos,
            } => self.check_spawn_expression(expr, *detached, *pos),

            Expression::Scope { body, pos } => self.check_scope_expression(body, *pos),
            Expression::JobsScope { body, pos } => self.check_jobs_scope(body, *pos),

            Expression::Cancel { task, pos } => self.check_cancel_expression(task, *pos),

            Expression::ParallelFor {
                binding,
                iterable,
                body,
                pos,
            } => self.check_parallel_for(binding, iterable, body, *pos),
            Expression::Send {
                target,
                message,
                pos,
            } => self.check_send_expression(target, message, *pos),
            Expression::Receive { handler, .. } => self.check_expression(handler),
            Expression::Select { cases, pos } => self.check_select_expression(cases, *pos),

            // Blocks
            Expression::Block { expressions, .. } => self.check_block_expression(expressions),

            // Variable binding
            Expression::Let {
                name,
                type_annotation,
                value,
                pos,
                ..
            } => self.check_let_expression(name, type_annotation, value, *pos),

            // Collections
            Expression::ArrayLiteral { elements, pos } => self.check_array_literal(elements, *pos),

            Expression::StructLiteral { name, fields, pos } => {
                self.check_struct_literal(name, fields, *pos)
            }

            Expression::IndexAccess { object, index, pos } => {
                self.check_index_access(object, index, *pos)
            }

            // Function definition
            Expression::Function {
                name,
                generics,
                params,
                return_type,
                body,
                pos,
                ..
            } => self.check_function_definition(name, generics, params, return_type, body, *pos),

            // Interface definition
            Expression::Interface {
                name,
                generics,
                methods,
                is_sealed,
                pos,
            } => self.check_interface_definition(name, generics, methods, *is_sealed, *pos),

            Expression::Extension {
                target_type,
                methods,
                pos,
            } => self.check_extension(target_type, methods, *pos),

            // For now, treat other expression types as unknown
            _ => Type::Unknown,
        }
    }

    /// Type check a binary operation
    fn check_binary_operation(
        &mut self,
        left: &Expression,
        op: &BinaryOperator,
        right: &Expression,
        pos: Position,
    ) -> Type {
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
    fn check_unary_operation(
        &mut self,
        op: &UnaryOperator,
        operand: &Expression,
        pos: Position,
    ) -> Type {
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

    fn check_await_expression(&mut self, expr: &Expression, pos: Position) -> Type {
        let awaited_type = self.check_expression(expr);
        match awaited_type.non_nullable() {
            Type::Task(inner) => inner.as_ref().clone(),
            Type::Struct { name, generics, .. } if name == "Promise" && !generics.is_empty() => {
                generics[0].clone()
            }
            Type::Struct { name, .. } if name == "Promise" => Type::Unknown,
            _ => {
                self.result.add_error(TypeError::InvalidAwaitTarget {
                    actual: awaited_type.clone(),
                    position: pos,
                });
                Type::Unknown
            }
        }
    }

    fn check_spawn_expression(&mut self, expr: &Expression, detached: bool, pos: Position) -> Type {
        let payload_type = self.check_expression(expr);
        if !detached && self.scope_depth == 0 {
            self.result
                .add_error(TypeError::TaskRequiresScope { position: pos });
        }
        Type::Task(Box::new(payload_type))
    }

    fn check_scope_expression(&mut self, body: &Expression, _pos: Position) -> Type {
        self.scope_depth += 1;
        let result = self.check_expression(body);
        self.scope_depth -= 1;
        result
    }

    fn check_jobs_scope(&mut self, body: &Expression, pos: Position) -> Type {
        // jobs.scope shares the same structured concurrency semantics as scope for now.
        self.check_scope_expression(body, pos)
    }

    fn check_cancel_expression(&mut self, task: &Expression, pos: Position) -> Type {
        let task_type = self.check_expression(task);
        if matches!(task_type.non_nullable(), Type::Task(_)) {
            Type::Bool
        } else {
            self.result.add_error(TypeError::CancelRequiresTask {
                actual: task_type.clone(),
                position: pos,
            });
            Type::Bool
        }
    }

    fn check_parallel_for(
        &mut self,
        binding: &str,
        iterable: &Expression,
        body: &Expression,
        pos: Position,
    ) -> Type {
        let iterable_type = self.check_expression(iterable);
        let element_type = match iterable_type.non_nullable() {
            Type::Array(inner) => inner.as_ref().clone(),
            Type::String => Type::Char,
            other => {
                self.result.add_error(TypeError::InvalidOperation {
                    operation: "parallel_for iterable".to_string(),
                    left_type: iterable_type.clone(),
                    right_type: Type::Unit,
                    position: pos,
                });
                other.clone()
            }
        };

        let saved_env = self.env.clone();
        let mut loop_env = Environment::with_parent(self.env.clone());
        loop_env.define_variable(binding.to_string(), element_type);
        self.env = loop_env;

        let body_type = self.check_expression(body);

        self.env = saved_env;

        if !body_type.is_assignable_to(&Type::Unit) {
            self.result.add_error(TypeError::InvalidOperation {
                operation: "parallel_for body".to_string(),
                left_type: body_type,
                right_type: Type::Unit,
                position: pos,
            });
        }

        Type::Unit
    }

    fn check_send_expression(
        &mut self,
        target: &Expression,
        message: &Expression,
        pos: Position,
    ) -> Type {
        let target_type = self.check_expression(target);
        let _ = self.check_expression(message);

        let promise_bool = Type::Struct {
            name: "Promise".to_string(),
            fields: HashMap::new(),
            generics: vec![Type::Bool],
        };

        match target_type.non_nullable() {
            Type::Struct { name, .. } if name == "Channel" => promise_bool,
            Type::Unknown => promise_bool,
            _ => {
                let expected_channel = Type::Struct {
                    name: "Channel".to_string(),
                    fields: HashMap::new(),
                    generics: vec![Type::Unknown],
                };
                self.result.add_error(TypeError::TypeMismatch {
                    expected: expected_channel,
                    actual: target_type.clone(),
                    position: pos,
                });
                promise_bool
            }
        }
    }

    fn check_select_expression(&mut self, cases: &[SelectCase], pos: Position) -> Type {
        if cases.is_empty() {
            self.result.add_error(TypeError::InvalidOperation {
                operation: "select".to_string(),
                left_type: Type::Unit,
                right_type: Type::Unit,
                position: pos,
            });
            return Type::Unit;
        }

        let mut accumulated: Option<Type> = None;
        let expected_channel = Type::Struct {
            name: "Channel".to_string(),
            fields: HashMap::new(),
            generics: vec![Type::Unknown],
        };

        for case in cases {
            let channel_type = self.check_expression(&case.channel);
            if !matches!(channel_type.non_nullable(), Type::Struct { name, .. } if name == "Channel")
            {
                self.result.add_error(TypeError::TypeMismatch {
                    expected: expected_channel.clone(),
                    actual: channel_type.clone(),
                    position: pos,
                });
            }

            let saved_env = self.env.clone();
            self.env = Environment::with_parent(saved_env.clone());
            self.bind_pattern(&case.pattern, Type::Unknown);
            let handler_type = self.check_expression(&case.handler);
            self.env = saved_env;

            if let Some(expected) = &accumulated {
                if !handler_type.is_assignable_to(expected) {
                    self.result.add_error(TypeError::TypeMismatch {
                        expected: expected.clone(),
                        actual: handler_type.clone(),
                        position: pos,
                    });
                }
            } else {
                accumulated = Some(handler_type.clone());
            }
        }

        accumulated.unwrap_or(Type::Unit)
    }

    fn bind_pattern(&mut self, pattern: &Pattern, ty: Type) {
        match pattern {
            Pattern::Identifier(name) => {
                self.env.define_variable(name.clone(), ty);
            }
            Pattern::Wildcard => {}
            Pattern::Array(elements) => {
                for element in elements {
                    self.bind_pattern(element.as_ref(), Type::Unknown);
                }
            }
            Pattern::Struct { fields, .. } => {
                for (_, field_pattern) in fields {
                    self.bind_pattern(field_pattern.as_ref(), Type::Unknown);
                }
            }
            Pattern::Enum { fields, .. } => {
                for field_pattern in fields {
                    self.bind_pattern(field_pattern.as_ref(), Type::Unknown);
                }
            }
            Pattern::Range { .. } | Pattern::Literal(_) => {}
        }
    }

    /// Type check a function call
    fn check_call_expression(
        &mut self,
        callee: &Expression,
        args: &[Expression],
        pos: Position,
    ) -> Type {
        // Complete call checking with full type resolution
        if let Expression::Identifier { name, .. } = callee {
            if name == "Channel" {
                if args.len() > 1 {
                    self.result.add_error(TypeError::ArgumentCountMismatch {
                        name: name.clone(),
                        expected: 1,
                        actual: args.len(),
                        position: pos,
                    });
                }

                if let Some(arg) = args.get(0) {
                    let capacity_type = self.check_expression(arg);
                    if !capacity_type.is_assignable_to(&Type::Int) {
                        self.result.add_error(TypeError::TypeMismatch {
                            expected: Type::Int,
                            actual: capacity_type,
                            position: pos,
                        });
                    }
                }

                let mut fields = HashMap::new();
                fields.insert(
                    "Sender".to_string(),
                    Type::Struct {
                        name: "Channel".to_string(),
                        fields: HashMap::new(),
                        generics: vec![Type::Unknown],
                    },
                );
                fields.insert(
                    "Receiver".to_string(),
                    Type::Struct {
                        name: "Channel".to_string(),
                        fields: HashMap::new(),
                        generics: vec![Type::Unknown],
                    },
                );

                return Type::Struct {
                    name: "ChannelEndpoints".to_string(),
                    fields,
                    generics: vec![Type::Unknown],
                };
            }

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
        } else if let Expression::MemberAccess { object, member, .. } = callee {
            // Method-style call like array.size() or string.length()
            let recv_ty = self.check_expression(object);
            // Validate no arguments for simple accessors
            for arg in args {
                let _ = self.check_expression(arg);
            }
            let base = recv_ty.non_nullable().clone();
            match (&base, member.as_str()) {
                (Type::Array(_), "size") | (Type::Array(_), "length") => Type::Int,
                (Type::String, "size") | (Type::String, "length") => Type::Int,
                _ => {
                    // Unknown method; treat as unknown return
                    Type::Unknown
                }
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
    fn check_member_access(
        &mut self,
        object: &Expression,
        member: &str,
        is_safe: bool,
        pos: Position,
    ) -> Type {
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
                    self.result.add_error(TypeError::UnknownField {
                        struct_name: self.extract_struct_name_from_type(&object_type),
                        field_name: member.to_string(),
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
    fn check_elvis_operator(
        &mut self,
        nullable: &Expression,
        default: &Expression,
        pos: Position,
    ) -> Type {
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

    /// Type check struct definition
    fn check_struct_definition(
        &mut self,
        name: &str,
        generics: &[String],
        fields: &[seen_parser::ast::StructField],
        pos: Position,
    ) -> Type {
        let struct_type = self.with_generics(generics, |checker| {
            let mut field_types = HashMap::new();
            for field in fields {
                let field_type = checker.resolve_ast_type(&field.field_type, pos);
                field_types.insert(field.name.clone(), field_type);
            }

            Type::Struct {
                name: name.to_string(),
                fields: field_types,
                generics: generics.iter().map(|g| Type::Generic(g.clone())).collect(),
            }
        });

        self.env.define_type(name.to_string(), struct_type);
        Type::Unit
    }

    /// Type check struct literal
    fn check_struct_literal(
        &mut self,
        name: &str,
        fields: &[(String, Expression)],
        pos: Position,
    ) -> Type {
        // Look up and clone the struct type to avoid borrow issues
        let struct_type = self.env.get_type(name).cloned();

        if let Some(struct_type) = struct_type {
            if let Type::Struct {
                name: struct_name,
                fields: expected_fields,
                ..
            } = &struct_type
            {
                // Check that all required fields are present and have correct types
                let mut provided_fields = std::collections::HashSet::new();

                for (field_name, field_expr) in fields {
                    provided_fields.insert(field_name.clone());

                    let field_type = self.check_expression(field_expr);

                    if let Some(expected_type) = expected_fields.get(field_name) {
                        if !field_type.is_assignable_to(expected_type) {
                            self.result.add_error(TypeError::TypeMismatch {
                                expected: expected_type.clone(),
                                actual: field_type,
                                position: pos,
                            });
                        }
                    } else {
                        self.result.add_error(TypeError::UnknownField {
                            struct_name: struct_name.clone(),
                            field_name: field_name.clone(),
                            position: pos,
                        });
                    }
                }

                // Check for missing fields
                for (expected_field, _) in expected_fields {
                    if !provided_fields.contains(expected_field) {
                        self.result.add_error(TypeError::MissingField {
                            struct_name: struct_name.clone(),
                            field_name: expected_field.clone(),
                            position: pos,
                        });
                    }
                }

                struct_type
            } else {
                self.result.add_error(TypeError::NotAStruct {
                    type_name: name.to_string(),
                    position: pos,
                });
                Type::Unknown
            }
        } else {
            self.result.add_error(TypeError::UnknownType {
                type_name: name.to_string(),
                position: pos,
            });
            Type::Unknown
        }
    }

    fn check_extension(
        &mut self,
        target_type: &seen_parser::Type,
        methods: &[Method],
        pos: Position,
    ) -> Type {
        let target = self.resolve_ast_type(target_type, pos);
        let base = target.non_nullable().clone();

        if let Type::Interface { name, .. } = base {
            if let Some(Type::Interface {
                is_sealed: true, ..
            }) = self.env.get_type(&name)
            {
                self.result.add_error(TypeError::SealedTypeExtension {
                    type_name: name,
                    position: pos,
                });
            }
        }

        for method in methods {
            // Best-effort: type check method body in current environment
            self.check_expression(&method.body);
        }

        Type::Unit
    }

    /// Type check if expression with smart casting support
    pub fn check_if_expression(
        &mut self,
        condition: &Expression,
        then_branch: &Expression,
        else_branch: Option<&Expression>,
        pos: Position,
    ) -> Type {
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
    fn analyze_condition_for_smart_casts(
        &mut self,
        condition: &Expression,
    ) -> HashMap<String, Type> {
        let mut smart_casts = HashMap::new();

        match condition {
            // Handle: if variable != null
            Expression::BinaryOp {
                left,
                op: BinaryOperator::NotEqual,
                right,
                ..
            } => {
                if let (Expression::Identifier { name, .. }, Expression::NullLiteral { .. }) =
                    (left.as_ref(), right.as_ref())
                {
                    if let Some(var_type) = self.env.get_variable(name).cloned() {
                        if let Type::Nullable(inner_type) = var_type {
                            smart_casts.insert(name.clone(), *inner_type);
                        }
                    }
                } else if let (
                    Expression::NullLiteral { .. },
                    Expression::Identifier { name, .. },
                ) = (left.as_ref(), right.as_ref())
                {
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
            Expression::BinaryOp {
                left,
                op: BinaryOperator::And,
                right,
                ..
            } => {
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
    fn check_let_expression(
        &mut self,
        name: &str,
        type_annotation: &Option<seen_parser::ast::Type>,
        value: &Expression,
        pos: Position,
    ) -> Type {
        let value_type = self.check_expression(value);

        let declared_type = if let Some(type_ann) = type_annotation {
            let declared = self.resolve_ast_type(type_ann, pos);
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
            self.env
                .define_variable(name.to_string(), declared_type.clone());
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

    /// Type check index access
    fn check_index_access(
        &mut self,
        object: &Expression,
        index: &Expression,
        pos: Position,
    ) -> Type {
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

    /// Type check function definition
    fn check_function_definition(
        &mut self,
        name: &str,
        generics: &[String],
        params: &[seen_parser::ast::Parameter],
        return_type: &Option<seen_parser::ast::Type>,
        body: &Expression,
        pos: Position,
    ) -> Type {
        self.with_generics(generics, |checker| {
            checker.check_function_definition_inner(name, params, return_type, body, pos.clone())
        })
    }

    fn check_function_definition_inner(
        &mut self,
        name: &str,
        params: &[seen_parser::ast::Parameter],
        return_type: &Option<seen_parser::ast::Type>,
        body: &Expression,
        pos: Position,
    ) -> Type {
        // Convert AST parameter types to checker types
        let mut checker_params = Vec::new();
        for param in params {
            let param_type = if let Some(param_type_ast) = &param.type_annotation {
                self.resolve_ast_type(param_type_ast, pos)
            } else {
                Type::Unknown
            };
            checker_params.push(crate::Parameter {
                name: param.name.clone(),
                param_type: param_type.clone(),
            });
        }

        // Convert return type
        let checker_return_type = if let Some(ret_type_ast) = return_type {
            Some(self.resolve_ast_type(ret_type_ast, pos))
        } else {
            Some(Type::Unit) // Default to Unit if no return type specified
        };

        // Create function signature
        let signature = FunctionSignature {
            name: name.to_string(),
            parameters: checker_params.clone(),
            return_type: checker_return_type.clone(),
        };

        // Register (or update) the function in the environment without duplicate error
        if !self.env.has_function(name) {
            self.env.define_function(name.to_string(), signature);
        }

        // Create new scope for function body
        let saved_env = self.env.clone();
        let mut function_env = Environment::with_parent(self.env.clone());

        // Add parameters to function scope
        for (param, checker_param) in params.iter().zip(checker_params.iter()) {
            function_env.define_variable(param.name.clone(), checker_param.param_type.clone());
        }

        // Set current environment to function scope
        self.env = function_env;

        // Store current function return type for return statement checking
        let saved_return_type = self.current_function_return_type.clone();
        self.current_function_return_type = checker_return_type.clone();

        // Type check the function body
        let body_type = self.check_expression(body);

        // Verify return type matches
        if let Some(expected_return) = &checker_return_type {
            if !body_type.is_assignable_to(expected_return) {
                self.result.add_error(TypeError::TypeMismatch {
                    expected: expected_return.clone(),
                    actual: body_type.clone(),
                    position: pos,
                });
            }
        }

        // Restore environment and return type
        self.env = saved_env;
        self.current_function_return_type = saved_return_type;

        // Function definitions return the function type (for now, Unit)
        Type::Unit
    }

    /// Type check an interface definition
    fn check_interface_definition(
        &mut self,
        name: &str,
        generics: &[String],
        methods: &[InterfaceMethod],
        is_sealed: bool,
        pos: Position,
    ) -> Type {
        let mut method_names = Vec::new();

        self.with_generics(generics, |checker| {
            for method in methods {
                method_names.push(method.name.clone());

                let mut params = Vec::new();
                for param in &method.params {
                    let param_type = if let Some(type_ann) = &param.type_annotation {
                        checker.resolve_ast_type(type_ann, pos)
                    } else {
                        Type::Unknown
                    };
                    params.push(crate::Parameter {
                        name: param.name.clone(),
                        param_type,
                    });
                }

                let return_type = if let Some(ret_type) = &method.return_type {
                    Some(checker.resolve_ast_type(ret_type, pos))
                } else {
                    Some(Type::Unit)
                };

                let signature = FunctionSignature {
                    name: format!("{}::{}", name, method.name),
                    parameters: params,
                    return_type,
                };

                checker
                    .env
                    .define_function(format!("{}::{}", name, method.name), signature);
            }
        });

        let interface_type = Type::Interface {
            name: name.to_string(),
            methods: method_names,
            generics: generics.iter().map(|g| Type::Generic(g.clone())).collect(),
            is_sealed,
        };

        self.env.define_type(name.to_string(), interface_type);

        Type::Unit
    }

    /// Extract struct name from a type for error reporting
    fn extract_struct_name_from_type(&self, type_: &Type) -> String {
        match type_ {
            Type::Struct { name, .. } => name.clone(),
            Type::Nullable(inner) => {
                if let Type::Struct { name, .. } = inner.as_ref() {
                    name.clone()
                } else {
                    format!("{:?}", inner)
                }
            }
            _ => format!("{:?}", type_),
        }
    }
}

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
    }
}
