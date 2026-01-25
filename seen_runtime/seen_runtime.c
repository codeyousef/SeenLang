// Seen Runtime Library Implementation
// Provides C implementations of Seen standard library functions

#define _POSIX_C_SOURCE 200809L

#include "seen_runtime.h"
#include <errno.h>
#include <sys/wait.h>
#include <sys/stat.h>
#include <unistd.h>

// ============================================================================
// Global State
// ============================================================================

static int g_argc = 0;
static char** g_argv = NULL;

void seen_runtime_init(int argc, char** argv) {
    g_argc = argc;
    g_argv = argv;
}

// ============================================================================
// File I/O Primitive Functions (used by Seen stdlib io.file)
// ============================================================================

// Open file and return file descriptor (or -1 on error)
int64_t __OpenFile(SeenString path, SeenString mode) {
    char* cpath = (char*)malloc(path.len + 1);
    memcpy(cpath, path.data, path.len);
    cpath[path.len] = 0;

    char* cmode = (char*)malloc(mode.len + 1);
    memcpy(cmode, mode.data, mode.len);
    cmode[mode.len] = 0;

    FILE* f = fopen(cpath, cmode);
    free(cpath);
    free(cmode);

    if (!f) {
        return -1;
    }
    return (int64_t)(intptr_t)f;
}

// Read entire file content
SeenString __ReadFile(int64_t fd) {
    FILE* f = (FILE*)(intptr_t)fd;
    if (!f) {
        SeenString empty = { 0, "" };
        return empty;
    }

    // Get current position and file size
    long cur = ftell(f);
    fseek(f, 0, SEEK_END);
    long size = ftell(f);
    fseek(f, cur, SEEK_SET);  // Restore position

    // For full file read, read from current position
    long remaining = size - cur;
    char* data = (char*)malloc(remaining + 1);
    size_t read = fread(data, 1, remaining, f);
    data[read] = 0;

    SeenString result = { read, data };
    return result;
}

// Read bytes from file
SeenArray __ReadFileBytes(int64_t fd, int64_t size) {
    SeenArray arr = seen_arr_new_ptr();
    FILE* f = (FILE*)(intptr_t)fd;
    if (!f) return arr;

    for (int64_t i = 0; i < size; i++) {
        int c = fgetc(f);
        if (c == EOF) break;
        int64_t* val = (int64_t*)malloc(sizeof(int64_t));
        *val = c;
        seen_arr_push_ptr(&arr, val);
    }
    return arr;
}

// Write string to file
int64_t __WriteFile(int64_t fd, SeenString content) {
    FILE* f = (FILE*)(intptr_t)fd;
    if (!f) return -1;

    size_t written = fwrite(content.data, 1, content.len, f);
    return (int64_t)written;
}

// Write bytes to file
int64_t __WriteFileBytes(int64_t fd, SeenArray bytes) {
    FILE* f = (FILE*)(intptr_t)fd;
    if (!f) return -1;

    for (int64_t i = 0; i < bytes.len; i++) {
        int64_t* val = (int64_t*)((char*)bytes.data + i * sizeof(int64_t));
        fputc((int)*val, f);
    }
    return bytes.len;
}

// Close file
int64_t __CloseFile(int64_t fd) {
    FILE* f = (FILE*)(intptr_t)fd;
    if (!f) return -1;
    return fclose(f);
}

// Get file size
int64_t __FileSize(int64_t fd) {
    FILE* f = (FILE*)(intptr_t)fd;
    if (!f) return -1;

    long cur = ftell(f);
    fseek(f, 0, SEEK_END);
    long size = ftell(f);
    fseek(f, cur, SEEK_SET);
    return size;
}

// Get file error message (empty if no error)
SeenString __FileError(int64_t fd) {
    FILE* f = (FILE*)(intptr_t)fd;
    if (!f || ferror(f)) {
        return seen_str_copy(strerror(errno));
    }
    SeenString empty = { 0, "" };
    return empty;
}

// Check if file exists
bool __FileExists(SeenString path) {
    char* cpath = (char*)malloc(path.len + 1);
    memcpy(cpath, path.data, path.len);
    cpath[path.len] = 0;

    FILE* f = fopen(cpath, "r");
    free(cpath);
    if (f) {
        fclose(f);
        return true;
    }
    return false;
}

