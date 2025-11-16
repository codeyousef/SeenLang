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
    /// Mutability tracking for variables (var vs let)
    mutable_vars: HashMap<String, bool>,
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
            mutable_vars: HashMap::new(),
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
            mutable_vars: HashMap::new(),
            functions: HashMap::new(),
            types: HashMap::new(),
            parent: Some(Box::new(parent)),
            smart_casts: HashMap::new(),
        }
    }

    /// Define a variable in this environment
    pub fn define_variable(&mut self, name: String, var_type: Type) {
        self.variables.insert(name.clone(), var_type);
        self.mutable_vars.insert(name, false);
    }

    /// Define a mutable variable in this environment
    pub fn define_mutable_variable(&mut self, name: String, var_type: Type) {
        self.variables.insert(name.clone(), var_type);
        self.mutable_vars.insert(name, true);
    }

    /// Check if a variable is mutable
    pub fn is_mutable(&self, name: &str) -> bool {
        self.mutable_vars
            .get(name)
            .copied()
            .unwrap_or_else(|| {
                self.parent
                    .as_ref()
                    .map(|p| p.is_mutable(name))
                    .unwrap_or(false)
            })
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
    #[allow(dead_code)]
    fn remove_smart_cast(&mut self, name: &str) {
        self.smart_casts.remove(name);
    }

    /// Create a child environment that inherits smart casts
    #[allow(dead_code)]
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
    /// Global prelude scope for manifest modules
    /// Contains all top-level functions from bundled modules
    prelude: HashMap<String, FunctionSignature>,
}

impl TypeChecker {
    fn predeclare_types(&mut self, program: &Program) {
        for expr in &program.expressions {
            self.predeclare_type_in_expression(expr);
        }
    }

