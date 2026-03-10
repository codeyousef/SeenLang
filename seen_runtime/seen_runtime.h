// Seen Runtime Library for Stage2 Bootstrap
// This provides C implementations of Seen standard library functions

#ifndef SEEN_RUNTIME_H
#define SEEN_RUNTIME_H

#include <stdio.h>
#include <stdlib.h>
#include <stdbool.h>
#include <string.h>
#include <stdint.h>
#include <inttypes.h>

// ============================================================================
// Aligned memory free — must use _aligned_free on Windows (mingw-w64)
// ============================================================================
#ifdef _WIN32
#include <malloc.h>
#define seen_aligned_free(p) _aligned_free(p)
#else
#define seen_aligned_free(p) free(p)
#endif

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
int64_t seen_arr_length_ptr(SeenArray* a);

// Pointer-based array accessor variants (avoid struct-by-value ABI issues)
SeenString seen_arr_get_str_ptr(SeenArray* a, int64_t idx);
FrontendDiagnostic* seen_arr_get_diag_ptr(SeenArray* a, int64_t idx);
void* seen_arr_get_ptr_ptr(SeenArray* a, int64_t idx);
void* seen_arr_get_element_ptr(SeenArray* a, int64_t idx);

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

// Create a string from an integer character code (Unicode code point)
// JavaScript-like String.fromCharCode functionality
SeenString String_fromCharCode(int64_t code);

// ============================================================================
// Char Classification & Conversion Functions (ASCII)
// ============================================================================

int64_t Char_isDigit(int64_t c);
int64_t Char_isAlpha(int64_t c);
int64_t Char_isAlphanumeric(int64_t c);
int64_t Char_isUpperCase(int64_t c);
int64_t Char_isLowerCase(int64_t c);
int64_t Char_isWhitespace(int64_t c);
int64_t Char_toUpperCase(int64_t c);
int64_t Char_toLowerCase(int64_t c);

// ============================================================================
// String Case Conversion, Parsing & Utility Functions
// ============================================================================

SeenString String_toUpperCase(SeenString s);
SeenString String_toLowerCase(SeenString s);
int64_t String_toInt(SeenString s);
double  String_toFloat(SeenString s);
SeenString String_reverse(SeenString s);
int64_t String_isEmpty(SeenString s);
int64_t String_count(SeenString s, SeenString needle);

// Unwrap functions for optional types
int64_t Int_unwrap(int64_t val);
void* Optional_unwrap(void* ptr);

