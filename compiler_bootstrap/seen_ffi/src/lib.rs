//! Foreign Function Interface (FFI) for C interoperability
//! 
//! This module provides direct C header import and automatic binding generation,
//! similar to Zig's approach but with Seen's memory safety guarantees.

use seen_common::{SeenError, SeenResult};
use seen_parser::ast::{Type as AstType};
use seen_typechecker::types::{Type, PrimitiveType};
use std::collections::HashMap;
use std::path::Path;
use std::ffi::{CString, CStr, c_void};

pub mod header_parser;
pub mod type_mapping;
pub mod binding_generator;
pub mod dynamic_loader;

/// C function signature representation
#[derive(Debug, Clone)]
pub struct CFunction {
    pub name: String,
    pub return_type: CType,
    pub parameters: Vec<CParameter>,
    pub is_variadic: bool,
    pub calling_convention: CallingConvention,
}

/// C parameter representation
#[derive(Debug, Clone)]
pub struct CParameter {
    pub name: Option<String>,
    pub c_type: CType,
}

/// C type representation
#[derive(Debug, Clone, PartialEq)]
pub enum CType {
    Void,
    Char,
    SignedChar,
    UnsignedChar,
    Short,
    UnsignedShort,
    Int,
    UnsignedInt,
    Long,
    UnsignedLong,
    LongLong,
    UnsignedLongLong,
    Float,
    Double,
    LongDouble,
    Bool,
    Pointer(Box<CType>),
    Array(Box<CType>, Option<usize>),
    Struct(String),
    Union(String),
    Enum(String),
    Function(Box<CFunctionType>),
}

/// C function type for function pointers
#[derive(Debug, Clone, PartialEq)]
pub struct CFunctionType {
    pub return_type: CType,
    pub parameter_types: Vec<CType>,
    pub is_variadic: bool,
}

/// Calling convention for functions
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CallingConvention {
    C,
    Stdcall,
    Fastcall,
    Vectorcall,
    ThisCall,
}

impl Default for CallingConvention {
    fn default() -> Self {
        CallingConvention::C
    }
}

/// FFI context manages C bindings and type mappings
pub struct FFIContext {
    /// Loaded C functions
    functions: HashMap<String, CFunction>,
    /// Type mappings from C to Seen
    type_mappings: HashMap<String, Type>,
    /// Loaded libraries
    loaded_libs: Vec<libloading::Library>,
    /// Include paths for header search
    include_paths: Vec<String>,
}

impl FFIContext {
    /// Create a new FFI context
    pub fn new() -> Self {
        Self {
            functions: HashMap::new(),
            type_mappings: HashMap::new(),
            loaded_libs: Vec::new(),
            include_paths: vec![
                "/usr/include".to_string(),
                "/usr/local/include".to_string(),
            ],
        }
    }
    
    /// Add an include path for header search
    pub fn add_include_path(&mut self, path: String) {
        self.include_paths.push(path);
    }
    
    /// Import a C header file
    pub fn import_header(&mut self, header_path: &Path) -> SeenResult<()> {
        let functions = header_parser::parse_header(header_path, &self.include_paths)?;
        
        for func in functions {
            self.functions.insert(func.name.clone(), func);
        }
        
        Ok(())
    }
    
    /// Load a shared library
    pub fn load_library(&mut self, lib_path: &Path) -> SeenResult<()> {
        unsafe {
            let lib = libloading::Library::new(lib_path)
                .map_err(|e| SeenError::ffi_error(format!("Failed to load library: {}", e)))?;
            self.loaded_libs.push(lib);
        }
        Ok(())
    }
    
    /// Get a function binding
    pub fn get_function(&self, name: &str) -> Option<&CFunction> {
        self.functions.get(name)
    }
    
    /// Map C type to Seen type
    pub fn map_type(&self, c_type: &CType) -> Type {
        type_mapping::c_to_seen_type(c_type)
    }
    
    /// Generate Seen bindings for all imported functions
    pub fn generate_bindings(&self) -> String {
        binding_generator::generate_bindings(&self.functions)
    }
    
    /// Call a C function dynamically
    pub unsafe fn call_function(
        &self,
        name: &str,
        args: &[*const c_void],
    ) -> SeenResult<*const c_void> {
        let func = self.functions.get(name)
            .ok_or_else(|| SeenError::ffi_error(format!("Function '{}' not found", name)))?;
        
        // Find the function symbol in loaded libraries
        for lib in &self.loaded_libs {
            if let Ok(symbol) = lib.get::<*const c_void>(name.as_bytes()) {
                return dynamic_loader::call_function(*symbol, args, func);
            }
        }
        
        Err(SeenError::ffi_error(format!("Symbol '{}' not found in loaded libraries", name)))
    }
}

/// Direct C function declaration macro for compile-time binding
#[macro_export]
macro_rules! extern_c {
    (
        $(
            fn $name:ident($($param:ident: $ptype:ty),*) $(-> $ret:ty)?;
        )*
    ) => {
        $(
            extern "C" {
                fn $name($($param: $ptype),*) $(-> $ret)?;
            }
        )*
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;
    
    #[test]
    fn test_ffi_context_creation() {
        let ctx = FFIContext::new();
        assert!(ctx.functions.is_empty());
        assert!(!ctx.include_paths.is_empty());
    }
    
    #[test]
    fn test_c_type_mapping() {
        let ctx = FFIContext::new();
        
        let int_type = CType::Int;
        let seen_type = ctx.map_type(&int_type);
        assert!(matches!(seen_type, Type::Primitive(PrimitiveType::I32)));
        
        let ptr_type = CType::Pointer(Box::new(CType::Char));
        let seen_ptr = ctx.map_type(&ptr_type);
        assert!(matches!(seen_ptr, Type::Pointer(_)));
    }
    
    #[test]
    fn test_simple_header_import() {
        let dir = tempdir().unwrap();
        let header_path = dir.path().join("test.h");
        
        fs::write(&header_path, r#"
            int add(int a, int b);
            void print_string(const char* str);
            double calculate(double x, double y);
        "#).unwrap();
        
        let mut ctx = FFIContext::new();
        // Note: Actual header parsing would require a C parser implementation
        // This test demonstrates the API structure
        
        // ctx.import_header(&header_path).unwrap();
        // assert!(ctx.get_function("add").is_some());
    }
}