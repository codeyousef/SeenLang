//! Type mapping between C and Seen types

use crate::CType;
use seen_typechecker::types::{Type, PrimitiveType};

/// Map C types to Seen types
pub fn c_to_seen_type(c_type: &CType) -> Type {
    match c_type {
        CType::Void => Type::Primitive(PrimitiveType::Unit),
        CType::Char | CType::SignedChar => Type::Primitive(PrimitiveType::I8),
        CType::UnsignedChar => Type::Primitive(PrimitiveType::U8),
        CType::Short => Type::Primitive(PrimitiveType::I16),
        CType::UnsignedShort => Type::Primitive(PrimitiveType::U16),
        CType::Int => Type::Primitive(PrimitiveType::I32),
        CType::UnsignedInt => Type::Primitive(PrimitiveType::U32),
        CType::Long => {
            // Platform-dependent: assume 64-bit on modern systems
            Type::Primitive(PrimitiveType::I64)
        }
        CType::UnsignedLong => Type::Primitive(PrimitiveType::U64),
        CType::LongLong => Type::Primitive(PrimitiveType::I64),
        CType::UnsignedLongLong => Type::Primitive(PrimitiveType::U64),
        CType::Float => Type::Primitive(PrimitiveType::F32),
        CType::Double => Type::Primitive(PrimitiveType::F64),
        CType::LongDouble => Type::Primitive(PrimitiveType::F64), // Map to f64
        CType::Bool => Type::Primitive(PrimitiveType::Bool),
        CType::Pointer(inner) => {
            // In the new memory model, pointers map directly to the inner type
            // The compiler will handle borrowing automatically
            c_to_seen_type(inner)
        }
        CType::Array(inner, size) => {
            Type::Array {
                element_type: Box::new(c_to_seen_type(inner)),
                size: *size,
            }
        }
        CType::Struct(name) => {
            Type::Struct {
                name: name.clone(),
                fields: vec![], // Fields populated separately
            }
        }
        CType::Union(name) => {
            // Unions represented as structs with special marker
            Type::Struct {
                name: format!("union_{}", name),
                fields: vec![],
            }
        }
        CType::Enum(name) => {
            // Enums map to named types in Seen
            Type::Named {
                name: name.clone(),
                args: vec![],
            }
        }
        CType::Function(func_type) => {
            let params = func_type.parameter_types.iter()
                .map(c_to_seen_type)
                .collect();
            let return_type = Box::new(c_to_seen_type(&func_type.return_type));
            
            Type::Function {
                params,
                return_type,
            }
        }
    }
}

/// Map Seen types to C types for reverse binding
pub fn seen_to_c_type(seen_type: &Type) -> Option<CType> {
    match seen_type {
        Type::Primitive(prim) => match prim {
            PrimitiveType::Unit => Some(CType::Void),
            PrimitiveType::Bool => Some(CType::Bool),
            PrimitiveType::I8 => Some(CType::SignedChar),
            PrimitiveType::U8 => Some(CType::UnsignedChar),
            PrimitiveType::I16 => Some(CType::Short),
            PrimitiveType::U16 => Some(CType::UnsignedShort),
            PrimitiveType::I32 => Some(CType::Int),
            PrimitiveType::U32 => Some(CType::UnsignedInt),
            PrimitiveType::I64 => Some(CType::LongLong),
            PrimitiveType::U64 => Some(CType::UnsignedLongLong),
            PrimitiveType::F32 => Some(CType::Float),
            PrimitiveType::F64 => Some(CType::Double),
            PrimitiveType::Str => {
                // String maps to const char*
                Some(CType::Pointer(Box::new(CType::Char)))
            }
            _ => None,
        },
        // No more Reference types - this case is removed in the new memory model
        Type::Array { element_type, size } => {
            seen_to_c_type(element_type).map(|t| CType::Array(Box::new(t), *size))
        }
        Type::Function { params, return_type } => {
            let param_types: Option<Vec<_>> = params.iter()
                .map(seen_to_c_type)
                .collect();
            
            let ret_type = seen_to_c_type(return_type)?;
            
            param_types.map(|pts| {
                CType::Function(Box::new(crate::CFunctionType {
                    return_type: ret_type,
                    parameter_types: pts,
                    is_variadic: false,
                }))
            })
        }
        _ => None,
    }
}

/// Get the size of a C type in bytes
pub fn c_type_size(c_type: &CType) -> usize {
    match c_type {
        CType::Void => 0,
        CType::Char | CType::SignedChar | CType::UnsignedChar => 1,
        CType::Short | CType::UnsignedShort => 2,
        CType::Int | CType::UnsignedInt => 4,
        CType::Float => 4,
        CType::Long | CType::UnsignedLong => 8, // Assuming 64-bit
        CType::LongLong | CType::UnsignedLongLong => 8,
        CType::Double => 8,
        CType::LongDouble => 16, // Platform-dependent
        CType::Bool => 1,
        CType::Pointer(_) => std::mem::size_of::<*const u8>(),
        CType::Array(inner, Some(size)) => c_type_size(inner) * size,
        CType::Array(_, None) => std::mem::size_of::<*const u8>(), // Unsized array = pointer
        _ => 0, // Structs, unions, enums need separate handling
    }
}

/// Get the alignment requirement of a C type
pub fn c_type_alignment(c_type: &CType) -> usize {
    match c_type {
        CType::Void => 1,
        CType::Char | CType::SignedChar | CType::UnsignedChar => 1,
        CType::Short | CType::UnsignedShort => 2,
        CType::Int | CType::UnsignedInt => 4,
        CType::Float => 4,
        CType::Long | CType::UnsignedLong => 8,
        CType::LongLong | CType::UnsignedLongLong => 8,
        CType::Double => 8,
        CType::LongDouble => 16,
        CType::Bool => 1,
        CType::Pointer(_) => std::mem::align_of::<*const u8>(),
        CType::Array(inner, _) => c_type_alignment(inner),
        _ => std::mem::align_of::<*const u8>(), // Default to pointer alignment
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_primitive_type_mapping() {
        assert!(matches!(
            c_to_seen_type(&CType::Int),
            Type::Primitive(PrimitiveType::I32)
        ));
        
        assert!(matches!(
            c_to_seen_type(&CType::Double),
            Type::Primitive(PrimitiveType::F64)
        ));
        
        assert!(matches!(
            c_to_seen_type(&CType::Void),
            Type::Primitive(PrimitiveType::Unit)
        ));
    }
    
    #[test]
    fn test_pointer_type_mapping() {
        let char_ptr = CType::Pointer(Box::new(CType::Char));
        let seen_type = c_to_seen_type(&char_ptr);
        
        // C char* should map to a pointer type in Seen's automatic inference system
        assert!(matches!(seen_type, Type::Primitive(_)));
    }
    
    #[test]
    fn test_bidirectional_mapping() {
        let seen_i32 = Type::Primitive(PrimitiveType::I32);
        let c_type = seen_to_c_type(&seen_i32).unwrap();
        assert_eq!(c_type, CType::Int);
        
        let back_to_seen = c_to_seen_type(&c_type);
        assert!(matches!(back_to_seen, Type::Primitive(PrimitiveType::I32)));
    }
    
    #[test]
    fn test_type_sizes() {
        assert_eq!(c_type_size(&CType::Char), 1);
        assert_eq!(c_type_size(&CType::Int), 4);
        assert_eq!(c_type_size(&CType::Double), 8);
        assert_eq!(c_type_size(&CType::Pointer(Box::new(CType::Int))), std::mem::size_of::<*const u8>());
    }
}