// Delete file
bool __DeleteFile(SeenString path) {
    char* cpath = (char*)malloc(path.len + 1);
    memcpy(cpath, path.data, path.len);
    cpath[path.len] = 0;

    int result = remove(cpath);
    free(cpath);
    return result == 0;
}

// Create directory
bool __CreateDirectory(SeenString path) {
    char* cpath = (char*)malloc(path.len + 1);
    memcpy(cpath, path.data, path.len);
    cpath[path.len] = 0;

    int result = mkdir(cpath, 0755);
    free(cpath);
    return result == 0;
}

// ============================================================================
// Process Primitive Functions (used by Seen stdlib process.process)
// ============================================================================

// Result type for command execution (matches Seen's CommandResult data type)
typedef struct {
    bool success;
    SeenString output;
} SeenCommandResult;

// Execute program by path and return exit code
int64_t __ExecuteProgram(SeenString path) {
    char* cpath = (char*)malloc(path.len + 1);
    memcpy(cpath, path.data, path.len);
    cpath[path.len] = 0;

    int status = system(cpath);
    free(cpath);

    return WEXITSTATUS(status);
}

// Execute command and capture output
// Returns a pointer to a malloc'd SeenCommandResult (to avoid ABI issues with large struct returns)
SeenCommandResult* __ExecuteCommand(SeenString cmd) {
    SeenCommandResult* result = (SeenCommandResult*)malloc(sizeof(SeenCommandResult));
    result->success = false;
    result->output.len = 0;
    result->output.data = "";

    char* ccmd = (char*)malloc(cmd.len + 1);
    memcpy(ccmd, cmd.data, cmd.len);
    ccmd[cmd.len] = 0;

    FILE* pipe = popen(ccmd, "r");
    free(ccmd);

    if (!pipe) {
        return result;
    }

    char* output = (char*)malloc(4096);
    size_t capacity = 4096;
    size_t length = 0;

    char buffer[256];
    while (fgets(buffer, sizeof(buffer), pipe)) {
        size_t buflen = strlen(buffer);
        if (length + buflen >= capacity) {
            capacity *= 2;
            output = (char*)realloc(output, capacity);
        }
        memcpy(output + length, buffer, buflen);
        length += buflen;
    }
    output[length] = 0;

    int status = pclose(pipe);
    result->success = (WEXITSTATUS(status) == 0);
    result->output.len = length;
    result->output.data = output;

    return result;
}

// ============================================================================
// Environment Primitive Functions (used by Seen stdlib env.env)
// ============================================================================

// Get command line arguments
SeenArray __GetCommandLineArgs(void) {
    SeenArray arr = seen_arr_new_str();
    for (int i = 0; i < g_argc; i++) {
        SeenString arg = seen_str_copy(g_argv[i]);
        seen_arr_push_str(&arr, arg);
    }
    return arr;
}

// Check if environment variable exists
bool __HasEnv(SeenString name) {
    char* cname = (char*)malloc(name.len + 1);
    memcpy(cname, name.data, name.len);
    cname[name.len] = 0;

    char* val = getenv(cname);
    free(cname);
    return val != NULL;
}

// Get environment variable value
SeenString __GetEnv(SeenString name) {
    char* cname = (char*)malloc(name.len + 1);
    memcpy(cname, name.data, name.len);
    cname[name.len] = 0;

    char* val = getenv(cname);
    free(cname);

    if (val) {
        return seen_str_copy(val);
    }
    SeenString empty = { 0, "" };
    return empty;
}

// Set environment variable
bool __SetEnv(SeenString name, SeenString value) {
    char* cname = (char*)malloc(name.len + 1);
    memcpy(cname, name.data, name.len);
    cname[name.len] = 0;

    char* cvalue = (char*)malloc(value.len + 1);
    memcpy(cvalue, value.data, value.len);
    cvalue[value.len] = 0;

    int result = setenv(cname, cvalue, 1);
    free(cname);
    free(cvalue);
    return result == 0;
}

// Remove environment variable
bool __RemoveEnv(SeenString name) {
    char* cname = (char*)malloc(name.len + 1);
    memcpy(cname, name.data, name.len);
    cname[name.len] = 0;

    int result = unsetenv(cname);
    free(cname);
    return result == 0;
}

// ============================================================================
// String Utility Functions
// ============================================================================