// Alias for compatibility
static inline char* seen_to_string(int64_t n) {
    char* buf = (char*)malloc(32);
    sprintf(buf, "%" PRId64, n);
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
// Generic Array_set (for generated code)
void Array_set(SeenArray* arr, int64_t index, void* element);

// Get FrontendDiagnostic from array
FrontendDiagnostic* seen_arr_get_diag(SeenArray a, int64_t idx);

// Generic get for pointer types (arrays of pointers - class types)
void* seen_arr_get_ptr(SeenArray a, int64_t idx);

// Generic get for inline elements (data types stored by value)
void* seen_arr_get_element(SeenArray a, int64_t idx);

// Get i64 value from array at index (for @derive(Deserialize))
int64_t seen_arr_get_i64(SeenArray* arr, int64_t idx);

// ============================================================================
// Print Functions
// ============================================================================

void println_cstr(const char* s);
void println_str(SeenString s);
void println(SeenString s);  // For LLVM backend
void print(SeenString s);    // Print without newline

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
// Panic Functions
// ============================================================================

// Called when integer overflow is detected (with --panic-on-overflow flag)
void seen_panic_overflow(const char* op, int64_t left, int64_t right);

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
void* StringBuilder_new_with_capacity(int64_t cap);
int64_t StringBuilder_append(void* sb, SeenString text);
SeenString StringBuilder_toString(void* sb);
int64_t StringBuilder_length(void* sb);
void StringBuilder_clear_impl(void* sb);
int64_t StringBuilder_appendFloat(void* sb, double f);
int64_t StringBuilder_appendInt(void* sb, int64_t n);

// Helper inline version for code that uses value-based StringBuilder
static inline StringBuilder StringBuilder_new_value(void) {
    StringBuilder sb = { seen_arr_new_str_ptr(), 0 };
    return sb;
}

// Fast-path push for StringBuilder (skips validation, relies on LTO inlining)
static inline void seen_arr_push_str_fast(SeenArray* arr, SeenString s) {
    if (__builtin_expect(arr->len >= arr->cap, 0)) {
        int64_t new_cap = arr->cap == 0 ? 8 : arr->cap * 2;
        arr->cap = new_cap;
        arr->data = realloc(arr->data, new_cap * sizeof(SeenString));
    }
    ((SeenString*)arr->data)[arr->len++] = s;
}

static inline void StringBuilder_append_value(StringBuilder* sb, SeenString text) {
    if (text.len == 0) return;
    sb->totalLength += text.len;
    seen_arr_push_str_fast(sb->parts, text);
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

// Find index of substring
int64_t indexOf(SeenString text, SeenString needle, int64_t start);

// ============================================================================
// Stdin/Stdout Functions (for LSP and interactive programs)
// ============================================================================

// Read a single line from stdin (blocking, includes newline if present)
SeenString __ReadStdinLine(void);

// Read exactly N bytes from stdin (blocking)
SeenString __ReadStdinBytes(int64_t count);

// Flush stdout buffer
void __FlushStdout(void);

// Print string to stdout without newline (for LSP Content-Length headers)
void __PrintRaw(SeenString s);

// ============================================================================
// Profiling Runtime Functions (for @profile decorator)
// ============================================================================

// Profile entry for tracking function call statistics
typedef struct {
    const char* name;           // Function name
    uint64_t call_count;        // Number of times called
    uint64_t total_ns;          // Total time spent in nanoseconds
    uint64_t start_ns;          // Start time of current call (for nested tracking)
    uint64_t self_ns;           // Time excluding children
    int depth;                  // Current call depth (for recursion)
} SeenProfileEntry;

// Called when entering a profiled function
void __seen_profile_enter(const char* func_name);

// Called when exiting a profiled function
void __seen_profile_exit(const char* func_name);

// Print profiling report (called automatically at program exit)
void __seen_profile_report(void);

// Initialize profiling system
void __seen_profile_init(void);

// ============================================================================
// Component Framework Runtime (for @component decorator)
// ============================================================================

// Register a component with the global registry
// Returns the component ID, or -1 on failure
int64_t __seen_component_register(SeenString name, int64_t parent_id);

// Unregister a component and remove from registry
void __seen_component_unregister(int64_t id);

// Set component state (0=Uninitialized, 1=Initialized, 2=Mounted, 3=Unmounted, 4=Destroyed)
void __seen_component_set_state(int64_t id, int64_t state);

// Get children of a component as an Array<Int>
SeenArray* __seen_component_get_children(int64_t id);

// Mount component tree recursively (depth-first)
void __seen_component_mount_tree(int64_t id);

// Unmount component tree recursively (reverse order)
void __seen_component_unmount_tree(int64_t id);

// Destroy component tree recursively (reverse order)
void __seen_component_destroy_tree(int64_t id);

// ============================================================================
// Store Framework Runtime (for @store decorator)
// ============================================================================

// Log a store mutation
void __seen_store_log_mutation(SeenString method_name, SeenString field_name);

// Take a store snapshot
void __seen_store_take_snapshot(void);

// Get total mutation count
int64_t __seen_store_get_mutation_count(void);

// Set current frame for mutation tracking
void __seen_store_set_frame(int64_t frame);

// ============================================================================
// Middleware Framework Runtime (for @middleware_stack decorator)
// ============================================================================

// Increment middleware invocation counter
void __seen_middleware_increment_count(void);

// Get middleware invocation count
int64_t __seen_middleware_get_count(void);

// ============================================================================
// CPU Feature Detection (for SIMD multi-versioned codegen)
// ============================================================================

// Detect CPU features (called automatically at startup)
void seen_cpu_detect(void);

// Check if a specific CPU feature is available (e.g., "avx2", "sse4.2", "neon")
int64_t seen_cpu_has_feature(SeenString name);

// Returns SIMD tier: 0=scalar, 1=SSE4.2, 2=AVX2, 3=AVX-512, 4=NEON, 5=SVE
int64_t seen_cpu_simd_tier(void);

// ============================================================================
// Perf Dashboard JSON Export
// ============================================================================

// Export runtime performance stats (CPU features, SIMD tier) as JSON to a file
void seen_perf_export_json(SeenString path);

// ============================================================================
// SIMD Vector Runtime Functions
// ============================================================================

// 4-wide float (SSE)
void* seen_simd_f4_splat(double val);
void* seen_simd_f4_add(void* a, void* b);
void* seen_simd_f4_sub(void* a, void* b);
void* seen_simd_f4_mul(void* a, void* b);
void* seen_simd_f4_div(void* a, void* b);
void* seen_simd_f4_min(void* a, void* b);
void* seen_simd_f4_max(void* a, void* b);
double seen_simd_f4_sum(void* a);
double seen_simd_f4_dot(void* a, void* b);
void* seen_simd_f4_load(void* ptr);
void seen_simd_f4_store(void* vec, void* ptr);

// 8-wide float (AVX2)
void* seen_simd_f8_splat(double val);
void* seen_simd_f8_add(void* a, void* b);
void* seen_simd_f8_sub(void* a, void* b);
void* seen_simd_f8_mul(void* a, void* b);
void* seen_simd_f8_div(void* a, void* b);
void* seen_simd_f8_min(void* a, void* b);
void* seen_simd_f8_max(void* a, void* b);
double seen_simd_f8_sum(void* a);
double seen_simd_f8_dot(void* a, void* b);
void* seen_simd_f8_load(void* ptr);
void seen_simd_f8_store(void* vec, void* ptr);

// Auto-dispatch SIMD array operations
double seen_simd_reduce_sum(void* arr_data, int64_t len);
double seen_simd_dot_product(void* a_data, void* b_data, int64_t len);
double seen_simd_reduce_min(void* arr_data, int64_t len);
double seen_simd_reduce_max(void* arr_data, int64_t len);
void seen_simd_prefix_sum(void* arr_data, int64_t len);

// ============================================================================
// 32-bit Index Arena Allocator
// ============================================================================

// Create a new arena with given capacity in bytes
void* seen_arena_new(int64_t capacity);

// Allocate size bytes from arena, returns 32-bit index (0xFFFFFFFF on failure)
int64_t seen_arena_alloc(void* arena, int64_t size);

// Convert 32-bit index to pointer
void* seen_arena_get(void* arena, int64_t index);

// Reset arena (bulk free, offset back to 0)
void seen_arena_reset(void* arena);

// Free arena and backing memory
void seen_arena_free(void* arena);

// Get bytes used in arena
int64_t seen_arena_used(void* arena);

// Get remaining capacity in arena
int64_t seen_arena_remaining(void* arena);

// ============================================================================
// Cache-Oblivious Layout Probes
// ============================================================================

// Create a new layout probe with a tag name
void* seen_probe_new(SeenString tag);

// Record an access at the given address
void seen_probe_access(void* probe, void* addr);

// Print probe report to stderr
void seen_probe_report(void* probe);

// Free probe resources
void seen_probe_free(void* probe);

// ============================================================================
// Region Runtime (for @preallocate decorator)
// Full declarations in seen_region.h
// ============================================================================

#include "seen_region.h"

// ============================================================================
// Stack Region Allocator
// ============================================================================
int64_t seen_stack_region_new(int64_t capacity);
int64_t seen_stack_region_alloc(int64_t handle, int64_t size);
void    seen_stack_region_pop(int64_t handle, int64_t size);
void    seen_stack_region_reset(int64_t handle);
void    seen_stack_region_destroy(int64_t handle);
int64_t seen_stack_region_remaining(int64_t handle);

// ============================================================================
// Pool Region Allocator
// ============================================================================
int64_t seen_pool_region_new(int64_t block_size, int64_t count);
int64_t seen_pool_region_alloc(int64_t handle);
void    seen_pool_region_free(int64_t handle, int64_t ptr);
void    seen_pool_region_destroy(int64_t handle);
void    seen_pool_region_reset(int64_t handle);
int64_t seen_pool_region_available(int64_t handle);

// ============================================================================
// Memory-Mapped Region
// ============================================================================
int64_t seen_mapped_new(int64_t path_len, char *path_data, int64_t size, int64_t flags);
int64_t seen_mapped_data(int64_t handle);
void    seen_mapped_sync(int64_t handle);
void    seen_mapped_free(int64_t handle);
int64_t seen_mapped_length(int64_t handle);

// ============================================================================
// RwLock (Reader-Writer Lock)
// ============================================================================
int64_t seen_rwlock_new(void);
void    seen_rwlock_read_lock(int64_t handle);
void    seen_rwlock_read_unlock(int64_t handle);
void    seen_rwlock_write_lock(int64_t handle);
void    seen_rwlock_write_unlock(int64_t handle);
void    seen_rwlock_destroy(int64_t handle);

// ============================================================================
// Barrier
// ============================================================================
int64_t seen_barrier_new(int64_t count);
int64_t seen_barrier_wait(int64_t handle);
void    seen_barrier_destroy(int64_t handle);

// ============================================================================
// Thread-Local Storage
// ============================================================================
int64_t seen_tls_new(void);
void    seen_tls_set(int64_t key, int64_t value);
int64_t seen_tls_get(int64_t key);
void    seen_tls_destroy(int64_t key);

// ============================================================================
// Work-Stealing Thread Pool
// ============================================================================
int64_t seen_ws_pool_new(int64_t nworkers);
void    seen_ws_pool_submit(int64_t pool_handle, int64_t fn_ptr, int64_t arg);
void    seen_ws_pool_shutdown(int64_t pool_handle);

// ============================================================================
// VSD Pinning Runtime (for @preallocate pinning extensions)
// Full declarations in seen_pinning.h
// ============================================================================

#include "seen_pinning.h"

// ============================================================================
// Identity Protection Runtime (for secure handle masking)
// Full declarations in seen_identity.h
// ============================================================================

#include "seen_identity.h"

// ============================================================================
// Trusted Execution Environment Runtime (for @enclave decorator)
// Full declarations in seen_tee.h
// ============================================================================

#include "seen_tee.h"

// ============================================================================
// Hot Reload Runtime (for @hot_reload decorator)
// Full declarations in seen_hotreload.h
// ============================================================================

#include "seen_hotreload.h"

// ============================================================================
// : Tuple structs for variadic generics (arities 1-8)
// ============================================================================

typedef struct { int64_t _0; } SeenTuple1;
typedef struct { int64_t _0; int64_t _1; } SeenTuple2;
typedef struct { int64_t _0; int64_t _1; int64_t _2; } SeenTuple3;
typedef struct { int64_t _0; int64_t _1; int64_t _2; int64_t _3; } SeenTuple4;
typedef struct { int64_t _0; int64_t _1; int64_t _2; int64_t _3; int64_t _4; } SeenTuple5;
typedef struct { int64_t _0; int64_t _1; int64_t _2; int64_t _3; int64_t _4; int64_t _5; } SeenTuple6;
typedef struct { int64_t _0; int64_t _1; int64_t _2; int64_t _3; int64_t _4; int64_t _5; int64_t _6; } SeenTuple7;
typedef struct { int64_t _0; int64_t _1; int64_t _2; int64_t _3; int64_t _4; int64_t _5; int64_t _6; int64_t _7; } SeenTuple8;

// ============================================================================
// SmallVec — inline storage with heap fallback
// ============================================================================
typedef struct {
    int64_t *data;      // heap data (NULL when using inline storage)
    int64_t  inline_buf[8]; // inline storage (up to 8 elements)
    int64_t  length;
    int64_t  capacity;
    int64_t  inline_cap; // threshold for heap fallback
} SeenSmallVec;

int64_t  seen_small_vec_new(int64_t inline_capacity);
void     seen_small_vec_push_i64(int64_t handle, int64_t value);
void     seen_small_vec_push_f64(int64_t handle, int64_t value);
void     seen_small_vec_push_str(int64_t handle, int64_t value);
int64_t  seen_small_vec_get_i64(int64_t handle, int64_t index);
int64_t  seen_small_vec_get_f64(int64_t handle, int64_t index);
int64_t  seen_small_vec_get_str(int64_t handle, int64_t index);
int64_t  seen_small_vec_length(int64_t handle);
void     seen_small_vec_clear(int64_t handle);

#endif // SEEN_RUNTIME_H
