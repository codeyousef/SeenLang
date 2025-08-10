//! Type definitions for the Seen type system

use serde::{Deserialize, Serialize};
use std::fmt;
use hashbrown::HashMap;

/// Unique identifier for types
pub type TypeId = u32;

/// Type variable identifier  
pub type TypeVar = u32;

/// The main type representation
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Type {
    /// Primitive types
    Primitive(PrimitiveType),
    /// Type variables for inference
    Variable(TypeVar),
    /// Function types
    Function {
        params: Vec<Type>,
        return_type: Box<Type>,
    },
    /// Tuple types
    Tuple(Vec<Type>),
    /// Array types
    Array {
        element_type: Box<Type>,
        size: Option<usize>,
    },
    /// Named types (structs, enums, etc.)
    Named {
        name: String,
        args: Vec<Type>,
    },
    /// Generic types
    Generic {
        name: String,
        bounds: Vec<Type>,
    },
    /// Struct types
    Struct {
        name: String,
        fields: Vec<(String, Type)>,
    },
    /// Enum types
    Enum {
        name: String,
        variants: Vec<(String, Vec<Type>)>,
    },
    /// Trait types
    Trait {
        name: String,
        methods: Vec<(String, Type)>,
    },
    /// Type implementing a trait
    TraitImpl {
        base_type: Box<Type>,
        trait_name: String,
    },
    /// Error type for error recovery
    Error,
    /// Unknown type (for type inference before unification)
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PrimitiveType {
    I8, I16, I32, I64, I128,
    U8, U16, U32, U64, U128,
    F32, F64,
    Bool,
    Char,
    Str,
    Unit,
    Never,
}

/// Type environment for tracking variable types
#[derive(Debug, Clone)]
pub struct TypeEnvironment {
    bindings: HashMap<String, Type>,
    parent: Option<Box<TypeEnvironment>>,
}

impl TypeEnvironment {
    pub fn new() -> Self {
        Self {
            bindings: HashMap::new(),
            parent: None,
        }
    }
    
    pub fn with_parent(parent: TypeEnvironment) -> Self {
        Self {
            bindings: HashMap::new(),
            parent: Some(Box::new(parent)),
        }
    }
    
    pub fn bind(&mut self, name: String, ty: Type) {
        self.bindings.insert(name, ty);
    }
    
    pub fn lookup(&self, name: &str) -> Option<&Type> {
        self.bindings.get(name)
            .or_else(|| self.parent.as_ref().and_then(|p| p.lookup(name)))
    }
    
    pub fn insert_function(&mut self, name: String, func_type: Type) {
        self.bindings.insert(format!("func:{}", name), func_type);
    }
    
    pub fn insert_type(&mut self, name: String, type_def: Type) {
        self.bindings.insert(format!("type:{}", name), type_def);
    }
    
    pub fn get_function_type(&self, name: &str) -> Option<&Type> {
        self.bindings.get(&format!("func:{}", name))
    }
    
    pub fn get_type(&self, name: &str) -> Option<&Type> {
        self.bindings.get(&format!("type:{}", name))
    }
    
    pub fn get_variable_type(&self, name: &str) -> Option<&Type> {
        self.bindings.get(&format!("var:{}", name))
    }
    
    pub fn contains_function(&self, name: &str) -> bool {
        self.bindings.contains_key(&format!("func:{}", name))
    }
    
    pub fn get(&self, key: &str) -> Option<&Type> {
        self.bindings.get(key)
            .or_else(|| self.parent.as_ref().and_then(|p| p.get(key)))
    }
    
    pub fn remove(&mut self, key: &str) -> Option<Type> {
        self.bindings.remove(key)
    }
}

/// Type substitution for inference
#[derive(Debug, Clone)]
pub struct Substitution {
    mappings: HashMap<TypeVar, Type>,
}

impl Substitution {
    pub fn new() -> Self {
        Self {
            mappings: HashMap::new(),
        }
    }
    
    pub fn bind(&mut self, var: TypeVar, ty: Type) {
        self.mappings.insert(var, ty);
    }
    
    pub fn lookup(&self, var: TypeVar) -> Option<&Type> {
        self.mappings.get(&var)
    }
    
    pub fn apply(&self, ty: &Type) -> Type {
        match ty {
            Type::Variable(var) => {
                if let Some(substituted) = self.lookup(*var) {
                    self.apply(substituted)
                } else {
                    ty.clone()
                }
            }
            Type::Function { params, return_type } => {
                Type::Function {
                    params: params.iter().map(|p| self.apply(p)).collect(),
                    return_type: Box::new(self.apply(return_type)),
                }
            }
            Type::Tuple(types) => {
                Type::Tuple(types.iter().map(|t| self.apply(t)).collect())
            }
            Type::Array { element_type, size } => {
                Type::Array {
                    element_type: Box::new(self.apply(element_type)),
                    size: *size,
                }
            }
            Type::Named { name, args } => {
                Type::Named {
                    name: name.clone(),
                    args: args.iter().map(|a| self.apply(a)).collect(),
                }
            }
            Type::TraitImpl { base_type, trait_name } => {
                Type::TraitImpl {
                    base_type: Box::new(self.apply(base_type)),
                    trait_name: trait_name.clone(),
                }
            }
            _ => ty.clone(),
        }
    }
    
    pub fn compose(&self, other: &Substitution) -> Substitution {
        let mut result = Substitution::new();
        
        // Apply this substitution to other's mappings
        for (var, ty) in &other.mappings {
            result.bind(*var, self.apply(ty));
        }
        
        // Add our mappings
        for (var, ty) in &self.mappings {
            if !result.mappings.contains_key(var) {
                result.bind(*var, ty.clone());
            }
        }
        
        result
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Primitive(p) => write!(f, "{}", p),
            Type::Variable(v) => write!(f, "T{}", v),
            Type::Function { params, return_type } => {
                write!(f, "(")?;
                for (i, param) in params.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", param)?;
                }
                write!(f, ") -> {}", return_type)
            }
            Type::Tuple(types) => {
                write!(f, "(")?;
                for (i, ty) in types.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", ty)?;
                }
                write!(f, ")")
            }
            Type::Array { element_type, size } => {
                if let Some(size) = size {
                    write!(f, "[{}; {}]", element_type, size)
                } else {
                    write!(f, "[{}]", element_type)
                }
            }
            Type::Named { name, args } => {
                write!(f, "{}", name)?;
                if !args.is_empty() {
                    write!(f, "<")?;
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 { write!(f, ", ")?; }
                        write!(f, "{}", arg)?;
                    }
                    write!(f, ">")?;
                }
                Ok(())
            }
            Type::Generic { name, bounds } => {
                write!(f, "{}", name)?;
                if !bounds.is_empty() {
                    write!(f, ": ")?;
                    for (i, bound) in bounds.iter().enumerate() {
                        if i > 0 { write!(f, " + ")?; }
                        write!(f, "{}", bound)?;
                    }
                }
                Ok(())
            }
            Type::Struct { name, fields } => {
                write!(f, "struct {} {{", name)?;
                for (i, (field_name, field_type)) in fields.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}: {}", field_name, field_type)?;
                }
                write!(f, "}}")
            }
            Type::Enum { name, variants } => {
                write!(f, "enum {} {{", name)?;
                for (i, (variant_name, variant_types)) in variants.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", variant_name)?;
                    if !variant_types.is_empty() {
                        write!(f, "(")?;
                        for (j, variant_type) in variant_types.iter().enumerate() {
                            if j > 0 { write!(f, ", ")?; }
                            write!(f, "{}", variant_type)?;
                        }
                        write!(f, ")")?;
                    }
                }
                write!(f, "}}")
            }
            Type::Trait { name, methods } => {
                write!(f, "trait {} {{", name)?;
                for (i, (method_name, method_type)) in methods.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}: {}", method_name, method_type)?;
                }
                write!(f, "}}")
            }
            Type::TraitImpl { base_type, trait_name } => {
                write!(f, "{}: {}", base_type, trait_name)
            }
            Type::Error => write!(f, "<!error!>"),
            Type::Unknown => write!(f, "<!unknown!>"),
        }
    }
}

impl fmt::Display for PrimitiveType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            PrimitiveType::I8 => "i8",
            PrimitiveType::I16 => "i16", 
            PrimitiveType::I32 => "i32",
            PrimitiveType::I64 => "i64",
            PrimitiveType::I128 => "i128",
            PrimitiveType::U8 => "u8",
            PrimitiveType::U16 => "u16",
            PrimitiveType::U32 => "u32", 
            PrimitiveType::U64 => "u64",
            PrimitiveType::U128 => "u128",
            PrimitiveType::F32 => "f32",
            PrimitiveType::F64 => "f64",
            PrimitiveType::Bool => "bool",
            PrimitiveType::Char => "char",
            PrimitiveType::Str => "str",
            PrimitiveType::Unit => "()",
            PrimitiveType::Never => "!",
        };
        write!(f, "{}", name)
    }
}

impl Default for TypeEnvironment {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for Substitution {
    fn default() -> Self {
        Self::new()
    }
}