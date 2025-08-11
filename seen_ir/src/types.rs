use inkwell::types::BasicTypeEnum;
// Note: In inkwell 0.2.0, we use numeric address space instead of named variants
use inkwell::context::Context;
use seen_parser::ast::Type as AstType;
use crate::error::{CodeGenError, Result};

/// Type system for the Seen language
pub struct TypeSystem<'ctx> {
    context: &'ctx Context,
}

impl<'ctx> TypeSystem<'ctx> {
    /// Create a new type system
    pub fn new(context: &'ctx Context) -> Self {
        Self { context }
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
                    BasicTypeEnum::ScalableVectorType(t) => t.array_type(10).into(),
                };
                
                Ok(array_type)
            },
            AstType::Struct(struct_name) => {
                // Create a struct type with the given name
                let struct_type = self.context.opaque_struct_type(struct_name);
                Ok(struct_type.into())
            },
        }
    }

    /// Get an LLVM type by name
    fn get_type_by_name(&self, name: &str) -> Result<BasicTypeEnum<'ctx>> {
        match name {
            "int" => Ok(self.context.i64_type().into()),
            "float" => Ok(self.context.f64_type().into()),
            "bool" => Ok(self.context.bool_type().into()),
            "string" => {
                // In LLVM, strings are typically represented as pointers to character arrays
                // For our MVP, we'll use i8 pointers to represent strings
                Ok(self.context.ptr_type(0.into()).into())
            }
            "void" => {
                // For void return types, use a special case in the calling code
                // This is a placeholder to maintain interface consistency
                Err(CodeGenError::UnknownType(format!("Void type not valid in this context")))
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
}
