//! Type checking implementation

use crate::types::*;
use crate::inference::InferenceEngine;
use seen_common::{SeenResult, SeenError, Diagnostics};
use seen_parser::{Program, Stmt, StmtKind, Expr, ExprKind, Literal, LiteralKind, BinaryOp, PatternKind};

/// Type checker
pub struct TypeChecker {
    pub env: TypeEnvironment,
    diagnostics: Diagnostics,
    pub inference: InferenceEngine,
}

impl TypeChecker {
    pub fn new() -> Self {
        let mut env = TypeEnvironment::new();
        
        // Add built-in functions to the type environment
        Self::populate_builtin_functions(&mut env);
        
        Self {
            env,
            diagnostics: Diagnostics::new(),
            inference: InferenceEngine::new(),
        }
    }
    
    fn populate_builtin_functions(env: &mut TypeEnvironment) {
        // println: (str) -> ()
        let println_type = Type::Function {
            params: vec![Type::Primitive(PrimitiveType::Str)],
            return_type: Box::new(Type::Primitive(PrimitiveType::Unit)),
        };
        env.insert_function("println".to_string(), println_type);
        
        // print: (str) -> ()
        let print_type = Type::Function {
            params: vec![Type::Primitive(PrimitiveType::Str)],
            return_type: Box::new(Type::Primitive(PrimitiveType::Unit)),
        };
        env.insert_function("print".to_string(), print_type);
        
        // debug: (T) -> ()
        let debug_type = Type::Function {
            params: vec![Type::Variable(0)], // Generic type T
            return_type: Box::new(Type::Primitive(PrimitiveType::Unit)),
        };
        env.insert_function("debug".to_string(), debug_type);
        
        // assert: (bool) -> ()
        let assert_type = Type::Function {
            params: vec![Type::Primitive(PrimitiveType::Bool)],
            return_type: Box::new(Type::Primitive(PrimitiveType::Unit)),
        };
        env.insert_function("assert".to_string(), assert_type);
        
        // panic: (str) -> !
        let panic_type = Type::Function {
            params: vec![Type::Primitive(PrimitiveType::Str)],
            return_type: Box::new(Type::Primitive(PrimitiveType::Never)),
        };
        env.insert_function("panic".to_string(), panic_type);
    }
    
    pub fn diagnostics(&self) -> &Diagnostics {
        &self.diagnostics
    }
    
