// Seen Runtime Library for Stage2 Bootstrap
// This provides C implementations of Seen standard library functions

#ifndef SEEN_RUNTIME_H
#define SEEN_RUNTIME_H

#include <stdio.h>
#include <stdlib.h>
#include <stdbool.h>
#include <string.h>
#include <stdint.h>

// ============================================================================
// Core Types
// ============================================================================

typedef struct {
    int64_t len;
    char* data;
} SeenString;

typedef struct {
    int64_t len;
    int64_t cap;
    int64_t element_size;
    void* data;
} SeenArray;

typedef struct {
    bool success;
    SeenString output;
} CommandResult;

typedef struct {
    bool success;
    SeenArray* diagnostics;
    void* program;  // Opaque pointer to parsed program
} FrontendResult;

typedef struct {
    SeenString file;
    int64_t line;
    int64_t column;
    SeenString severity;
    SeenString message;
} FrontendDiagnostic;

// Forward declarations for generator types
typedef struct CGenerator CGenerator;

// ============================================================================
// String Functions
// ============================================================================

// Create SeenString from C string (does not copy)
SeenString seen_cstr_to_str(const char* s);

// Create SeenString with copy
static inline SeenString seen_str_copy(const char* s) {
    size_t len = strlen(s);
    char* data = (char*)malloc(len + 1);
    memcpy(data, s, len + 1);
    SeenString result = { len, data };
    return result;
}

// String length
int64_t seen_str_length(SeenString s);
int64_t seen_length(SeenString s);  // For LLVM backend

// Array length (declared here, implemented in .c for LLVM linking)
int64_t seen_arr_length(SeenArray a);

// Generic length macro for C code
#define seen_length_generic(x) _Generic((x), \
    SeenString: seen_str_length, \
    SeenArray: seen_arr_length)(x)

// String concatenation (SeenString + char*)
static inline SeenString seen_str_concat(SeenString a, const char* b) {
    size_t blen = strlen(b);
    char* newdata = (char*)malloc(a.len + blen + 1);
    memcpy(newdata, a.data, a.len);
    memcpy(newdata + a.len, b, blen + 1);
    SeenString result = { a.len + blen, newdata };
    return result;
}

// String concatenation (SeenString + SeenString)
SeenString seen_str_concat_ss(SeenString a, SeenString b);

// String equality (SeenString == char*)
static inline bool seen_str_eq(SeenString a, const char* b) {
    size_t blen = strlen(b);
    if (a.len != (int64_t)blen) return false;
    return memcmp(a.data, b, blen) == 0;
}

// String equality (SeenString == SeenString) - exported for LLVM backend
bool seen_str_eq_ss(SeenString a, SeenString b);
bool seen_str_ne_ss(SeenString a, SeenString b);

// Check if string ends with suffix
static inline bool seen_ends_with(SeenString s, const char* suffix) {
    size_t slen = strlen(suffix);
    if ((int64_t)slen > s.len) return false;
    return memcmp(s.data + s.len - slen, suffix, slen) == 0;
}

// Check if string starts with prefix
static inline bool seen_starts_with(SeenString s, const char* prefix) {
    size_t plen = strlen(prefix);
    if ((int64_t)plen > s.len) return false;
    return memcmp(s.data, prefix, plen) == 0;
}

// Substring
SeenString seen_substring(SeenString s, int64_t start, int64_t end);

// Integer to string
SeenString seen_int_to_string(int64_t n);

// Bool to string
SeenString seen_bool_to_string(bool b);

// Char (Unicode code point) to string
SeenString seen_char_to_str(int64_t c);

// Get character at index (returns code point)
int64_t seen_char_at(SeenString s, int64_t index);

// Char to Int conversion (identity since Char is stored as i64)
int64_t Char_toInt(int64_t c);

// Unwrap functions for optional types
int64_t Int_unwrap(int64_t val);
void* Optional_unwrap(void* ptr);

// Alias for compatibility
static inline char* seen_to_string(int64_t n) {
    char* buf = (char*)malloc(32);
    sprintf(buf, "%ld", n);
    return buf;
}

// Check if string contains substring
static inline bool seen_contains(SeenString s, const char* needle) {
    size_t nlen = strlen(needle);
    if (nlen == 0) return true;
    if ((int64_t)nlen > s.len) return false;
    for (int64_t i = 0; i <= s.len - (int64_t)nlen; i++) {
        if (memcmp(s.data + i, needle, nlen) == 0) return true;
    }
    return false;
}