SeenArray split(SeenString text, SeenString delimiter) {
    SeenArray result = seen_arr_new_str();

    if (delimiter.len == 0) {
        // Split into characters
        for (int64_t i = 0; i < text.len; i++) {
            char* ch = (char*)malloc(2);
            ch[0] = text.data[i];
            ch[1] = 0;
            SeenString s = { 1, ch };
            seen_arr_push_str(&result, s);
        }
        return result;
    }

    int64_t start = 0;
    for (int64_t i = 0; i <= text.len - delimiter.len; i++) {
        if (memcmp(text.data + i, delimiter.data, delimiter.len) == 0) {
            // Found delimiter
            int64_t len = i - start;
            char* part = (char*)malloc(len + 1);
            memcpy(part, text.data + start, len);
            part[len] = 0;
            SeenString s = { len, part };
            seen_arr_push_str(&result, s);
            start = i + delimiter.len;
            i = start - 1; // Will be incremented by loop
        }
    }

    // Add remaining part
    int64_t len = text.len - start;
    char* part = (char*)malloc(len + 1);
    memcpy(part, text.data + start, len);
    part[len] = 0;
    SeenString s = { len, part };
    seen_arr_push_str(&result, s);

    return result;
}

static inline bool is_whitespace(char c) {
    return c == ' ' || c == '\t' || c == '\n' || c == '\r';
}

SeenString trim(SeenString text) {
    int64_t start = 0;
    int64_t end = text.len;

    while (start < end && is_whitespace(text.data[start])) {
        start++;
    }

    while (end > start && is_whitespace(text.data[end - 1])) {
        end--;
    }

    if (start == 0 && end == text.len) {
        return text;
    }

    int64_t len = end - start;
    char* data = (char*)malloc(len + 1);
    memcpy(data, text.data + start, len);
    data[len] = 0;
    SeenString result = { len, data };
    return result;
}

// ============================================================================
// Array Functions (implementations for LLVM backend linking)
// ============================================================================

SeenString seen_arr_get_str(SeenArray a, int64_t idx) {
    if (a.element_size != (int64_t)sizeof(SeenString)) {
        fprintf(stderr, "seen_arr_get_str: element_size mismatch (%ld)\n", (long)a.element_size);
        abort();
    }
    if (idx < 0 || idx >= a.len) {
        SeenString empty = { 0, "" };
        return empty;
    }
    return ((SeenString*)a.data)[idx];
}

SeenArray seen_arr_new_str(void) {
    void* data = malloc(8 * sizeof(SeenString));
    // element_size = sizeof(SeenString) = 16 bytes
    SeenArray arr = { 0, 8, sizeof(SeenString), data };
    return arr;
}

SeenArray seen_arr_new_ptr(void) {
    void* data = malloc(8 * sizeof(void*));
    SeenArray arr = { 0, 8, sizeof(void*), data };
    return arr;
}

// Heap-allocated versions that return ptr for LLVM ABI compatibility
SeenArray* seen_arr_new_str_ptr(void) {
    SeenArray* arr = (SeenArray*)malloc(sizeof(SeenArray));
    arr->data = malloc(8 * sizeof(SeenString));
    arr->len = 0;
    arr->cap = 8;
    arr->element_size = sizeof(SeenString);
    return arr;
}

SeenArray* seen_arr_new_ptr_ptr(void) {
    SeenArray* arr = (SeenArray*)malloc(sizeof(SeenArray));
    arr->data = malloc(8 * sizeof(void*));
    arr->len = 0;
    arr->cap = 8;
    arr->element_size = sizeof(void*);
    return arr;
}

// Create array with custom element_size (for data types like ItemNode)
SeenArray* seen_arr_new_with_size_ptr(int64_t element_size) {
    SeenArray* arr = (SeenArray*)malloc(sizeof(SeenArray));
    if (element_size <= 0 || element_size > 4096) {
        fprintf(stderr, "seen_arr_new_with_size_ptr: invalid element_size=%ld\n", (long)element_size);
        abort();
    }
    if (element_size != 16 && element_size != 24 && element_size != 32 && element_size != 48 && element_size != 88) {
        fprintf(stderr, "seen_arr_new_with_size_ptr: unexpected element_size=%ld\n", (long)element_size);
    }
    arr->data = malloc(8 * element_size);
    arr->len = 0;
    arr->cap = 8;
    arr->element_size = element_size;
    return arr;
}

