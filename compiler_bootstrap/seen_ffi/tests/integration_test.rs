use seen_ffi::{FFIContext, CType, CFunction, CParameter, CallingConvention};
use std::path::Path;
use tempfile::tempdir;
use std::fs;

#[test]
fn test_ffi_context_basic() {
    let mut ctx = FFIContext::new();
    
    // Should have default include paths
    assert!(ctx.get_function("nonexistent").is_none());
}

#[test]
fn test_type_mapping() {
    use seen_typechecker::types::{Type, PrimitiveType};
    
    let ctx = FFIContext::new();
    
    // Test int mapping
    let int_type = ctx.map_type(&CType::Int);
    assert!(matches!(int_type, Type::Primitive(PrimitiveType::I32)));
    
    // Test pointer mapping
    let ptr_type = ctx.map_type(&CType::Pointer(Box::new(CType::Char)));
    // C pointers now map to primitive types in the automatic inference system
    assert!(matches!(ptr_type, Type::Primitive(_)));
    
    // Test void mapping
    let void_type = ctx.map_type(&CType::Void);
    assert!(matches!(void_type, Type::Primitive(PrimitiveType::Unit)));
}

#[test]
fn test_binding_generation() {
    let ctx = FFIContext::new();
    let bindings = ctx.generate_bindings();
    
    // Should generate header comment
    assert!(bindings.contains("Auto-generated C bindings"));
}

#[test]
fn test_simple_c_header_parsing() {
    let dir = tempdir().unwrap();
    let header_path = dir.path().join("math.h");
    
    // Create a simple C header
    fs::write(&header_path, r#"
// Simple math functions
int add(int a, int b);
double multiply(double x, double y);
void print_result(int value);
int* allocate_array(int size);
"#).unwrap();
    
    let mut ctx = FFIContext::new();
    
    // Import should succeed (even if parsing is simplified)
    let result = ctx.import_header(&header_path);
    assert!(result.is_ok());
}

#[test]
fn test_c_type_sizes() {
    use seen_ffi::type_mapping::c_type_size;
    
    assert_eq!(c_type_size(&CType::Char), 1);
    assert_eq!(c_type_size(&CType::Short), 2);
    assert_eq!(c_type_size(&CType::Int), 4);
    assert_eq!(c_type_size(&CType::Double), 8);
    assert_eq!(c_type_size(&CType::LongLong), 8);
    
    // Array size
    let arr_type = CType::Array(Box::new(CType::Int), Some(10));
    assert_eq!(c_type_size(&arr_type), 40); // 4 * 10
}

#[test]
fn test_c_type_alignment() {
    use seen_ffi::type_mapping::c_type_alignment;
    
    assert_eq!(c_type_alignment(&CType::Char), 1);
    assert_eq!(c_type_alignment(&CType::Short), 2);
    assert_eq!(c_type_alignment(&CType::Int), 4);
    assert_eq!(c_type_alignment(&CType::Double), 8);
}

#[test]
fn test_bidirectional_type_conversion() {
    use seen_ffi::type_mapping::{c_to_seen_type, seen_to_c_type};
    use seen_typechecker::types::{Type, PrimitiveType};
    
    // Test round-trip for basic types
    let types = vec![
        CType::Int,
        CType::Float,
        CType::Double,
        CType::Bool,
        CType::Char,
    ];
    
    for c_type in types {
        let seen_type = c_to_seen_type(&c_type);
        let back_to_c = seen_to_c_type(&seen_type);
        assert!(back_to_c.is_some());
    }
}

#[test]
fn test_function_binding_generation() {
    use seen_ffi::binding_generator::generate_bindings;
    use std::collections::HashMap;
    
    let mut functions = HashMap::new();
    
    // Add a simple function
    functions.insert("sqrt".to_string(), CFunction {
        name: "sqrt".to_string(),
        return_type: CType::Double,
        parameters: vec![
            CParameter {
                name: Some("x".to_string()),
                c_type: CType::Double,
            }
        ],
        is_variadic: false,
        calling_convention: CallingConvention::C,
    });
    
    let bindings = generate_bindings(&functions);
    
    // Check generated binding contains expected elements
    assert!(bindings.contains("fun sqrt"));
    assert!(bindings.contains("x: f64"));
    assert!(bindings.contains("-> f64"));
    assert!(bindings.contains("@extern"));
}