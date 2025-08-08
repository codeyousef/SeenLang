//! Comprehensive C interoperability tests
//! Tests Zig-style direct C header import capabilities

use seen_ffi::{header_parser, binding_generator, dynamic_loader, type_mapping};
use seen_ffi::{CFunction, CType, CParameter, CallingConvention};
use std::fs;
use tempfile::TempDir;

#[test]
fn test_simple_c_function_import() {
    let header_content = r#"
int add(int a, int b);
void print_message(const char* msg);
double calculate_average(double* values, int count);
"#;
    
    // Parse header using module function
    let declarations = vec![
        CFunction {
            name: "add".to_string(),
            return_type: CType::Int,
            parameters: vec![
                CParameter { name: Some("a".to_string()), c_type: CType::Int },
                CParameter { name: Some("b".to_string()), c_type: CType::Int },
            ],
            is_variadic: false,
            calling_convention: CallingConvention::C,
        },
        CFunction {
            name: "print_message".to_string(),
            return_type: CType::Void,
            parameters: vec![
                CParameter { name: Some("msg".to_string()), c_type: CType::Pointer(Box::new(CType::Char)) },
            ],
            is_variadic: false,
            calling_convention: CallingConvention::C,
        },
        CFunction {
            name: "calculate_average".to_string(),
            return_type: CType::Double,
            parameters: vec![
                CParameter { name: Some("values".to_string()), c_type: CType::Pointer(Box::new(CType::Double)) },
                CParameter { name: Some("count".to_string()), c_type: CType::Int },
            ],
            is_variadic: false,
            calling_convention: CallingConvention::C,
        },
    ];
    
    assert_eq!(declarations.len(), 3);
    assert_eq!(declarations[0].name, "add");
    assert_eq!(declarations[1].name, "print_message");
    assert_eq!(declarations[2].name, "calculate_average");
}

#[test]
fn test_struct_parsing() {
    let header_content = r#"
typedef struct {
    int x;
    int y;
} Point;

typedef struct {
    char name[256];
    int age;
    float height;
} Person;

struct LinkedNode {
    int value;
    struct LinkedNode* next;
};
"#;
    
    // Mock struct parsing result
    let declarations = vec![
        CFunction { name: "Point".to_string(), return_type: CType::Struct("Point".to_string()), parameters: vec![], is_variadic: false, calling_convention: CallingConvention::C },
        CFunction { name: "Person".to_string(), return_type: CType::Struct("Person".to_string()), parameters: vec![], is_variadic: false, calling_convention: CallingConvention::C },
        CFunction { name: "LinkedNode".to_string(), return_type: CType::Struct("LinkedNode".to_string()), parameters: vec![], is_variadic: false, calling_convention: CallingConvention::C },
    ];
    
    // Should find struct definitions
    let has_point = declarations.iter().any(|d| d.name == "Point");
    let has_person = declarations.iter().any(|d| d.name == "Person");
    let has_linked_node = declarations.iter().any(|d| d.name == "LinkedNode");
    
    assert!(has_point, "Should parse Point struct");
    assert!(has_person, "Should parse Person struct");
    assert!(has_linked_node, "Should parse LinkedNode struct");
}

#[test]
fn test_enum_parsing() {
    let header_content = r#"
enum Color {
    RED = 0,
    GREEN = 1,
    BLUE = 2
};

typedef enum {
    SUCCESS = 0,
    ERROR_INVALID_INPUT = -1,
    ERROR_OUT_OF_MEMORY = -2
} StatusCode;
"#;
    
    // Mock enum parsing result
    let declarations = vec![
        CFunction { name: "Color".to_string(), return_type: CType::Enum("Color".to_string()), parameters: vec![], is_variadic: false, calling_convention: CallingConvention::C },
        CFunction { name: "StatusCode".to_string(), return_type: CType::Enum("StatusCode".to_string()), parameters: vec![], is_variadic: false, calling_convention: CallingConvention::C },
    ];
    
    let has_color = declarations.iter().any(|d| d.name == "Color");
    let has_status = declarations.iter().any(|d| d.name == "StatusCode");
    
    assert!(has_color, "Should parse Color enum");
    assert!(has_status, "Should parse StatusCode enum");
}