void seen_arr_push_str(SeenArray* arr, SeenString s) {
    if (arr->element_size != (int64_t)sizeof(SeenString)) {
        fprintf(stderr, "seen_arr_push_str: element_size mismatch (%ld)\n", (long)arr->element_size);
        abort();
    }
    if (arr->len < 0 || arr->cap < 0 || arr->len > arr->cap) {
        fprintf(stderr, "seen_arr_push_str: invalid len/cap (len=%ld cap=%ld)\n", (long)arr->len, (long)arr->cap);
        abort();
    }
    if (arr->len >= arr->cap) {
        // Handle initial capacity of 0
        int64_t new_cap = (arr->cap == 0) ? 8 : arr->cap * 2;
        if (new_cap < arr->cap) {
            fprintf(stderr, "seen_arr_push_str: capacity overflow (cap=%ld)\n", (long)arr->cap);
            abort();
        }
        arr->cap = new_cap;
        arr->data = realloc(arr->data, arr->cap * sizeof(SeenString));
    }
    ((SeenString*)arr->data)[arr->len++] = s;
}

// Push i64 by value (not pointer) - for Array<Int>
void seen_arr_push_i64(SeenArray* arr, int64_t val) {
    if (arr->element_size != (int64_t)sizeof(int64_t)) {
        fprintf(stderr, "seen_arr_push_i64: element_size mismatch (%ld)\n", (long)arr->element_size);
        abort();
    }
    if (arr->len < 0 || arr->cap < 0 || arr->len > arr->cap) {
        fprintf(stderr, "seen_arr_push_i64: invalid len/cap (len=%ld cap=%ld)\n", (long)arr->len, (long)arr->cap);
        abort();
    }
    if (arr->len >= arr->cap) {
        int64_t new_cap = (arr->cap == 0) ? 8 : arr->cap * 2;
        if (new_cap < arr->cap) {
            fprintf(stderr, "seen_arr_push_i64: capacity overflow (cap=%ld)\n", (long)arr->cap);
            abort();
        }
        arr->cap = new_cap;
        arr->data = realloc(arr->data, arr->cap * sizeof(int64_t));
    }
    ((int64_t*)arr->data)[arr->len++] = val;
}

// Generic push for pointer types (e.g., Array<Token>)
void seen_arr_push_ptr(SeenArray* arr, void* p) {
    if (arr->element_size != (int64_t)sizeof(void*)) {
        fprintf(stderr, "seen_arr_push_ptr: element_size mismatch (%ld)\n", (long)arr->element_size);
        abort();
    }
    if (arr->len < 0 || arr->cap < 0 || arr->len > arr->cap) {
        fprintf(stderr, "seen_arr_push_ptr: invalid len/cap (len=%ld cap=%ld)\n", (long)arr->len, (long)arr->cap);
        abort();
    }
    if (arr->len >= arr->cap) {
        // Handle initial capacity of 0
        int64_t new_cap = (arr->cap == 0) ? 8 : arr->cap * 2;
        if (new_cap < arr->cap) {
            fprintf(stderr, "seen_arr_push_ptr: capacity overflow (cap=%ld)\n", (long)arr->cap);
            abort();
        }
        arr->cap = new_cap;
        arr->data = realloc(arr->data, arr->cap * sizeof(void*));
    }
    ((void**)arr->data)[arr->len++] = p;
}