    fn predeclare_type_in_expression(&mut self, expr: &Expression) {
        match expr {
            Expression::StructDefinition { name, generics, .. } => {
                self.predeclare_struct_type(name, generics)
            }
            Expression::ClassDefinition { name, generics, .. } => {
                self.predeclare_struct_type(name, generics)
            }
            Expression::EnumDefinition {
                name,
                generics,
                variants,
                ..
            } => self.predeclare_enum_type_with_variants(name, generics, variants),
            Expression::Interface { name, generics, .. } => {
                self.predeclare_interface_type(name, generics)
            }
            _ => {}
        }
    }

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
            "__GetEnv".to_string(),
            FunctionSignature {
                name: "__GetEnv".to_string(),
                parameters: vec![Parameter {
                    name: "name".to_string(),
                    param_type: Type::String,
                }],
                return_type: Some(Type::String),
            },
        );
        env.define_function(
            "__HasEnv".to_string(),
            FunctionSignature {
                name: "__HasEnv".to_string(),
                parameters: vec![Parameter {
                    name: "name".to_string(),
                    param_type: Type::String,
                }],
                return_type: Some(Type::Bool),
            },
        );
        env.define_function(
            "__SetEnv".to_string(),
            FunctionSignature {
                name: "__SetEnv".to_string(),
                parameters: vec![
                    Parameter {
                        name: "name".to_string(),
                        param_type: Type::String,
                    },
                    Parameter {
                        name: "value".to_string(),
                        param_type: Type::String,
                    },
                ],
                return_type: Some(Type::Bool),
            },
        );
        env.define_function(
            "__RemoveEnv".to_string(),
            FunctionSignature {
                name: "__RemoveEnv".to_string(),
                parameters: vec![Parameter {
                    name: "name".to_string(),
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
            "__CommandOutput".to_string(),
            FunctionSignature {
                name: "__CommandOutput".to_string(),
                parameters: vec![Parameter {
                    name: "command".to_string(),
                    param_type: Type::String,
                }],
                return_type: Some(Type::String),
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
        env.define_function(
            "abort".to_string(),
            FunctionSignature {
                name: "abort".to_string(),
                parameters: vec![Parameter {
                    name: "message".to_string(),
                    param_type: Type::String,
                }],
                return_type: Some(Type::Never),
            },
        );
        env.define_function(
            "range".to_string(),
            FunctionSignature {
                name: "range".to_string(),
                parameters: vec![
                    Parameter {
                        name: "start".to_string(),
                        param_type: Type::Int,
                    },
                    Parameter {
                        name: "end".to_string(),
                        param_type: Type::Int,
                    },
                ],
                return_type: Some(Type::Array(Box::new(Type::Int))),
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

        // Add exit function for process termination
        env.define_function(
            "exit".to_string(),
            FunctionSignature {
                name: "exit".to_string(),
                parameters: vec![Parameter {
                    name: "code".to_string(),
                    param_type: Type::Int,
                }],
                return_type: Some(Type::Unit),
            },
        );

        // Add print functions for benchmarks
        env.define_function(
            "__Print".to_string(),
            FunctionSignature {
                name: "__Print".to_string(),
                parameters: vec![Parameter {
                    name: "msg".to_string(),
                    param_type: Type::String,
                }],
                return_type: Some(Type::Unit),
            },
        );
        env.define_function(
            "__Println".to_string(),
            FunctionSignature {
                name: "__Println".to_string(),
                parameters: vec![Parameter {
                    name: "msg".to_string(),
                    param_type: Type::String,
                }],
                return_type: Some(Type::Unit),
            },
        );

        // String conversion functions
        env.define_function(
            "__IntToString".to_string(),
            FunctionSignature {
                name: "__IntToString".to_string(),
                parameters: vec![Parameter {
                    name: "value".to_string(),
                    param_type: Type::Int,
                }],
                return_type: Some(Type::String),
            },
        );
        env.define_function(
            "__FloatToString".to_string(),
            FunctionSignature {
                name: "__FloatToString".to_string(),
                parameters: vec![Parameter {
                    name: "value".to_string(),
                    param_type: Type::Float,
                }],
                return_type: Some(Type::String),
            },
        );
        env.define_function(
            "__BoolToString".to_string(),
            FunctionSignature {
                name: "__BoolToString".to_string(),
                parameters: vec![Parameter {
                    name: "value".to_string(),
                    param_type: Type::Bool,
                }],
                return_type: Some(Type::String),
            },
        );

        // Add super as a variadic function for calling parent constructors
        // Accepts any parameters (we can't easily specify variadic in type system)
        // In practice, the code generator will handle super calls specially
        env.define_function(
            "super".to_string(),
            FunctionSignature {
                name: "super".to_string(),
                parameters: vec![], // Will be validated by inheritance checking later
                return_type: Some(Type::Unit),
            },
        );

        // Add throw function for exception handling
        env.define_function(
            "throw".to_string(),
            FunctionSignature {
                name: "throw".to_string(),
                parameters: vec![Parameter {
                    name: "exception".to_string(),
                    param_type: Type::Unknown, // Accept any exception type
                }],
                return_type: Some(Type::Unit),
            },
        );

        Self {
            env,
            result: TypeCheckResult::new(),
            current_function_return_type: None,
            generic_stack: Vec::new(),
            scope_depth: 0,
            prelude: HashMap::new(),
        }
    }

    /// Populate prelude scope with all top-level functions from the program
    /// This enables cross-module function visibility for manifest-bundled modules
    fn populate_prelude(&mut self, program: &Program) {
        // Only populate prelude when manifest modules are enabled
        if std::env::var("SEEN_ENABLE_MANIFEST_MODULES").is_ok() {
            // First, copy all built-in functions from env to prelude
            // This ensures built-ins like exit(), throw(), super() are visible across modules
            for (name, sig) in &self.env.functions {
                self.prelude.insert(name.clone(), sig.clone());
            }

            // Then add program-level functions
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
                    self.prelude.insert(name.clone(), sig);
                }
            }
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
                // Return type (default Unit). Accept either explicit Unit or legacy Void.
                let ret_ty = return_type
                    .as_ref()
                    .map(|t| self.resolve_ast_type(t, Position::start()))
                    .or(Some(Type::Unit));
                // Normalize legacy Void to Unit
                let ret = match ret_ty {
                    Some(Type::Struct { name, .. }) if name == "Void" => Some(Type::Unit),
                    other => other,
                };
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

    fn predeclare_struct_type(&mut self, name: &str, generics: &[String]) {
        if self.env.get_type(name).is_some() {
            return;
        }
        let placeholder = Type::Struct {
            name: name.to_string(),
            fields: HashMap::new(),
            generics: generics.iter().map(|g| Type::Generic(g.clone())).collect(),
        };
        self.env.define_type(name.to_string(), placeholder);
    }

    fn predeclare_enum_type(&mut self, name: &str, generics: &[String]) {
        if self.env.get_type(name).is_some() {
            return;
        }
        let placeholder = Type::Enum {
            name: name.to_string(),
            variants: Vec::new(),
            generics: generics.iter().map(|g| Type::Generic(g.clone())).collect(),
        };
        self.env.define_type(name.to_string(), placeholder);
    }

    fn predeclare_enum_type_with_variants(
        &mut self,
        name: &str,
        generics: &[String],
        variants: &[seen_parser::ast::EnumVariant],
    ) {
        if self.env.get_type(name).is_some() {
            return;
        }
        // Extract variant names immediately during predeclaration
        let variant_names: Vec<String> = variants.iter().map(|v| v.name.clone()).collect();
        let enum_type = Type::Enum {
            name: name.to_string(),
            variants: variant_names,
            generics: generics.iter().map(|g| Type::Generic(g.clone())).collect(),
        };
        self.env.define_type(name.to_string(), enum_type);
    }

    fn predeclare_interface_type(&mut self, name: &str, generics: &[String]) {
        if self.env.get_type(name).is_some() {
            return;
        }
        let placeholder = Type::Interface {
            name: name.to_string(),
            methods: Vec::new(),
            generics: generics.iter().map(|g| Type::Generic(g.clone())).collect(),
            is_sealed: false,
        };
        self.env.define_type(name.to_string(), placeholder);
    }

    fn handle_import(
        &mut self,
        module_path: &[String],
        symbols: &[seen_parser::ast::ImportSymbol],
        _pos: Position,
    ) {
        // Special handling for commonly imported modules - add stubs for known functions
        // This allows the self-hosted compiler to reference standard functions
        let module_name = module_path.join(".");

        match module_name.as_str() {
            "bootstrap.frontend" => {
                // Add known exports from bootstrap.frontend
                for symbol in symbols {
                    match symbol.name.as_str() {
                        "FrontendResult" => {
                            let mut fields = HashMap::new();
                            fields.insert("success".to_string(), Type::Bool);
                            fields.insert(
                                "diagnostics".to_string(),
                                Type::Array(Box::new(Type::Unknown)),
                            );
                            self.env.define_type(
                                "FrontendResult".to_string(),
                                Type::Struct {
                                    name: "FrontendResult".to_string(),
                                    fields,
                                    generics: vec![],
                                },
                            );
                        }
                        "FrontendDiagnostic" => {
                            let mut fields = HashMap::new();
                            fields.insert("file".to_string(), Type::String);
                            fields.insert("line".to_string(), Type::Int);
                            fields.insert("column".to_string(), Type::Int);
                            fields.insert("severity".to_string(), Type::String);
                            fields.insert("message".to_string(), Type::String);
                            self.env.define_type(
                                "FrontendDiagnostic".to_string(),
                                Type::Struct {
                                    name: "FrontendDiagnostic".to_string(),
                                    fields,
                                    generics: vec![],
                                },
                            );
                        }
                        "run_frontend" => {
                            // Register run_frontend function
                            let mut result_fields = HashMap::new();
                            result_fields.insert("success".to_string(), Type::Bool);
                            result_fields.insert(
                                "diagnostics".to_string(),
                                Type::Array(Box::new(Type::Unknown)),
                            );

                            self.env.define_function(
                                "run_frontend".to_string(),
                                FunctionSignature {
                                    name: "run_frontend".to_string(),
                                    parameters: vec![
                                        Parameter {
                                            name: "source".to_string(),
                                            param_type: Type::String,
                                        },
                                        Parameter {
                                            name: "fileLabel".to_string(),
                                            param_type: Type::String,
                                        },
                                        Parameter {
                                            name: "language".to_string(),
                                            param_type: Type::String,
                                        },
                                    ],
                                    return_type: Some(Type::Struct {
                                        name: "FrontendResult".to_string(),
                                        fields: result_fields,
                                        generics: vec![],
                                    }),
                                },
                            );
                        }
                        _ => {}
                    }
                }
            }
            _ => {
                // For other modules, just mark them as imported without error
                // This allows the compiler to continue checking the rest of the code
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
            "Void" | "()" | "Unit" => Type::Unit,
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
            "Map" | "HashMap" | "Dict" => {
                if resolved_args.len() == 2 {
                    Type::Map {
                        key_type: Box::new(resolved_args[0].clone()),
                        value_type: Box::new(resolved_args[1].clone()),
                    }
                } else {
                    self.result.add_error(TypeError::GenericArityMismatch {
                        type_name: ast_type.name.clone(),
                        expected: 2,
                        actual: resolved_args.len(),
                        position: pos,
                    });
                    Type::Unknown
                }
            }
            _ => {
                if let Some(mut def) = self.env.get_type(&ast_type.name).cloned() {
                    // CRITICAL FIX: Refresh empty struct definitions
                    if let Type::Struct { name, fields, .. } = &def {
                        if fields.is_empty() {
                            // Try to get a fresher version
                            if let Some(fresh) = self.env.get_type(name) {
                                if let Type::Struct {
                                    fields: fresh_fields,
                                    ..
                                } = fresh
                                {
                                    if !fresh_fields.is_empty() {
                                        def = fresh.clone();
                                    }
                                }
                            }
                        }
                    }
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

                // CRITICAL FIX: Refresh enum variants if empty (predeclared but not yet fully defined)
                let actual_variants = if variants.is_empty() {
                    if let Some(fresh_type) = self.env.get_type(&name) {
                        if let Type::Enum {
                            variants: fresh_variants,
                            ..
                        } = fresh_type
                        {
                            if !fresh_variants.is_empty() {
                                fresh_variants.clone()
                            } else {
                                variants
                            }
                        } else {
                            variants
                        }
                    } else {
                        variants
                    }
                } else {
                    variants
                };

                Type::Enum {
                    name,
                    variants: actual_variants,
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

    fn placeholder_generic_args(&self, definition: &Type) -> Vec<Type> {
        match definition {
            Type::Struct { generics, .. }
            | Type::Enum { generics, .. }
            | Type::Interface { generics, .. } => generics.iter().map(|_| Type::Unknown).collect(),
            _ => Vec::new(),
        }
    }

    fn type_from_identifier(&mut self, name: &str, pos: Position) -> Option<Type> {
        if let Some(definition) = self.env.get_type(name).cloned() {
            let placeholders = self.placeholder_generic_args(&definition);
            return Some(self.instantiate_type(definition, &placeholders, pos));
        }

        match name {
            "Int" => Some(Type::Int),
            "UInt" => Some(Type::UInt),
            "Float" => Some(Type::Float),
            "Bool" => Some(Type::Bool),
            "String" => Some(Type::String),
            "Char" => Some(Type::Char),
            "Unit" | "Void" | "()" => Some(Type::Unit),
            "Never" => Some(Type::Never),
            "Array" | "List" => Some(Type::Array(Box::new(Type::Unknown))),
            _ => None,
        }
    }

    fn lookup_this_field_type(&self, field: &str) -> Option<Type> {
        if let Some(this_type) = self.env.get_variable("this") {
            match this_type.non_nullable() {
                Type::Struct { fields, .. } => fields.get(field).cloned(),
                _ => None,
            }
        } else {
            None
        }
    }

    fn resolve_receiver_type(&mut self, receiver: &Receiver) -> Type {
        if let Some(existing) = self.env.get_type(&receiver.type_name).cloned() {
            existing
        } else {
            Type::Struct {
                name: receiver.type_name.clone(),
                fields: HashMap::new(),
                generics: Vec::new(),
            }
        }
    }

    fn substitute_generics(&self, ty: &Type, mapping: &HashMap<String, Type>) -> Type {
        match ty {
            Type::Generic(name) => mapping.get(name).cloned().unwrap_or_else(|| ty.clone()),
            Type::Array(inner) => Type::Array(Box::new(self.substitute_generics(inner, mapping))),
            Type::Map {
                key_type,
                value_type,
            } => Type::Map {
                key_type: Box::new(self.substitute_generics(key_type, mapping)),
                value_type: Box::new(self.substitute_generics(value_type, mapping)),
            },
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
        // FIRST: Populate prelude with all top-level functions for manifest modules
        // This makes functions from all bundled modules visible to each other
        self.populate_prelude(program);

        // Predeclare type names first (placeholders with empty fields)
        self.predeclare_types(program);

        // Then fully process all struct/class/enum definitions to populate fields
        for expression in &program.expressions {
            match expression {
                Expression::StructDefinition { .. }
                | Expression::ClassDefinition { .. }
                | Expression::EnumDefinition { .. }
                | Expression::Interface { .. } => {
                    self.check_expression(expression);
                }
                _ => {}
            }
        }

        // CRITICAL: Fix up struct field types that reference other structs
        // When struct A has field of type B, it may have captured B's empty placeholder
        self.fixup_struct_field_types();

        // NOW predeclare function signatures (they'll see complete struct types)
        self.predeclare_signatures(program);

        // Finally check remaining expressions
        for expression in &program.expressions {
            match expression {
                Expression::StructDefinition { .. }
                | Expression::ClassDefinition { .. }
                | Expression::EnumDefinition { .. }
                | Expression::Interface { .. } => {
                    // Already processed above
                }
                _ => {
                    self.check_expression(expression);
                }
            }
        }

        // Collect all variables and functions into the result
        self.collect_environment();

        std::mem::take(&mut self.result)
    }

    /// Collect environment data into the result
    /// Fix up struct field types after all structs are fully defined
    /// This resolves cases where struct A has a field of type struct B,
    /// but B was only a placeholder when A was defined
    fn fixup_struct_field_types(&mut self) {
        if std::env::var("SEEN_DEBUG_TYPES").is_ok() {
            eprintln!(
                "[FIXUP] Starting fixup of {} struct types",
                self.env.types.len()
            );
        }

        // Multiple passes to handle deeply nested struct fields
        // Each pass resolves one level of nesting, avoiding expensive deep traversal
        let max_passes = 10;
        for pass in 0..max_passes {
            let type_names: Vec<String> = self.env.types.keys().cloned().collect();
            let mut any_changed = false;
            let mut changed_count = 0;

            if std::env::var("SEEN_DEBUG_TYPES").is_ok() {
                eprintln!("[FIXUP] Pass {} starting...", pass);
            }

            // Phase 1: Fix empty placeholder structs (single shallow pass per iteration)
            // This replaces all empty struct types with their full definitions
            for type_name in &type_names {
                if let Some(struct_type) = self.env.types.get(type_name).cloned() {
                    if let Type::Struct {
                        name,
                        fields,
                        generics,
                    } = &struct_type
                    {
                        if fields.is_empty() {
                            continue;
                        }

                        // For non-empty structs, do ONE shallow pass to replace empty field types
                        let mut fixed_fields = HashMap::new();
                        let mut changed = false;

                        for (field_name, field_type) in fields {
                            let fixed_type = self.fixup_type_shallow(field_type);
                            if field_type != &fixed_type {
                                changed = true;
                                any_changed = true;
                            }
                            fixed_fields.insert(field_name.clone(), fixed_type);
                        }

                        if changed {
                            changed_count += 1;
                            let fixed_struct = Type::Struct {
                                name: name.clone(),
                                fields: fixed_fields,
                                generics: generics.clone(),
                            };
                            self.env.define_type(type_name.clone(), fixed_struct);
                        }
                    }
                }
            }

            if std::env::var("SEEN_DEBUG_TYPES").is_ok() {
                eprintln!("[FIXUP] Pass {} updated {} structs", pass, changed_count);
            }

            // If nothing changed this pass, we're done
            if !any_changed {
                if std::env::var("SEEN_DEBUG_TYPES").is_ok() {
                    eprintln!("[FIXUP] Fixup converged after {} passes", pass + 1);
                }
                break;
            }
        }

        // Phase 2: Fix function signatures (parameters and return types)
        // Functions just use shallow fixup since structs are already fixed
        let func_names: Vec<String> = self.env.functions.keys().cloned().collect();

        for func_name in func_names {
            if let Some(signature) = self.env.functions.get(&func_name).cloned() {
                let mut changed = false;
                let mut fixed_params = Vec::new();

                for param in &signature.parameters {
                    let fixed_type = self.fixup_type_shallow(&param.param_type);
                    if fixed_type != param.param_type {
                        changed = true;
                        if std::env::var("SEEN_DEBUG_TYPES").is_ok() {
                            eprintln!(
                                "[DEBUG] Fixed function {} param {}: changed type",
                                func_name, param.name
                            );
                        }
                    }
                    fixed_params.push(Parameter {
                        name: param.name.clone(),
                        param_type: fixed_type,
                    });
                }

                let fixed_return = if let Some(ret_ty) = &signature.return_type {
                    let fixed = self.fixup_type_shallow(ret_ty);
                    if &fixed != ret_ty {
                        changed = true;
                        if std::env::var("SEEN_DEBUG_TYPES").is_ok() {
                            eprintln!("[DEBUG] Fixed function {} return type", func_name);
                        }
                    }
                    Some(fixed)
                } else {
                    None
                };

                if changed {
                    let fixed_signature = FunctionSignature {
                        name: signature.name,
                        parameters: fixed_params,
                        return_type: fixed_return,
                    };
                    self.env.define_function(func_name, fixed_signature);
                }
            }
        }
    }

    /// Shallow fixup - only replaces empty structs with full definitions from environment,
    /// but doesn't recursively process non-empty struct fields. This is much faster for
    /// large codebases and prevents exponential blowup.
    fn fixup_type_shallow(&self, ty: &Type) -> Type {
        match ty {
            Type::Struct { name, fields, .. } if fields.is_empty() => {
                // Try to get the full definition from environment
                if let Some(full_type) = self.env.get_type(name) {
                    if let Type::Struct {
                        fields: full_fields,
                        ..
                    } = full_type
                    {
                        if !full_fields.is_empty() {
                            if std::env::var("SEEN_DEBUG_TYPES").is_ok() {
                                eprintln!(
                                    "[DEBUG] Replacing empty {} with {} fields",
                                    name,
                                    full_fields.len()
                                );
                            }
                            return full_type.clone();
                        }
                    }
                }
                ty.clone()
            }
            Type::Nullable(inner) => {
                let fixed_inner = self.fixup_type_shallow(inner);
                if &fixed_inner != inner.as_ref() {
                    Type::Nullable(Box::new(fixed_inner))
                } else {
                    ty.clone()
                }
            }
            Type::Array(inner) => {
                let fixed_inner = self.fixup_type_shallow(inner);
                if &fixed_inner != inner.as_ref() {
                    Type::Array(Box::new(fixed_inner))
                } else {
                    ty.clone()
                }
            }
            Type::Map {
                key_type,
                value_type,
            } => {
                let fixed_key = self.fixup_type_shallow(key_type);
                let fixed_val = self.fixup_type_shallow(value_type);
                if &fixed_key != key_type.as_ref() || &fixed_val != value_type.as_ref() {
                    Type::Map {
                        key_type: Box::new(fixed_key),
                        value_type: Box::new(fixed_val),
                    }
                } else {
                    ty.clone()
                }
            }
            Type::Function {
                params,
                return_type,
                is_async,
            } => {
                let mut fixed_params = Vec::new();
                let mut changed = false;

                for param_ty in params {
                    let fixed = self.fixup_type_shallow(param_ty);
                    if &fixed != param_ty {
                        changed = true;
                    }
                    fixed_params.push(fixed);
                }

                let fixed_return = self.fixup_type_shallow(return_type);
                if &fixed_return != return_type.as_ref() {
                    changed = true;
                }

                if changed {
                    Type::Function {
                        params: fixed_params,
                        return_type: Box::new(fixed_return),
                        is_async: *is_async,
                    }
                } else {
                    ty.clone()
                }
            }
            _ => ty.clone(),
        }
    }

    /// Deeply fix up a type by replacing empty struct placeholders with full definitions
    /// and recursively processing all nested types including non-empty struct fields.
    /// This is the critical fix for the "stale type problem" where nested struct types
    /// remain empty even after their definitions are complete.
    ///
    /// Note: This is kept for compatibility but shallow fixup is preferred for performance.
    #[allow(dead_code)]
    fn fixup_type_deep(&self, ty: &Type) -> Type {
        use std::collections::HashSet;
        let mut visited = HashSet::new();
        self.fixup_type_deep_impl(ty, &mut visited)
    }

    /// Internal implementation with cycle detection
    fn fixup_type_deep_impl(
        &self,
        ty: &Type,
        visited: &mut std::collections::HashSet<String>,
    ) -> Type {
        match ty {
            Type::Struct {
                name,
                fields,
                generics,
            } => {
                // Cycle detection: if we're already processing this struct, return it as-is
                if visited.contains(name) {
                    return ty.clone();
                }

                // Mark this struct as being processed
                visited.insert(name.clone());

                // First, check if this is an empty placeholder that should be replaced
                if fields.is_empty() {
                    if let Some(full_type) = self.env.get_type(name) {
                        if let Type::Struct {
                            fields: full_fields,
                            ..
                        } = full_type
                        {
                            if !full_fields.is_empty() {
                                if std::env::var("SEEN_DEBUG_TYPES").is_ok() {
                                    eprintln!(
                                        "[DEBUG] Replacing empty {} with {} fields",
                                        name,
                                        full_fields.len()
                                    );
                                }
                                // Recursively fix the full type to catch nested stale types
                                let result = self.fixup_type_deep_impl(full_type, visited);
                                visited.remove(name);
                                return result;
                            }
                        }
                    }
                    visited.remove(name);
                    return ty.clone();
                }

                // Even for non-empty structs, recursively fix all field types
                // This is KEY to solving the deep nesting problem
                let mut fixed_fields = HashMap::new();
                let mut changed = false;

                for (field_name, field_type) in fields {
                    let fixed_type = self.fixup_type_deep_impl(field_type, visited);
                    if &fixed_type != field_type {
                        changed = true;
                        if std::env::var("SEEN_DEBUG_TYPES").is_ok() {
                            eprintln!("[DEBUG] Fixed nested field {}.{}", name, field_name);
                        }
                    }
                    fixed_fields.insert(field_name.clone(), fixed_type);
                }

                // Remove from visited set before returning
                visited.remove(name);

                if changed {
                    Type::Struct {
                        name: name.clone(),
                        fields: fixed_fields,
                        generics: generics.clone(),
                    }
                } else {
                    ty.clone()
                }
            }
            Type::Nullable(inner) => {
                let fixed_inner = self.fixup_type_deep_impl(inner, visited);
                if &fixed_inner != inner.as_ref() {
                    Type::Nullable(Box::new(fixed_inner))
                } else {
                    ty.clone()
                }
            }
            Type::Array(inner) => {
                let fixed_inner = self.fixup_type_deep_impl(inner, visited);
                if &fixed_inner != inner.as_ref() {
                    Type::Array(Box::new(fixed_inner))
                } else {
                    ty.clone()
                }
            }
            Type::Map {
                key_type,
                value_type,
            } => {
                let fixed_key = self.fixup_type_deep_impl(key_type, visited);
                let fixed_val = self.fixup_type_deep_impl(value_type, visited);
                if &fixed_key != key_type.as_ref() || &fixed_val != value_type.as_ref() {
                    Type::Map {
                        key_type: Box::new(fixed_key),
                        value_type: Box::new(fixed_val),
                    }
                } else {
                    ty.clone()
                }
            }
            Type::Function {
                params,
                return_type,
                is_async,
            } => {
                // Fix parameter types and return type
                let mut fixed_params = Vec::new();
                let mut changed = false;

                for param_ty in params {
                    let fixed = self.fixup_type_deep_impl(param_ty, visited);
                    if &fixed != param_ty {
                        changed = true;
                    }
                    fixed_params.push(fixed);
                }

                let fixed_return = self.fixup_type_deep_impl(return_type, visited);
                if &fixed_return != return_type.as_ref() {
                    changed = true;
                }

                if changed {
                    Type::Function {
                        params: fixed_params,
                        return_type: Box::new(fixed_return),
                        is_async: *is_async,
                    }
                } else {
                    ty.clone()
                }
            }
            _ => ty.clone(),
        }
    }

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
            // Import declarations: resolve symbols and add to environment
            Expression::Import {
                module_path,
                symbols,
                pos,
            } => {
                self.handle_import(module_path, symbols, *pos);
                Type::Unit
            }
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
                } else if let Some(field_type) = self.lookup_this_field_type(name) {
                    field_type
                } else if let Some(type_value) = self.type_from_identifier(name, *pos) {
                    type_value
                } else if name == "throw" {
                    // MVP: 'throw' is a statement keyword, but parser treats it as identifier
                    // Allow it without error - will be validated at runtime
                    Type::Unknown
                } else if matches!(
                    name.as_str(),
                    "I8" | "AST"
                        | "VariableDeclaration"
                        | "Interface"
                        | "Enum"
                        | "Import"
                        | "ParseError"
                ) {
                    // MVP: Type names used as identifiers (likely enum variants or type constructors)
                    // Allow without error - these are probably enum variants from unloaded modules
                    Type::Unknown
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

            Expression::Cast {
                expr,
                target_type,
                pos,
            } => {
                self.check_expression(expr);
                self.resolve_ast_type(target_type, *pos)
            }

            Expression::TypeCheck {
                expr,
                target_type,
                pos,
            } => {
                self.check_expression(expr);
                let _ = self.resolve_ast_type(target_type, *pos);
                Type::Bool
            }

            // Struct definition
            Expression::StructDefinition {
                name,
                generics,
                fields,
                pos,
                ..
            } => self.check_struct_definition(name, generics, fields, *pos),

            Expression::ClassDefinition {
                name,
                generics,
                fields,
                methods,
                pos,
                ..
            } => self.check_class_definition(name, generics, fields, methods, *pos),

            Expression::EnumDefinition {
                name,
                generics,
                variants,
                pos,
                ..
            } => self.check_enum_definition(name, generics, variants, *pos),

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
                is_mutable,
                pos,
                ..
            } => self.check_let_expression(name, type_annotation, value, *is_mutable, *pos),

            // Assignment
            Expression::Assignment { target, value, op, pos } => {
                self.check_assignment_expression(target, value, op, *pos)
            }

            // Collections
            Expression::ArrayLiteral { elements, pos } => self.check_array_literal(elements, *pos),

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
                receiver,
                is_external,
                pos,
                ..
            } => self.check_function_definition(
                name,
                generics,
                params,
                return_type,
                receiver.as_ref(),
                body,
                *is_external,
                *pos,
            ),

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
            BinaryOperator::BitwiseOr => "|",
            BinaryOperator::BitwiseXor => "^",
            BinaryOperator::BitwiseAnd => "&",
            BinaryOperator::LeftShift => "<<",
            BinaryOperator::RightShift => ">>",
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
            UnaryOperator::BitwiseNot => {
                let base = operand_type.non_nullable();
                if matches!(base, Type::Int | Type::UInt) {
                    operand_type
                } else {
                    self.result.add_error(TypeError::InvalidOperation {
                        operation: "bitwise not".to_string(),
                        left_type: operand_type.clone(),
                        right_type: Type::Unit,
                        position: pos,
                    });
                    Type::Unknown
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
        // jobs.scope shares the same structured concurrency semantics as scope.
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

            // Handle Map<K, V>() constructor
            if name == "Map" {
                // Map() expects 0 arguments (creates empty map)
                if !args.is_empty() {
                    self.result.add_error(TypeError::ArgumentCountMismatch {
                        name: name.clone(),
                        expected: 0,
                        actual: args.len(),
                        position: pos,
                    });
                }

                // Return a generic Map type (will be parameterized by usage)
                return Type::Struct {
                    name: "Map".to_string(),
                    fields: HashMap::new(),
                    generics: vec![Type::Unknown, Type::Unknown], // K, V
                };
            }

            // Try to find function in environment first, then prelude
            let signature = self
                .env
                .get_function(name)
                .cloned()
                .or_else(|| self.prelude.get(name).cloned());

            if let Some(signature) = signature {
                // Special handling for super - it's variadic and validated by inheritance
                if name == "super" {
                    // Just check that arguments type-check, don't validate count
                    for arg in args {
                        self.check_expression(arg);
                    }
                    return Type::Unit;
                }

                // Check argument count (allow fewer args for default parameters)
                // MVP: Allow fewer arguments assuming they have defaults
                if args.len() > signature.parameters.len() {
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
            } else if let Some(constructor_type) = self.type_from_identifier(name, pos) {
                for arg in args {
                    self.check_expression(arg);
                }
                constructor_type
            } else {
                self.result.add_error(TypeError::UndefinedFunction {
                    name: name.clone(),
                    position: pos,
                });
                Type::Unknown
            }
        } else if let Expression::MemberAccess { object, member, .. } = callee {
            // Method-style call like obj.method(...)
            let recv_ty = self.check_expression(object);
            let base = recv_ty.non_nullable().clone();

            // Fast-path: common accessors
            if matches!(
                (&base, member.as_str()),
                (Type::Array(_), "size")
                    | (Type::Array(_), "length")
                    | (Type::String, "size")
                    | (Type::String, "length")
            ) {
                // Validate no-arg accessors but still type-check the provided args for side diagnostics
                for arg in args {
                    let _ = self.check_expression(arg);
                }
                return Type::Int;
            }

            // Resolve methods declared as "Type::method" in the environment/prelude
            if let Type::Struct {
                name: struct_name, ..
            } = &base
            {
                let method_name = format!("{}::{}", struct_name, member);
                if let Some(signature) = self
                    .env
                    .get_function(&method_name)
                    .cloned()
                    .or_else(|| self.prelude.get(&method_name).cloned())
                {
                    // Determine expected parameters: drop implicit receiver if present
                    let expected_params = if let Some(first) = signature.parameters.first() {
                        if let Type::Struct { name, .. } = &first.param_type {
                            if name == struct_name {
                                &signature.parameters[1..]
                            } else {
                                &signature.parameters[..]
                            }
                        } else {
                            &signature.parameters[..]
                        }
                    } else {
                        &signature.parameters[..]
                    };

                    // Check argument count allowing defaults (fewer args ok)
                    if args.len() > expected_params.len() {
                        self.result.add_error(TypeError::ArgumentCountMismatch {
                            name: method_name.clone(),
                            expected: expected_params.len(),
                            actual: args.len(),
                            position: pos,
                        });
                    }

                    // Validate argument types against expected parameters (zip stops at shorter)
                    for (arg_expr, param) in args.iter().zip(expected_params.iter()) {
                        let arg_ty = self.check_expression(arg_expr);
                        if !arg_ty.is_assignable_to(&param.param_type) {
                            self.result.add_error(TypeError::TypeMismatch {
                                expected: param.param_type.clone(),
                                actual: arg_ty,
                                position: pos,
                            });
                        }
                    }

                    return signature.return_type.clone().unwrap_or(Type::Unit);
                }
            }

            // Fallback: type-check args and return Unknown (unresolved method)
            for arg in args {
                let _ = self.check_expression(arg);
            }
            Type::Unknown
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
        let mut object_type = self.check_expression(object);

        // CRITICAL FIX: If we got a struct with empty fields, it might be a stale placeholder
        // Look it up fresh from the environment
        if let Type::Struct { name, fields, .. } = &object_type {
            if fields.is_empty() {
                if let Some(fresh_type) = self.env.get_type(name) {
                    if let Type::Struct {
                        fields: fresh_fields,
                        ..
                    } = fresh_type
                    {
                        if !fresh_fields.is_empty() {
                            object_type = fresh_type.clone();
                        }
                    }
                }
            }
        }

        // CRITICAL FIX: Refresh enum if variants are empty (similar to struct fixup above)
        if let Type::Enum { name, variants, .. } = &object_type {
            if variants.is_empty() {
                if let Some(fresh_type) = self.env.get_type(name) {
                    if std::env::var("SEEN_DEBUG_TYPES").is_ok() {
                        eprintln!(
                            "[DEBUG] Attempting to refresh enum '{}', fresh type: {:?}",
                            name, fresh_type
                        );
                    }
                    if let Type::Enum {
                        variants: fresh_variants,
                        ..
                    } = fresh_type
                    {
                        if !fresh_variants.is_empty() {
                            if std::env::var("SEEN_DEBUG_TYPES").is_ok() {
                                eprintln!(
                                    "[DEBUG] Successfully refreshed enum '{}' with {} variants",
                                    name,
                                    fresh_variants.len()
                                );
                            }
                            object_type = fresh_type.clone();
                        }
                    }
                }
            }
        }

        // Debug: log field access attempts
        if std::env::var("SEEN_DEBUG_TYPES").is_ok() {
            eprintln!(
                "[DEBUG] Field access: {}.{} on type {:?}",
                self.extract_struct_name_from_type(&object_type),
                member,
                object_type
            );
        }

        match &object_type {
            // Handle enum variant access: EnumName.Variant
            Type::Enum { name, variants, .. } => {
                // Check if member is a valid variant name (enum already refreshed above)
                // MVP: Case-insensitive match for enum variants
                let member_lower = member.to_lowercase();
                let found = variants.iter().any(|v| v.to_lowercase() == member_lower);

                if found {
                    // Return the enum type itself (enum variants are values of the enum type)
                    object_type
                } else {
                    self.result.add_error(TypeError::UnknownField {
                        struct_name: name.clone(),
                        field_name: member.to_string(),
                        position: pos,
                    });
                    Type::Unknown
                }
            }
            Type::Struct { fields, name, .. } => {
                // MVP FIX: If struct has no fields, treat it like Unknown (incomplete module loading)
                // This happens when structs are imported from unloaded modules
                if fields.is_empty() {
                    return Type::Unknown;
                }

                if let Some(field_type) = fields.get(member) {
                    // CRITICAL FIX: Refresh the field type itself if it's an empty struct
                    let mut result_type = field_type.clone();
                    if let Type::Struct { name, fields, .. } = &result_type {
                        if fields.is_empty() {
                            if let Some(fresh_type) = self.env.get_type(name) {
                                if let Type::Struct {
                                    fields: fresh_fields,
                                    ..
                                } = fresh_type
                                {
                                    if !fresh_fields.is_empty() {
                                        result_type = fresh_type.clone();
                                    }
                                }
                            }
                        }
                    }

                    if is_safe && object_type.is_nullable() {
                        result_type.nullable()
                    } else {
                        result_type
                    }
                } else {
                    self.result.add_error(TypeError::UnknownField {
                        struct_name: name.clone(),
                        field_name: member.to_string(),
                        position: pos,
                    });
                    Type::Unknown
                }
            }
            Type::Nullable(inner) if is_safe => {
                // Safe navigation on nullable type
                let mut inner_type = inner.as_ref().clone();

                // CRITICAL FIX: Fresh lookup for nullable inner types too
                if let Type::Struct { name, fields, .. } = &inner_type {
                    if fields.is_empty() {
                        if let Some(fresh_type) = self.env.get_type(name) {
                            if let Type::Struct {
                                fields: fresh_fields,
                                ..
                            } = fresh_type
                            {
                                if !fresh_fields.is_empty() {
                                    inner_type = fresh_type.clone();
                                }
                            }
                        }
                    }
                }

                if let Type::Struct { fields, .. } = &inner_type {
                    if let Some(field_type) = fields.get(member) {
                        field_type.clone().nullable()
                    } else {
                        self.result.add_error(TypeError::UnknownField {
                            struct_name: self.extract_struct_name_from_type(&inner_type),
                            field_name: member.to_string(),
                            position: pos,
                        });
                        Type::Unknown
                    }
                } else {
                    Type::Unknown
                }
            }
            Type::Unknown => {
                // Allow field access on Unknown types (type inference in progress)
                Type::Unknown
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

        // Debug: log struct registration
        if std::env::var("SEEN_DEBUG_TYPES").is_ok() {
            if let Type::Struct { fields: ref f, .. } = struct_type {
                eprintln!(
                    "[DEBUG] Registering struct '{}' with {} fields: {:?}",
                    name,
                    f.len(),
                    f.keys().collect::<Vec<_>>()
                );
            }
        }

        self.env.define_type(name.to_string(), struct_type);
        Type::Unit
    }

    /// Type check enum definition
    fn check_enum_definition(
        &mut self,
        name: &str,
        generics: &[String],
        variants: &[seen_parser::ast::EnumVariant],
        _pos: Position,
    ) -> Type {
        // Extract variant names from the AST
        let variant_names: Vec<String> = variants.iter().map(|v| v.name.clone()).collect();

        let enum_type = Type::Enum {
            name: name.to_string(),
            variants: variant_names,
            generics: generics.iter().map(|g| Type::Generic(g.clone())).collect(),
        };

        // Debug: log enum registration
        if std::env::var("SEEN_DEBUG_TYPES").is_ok() {
            if let Type::Enum {
                variants: ref v, ..
            } = enum_type
            {
                eprintln!(
                    "[DEBUG] Registering enum '{}' with {} variants: {:?}",
                    name,
                    v.len(),
                    v
                );
            }
        }

        self.env.define_type(name.to_string(), enum_type);
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

    fn check_class_definition(
        &mut self,
        name: &str,
        generics: &[String],
        fields: &[seen_parser::ast::ClassField],
        methods: &[Method],
        pos: Position,
    ) -> Type {
        let class_type = self.with_generics(generics, |checker| {
            checker.build_class_type(name, generics, fields, pos.clone())
        });

        self.env.define_type(name.to_string(), class_type.clone());

        self.with_generics(generics, |checker| {
            checker.check_class_methods(name, &class_type, methods);
        });

        Type::Unit
    }

    fn build_class_type(
        &mut self,
        name: &str,
        generics: &[String],
        fields: &[seen_parser::ast::ClassField],
        pos: Position,
    ) -> Type {
        let mut field_types = HashMap::new();
        for field in fields {
            let field_type = self.resolve_ast_type(&field.field_type, pos);
            if let Some(default_value) = &field.default_value {
                let default_type = self.check_expression(default_value);
                if !default_type.is_assignable_to(&field_type) {
                    self.result.add_error(TypeError::TypeMismatch {
                        expected: field_type.clone(),
                        actual: default_type,
                        position: pos,
                    });
                }
            }
            field_types.insert(field.name.clone(), field_type);
        }

        Type::Struct {
            name: name.to_string(),
            fields: field_types,
            generics: generics.iter().map(|g| Type::Generic(g.clone())).collect(),
        }
    }

    fn check_class_methods(&mut self, class_name: &str, class_type: &Type, methods: &[Method]) {
        let method_infos: Vec<MethodSignatureInfo> = methods
            .iter()
            .map(|method| self.build_method_signature_info(method, true))
            .collect();

        for (method, info) in methods.iter().zip(method_infos.iter()) {
            self.check_class_method(class_name, class_type, method, info, &method_infos);
        }
    }

    fn build_method_signature_info(
        &mut self,
        method: &Method,
        force_instance: bool,
    ) -> MethodSignatureInfo {
        let mut params = Vec::new();
        for param in &method.parameters {
            let param_type = if let Some(ast_type) = &param.type_annotation {
                self.resolve_ast_type(ast_type, method.pos)
            } else {
                Type::Unknown
            };
            params.push(Parameter {
                name: param.name.clone(),
                param_type,
            });
        }

        let return_type = if let Some(ret_type) = &method.return_type {
            Some(self.resolve_ast_type(ret_type, method.pos))
        } else {
            Some(Type::Unit)
        };

        MethodSignatureInfo {
            name: method.name.clone(),
            params,
            return_type,
            is_static: if force_instance {
                false
            } else {
                method.is_static
            },
            pos: method.pos,
        }
    }

    fn check_class_method(
        &mut self,
        class_name: &str,
        class_type: &Type,
        method: &Method,
        info: &MethodSignatureInfo,
        all_infos: &[MethodSignatureInfo],
    ) {
        let method_name = format!("{}::{}", class_name, method.name);
        let method_pos = info.pos;

        let mut signature_params = Vec::new();
        if !info.is_static {
            signature_params.push(Parameter {
                name: "this".to_string(),
                param_type: class_type.clone(),
            });
        }
        signature_params.extend(info.params.clone());

        let signature = FunctionSignature {
            name: method_name.clone(),
            parameters: signature_params,
            return_type: info.return_type.clone(),
        };

        if !self.env.has_function(&method_name) {
            self.env.define_function(method_name.clone(), signature);
        }

        let saved_env = self.env.clone();
        let mut method_env = Environment::with_parent(self.env.clone());

        if !info.is_static {
            method_env.define_variable("this".to_string(), class_type.clone());
            if let Type::Struct { fields, .. } = class_type.clone() {
                for (field_name, field_type) in fields {
                    method_env.define_variable(field_name, field_type);
                }
            }
        }

        for alias in all_infos {
            let alias_signature = FunctionSignature {
                name: alias.name.clone(),
                parameters: alias.params.clone(),
                return_type: alias.return_type.clone(),
            };
            method_env.define_function(alias.name.clone(), alias_signature);
        }

        for param in info.params.iter() {
            method_env.define_variable(param.name.clone(), param.param_type.clone());
        }

        self.env = method_env;

        let saved_return_type = self.current_function_return_type.clone();
        self.current_function_return_type = info.return_type.clone();

        let mut body_type = self.check_expression(&method.body);
        if let Some(expected_return) = &info.return_type {
            if expected_return.is_unit_like() && !body_type.is_never() {
                body_type = Type::Unit;
            }
        }

        // MVP: Skip return type check for constructors named "new" - they implicitly return this
        let is_constructor = method.name == "new" || method.name == "constructor";
        if let Some(expected_return) = &info.return_type {
            if !is_constructor && !body_type.is_assignable_to(expected_return) {
                self.result.add_error(TypeError::TypeMismatch {
                    expected: expected_return.clone(),
                    actual: body_type,
                    position: method_pos,
                });
            }
        }

        self.env = saved_env;
        self.current_function_return_type = saved_return_type;
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

            Expression::TypeCheck {
                expr,
                target_type,
                pos,
            } => {
                if let Expression::Identifier { name, .. } = expr.as_ref() {
                    let resolved = self.resolve_ast_type(target_type, *pos);
                    smart_casts.insert(name.clone(), resolved);
                }
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

        if expressions.len() == 1 {
            return self.check_expression(&expressions[0]);
        }

        let (last, rest) = expressions.split_last().expect("non-empty vector");
        let mut short_circuited = false;
        for expr in rest {
            let ty = self.check_statement_expression(expr);
            if ty.is_never() {
                short_circuited = true;
            }
        }

        let last_type = self.check_expression(last);
        if short_circuited {
            Type::Never
        } else {
            last_type
        }
    }

    fn check_statement_expression(&mut self, expression: &Expression) -> Type {
        let ty = self.check_expression(expression);
        if ty.is_never() {
            Type::Never
        } else {
            Type::Unit
        }
    }

    /// Type check let expression
    fn check_let_expression(
        &mut self,
        name: &str,
        type_annotation: &Option<seen_parser::ast::Type>,
        value: &Expression,
        is_mutable: bool,
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
            if is_mutable {
                self.env
                    .define_mutable_variable(name.to_string(), declared_type.clone());
            } else {
                self.env
                    .define_variable(name.to_string(), declared_type.clone());
            }
        }

        // Let/var declarations are statements; they evaluate to Unit
        Type::Unit
    }

    fn check_assignment_expression(
        &mut self,
        target: &Expression,
        value: &Expression,
        op: &seen_parser::ast::AssignmentOperator,
        pos: Position,
    ) -> Type {
        use seen_parser::ast::AssignmentOperator;

        let value_type = self.check_expression(value);

        match target {
            Expression::Identifier { name, .. } => {
                if let Some(var_type) = self.env.get_variable(name) {
                    if !self.env.is_mutable(name) {
                        self.result.add_error(TypeError::ImmutableAssignment {
                            name: name.clone(),
                            position: pos,
                        });
                        return Type::Unit;
                    }

                    match op {
                        AssignmentOperator::Assign => {
                            if !value_type.is_assignable_to(var_type) {
                                self.result.add_error(TypeError::TypeMismatch {
                                    expected: var_type.clone(),
                                    actual: value_type,
                                    position: pos,
                                });
                            }
                        }
                        AssignmentOperator::AddAssign
                        | AssignmentOperator::SubAssign
                        | AssignmentOperator::MulAssign
                        | AssignmentOperator::DivAssign
                        | AssignmentOperator::ModAssign => {
                            if !var_type.is_numeric() {
                                self.result.add_error(TypeError::InvalidOperandType {
                                    operator: format!("{:?}", op),
                                    operand_type: var_type.clone(),
                                    position: pos,
                                });
                            }
                            if !value_type.is_numeric() {
                                self.result.add_error(TypeError::InvalidOperandType {
                                    operator: format!("{:?}", op),
                                    operand_type: value_type,
                                    position: pos,
                                });
                            }
                        }
                    }
                } else {
                    self.result.add_error(TypeError::UndefinedVariable {
                        name: name.clone(),
                        position: pos,
                    });
                }
            }
            Expression::MemberAccess { object, member, .. } => {
                let _object_type = self.check_expression(object);
                if !value_type.is_assignable_to(&Type::Unknown) {
                    self.result.add_error(TypeError::TypeMismatch {
                        expected: Type::Unknown,
                        actual: value_type,
                        position: pos,
                    });
                }
            }
            Expression::IndexAccess { object, index, .. } => {
                let _object_type = self.check_expression(object);
                let _index_type = self.check_expression(index);
            }
            _ => {
                self.result.add_error(TypeError::InvalidAssignmentTarget {
                    position: pos,
                });
            }
        }

        Type::Unit
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
            Type::Unknown => {
                // Allow indexing on Unknown types (type inference in progress)
                Type::Unknown
            }
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
        receiver: Option<&Receiver>,
        body: &Expression,
        is_external: bool,
        pos: Position,
    ) -> Type {
        self.with_generics(generics, |checker| {
            checker.check_function_definition_inner(
                name,
                params,
                return_type,
                receiver,
                body,
                is_external,
                pos.clone(),
            )
        })
    }

    fn check_function_definition_inner(
        &mut self,
        name: &str,
        params: &[seen_parser::ast::Parameter],
        return_type: &Option<seen_parser::ast::Type>,
        receiver: Option<&Receiver>,
        body: &Expression,
        is_external: bool,
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

        let mut signature_params = Vec::new();
        let mut receiver_binding: Option<(String, Type)> = None;
        if let Some(receiver) = receiver {
            let receiver_type = self.resolve_receiver_type(receiver);
            signature_params.push(crate::Parameter {
                name: receiver.name.clone(),
                param_type: receiver_type.clone(),
            });
            receiver_binding = Some((receiver.name.clone(), receiver_type));
        }

        signature_params.extend(checker_params.clone());

        // Convert return type
        let checker_return_type = if let Some(ret_type_ast) = return_type {
            Some(self.resolve_ast_type(ret_type_ast, pos))
        } else {
            Some(Type::Unit) // Default to Unit if no return type specified
        };

        // Create function signature
        let signature = FunctionSignature {
            name: name.to_string(),
            parameters: signature_params.clone(),
            return_type: checker_return_type.clone(),
        };

        // Register (or update) the function in the environment without duplicate error
        if !self.env.has_function(name) {
            self.env.define_function(name.to_string(), signature);
        }

        // For external functions, skip body type checking
        if is_external {
            // External functions are just declarations, no body to check
            return Type::Unit;
        }

        // Create new scope for function body
        let saved_env = self.env.clone();
        let mut function_env = Environment::with_parent(self.env.clone());

        if let Some((receiver_name, receiver_type)) = receiver_binding.clone() {
            function_env.define_variable(receiver_name.clone(), receiver_type.clone());
            if receiver_name != "this" {
                function_env.define_variable("this".to_string(), receiver_type);
            }
        }

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
        let mut body_type = self.check_expression(body);
        if let Some(expected_return) = &checker_return_type {
            if expected_return.is_unit_like() && !body_type.is_never() {
                body_type = Type::Unit;
            }
        }

        // Verify return type matches
        // MVP: Skip return type check for constructors named "new" - they implicitly return this
        let is_constructor = name == "new" || name == "constructor";
        if let Some(expected_return) = &checker_return_type {
            if !is_constructor && !body_type.is_assignable_to(expected_return) {
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

        // Function definitions return the function type (Unit)
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

struct MethodSignatureInfo {
    name: String,
    params: Vec<Parameter>,
    return_type: Option<Type>,
    is_static: bool,
    pos: Position,
}

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
    }
}
