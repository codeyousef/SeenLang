//! Type checking implementation

use crate::types::*;
use crate::inference::InferenceEngine;
use seen_common::{SeenResult, SeenError, Diagnostics};
use seen_parser::{Program, Block, Stmt, StmtKind, Expr, ExprKind, Literal, LiteralKind, BinaryOp, PatternKind, Let};

/// Type checker
pub struct TypeChecker {
    pub env: TypeEnvironment,
    diagnostics: Diagnostics,
    pub inference: InferenceEngine,
}

impl TypeChecker {
    pub fn new() -> Self {
        Self {
            env: TypeEnvironment::new(),
            diagnostics: Diagnostics::new(),
            inference: InferenceEngine::new(),
        }
    }
    
    pub fn diagnostics(&self) -> &Diagnostics {
        &self.diagnostics
    }
    
    pub fn check_program(&mut self, program: &Program) -> SeenResult<()> {
        // Comprehensive type checking implementation
        for item in &program.items {
            match &item.kind {
                seen_parser::ItemKind::Function(func) => {
                    // Function validation and type checking
                    if func.name.value.is_empty() {
                        self.diagnostics.error("Function name cannot be empty", func.name.span);
                    }
                    
                    // Store function type in environment
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
                    
                    // Type check function body
                    self.check_block(&func.body)?;
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
                seen_parser::ItemKind::Trait(_) => {
                    // Trait definitions - not needed for MVP type checking
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
                            // Look up in type environment
                            if let Some(ty) = self.env.get_type(name) {
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
                    if let Some(init_expr) = &let_stmt.initializer {
                        let inferred_type = self.infer_expression_type(init_expr)?;
                        self.env.bind(format!("var:{}", name.value), inferred_type);
                    }
                }
            }
            StmtKind::Expr(expr) => {
                // Type check expression statements
                self.infer_expression_type(expr)?;
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
                } else {
                    Err(seen_common::SeenError::type_error(&format!("Undefined variable: {}", name.value)))
                }
            }
            ExprKind::Binary { left, op, right } => {
                let left_type = self.infer_expression_type(left)?;
                let right_type = self.infer_expression_type(right)?;
                
                // Unify operand types for most binary operations
                match op {
                    BinaryOp::Add | BinaryOp::Sub |
                    BinaryOp::Mul | BinaryOp::Div => {
                        self.inference.unify(&left_type, &right_type)?;
                        Ok(left_type)
                    }
                    BinaryOp::Eq | BinaryOp::Ne |
                    BinaryOp::Lt | BinaryOp::Le |
                    BinaryOp::Gt | BinaryOp::Ge => {
                        self.inference.unify(&left_type, &right_type)?;
                        Ok(Type::Primitive(PrimitiveType::Bool))
                    }
                    BinaryOp::And | BinaryOp::Or => {
                        let bool_type = Type::Primitive(PrimitiveType::Bool);
                        self.inference.unify(&left_type, &bool_type)?;
                        self.inference.unify(&right_type, &bool_type)?;
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
                if let ExprKind::Identifier(func_name) = function.kind.as_ref() {
                    if let Some(func_type) = self.env.get_function_type(func_name.value).cloned() {
                        match func_type {
                            Type::Function { params, return_type } => {
                                // Check argument count
                                if args.len() != params.len() {
                                    return Err(seen_common::SeenError::type_error(
                                        &format!("Function {} expects {} arguments, got {}", 
                                                func_name.value, params.len(), args.len())
                                    ));
                                }
                                
                                // Type check each argument
                                for (arg, param_type) in args.iter().zip(params.iter()) {
                                    let arg_type = self.infer_expression_type(arg)?;
                                    self.inference.unify(&arg_type, param_type)?;
                                }
                                
                                Ok(*return_type)
                            }
                            _ => Err(seen_common::SeenError::type_error(
                                &format!("{} is not a function", func_name.value)
                            ))
                        }
                    } else {
                        Err(seen_common::SeenError::type_error(
                            &format!("Undefined function: {}", func_name.value)
                        ))
                    }
                } else {
                    Err(seen_common::SeenError::type_error("Complex function expressions not supported"))
                }
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
}

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
    }
}