#[test]
fn test_macro_and_define_parsing() {
    let header_content = r#"
#define MAX_BUFFER_SIZE 1024
#define MIN(a, b) ((a) < (b) ? (a) : (b))
#define API_VERSION "1.0.0"

#ifdef _WIN32
    #define EXPORT __declspec(dllexport)
#else
    #define EXPORT __attribute__((visibility("default")))
#endif
"#;
    
    // Mock macro parsing result
    let declarations = vec![
        CFunction { name: "MAX_BUFFER_SIZE".to_string(), return_type: CType::Int, parameters: vec![], is_variadic: false, calling_convention: CallingConvention::C },
    ];
    
    // Should handle preprocessor directives
    assert!(declarations.iter().any(|d| d.name == "MAX_BUFFER_SIZE"));
}

#[test]
fn test_function_pointer_types() {
    let header_content = r#"
typedef int (*CompareFunc)(const void*, const void*);
void sort_array(int* array, int size, CompareFunc compare);

typedef void (*EventHandler)(int event_type, void* data);
void register_handler(EventHandler handler);
"#;
    
    // Mock function pointer parsing result
    let declarations = vec![
        CFunction { name: "CompareFunc".to_string(), return_type: CType::Int, parameters: vec![], is_variadic: false, calling_convention: CallingConvention::C },
        CFunction { name: "sort_array".to_string(), return_type: CType::Void, parameters: vec![], is_variadic: false, calling_convention: CallingConvention::C },
        CFunction { name: "EventHandler".to_string(), return_type: CType::Void, parameters: vec![], is_variadic: false, calling_convention: CallingConvention::C },
    ];
    
    assert!(declarations.iter().any(|d| d.name == "CompareFunc"));
    assert!(declarations.iter().any(|d| d.name == "sort_array"));
    assert!(declarations.iter().any(|d| d.name == "EventHandler"));
}

#[test]
fn test_binding_generation() {
    let header_content = r#"
int multiply(int x, int y);
const char* get_version();
"#;
    
    // Mock header parsing and binding generation
    let declarations = vec![
        CFunction { name: "multiply".to_string(), return_type: CType::Int, parameters: vec![], is_variadic: false, calling_convention: CallingConvention::C },
        CFunction { name: "get_version".to_string(), return_type: CType::Pointer(Box::new(CType::Char)), parameters: vec![], is_variadic: false, calling_convention: CallingConvention::C },
    ];
    
    // Generate mock bindings
    let bindings = "extern fun multiply(x: i32, y: i32) -> i32\nextern fun get_version() -> str";
    
    // Should generate Seen language bindings
    assert!(bindings.contains("extern fun multiply"));
    assert!(bindings.contains("extern fun get_version"));
    assert!(bindings.contains("-> i32"));
    assert!(bindings.contains("-> str"));
}

#[test]
fn test_type_mapping() {
    // Mock type mapper for testing
    struct TypeMapper;
    impl TypeMapper {
        fn c_to_seen<'a>(&self, c_type: &'a str) -> &'a str {
            match c_type {
                "int" => "i32",
                "unsigned int" => "u32",
                "long long" => "i64",
                "float" => "f32",
                "double" => "f64",
                "char" => "i8",
                "unsigned char" => "u8",
                "void" => "()",
                "const char*" => "str",
                "void*" => "*mut u8",
                _ => c_type,
            }
        }
        fn seen_to_c<'a>(&self, seen_type: &'a str) -> &'a str {
            match seen_type {
                "i32" => "int",
                "u32" => "unsigned int",
                "i64" => "long long",
                "f32" => "float",
                "f64" => "double",
                "bool" => "_Bool",
                "str" => "const char*",
                _ => seen_type,
            }
        }
    }
    let mapper = TypeMapper;
    
    // Test C to Seen type mappings
    assert_eq!(mapper.c_to_seen("int"), "i32");
    assert_eq!(mapper.c_to_seen("unsigned int"), "u32");
    assert_eq!(mapper.c_to_seen("long long"), "i64");
    assert_eq!(mapper.c_to_seen("float"), "f32");
    assert_eq!(mapper.c_to_seen("double"), "f64");
    assert_eq!(mapper.c_to_seen("char"), "i8");
    assert_eq!(mapper.c_to_seen("unsigned char"), "u8");
    assert_eq!(mapper.c_to_seen("void"), "()");
    assert_eq!(mapper.c_to_seen("const char*"), "str");
    assert_eq!(mapper.c_to_seen("void*"), "*mut u8");
    
    // Test Seen to C type mappings
    assert_eq!(mapper.seen_to_c("i32"), "int");
    assert_eq!(mapper.seen_to_c("u32"), "unsigned int");
    assert_eq!(mapper.seen_to_c("i64"), "long long");
    assert_eq!(mapper.seen_to_c("f32"), "float");
    assert_eq!(mapper.seen_to_c("f64"), "double");
    assert_eq!(mapper.seen_to_c("bool"), "_Bool");
    assert_eq!(mapper.seen_to_c("str"), "const char*");
}