// Generic push that copies element using element_size (for data types like ItemNode)
// This is the proper version for inline structs
int64_t Array_push(SeenArray* arr, void* element) {
    if (!arr) return 0;
    if (arr->element_size <= 0) {
        fprintf(stderr, "Array_push: invalid element_size=%ld\n", (long)arr->element_size);
        abort();
    }
    if (arr->len < 0 || arr->cap < 0 || arr->len > arr->cap) {
        fprintf(stderr, "Array_push: invalid len/cap (len=%ld cap=%ld)\n", (long)arr->len, (long)arr->cap);
        abort();
    }

    // Ensure capacity
    if (arr->len >= arr->cap) {
        int64_t new_cap = (arr->cap == 0) ? 8 : arr->cap * 2;
        if (new_cap < arr->cap) {
            fprintf(stderr, "Array_push: capacity overflow (cap=%ld)\n", (long)arr->cap);
            abort();
        }
        if ((uint64_t)new_cap > (uint64_t)SIZE_MAX / (uint64_t)arr->element_size) {
            fprintf(stderr, "Array_push: allocation overflow (cap=%ld elem=%ld)\n", (long)new_cap, (long)arr->element_size);
            abort();
        }
        size_t new_size = (size_t)new_cap * (size_t)arr->element_size;
        void* new_data = realloc(arr->data, new_size);
        if (!new_data && new_size != 0) {
            fprintf(stderr, "Array_push: realloc failed (bytes=%zu)\n", new_size);
            abort();
        }
        arr->data = new_data;
        arr->cap = new_cap;
    }

    // Copy element using element_size (correct for both data types and pointers)
    // For data types: copies full struct (e.g., 88 bytes for ItemNode)
    // For pointer arrays: element_size is 8, copies 8-byte pointer
    if ((uint64_t)arr->len > (uint64_t)SIZE_MAX / (uint64_t)arr->element_size) {
        fprintf(stderr, "Array_push: offset overflow (len=%ld elem=%ld)\n", (long)arr->len, (long)arr->element_size);
        abort();
    }
    void* dest = (char*)arr->data + (arr->len * arr->element_size);
    memcpy(dest, element, arr->element_size);
    arr->len++;

    return arr->len;  // Return new length
}

FrontendDiagnostic* seen_arr_get_diag(SeenArray a, int64_t idx) {
    if (idx < 0 || idx >= a.len) {
        static FrontendDiagnostic empty = { { 0, "" }, 0, 0, { 0, "" }, { 0, "" } };
        return &empty;
    }
    return ((FrontendDiagnostic**)a.data)[idx];
}

// Generic getter for pointer types (arrays of pointers - class types)
void* seen_arr_get_ptr(SeenArray a, int64_t idx) {
    if (a.element_size != (int64_t)sizeof(void*)) {
        fprintf(stderr, "seen_arr_get_ptr: element_size mismatch (%ld)\n", (long)a.element_size);
        abort();
    }
    if (idx < 0 || idx >= a.len) {
        return NULL;
    }
    return ((void**)a.data)[idx];
}

// Generic getter for inline elements (data types stored by value)
// Returns a pointer to the element at the given index, using element_size for offset
void* seen_arr_get_element(SeenArray a, int64_t idx) {
    if (a.element_size <= 0) {
        fprintf(stderr, "seen_arr_get_element: invalid element_size (%ld)\n", (long)a.element_size);
        abort();
    }
    if (idx < 0 || idx >= a.len) {
        return NULL;
    }
    // Calculate offset using element_size for inline structs
    return (char*)a.data + (idx * a.element_size);
}

int64_t seen_arr_length(SeenArray a) {
    return a.len;
}

int64_t seen_arr_length_ptr(SeenArray* a) {
    if (!a) return 0;
    return a->len;
}

// ============================================================================
// String Functions (implementations for LLVM backend linking)
// ============================================================================

SeenString seen_cstr_to_str(const char* s) {
    SeenString result = { strlen(s), (char*)s };
    return result;
}

int64_t seen_str_length(SeenString s) {
    return s.len;
}

int64_t seen_length(SeenString s) {
    return s.len;
}

