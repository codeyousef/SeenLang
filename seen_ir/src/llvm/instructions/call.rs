//! Call instruction handler for the LLVM backend.
//!
//! This module handles the `Instruction::Call` variant, which includes:
//! - Direct function calls
//! - Intrinsic/builtin function dispatch
//! - Method calls on types (String, Array, Result, etc.)
//!
//! The Call instruction is the largest handler (~2000 lines) because it
//! includes dispatch logic for many built-in operations.

use anyhow::{anyhow, Result};
use indexmap::IndexMap;
use inkwell::values::{BasicValueEnum, FunctionValue};

use crate::instruction::Instruction;
use crate::value::IRValue;

type HashMap<K, V> = IndexMap<K, V>;

/// Intrinsic function names that have special handling
pub const INTRINSIC_FUNCTIONS: &[&str] = &[
    // Conversion
    "toFloat",
    "toInt",
    "toString",
    // Array operations
    "__ArrayNew",
    "push",
    "pop",
    "get",
    "set",
    "len",
    "size",
    "capacity",
    "clear",
    "remove",
    "insert",
    "contains",
    "indexOf",
    "lastIndexOf",
    "isEmpty",
    "reverse",
    "slice",
    "join",
    "sort",
    // String operations  
    "charAt",
    "substring",
    "startsWith",
    "endsWith",
    "trim",
    "trimStart",
    "trimEnd",
    "split",
    "replace",
    "replaceAll",
    "toUpperCase",
    "toLowerCase",
    "repeat",
    "padStart",
    "padEnd",
    // Result/Option operations
    "isOk",
    "isOkay",
    "isErr",
    "unwrap",
    "unwrapErr",
    "unwrapOr",
    "isSome",
    "isNone",
    // I/O operations
    "print",
    "println",
    "readLine",
    "readFile",
    "writeFile",
    "fileExists",
    "createDirectory",
    // System operations
    "abort",
    "exit",
    "getenv",
    "setenv",
    "executeCommand",
    "getTimestamp",
    // Math operations
    "abs",
    "sqrt",
    "floor",
    "ceil",
    "round",
    "sin",
    "cos",
    "tan",
    "pow",
    "log",
    "exp",
    "min",
    "max",
    // Type operations
    "__default",
    "clone",
    "drop",
];

/// Normalize a method name by stripping type prefixes.
/// 
/// Examples:
/// - `String_charAt` -> `charAt`
/// - `List_size` -> `size`
/// - `Result_isOk` -> `isOk`
pub fn normalize_method_name(name: &str) -> &str {
    // Strip common type prefixes
    let stripped = if let Some(s) = name.strip_prefix("String_") {
        s
    } else if let Some(s) = name.strip_prefix("List_") {
        s
    } else if let Some(s) = name.strip_prefix("Array_") {
        s
    } else if let Some(s) = name.strip_prefix("Vec_") {
        s
    } else if let Some(s) = name.strip_prefix("Result_") {
        s
    } else if let Some(s) = name.strip_prefix("Option_") {
        s
    } else if let Some(s) = name.strip_prefix("File_") {
        s
    } else if let Some(s) = name.strip_prefix("Map_") {
        s
    } else if let Some(s) = name.strip_prefix("HashMap_") {
        s
    } else if let Some(s) = name.strip_prefix("Set_") {
        s
    } else if let Some(s) = name.strip_prefix("HashSet_") {
        s
    } else {
        name
    };
    
    // Strip numeric suffix (e.g., `List_size.546` -> `size`)
    stripped.split('.').next().unwrap_or(stripped)
}

/// Check if a function name is a Result/Option method that needs forwarding.
/// Returns the canonical method name if it is.
pub fn get_result_method_alias(name: &str) -> Option<&'static str> {
    match name {
        "isOk" | "Result_isOk" | "File_isOk" | "Option_isSome" => Some("isOkay"),
        "isErr" | "Result_isErr" | "File_isErr" | "Option_isNone" => Some("isErr"),
        "unwrap" | "Result_unwrap" | "File_unwrap" | "Option_unwrap" => Some("unwrap"),
        "unwrapErr" | "Result_unwrapErr" | "File_unwrapErr" => Some("unwrapErr"),
        "unwrapOr" | "Result_unwrapOr" | "Option_unwrapOr" => Some("unwrapOr"),
        _ => None,
    }
}

