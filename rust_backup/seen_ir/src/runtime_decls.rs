//! Runtime function declarations for LLVM IR generation.
//!
//! This module defines all runtime function signatures that need to be declared
//! in LLVM IR for Seen programs. These declarations ensure ABI compatibility
//! between the Rust bootstrap compiler and the self-hosted Seen compiler.

/// A runtime function declaration.
pub struct RuntimeDecl {
    pub name: &'static str,
    pub signature: &'static str,
    pub category: &'static str,
}

/// All runtime function declarations organized by category.
/// These are the core runtime functions that are always needed.
pub const RUNTIME_DECLS: &[RuntimeDecl] = &[
    // ========================================================================
    // Core Runtime Functions
    // ========================================================================
    RuntimeDecl {
        name: "seen_runtime_init",
        signature: "declare void @seen_runtime_init(i32, ptr)",
        category: "Core",
    },
    RuntimeDecl {
        name: "println",
        signature: "declare void @println(%SeenString)",
        category: "Core",
    },
    RuntimeDecl {
        name: "seen_cstr_to_str",
        signature: "declare %SeenString @seen_cstr_to_str(ptr)",
        category: "Core",
    },

    // ========================================================================
    // String Functions
    // ========================================================================
    RuntimeDecl {
        name: "seen_length",
        signature: "declare i64 @seen_length(%SeenString)",
        category: "String",
    },
    RuntimeDecl {
        name: "seen_substring",
        signature: "declare %SeenString @seen_substring(%SeenString, i64, i64)",
        category: "String",
    },
    RuntimeDecl {
        name: "startsWith",
        signature: "declare i1 @startsWith(%SeenString, %SeenString)",
        category: "String",
    },
    RuntimeDecl {
        name: "endsWith",
        signature: "declare i1 @endsWith(%SeenString, %SeenString)",
        category: "String",
    },
    RuntimeDecl {
        name: "contains",
        signature: "declare i1 @contains(%SeenString, %SeenString)",
        category: "String",
    },
    RuntimeDecl {
        name: "seen_str_concat_ss",
        signature: "declare %SeenString @seen_str_concat_ss(%SeenString, %SeenString)",
        category: "String",
    },
    RuntimeDecl {
        name: "seen_int_to_string",
        signature: "declare %SeenString @seen_int_to_string(i64)",
        category: "String",
    },
    RuntimeDecl {
        name: "seen_char_to_str",
        signature: "declare %SeenString @seen_char_to_str(i64)",
        category: "String",
    },
    RuntimeDecl {
        name: "seen_bool_to_string",
        signature: "declare %SeenString @seen_bool_to_string(i1)",
        category: "String",
    },
    RuntimeDecl {
        name: "seen_char_at",
        signature: "declare i64 @seen_char_at(%SeenString, i64)",
        category: "String",
    },
    RuntimeDecl {
        name: "seen_str_eq_ss",
        signature: "declare i1 @seen_str_eq_ss(%SeenString, %SeenString)",
        category: "String",
    },
    RuntimeDecl {
        name: "seen_str_ne_ss",
        signature: "declare i1 @seen_str_ne_ss(%SeenString, %SeenString)",
        category: "String",
    },
    // NOTE: indexOf, lastIndexOf are NOT included here - they may be defined in user code
    // and are conditionally declared by the self-hosted compiler

    // ========================================================================
    // StringBuilder Functions
    // ========================================================================
    RuntimeDecl {
        name: "StringBuilder_new",
        signature: "declare ptr @StringBuilder_new()",
        category: "StringBuilder",
    },
    RuntimeDecl {
        name: "StringBuilder_append",
        signature: "declare i64 @StringBuilder_append(ptr, %SeenString)",
        category: "StringBuilder",
    },
    RuntimeDecl {
        name: "StringBuilder_appendChar",
        signature: "declare i64 @StringBuilder_appendChar(ptr, i64)",
        category: "StringBuilder",
    },
    RuntimeDecl {
        name: "StringBuilder_toString",
        signature: "declare %SeenString @StringBuilder_toString(ptr)",
        category: "StringBuilder",
    },

    // ========================================================================
    // Array Functions
    // ========================================================================
    RuntimeDecl {
        name: "seen_arr_new_str_ptr",
        signature: "declare ptr @seen_arr_new_str_ptr()",
        category: "Array",
    },
    RuntimeDecl {
        name: "seen_arr_new_ptr_ptr",
        signature: "declare ptr @seen_arr_new_ptr_ptr()",
        category: "Array",
    },
    RuntimeDecl {
        name: "seen_arr_new_with_size_ptr",
        signature: "declare ptr @seen_arr_new_with_size_ptr(i64)",
        category: "Array",
    },
    RuntimeDecl {
        name: "seen_arr_length",
        signature: "declare i64 @seen_arr_length(ptr byval(%SeenArray) align 8)",
        category: "Array",
    },
    RuntimeDecl {
        name: "seen_arr_length_ptr",
        signature: "declare i64 @seen_arr_length_ptr(ptr)",
        category: "Array",
    },
    RuntimeDecl {
        name: "seen_arr_get_str",
        signature: "declare %SeenString @seen_arr_get_str(ptr byval(%SeenArray) align 8, i64)",
        category: "Array",
    },
    RuntimeDecl {
        name: "seen_arr_get_diag",
        signature: "declare ptr @seen_arr_get_diag(ptr byval(%SeenArray) align 8, i64)",
        category: "Array",
    },
    RuntimeDecl {
        name: "seen_arr_get_ptr",
        signature: "declare ptr @seen_arr_get_ptr(ptr byval(%SeenArray) align 8, i64)",
        category: "Array",
    },
    RuntimeDecl {
        name: "seen_arr_get_element",
        signature: "declare ptr @seen_arr_get_element(ptr byval(%SeenArray) align 8, i64)",
        category: "Array",
    },
    RuntimeDecl {
        name: "Array_push",
        signature: "declare i64 @Array_push(ptr, ptr)",
        category: "Array",
    },
    RuntimeDecl {
        name: "seen_arr_push_str",
        signature: "declare void @seen_arr_push_str(ptr, %SeenString)",
        category: "Array",
    },
    RuntimeDecl {
        name: "seen_arr_push_i64",
        signature: "declare void @seen_arr_push_i64(ptr, i64)",
        category: "Array",
    },
    // NOTE: args() is NOT included here - it may be defined in user code
    // and is conditionally declared by the self-hosted compiler

    // ========================================================================
    // File I/O Primitives
    // ========================================================================
    RuntimeDecl {
        name: "__OpenFile",
        signature: "declare i64 @__OpenFile(%SeenString, %SeenString)",
        category: "FileIO",
    },
    RuntimeDecl {
        name: "__ReadFile",
        signature: "declare %SeenString @__ReadFile(i64)",
        category: "FileIO",
    },
    RuntimeDecl {
        name: "__ReadFileBytes",
        signature: "declare %SeenArray @__ReadFileBytes(i64, i64)",
        category: "FileIO",
    },
    RuntimeDecl {
        name: "__WriteFile",
        signature: "declare i64 @__WriteFile(i64, %SeenString)",
        category: "FileIO",
    },
    RuntimeDecl {
        name: "__WriteFileBytes",
        signature: "declare i64 @__WriteFileBytes(i64, %SeenArray)",
        category: "FileIO",
    },
    RuntimeDecl {
        name: "__CloseFile",
        signature: "declare i64 @__CloseFile(i64)",
        category: "FileIO",
    },
    RuntimeDecl {
        name: "__FileSize",
        signature: "declare i64 @__FileSize(i64)",
        category: "FileIO",
    },
    RuntimeDecl {
        name: "__FileError",
        signature: "declare %SeenString @__FileError(i64)",
        category: "FileIO",
    },
    RuntimeDecl {
        name: "__FileExists",
        signature: "declare i1 @__FileExists(%SeenString)",
        category: "FileIO",
    },
    RuntimeDecl {
        name: "__DeleteFile",
        signature: "declare i1 @__DeleteFile(%SeenString)",
        category: "FileIO",
    },
    RuntimeDecl {
        name: "__CreateDirectory",
        signature: "declare i1 @__CreateDirectory(%SeenString)",
        category: "FileIO",
    },
    // NOTE: readText, writeText, exists are NOT included here - they may be defined in user code
    // and are conditionally declared by the self-hosted compiler

    // ========================================================================
    // Process/Command Execution
    // ========================================================================
    RuntimeDecl {
        name: "__ExecuteProgram",
        signature: "declare i64 @__ExecuteProgram(%SeenString)",
        category: "Process",
    },
    RuntimeDecl {
        name: "__ExecuteCommand",
        signature: "declare ptr @__ExecuteCommand(%SeenString)",
        category: "Process",
    },
    // NOTE: runCommand, commandWasSuccessful are NOT included here - they may be defined in user code
    // and are conditionally declared by the self-hosted compiler

    // ========================================================================
    // Environment Variables
    // ========================================================================
    RuntimeDecl {
        name: "__GetCommandLineArgs",
        signature: "declare %SeenArray @__GetCommandLineArgs()",
        category: "Environment",
    },
    RuntimeDecl {
        name: "__HasEnv",
        signature: "declare i1 @__HasEnv(%SeenString)",
        category: "Environment",
    },
    RuntimeDecl {
        name: "__GetEnv",
        signature: "declare %SeenString @__GetEnv(%SeenString)",
        category: "Environment",
    },
    RuntimeDecl {
        name: "__SetEnv",
        signature: "declare i1 @__SetEnv(%SeenString, %SeenString)",
        category: "Environment",
    },
    RuntimeDecl {
        name: "__RemoveEnv",
        signature: "declare i1 @__RemoveEnv(%SeenString)",
        category: "Environment",
    },
    RuntimeDecl {
        name: "__Print",
        signature: "declare void @__Print(%SeenString)",
        category: "Environment",
    },
    RuntimeDecl {
        name: "__PrintInt",
        signature: "declare void @__PrintInt(i64)",
        category: "Environment",
    },

    // ========================================================================
    // Map Functions (generic key-value store)
    // ========================================================================
    RuntimeDecl {
        name: "Map_new",
        signature: "declare ptr @Map_new()",
        category: "Map",
    },
    RuntimeDecl {
        name: "Map_set",
        signature: "declare i64 @Map_set(ptr, %SeenString, i64)",
        category: "Map",
    },
    RuntimeDecl {
        name: "Map_put",
        signature: "declare i64 @Map_put(ptr, %SeenString, i64)",
        category: "Map",
    },
    RuntimeDecl {
        name: "Map_get",
        signature: "declare i64 @Map_get(ptr, %SeenString)",
        category: "Map",
    },
    RuntimeDecl {
        name: "Map_size",
        signature: "declare i64 @Map_size(ptr)",
        category: "Map",
    },
    RuntimeDecl {
        name: "Map_containsKey",
        signature: "declare i1 @Map_containsKey(ptr, %SeenString)",
        category: "Map",
    },
    RuntimeDecl {
        name: "Map_containsValue",
        signature: "declare i1 @Map_containsValue(ptr, i64)",
        category: "Map",
    },
    RuntimeDecl {
        name: "Map_keys",
        signature: "declare %SeenArray @Map_keys(ptr)",
        category: "Map",
    },
    RuntimeDecl {
        name: "Map_values",
        signature: "declare %SeenArray @Map_values(ptr)",
        category: "Map",
    },

    // ========================================================================
    // Memory Allocation
    // ========================================================================
    RuntimeDecl {
        name: "malloc",
        signature: "declare ptr @malloc(i64)",
        category: "Memory",
    },
    RuntimeDecl {
        name: "free",
        signature: "declare void @free(ptr)",
        category: "Memory",
    },
    RuntimeDecl {
        name: "abort",
        signature: "declare i64 @abort(i64)",
        category: "Memory",
    },

    // ========================================================================
    // Conversion/Utility Functions
    // ========================================================================
    RuntimeDecl {
        name: "Char_toInt",
        signature: "declare i64 @Char_toInt(i64)",
        category: "Conversion",
    },
    RuntimeDecl {
        name: "Int_unwrap",
        signature: "declare i64 @Int_unwrap(i64)",
        category: "Conversion",
    },
    RuntimeDecl {
        name: "Optional_unwrap",
        signature: "declare ptr @Optional_unwrap(ptr)",
        category: "Conversion",
    },
    RuntimeDecl {
        name: "SeenTokenType_unwrap",
        signature: "declare i64 @SeenTokenType_unwrap(i64)",
        category: "Conversion",
    },
    RuntimeDecl {
        name: "Some",
        signature: "declare i64 @Some(ptr)",
        category: "Conversion",
    },
    RuntimeDecl {
        name: "None",
        signature: "declare i64 @None()",
        category: "Conversion",
    },

    // ========================================================================
    // Result Type Functions
    // ========================================================================
    RuntimeDecl {
        name: "Result_isOkay",
        signature: "declare i1 @Result_isOkay(ptr)",
        category: "Result",
    },
    RuntimeDecl {
        name: "Result_isErr",
        signature: "declare i1 @Result_isErr(ptr)",
        category: "Result",
    },
    RuntimeDecl {
        name: "Result_unwrap",
        signature: "declare ptr @Result_unwrap(ptr)",
        category: "Result",
    },
    RuntimeDecl {
        name: "Result_unwrapErr",
        signature: "declare %SeenString @Result_unwrapErr(ptr)",
        category: "Result",
    },
    RuntimeDecl {
        name: "Err",
        signature: "declare i64 @Err(%SeenString)",
        category: "Result",
    },
    RuntimeDecl {
        name: "Ok",
        signature: "declare i64 @Ok(i64)",
        category: "Result",
    },

    // ========================================================================
    // TypeError/Location Accessors (for error reporting)
    // ========================================================================
    RuntimeDecl {
        name: "TypeError_getMessage",
        signature: "declare %SeenString @TypeError_getMessage(ptr)",
        category: "TypeSystem",
    },
    RuntimeDecl {
        name: "TypeError_getContext",
        signature: "declare %SeenString @TypeError_getContext(ptr)",
        category: "TypeSystem",
    },
    RuntimeDecl {
        name: "TypeError_getLocation",
        signature: "declare ptr @TypeError_getLocation(ptr)",
        category: "TypeSystem",
    },
    RuntimeDecl {
        name: "TypeError_getExpected",
        signature: "declare ptr @TypeError_getExpected(ptr)",
        category: "TypeSystem",
    },
    RuntimeDecl {
        name: "TypeError_getActual",
        signature: "declare ptr @TypeError_getActual(ptr)",
        category: "TypeSystem",
    },
    RuntimeDecl {
        name: "Location_line",
        signature: "declare i64 @Location_line(ptr)",
        category: "TypeSystem",
    },
    RuntimeDecl {
        name: "Location_column",
        signature: "declare i64 @Location_column(ptr)",
        category: "TypeSystem",
    },
    RuntimeDecl {
        name: "Type_name",
        signature: "declare %SeenString @Type_name(ptr)",
        category: "TypeSystem",
    },
];

