use crate::error::{CodeGenError, Result};
// Note: In inkwell 0.2.0, we use numeric address space instead of named variants
use inkwell::context::Context;
use inkwell::types::BasicTypeEnum;
use seen_parser::ast::Type as AstType;

/// Information about a struct type
#[derive(Debug, Clone)]
pub struct StructInfo<'ctx> {
    pub llvm_type: inkwell::types::StructType<'ctx>,
    pub field_names: Vec<String>,
    pub field_types: Vec<BasicTypeEnum<'ctx>>,
}

/// Information about an enum variant
#[derive(Debug, Clone)]
pub struct EnumVariantInfo<'ctx> {
    pub name: String,
    pub tag: u64,
    pub data_types: Option<Vec<BasicTypeEnum<'ctx>>>,
}

/// Information about an enum type
#[derive(Debug, Clone)]
pub struct EnumInfo<'ctx> {
    pub llvm_type: inkwell::types::StructType<'ctx>,
    pub variants: Vec<EnumVariantInfo<'ctx>>,
}

/// Type system for the Seen language
pub struct TypeSystem<'ctx> {
    context: &'ctx Context,
    pub struct_types: std::collections::HashMap<String, StructInfo<'ctx>>,
    pub enum_types: std::collections::HashMap<String, EnumInfo<'ctx>>,
}

impl<'ctx> TypeSystem<'ctx> {
    /// Create a new type system
    pub fn new(context: &'ctx Context) -> Self {
        Self { 
            context,
            struct_types: std::collections::HashMap::new(),
            enum_types: std::collections::HashMap::new(),
        }
    }