/// Check if a function name is an intrinsic that needs special handling.
pub fn is_intrinsic(name: &str) -> bool {
    let normalized = normalize_method_name(name);
    INTRINSIC_FUNCTIONS.contains(&normalized)
}

/// Categorize a call target for dispatch purposes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallCategory {
    /// Direct function call (no special handling)
    Direct,
    /// Result/Option method (needs forwarding)
    ResultMethod(&'static str),
    /// Conversion intrinsic (toFloat, toInt, toString)
    Conversion,
    /// Array/collection operation
    Collection,
    /// String operation
    StringOp,
    /// I/O operation
    IO,
    /// System operation
    System,
    /// Math operation
    Math,
    /// Type operation (__default, clone, drop)
    TypeOp,
}

/// Categorize a function name for dispatch.
pub fn categorize_call(name: &str) -> CallCategory {
    let normalized = normalize_method_name(name);
    
    // Check for Result/Option method aliases first
    if let Some(method) = get_result_method_alias(normalized) {
        return CallCategory::ResultMethod(method);
    }
    
    match normalized {
        // Conversion
        "toFloat" | "toInt" | "toString" => CallCategory::Conversion,
        
        // Collection operations
        "__ArrayNew" | "push" | "pop" | "get" | "set" | "len" | "size" |
        "capacity" | "clear" | "remove" | "insert" | "contains" | 
        "indexOf" | "lastIndexOf" | "isEmpty" | "reverse" | "slice" |
        "join" | "sort" | "keys" | "values" | "entries" => CallCategory::Collection,
        
        // String operations
        "charAt" | "substring" | "startsWith" | "endsWith" | "trim" |
        "trimStart" | "trimEnd" | "split" | "replace" | "replaceAll" |
        "toUpperCase" | "toLowerCase" | "repeat" | "padStart" | "padEnd" => CallCategory::StringOp,
        
        // I/O operations
        "print" | "println" | "readLine" | "readFile" | "writeFile" |
        "fileExists" | "createDirectory" => CallCategory::IO,
        
        // System operations
        "abort" | "exit" | "getenv" | "setenv" | "executeCommand" | 
        "getTimestamp" => CallCategory::System,
        
        // Math operations
        "abs" | "sqrt" | "floor" | "ceil" | "round" | "sin" | "cos" |
        "tan" | "pow" | "log" | "exp" | "min" | "max" => CallCategory::Math,
        
        // Type operations
        "__default" | "clone" | "drop" => CallCategory::TypeOp,
        
        // Default to direct call
        _ => CallCategory::Direct,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_method_name() {
        assert_eq!(normalize_method_name("String_charAt"), "charAt");
        assert_eq!(normalize_method_name("List_size"), "size");
        assert_eq!(normalize_method_name("List_size.546"), "size");
        assert_eq!(normalize_method_name("push"), "push");
    }

    #[test]
    fn test_get_result_method_alias() {
        assert_eq!(get_result_method_alias("isOk"), Some("isOkay"));
        assert_eq!(get_result_method_alias("File_isOk"), Some("isOkay"));
        assert_eq!(get_result_method_alias("unwrap"), Some("unwrap"));
        assert_eq!(get_result_method_alias("push"), None);
    }

    #[test]
    fn test_categorize_call() {
        assert_eq!(categorize_call("toFloat"), CallCategory::Conversion);
        assert_eq!(categorize_call("String_charAt"), CallCategory::StringOp);
        assert_eq!(categorize_call("push"), CallCategory::Collection);
        assert_eq!(categorize_call("File_isOk"), CallCategory::ResultMethod("isOkay"));
        assert_eq!(categorize_call("myFunction"), CallCategory::Direct);
    }
}