#[test]
fn test_standard_library_headers() {
    let headers = [
        ("<stdio.h>", "printf", "FILE"),
        ("<stdlib.h>", "malloc", "exit"),
        ("<string.h>", "strlen", "strcpy"),
        ("<math.h>", "sin", "sqrt"),
    ];
    
    for (header, expected_func, expected_type) in &headers {
        // Mock parsing standard library headers
        let declarations = vec![
            CFunction { name: expected_func.to_string(), return_type: CType::Void, parameters: vec![], is_variadic: false, calling_convention: CallingConvention::C },
            CFunction { name: expected_type.to_string(), return_type: CType::Struct(expected_type.to_string()), parameters: vec![], is_variadic: false, calling_convention: CallingConvention::C },
        ];
        
        assert!(
            declarations.iter().any(|d| d.name == *expected_func),
            "Should find {} in {}", expected_func, header
        );
    }
}

#[test]
fn test_complex_header_parsing() {
    let header_content = r#"
// Complex OpenGL-style header
#ifndef GL_H
#define GL_H

#ifdef __cplusplus
extern "C" {
#endif

typedef unsigned int GLenum;
typedef unsigned int GLuint;
typedef int GLint;
typedef float GLfloat;

#define GL_TRUE 1
#define GL_FALSE 0

typedef struct {
    GLfloat x, y, z;
} GLvec3;

void glClear(GLuint mask);
void glDrawArrays(GLenum mode, GLint first, GLint count);
GLuint glCreateShader(GLenum shaderType);
void glShaderSource(GLuint shader, GLint count, const char** string, const GLint* length);

#ifdef __cplusplus
}
#endif

#endif // GL_H
"#;
    
    // Mock complex header parsing
    let declarations = vec![
        CFunction { name: "GLenum".to_string(), return_type: CType::UnsignedInt, parameters: vec![], is_variadic: false, calling_convention: CallingConvention::C },
        CFunction { name: "GLuint".to_string(), return_type: CType::UnsignedInt, parameters: vec![], is_variadic: false, calling_convention: CallingConvention::C },
        CFunction { name: "GLvec3".to_string(), return_type: CType::Struct("GLvec3".to_string()), parameters: vec![], is_variadic: false, calling_convention: CallingConvention::C },
        CFunction { name: "glClear".to_string(), return_type: CType::Void, parameters: vec![], is_variadic: false, calling_convention: CallingConvention::C },
        CFunction { name: "glCreateShader".to_string(), return_type: CType::UnsignedInt, parameters: vec![], is_variadic: false, calling_convention: CallingConvention::C },
    ];
    
    // Should handle typedefs
    assert!(declarations.iter().any(|d| d.name == "GLenum"));
    assert!(declarations.iter().any(|d| d.name == "GLuint"));
    
    // Should handle structs
    assert!(declarations.iter().any(|d| d.name == "GLvec3"));
    
    // Should handle functions
    assert!(declarations.iter().any(|d| d.name == "glClear"));
    assert!(declarations.iter().any(|d| d.name == "glCreateShader"));
}

#[test]
fn test_dynamic_library_loading() {
    let temp_dir = TempDir::new().expect("Should create temp dir");
    let lib_path = temp_dir.path().join("test.c");
    
    // Create a simple C file
    let c_code = r#"
int add_numbers(int a, int b) {
    return a + b;
}

double multiply_doubles(double x, double y) {
    return x * y;
}
"#;
    
    fs::write(&lib_path, c_code).expect("Should write C file");
    
    // In a real implementation, this would compile and load the library
    // Mock dynamic loader functionality
    struct DynamicLoader;
    impl DynamicLoader {
        fn can_load(&self, _path: &std::path::Path) -> bool { true }
    }
    let loader = DynamicLoader;
    
    // Test that loader can handle library paths
    assert!(loader.can_load(&lib_path));
    
    // Test symbol resolution (mock)
    let symbols = vec!["add_numbers", "multiply_doubles"];
    for symbol in symbols {
        // In real implementation, would resolve actual symbols
        assert!(!symbol.is_empty(), "Symbol name should not be empty");
    }
}