SeenString seen_substring(SeenString s, int64_t start, int64_t end) {
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

SeenString seen_str_concat_ss(SeenString a, SeenString b) {
    char* newdata = (char*)malloc(a.len + b.len + 1);
    memcpy(newdata, a.data, a.len);
    memcpy(newdata + a.len, b.data, b.len);
    newdata[a.len + b.len] = 0;
    SeenString result = { a.len + b.len, newdata };
    return result;
}

SeenString seen_int_to_string(int64_t n) {
    char* buf = (char*)malloc(32);
    sprintf(buf, "%ld", n);
    SeenString result = { strlen(buf), buf };
    return result;
}

SeenString seen_bool_to_string(bool b) {
    if (b) {
        SeenString result = { 4, "true" };
        return result;
    } else {
        SeenString result = { 5, "false" };
        return result;
    }
}

SeenString seen_char_to_str(int64_t c) {
    // Convert a Unicode code point to a UTF-8 string
    char* buf = (char*)malloc(8);  // Max 4 bytes for UTF-8 + null
    int len = 0;
    if (c < 0x80) {
        buf[0] = (char)c;
        len = 1;
    } else if (c < 0x800) {
        buf[0] = (char)(0xC0 | (c >> 6));
        buf[1] = (char)(0x80 | (c & 0x3F));
        len = 2;
    } else if (c < 0x10000) {
        buf[0] = (char)(0xE0 | (c >> 12));
        buf[1] = (char)(0x80 | ((c >> 6) & 0x3F));
        buf[2] = (char)(0x80 | (c & 0x3F));
        len = 3;
    } else {
        buf[0] = (char)(0xF0 | (c >> 18));
        buf[1] = (char)(0x80 | ((c >> 12) & 0x3F));
        buf[2] = (char)(0x80 | ((c >> 6) & 0x3F));
        buf[3] = (char)(0x80 | (c & 0x3F));
        len = 4;
    }
    buf[len] = '\0';
    SeenString result = { len, buf };
    return result;
}

int64_t seen_char_at(SeenString s, int64_t index) {
    // Get character code point at index
    // For now, just return the byte value (ASCII-compatible)
    if (index < 0 || index >= s.len) {
        return 0;  // Out of bounds
    }
    return (int64_t)(unsigned char)s.data[index];
}

int64_t Char_toInt(int64_t c) {
    // Char is already stored as i64 code point, so this is identity
    return c;
}

int64_t Int_unwrap(int64_t val) {
    // Unwrap for Int - just identity since Int is non-optional
    return val;
}

void* Optional_unwrap(void* ptr) {
    // Unwrap for optional pointer types - return the pointer
    // In a full implementation, this would check for null and panic
    return ptr;
}

bool startsWith(SeenString text, SeenString prefix) {
    if (prefix.len > text.len) return false;
    return memcmp(text.data, prefix.data, prefix.len) == 0;
}

bool endsWith(SeenString text, SeenString suffix) {
    if (suffix.len > text.len) return false;
    return memcmp(text.data + text.len - suffix.len, suffix.data, suffix.len) == 0;
}

bool contains(SeenString text, SeenString needle) {
    if (needle.len == 0) return true;
    if (needle.len > text.len) return false;
    for (int64_t i = 0; i <= text.len - needle.len; i++) {
        if (memcmp(text.data + i, needle.data, needle.len) == 0) return true;
    }
    return false;
}

bool seen_str_eq_ss(SeenString a, SeenString b) {
    if (a.len != b.len) return false;
    if (a.len == 0) return true;
    return memcmp(a.data, b.data, a.len) == 0;
}

bool seen_str_ne_ss(SeenString a, SeenString b) {
    return !seen_str_eq_ss(a, b);
}

// ============================================================================
// Print Functions
// ============================================================================

void println_cstr(const char* s) {
    printf("%s\n", s);
}

void println_str(SeenString s) {
    printf("%.*s\n", (int)s.len, s.data);
}

void println(SeenString s) {
    printf("%.*s\n", (int)s.len, s.data);
    fflush(stdout);
}

// ============================================================================
// StringBuilder Implementation (LLVM pointer-based calling convention)
// ============================================================================

// Allocate a StringBuilder on the heap and return pointer
void* StringBuilder_new(void) {
    StringBuilder* sb = (StringBuilder*)malloc(sizeof(StringBuilder));
    *sb = StringBuilder_new_value();  // Call inline version
    return sb;
}

// Append text to StringBuilder, returns 0 for LLVM compatibility
int64_t StringBuilder_append(void* s, SeenString str) {
    StringBuilder* sb = (StringBuilder*)s;
    StringBuilder_append_value(sb, str);  // Call inline version
    return 0;
}

// Convert StringBuilder to string
SeenString StringBuilder_toString(void* s) {
    StringBuilder* sb = (StringBuilder*)s;
    return StringBuilder_toString_value(sb);  // Call inline version
}

// Get length of StringBuilder
int64_t StringBuilder_length(void* s) {
    StringBuilder* sb = (StringBuilder*)s;
    return sb->totalLength;
}

// Clear StringBuilder
void StringBuilder_clear_impl(void* s) {
    StringBuilder* sb = (StringBuilder*)s;
    StringBuilder_clear(sb);  // Call inline version
}

// ============================================================================
// Map (Hash Map) Implementation
// Simple linear-search map for small collections (like keyword maps)
// ============================================================================

#define MAP_INITIAL_CAPACITY 32

typedef struct {
    uint64_t magic;
    SeenString* keys;
    int64_t* values;
    int64_t size;
    int64_t capacity;
} SeenMap;

static void Map_check(SeenMap* map, const char* fn) {
    if (!map || map->magic != 0x5345454E4D4150ULL) {
        fprintf(stderr, "%s: invalid map pointer\n", fn);
        abort();
    }
    if (map->capacity <= 0 || map->size < 0 || map->size > map->capacity || !map->keys || !map->values) {
        fprintf(stderr, "%s: corrupt map state (size=%ld cap=%ld)\n", fn, (long)map->size, (long)map->capacity);
        abort();
    }
}

void* Map_new(void) {
    SeenMap* map = (SeenMap*)malloc(sizeof(SeenMap));
    map->magic = 0x5345454E4D4150ULL;
    map->capacity = MAP_INITIAL_CAPACITY;
    map->size = 0;
    map->keys = (SeenString*)malloc(sizeof(SeenString) * map->capacity);
    map->values = (int64_t*)malloc(sizeof(int64_t) * map->capacity);
    return map;
}

static void Map_grow(SeenMap* map) {
    Map_check(map, "Map_grow");
    int64_t new_capacity = map->capacity * 2;
    SeenString* new_keys = (SeenString*)malloc(sizeof(SeenString) * new_capacity);
    int64_t* new_values = (int64_t*)malloc(sizeof(int64_t) * new_capacity);

    for (int64_t i = 0; i < map->size; i++) {
        new_keys[i] = map->keys[i];
        new_values[i] = map->values[i];
    }

    free(map->keys);
    free(map->values);
    map->keys = new_keys;
    map->values = new_values;
    map->capacity = new_capacity;
}

int64_t Map_put(void* m, SeenString key, int64_t value) {
    SeenMap* map = (SeenMap*)m;
    Map_check(map, "Map_put");

    // Check if key already exists
    for (int64_t i = 0; i < map->size; i++) {
        if (map->keys[i].len == key.len &&
            memcmp(map->keys[i].data, key.data, key.len) == 0) {
            int64_t old_value = map->values[i];
            map->values[i] = value;
            return old_value;
        }
    }

    // Key not found, add new entry
    if (map->size >= map->capacity) {
        Map_grow(map);
    }

    // Copy the key string data
    char* key_copy = (char*)malloc(key.len + 1);
    memcpy(key_copy, key.data, key.len);
    key_copy[key.len] = 0;

    map->keys[map->size].len = key.len;
    map->keys[map->size].data = key_copy;
    map->values[map->size] = value;
    map->size++;

    return 0;  // No previous value
}

int64_t Map_set(void* m, SeenString key, int64_t value) {
    return Map_put(m, key, value);
}

int64_t Map_get(void* m, SeenString key) {
    SeenMap* map = (SeenMap*)m;
    Map_check(map, "Map_get");

    for (int64_t i = 0; i < map->size; i++) {
        if (map->keys[i].len == key.len &&
            memcmp(map->keys[i].data, key.data, key.len) == 0) {
            return map->values[i];
        }
    }

    return 0;  // Not found, return 0 (or could use sentinel)
}

int64_t Map_size(void* m) {
    SeenMap* map = (SeenMap*)m;
    Map_check(map, "Map_size");
    return map->size;
}

bool Map_containsKey(void* m, SeenString key) {
    SeenMap* map = (SeenMap*)m;
    Map_check(map, "Map_containsKey");

    for (int64_t i = 0; i < map->size; i++) {
        if (map->keys[i].len == key.len &&
            memcmp(map->keys[i].data, key.data, key.len) == 0) {
            return true;
        }
    }

    return false;
}

bool Map_containsValue(void* m, int64_t value) {
    SeenMap* map = (SeenMap*)m;
    Map_check(map, "Map_containsValue");

    for (int64_t i = 0; i < map->size; i++) {
        if (map->values[i] == value) {
            return true;
        }
    }

    return false;
}

SeenArray Map_keys(void* m) {
    SeenMap* map = (SeenMap*)m;
    Map_check(map, "Map_keys");

    SeenArray result;
    result.len = map->size;
    result.cap = map->size;
    result.element_size = sizeof(SeenString);
    result.data = malloc(sizeof(SeenString) * map->size);

    for (int64_t i = 0; i < map->size; i++) {
        ((SeenString*)result.data)[i] = map->keys[i];
    }

    return result;
}

SeenArray Map_values(void* m) {
    SeenMap* map = (SeenMap*)m;
    Map_check(map, "Map_values");

    SeenArray result;
    result.len = map->size;
    result.cap = map->size;
    result.element_size = sizeof(int64_t);
    result.data = malloc(sizeof(int64_t) * map->size);

    for (int64_t i = 0; i < map->size; i++) {
        ((int64_t*)result.data)[i] = map->values[i];
    }

    return result;
}

// ============================================================================
// Bootstrap Stub Functions
// These are placeholders for functionality not yet fully compiled
// The actual implementations are now compiled from Seen source code
// ============================================================================

// C Generator stubs (unused for LLVM backend but kept for future)
void* CGenerator_new(void) {
    printf("ERROR: CGenerator_new stub called - C backend not implemented\n");
    return NULL;
}

SeenString CGenerator_generate(void* gen, void* program) {
    printf("ERROR: CGenerator_generate stub called - C backend not implemented\n");
    SeenString empty = { 0, "" };
    return empty;
}

// ============================================================================
// Result/Option Type Stubs
// ============================================================================

void* Ok(void* value) { return value; }
void* Err(SeenString message) { return NULL; }
void* Some(void* value) { return value; }
void* None(void) { return NULL; }
bool Result_isOkay(void* result) { return result != NULL; }
SeenString Result_unwrapErr(void* result) { return (SeenString){ 0, "" }; }
// FsFileResult = { i1 isOk (padded to 8 bytes), ptr file, SeenString error }
// file is at offset 8, error is at offset 16
void* FsFileResult_unwrap(void* result) {
    return *((void**)((char*)result + 8));
}
SeenString FsFileResult_unwrapErr(void* result) {
    return *((SeenString*)((char*)result + 16));
}

// SeenTokenType is actually i64 (enum), unwrap just returns the value
int64_t SeenTokenType_unwrap(int64_t value) { return value; }

// ============================================================================
// Typechecker Type Stubs
// ============================================================================

void* TypeError(void) { return malloc(64); }
void* FunctionType(void) { return malloc(64); }
void* ClassType(void) { return malloc(64); }
void* InterfaceType(void) { return malloc(64); }
void* Location(void) { return malloc(32); }

void* TypeError_getLocation(void* e) { return malloc(32); }
void* TypeError_getExpected(void* e) { return malloc(32); }
void* TypeError_getActual(void* e) { return malloc(32); }
SeenString TypeError_getContext(void* e) { return (SeenString){ 0, "" }; }
SeenString TypeError_getMessage(void* e) { return (SeenString){ 5, "error" }; }

// ============================================================================
// Parser Node Constructor Stubs
// ============================================================================

void* TypeNode_new(void) {
    // Simple struct with name field
    void* node = malloc(64);
    memset(node, 0, 64);
    return node;
}

void* ItemNode_new(void) {
    void* node = malloc(256);
    memset(node, 0, 256);
    return node;
}

void* ParamNode_new(void) {
    void* node = malloc(128);
    memset(node, 0, 128);
    return node;
}

void* ImportSymbolNode_new(void) {
    void* node = malloc(64);
    memset(node, 0, 64);
    return node;
}

// ============================================================================
// String/StringBuilder Stubs
// ============================================================================

int64_t indexOf(SeenString text, SeenString needle, int64_t start) {
    if (needle.len == 0) return start;
    if (text.len < needle.len + start) return -1;

    for (int64_t i = start; i <= text.len - needle.len; i++) {
        bool found = true;
        for (int64_t j = 0; j < needle.len; j++) {
            if (text.data[i + j] != needle.data[j]) {
                found = false;
                break;
            }
        }
        if (found) return i;
    }
    return -1;
}

int64_t StringBuilder_appendChar(void* sb, int64_t ch) {
    StringBuilder* s = (StringBuilder*)sb;
    SeenString charStr = seen_char_to_str(ch);
    StringBuilder_append_value(s, charStr);
    return 0;
}

// ============================================================================
// Main wrapper
// ============================================================================

// The actual main function should be defined in user code as seen_main
// This wrapper initializes the runtime
#ifdef SEEN_RUNTIME_MAIN
extern int seen_main(void);

int main(int argc, char** argv) {
    seen_runtime_init(argc, argv);
    return seen_main();
}
#endif
