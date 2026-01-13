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
    void* data;
} SeenArray;

typedef struct {
    bool success;
    SeenString output;
    int exitCode;
} CommandResult;

typedef struct {
    bool success;
    SeenArray diagnostics;
    void* program;  // Opaque pointer to parsed program
} FrontendResult;

typedef struct {
    SeenString severity;
    SeenString file;
    int64_t line;
    int64_t column;
    SeenString message;
} FrontendDiagnostic;

// Forward declarations for generator types
typedef struct CGenerator CGenerator;

// ============================================================================
// String Functions
// ============================================================================

// Create SeenString from C string (does not copy)
static inline SeenString seen_cstr_to_str(const char* s) {
    SeenString result = { strlen(s), (char*)s };
    return result;
}

// Create SeenString with copy
static inline SeenString seen_str_copy(const char* s) {
    size_t len = strlen(s);
    char* data = (char*)malloc(len + 1);
    memcpy(data, s, len + 1);
    SeenString result = { len, data };
    return result;
}

// String length
static inline int64_t seen_str_length(SeenString s) {
    return s.len;
}

// Array length
static inline int64_t seen_arr_length(SeenArray a) {
    return a.len;
}

// Generic length macro
#define seen_length(x) _Generic((x), \
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
static inline SeenString seen_str_concat_ss(SeenString a, SeenString b) {
    char* newdata = (char*)malloc(a.len + b.len + 1);
    memcpy(newdata, a.data, a.len);
    memcpy(newdata + a.len, b.data, b.len);
    newdata[a.len + b.len] = 0;
    SeenString result = { a.len + b.len, newdata };
    return result;
}

// String equality (SeenString == char*)
static inline bool seen_str_eq(SeenString a, const char* b) {
    size_t blen = strlen(b);
    if (a.len != (int64_t)blen) return false;
    return memcmp(a.data, b, blen) == 0;
}

// String equality (SeenString == SeenString)
static inline bool seen_str_eq_ss(SeenString a, SeenString b) {
    if (a.len != b.len) return false;
    return memcmp(a.data, b.data, a.len) == 0;
}

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
static inline SeenString seen_substring(SeenString s, int64_t start, int64_t end) {
    if (start < 0) start = 0;
    if (end > s.len) end = s.len;
    if (start >= end) {
        SeenString empty = { 0, "" };
        return empty;
    }
    int64_t newlen = end - start;
    char* newdata = (char*)malloc(newlen + 1);
    memcpy(newdata, s.data + start, newlen);
    newdata[newlen] = 0;
    SeenString result = { newlen, newdata };
    return result;
}

// Integer to string
static inline SeenString seen_int_to_string(int64_t n) {
    char* buf = (char*)malloc(32);
    sprintf(buf, "%ld", n);
    SeenString result = { strlen(buf), buf };
    return result;
}

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
// Array Functions
// ============================================================================

// Get string from array
static inline SeenString seen_arr_get_str(SeenArray a, int64_t idx) {
    if (idx < 0 || idx >= a.len) {
        SeenString empty = { 0, "" };
        return empty;
    }
    return ((SeenString*)a.data)[idx];
}

// Create empty string array
static inline SeenArray seen_arr_new_str(void) {
    SeenArray arr = { 0, 8, malloc(8 * sizeof(SeenString)) };
    return arr;
}

// Push string to array
static inline void seen_arr_push_str(SeenArray* arr, SeenString s) {
    if (arr->len >= arr->cap) {
        arr->cap *= 2;
        arr->data = realloc(arr->data, arr->cap * sizeof(SeenString));
    }
    ((SeenString*)arr->data)[arr->len++] = s;
}

// Get FrontendDiagnostic from array
static inline FrontendDiagnostic seen_arr_get_diag(SeenArray a, int64_t idx) {
    if (idx < 0 || idx >= a.len) {
        FrontendDiagnostic empty = { { 0, "" }, { 0, "" }, 0, 0, { 0, "" } };
        return empty;
    }
    return ((FrontendDiagnostic*)a.data)[idx];
}

// ============================================================================
// Print Functions
// ============================================================================

static inline void println_cstr(const char* s) {
    printf("%s\n", s);
}

static inline void println_str(SeenString s) {
    printf("%.*s\n", (int)s.len, s.data);
}

#define println(x) _Generic((x), \
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
    SeenArray parts;
    int64_t totalLength;
} StringBuilder;

static inline StringBuilder StringBuilder_new(void) {
    StringBuilder sb = { seen_arr_new_str(), 0 };
    return sb;
}

static inline void StringBuilder_append(StringBuilder* sb, SeenString text) {
    if (text.len == 0) return;
    sb->totalLength += text.len;
    seen_arr_push_str(&sb->parts, text);
}

static inline void StringBuilder_appendLine(StringBuilder* sb, SeenString text) {
    StringBuilder_append(sb, text);
    StringBuilder_append(sb, seen_cstr_to_str("\n"));
}

static inline void StringBuilder_clear(StringBuilder* sb) {
    sb->parts.len = 0;
    sb->totalLength = 0;
}

static inline SeenString StringBuilder_toString(StringBuilder* sb) {
    char* data = (char*)malloc(sb->totalLength + 1);
    char* ptr = data;
    for (int64_t i = 0; i < sb->parts.len; i++) {
        SeenString part = ((SeenString*)sb->parts.data)[i];
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
static inline bool startsWith(SeenString text, SeenString prefix) {
    if (prefix.len > text.len) return false;
    return memcmp(text.data, prefix.data, prefix.len) == 0;
}

// Check if ends with suffix
static inline bool endsWith(SeenString text, SeenString suffix) {
    if (suffix.len > text.len) return false;
    return memcmp(text.data + text.len - suffix.len, suffix.data, suffix.len) == 0;
}

// Check if contains substring
static inline bool contains(SeenString text, SeenString needle) {
    if (needle.len == 0) return true;
    if (needle.len > text.len) return false;
    for (int64_t i = 0; i <= text.len - needle.len; i++) {
        if (memcmp(text.data + i, needle.data, needle.len) == 0) return true;
    }
    return false;
}

#endif // SEEN_RUNTIME_H