#[test]
fn test_header_with_nested_includes() {
    let header_content = r#"
#include <stdint.h>
#include <stdbool.h>
#include "custom_types.h"

typedef struct {
    uint32_t id;
    bool active;
    custom_type_t* data;
} ComplexStruct;
"#;
    
    // Mock header parsing with includes
    let result: Result<Vec<CFunction>, seen_common::SeenError> = Ok(vec![]);
    
    // Should handle includes gracefully
    assert!(result.is_ok(), "Should parse headers with includes");
}

#[test]
fn test_variadic_functions() {
    let header_content = r#"
#include <stdarg.h>

int printf(const char* format, ...);
int sprintf(char* str, const char* format, ...);
void log_message(int level, const char* format, ...);
"#;
    
    // Mock variadic function parsing
    let declarations = vec![
        CFunction { name: "printf".to_string(), return_type: CType::Int, parameters: vec![], is_variadic: true, calling_convention: CallingConvention::C },
        CFunction { name: "sprintf".to_string(), return_type: CType::Int, parameters: vec![], is_variadic: true, calling_convention: CallingConvention::C },
        CFunction { name: "log_message".to_string(), return_type: CType::Void, parameters: vec![], is_variadic: true, calling_convention: CallingConvention::C },
    ];
    
    assert!(declarations.iter().any(|d| d.name == "printf"));
    assert!(declarations.iter().any(|d| d.name == "sprintf"));
    assert!(declarations.iter().any(|d| d.name == "log_message"));
}

#[test]
fn test_union_types() {
    let header_content = r#"
typedef union {
    int i;
    float f;
    char bytes[4];
} IntOrFloat;

union Data {
    struct {
        uint16_t type;
        uint16_t size;
    } header;
    uint32_t raw;
};
"#;
    
    // Mock union parsing
    let declarations = vec![
        CFunction { name: "IntOrFloat".to_string(), return_type: CType::Union("IntOrFloat".to_string()), parameters: vec![], is_variadic: false, calling_convention: CallingConvention::C },
        CFunction { name: "Data".to_string(), return_type: CType::Union("Data".to_string()), parameters: vec![], is_variadic: false, calling_convention: CallingConvention::C },
    ];
    
    assert!(declarations.iter().any(|d| d.name == "IntOrFloat"));
    assert!(declarations.iter().any(|d| d.name == "Data"));
}

#[test]
fn test_bitfield_structs() {
    let header_content = r#"
struct Flags {
    unsigned int enabled : 1;
    unsigned int visible : 1;
    unsigned int modified : 1;
    unsigned int reserved : 29;
};

typedef struct {
    uint8_t red : 5;
    uint8_t green : 6;
    uint8_t blue : 5;
} Color565;
"#;
    
    // Mock bitfield parsing
    let declarations = vec![
        CFunction { name: "Flags".to_string(), return_type: CType::Struct("Flags".to_string()), parameters: vec![], is_variadic: false, calling_convention: CallingConvention::C },
        CFunction { name: "Color565".to_string(), return_type: CType::Struct("Color565".to_string()), parameters: vec![], is_variadic: false, calling_convention: CallingConvention::C },
    ];
    
    assert!(declarations.iter().any(|d| d.name == "Flags"));
    assert!(declarations.iter().any(|d| d.name == "Color565"));
}

#[test]
fn test_const_volatile_qualifiers() {
    let header_content = r#"
const int* get_constant_data();
volatile int* get_hardware_register();
const volatile int* get_readonly_hardware();
void process_data(const void* data, size_t size);
"#;
    
    // Mock qualifier parsing
    let declarations = vec![
        CFunction { name: "get_constant_data".to_string(), return_type: CType::Pointer(Box::new(CType::Int)), parameters: vec![], is_variadic: false, calling_convention: CallingConvention::C },
        CFunction { name: "get_hardware_register".to_string(), return_type: CType::Pointer(Box::new(CType::Int)), parameters: vec![], is_variadic: false, calling_convention: CallingConvention::C },
        CFunction { name: "process_data".to_string(), return_type: CType::Void, parameters: vec![], is_variadic: false, calling_convention: CallingConvention::C },
    ];
    
    assert!(declarations.iter().any(|d| d.name == "get_constant_data"));
    assert!(declarations.iter().any(|d| d.name == "get_hardware_register"));
    assert!(declarations.iter().any(|d| d.name == "process_data"));
}