// ============================================================================
// Array Functions (exported for LLVM linking)
// ============================================================================

// Get string from array
SeenString seen_arr_get_str(SeenArray a, int64_t idx);

// Create empty string array
SeenArray seen_arr_new_str(void);

// Create empty pointer array
SeenArray seen_arr_new_ptr(void);

// Create heap-allocated string array
SeenArray* seen_arr_new_str_ptr(void);

// Create heap-allocated pointer array
SeenArray* seen_arr_new_ptr_ptr(void);

// Create array with custom element_size (for data types)
SeenArray* seen_arr_new_with_size_ptr(int64_t element_size);

// Push string to array
void seen_arr_push_str(SeenArray* arr, SeenString s);

// Push i64 by value (not pointer) - for Array<Int>
void seen_arr_push_i64(SeenArray* arr, int64_t val);

// Generic push for pointer types
void seen_arr_push_ptr(SeenArray* arr, void* p);

// Generic Array_push (for generated code)
int64_t Array_push(SeenArray* arr, void* element);

// Get FrontendDiagnostic from array
FrontendDiagnostic* seen_arr_get_diag(SeenArray a, int64_t idx);

// Generic get for pointer types (arrays of pointers - class types)
void* seen_arr_get_ptr(SeenArray a, int64_t idx);

// Generic get for inline elements (data types stored by value)
void* seen_arr_get_element(SeenArray a, int64_t idx);

// ============================================================================
// Print Functions
// ============================================================================

void println_cstr(const char* s);
void println_str(SeenString s);
void println(SeenString s);  // For LLVM backend

#define println_generic(x) _Generic((x), \
    char*: println_cstr, \
    const char*: println_cstr, \
    SeenString: println_str)(x)

// ============================================================================
// File I/O Functions
// ============================================================================

// Read entire file to string
SeenString readText(SeenString path);

// Write string to file
bool writeText(SeenString path, SeenString content);

// ============================================================================
// Process Functions
// ============================================================================

// Run shell command
CommandResult runCommand(SeenString cmd);

// Check if command was successful
bool commandWasSuccessful(CommandResult result);

// ============================================================================
// Environment Functions
// ============================================================================

// Get command line arguments
SeenArray args(void);

// Initialize runtime with argc/argv (call from main)
void seen_runtime_init(int argc, char** argv);

// ============================================================================
// StringBuilder
// ============================================================================

typedef struct {
    SeenArray* parts;
    int64_t totalLength;
} StringBuilder;

// LLVM backend uses pointer-based calling convention - these are declared
// as external functions implemented in seen_runtime.c
void* StringBuilder_new(void);
int64_t StringBuilder_append(void* sb, SeenString text);
SeenString StringBuilder_toString(void* sb);
int64_t StringBuilder_length(void* sb);
void StringBuilder_clear_impl(void* sb);

// Helper inline version for code that uses value-based StringBuilder
static inline StringBuilder StringBuilder_new_value(void) {
    StringBuilder sb = { seen_arr_new_str_ptr(), 0 };
    return sb;
}

static inline void StringBuilder_append_value(StringBuilder* sb, SeenString text) {
    if (text.len == 0) return;
    sb->totalLength += text.len;
    seen_arr_push_str(sb->parts, text);
}

static inline void StringBuilder_appendLine(StringBuilder* sb, SeenString text) {
    StringBuilder_append_value(sb, text);
    StringBuilder_append_value(sb, seen_cstr_to_str("\n"));
}

static inline void StringBuilder_clear(StringBuilder* sb) {
    sb->parts->len = 0;
    sb->totalLength = 0;
}

static inline SeenString StringBuilder_toString_value(StringBuilder* sb) {
    char* data = (char*)malloc(sb->totalLength + 1);
    char* ptr = data;
    for (int64_t i = 0; i < sb->parts->len; i++) {
        SeenString part = ((SeenString*)sb->parts->data)[i];
        memcpy(ptr, part.data, part.len);
        ptr += part.len;
    }
    *ptr = 0;
    SeenString result = { sb->totalLength, data };
    return result;
}

// ============================================================================
// String Utility Functions (from str.string module)
// ============================================================================

// Split string by delimiter
SeenArray split(SeenString text, SeenString delimiter);

// Trim whitespace
SeenString trim(SeenString text);

// Check if starts with prefix
bool startsWith(SeenString text, SeenString prefix);

// Check if ends with suffix
bool endsWith(SeenString text, SeenString suffix);

// Check if contains substring
bool contains(SeenString text, SeenString needle);

#endif // SEEN_RUNTIME_H