    /// Convert an AST type to an LLVM type
    pub fn convert_type(&self, ast_type: &AstType) -> Result<BasicTypeEnum<'ctx>> {
        match ast_type {
            AstType::Simple(name) => self.get_type_by_name(name),
            AstType::Array(element_type) => {
                let element_llvm_type = self.convert_type(element_type)?;

                // For MVP, we'll use fixed size arrays of length 10
                // Later versions would support variable length arrays
                let array_type = match element_llvm_type {
                    BasicTypeEnum::IntType(t) => t.array_type(10).into(),
                    BasicTypeEnum::FloatType(t) => t.array_type(10).into(),
                    BasicTypeEnum::PointerType(t) => t.array_type(10).into(),
                    BasicTypeEnum::ArrayType(t) => t.array_type(10).into(),
                    BasicTypeEnum::StructType(t) => t.array_type(10).into(),
                    BasicTypeEnum::VectorType(t) => t.array_type(10).into(),
                    BasicTypeEnum::ScalableVectorType(_) => {
                        todo!("Handle ScalableVectorType in array creation")
                    }
                };

                Ok(array_type)
            }
            AstType::Struct(struct_name) => {
                if let Some(struct_info) = self.struct_types.get(struct_name) {
                    Ok(struct_info.llvm_type.into())
                } else {
                    Err(CodeGenError::UnknownType(format!(
                        "Struct type '{}' not found. Make sure it is declared before use.",
                        struct_name
                    )))
                }
            }
            AstType::Enum(enum_name) => {
                if let Some(enum_info) = self.enum_types.get(enum_name) {
                    Ok(enum_info.llvm_type.into())
                } else {
                    Err(CodeGenError::UnknownType(format!(
                        "Enum type '{}' not found. Make sure it is declared before use.",
                        enum_name
                    )))
                }
            }
            AstType::Generic(name, args) => {
                // Basic monomorphization - create concrete enum types for common generics
                match name.as_str() {
                    "Option" => {
                        if args.len() != 1 {
                            return Err(CodeGenError::UnsupportedOperation(
                                "Option must have exactly one type parameter".to_string()
                            ));
                        }
                        
                        // Create a monomorphized Option enum for this specific type
                        let concrete_name = format!("Option_{}", self.type_name_for_monomorphization(&args[0]));
                        
                        if let Some(enum_info) = self.enum_types.get(&concrete_name) {
                            Ok(enum_info.llvm_type.into())
                        } else {
                            // For now, return error - we'll implement auto-generation later
                            Err(CodeGenError::UnsupportedOperation(
                                format!("Monomorphized type '{}' not yet created", concrete_name)
                            ))
                        }
                    }
                    "Result" => {
                        if args.len() != 2 {
                            return Err(CodeGenError::UnsupportedOperation(
                                "Result must have exactly two type parameters".to_string()
                            ));
                        }
                        
                        // Create a monomorphized Result enum for these specific types
                        let concrete_name = format!("Result_{}_{}", 
                            self.type_name_for_monomorphization(&args[0]),
                            self.type_name_for_monomorphization(&args[1])
                        );
                        
                        if let Some(enum_info) = self.enum_types.get(&concrete_name) {
                            Ok(enum_info.llvm_type.into())
                        } else {
                            // For now, return error - we'll implement auto-generation later
                            Err(CodeGenError::UnsupportedOperation(
                                format!("Monomorphized type '{}' not yet created", concrete_name)
                            ))
                        }
                    }
                    "Vec" => {
                        if args.len() != 1 {
                            return Err(CodeGenError::UnsupportedOperation(
                                "Vec must have exactly one type parameter".to_string()
                            ));
                        }
                        
                        // Create a monomorphized Vec struct for this specific type
                        let concrete_name = format!("Vec_{}", self.type_name_for_monomorphization(&args[0]));
                        
                        if let Some(struct_info) = self.struct_types.get(&concrete_name) {
                            Ok(struct_info.llvm_type.into())
                        } else {
                            // For now, return error - we'll implement auto-generation later
                            Err(CodeGenError::UnsupportedOperation(
                                format!("Monomorphized type '{}' not yet created", concrete_name)
                            ))
                        }
                    }
                    _ => {
                        Err(CodeGenError::UnsupportedOperation(
                            format!("Generic type '{}' not yet supported", name)
                        ))
                    }
                }
            }
            AstType::Pointer(inner_type) => {
                // Pointer to another type
                let _inner_llvm_type = self.convert_type(inner_type)?;
                // For now, all pointers are i8 pointers (generic pointer)
                Ok(self.context.ptr_type(0.into()).into())
            }
        }
    }

    /// Get an LLVM type by name
    fn get_type_by_name(&self, name: &str) -> Result<BasicTypeEnum<'ctx>> {
        match name {
            "int" | "Int" => Ok(self.context.i64_type().into()),
            "float" | "Float" => Ok(self.context.f64_type().into()),
            "bool" | "Bool" => Ok(self.context.bool_type().into()),
            "string" | "String" => {
                // In LLVM, strings are typically represented as pointers to character arrays
                // For our MVP, we'll use i8 pointers to represent strings
                Ok(self.context.ptr_type(0.into()).into())
            }
            "char" | "Char" => Ok(self.context.i8_type().into()),
            "void" | "Void" => {
                // For void return types, use a special case in the calling code
                // This is a placeholder to maintain interface consistency
                Err(CodeGenError::UnknownType(format!(
                    "Void type not valid in this context"
                )))
            }
            _ => Err(CodeGenError::UnknownType(name.to_string())),
        }
    }

    /// Get the void type
    pub fn void_type(&self) -> inkwell::types::VoidType<'ctx> {
        self.context.void_type()
    }

    /// Get the integer type
    pub fn int_type(&self) -> inkwell::types::IntType<'ctx> {
        self.context.i64_type()
    }

    /// Get the float type
    pub fn float_type(&self) -> inkwell::types::FloatType<'ctx> {
        self.context.f64_type()
    }

    /// Get the boolean type
    pub fn bool_type(&self) -> inkwell::types::IntType<'ctx> {
        self.context.bool_type()
    }

    /// Get the string type (pointer to i8)
    pub fn string_type(&self) -> inkwell::types::PointerType<'ctx> {
        self.context.ptr_type(0.into())
    }

    /// Get the char type (i8)
    pub fn char_type(&self) -> inkwell::types::IntType<'ctx> {
        self.context.i8_type()
    }

    /// Define a struct type with field information
    pub fn define_struct_type(&mut self, name: &str, field_names: Vec<String>, field_types: Vec<BasicTypeEnum<'ctx>>) -> Result<inkwell::types::StructType<'ctx>> {
        let struct_type = self.context.opaque_struct_type(name);
        struct_type.set_body(&field_types, false);
        
        let struct_info = StructInfo {
            llvm_type: struct_type,
            field_names,
            field_types,
        };
        
        self.struct_types.insert(name.to_string(), struct_info);
        Ok(struct_type)
    }

    /// Get a struct type by name
    pub fn get_struct_type(&self, name: &str) -> Option<inkwell::types::StructType<'ctx>> {
        self.struct_types.get(name).map(|info| info.llvm_type)
    }

    /// Get field index by name
    pub fn get_field_index(&self, struct_name: &str, field_name: &str) -> Option<usize> {
        self.struct_types.get(struct_name)
            .and_then(|info| info.field_names.iter().position(|name| name == field_name))
    }

    /// Get struct info by name
    pub fn get_struct_info(&self, name: &str) -> Option<&StructInfo<'ctx>> {
        self.struct_types.get(name)
    }

    /// Define an enum type with variant information
    pub fn define_enum_type(&mut self, name: &str, variants: &[seen_parser::ast::EnumVariant]) -> Result<inkwell::types::StructType<'ctx>> {
        // Create enum as a tagged union: { i64 tag, [largest_variant_size] data }
        let tag_type = self.context.i64_type();
        
        // Find the largest variant data size
        let mut max_size = 0u32;
        let mut enum_variants = Vec::new();
        
        for (index, variant) in variants.iter().enumerate() {
            let data_types = if let Some(data) = &variant.data {
                let mut types = Vec::new();
                for data_type in data {
                    types.push(self.convert_type(data_type)?);
                }
                
                // Calculate size (simplified - just count fields for now)
                max_size = max_size.max(types.len() as u32);
                Some(types)
            } else {
                None
            };
            
            enum_variants.push(EnumVariantInfo {
                name: variant.name.clone(),
                tag: index as u64,
                data_types,
            });
        }
        
        // Create a struct type for the enum: { tag, data_array }
        // For simplicity, use an array of i64s for the data union
        let data_type = self.context.i64_type().array_type(max_size.max(1));
        let enum_struct_type = self.context.opaque_struct_type(name);
        enum_struct_type.set_body(&[tag_type.into(), data_type.into()], false);
        
        let enum_info = EnumInfo {
            llvm_type: enum_struct_type,
            variants: enum_variants,
        };
        
        self.enum_types.insert(name.to_string(), enum_info);
        Ok(enum_struct_type)
    }

    /// Get an enum type by name
    pub fn get_enum_type(&self, name: &str) -> Option<inkwell::types::StructType<'ctx>> {
        self.enum_types.get(name).map(|info| info.llvm_type)
    }

    /// Get enum info by name
    pub fn get_enum_info(&self, name: &str) -> Option<&EnumInfo<'ctx>> {
        self.enum_types.get(name)
    }

    /// Get variant info by name
    pub fn get_variant_info(&self, enum_name: &str, variant_name: &str) -> Option<&EnumVariantInfo<'ctx>> {
        self.enum_types.get(enum_name)
            .and_then(|info| info.variants.iter().find(|v| v.name == variant_name))
    }

    /// Generate a mangled name for a type for use in monomorphization
    pub fn type_name_for_monomorphization(&self, ast_type: &AstType) -> String {
        match ast_type {
            AstType::Simple(name) => name.clone(),
            AstType::Array(element_type) => format!("Array_{}", self.type_name_for_monomorphization(element_type)),
            AstType::Struct(name) => name.clone(),
            AstType::Enum(name) => name.clone(),
            AstType::Generic(name, args) => {
                let arg_names: Vec<String> = args.iter()
                    .map(|arg| self.type_name_for_monomorphization(arg))
                    .collect();
                format!("{}_{}", name, arg_names.join("_"))
            }
            AstType::Pointer(inner_type) => {
                format!("Ptr_{}", self.type_name_for_monomorphization(inner_type))
            }
        }
    }
}
