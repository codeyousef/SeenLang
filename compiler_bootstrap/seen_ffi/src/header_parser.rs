//! C header parser for automatic binding generation

use crate::{CFunction, CParameter, CType, CallingConvention};
use seen_common::{SeenError, SeenResult};
use std::path::Path;
use std::process::Command;

/// Parse a C header file and extract function declarations
/// 
/// This uses clang's AST dump functionality to parse C headers accurately
pub fn parse_header(header_path: &Path, include_paths: &[String]) -> SeenResult<Vec<CFunction>> {
    // For MVP, we'll implement a simple regex-based parser
    // In production, this would use libclang or tree-sitter-c
    
    let content = std::fs::read_to_string(header_path)
        .map_err(|e| SeenError::ffi_error(format!("Failed to read header: {}", e)))?;
    
    let mut functions = Vec::new();
    
    // Simple function declaration regex
    // Matches: return_type function_name(param1_type param1_name, ...)
    let func_regex = regex::Regex::new(
        r"(?m)^([a-zA-Z_][a-zA-Z0-9_*\s]*)\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*\(([^)]*)\)\s*;"
    ).unwrap();
    
    for cap in func_regex.captures_iter(&content) {
        let return_type_str = cap.get(1).map_or("", |m| m.as_str()).trim();
        let function_name = cap.get(2).map_or("", |m| m.as_str());
        let params_str = cap.get(3).map_or("", |m| m.as_str());
        
        let return_type = parse_c_type(return_type_str);
        let parameters = parse_parameters(params_str);
        
        functions.push(CFunction {
            name: function_name.to_string(),
            return_type,
            parameters,
            is_variadic: params_str.contains("..."),
            calling_convention: CallingConvention::C,
        });
    }
    
    Ok(functions)
}

/// Parse a C type string into CType
fn parse_c_type(type_str: &str) -> CType {
    let type_str = type_str.trim();
    
    // Handle pointer types
    if type_str.ends_with('*') {
        let base = type_str.trim_end_matches('*').trim();
        return CType::Pointer(Box::new(parse_c_type(base)));
    }
    
    // Handle const qualifier
    let type_str = type_str.trim_start_matches("const").trim();
    
    // Map basic types
    match type_str {
        "void" => CType::Void,
        "char" => CType::Char,
        "signed char" => CType::SignedChar,
        "unsigned char" => CType::UnsignedChar,
        "short" | "short int" => CType::Short,
        "unsigned short" | "unsigned short int" => CType::UnsignedShort,
        "int" => CType::Int,
        "unsigned" | "unsigned int" => CType::UnsignedInt,
        "long" | "long int" => CType::Long,
        "unsigned long" | "unsigned long int" => CType::UnsignedLong,
        "long long" | "long long int" => CType::LongLong,
        "unsigned long long" | "unsigned long long int" => CType::UnsignedLongLong,
        "float" => CType::Float,
        "double" => CType::Double,
        "long double" => CType::LongDouble,
        "_Bool" | "bool" => CType::Bool,
        _ => {
            // Check for struct/union/enum
            if type_str.starts_with("struct ") {
                CType::Struct(type_str.trim_start_matches("struct ").to_string())
            } else if type_str.starts_with("union ") {
                CType::Union(type_str.trim_start_matches("union ").to_string())
            } else if type_str.starts_with("enum ") {
                CType::Enum(type_str.trim_start_matches("enum ").to_string())
            } else {
                // Default to int for unknown types
                CType::Int
            }
        }
    }
}

/// Parse function parameters
fn parse_parameters(params_str: &str) -> Vec<CParameter> {
    if params_str.trim().is_empty() || params_str.trim() == "void" {
        return Vec::new();
    }
    
    let mut parameters = Vec::new();
    
    for param in params_str.split(',') {
        let param = param.trim();
        if param == "..." {
            continue; // Skip variadic marker
        }
        
        // Split type and name (simplified - doesn't handle all cases)
        let parts: Vec<&str> = param.rsplitn(2, ' ').collect();
        
        let (c_type, name) = if parts.len() == 2 {
            let name = Some(parts[0].to_string());
            let type_str = parts[1];
            (parse_c_type(type_str), name)
        } else {
            // No parameter name
            (parse_c_type(param), None)
        };
        
        parameters.push(CParameter { name, c_type });
    }
    
    parameters
}

/// Use clang to parse header (more accurate but requires clang)
pub fn parse_header_with_clang(
    header_path: &Path,
    include_paths: &[String],
) -> SeenResult<String> {
    let mut cmd = Command::new("clang");
    cmd.arg("-Xclang")
        .arg("-ast-dump")
        .arg("-fsyntax-only");
    
    for path in include_paths {
        cmd.arg(format!("-I{}", path));
    }
    
    cmd.arg(header_path);
    
    let output = cmd.output()
        .map_err(|e| SeenError::ffi_error(format!("Failed to run clang: {}", e)))?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(SeenError::ffi_error(format!("Clang failed: {}", stderr)));
    }
    
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_c_type() {
        assert!(matches!(parse_c_type("int"), CType::Int));
        assert!(matches!(parse_c_type("void"), CType::Void));
        assert!(matches!(parse_c_type("double"), CType::Double));
        assert!(matches!(parse_c_type("char*"), CType::Pointer(_)));
        assert!(matches!(parse_c_type("const char*"), CType::Pointer(_)));
        assert!(matches!(parse_c_type("struct Point"), CType::Struct(_)));
    }
    
    #[test]
    fn test_parse_parameters() {
        let params = parse_parameters("int x, double y");
        assert_eq!(params.len(), 2);
        assert!(matches!(params[0].c_type, CType::Int));
        assert_eq!(params[0].name, Some("x".to_string()));
        
        let empty_params = parse_parameters("void");
        assert!(empty_params.is_empty());
        
        let variadic = parse_parameters("const char* fmt, ...");
        assert_eq!(variadic.len(), 1);
    }
}