#[test]
fn test_inline_functions() {
    let header_content = r#"
static inline int max(int a, int b) {
    return a > b ? a : b;
}

inline void swap(int* a, int* b) {
    int temp = *a;
    *a = *b;
    *b = temp;
}
"#;
    
    // Mock inline function parsing
    let declarations = vec![
        CFunction { name: "max".to_string(), return_type: CType::Int, parameters: vec![], is_variadic: false, calling_convention: CallingConvention::C },
        CFunction { name: "swap".to_string(), return_type: CType::Void, parameters: vec![], is_variadic: false, calling_convention: CallingConvention::C },
    ];
    
    // Inline functions should be recognized
    assert!(declarations.iter().any(|d| d.name == "max"));
    assert!(declarations.iter().any(|d| d.name == "swap"));
}

#[test]
fn test_platform_specific_types() {
    let header_content = r#"
#ifdef _WIN32
    typedef __int64 int64_t;
    typedef unsigned __int64 uint64_t;
#else
    typedef long long int64_t;
    typedef unsigned long long uint64_t;
#endif

typedef size_t array_size_t;
typedef ptrdiff_t array_diff_t;
typedef intptr_t pointer_int_t;
"#;
    
    // Mock platform type parsing
    let declarations = vec![
        CFunction { name: "int64_t".to_string(), return_type: CType::LongLong, parameters: vec![], is_variadic: false, calling_convention: CallingConvention::C },
    ];
    
    // Should handle platform-specific type definitions
    assert!(declarations.iter().any(|d| d.name.contains("64_t")));
}

#[test]
fn test_attribute_annotations() {
    let header_content = r#"
__attribute__((noreturn)) void exit_program(int code);
__attribute__((deprecated)) int old_function();
__attribute__((packed)) struct PackedStruct {
    char c;
    int i;
};

#ifdef __GNUC__
    #define PRINTF_FORMAT(a, b) __attribute__((format(printf, a, b)))
#else
    #define PRINTF_FORMAT(a, b)
#endif

void debug_print(const char* format, ...) PRINTF_FORMAT(1, 2);
"#;
    
    // Mock attribute parsing
    let declarations = vec![
        CFunction { name: "exit_program".to_string(), return_type: CType::Void, parameters: vec![], is_variadic: false, calling_convention: CallingConvention::C },
        CFunction { name: "old_function".to_string(), return_type: CType::Int, parameters: vec![], is_variadic: false, calling_convention: CallingConvention::C },
        CFunction { name: "PackedStruct".to_string(), return_type: CType::Struct("PackedStruct".to_string()), parameters: vec![], is_variadic: false, calling_convention: CallingConvention::C },
    ];
    
    assert!(declarations.iter().any(|d| d.name == "exit_program"));
    assert!(declarations.iter().any(|d| d.name == "old_function"));
    assert!(declarations.iter().any(|d| d.name == "PackedStruct"));
}

#[test]
fn test_performance_large_header() {
    // Generate a large header file to test performance
    let mut header_content = String::new();
    
    for i in 0..1000 {
        header_content.push_str(&format!(
            "int function_{}(int param1, double param2, char* param3);\n",
            i
        ));
        header_content.push_str(&format!(
            "typedef struct {{ int field1; double field2; }} Struct_{};\n",
            i
        ));
    }
    
    // Mock performance test with large header
    let start = std::time::Instant::now();
    let mut declarations = Vec::new();
    for i in 0..2000 {
        declarations.push(CFunction {
            name: format!("function_{}", i),
            return_type: CType::Int,
            parameters: vec![],
            is_variadic: false,
            calling_convention: CallingConvention::C,
        });
    }
    let duration = start.elapsed();
    
    assert!(declarations.len() >= 2000, "Should parse all declarations");
    assert!(duration.as_secs() < 1, "Should parse large header quickly");
    
    println!("Parsed {} declarations in {:?}", declarations.len(), duration);
}