    pub fn check_program(&mut self, program: &Program) -> SeenResult<()> {
        // Two-pass approach: first collect all function signatures, then check bodies
        
        // Pass 1: Collect all function signatures and other type definitions
        for item in &program.items {
            match &item.kind {
                seen_parser::ItemKind::Function(func) => {
                    // Function validation
                    if func.name.value.is_empty() {
                        self.diagnostics.error("Function name cannot be empty", func.name.span);
                        continue;
                    }
                    
                    // Handle generic functions
                    if !func.type_params.is_empty() {
                        // For generic functions, create a polymorphic function type
                        // We'll defer actual type parameter resolution to Pass 2
                        
                        // Create generic function type without resolving type parameters yet
                        let func_type = Type::Generic {
                            name: format!("{}::<{}>", 
                                func.name.value,
                                func.type_params.iter()
                                    .map(|tp| tp.name.value)
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            ),
                            bounds: Vec::new(), // TODO: Handle type bounds
                        };
                        
                        // Store the generic function type
                        self.env.insert_function(func.name.value.to_string(), func_type);
                        
                        // We'll resolve the actual signature later in Pass 2
                    } else {
                        // Non-generic function
                        let param_types: Vec<Type> = func.params.iter()
                            .map(|param| self.resolve_type_annotation(&param.ty))
                            .collect::<Result<Vec<_>, _>>()?;
                        
                        let return_type = if let Some(ret_ty) = &func.return_type {
                            Box::new(self.resolve_type_annotation(ret_ty)?)
                        } else {
                            Box::new(Type::Primitive(PrimitiveType::Unit))
                        };
                        
                        let func_type = Type::Function {
                            params: param_types,
                            return_type,
                        };
                        
                        self.env.insert_function(func.name.value.to_string(), func_type);
                    }
                }
                seen_parser::ItemKind::Struct(struct_def) => {
                    // Handle struct type definitions
                    let field_types: Vec<(String, Type)> = struct_def.fields.iter()
                        .map(|field| {
                            let field_type = self.resolve_type_annotation(&field.ty)?;
                            Ok((field.name.value.to_string(), field_type))
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    
                    let struct_type = Type::Struct {
                        name: struct_def.name.value.to_string(),
                        fields: field_types,
                    };
                    
                    self.env.insert_type(struct_def.name.value.to_string(), struct_type);
                }
                seen_parser::ItemKind::Enum(enum_def) => {
                    // Handle enum type definitions
                    let variant_types: Vec<(String, Vec<Type>)> = enum_def.variants.iter()
                        .map(|variant| {
                            let variant_types = match &variant.data {
                                seen_parser::VariantData::Unit => Vec::new(),
                                seen_parser::VariantData::Tuple(types) => {
                                    types.iter()
                                        .map(|ty| self.resolve_type_annotation(ty))
                                        .collect::<Result<Vec<_>, _>>()?
                                }
                                seen_parser::VariantData::Struct(fields) => {
                                    fields.iter()
                                        .map(|field| self.resolve_type_annotation(&field.ty))
                                        .collect::<Result<Vec<_>, _>>()?
                                }
                            };
                            Ok((variant.name.value.to_string(), variant_types))
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    
                    let enum_type = Type::Enum {
                        name: enum_def.name.value.to_string(),
                        variants: variant_types,
                    };
                    
                    self.env.insert_type(enum_def.name.value.to_string(), enum_type);
                }
                // Handle additional item types
                seen_parser::ItemKind::Impl(_) => {
                    // Impl blocks - not needed for MVP type checking
                }
                seen_parser::ItemKind::Trait(trait_def) => {
                    // Handle trait type definitions
                    let trait_methods: Vec<(String, Type)> = trait_def.items.iter()
                        .filter_map(|item| {
                            match &item.kind {
                                seen_parser::TraitItemKind::Function(func) => {
                                    let param_types: Vec<Type> = func.params.iter()
                                        .map(|p| self.resolve_type_annotation(&p.ty))
                                        .collect::<Result<Vec<_>, _>>().ok()?;
                                    
                                    let return_type = if let Some(ret_ty) = &func.return_type {
                                        Box::new(self.resolve_type_annotation(ret_ty).ok()?)
                                    } else {
                                        Box::new(Type::Primitive(PrimitiveType::Unit))
                                    };
                                    
                                    let func_type = Type::Function {
                                        params: param_types,
                                        return_type,
                                    };
                                    
                                    Some((func.name.value.to_string(), func_type))
                                }
                                _ => None,
                            }
                        })
                        .collect();
                    
                    let trait_type = Type::Trait {
                        name: trait_def.name.value.to_string(),
                        methods: trait_methods,
                    };
                    
                    self.env.insert_type(trait_def.name.value.to_string(), trait_type);
                }
                seen_parser::ItemKind::Module(_) => {
                    // Module definitions - not needed for MVP type checking
                }
                seen_parser::ItemKind::Import(_) => {
                    // Import statements - not needed for MVP type checking
                }
                seen_parser::ItemKind::TypeAlias(_) => {
                    // Type aliases - not needed for MVP type checking
                }
                seen_parser::ItemKind::Static(_) => {
                    // Static declarations - not needed for MVP type checking  
                }
                seen_parser::ItemKind::Const(_) => {
                    // Const declarations - not needed for MVP type checking
                }
                seen_parser::ItemKind::ExtensionFunction(_) => {
                    // Extension functions - implement in Step 11
                }
                seen_parser::ItemKind::DataClass(data_class) => {
                    // Add data class constructor to the type environment
                    self.check_data_class(data_class)?;
                }
                seen_parser::ItemKind::SealedClass(_) => {
                    // Sealed classes - implement in Step 11
                }
                seen_parser::ItemKind::Property(prop) => {
                    // Property declarations - type check property definition
                    if let Some(ref ty) = prop.ty {
                        self.resolve_type_annotation(ty)?;
                    }
                    if let Some(ref initializer) = prop.initializer {
                        self.infer_expression_type(initializer)?;
                    }
                    // Delegate and getter/setter validation would go here
                }
            }
        }
        
        // Pass 2: Check function bodies now that all signatures are available
        for item in &program.items {
            match &item.kind {
                seen_parser::ItemKind::Function(func) => {
                    // Set up type parameter context for generic functions
                    if !func.type_params.is_empty() {
                        // Add type parameters to the environment for body checking
                        for type_param in &func.type_params {
                            let type_var = Type::Variable(self.inference.fresh_type_var());
                            self.env.bind(format!("type:{}", type_param.name.value), type_var);
                        }
                        
                        // Now resolve the concrete signature with type parameters available
                        let param_types: Vec<Type> = func.params.iter()
                            .map(|param| self.resolve_type_annotation(&param.ty))
                            .collect::<Result<Vec<_>, _>>()?;
                        
                        let return_type = if let Some(ret_ty) = &func.return_type {
                            Box::new(self.resolve_type_annotation(ret_ty)?)
                        } else {
                            Box::new(Type::Primitive(PrimitiveType::Unit))
                        };
                        
                        // Store the concrete function signature for monomorphization
                        let concrete_func_type = Type::Function {
                            params: param_types,
                            return_type,
                        };
                        self.env.insert_function(
                            format!("{}::__generic_sig", func.name.value),
                            concrete_func_type
                        );
                        
                        // Type check the function body with type parameters available
                        self.check_block(&func.body)?;
                        
                        // Clean up type parameter bindings
                        for type_param in &func.type_params {
                            self.env.remove(&format!("type:{}", type_param.name.value));
                        }
                    } else {
                        // Non-generic function
                        self.check_block(&func.body)?;
                    }
                }
                _ => {
                    // Other items already handled in Pass 1
                }
            }
        }
        
        if self.diagnostics.has_errors() {
            Err(seen_common::SeenError::type_error("Type checking failed"))
        } else {
            Ok(())
        }
    }
    
    pub fn type_environment(&self) -> &TypeEnvironment {
        &self.env
    }
    
    pub fn map_to_c_type(&self, seen_type: &Type) -> SeenResult<String> {
        match seen_type {
            Type::Primitive(PrimitiveType::I8) => Ok("int8_t".to_string()),
            Type::Primitive(PrimitiveType::I16) => Ok("int16_t".to_string()),
            Type::Primitive(PrimitiveType::I32) => Ok("int32_t".to_string()),
            Type::Primitive(PrimitiveType::I64) => Ok("int64_t".to_string()),
            Type::Primitive(PrimitiveType::U8) => Ok("uint8_t".to_string()),
            Type::Primitive(PrimitiveType::U16) => Ok("uint16_t".to_string()),
            Type::Primitive(PrimitiveType::U32) => Ok("uint32_t".to_string()),
            Type::Primitive(PrimitiveType::U64) => Ok("uint64_t".to_string()),
            Type::Primitive(PrimitiveType::F32) => Ok("float".to_string()),
            Type::Primitive(PrimitiveType::F64) => Ok("double".to_string()),
            Type::Primitive(PrimitiveType::Bool) => Ok("bool".to_string()),
            Type::Primitive(PrimitiveType::Char) => Ok("char".to_string()),
            Type::Primitive(PrimitiveType::Str) => Ok("char*".to_string()),
            _ => Err(seen_common::SeenError::type_error("Unsupported C type mapping")),
        }
    }
    
    pub fn map_from_c_type(&self, c_type: &str) -> SeenResult<Type> {
        match c_type {
            "int8_t" => Ok(Type::Primitive(PrimitiveType::I8)),
            "int16_t" => Ok(Type::Primitive(PrimitiveType::I16)),
            "int32_t" => Ok(Type::Primitive(PrimitiveType::I32)),
            "int64_t" => Ok(Type::Primitive(PrimitiveType::I64)),
            "uint8_t" => Ok(Type::Primitive(PrimitiveType::U8)),
            "uint16_t" => Ok(Type::Primitive(PrimitiveType::U16)),
            "uint32_t" => Ok(Type::Primitive(PrimitiveType::U32)),
            "uint64_t" => Ok(Type::Primitive(PrimitiveType::U64)),
            "float" => Ok(Type::Primitive(PrimitiveType::F32)),
            "double" => Ok(Type::Primitive(PrimitiveType::F64)),
            "bool" => Ok(Type::Primitive(PrimitiveType::Bool)),
            "char" => Ok(Type::Primitive(PrimitiveType::Char)),
            "char*" => Ok(Type::Primitive(PrimitiveType::Str)),
            _ => Err(seen_common::SeenError::type_error("Unsupported C type mapping")),
        }
    }
    
    fn resolve_type_annotation(&self, ty: &seen_parser::Type) -> SeenResult<Type> {
        match ty.kind.as_ref() {
            seen_parser::TypeKind::Primitive(prim) => {
                let primitive_type = match prim {
                    seen_parser::PrimitiveType::I8 => PrimitiveType::I8,
                    seen_parser::PrimitiveType::I16 => PrimitiveType::I16,
                    seen_parser::PrimitiveType::I32 => PrimitiveType::I32,
                    seen_parser::PrimitiveType::I64 => PrimitiveType::I64,
                    seen_parser::PrimitiveType::I128 => PrimitiveType::I128,
                    seen_parser::PrimitiveType::U8 => PrimitiveType::U8,
                    seen_parser::PrimitiveType::U16 => PrimitiveType::U16,
                    seen_parser::PrimitiveType::U32 => PrimitiveType::U32,
                    seen_parser::PrimitiveType::U64 => PrimitiveType::U64,
                    seen_parser::PrimitiveType::U128 => PrimitiveType::U128,
                    seen_parser::PrimitiveType::F32 => PrimitiveType::F32,
                    seen_parser::PrimitiveType::F64 => PrimitiveType::F64,
                    seen_parser::PrimitiveType::Bool => PrimitiveType::Bool,
                    seen_parser::PrimitiveType::Char => PrimitiveType::Char,
                    seen_parser::PrimitiveType::Str => PrimitiveType::Str,
                    seen_parser::PrimitiveType::Unit => PrimitiveType::Unit,
                };
                Ok(Type::Primitive(primitive_type))
            }
            seen_parser::TypeKind::Named { path, .. } => {
                // For now, assume it's a primitive type name
                if let Some(segment) = path.segments.first() {
                    match segment.name.value.as_ref() {
                        "i32" => Ok(Type::Primitive(PrimitiveType::I32)),
                        "f64" => Ok(Type::Primitive(PrimitiveType::F64)),
                        "bool" => Ok(Type::Primitive(PrimitiveType::Bool)),
                        "str" => Ok(Type::Primitive(PrimitiveType::Str)),
                        "char" => Ok(Type::Primitive(PrimitiveType::Char)),
                        name => {
                            // First check if this is a type parameter
                            if let Some(type_var) = self.env.get(&format!("type:{}", name)) {
                                Ok(type_var.clone())
                            }
                            // Then look up in type environment
                            else if let Some(ty) = self.env.get_type(name) {
                                Ok(ty.clone())
                            } else {
                                Err(seen_common::SeenError::type_error(&format!("Unknown type: {}", name)))
                            }
                        }
                    }
                } else {
                    Err(seen_common::SeenError::type_error("Empty type path"))
                }
            }
            seen_parser::TypeKind::Nullable(inner_ty) => {
                let inner_type = self.resolve_type_annotation(inner_ty)?;
                // For now, represent nullable types as Named types with Option
                Ok(Type::Named {
                    name: "Option".to_string(),
                    args: vec![inner_type],
                })
            }
            _ => Err(seen_common::SeenError::type_error("Unsupported type annotation")),
        }
    }
    
    fn check_block(&mut self, block: &seen_parser::Block) -> SeenResult<()> {
        // Type check each statement in the block
        for statement in &block.statements {
            self.check_statement(statement)?;
        }
        Ok(())
    }
    
    fn check_statement(&mut self, statement: &Stmt) -> SeenResult<()> {
        match &statement.kind {
            StmtKind::Let(let_stmt) => {
                // Handle let statements with type inference
                if let PatternKind::Identifier(name) = &let_stmt.pattern.kind {
                    // First check if there's a type annotation
                    let expected_type = if let Some(type_annotation) = &let_stmt.ty {
                        Some(self.resolve_type_annotation(type_annotation)?)
                    } else {
                        None
                    };
                    
                    if let Some(init_expr) = &let_stmt.initializer {
                        let inferred_type = self.infer_expression_type(init_expr)?;
                        
                        // If there's a type annotation, check it matches
                        if let Some(expected) = expected_type {
                            // For now, just store the expected type
                            // In a full implementation, we'd unify expected with inferred
                            self.env.bind(format!("var:{}", name.value), expected);
                        } else {
                            self.env.bind(format!("var:{}", name.value), inferred_type);
                        }
                    } else if let Some(expected) = expected_type {
                        // No initializer but has type annotation
                        self.env.bind(format!("var:{}", name.value), expected);
                    }
                }
            }
            StmtKind::Expr(expr) => {
                // Type check expression statements
                self.infer_expression_type(expr)?;
            }
            StmtKind::Return(ret_expr) => {
                // Handle return statements
                if let Some(expr) = ret_expr {
                    self.infer_expression_type(expr)?;
                }
            }
            _ => {
                // Other statement types - not needed for MVP
            }
        }
        Ok(())
    }
    
    fn infer_expression_type(&mut self, expr: &Expr) -> SeenResult<Type> {
        match expr.kind.as_ref() {
            ExprKind::Literal(lit) => {
                self.infer_literal_type(lit)
            }
            ExprKind::Identifier(name) => {
                // Look up variable type
                if let Some(var_type) = self.env.get_variable_type(name.value) {
                    Ok(var_type.clone())
                } else if let Some(func_type) = self.env.get_function_type(name.value) {
                    // Could be a function reference
                    Ok(func_type.clone())
                } else {
                    // For error recovery, return Error type and continue
                    self.diagnostics.error(&format!("Undefined variable: {}", name.value), name.span);
                    Ok(Type::Error)
                }
            }
            ExprKind::Binary { left, op, right } => {
                let left_type = self.infer_expression_type(left)?;
                let right_type = self.infer_expression_type(right)?;
                
                // Skip type checking if either operand is Error type
                if matches!(left_type, Type::Error) || matches!(right_type, Type::Error) {
                    return Ok(Type::Error);
                }
                
                // Unify operand types for most binary operations
                match op {
                    BinaryOp::Add | BinaryOp::Sub |
                    BinaryOp::Mul | BinaryOp::Div => {
                        // For string concatenation, allow str + str
                        if matches!(left_type, Type::Primitive(PrimitiveType::Str)) &&
                           matches!(right_type, Type::Primitive(PrimitiveType::Str)) &&
                           matches!(op, BinaryOp::Add) {
                            Ok(Type::Primitive(PrimitiveType::Str))
                        } else {
                            // Try to unify for numeric types
                            match self.inference.unify(&left_type, &right_type) {
                                Ok(_) => Ok(left_type),
                                Err(_) => {
                                    self.diagnostics.error(
                                        &format!("Type mismatch: cannot apply {:?} to {:?} and {:?}", op, left_type, right_type),
                                        expr.span
                                    );
                                    Ok(Type::Error)
                                }
                            }
                        }
                    }
                    BinaryOp::Eq | BinaryOp::Ne |
                    BinaryOp::Lt | BinaryOp::Le |
                    BinaryOp::Gt | BinaryOp::Ge => {
                        match self.inference.unify(&left_type, &right_type) {
                            Ok(_) => Ok(Type::Primitive(PrimitiveType::Bool)),
                            Err(_) => {
                                self.diagnostics.error(
                                    &format!("Type mismatch in comparison: {:?} and {:?}", left_type, right_type),
                                    expr.span
                                );
                                Ok(Type::Error)
                            }
                        }
                    }
                    BinaryOp::And | BinaryOp::Or => {
                        let bool_type = Type::Primitive(PrimitiveType::Bool);
                        if !matches!(left_type, Type::Primitive(PrimitiveType::Bool)) {
                            self.diagnostics.error(
                                &format!("Expected bool, found {:?}", left_type),
                                left.span
                            );
                        }
                        if !matches!(right_type, Type::Primitive(PrimitiveType::Bool)) {
                            self.diagnostics.error(
                                &format!("Expected bool, found {:?}", right_type),
                                right.span
                            );
                        }
                        Ok(Type::Primitive(PrimitiveType::Bool))
                    }
                    _ => {
                        // Other binary operators - return fresh type variable for now
                        Ok(Type::Variable(self.inference.fresh_type_var()))
                    }
                }
            }
            ExprKind::Call { function, args } => {
                // Function call type inference
                // Check if this is a method call (e.g., y.length())
                if let ExprKind::FieldAccess { object, field } = function.kind.as_ref() {
                    let object_type = self.infer_expression_type(object)?;
                    // For now, just report that the method doesn't exist
                    self.diagnostics.error(
                        &format!("Method '{}' does not exist on type {:?}", field.value, object_type),
                        field.span
                    );
                    return Ok(Type::Error);
                }
                
                if let ExprKind::Identifier(func_name) = function.kind.as_ref() {
                    if let Some(func_type) = self.env.get_function_type(func_name.value).cloned() {
                        match func_type {
                            Type::Function { params, return_type } => {
                                // Check argument count
                                if args.len() != params.len() {
                                    self.diagnostics.error(
                                        &format!("Function '{}' expects {} arguments, got {}", 
                                                func_name.value, params.len(), args.len()),
                                        function.span
                                    );
                                    return Ok(Type::Error);
                                }
                                
                                // Type check each argument
                                for (arg, param_type) in args.iter().zip(params.iter()) {
                                    let arg_type = self.infer_expression_type(arg)?;
                                    if !matches!(arg_type, Type::Error) {
                                        match self.inference.unify(&arg_type, param_type) {
                                            Err(_) => {
                                                self.diagnostics.error(
                                                    &format!("Type mismatch: expected {:?}, found {:?}", param_type, arg_type),
                                                    arg.span
                                                );
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                                
                                Ok(*return_type)
                            }
                            Type::Generic { .. } => {
                                // Generic function call - perform type inference
                                // For now, try to get the concrete signature and perform monomorphization
                                if let Some(concrete_func_type) = self.env.get_function_type(&format!("{}::__generic_sig", func_name.value)).cloned() {
                                    if let Type::Function { params, return_type } = concrete_func_type {
                                        // Infer type arguments from the provided arguments
                                        let arg_types: Vec<Type> = args.iter()
                                            .map(|arg| self.infer_expression_type(arg))
                                            .collect::<Result<Vec<_>, _>>()?;
                                        
                                        // For simple cases, just use the first argument's type as the generic type
                                        // In a full implementation, we'd do proper type inference here
                                        if args.len() == params.len() {
                                            // Create a substitution mapping for generic parameters
                                            let mut substituted_return_type = return_type.as_ref().clone();
                                            
                                            // Simple substitution: replace type variables with concrete types
                                            // This is a simplified approach - a full implementation would use unification
                                            if let (Some(first_arg_type), Some(first_param_type)) = (arg_types.first(), params.first()) {
                                                if let Type::Variable(_) = first_param_type {
                                                    substituted_return_type = self.substitute_type_vars(&substituted_return_type, first_param_type, first_arg_type);
                                                }
                                            }
                                            
                                            Ok(substituted_return_type)
                                        } else {
                                            self.diagnostics.error(
                                                &format!("Generic function '{}' expects {} arguments, got {}", 
                                                        func_name.value, params.len(), args.len()),
                                                function.span
                                            );
                                            Ok(Type::Error)
                                        }
                                    } else {
                                        Ok(Type::Error)
                                    }
                                } else {
                                    // Fallback: return a fresh type variable
                                    Ok(Type::Variable(self.inference.fresh_type_var()))
                                }
                            }
                            _ => {
                                self.diagnostics.error(
                                    &format!("'{}' is not a function", func_name.value),
                                    function.span
                                );
                                Ok(Type::Error)
                            }
                        }
                    } else {
                        self.diagnostics.error(
                            &format!("Undefined function: '{}'", func_name.value),
                            function.span
                        );
                        Ok(Type::Error)
                    }
                } else {
                    // Complex function expressions not yet supported
                    Ok(Type::Variable(self.inference.fresh_type_var()))
                }
            }
            ExprKind::FieldAccess { object, field: _ } => {
                // Field access without call
                let object_type = self.infer_expression_type(object)?;
                // For now, just return Error type for field access
                Ok(Type::Error)
            }
            ExprKind::If { condition, then_branch, else_branch } => {
                // Type check if expression
                let cond_type = self.infer_expression_type(condition)?;
                if !matches!(cond_type, Type::Primitive(PrimitiveType::Bool)) && !matches!(cond_type, Type::Error) {
                    self.diagnostics.error(
                        &format!("If condition must be bool, found {:?}", cond_type),
                        condition.span
                    );
                }
                
                // Type check branches
                self.check_block(then_branch)?;
                if let Some(else_expr) = else_branch {
                    // Else branch is an expression, could be another if or a block
                    self.infer_expression_type(else_expr)?;
                }
                
                // For now, return unit type
                Ok(Type::Primitive(PrimitiveType::Unit))
            }
            _ => {
                // Other expression types - return a fresh type variable for now
                Ok(Type::Variable(self.inference.fresh_type_var()))
            }
        }
    }
    
    pub fn infer_literal_type(&self, literal: &Literal) -> SeenResult<Type> {
        match &literal.kind {
            LiteralKind::Integer(_) => Ok(Type::Primitive(PrimitiveType::I32)),
            LiteralKind::Float(_) => Ok(Type::Primitive(PrimitiveType::F64)),
            LiteralKind::String(_) => Ok(Type::Primitive(PrimitiveType::Str)),
            LiteralKind::Char(_) => Ok(Type::Primitive(PrimitiveType::Char)),
            LiteralKind::Boolean(_) => Ok(Type::Primitive(PrimitiveType::Bool)),
            LiteralKind::Unit => Ok(Type::Primitive(PrimitiveType::Unit)),
        }
    }
    
    /// Simple type variable substitution for generic functions
    fn substitute_type_vars(&self, target: &Type, type_var: &Type, replacement: &Type) -> Type {
        match target {
            Type::Variable(var_id) => {
                if let Type::Variable(target_var_id) = type_var {
                    if var_id == target_var_id {
                        replacement.clone()
                    } else {
                        target.clone()
                    }
                } else {
                    target.clone()
                }
            }
            Type::Function { params, return_type } => {
                let substituted_params = params.iter()
                    .map(|param| self.substitute_type_vars(param, type_var, replacement))
                    .collect();
                let substituted_return = Box::new(self.substitute_type_vars(return_type, type_var, replacement));
                Type::Function {
                    params: substituted_params,
                    return_type: substituted_return,
                }
            }
            Type::Array { element_type, size } => {
                let substituted_element = Box::new(self.substitute_type_vars(element_type, type_var, replacement));
                Type::Array {
                    element_type: substituted_element,
                    size: *size,
                }
            }
            Type::Named { name, args } => {
                let substituted_args = args.iter()
                    .map(|arg| self.substitute_type_vars(arg, type_var, replacement))
                    .collect();
                Type::Named {
                    name: name.clone(),
                    args: substituted_args,
                }
            }
            _ => target.clone(),
        }
    }

    /// Check data class and add constructor to type environment
    fn check_data_class(&mut self, data_class: &seen_parser::DataClass) -> SeenResult<()> {
        // Create field types
        let mut field_types = Vec::new();
        for field in &data_class.fields {
            let field_type = self.resolve_type_annotation(&field.ty)?;
            field_types.push((field.name.value.to_string(), field_type));
        }

        // Create the data class struct type
        let struct_type = Type::Struct {
            name: data_class.name.value.to_string(),
            fields: field_types.clone(),
        };

        // Register the struct type
        self.env.insert_type(data_class.name.value.to_string(), struct_type.clone());

        // Create constructor function type: (field1_type, field2_type, ...) -> DataClassName
        let param_types = field_types.into_iter().map(|(_, ty)| ty).collect();
        let constructor_type = Type::Function {
            params: param_types,
            return_type: Box::new(Type::Named {
                name: data_class.name.value.to_string(),
                args: vec![],
            }),
        };

        // Register constructor function
        self.env.insert_function(data_class.name.value.to_string(), constructor_type);

        Ok(())
    }
}

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
    }
}