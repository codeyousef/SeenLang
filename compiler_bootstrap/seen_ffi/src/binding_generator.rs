//! Generate Seen bindings from C function declarations

use crate::{CFunction, CType};
use std::collections::HashMap;

/// Generate Seen code bindings for C functions
pub fn generate_bindings(functions: &HashMap<String, CFunction>) -> String {
    let mut output = String::new();
    
    output.push_str("// Auto-generated C bindings\n");
    output.push_str("// Do not edit manually\n\n");
    
    for (name, func) in functions {
        output.push_str(&generate_function_binding(func));
        output.push('\n');
    }
    
    output
}

/// Generate a single function binding
fn generate_function_binding(func: &CFunction) -> String {
    let mut binding = String::new();
    
    // Add documentation comment
    binding.push_str(&format!("// C function: {}\n", func.name));
    
    // Generate extern declaration
    binding.push_str("@extern(\"C\")\n");
    binding.push_str(&format!("fun {}(", func.name));
    
    // Parameters
    let params: Vec<String> = func.parameters.iter().enumerate().map(|(i, param)| {
        let name = param.name.as_ref()
            .map(|s| s.as_str())
            .unwrap_or_else(|| {
                // This will be handled below where we use the name
                ""
            });
        let actual_name = if name.is_empty() {
            format!("arg{}", i)
        } else {
            name.to_string()
        };
        let type_str = c_type_to_seen_string(&param.c_type);
        format!("{}: {}", actual_name, type_str)
    }).collect();
    
    binding.push_str(&params.join(", "));
    
    if func.is_variadic {
        if !params.is_empty() {
            binding.push_str(", ");
        }
        binding.push_str("...");
    }
    
    binding.push(')');
    
    // Return type
    if !matches!(func.return_type, CType::Void) {
        binding.push_str(" -> ");
        binding.push_str(&c_type_to_seen_string(&func.return_type));
    }
    
    binding.push_str("\n");
    
    // Generate safe wrapper function
    binding.push_str(&generate_safe_wrapper(func));
    
    binding
}

/// Generate a safe wrapper function
fn generate_safe_wrapper(func: &CFunction) -> String {
    let mut wrapper = String::new();
    
    wrapper.push_str(&format!("\n// Safe wrapper for {}\n", func.name));
    wrapper.push_str(&format!("fun safe_{}(", func.name));
    
    // Parameters with null checks for pointers
    let params: Vec<String> = func.parameters.iter().enumerate().map(|(i, param)| {
        let name = param.name.as_ref()
            .map(|s| s.as_str())
            .unwrap_or_else(|| "");
        let actual_name = if name.is_empty() {
            format!("arg{}", i)
        } else {
            name.to_string()
        };
        let type_str = c_type_to_seen_string_safe(&param.c_type);
        format!("{}: {}", actual_name, type_str)
    }).collect();
    
    wrapper.push_str(&params.join(", "));
    wrapper.push(')');
    
    // Return type with Result wrapper for fallible operations
    let needs_result = func.parameters.iter().any(|p| matches!(p.c_type, CType::Pointer(_)));
    
    if needs_result {
        wrapper.push_str(" -> Result<");
        if !matches!(func.return_type, CType::Void) {
            wrapper.push_str(&c_type_to_seen_string_safe(&func.return_type));
        } else {
            wrapper.push_str("()");
        }
        wrapper.push_str(", FFIError>");
    } else if !matches!(func.return_type, CType::Void) {
        wrapper.push_str(" -> ");
        wrapper.push_str(&c_type_to_seen_string_safe(&func.return_type));
    }
    
    wrapper.push_str(" {\n");
    
    // Add null checks for pointer parameters
    for (i, param) in func.parameters.iter().enumerate() {
        if matches!(param.c_type, CType::Pointer(_)) {
            let name = param.name.as_ref()
                .map(|s| s.as_str())
                .unwrap_or_else(|| "");
            let actual_name = if name.is_empty() {
                format!("arg{}", i)
            } else {
                name.to_string()
            };
            wrapper.push_str(&format!("    if {} == null {{\n", actual_name));
            wrapper.push_str(&format!("        return Err(FFIError::NullPointer(\"{}\"))\n", actual_name));
            wrapper.push_str("    }\n");
        }
    }
    
    // Call the unsafe function
    wrapper.push_str("    ");
    if needs_result {
        wrapper.push_str("Ok(");
    }
    
    if !matches!(func.return_type, CType::Void) {
        wrapper.push_str("return ");
    }
    
    wrapper.push_str(&format!("{}(", func.name));
    
    let args: Vec<String> = func.parameters.iter().enumerate().map(|(i, param)| {
        if let Some(name) = &param.name {
            name.clone()
        } else {
            format!("arg{}", i)
        }
    }).collect();
    
    wrapper.push_str(&args.join(", "));
    wrapper.push(')');
    
    if needs_result {
        wrapper.push(')');
    }
    
    wrapper.push_str("\n}\n");
    
    wrapper
}