/// Section marker for runtime function declarations.
/// This marker is used by the self-hosted compiler to detect when runtime
/// declarations are included in the type header file.
pub const RUNTIME_SECTION_MARKER: &str = "; Runtime Function Declarations";

/// Emit all runtime function declarations as LLVM IR text.
/// Returns a string containing all declarations, grouped by category.
pub fn emit_all_declarations() -> String {
    let mut output = String::with_capacity(8192);

    // Keep track of current category for section headers
    let mut current_category = "";

    for decl in RUNTIME_DECLS {
        // Emit category header if we're starting a new category
        if decl.category != current_category {
            if !current_category.is_empty() {
                output.push('\n');
            }
            output.push_str("; ");
            output.push_str(decl.category);
            output.push_str(" Functions\n");
            current_category = decl.category;
        }

        output.push_str(decl.signature);
        output.push('\n');
    }

    output
}

/// Get the number of runtime declarations.
pub fn declaration_count() -> usize {
    RUNTIME_DECLS.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_emit_all_declarations() {
        let output = emit_all_declarations();
        // Should contain all declarations
        assert!(output.contains("@println"));
        assert!(output.contains("@seen_str_concat_ss"));
        assert!(output.contains("@Map_new"));
        assert!(output.contains("@malloc"));
        // Should have category headers
        assert!(output.contains("; Core Functions"));
        assert!(output.contains("; String Functions"));
    }

    #[test]
    fn test_declaration_count() {
        // We expect approximately 90+ declarations
        assert!(declaration_count() >= 80);
    }

    #[test]
    fn test_section_marker() {
        assert_eq!(RUNTIME_SECTION_MARKER, "; Runtime Function Declarations");
    }
}