/// Convert C type to Seen type string
fn c_type_to_seen_string(c_type: &CType) -> String {
    match c_type {
        CType::Void => "()".to_string(),
        CType::Char | CType::SignedChar => "i8".to_string(),
        CType::UnsignedChar => "u8".to_string(),
        CType::Short => "i16".to_string(),
        CType::UnsignedShort => "u16".to_string(),
        CType::Int => "i32".to_string(),
        CType::UnsignedInt => "u32".to_string(),
        CType::Long => "i64".to_string(),
        CType::UnsignedLong => "u64".to_string(),
        CType::LongLong => "i64".to_string(),
        CType::UnsignedLongLong => "u64".to_string(),
        CType::Float => "f32".to_string(),
        CType::Double => "f64".to_string(),
        CType::LongDouble => "f64".to_string(),
        CType::Bool => "bool".to_string(),
        CType::Pointer(inner) => {
            if matches!(**inner, CType::Char) {
                "*const u8".to_string() // C string
            } else {
                format!("*{}", c_type_to_seen_string(inner))
            }
        }
        CType::Array(inner, Some(size)) => {
            format!("[{}; {}]", c_type_to_seen_string(inner), size)
        }
        CType::Array(inner, None) => {
            format!("*{}", c_type_to_seen_string(inner))
        }
        CType::Struct(name) => format!("C_{}", name),
        CType::Union(name) => format!("C_Union_{}", name),
        CType::Enum(name) => format!("C_{}", name),
        CType::Function(_) => "*const ()".to_string(), // Function pointer
    }
}

/// Convert C type to safe Seen type string
fn c_type_to_seen_string_safe(c_type: &CType) -> String {
    match c_type {
        CType::Pointer(inner) if matches!(**inner, CType::Char) => {
            "String".to_string() // Safe string wrapper
        }
        CType::Pointer(inner) => {
            format!("Option<*{}>", c_type_to_seen_string(inner))
        }
        _ => c_type_to_seen_string(c_type),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CParameter, CallingConvention};
    
    #[test]
    fn test_generate_function_binding() {
        let func = CFunction {
            name: "printf".to_string(),
            return_type: CType::Int,
            parameters: vec![
                CParameter {
                    name: Some("format".to_string()),
                    c_type: CType::Pointer(Box::new(CType::Char)),
                },
            ],
            is_variadic: true,
            calling_convention: CallingConvention::C,
        };
        
        let binding = generate_function_binding(&func);
        assert!(binding.contains("@extern(\"C\")"));
        assert!(binding.contains("fun printf"));
        assert!(binding.contains("..."));
        assert!(binding.contains("-> i32"));
    }
    
    #[test]
    fn test_type_conversion() {
        assert_eq!(c_type_to_seen_string(&CType::Int), "i32");
        assert_eq!(c_type_to_seen_string(&CType::Double), "f64");
        assert_eq!(c_type_to_seen_string(&CType::Void), "()");
        
        let char_ptr = CType::Pointer(Box::new(CType::Char));
        assert_eq!(c_type_to_seen_string(&char_ptr), "*const u8");
        assert_eq!(c_type_to_seen_string_safe(&char_ptr), "String");
    }
}