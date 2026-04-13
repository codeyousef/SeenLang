// Seen Runtime Library Implementation
// Provides C implementations of Seen standard library functions

#ifdef _WIN32
// ============================================================================
// Windows (mingw-w64) build — pull in compatibility layer
// ============================================================================
#include "seen_runtime.h"
#include "seen_compat_win32.h"
#include <errno.h>
#include <sys/stat.h>
#include <math.h>
#include <time.h>

// On Windows, seen_aligned_realloc must use _aligned_realloc/_aligned_free
// instead of allocating new + memcpy + free (which would crash with _aligned_malloc memory).
#define SEEN_WIN32_ALIGNED 1

#else // !_WIN32
// ============================================================================
// POSIX build (Linux, macOS, etc.)
// ============================================================================
#define _GNU_SOURCE
#define _POSIX_C_SOURCE 200809L

#include "seen_runtime.h"
#include <errno.h>
#include <sys/wait.h>
#include <sys/stat.h>
#include <unistd.h>
#if !defined(__ANDROID__)
#include <execinfo.h>
#endif
#include <math.h>
#include <time.h>
#include <sched.h>
#include <pthread.h>
#include <spawn.h>

// Apple platform headers
#if defined(__APPLE__)
#include <TargetConditionals.h>
#endif

// Apple/Android aligned_alloc compatibility shim.
// Some POSIX toolchains either lack aligned_alloc or require stricter C11
// feature availability than the runtime build currently uses.
#if defined(__APPLE__) || defined(__ANDROID__)
#include <stdlib.h>
static inline void* seen_aligned_alloc(size_t alignment, size_t size) {
    void* ptr = NULL;
    // Round size up to a multiple of alignment
    size_t aligned_size = ((size + alignment - 1) / alignment) * alignment;
    if (aligned_size == 0) aligned_size = alignment;
    if (posix_memalign(&ptr, alignment, aligned_size) != 0)
        return NULL;
    return ptr;
}
#define aligned_alloc(a, s) seen_aligned_alloc(a, s)
#endif

// On POSIX, aligned memory can be freed with regular free()
#define seen_aligned_free(p) free(p)

#endif // _WIN32

#include <ctype.h>

// ============================================================================
// Pool Allocator - size-class slab allocator with free-list recycling
// ============================================================================

#define POOL_NUM_CLASSES 10      // size classes: 8,16,24,...,80
#define POOL_SLAB_SIZE (1 << 20) // 1MB slabs
#define POOL_MAX_SIZE (POOL_NUM_CLASSES * 8) // 80 bytes max

typedef struct PoolSlab {
    char *base;
    size_t used;
    struct PoolSlab *next;
} PoolSlab;

static void *pool_freelists[POOL_NUM_CLASSES]; // per-class free lists
static PoolSlab *pool_slabs[POOL_NUM_CLASSES]; // per-class slab chains

static inline int pool_class(size_t size) {
    return (int)((size + 7) / 8) - 1; // 8->0, 16->1, ..., 80->9
}

__attribute__((hot, malloc))
void *seen_pool_alloc(int64_t size) {
    if (__builtin_expect(size <= 0 || size > POOL_MAX_SIZE, 0))
        return malloc(size > 0 ? size : 1);
    int cls = pool_class((size_t)size);
    size_t slot = (size_t)(cls + 1) * 8;

    // Fast path: pop free list
    void *p = pool_freelists[cls];
    if (__builtin_expect(p != NULL, 1)) {
        void *next = *(void **)p;
        pool_freelists[cls] = next;
        // Prefetch next free-list entry for the NEXT allocation of this size class
        __builtin_prefetch(next, 1, 3);
        return p;
    }

    // Slow path: bump-allocate from slab
    PoolSlab *slab = pool_slabs[cls];
    if (__builtin_expect(!slab || slab->used + slot > POOL_SLAB_SIZE, 0)) {
        slab = (PoolSlab *)malloc(sizeof(PoolSlab));
        slab->base = (char *)malloc(POOL_SLAB_SIZE);
        slab->used = 0;
        slab->next = pool_slabs[cls];
        pool_slabs[cls] = slab;
    }
    p = slab->base + slab->used;
    slab->used += slot;
    return p;
}

__attribute__((hot))
void seen_pool_free(void *ptr, int64_t size) {
    if (__builtin_expect(!ptr || size <= 0 || size > POOL_MAX_SIZE, 0)) {
        free(ptr); return;
    }
    int cls = pool_class((size_t)size);
    *(void **)ptr = pool_freelists[cls];
    pool_freelists[cls] = ptr;
}

// ============================================================================
// Global State
// ============================================================================

static int g_argc = 0;
static char** g_argv = NULL;

// Debug counters (guarded behind SEEN_RUNTIME_DEBUG env var)
#ifdef SEEN_RUNTIME_DEBUG_COUNTERS
static long g_substring_count = 0;
static long g_concat_count = 0;
static long g_length_count = 0;
static long g_contains_count = 0;

void seen_debug_print_counters(void) {
    fprintf(stderr, "[RUNTIME DEBUG] substring=%ld concat=%ld length=%ld contains=%ld\n",
            g_substring_count, g_concat_count, g_length_count, g_contains_count);
}
#else
void seen_debug_print_counters(void) {}
#endif

void seen_runtime_init(int argc, char** argv) {
    g_argc = argc;
    g_argv = argv;
}

// ============================================================================
// Panic Functions (for --panic-on-overflow and other runtime checks)
// ============================================================================

// Called when integer overflow is detected (with --panic-on-overflow flag)
void seen_panic_overflow(const char* op, int64_t left, int64_t right) {
    fprintf(stderr, "PANIC: Integer overflow in %s: %lld %s %lld\n",
            op, (long long)left, op, (long long)right);
    abort();
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

#ifdef _WIN32
    int result = _mkdir(cpath);
#else
    int result = mkdir(cpath, 0755);
#endif
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

// Execute program by path and return exit code.
#ifdef _WIN32
int64_t __ExecuteProgram(SeenString path) {
    char* cpath = (char*)malloc(path.len + 1);
    memcpy(cpath, path.data, path.len);
    cpath[path.len] = 0;

    int status = system(cpath);
    free(cpath);
    return (int64_t)status;
}
#else
// Uses posix_spawn via /bin/sh -c (like system(3) but available on all Apple platforms).
extern char **environ;
int64_t __ExecuteProgram(SeenString path) {
    char* cpath = (char*)malloc(path.len + 1);
    memcpy(cpath, path.data, path.len);
    cpath[path.len] = 0;

#if defined(__ANDROID__)
    int status = system(cpath);
    free(cpath);
    return (int64_t)status;
#else

    pid_t pid;
    char *argv[] = {"/bin/sh", "-c", cpath, NULL};
    int err = posix_spawn(&pid, "/bin/sh", NULL, NULL, argv, environ);
    if (err != 0) {
        free(cpath);
        return 127;
    }

    int status = 0;
    waitpid(pid, &status, 0);
    free(cpath);

    if (WIFEXITED(status)) return (int64_t)WEXITSTATUS(status);
    if (WIFSIGNALED(status)) return (int64_t)(128 + WTERMSIG(status));
    return -1;
#endif
}
#endif

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
#ifdef _WIN32
    // On Windows, pclose/_pclose returns the exit code directly
    result->success = (status == 0);
#else
    result->success = (status != -1 && WIFEXITED(status) && WEXITSTATUS(status) == 0);
#endif
    result->output.len = length;
    result->output.data = output;

    return result;
}

// ============================================================================
// Process Primitives (fork/waitpid/exit/getpid)
// ============================================================================

// Fork the current process. Returns child PID in parent, 0 in child, -1 on error.
// Flushes all stdio buffers before forking to avoid duplicate output.
// Uses __ prefix (like all other runtime functions) to avoid linker conflicts with
// Seen-generated wrapper symbols.
// On iOS, fork() returns -1 with errno=ENOSYS at runtime — callers handle this.
// On Windows, fork() is not available — always returns -1.
int64_t __seen_fork(void) {
#ifdef _WIN32
    return -1;
#else
    fflush(NULL);
    return (int64_t)fork();
#endif
}

// Wait for a child process to exit. Returns the child's exit code, or -1 on error.
// flags=0 for blocking wait; flags=1 for WNOHANG (non-blocking).
int64_t __seen_waitpid(int64_t pid, int64_t flags) {
#ifdef _WIN32
    // fork() always returns -1 on Windows, so waitpid is a no-op
    (void)pid; (void)flags;
    return -1;
#else
    int status = 0;
    pid_t result = waitpid((pid_t)pid, &status, (int)flags);
    if (result < 0) return -1;
    if (WIFEXITED(status)) return (int64_t)WEXITSTATUS(status);
    if (WIFSIGNALED(status)) return (int64_t)(128 + WTERMSIG(status));
    return -1;
#endif
}

// Terminate the current process immediately (bypasses C atexit handlers).
// Safe to call in child processes spawned by __seen_fork().
void __seen_exit(int64_t code) {
    _exit((int)code);
}

// Return the PID of the current process.
int64_t __seen_getpid(void) {
#ifdef _WIN32
    return (int64_t)GetCurrentProcessId();
#else
    return (int64_t)getpid();
#endif
}

// ============================================================================
// Environment Primitive Functions (used by Seen stdlib env.env)
// ============================================================================

// Get command line arguments - returns heap-allocated array (matches LLVM ABI expectation)
SeenArray* __GetCommandLineArgs(void) {
    SeenArray* arr = seen_arr_new_str_ptr();
    for (int i = 0; i < g_argc; i++) {
        SeenString arg = seen_str_copy(g_argv[i]);
        seen_arr_push_str(arr, arg);
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

#ifdef _WIN32
    int result = _putenv_s(cname, cvalue);
#else
    int result = setenv(cname, cvalue, 1);
#endif
    free(cname);
    free(cvalue);
    return result == 0;
}

// Remove environment variable
bool __RemoveEnv(SeenString name) {
    char* cname = (char*)malloc(name.len + 1);
    memcpy(cname, name.data, name.len);
    cname[name.len] = 0;

#ifdef _WIN32
    // On Windows, _putenv with "NAME=" removes the variable
    char* buf = (char*)malloc(name.len + 2);
    memcpy(buf, cname, name.len);
    buf[name.len] = '=';
    buf[name.len + 1] = 0;
    int result = _putenv(buf);
    free(buf);
#else
    int result = unsetenv(cname);
#endif
    free(cname);
    return result == 0;
}

// ============================================================================
// String Utility Functions
// ============================================================================

static inline int64_t seen_utf8_char_len(unsigned char c) {
    if ((c & 0x80) == 0) {
        return 1;
    }
    if ((c & 0xE0) == 0xC0) {
        return 2;
    }
    if ((c & 0xF0) == 0xE0) {
        return 3;
    }
    if ((c & 0xF8) == 0xF0) {
        return 4;
    }
    return 1;
}

static int64_t seen_utf8_offset_for_index(SeenString s, int64_t index) {
    if (index <= 0) {
        return 0;
    }

    const unsigned char* data = (const unsigned char*)s.data;
    int64_t byte_idx = 0;
    int64_t codepoint_idx = 0;

    while (byte_idx < s.len && codepoint_idx < index) {
        byte_idx += seen_utf8_char_len(data[byte_idx]);
        if (byte_idx > s.len) {
            byte_idx = s.len;
        }
        codepoint_idx++;
    }

    return byte_idx;
}

static int64_t seen_utf8_codepoint_count(SeenString s) {
    const unsigned char* data = (const unsigned char*)s.data;
    int64_t codepoint_count = 0;
    int64_t byte_idx = 0;

    while (byte_idx < s.len) {
        byte_idx += seen_utf8_char_len(data[byte_idx]);
        if (byte_idx > s.len) {
            byte_idx = s.len;
        }
        codepoint_count++;
    }

    return codepoint_count;
}

SeenArray split(SeenString text, SeenString delimiter) {
    SeenArray result = seen_arr_new_str();

    if (delimiter.len == 0) {
        // Split into characters
        int64_t byte_idx = 0;
        while (byte_idx < text.len) {
            int64_t next_byte_idx = byte_idx + seen_utf8_char_len((unsigned char)text.data[byte_idx]);
            if (next_byte_idx > text.len) {
                next_byte_idx = text.len;
            }
            int64_t ch_len = next_byte_idx - byte_idx;
            char* ch = (char*)malloc(ch_len + 1);
            memcpy(ch, text.data + byte_idx, ch_len);
            ch[ch_len] = 0;
            SeenString s = { ch_len, ch };
            seen_arr_push_str(&result, s);
            byte_idx = next_byte_idx;
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

__attribute__((weak)) SeenString trim(SeenString text) {
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

static long g_arr_get_str_count = 0;
SeenString seen_arr_get_str(SeenArray a, int64_t idx) {
    g_arr_get_str_count++;
    // Note: codegen may set element_size=8 for String arrays; this is harmless
    // since this function uses sizeof(SeenString) for indexing regardless
    if (idx < 0 || idx >= a.len) {
        SeenString empty = { 0, "" };
        return empty;
    }
    SeenString result = ((SeenString*)a.data)[idx];
    // Validate the result string
    if (result.len < 0 || result.len > 100000000) {
        fprintf(stderr, "seen_arr_get_str: result has invalid len=%ld at count=%ld idx=%ld\n", (long)result.len, g_arr_get_str_count, (long)idx);
        fprintf(stderr, "  arr.len=%ld arr.cap=%ld arr.element_size=%ld arr.data=%p\n", (long)a.len, (long)a.cap, (long)a.element_size, a.data);
        fprintf(stderr, "  result.data=%p\n", (void*)result.data);
        // Dump raw bytes to see what's actually stored
        fprintf(stderr, "  Raw bytes at idx 0: ");
        for (int i = 0; i < 24 && i < a.element_size; i++) {
            fprintf(stderr, "%02x ", ((unsigned char*)a.data)[i]);
        }
        fprintf(stderr, "\n");
        abort();
    }
    if (result.len > 0 && result.data == NULL) {
        fprintf(stderr, "seen_arr_get_str: result has NULL data with len=%ld at count=%ld idx=%ld\n", (long)result.len, g_arr_get_str_count, (long)idx);
        abort();
    }
    return result;
}

// Aligned allocation helpers for SIMD-friendly array data
static inline size_t seen_align_up(size_t n, size_t align) {
    return (n + align - 1) & ~(align - 1);
}

static void* seen_aligned_realloc(void* old, size_t old_size, size_t new_size) {
    size_t aligned_size = seen_align_up(new_size, 32);
    if (aligned_size == 0) aligned_size = 32;
#ifdef SEEN_WIN32_ALIGNED
    // On Windows, _aligned_malloc memory must be reallocated with _aligned_realloc
    (void)old_size; // _aligned_realloc doesn't need old_size
    void* p = _aligned_realloc(old, aligned_size, 32);
    if (!p) { fprintf(stderr, "seen_aligned_realloc: OOM\n"); abort(); }
    return p;
#else
    void* p = aligned_alloc(32, aligned_size);
    if (!p) { fprintf(stderr, "seen_aligned_realloc: OOM\n"); abort(); }
    if (old) { memcpy(p, old, old_size < new_size ? old_size : new_size); free(old); }
    return p;
#endif
}

SeenArray seen_arr_new_str(void) {
    size_t sz = 8 * sizeof(SeenString);
    void* data = aligned_alloc(32, seen_align_up(sz, 32));
    // element_size = sizeof(SeenString) = 16 bytes
    SeenArray arr = { 0, 8, sizeof(SeenString), data };
    return arr;
}

SeenArray seen_arr_new_ptr(void) {
    size_t sz = 8 * sizeof(void*);
    void* data = aligned_alloc(32, seen_align_up(sz, 32));
    SeenArray arr = { 0, 8, sizeof(void*), data };
    return arr;
}

// Heap-allocated versions that return ptr for LLVM ABI compatibility
SeenArray* seen_arr_new_str_ptr(void) {
    SeenArray* arr = (SeenArray*)malloc(sizeof(SeenArray));
    size_t sz = 8 * sizeof(SeenString);
    arr->data = aligned_alloc(32, seen_align_up(sz, 32));
    arr->len = 0;
    arr->cap = 8;
    arr->element_size = sizeof(SeenString);
    return arr;
}

SeenArray* seen_arr_new_ptr_ptr(void) {
    SeenArray* arr = (SeenArray*)malloc(sizeof(SeenArray));
    size_t sz = 8 * sizeof(void*);
    arr->data = aligned_alloc(32, seen_align_up(sz, 32));
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
    size_t sz = 8 * (size_t)element_size;
    arr->data = aligned_alloc(32, seen_align_up(sz, 32));
    arr->len = 0;
    arr->cap = 8;
    arr->element_size = element_size;
    return arr;
}

void seen_arr_push_str(SeenArray* arr, SeenString s) {
    // Validate input string
    if (s.len < 0 || s.len > 100000000) {
        fprintf(stderr, "seen_arr_push_str: invalid input string len=%ld\n", (long)s.len);
        abort();
    }
    if (s.len > 0 && s.data == NULL) {
        fprintf(stderr, "seen_arr_push_str: NULL data with len=%ld\n", (long)s.len);
        abort();
    }
    // Auto-correct element_size: codegen may set 8 (ptr size) for String arrays
    if (arr->element_size != (int64_t)sizeof(SeenString)) {
        arr->element_size = sizeof(SeenString);
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
        size_t old_sz = (size_t)arr->cap * sizeof(SeenString);
        arr->cap = new_cap;
        arr->data = seen_aligned_realloc(arr->data, old_sz, (size_t)arr->cap * sizeof(SeenString));
    }
    ((SeenString*)arr->data)[arr->len++] = s;
}

// Push i64 by value (not pointer) - for Array<Int>
void seen_arr_push_i64(SeenArray* arr, int64_t val) {
    if (arr->element_size < (int64_t)sizeof(int64_t)) {
        fprintf(stderr, "seen_arr_push_i64: element_size too small (%ld)\n", (long)arr->element_size);
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
        size_t old_sz = (size_t)arr->cap * (size_t)arr->element_size;
        arr->cap = new_cap;
        arr->data = seen_aligned_realloc(arr->data, old_sz, (size_t)arr->cap * (size_t)arr->element_size);
    }
    if (arr->element_size == (int64_t)sizeof(int64_t)) {
        ((int64_t*)arr->data)[arr->len++] = val;
    } else {
        // Multi-byte slot: val is a pointer to an inline struct (data type handle).
        // Copy the struct data from the heap object into the array slot so that
        // GEP-based field access on the slot works correctly.
        char* slot = (char*)arr->data + arr->len * arr->element_size;
        if (val != 0) {
            memcpy(slot, (void*)(intptr_t)val, (size_t)arr->element_size);
        } else {
            memset(slot, 0, (size_t)arr->element_size);
        }
        arr->len++;
    }
}

void seen_arr_push_f64(SeenArray* arr, double val) {
    if (!arr) {
        fprintf(stderr, "seen_arr_push_f64: NULL array\n");
        abort();
    }
    if (arr->element_size < (int64_t)sizeof(double)) {
        fprintf(stderr, "seen_arr_push_f64: element_size too small (%ld)\n", (long)arr->element_size);
        abort();
    }
    if (arr->len < 0 || arr->cap < 0 || arr->len > arr->cap) {
        fprintf(stderr, "seen_arr_push_f64: invalid len/cap (len=%ld cap=%ld)\n", (long)arr->len, (long)arr->cap);
        abort();
    }
    if (arr->len >= arr->cap) {
        size_t old_sz = (size_t)arr->cap * (size_t)arr->element_size;
        int64_t new_cap = (arr->cap == 0) ? 8 : arr->cap * 2;
        arr->cap = new_cap;
        arr->data = seen_aligned_realloc(arr->data, old_sz, (size_t)arr->cap * (size_t)arr->element_size);
    }
    ((double*)arr->data)[arr->len++] = val;
}

int64_t seen_arr_pop_i64(SeenArray* arr) {
    if (!arr || arr->len == 0) return 0;
    arr->len--;
    int64_t* data = (int64_t*)arr->data;
    return data[arr->len];
}

double seen_arr_pop_f64(SeenArray* arr) {
    if (!arr || arr->len == 0) return 0.0;
    arr->len--;
    double* data = (double*)arr->data;
    return data[arr->len];
}

SeenString seen_arr_pop_str(SeenArray* arr) {
    if (!arr || arr->len == 0) { SeenString empty = {0, NULL}; return empty; }
    arr->len--;
    SeenString* data = (SeenString*)arr->data;
    return data[arr->len];
}

// Clear array (reset length to 0 without freeing buffer — reuse allocation)
void seen_arr_clear(SeenArray* arr) {
    if (arr) arr->len = 0;
}

// Wrapper for malloc with tracking
void* tracked_malloc(size_t size) {
    void* ptr = malloc(size);
    if (!ptr && size != 0) {
        fprintf(stderr, "tracked_malloc: malloc(%zu) failed\n", size);
        abort();
    }
    return ptr;
}

// Generic push for pointer types (e.g., Array<Token>)
void seen_arr_push_ptr(SeenArray* arr, void* p) {
    if (!arr) {
        fprintf(stderr, "seen_arr_push_ptr: NULL array\n");
        abort();
    }
    if (arr->element_size < (int64_t)sizeof(void*)) {
        fprintf(stderr, "seen_arr_push_ptr: element_size too small (%ld)\n", (long)arr->element_size);
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
        size_t old_sz = (size_t)arr->cap * (size_t)arr->element_size;
        arr->cap = new_cap;
        arr->data = seen_aligned_realloc(arr->data, old_sz, (size_t)arr->cap * (size_t)arr->element_size);
    }
    if (arr->element_size == (int64_t)sizeof(void*)) {
        ((void**)arr->data)[arr->len++] = p;
    } else {
        // Multi-byte slot: store ptr at beginning, zero rest (bootstrap compatibility)
        char* slot = (char*)arr->data + arr->len * arr->element_size;
        *(void**)slot = p;
        memset(slot + sizeof(void*), 0, (size_t)(arr->element_size - sizeof(void*)));
        arr->len++;
    }
}

// Generic push that copies element using element_size (for data types like ItemNode)
// This is the proper version for inline structs
int64_t Array_push(SeenArray* arr, void* element) {
    if (!arr) return 0;
    if (arr->element_size <= 0 || arr->element_size > 4096) {
        fprintf(stderr, "Array_push: invalid element_size=%ld\n", (long)arr->element_size);
        abort();
    }
    if (arr->len < 0 || arr->cap < 0 || arr->len > arr->cap) {
        fprintf(stderr, "Array_push: invalid len/cap (len=%ld cap=%ld)\n", (long)arr->len, (long)arr->cap);
        abort();
    }
    // Sanity check: element should not be NULL for data types
    if (!element) {
        fprintf(stderr, "Array_push: NULL element\n");
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
        size_t old_size = (size_t)arr->cap * (size_t)arr->element_size;
        size_t new_size = (size_t)new_cap * (size_t)arr->element_size;
        arr->data = seen_aligned_realloc(arr->data, old_size, new_size);
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

// Set element at index (copies element_size bytes)
void Array_set(SeenArray* arr, int64_t index, void* element) {
    if (!arr || !element) {
        fprintf(stderr, "Array_set: invalid args (arr=%p element=%p)\n", (void*)arr, element);
        abort();
    }
    if (arr->element_size <= 0) {
        fprintf(stderr, "Array_set: invalid element_size=%ld\n", (long)arr->element_size);
        abort();
    }
    if (index < 0) {
        fprintf(stderr, "Array_set: index out of bounds (idx=%ld len=%ld)\n", (long)index, (long)arr->len);
        abort();
    }
    if (index == arr->len) {
        (void)Array_push(arr, element);
        return;
    }
    if (index > arr->len) {
        fprintf(stderr, "Array_set: index out of bounds (idx=%ld len=%ld)\n", (long)index, (long)arr->len);
        abort();
    }
    if ((uint64_t)index > (uint64_t)SIZE_MAX / (uint64_t)arr->element_size) {
        fprintf(stderr, "Array_set: offset overflow (idx=%ld elem=%ld)\n", (long)index, (long)arr->element_size);
        abort();
    }
    void* dest = (char*)arr->data + (index * arr->element_size);
    memcpy(dest, element, arr->element_size);
}

// Direct i64 set - avoids alloca in caller (prevents stack overflow in tight loops)
void seen_arr_set_i64(SeenArray* arr, int64_t index, int64_t val) {
    if (!arr || index < 0 || index >= arr->len) return;
    if (arr->element_size == (int64_t)sizeof(int64_t)) {
        ((int64_t*)arr->data)[index] = val;
    } else {
        // Multi-byte slot: val is a pointer to struct data, copy into the slot
        char* slot = (char*)arr->data + index * arr->element_size;
        if (val != 0) {
            memcpy(slot, (void*)(intptr_t)val, (size_t)arr->element_size);
        } else {
            memset(slot, 0, (size_t)arr->element_size);
        }
    }
}

// Bulk array initialization - creates array pre-filled with a value
SeenArray* seen_arr_new_filled_i64(int64_t count, int64_t value) {
    SeenArray* arr = (SeenArray*)malloc(sizeof(SeenArray));
    arr->len = count;
    arr->cap = count;
    arr->element_size = sizeof(int64_t);
    size_t sz = (size_t)count * sizeof(int64_t);
    arr->data = aligned_alloc(32, seen_align_up(sz ? sz : 32, 32));
    int64_t* data = (int64_t*)arr->data;
    for (int64_t i = 0; i < count; i++) data[i] = value;
    return arr;
}

SeenArray* seen_arr_new_filled_double(int64_t count, double value) {
    SeenArray* arr = (SeenArray*)malloc(sizeof(SeenArray));
    arr->len = count;
    arr->cap = count;
    arr->element_size = sizeof(double);
    size_t sz = (size_t)count * sizeof(double);
    arr->data = aligned_alloc(32, seen_align_up(sz ? sz : 32, 32));
    double* data = (double*)arr->data;
    for (int64_t i = 0; i < count; i++) data[i] = value;
    return arr;
}

// Direct double set - avoids alloca in caller
void seen_arr_set_double(SeenArray* arr, int64_t index, double val) {
    if (!arr || index < 0 || index >= arr->len) return;
    ((double*)arr->data)[index] = val;
}

void seen_arr_set_str(SeenArray* arr, int64_t index, SeenString s) {
    if (!arr || index < 0 || index >= arr->len) return;
    if (arr->element_size != (int64_t)sizeof(SeenString)) {
        arr->element_size = sizeof(SeenString);
    }
    ((SeenString*)arr->data)[index] = s;
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

// Get i64 value from array at index (for @derive(Deserialize))
// Used to deserialize integer/pointer fields from a byte array
int64_t seen_arr_get_i64(SeenArray* arr, int64_t idx) {
    if (!arr) return 0;
    if (idx < 0 || idx >= arr->len) return 0;
    // Assume elements are stored as i64 (8 bytes each)
    if (arr->element_size == sizeof(int64_t)) {
        return ((int64_t*)arr->data)[idx];
    }
    // Fallback: read 8 bytes starting at element offset
    return ((int64_t*)((char*)arr->data + idx * arr->element_size))[0];
}

int64_t seen_arr_length(SeenArray a) {
    return a.len;
}

int64_t seen_arr_length_ptr(SeenArray* a) {
    if (!a) return 0;
    return a->len;
}

int64_t seen_arr_data_ptr(SeenArray* arr) {
    if (!arr) return 0;
    return (int64_t)(arr->data);
}

// Pointer-based array accessor variants (avoid struct-by-value ABI issues)
SeenString seen_arr_get_str_ptr(SeenArray* a, int64_t idx) {
    static SeenString empty = { 0, "" };
    if (!a) return empty;
    // Note: codegen may set element_size=8 for String arrays; harmless
    if (idx < 0 || idx >= a->len) {
        return empty;
    }
    return ((SeenString*)a->data)[idx];
}

FrontendDiagnostic* seen_arr_get_diag_ptr(SeenArray* a, int64_t idx) {
    static FrontendDiagnostic empty = { { 0, "" }, 0, 0, { 0, "" }, { 0, "" } };
    if (!a) return &empty;
    if (idx < 0 || idx >= a->len) {
        return &empty;
    }
    return ((FrontendDiagnostic**)a->data)[idx];
}

void* seen_arr_get_ptr_ptr(SeenArray* a, int64_t idx) {
    if (!a) return NULL;
    if (a->element_size != (int64_t)sizeof(void*)) {
        fprintf(stderr, "seen_arr_get_ptr_ptr: element_size mismatch (%ld) at idx=%ld, array len=%ld cap=%ld\n",
                (long)a->element_size, (long)idx, (long)a->len, (long)a->cap);
        fprintf(stderr, "  Expected element_size=8 (ptr) but got %ld\n", (long)a->element_size);
        fprintf(stderr, "  This may indicate an Array<String> (16 bytes) or other non-pointer array\n");
        fprintf(stderr, "  being accessed as if it were an Array of pointers.\n");
        abort();
    }
    if (idx < 0 || idx >= a->len) {
        return NULL;
    }
    return ((void**)a->data)[idx];
}

void* seen_arr_get_element_ptr(SeenArray* a, int64_t idx) {
    if (!a) return NULL;
    if (a->element_size <= 0) {
        fprintf(stderr, "seen_arr_get_element_ptr: invalid element_size (%ld)\n", (long)a->element_size);
        abort();
    }
    if (idx < 0 || idx >= a->len) {
        return NULL;
    }
    return (char*)a->data + (idx * a->element_size);
}

// ============================================================================
// String Functions (implementations for LLVM backend linking)
// ============================================================================

SeenString seen_cstr_to_str(const char* s) {
    SeenString result = { strlen(s), (char*)s };
    return result;
}

// Convert SeenString to null-terminated C string (malloc'd copy)
char* seen_str_to_cstr(SeenString s) {
    char* buf = (char*)malloc(s.len + 1);
    if (buf) {
        memcpy(buf, s.data, s.len);
        buf[s.len] = '\0';
    }
    return buf;
}

int64_t seen_str_length(SeenString s) {
    return s.len;
}

int64_t seen_length(SeenString s) {
#ifdef SEEN_RUNTIME_DEBUG_COUNTERS
    g_length_count++;
#endif
    return s.len;
}

SeenString seen_substring(SeenString s, int64_t start, int64_t end) {
#ifdef SEEN_RUNTIME_DEBUG_COUNTERS
    g_substring_count++;
    if (g_substring_count % 1000000 == 0) {
        seen_debug_print_counters();
    }
#endif
    // Validate input string
    if (s.len < 0 || s.len > 100000000) {
        fprintf(stderr, "[RUNTIME ERROR] seen_substring: invalid string len=%ld\n", (long)s.len);
        fprintf(stderr, "  s.data=%p start=%ld end=%ld\n", (void*)s.data, (long)start, (long)end);
        abort();
    }
    if (s.len > 0 && s.data == NULL) {
        fprintf(stderr, "[RUNTIME ERROR] seen_substring: NULL data with len=%ld\n", (long)s.len);
        abort();
    }
    if (start < 0) start = 0;
    if (end > s.len) end = s.len;
    if (start >= end) {
        SeenString empty = { 0, "" };
        return empty;
    }
    int64_t newlen = end - start;
    char* newdata = (char*)seen_pool_alloc(newlen + 1);
    memcpy(newdata, s.data + start, newlen);
    newdata[newlen] = 0;
    SeenString result = { newlen, newdata };
    return result;
}

SeenString seen_str_concat_ss(SeenString a, SeenString b) {
    // Handle zero-initialized strings (NULL data with len=0 is a valid empty string)
    if (a.data == NULL && a.len == 0 && b.data == NULL && b.len == 0) {
        SeenString result = { 0, (char*)"" };
        return result;
    }
    if (a.data == NULL && a.len == 0) {
        // a is empty, return copy of b
        char* newdata = (char*)seen_pool_alloc(b.len + 1);
        memcpy(newdata, b.data, b.len);
        newdata[b.len] = 0;
        SeenString result = { b.len, newdata };
        return result;
    }
    if (b.data == NULL && b.len == 0) {
        // b is empty, return copy of a
        char* newdata = (char*)seen_pool_alloc(a.len + 1);
        memcpy(newdata, a.data, a.len);
        newdata[a.len] = 0;
        SeenString result = { a.len, newdata };
        return result;
    }
    // Non-zero length with NULL data is a real error
    if (a.data == NULL || b.data == NULL) {
        fprintf(stderr, "[RUNTIME ERROR] seen_str_concat_ss: NULL data pointer with non-zero length!\n");
        fprintf(stderr, "  a.len=%" PRId64 ", a.data=%p\n", a.len, a.data);
        fprintf(stderr, "  b.len=%" PRId64 ", b.data=%p\n", b.len, b.data);
        fflush(stderr);
#if !defined(_WIN32) && !defined(__ANDROID__)
        void* bt[20];
        int nptrs = backtrace(bt, 20);
        backtrace_symbols_fd(bt, nptrs, 2);
#endif
        abort();
    }
    char* newdata = (char*)seen_pool_alloc(a.len + b.len + 1);
    memcpy(newdata, a.data, a.len);
    memcpy(newdata + a.len, b.data, b.len);
    newdata[a.len + b.len] = 0;
    SeenString result = { a.len + b.len, newdata };
    return result;
}

SeenString seen_int_to_string(int64_t n) {
    char* buf = (char*)seen_pool_alloc(32);
    sprintf(buf, "%" PRId64, n);
    SeenString result = { strlen(buf), buf };
    return result;
}

SeenString seen_float_to_string(double f) {
    char* buf = (char*)seen_pool_alloc(32);
    sprintf(buf, "%.6g", f);
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
    // Get UTF-8 character code point at codepoint index (not byte index)
    if (index < 0) {
        return 0;
    }

    const unsigned char* data = (const unsigned char*)s.data;
    int64_t codepoint_idx = 0;
    int64_t byte_idx = 0;

    // Iterate through UTF-8 codepoints until we reach the desired index
    while (byte_idx < s.len && codepoint_idx < index) {
        unsigned char c = data[byte_idx];

        // Determine byte length of this UTF-8 character
        if ((c & 0x80) == 0) {
            byte_idx += 1;  // 1-byte (ASCII)
        } else if ((c & 0xE0) == 0xC0) {
            byte_idx += 2;  // 2-byte
        } else if ((c & 0xF0) == 0xE0) {
            byte_idx += 3;  // 3-byte
        } else if ((c & 0xF8) == 0xF0) {
            byte_idx += 4;  // 4-byte
        } else {
            // Invalid UTF-8 sequence, skip byte
            byte_idx += 1;
        }
        codepoint_idx++;
    }

    if (byte_idx >= s.len) {
        return 0;  // Out of bounds
    }

    // Decode the UTF-8 codepoint at byte_idx
    unsigned char c = data[byte_idx];

    if ((c & 0x80) == 0) {
        // 1-byte ASCII
        return (int64_t)c;
    } else if ((c & 0xE0) == 0xC0 && byte_idx + 1 < s.len) {
        // 2-byte
        int64_t cp = ((c & 0x1F) << 6) | (data[byte_idx + 1] & 0x3F);
        return cp;
    } else if ((c & 0xF0) == 0xE0 && byte_idx + 2 < s.len) {
        // 3-byte
        int64_t cp = ((c & 0x0F) << 12) |
                     ((data[byte_idx + 1] & 0x3F) << 6) |
                     (data[byte_idx + 2] & 0x3F);
        return cp;
    } else if ((c & 0xF8) == 0xF0 && byte_idx + 3 < s.len) {
        // 4-byte
        int64_t cp = ((c & 0x07) << 18) |
                     ((data[byte_idx + 1] & 0x3F) << 12) |
                     ((data[byte_idx + 2] & 0x3F) << 6) |
                     (data[byte_idx + 3] & 0x3F);
        return cp;
    }

    // Invalid UTF-8, return replacement character
    return 0xFFFD;
}

// Get the raw byte at a specific byte index (used by lexer for byte-level scanning)
int64_t seen_byte_at(SeenString s, int64_t byte_idx) {
    if (byte_idx < 0 || byte_idx >= s.len) {
        return 0;
    }
    return (int64_t)(unsigned char)s.data[byte_idx];
}

// String.byteAt() method - byte-level access (generated by old stage1_frozen as @String_byteAt)
// This is an alias for seen_byte_at when stage1_frozen's generic method dispatch is used.
int64_t String_byteAt(SeenString s, int64_t byte_idx) {
    return seen_byte_at(s, byte_idx);
}

// Get the number of UTF-8 codepoints (characters) in a string
int64_t seen_string_length_codepoints(SeenString s) {
    return seen_utf8_codepoint_count(s);
}

int64_t Char_toInt(int64_t c) {
    // Char is already stored as i64 code point, so this is identity
    return c;
}

// Create a string from an integer character code (Unicode code point)
// This provides JavaScript-like String.fromCharCode functionality
SeenString String_fromCharCode(int64_t code) {
    return seen_char_to_str(code);
}

// ============================================================================
// Char Classification & Conversion (ASCII)
// ============================================================================

int64_t Char_isDigit(int64_t c) {
    return (c >= '0' && c <= '9') ? 1 : 0;
}

int64_t Char_isAlpha(int64_t c) {
    return ((c >= 'A' && c <= 'Z') || (c >= 'a' && c <= 'z')) ? 1 : 0;
}

int64_t Char_isAlphanumeric(int64_t c) {
    return (Char_isAlpha(c) || Char_isDigit(c)) ? 1 : 0;
}

int64_t Char_isUpperCase(int64_t c) {
    return (c >= 'A' && c <= 'Z') ? 1 : 0;
}

int64_t Char_isLowerCase(int64_t c) {
    return (c >= 'a' && c <= 'z') ? 1 : 0;
}

int64_t Char_isWhitespace(int64_t c) {
    return (c == ' ' || c == '\t' || c == '\n' || c == '\r' ||
            c == 11 || c == 12 || c == 160) ? 1 : 0;
}

int64_t Char_toUpperCase(int64_t c) {
    if (c >= 'a' && c <= 'z') return c - 32;
    return c;
}

int64_t Char_toLowerCase(int64_t c) {
    if (c >= 'A' && c <= 'Z') return c + 32;
    return c;
}

// ============================================================================
// String Case Conversion, Parsing & Utilities
// ============================================================================

SeenString String_toUpperCase(SeenString s) {
    if (s.len == 0) return s;
    char* buf = (char*)malloc(s.len + 1);
    for (int64_t i = 0; i < s.len; i++) {
        unsigned char c = (unsigned char)s.data[i];
        buf[i] = (c >= 'a' && c <= 'z') ? (char)(c - 32) : (char)c;
    }
    buf[s.len] = '\0';
    SeenString result = { s.len, buf };
    return result;
}

SeenString String_toLowerCase(SeenString s) {
    if (s.len == 0) return s;
    char* buf = (char*)malloc(s.len + 1);
    for (int64_t i = 0; i < s.len; i++) {
        unsigned char c = (unsigned char)s.data[i];
        buf[i] = (c >= 'A' && c <= 'Z') ? (char)(c + 32) : (char)c;
    }
    buf[s.len] = '\0';
    SeenString result = { s.len, buf };
    return result;
}

int64_t String_toInt(SeenString s) {
    if (s.len == 0 || !s.data) return 0;
    // Null-terminate for strtoll
    char buf[32];
    int64_t copylen = s.len < 31 ? s.len : 31;
    memcpy(buf, s.data, copylen);
    buf[copylen] = '\0';
    char* end;
    long long val = strtoll(buf, &end, 10);
    if (end == buf) return 0; // no digits parsed
    return (int64_t)val;
}

double String_toFloat(SeenString s) {
    if (s.len == 0 || !s.data) return 0.0;
    char buf[64];
    int64_t copylen = s.len < 63 ? s.len : 63;
    memcpy(buf, s.data, copylen);
    buf[copylen] = '\0';
    char* end;
    double val = strtod(buf, &end);
    if (end == buf) return 0.0;
    return val;
}

SeenString String_reverse(SeenString s) {
    if (s.len <= 1) return s;
    char* buf = (char*)malloc(s.len + 1);
    for (int64_t i = 0; i < s.len; i++) {
        buf[i] = s.data[s.len - 1 - i];
    }
    buf[s.len] = '\0';
    SeenString result = { s.len, buf };
    return result;
}

int64_t String_isEmpty(SeenString s) {
    return (s.len == 0) ? 1 : 0;
}

int64_t String_count(SeenString s, SeenString needle) {
    if (needle.len == 0 || needle.len > s.len) return 0;
    int64_t count = 0;
    for (int64_t i = 0; i <= s.len - needle.len; i++) {
        if (memcmp(s.data + i, needle.data, needle.len) == 0) {
            count++;
            i += needle.len - 1; // non-overlapping
        }
    }
    return count;
}

SeenString String_replace(SeenString s, SeenString old, SeenString replacement) {
    if (old.len == 0 || s.len == 0) {
        char* copy = (char*)malloc(s.len + 1);
        if (s.len > 0) memcpy(copy, s.data, s.len);
        copy[s.len] = '\0';
        SeenString result = { s.len, copy };
        return result;
    }
    // Count occurrences first to allocate exact size
    int64_t count = 0;
    for (int64_t i = 0; i <= s.len - old.len; i++) {
        if (memcmp(s.data + i, old.data, old.len) == 0) {
            count++;
            i += old.len - 1;
        }
    }
    int64_t newLen = s.len + count * (replacement.len - old.len);
    char* buf = (char*)malloc(newLen + 1);
    int64_t pos = 0;
    for (int64_t i = 0; i < s.len; ) {
        if (i <= s.len - old.len && memcmp(s.data + i, old.data, old.len) == 0) {
            if (replacement.len > 0) memcpy(buf + pos, replacement.data, replacement.len);
            pos += replacement.len;
            i += old.len;
        } else {
            buf[pos++] = s.data[i++];
        }
    }
    buf[newLen] = '\0';
    SeenString result = { newLen, buf };
    return result;
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

// Option<T>.unwrap() specialized implementations
// Option struct layout: { hasValue: i8, value: T } with potential padding
// For Bool: { i8, i8 } where value is at offset 1
bool Option_unwrap_bool(void* option) {
    if (!option) {
        fprintf(stderr, "Option_unwrap_bool: called on null\n");
        abort();
    }
    // Check hasValue field (first byte)
    uint8_t* ptr = (uint8_t*)option;
    if (!ptr[0]) {
        fprintf(stderr, "Option_unwrap_bool: called on None\n");
        abort();
    }
    // Return value field (second byte interpreted as bool)
    return ptr[1] != 0;
}

int64_t Option_unwrap_int(void* option) {
    if (!option) {
        fprintf(stderr, "Option_unwrap_int: called on null\n");
        abort();
    }
    // Option<Int> layout: { hasValue: i8, padding: 7 bytes, value: i64 }
    uint8_t* ptr = (uint8_t*)option;
    if (!ptr[0]) {
        fprintf(stderr, "Option_unwrap_int: called on None\n");
        abort();
    }
    // Value is at offset 8 (after hasValue + padding)
    return *(int64_t*)(ptr + 8);
}

SeenString Option_unwrap_string(void* option) {
    if (!option) {
        fprintf(stderr, "Option_unwrap_string: called on null\n");
        abort();
    }
    // Option<String> layout: { hasValue: i8, padding: 7 bytes, value: SeenString }
    uint8_t* ptr = (uint8_t*)option;
    if (!ptr[0]) {
        fprintf(stderr, "Option_unwrap_string: called on None\n");
        abort();
    }
    // Value is at offset 8 (after hasValue + padding)
    return *(SeenString*)(ptr + 8);
}

// Unwrap float from Option<Float>
double Option_unwrap_float(void* option) {
    if (!option) {
        fprintf(stderr, "Option_unwrap_float: called on null\n");
        abort();
    }
    // Option<Float> layout: { hasValue: i8, padding: 7 bytes, value: double }
    uint8_t* ptr = (uint8_t*)option;
    if (!ptr[0]) {
        fprintf(stderr, "Option_unwrap_float: called on None\n");
        abort();
    }
    // Value is at offset 8 (after hasValue + padding)
    return *(double*)(ptr + 8);
}

// Unwrap boxed class type from Option<T> where T is a class
// Used for Vec, Map, HashMap, Result, and custom classes
// Returns the boxed i64 pointer value
int64_t Option_unwrap_boxed(void* option) {
    if (!option) {
        fprintf(stderr, "Option_unwrap_boxed: called on null\n");
        abort();
    }
    // Option<T> layout for class types: { hasValue: i8, padding: 7 bytes, value: i64 }
    uint8_t* ptr = (uint8_t*)option;
    if (!ptr[0]) {
        fprintf(stderr, "Option_unwrap_boxed: called on None\n");
        abort();
    }
    // Value is at offset 8 (after hasValue + padding)
    return *(int64_t*)(ptr + 8);
}

void* Option_unwrap(void* option) {
    if (!option) {
        fprintf(stderr, "Option_unwrap: called on null\n");
        abort();
    }
    // Generic Option unwrap - assumes value is a pointer at offset 8
    uint8_t* ptr = (uint8_t*)option;
    if (!ptr[0]) {
        fprintf(stderr, "Option_unwrap: called on None\n");
        abort();
    }
    return *(void**)(ptr + 8);
}

// Nullable String unwrap: String? (i64 pointer-or-null) -> String
// Nullable strings are stored as i64 where:
// - 0 means null
// - non-zero is a pointer to a heap-allocated SeenString struct
SeenString String_unwrap(int64_t nullable_str) {
    if (nullable_str == 0) {
        fprintf(stderr, "String_unwrap: called on null\n");
        abort();
    }
    // The i64 is a pointer to a SeenString struct on the heap
    SeenString* str_ptr = (SeenString*)nullable_str;
    return *str_ptr;
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

void print(SeenString s) {
    printf("%.*s", (int)s.len, s.data);
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

// Allocate a StringBuilder with pre-allocated capacity for parts array
void* StringBuilder_new_with_capacity(int64_t cap) {
    StringBuilder* sb = (StringBuilder*)malloc(sizeof(StringBuilder));
    SeenArray* parts = (SeenArray*)malloc(sizeof(SeenArray));
    parts->data = malloc(cap * sizeof(SeenString));
    parts->len = 0;
    parts->cap = cap;
    sb->parts = parts;
    sb->totalLength = 0;
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

// Fast integer-to-string (avoids sprintf overhead)
static inline int fast_i64_to_buf(char* buf, int64_t n) {
    if (n == 0) { buf[0] = '0'; buf[1] = '\0'; return 1; }
    char tmp[24];
    int i = 0;
    int neg = 0;
    uint64_t u;
    if (n < 0) { neg = 1; u = (uint64_t)(-(n + 1)) + 1; } else { u = (uint64_t)n; }
    while (u > 0) { tmp[i++] = '0' + (char)(u % 10); u /= 10; }
    int len = 0;
    if (neg) buf[len++] = '-';
    while (i > 0) buf[len++] = tmp[--i];
    buf[len] = '\0';
    return len;
}

// Fast float-to-string with 6 decimal places (avoids snprintf overhead)
static inline int fast_f64_to_buf(char* buf, double f) {
    int pos = 0;
    if (f < 0) { buf[pos++] = '-'; f = -f; }
    // Handle integer part
    uint64_t int_part = (uint64_t)f;
    double frac = f - (double)int_part;
    // Write integer part
    if (int_part == 0) {
        buf[pos++] = '0';
    } else {
        char tmp[24];
        int i = 0;
        uint64_t u = int_part;
        while (u > 0) { tmp[i++] = '0' + (char)(u % 10); u /= 10; }
        while (i > 0) buf[pos++] = tmp[--i];
    }
    // Write decimal point and 6 fractional digits
    buf[pos++] = '.';
    // Multiply by 1e6 and round
    uint64_t frac_digits = (uint64_t)(frac * 1000000.0 + 0.5);
    if (frac_digits >= 1000000) { frac_digits = 999999; }
    buf[pos + 5] = '0' + (char)(frac_digits % 10); frac_digits /= 10;
    buf[pos + 4] = '0' + (char)(frac_digits % 10); frac_digits /= 10;
    buf[pos + 3] = '0' + (char)(frac_digits % 10); frac_digits /= 10;
    buf[pos + 2] = '0' + (char)(frac_digits % 10); frac_digits /= 10;
    buf[pos + 1] = '0' + (char)(frac_digits % 10); frac_digits /= 10;
    buf[pos + 0] = '0' + (char)(frac_digits % 10);
    pos += 6;
    buf[pos] = '\0';
    return pos;
}

// Fused append float — fast dtoa + pool alloc + direct push
int64_t StringBuilder_appendFloat(void* s, double f) {
    StringBuilder* sb = (StringBuilder*)s;
    char buf[64];
    int len = fast_f64_to_buf(buf, f);
    char* data = (char*)seen_pool_alloc(len + 1);
    memcpy(data, buf, len + 1);
    SeenString str = { len, data };
    sb->totalLength += len;
    seen_arr_push_str_fast(sb->parts, str);
    return 0;
}

// Fused append int — fast itoa + pool alloc + direct push
int64_t StringBuilder_appendInt(void* s, int64_t n) {
    StringBuilder* sb = (StringBuilder*)s;
    char* buf = (char*)seen_pool_alloc(32);
    int len = fast_i64_to_buf(buf, n);
    SeenString str = { len, buf };
    sb->totalLength += len;
    seen_arr_push_str_fast(sb->parts, str);
    return 0;
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
__attribute__((weak)) void* Some(void* value) { return value; }
__attribute__((weak)) void* None(void) { return NULL; }

// Generic default value - returns null/zero for any type
// Used by Option<T> for default initialization
void* __default(void) { return NULL; }
bool Result_isOkay(void* result) { return result != NULL; }
SeenString Result_unwrapErr(void* result) { return (SeenString){ 0, "" }; }

// SeenTokenType is actually i64 (enum), unwrap just returns the value
int64_t SeenTokenType_unwrap(int64_t value) { return value; }

// ============================================================================
// LSP Type Unwrap Functions
// For nullable class types (T?), unwrap returns the pointer-as-int64
// ============================================================================

int64_t LspPosition_unwrap(void* ptr) {
    if (!ptr) {
        fprintf(stderr, "LspPosition_unwrap: called on null\n");
        abort();
    }
    return (int64_t)ptr;
}

int64_t Position_unwrap(void* ptr) {
    if (!ptr) {
        fprintf(stderr, "Position_unwrap: called on null\n");
        abort();
    }
    return (int64_t)ptr;
}

int64_t Range_unwrap(void* ptr) {
    if (!ptr) {
        fprintf(stderr, "Range_unwrap: called on null\n");
        abort();
    }
    return (int64_t)ptr;
}

int64_t TextDocumentIdentifier_unwrap(void* ptr) {
    return (int64_t)ptr;
}

int64_t TextDocumentItem_unwrap(void* ptr) {
    return (int64_t)ptr;
}

int64_t TextDocumentContentChangeEvent_unwrap(void* ptr) {
    return (int64_t)ptr;
}

int64_t VersionedTextDocumentIdentifier_unwrap(void* ptr) {
    return (int64_t)ptr;
}

void* Document_unwrap(void* ptr) {
    if (!ptr) {
        fprintf(stderr, "Document_unwrap: called on null\n");
        abort();
    }
    return ptr;
}

void* Hover_unwrap(void* ptr) {
    return ptr;
}

void* MarkupContent_unwrap(void* ptr) {
    return ptr;
}

void* TextEdit_unwrap(void* ptr) {
    return ptr;
}

void* CodeAction_unwrap(void* ptr) {
    return ptr;
}

void* CompletionItem_unwrap(void* ptr) {
    return ptr;
}

void* CompletionList_unwrap(void* ptr) {
    return ptr;
}

void* DidOpenTextDocumentParams_unwrap(void* ptr) {
    return ptr;
}

void* DidChangeTextDocumentParams_unwrap(void* ptr) {
    return ptr;
}

void* DidCloseTextDocumentParams_unwrap(void* ptr) {
    return ptr;
}

void* CompletionParams_unwrap(void* ptr) {
    return ptr;
}

void* TextDocumentPositionParams_unwrap(void* ptr) {
    return ptr;
}

void* HoverParams_unwrap(void* ptr) {
    return ptr;
}

void* DefinitionParams_unwrap(void* ptr) {
    return ptr;
}

void* CodeActionParams_unwrap(void* ptr) {
    return ptr;
}

void* DocumentFormattingParams_unwrap(void* ptr) {
    return ptr;
}

void* SeenDocument_unwrap(void* ptr) {
    return ptr;
}

void* Location_unwrap(void* ptr) {
    return ptr;
}

void* Diagnostic_unwrap(void* ptr) {
    return ptr;
}

void* InitializeParams_unwrap(void* ptr) {
    return ptr;
}

void* LspError_unwrap(void* ptr) {
    return ptr;
}

void* LspMessage_unwrap(void* ptr) {
    return ptr;
}

int64_t JsonValue_unwrap(void* ptr) {
    return (int64_t)ptr;
}

// ============================================================================
// Typechecker Type Stubs
// ============================================================================

void* TypeError(void) { return malloc(64); }
void* FunctionType(void) { return malloc(64); }
void* ClassType(void) { return malloc(64); }
void* InterfaceType(void) { return malloc(64); }
void* Location(void) { return malloc(32); }

// TypeError methods are now defined in compiler_seen/src/typechecker/interfaces.seen
// as a proper Seen class with full implementations; stubs removed to avoid duplicate symbols.

// ============================================================================
// Parser Node Constructor Stubs
// These have weak linkage so user-defined versions can override them
// ============================================================================

__attribute__((weak)) void* TypeNode_new(void) {
    // Simple struct with name field
    void* node = malloc(64);
    memset(node, 0, 64);
    return node;
}

__attribute__((weak)) void* ItemNode_new(void) {
    void* node = malloc(256);
    memset(node, 0, 256);
    return node;
}

__attribute__((weak)) void* ParamNode_new(void) {
    void* node = malloc(128);
    memset(node, 0, 128);
    return node;
}

__attribute__((weak)) void* ImportSymbolNode_new(void) {
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
// Stdin/Stdout Functions (for LSP and interactive programs)
// ============================================================================

// Read a single line from stdin (blocking)
// Returns the line including newline character if present
SeenString __ReadStdinLine(void) {
#ifdef _WIN32
    // getline is not available on Windows/mingw; use fgets with dynamic buffer
    size_t capacity = 256;
    char* buffer = (char*)malloc(capacity);
    if (!buffer) { SeenString empty = { 0, "" }; return empty; }
    size_t len = 0;
    while (1) {
        if (!fgets(buffer + len, (int)(capacity - len), stdin)) {
            if (len == 0) { free(buffer); SeenString empty = { 0, "" }; return empty; }
            break;
        }
        len = strlen(buffer);
        if (len > 0 && buffer[len - 1] == '\n') break;
        if (len + 1 >= capacity) {
            capacity *= 2;
            buffer = (char*)realloc(buffer, capacity);
            if (!buffer) { SeenString empty = { 0, "" }; return empty; }
        }
    }
    SeenString result = { (int64_t)len, buffer };
    return result;
#else
    char* buffer = NULL;
    size_t capacity = 0;
    ssize_t len = getline(&buffer, &capacity, stdin);

    if (len < 0) {
        // EOF or error
        free(buffer);
        SeenString empty = { 0, "" };
        return empty;
    }

    // getline includes the newline, keep it
    SeenString result = { len, buffer };
    return result;
#endif
}

// Read exactly N bytes from stdin (blocking)
// Returns exactly count bytes, or fewer if EOF is reached
SeenString __ReadStdinBytes(int64_t count) {
    if (count <= 0) {
        SeenString empty = { 0, "" };
        return empty;
    }

    char* buffer = (char*)malloc(count + 1);
    if (!buffer) {
        SeenString empty = { 0, "" };
        return empty;
    }

    size_t total_read = 0;
    while (total_read < (size_t)count) {
        size_t read = fread(buffer + total_read, 1, count - total_read, stdin);
        if (read == 0) {
            // EOF or error
            break;
        }
        total_read += read;
    }

    buffer[total_read] = '\0';
    SeenString result = { total_read, buffer };
    return result;
}

// Flush stdout buffer
void __FlushStdout(void) {
    fflush(stdout);
}

// Print string to stdout without newline (for LSP Content-Length headers)
void __PrintRaw(SeenString s) {
    fwrite(s.data, 1, s.len, stdout);
}

// ============================================================================
// Benchmark Intrinsics
// ============================================================================

void __Print(SeenString s) {
    fwrite(s.data, 1, s.len, stdout);
}

void __PrintInt(int64_t n) {
    printf("%lld", (long long)n);
}

void __PrintFloat(double f) {
    printf("%.9f", f);
}

double __IntToFloat(int64_t i) {
    return (double)i;
}

int64_t __FloatToInt(double f) {
    return (int64_t)f;
}

double __GetTime(void) {
#ifdef _WIN32
    LARGE_INTEGER freq, count;
    QueryPerformanceFrequency(&freq);
    QueryPerformanceCounter(&count);
    return (double)count.QuadPart / (double)freq.QuadPart;
#else
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (double)ts.tv_sec + (double)ts.tv_nsec / 1e9;
#endif
}

double __Sqrt(double x) {
    return sqrt(x);
}

double __Sin(double x) { return sin(x); }
double __Cos(double x) { return cos(x); }
double __Exp(double x) { return exp(x); }
double __Log(double x) { return log(x); }
double __Fabs(double x) { return fabs(x); }
double __Pow(double x, double y) { return pow(x, y); }
double __Asin(double x) { return asin(x); }
double __Acos(double x) { return acos(x); }
double __Atan(double x) { return atan(x); }
double __Atan2(double y, double x) { return atan2(y, x); }
double __Sinh(double x) { return sinh(x); }
double __Cosh(double x) { return cosh(x); }
double __Tanh(double x) { return tanh(x); }
double __Tan(double x) { return tan(x); }
double __Log10(double x) { return log10(x); }

SeenString __FloatToString(double f) {
    char buf[64];
    int len = snprintf(buf, sizeof(buf), "%.6f", f);
    char* data = (char*)seen_pool_alloc(len + 1);
    memcpy(data, buf, len + 1);
    SeenString result = { len, data };
    return result;
}

// ============================================================================
// Profiling Runtime Functions (for @profile decorator)
// ============================================================================

// Maximum number of unique profiled functions
#define SEEN_MAX_PROFILE_ENTRIES 1024

// Global profiling state
static SeenProfileEntry g_profile_entries[SEEN_MAX_PROFILE_ENTRIES];
static int g_profile_count = 0;
static int g_profile_initialized = 0;

// Call stack for tracking nested calls (to calculate self time)
#define SEEN_MAX_PROFILE_DEPTH 256
static int g_profile_stack_indices[SEEN_MAX_PROFILE_DEPTH];
static uint64_t g_profile_stack_starts[SEEN_MAX_PROFILE_DEPTH];
static int g_profile_stack_depth = 0;

// Get current time in nanoseconds
static uint64_t __seen_profile_now_ns(void) {
#ifdef _WIN32
    LARGE_INTEGER freq, count;
    QueryPerformanceFrequency(&freq);
    QueryPerformanceCounter(&count);
    return (uint64_t)((double)count.QuadPart / (double)freq.QuadPart * 1e9);
#else
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (uint64_t)ts.tv_sec * 1000000000ULL + (uint64_t)ts.tv_nsec;
#endif
}

// Find or create profile entry for a function
static int __seen_profile_find_or_create(const char* func_name) {
    // Linear search (could be optimized with hash table for many functions)
    for (int i = 0; i < g_profile_count; i++) {
        if (strcmp(g_profile_entries[i].name, func_name) == 0) {
            return i;
        }
    }
    // Create new entry
    if (g_profile_count < SEEN_MAX_PROFILE_ENTRIES) {
        int idx = g_profile_count++;
        g_profile_entries[idx].name = func_name;
        g_profile_entries[idx].call_count = 0;
        g_profile_entries[idx].total_ns = 0;
        g_profile_entries[idx].self_ns = 0;
        g_profile_entries[idx].start_ns = 0;
        g_profile_entries[idx].depth = 0;
        return idx;
    }
    return -1;  // Table full
}

// Compare function for qsort (sort by total time descending)
static int __seen_profile_compare(const void* a, const void* b) {
    const SeenProfileEntry* ea = (const SeenProfileEntry*)a;
    const SeenProfileEntry* eb = (const SeenProfileEntry*)b;
    if (eb->total_ns > ea->total_ns) return 1;
    if (eb->total_ns < ea->total_ns) return -1;
    return 0;
}

void __seen_profile_init(void) {
    if (!g_profile_initialized) {
        g_profile_initialized = 1;
        atexit(__seen_profile_report);
    }
}

void __seen_profile_enter(const char* func_name) {
    __seen_profile_init();

    int idx = __seen_profile_find_or_create(func_name);
    if (idx < 0) return;

    uint64_t now = __seen_profile_now_ns();

    SeenProfileEntry* entry = &g_profile_entries[idx];
    entry->call_count++;
    entry->depth++;

    if (entry->depth == 1) {
        // First entry (not recursive)
        entry->start_ns = now;
    }

    // Push to call stack
    if (g_profile_stack_depth < SEEN_MAX_PROFILE_DEPTH) {
        g_profile_stack_indices[g_profile_stack_depth] = idx;
        g_profile_stack_starts[g_profile_stack_depth] = now;
        g_profile_stack_depth++;
    }
}

void __seen_profile_exit(const char* func_name) {
    uint64_t now = __seen_profile_now_ns();

    int idx = __seen_profile_find_or_create(func_name);
    if (idx < 0) return;

    SeenProfileEntry* entry = &g_profile_entries[idx];
    entry->depth--;

    if (entry->depth == 0 && entry->start_ns > 0) {
        // Calculate time for this call
        uint64_t duration = now - entry->start_ns;
        entry->total_ns += duration;
        entry->start_ns = 0;
    }

    // Pop from call stack and calculate self time
    if (g_profile_stack_depth > 0) {
        g_profile_stack_depth--;
        uint64_t call_duration = now - g_profile_stack_starts[g_profile_stack_depth];
        entry->self_ns += call_duration;

        // Subtract from parent's self time if there is a parent
        if (g_profile_stack_depth > 0) {
            int parent_idx = g_profile_stack_indices[g_profile_stack_depth - 1];
            if (parent_idx >= 0 && parent_idx < g_profile_count) {
                // Don't subtract now - will be handled when parent exits
            }
        }
    }
}

void __seen_profile_report(void) {
    if (g_profile_count == 0) return;

    fprintf(stderr, "\n");
    fprintf(stderr, "╔════════════════════════════════════════════════════════════════════════════╗\n");
    fprintf(stderr, "║                         SEEN PROFILING REPORT                              ║\n");
    fprintf(stderr, "╠════════════════════════════════════════════════════════════════════════════╣\n");

    // Sort by total time
    qsort(g_profile_entries, g_profile_count, sizeof(SeenProfileEntry), __seen_profile_compare);

    // Calculate total program time
    uint64_t total_program_ns = 0;
    for (int i = 0; i < g_profile_count; i++) {
        total_program_ns += g_profile_entries[i].self_ns;
    }

    fprintf(stderr, "║ %-40s %8s %10s %10s ║\n", "Function", "Calls", "Total(ms)", "Self(ms)");
    fprintf(stderr, "╠════════════════════════════════════════════════════════════════════════════╣\n");

    for (int i = 0; i < g_profile_count && i < 20; i++) {  // Top 20
        SeenProfileEntry* e = &g_profile_entries[i];
        double total_ms = (double)e->total_ns / 1000000.0;
        double self_ms = (double)e->self_ns / 1000000.0;

        // Truncate function name if too long
        char name_buf[41];
        strncpy(name_buf, e->name, 40);
        name_buf[40] = '\0';

        fprintf(stderr, "║ %-40s %8lu %10.2f %10.2f ║\n",
                name_buf, (unsigned long)e->call_count, total_ms, self_ms);
    }

    if (g_profile_count > 20) {
        fprintf(stderr, "║ ... and %d more functions                                                  ║\n",
                g_profile_count - 20);
    }

    fprintf(stderr, "╠════════════════════════════════════════════════════════════════════════════╣\n");
    fprintf(stderr, "║ Total profiled functions: %-4d                                             ║\n", g_profile_count);
    fprintf(stderr, "╚════════════════════════════════════════════════════════════════════════════╝\n");
}

// ============================================================================
// Component Framework Runtime (for @component decorator)
// ============================================================================

#define SEEN_MAX_COMPONENTS 4096
#define SEEN_MAX_CHILDREN 256

// Component state enum (matches ComponentState in seen_std)
typedef enum {
    SEEN_COMPONENT_UNINITIALIZED = 0,
    SEEN_COMPONENT_INITIALIZED = 1,
    SEEN_COMPONENT_MOUNTED = 2,
    SEEN_COMPONENT_UNMOUNTED = 3,
    SEEN_COMPONENT_DESTROYED = 4
} SeenComponentState;

// Component info stored in registry
typedef struct {
    int64_t id;
    char name[128];
    SeenComponentState state;
    int64_t parent_id;
    int64_t children[SEEN_MAX_CHILDREN];
    int child_count;
    int is_deterministic;
} SeenComponentInfo;

// Global component registry
static SeenComponentInfo g_components[SEEN_MAX_COMPONENTS];
static int g_component_count = 0;
static int64_t g_next_component_id = 1;
static int g_component_initialized = 0;

static void __seen_component_init(void) {
    if (g_component_initialized) return;
    g_component_initialized = 1;
    g_component_count = 0;
    g_next_component_id = 1;
    memset(g_components, 0, sizeof(g_components));
}

// Find component by ID
static SeenComponentInfo* __seen_component_find(int64_t id) {
    for (int i = 0; i < g_component_count; i++) {
        if (g_components[i].id == id) {
            return &g_components[i];
        }
    }
    return NULL;
}

// Register a new component and return its ID
int64_t __seen_component_register(SeenString name, int64_t parent_id) {
    __seen_component_init();

    if (g_component_count >= SEEN_MAX_COMPONENTS) {
        fprintf(stderr, "ERROR: Component registry full (max %d)\n", SEEN_MAX_COMPONENTS);
        return -1;
    }

    int64_t id = g_next_component_id++;
    SeenComponentInfo* info = &g_components[g_component_count++];

    info->id = id;
    // Copy name from SeenString
    if (name.data && name.len > 0) {
        int copy_len = (name.len < 127) ? name.len : 127;
        memcpy(info->name, name.data, copy_len);
        info->name[copy_len] = '\0';
    } else {
        strcpy(info->name, "unknown");
    }
    info->state = SEEN_COMPONENT_INITIALIZED;
    info->parent_id = parent_id;
    info->child_count = 0;
    info->is_deterministic = 1;

    // Add to parent's children list
    if (parent_id > 0) {
        SeenComponentInfo* parent = __seen_component_find(parent_id);
        if (parent && parent->child_count < SEEN_MAX_CHILDREN) {
            parent->children[parent->child_count++] = id;
        }
    }

    return id;
}

// Unregister a component
void __seen_component_unregister(int64_t id) {
    SeenComponentInfo* info = __seen_component_find(id);
    if (!info) return;

    // Remove from parent's children list
    if (info->parent_id > 0) {
        SeenComponentInfo* parent = __seen_component_find(info->parent_id);
        if (parent) {
            for (int i = 0; i < parent->child_count; i++) {
                if (parent->children[i] == id) {
                    // Shift remaining children down
                    for (int j = i; j < parent->child_count - 1; j++) {
                        parent->children[j] = parent->children[j + 1];
                    }
                    parent->child_count--;
                    break;
                }
            }
        }
    }

    // Mark as destroyed (don't actually remove to avoid array compaction issues)
    info->state = SEEN_COMPONENT_DESTROYED;
    info->id = 0;  // Mark slot as reusable
}

// Set component state
void __seen_component_set_state(int64_t id, int64_t state) {
    SeenComponentInfo* info = __seen_component_find(id);
    if (info) {
        info->state = (SeenComponentState)state;
    }
}

// Get children of a component - returns Array<Int>
SeenArray* __seen_component_get_children(int64_t id) {
    SeenArray* arr = seen_arr_new_ptr_ptr();
    if (!arr) return NULL;

    SeenComponentInfo* info = __seen_component_find(id);
    if (info) {
        for (int i = 0; i < info->child_count; i++) {
            seen_arr_push_i64(arr, info->children[i]);
        }
    }

    return arr;
}

// Mount component tree (recursive helper - called from generated code)
void __seen_component_mount_tree(int64_t id) {
    SeenComponentInfo* info = __seen_component_find(id);
    if (!info) return;

    // Mount children first (depth-first)
    for (int i = 0; i < info->child_count; i++) {
        __seen_component_mount_tree(info->children[i]);
    }

    // Set state to mounted
    info->state = SEEN_COMPONENT_MOUNTED;
}

// Unmount component tree (recursive helper - called from generated code)
void __seen_component_unmount_tree(int64_t id) {
    SeenComponentInfo* info = __seen_component_find(id);
    if (!info) return;

    // Set state to unmounted first
    info->state = SEEN_COMPONENT_UNMOUNTED;

    // Unmount children in reverse order
    for (int i = info->child_count - 1; i >= 0; i--) {
        __seen_component_unmount_tree(info->children[i]);
    }
}

// Destroy component tree (recursive helper - called from generated code)
void __seen_component_destroy_tree(int64_t id) {
    SeenComponentInfo* info = __seen_component_find(id);
    if (!info) return;

    // Destroy children first (reverse order)
    for (int i = info->child_count - 1; i >= 0; i--) {
        __seen_component_destroy_tree(info->children[i]);
    }

    // Mark as destroyed
    info->state = SEEN_COMPONENT_DESTROYED;
}

// ============================================================================
// Vec<T> Runtime Helpers
// Vec is a simple growable array: { data: ptr, length: i64, capacity: i64 }
// All elements are stored as i64 (class types are pointer-as-int, primitives are direct)
// ============================================================================

typedef struct {
    int64_t* data;
    int64_t length;
    int64_t capacity;
} SeenVec;

void* Vec_new(void) {
    SeenVec* vec = (SeenVec*)malloc(sizeof(SeenVec));
    vec->data = NULL;
    vec->length = 0;
    vec->capacity = 0;
    return vec;
}

void Vec_push(void* vecPtr, int64_t value) {
    SeenVec* vec = (SeenVec*)vecPtr;
    if (vec->length >= vec->capacity) {
        int64_t newCap = vec->capacity == 0 ? 8 : vec->capacity * 2;
        vec->data = (int64_t*)realloc(vec->data, sizeof(int64_t) * newCap);
        vec->capacity = newCap;
    }
    vec->data[vec->length++] = value;
}

int64_t Vec_get(void* vecPtr, int64_t index) {
    SeenVec* vec = (SeenVec*)vecPtr;
    if (index < 0 || index >= vec->length) return 0;
    return vec->data[index];
}

void Vec_set(void* vecPtr, int64_t index, int64_t value) {
    SeenVec* vec = (SeenVec*)vecPtr;
    if (index < 0 || index >= vec->length) return;
    vec->data[index] = value;
}

int64_t Vec_pop(void* vecPtr) {
    SeenVec* vec = (SeenVec*)vecPtr;
    if (vec->length == 0) return 0;
    vec->length--;
    return vec->data[vec->length];
}

void Vec_clear(void* vecPtr) {
    SeenVec* vec = (SeenVec*)vecPtr;
    vec->length = 0;
}

int64_t Vec_capacity(void* vecPtr) {
    SeenVec* vec = (SeenVec*)vecPtr;
    return vec->capacity;
}

void Vec_ensureCapacity(void* vecPtr, int64_t capacity) {
    SeenVec* vec = (SeenVec*)vecPtr;
    if (capacity <= vec->capacity) return;
    int64_t newCap = vec->capacity == 0 ? 8 : vec->capacity;
    while (newCap < capacity) {
        newCap *= 2;
    }
    vec->data = (int64_t*)realloc(vec->data, sizeof(int64_t) * newCap);
    vec->capacity = newCap;
}

// Vec<Int> utility methods

int64_t Vec_contains(void* vecPtr, int64_t value) {
    SeenVec* vec = (SeenVec*)vecPtr;
    for (int64_t i = 0; i < vec->length; i++) {
        if (vec->data[i] == value) return 1;
    }
    return 0;
}

int64_t Vec_indexOf(void* vecPtr, int64_t value) {
    SeenVec* vec = (SeenVec*)vecPtr;
    for (int64_t i = 0; i < vec->length; i++) {
        if (vec->data[i] == value) return i;
    }
    return -1;
}

void Vec_reverse(void* vecPtr) {
    SeenVec* vec = (SeenVec*)vecPtr;
    int64_t left = 0, right = vec->length - 1;
    while (left < right) {
        int64_t tmp = vec->data[left];
        vec->data[left] = vec->data[right];
        vec->data[right] = tmp;
        left++;
        right--;
    }
}

int64_t Vec_remove(void* vecPtr, int64_t index) {
    SeenVec* vec = (SeenVec*)vecPtr;
    if (index < 0 || index >= vec->length) return 0;
    int64_t removed = vec->data[index];
    for (int64_t i = index; i < vec->length - 1; i++) {
        vec->data[i] = vec->data[i + 1];
    }
    vec->length--;
    return removed;
}

void Vec_insert(void* vecPtr, int64_t index, int64_t value) {
    SeenVec* vec = (SeenVec*)vecPtr;
    if (index < 0 || index > vec->length) return;
    if (vec->length >= vec->capacity) {
        int64_t newCap = vec->capacity == 0 ? 8 : vec->capacity * 2;
        vec->data = (int64_t*)realloc(vec->data, sizeof(int64_t) * newCap);
        vec->capacity = newCap;
    }
    for (int64_t i = vec->length; i > index; i--) {
        vec->data[i] = vec->data[i - 1];
    }
    vec->data[index] = value;
    vec->length++;
}

int64_t Vec_first(void* vecPtr) {
    SeenVec* vec = (SeenVec*)vecPtr;
    if (vec->length == 0) return 0;
    return vec->data[0];
}

int64_t Vec_last(void* vecPtr) {
    SeenVec* vec = (SeenVec*)vecPtr;
    if (vec->length == 0) return 0;
    return vec->data[vec->length - 1];
}

static void Vec_sort_helper(int64_t* data, int64_t low, int64_t high) {
    if (low >= high) return;
    int64_t pivot = data[high];
    int64_t i = low - 1;
    for (int64_t j = low; j < high; j++) {
        if (data[j] <= pivot) {
            i++;
            int64_t tmp = data[i];
            data[i] = data[j];
            data[j] = tmp;
        }
    }
    int64_t tmp = data[i + 1];
    data[i + 1] = data[high];
    data[high] = tmp;
    int64_t pi = i + 1;
    Vec_sort_helper(data, low, pi - 1);
    Vec_sort_helper(data, pi + 1, high);
}

void Vec_sort(void* vecPtr) {
    SeenVec* vec = (SeenVec*)vecPtr;
    if (vec->length <= 1) return;
    Vec_sort_helper(vec->data, 0, vec->length - 1);
}

int64_t Vec_len(void* vecPtr) {
    SeenVec* vec = (SeenVec*)vecPtr;
    return vec->length;
}

// ============================================================================
// BTreeMap<K,V> Runtime Helpers - Simplified Linear Implementation
// BTreeMap uses a linear array for simplicity: { entries: ptr, capacity: i64, length: i64 }
// Each entry is { key: i64, value: i64 }
// ============================================================================

typedef struct {
    int64_t key;
    int64_t value;
} BTreeMapEntry;

typedef struct {
    BTreeMapEntry* entries;
    int64_t capacity;
    int64_t length;
} SeenBTreeMap;

// Helper: create Option<V> as Some(value) or None
static int64_t BTreeMap_make_some(int64_t value) {
    // Option layout: { i1 hasValue, i64 value }
    uint8_t* opt = (uint8_t*)malloc(16);
    *opt = 1;  // hasValue = true
    *(int64_t*)(opt + 8) = value;
    return (int64_t)opt;
}

static int64_t BTreeMap_make_none(void) {
    uint8_t* opt = (uint8_t*)malloc(16);
    *opt = 0;  // hasValue = false
    return (int64_t)opt;
}

void* BTreeMap_new(void) {
    SeenBTreeMap* map = (SeenBTreeMap*)malloc(sizeof(SeenBTreeMap));
    map->capacity = 16;
    map->entries = (BTreeMapEntry*)malloc(sizeof(BTreeMapEntry) * map->capacity);
    map->length = 0;
    return map;
}

bool BTreeMap_insert(void* mapPtr, int64_t key, int64_t value) {
    SeenBTreeMap* map = (SeenBTreeMap*)mapPtr;
    // Check if key exists
    for (int64_t i = 0; i < map->length; i++) {
        if (map->entries[i].key == key) {
            map->entries[i].value = value;
            return false;  // Updated existing
        }
    }
    // Grow if needed
    if (map->length >= map->capacity) {
        map->capacity *= 2;
        map->entries = realloc(map->entries, sizeof(BTreeMapEntry) * map->capacity);
    }
    map->entries[map->length].key = key;
    map->entries[map->length].value = value;
    map->length++;
    return true;  // New entry
}

int64_t BTreeMap_get(void* mapPtr, int64_t key) {
    SeenBTreeMap* map = (SeenBTreeMap*)mapPtr;
    for (int64_t i = 0; i < map->length; i++) {
        if (map->entries[i].key == key) {
            return BTreeMap_make_some(map->entries[i].value);
        }
    }
    return BTreeMap_make_none();
}

bool BTreeMap_containsKey(void* mapPtr, int64_t key) {
    SeenBTreeMap* map = (SeenBTreeMap*)mapPtr;
    for (int64_t i = 0; i < map->length; i++) {
        if (map->entries[i].key == key) return true;
    }
    return false;
}

int64_t BTreeMap_keys(void* mapPtr) {
    SeenBTreeMap* map = (SeenBTreeMap*)mapPtr;
    SeenVec* vec = Vec_new();
    for (int64_t i = 0; i < map->length; i++) {
        Vec_push(vec, map->entries[i].key);
    }
    return (int64_t)vec;
}

int64_t BTreeMap_values(void* mapPtr) {
    SeenBTreeMap* map = (SeenBTreeMap*)mapPtr;
    SeenVec* vec = Vec_new();
    for (int64_t i = 0; i < map->length; i++) {
        Vec_push(vec, map->entries[i].value);
    }
    return (int64_t)vec;
}

int64_t BTreeMap_remove(void* mapPtr, int64_t key) {
    SeenBTreeMap* map = (SeenBTreeMap*)mapPtr;
    for (int64_t i = 0; i < map->length; i++) {
        if (map->entries[i].key == key) {
            int64_t value = map->entries[i].value;
            // Shift remaining entries
            for (int64_t j = i; j < map->length - 1; j++) {
                map->entries[j] = map->entries[j + 1];
            }
            map->length--;
            return BTreeMap_make_some(value);
        }
    }
    return BTreeMap_make_none();
}

void BTreeMap_clear(void* mapPtr) {
    SeenBTreeMap* map = (SeenBTreeMap*)mapPtr;
    map->length = 0;
}

// ============================================================================
// Vec with String elements
// Vec_str stores SeenString values (16 bytes each: len + data ptr)
// ============================================================================

typedef struct {
    SeenString* data;
    int64_t length;
    int64_t capacity;
} SeenVecStr;

void* Vec_new_str(void) {
    SeenVecStr* vec = (SeenVecStr*)malloc(sizeof(SeenVecStr));
    vec->capacity = 8;
    vec->data = (SeenString*)malloc(sizeof(SeenString) * vec->capacity);
    vec->length = 0;
    return vec;
}

void Vec_push_str(void* vecPtr, SeenString value) {
    SeenVecStr* vec = (SeenVecStr*)vecPtr;
    if (vec->length >= vec->capacity) {
        vec->capacity *= 2;
        vec->data = realloc(vec->data, sizeof(SeenString) * vec->capacity);
    }
    vec->data[vec->length++] = value;
}

SeenString Vec_get_str(void* vecPtr, int64_t index) {
    SeenVecStr* vec = (SeenVecStr*)vecPtr;
    if (index < 0 || index >= vec->length) {
        SeenString empty = {0, NULL};
        return empty;
    }
    return vec->data[index];
}

int64_t Vec_len_str(void* vecPtr) {
    SeenVecStr* vec = (SeenVecStr*)vecPtr;
    return vec->length;
}

int64_t Vec_capacity_str(void* vecPtr) {
    SeenVecStr* vec = (SeenVecStr*)vecPtr;
    return vec->capacity;
}

void Vec_ensureCapacity_str(void* vecPtr, int64_t capacity) {
    SeenVecStr* vec = (SeenVecStr*)vecPtr;
    if (capacity <= vec->capacity) return;
    int64_t newCap = vec->capacity == 0 ? 8 : vec->capacity;
    while (newCap < capacity) {
        newCap *= 2;
    }
    vec->data = (SeenString*)realloc(vec->data, sizeof(SeenString) * newCap);
    vec->capacity = newCap;
}

void Vec_set_str(void* vecPtr, int64_t index, SeenString value) {
    SeenVecStr* vec = (SeenVecStr*)vecPtr;
    if (index < 0 || index >= vec->length) return;
    vec->data[index] = value;
}

SeenString Vec_pop_str(void* vecPtr) {
    SeenVecStr* vec = (SeenVecStr*)vecPtr;
    if (vec->length == 0) {
        SeenString empty = {0, NULL};
        return empty;
    }
    vec->length--;
    return vec->data[vec->length];
}

void Vec_clear_str(void* vecPtr) {
    SeenVecStr* vec = (SeenVecStr*)vecPtr;
    vec->length = 0;
}

// Vec<String> utility methods

int64_t Vec_contains_str(void* vecPtr, SeenString value) {
    SeenVecStr* vec = (SeenVecStr*)vecPtr;
    for (int64_t i = 0; i < vec->length; i++) {
        if (seen_str_eq_ss(vec->data[i], value)) return 1;
    }
    return 0;
}

int64_t Vec_indexOf_str(void* vecPtr, SeenString value) {
    SeenVecStr* vec = (SeenVecStr*)vecPtr;
    for (int64_t i = 0; i < vec->length; i++) {
        if (seen_str_eq_ss(vec->data[i], value)) return i;
    }
    return -1;
}

void Vec_reverse_str(void* vecPtr) {
    SeenVecStr* vec = (SeenVecStr*)vecPtr;
    int64_t left = 0, right = vec->length - 1;
    while (left < right) {
        SeenString tmp = vec->data[left];
        vec->data[left] = vec->data[right];
        vec->data[right] = tmp;
        left++;
        right--;
    }
}

SeenString Vec_remove_str(void* vecPtr, int64_t index) {
    SeenVecStr* vec = (SeenVecStr*)vecPtr;
    if (index < 0 || index >= vec->length) {
        SeenString empty = {0, NULL};
        return empty;
    }
    SeenString removed = vec->data[index];
    for (int64_t i = index; i < vec->length - 1; i++) {
        vec->data[i] = vec->data[i + 1];
    }
    vec->length--;
    return removed;
}

void Vec_insert_str(void* vecPtr, int64_t index, SeenString value) {
    SeenVecStr* vec = (SeenVecStr*)vecPtr;
    if (index < 0 || index > vec->length) return;
    if (vec->length >= vec->capacity) {
        vec->capacity *= 2;
        vec->data = realloc(vec->data, sizeof(SeenString) * vec->capacity);
    }
    for (int64_t i = vec->length; i > index; i--) {
        vec->data[i] = vec->data[i - 1];
    }
    vec->data[index] = value;
    vec->length++;
}

SeenString Vec_first_str(void* vecPtr) {
    SeenVecStr* vec = (SeenVecStr*)vecPtr;
    if (vec->length == 0) {
        SeenString empty = {0, NULL};
        return empty;
    }
    return vec->data[0];
}

SeenString Vec_last_str(void* vecPtr) {
    SeenVecStr* vec = (SeenVecStr*)vecPtr;
    if (vec->length == 0) {
        SeenString empty = {0, NULL};
        return empty;
    }
    return vec->data[vec->length - 1];
}

// ============================================================================
// Vec with Float elements
// Vec_float stores double values (8 bytes each)
// ============================================================================

typedef struct {
    double* data;
    int64_t length;
    int64_t capacity;
} SeenVecFloat;

void* Vec_new_float(void) {
    SeenVecFloat* vec = (SeenVecFloat*)malloc(sizeof(SeenVecFloat));
    vec->capacity = 8;
    vec->data = (double*)malloc(sizeof(double) * vec->capacity);
    vec->length = 0;
    return vec;
}

void Vec_push_float(void* vecPtr, double value) {
    SeenVecFloat* vec = (SeenVecFloat*)vecPtr;
    if (vec->length >= vec->capacity) {
        vec->capacity *= 2;
        vec->data = realloc(vec->data, sizeof(double) * vec->capacity);
    }
    vec->data[vec->length++] = value;
}

double Vec_get_float(void* vecPtr, int64_t index) {
    SeenVecFloat* vec = (SeenVecFloat*)vecPtr;
    if (index < 0 || index >= vec->length) return 0.0;
    return vec->data[index];
}

void Vec_set_float(void* vecPtr, int64_t index, double value) {
    SeenVecFloat* vec = (SeenVecFloat*)vecPtr;
    if (index < 0 || index >= vec->length) return;
    vec->data[index] = value;
}

double Vec_pop_float(void* vecPtr) {
    SeenVecFloat* vec = (SeenVecFloat*)vecPtr;
    if (vec->length == 0) return 0.0;
    vec->length--;
    return vec->data[vec->length];
}

void Vec_clear_float(void* vecPtr) {
    SeenVecFloat* vec = (SeenVecFloat*)vecPtr;
    vec->length = 0;
}

int64_t Vec_len_float(void* vecPtr) {
    SeenVecFloat* vec = (SeenVecFloat*)vecPtr;
    return vec->length;
}

int64_t Vec_capacity_float(void* vecPtr) {
    SeenVecFloat* vec = (SeenVecFloat*)vecPtr;
    return vec->capacity;
}

void Vec_ensureCapacity_float(void* vecPtr, int64_t capacity) {
    SeenVecFloat* vec = (SeenVecFloat*)vecPtr;
    if (capacity <= vec->capacity) return;
    int64_t newCap = vec->capacity == 0 ? 8 : vec->capacity;
    while (newCap < capacity) {
        newCap *= 2;
    }
    vec->data = (double*)realloc(vec->data, sizeof(double) * newCap);
    vec->capacity = newCap;
}

int64_t Vec_contains_float(void* vecPtr, double value) {
    SeenVecFloat* vec = (SeenVecFloat*)vecPtr;
    for (int64_t i = 0; i < vec->length; i++) {
        if (vec->data[i] == value) return 1;
    }
    return 0;
}

int64_t Vec_indexOf_float(void* vecPtr, double value) {
    SeenVecFloat* vec = (SeenVecFloat*)vecPtr;
    for (int64_t i = 0; i < vec->length; i++) {
        if (vec->data[i] == value) return i;
    }
    return -1;
}

void Vec_reverse_float(void* vecPtr) {
    SeenVecFloat* vec = (SeenVecFloat*)vecPtr;
    int64_t left = 0, right = vec->length - 1;
    while (left < right) {
        double tmp = vec->data[left];
        vec->data[left] = vec->data[right];
        vec->data[right] = tmp;
        left++;
        right--;
    }
}

double Vec_remove_float(void* vecPtr, int64_t index) {
    SeenVecFloat* vec = (SeenVecFloat*)vecPtr;
    if (index < 0 || index >= vec->length) return 0.0;
    double removed = vec->data[index];
    for (int64_t i = index; i < vec->length - 1; i++) {
        vec->data[i] = vec->data[i + 1];
    }
    vec->length--;
    return removed;
}

void Vec_insert_float(void* vecPtr, int64_t index, double value) {
    SeenVecFloat* vec = (SeenVecFloat*)vecPtr;
    if (index < 0 || index > vec->length) return;
    if (vec->length >= vec->capacity) {
        vec->capacity *= 2;
        vec->data = realloc(vec->data, sizeof(double) * vec->capacity);
    }
    for (int64_t i = vec->length; i > index; i--) {
        vec->data[i] = vec->data[i - 1];
    }
    vec->data[index] = value;
    vec->length++;
}

double Vec_first_float(void* vecPtr) {
    SeenVecFloat* vec = (SeenVecFloat*)vecPtr;
    if (vec->length == 0) return 0.0;
    return vec->data[0];
}

double Vec_last_float(void* vecPtr) {
    SeenVecFloat* vec = (SeenVecFloat*)vecPtr;
    if (vec->length == 0) return 0.0;
    return vec->data[vec->length - 1];
}

static void Vec_sort_float_helper(double* data, int64_t low, int64_t high) {
    if (low >= high) return;
    double pivot = data[high];
    int64_t i = low - 1;
    for (int64_t j = low; j < high; j++) {
        if (data[j] <= pivot) {
            i++;
            double tmp = data[i];
            data[i] = data[j];
            data[j] = tmp;
        }
    }
    double tmp = data[i + 1];
    data[i + 1] = data[high];
    data[high] = tmp;
    int64_t pi = i + 1;
    Vec_sort_float_helper(data, low, pi - 1);
    Vec_sort_float_helper(data, pi + 1, high);
}

void Vec_sort_float(void* vecPtr) {
    SeenVecFloat* vec = (SeenVecFloat*)vecPtr;
    if (vec->length <= 1) return;
    Vec_sort_float_helper(vec->data, 0, vec->length - 1);
}

// String.split() heap-allocated wrapper
void* String_split(SeenString text, SeenString delimiter) {
    SeenArray result = split(text, delimiter);
    SeenArray* heap = (SeenArray*)malloc(sizeof(SeenArray));
    *heap = result;
    return heap;
}

// ============================================================================
// BTreeMap with String keys
// BTreeMap_str uses strcmp for key comparison (SeenString keys, i64 values)
// ============================================================================

typedef struct {
    SeenString key;
    int64_t value;
} BTreeEntryStr;

typedef struct {
    BTreeEntryStr* entries;
    int64_t capacity;
    int64_t length;
} SeenBTreeMapStr;

void* BTreeMap_new_str(void) {
    SeenBTreeMapStr* map = (SeenBTreeMapStr*)malloc(sizeof(SeenBTreeMapStr));
    map->capacity = 16;
    map->entries = (BTreeEntryStr*)malloc(sizeof(BTreeEntryStr) * map->capacity);
    map->length = 0;
    return map;
}

bool BTreeMap_insert_str(void* mapPtr, SeenString key, int64_t value) {
    SeenBTreeMapStr* map = (SeenBTreeMapStr*)mapPtr;
    // Check if key exists
    for (int64_t i = 0; i < map->length; i++) {
        if (map->entries[i].key.len == key.len &&
            memcmp(map->entries[i].key.data, key.data, key.len) == 0) {
            map->entries[i].value = value;
            return false;  // Updated existing
        }
    }
    // Grow if needed
    if (map->length >= map->capacity) {
        map->capacity *= 2;
        map->entries = realloc(map->entries, sizeof(BTreeEntryStr) * map->capacity);
    }
    map->entries[map->length].key = key;
    map->entries[map->length].value = value;
    map->length++;
    return true;  // New entry
}

int64_t BTreeMap_get_str(void* mapPtr, SeenString key) {
    SeenBTreeMapStr* map = (SeenBTreeMapStr*)mapPtr;
    for (int64_t i = 0; i < map->length; i++) {
        if (map->entries[i].key.len == key.len &&
            memcmp(map->entries[i].key.data, key.data, key.len) == 0) {
            return BTreeMap_make_some(map->entries[i].value);
        }
    }
    return BTreeMap_make_none();
}

bool BTreeMap_containsKey_str(void* mapPtr, SeenString key) {
    SeenBTreeMapStr* map = (SeenBTreeMapStr*)mapPtr;
    for (int64_t i = 0; i < map->length; i++) {
        if (map->entries[i].key.len == key.len &&
            memcmp(map->entries[i].key.data, key.data, key.len) == 0) {
            return true;
        }
    }
    return false;
}

int64_t BTreeMap_keys_str(void* mapPtr) {
    SeenBTreeMapStr* map = (SeenBTreeMapStr*)mapPtr;
    SeenVecStr* vec = Vec_new_str();
    for (int64_t i = 0; i < map->length; i++) {
        Vec_push_str(vec, map->entries[i].key);
    }
    return (int64_t)vec;
}

int64_t BTreeMap_values_str(void* mapPtr) {
    SeenBTreeMapStr* map = (SeenBTreeMapStr*)mapPtr;
    SeenVec* vec = Vec_new();
    for (int64_t i = 0; i < map->length; i++) {
        Vec_push(vec, map->entries[i].value);
    }
    return (int64_t)vec;
}

int64_t BTreeMap_size_str(void* mapPtr) {
    SeenBTreeMapStr* map = (SeenBTreeMapStr*)mapPtr;
    return map->length;
}

int64_t BTreeMap_remove_str(void* mapPtr, SeenString key) {
    SeenBTreeMapStr* map = (SeenBTreeMapStr*)mapPtr;
    for (int64_t i = 0; i < map->length; i++) {
        if (map->entries[i].key.len == key.len &&
            memcmp(map->entries[i].key.data, key.data, key.len) == 0) {
            int64_t value = map->entries[i].value;
            // Shift remaining entries
            for (int64_t j = i; j < map->length - 1; j++) {
                map->entries[j] = map->entries[j + 1];
            }
            map->length--;
            return BTreeMap_make_some(value);
        }
    }
    return BTreeMap_make_none();
}

void BTreeMap_clear_str(void* mapPtr) {
    SeenBTreeMapStr* map = (SeenBTreeMapStr*)mapPtr;
    map->length = 0;
}

// ============================================================================
// BTreeMap with String keys AND String values
// BTreeMap_str_str for BTreeMap<String, String> (used by VDOM)
// ============================================================================

typedef struct {
    SeenString key;
    SeenString value;
} BTreeEntryStrStr;

typedef struct {
    BTreeEntryStrStr* entries;
    int64_t capacity;
    int64_t length;
} SeenBTreeMapStrStr;

void* BTreeMap_new_str_str(void) {
    SeenBTreeMapStrStr* map = (SeenBTreeMapStrStr*)malloc(sizeof(SeenBTreeMapStrStr));
    map->capacity = 16;
    map->entries = (BTreeEntryStrStr*)malloc(sizeof(BTreeEntryStrStr) * map->capacity);
    map->length = 0;
    return map;
}

bool BTreeMap_insert_str_str(void* mapPtr, SeenString key, SeenString value) {
    SeenBTreeMapStrStr* map = (SeenBTreeMapStrStr*)mapPtr;
    // Check if key exists
    for (int64_t i = 0; i < map->length; i++) {
        if (map->entries[i].key.len == key.len &&
            memcmp(map->entries[i].key.data, key.data, key.len) == 0) {
            map->entries[i].value = value;
            return false;  // Updated existing
        }
    }
    // Grow if needed
    if (map->length >= map->capacity) {
        map->capacity *= 2;
        map->entries = realloc(map->entries, sizeof(BTreeEntryStrStr) * map->capacity);
    }
    map->entries[map->length].key = key;
    map->entries[map->length].value = value;
    map->length++;
    return true;  // New entry
}

// Helper: create Option<String> as Some(value)
static int64_t BTreeMap_make_some_str(SeenString value) {
    // Option<String> layout: { i1 hasValue (8 bytes with padding), SeenString value (16 bytes) } = 24 bytes
    uint8_t* opt = (uint8_t*)malloc(24);
    *opt = 1;  // hasValue = true
    *(SeenString*)(opt + 8) = value;  // Store SeenString at offset 8
    return (int64_t)opt;
}

static int64_t BTreeMap_make_none_str(void) {
    uint8_t* opt = (uint8_t*)malloc(24);
    *opt = 0;  // hasValue = false
    // Leave value uninitialized (will be checked via hasValue before access)
    return (int64_t)opt;
}

// Returns Option<String> as i64 (pointer to { i1, SeenString })
int64_t BTreeMap_get_str_str(void* mapPtr, SeenString key) {
    SeenBTreeMapStrStr* map = (SeenBTreeMapStrStr*)mapPtr;
    for (int64_t i = 0; i < map->length; i++) {
        if (map->entries[i].key.len == key.len &&
            memcmp(map->entries[i].key.data, key.data, key.len) == 0) {
            return BTreeMap_make_some_str(map->entries[i].value);
        }
    }
    return BTreeMap_make_none_str();
}

bool BTreeMap_containsKey_str_str(void* mapPtr, SeenString key) {
    SeenBTreeMapStrStr* map = (SeenBTreeMapStrStr*)mapPtr;
    for (int64_t i = 0; i < map->length; i++) {
        if (map->entries[i].key.len == key.len &&
            memcmp(map->entries[i].key.data, key.data, key.len) == 0) {
            return true;
        }
    }
    return false;
}

int64_t BTreeMap_keys_str_str(void* mapPtr) {
    SeenBTreeMapStrStr* map = (SeenBTreeMapStrStr*)mapPtr;
    SeenVecStr* vec = Vec_new_str();
    for (int64_t i = 0; i < map->length; i++) {
        Vec_push_str(vec, map->entries[i].key);
    }
    return (int64_t)vec;
}

int64_t BTreeMap_values_str_str(void* mapPtr) {
    SeenBTreeMapStrStr* map = (SeenBTreeMapStrStr*)mapPtr;
    SeenVecStr* vec = Vec_new_str();
    for (int64_t i = 0; i < map->length; i++) {
        Vec_push_str(vec, map->entries[i].value);
    }
    return (int64_t)vec;
}

int64_t BTreeMap_size_str_str(void* mapPtr) {
    SeenBTreeMapStrStr* map = (SeenBTreeMapStrStr*)mapPtr;
    return map->length;
}

// Returns Option<String> as i64 (pointer to { i1, SeenString })
int64_t BTreeMap_remove_str_str(void* mapPtr, SeenString key) {
    SeenBTreeMapStrStr* map = (SeenBTreeMapStrStr*)mapPtr;
    for (int64_t i = 0; i < map->length; i++) {
        if (map->entries[i].key.len == key.len &&
            memcmp(map->entries[i].key.data, key.data, key.len) == 0) {
            SeenString value = map->entries[i].value;
            // Shift remaining entries
            for (int64_t j = i; j < map->length - 1; j++) {
                map->entries[j] = map->entries[j + 1];
            }
            map->length--;
            return BTreeMap_make_some_str(value);
        }
    }
    return BTreeMap_make_none_str();
}

void BTreeMap_clear_str_str(void* mapPtr) {
    SeenBTreeMapStrStr* map = (SeenBTreeMapStrStr*)mapPtr;
    map->length = 0;
}

// ============================================================================
// BTreeMap<Int, String> Runtime
// ============================================================================

typedef struct {
    int64_t key;
    SeenString value;
} BTreeEntryIntStr;

typedef struct {
    BTreeEntryIntStr* entries;
    int64_t capacity;
    int64_t length;
} SeenBTreeMapIntStr;

void* BTreeMap_new_int_str(void) {
    SeenBTreeMapIntStr* map = (SeenBTreeMapIntStr*)malloc(sizeof(SeenBTreeMapIntStr));
    map->capacity = 16;
    map->entries = (BTreeEntryIntStr*)malloc(sizeof(BTreeEntryIntStr) * map->capacity);
    map->length = 0;
    return map;
}

bool BTreeMap_insert_int_str(void* mapPtr, int64_t key, SeenString value) {
    SeenBTreeMapIntStr* map = (SeenBTreeMapIntStr*)mapPtr;
    for (int64_t i = 0; i < map->length; i++) {
        if (map->entries[i].key == key) {
            map->entries[i].value = value;
            return false;
        }
    }
    if (map->length >= map->capacity) {
        map->capacity *= 2;
        map->entries = realloc(map->entries, sizeof(BTreeEntryIntStr) * map->capacity);
    }
    map->entries[map->length].key = key;
    map->entries[map->length].value = value;
    map->length++;
    return true;
}

int64_t BTreeMap_get_int_str(void* mapPtr, int64_t key) {
    SeenBTreeMapIntStr* map = (SeenBTreeMapIntStr*)mapPtr;
    for (int64_t i = 0; i < map->length; i++) {
        if (map->entries[i].key == key) {
            return BTreeMap_make_some_str(map->entries[i].value);
        }
    }
    return BTreeMap_make_none_str();
}

bool BTreeMap_containsKey_int_str(void* mapPtr, int64_t key) {
    SeenBTreeMapIntStr* map = (SeenBTreeMapIntStr*)mapPtr;
    for (int64_t i = 0; i < map->length; i++) {
        if (map->entries[i].key == key) return true;
    }
    return false;
}

int64_t BTreeMap_keys_int_str(void* mapPtr) {
    SeenBTreeMapIntStr* map = (SeenBTreeMapIntStr*)mapPtr;
    SeenVec* vec = Vec_new();
    for (int64_t i = 0; i < map->length; i++) {
        Vec_push(vec, map->entries[i].key);
    }
    return (int64_t)vec;
}

int64_t BTreeMap_values_int_str(void* mapPtr) {
    SeenBTreeMapIntStr* map = (SeenBTreeMapIntStr*)mapPtr;
    SeenVecStr* vec = Vec_new_str();
    for (int64_t i = 0; i < map->length; i++) {
        Vec_push_str(vec, map->entries[i].value);
    }
    return (int64_t)vec;
}

int64_t BTreeMap_size_int_str(void* mapPtr) {
    SeenBTreeMapIntStr* map = (SeenBTreeMapIntStr*)mapPtr;
    return map->length;
}

int64_t BTreeMap_remove_int_str(void* mapPtr, int64_t key) {
    SeenBTreeMapIntStr* map = (SeenBTreeMapIntStr*)mapPtr;
    for (int64_t i = 0; i < map->length; i++) {
        if (map->entries[i].key == key) {
            SeenString value = map->entries[i].value;
            for (int64_t j = i; j < map->length - 1; j++) {
                map->entries[j] = map->entries[j + 1];
            }
            map->length--;
            return BTreeMap_make_some_str(value);
        }
    }
    return BTreeMap_make_none_str();
}

void BTreeMap_clear_int_str(void* mapPtr) {
    SeenBTreeMapIntStr* map = (SeenBTreeMapIntStr*)mapPtr;
    map->length = 0;
}

// ============================================================================
// Store Framework Runtime (for @store decorator)
// ============================================================================

#define SEEN_MAX_MUTATIONS 65536
#define SEEN_MAX_SNAPSHOTS 1024

// Mutation entry for replay
typedef struct {
    int64_t frame_id;
    int64_t timestamp;
    char method_name[64];
    char field_name[64];
} SeenMutationEntry;

// Snapshot entry
typedef struct {
    int64_t frame_id;
    int64_t timestamp;
    int mutation_index;
} SeenSnapshotEntry;

// Global store registry
static SeenMutationEntry g_mutations[SEEN_MAX_MUTATIONS];
static int g_mutation_count = 0;
static SeenSnapshotEntry g_snapshots[SEEN_MAX_SNAPSHOTS];
static int g_snapshot_count = 0;
static int64_t g_store_frame = 0;
static int64_t g_store_timestamp = 0;
static int g_store_initialized = 0;

static void __seen_store_init(void) {
    if (g_store_initialized) return;
    g_store_initialized = 1;
    g_mutation_count = 0;
    g_snapshot_count = 0;
    g_store_frame = 0;
    g_store_timestamp = 0;
}

// Log a mutation
void __seen_store_log_mutation(SeenString method_name, SeenString field_name) {
    __seen_store_init();

    if (g_mutation_count >= SEEN_MAX_MUTATIONS) {
        fprintf(stderr, "WARNING: Store mutation log full (max %d)\n", SEEN_MAX_MUTATIONS);
        return;
    }

    SeenMutationEntry* entry = &g_mutations[g_mutation_count++];
    entry->frame_id = g_store_frame;
    entry->timestamp = g_store_timestamp++;

    // Copy method name
    if (method_name.data && method_name.len > 0) {
        int copy_len = (method_name.len < 63) ? method_name.len : 63;
        memcpy(entry->method_name, method_name.data, copy_len);
        entry->method_name[copy_len] = '\0';
    } else {
        entry->method_name[0] = '\0';
    }

    // Copy field name
    if (field_name.data && field_name.len > 0) {
        int copy_len = (field_name.len < 63) ? field_name.len : 63;
        memcpy(entry->field_name, field_name.data, copy_len);
        entry->field_name[copy_len] = '\0';
    } else {
        entry->field_name[0] = '\0';
    }
}

// Take a snapshot
void __seen_store_take_snapshot(void) {
    __seen_store_init();

    if (g_snapshot_count >= SEEN_MAX_SNAPSHOTS) {
        fprintf(stderr, "WARNING: Store snapshot log full (max %d)\n", SEEN_MAX_SNAPSHOTS);
        return;
    }

    SeenSnapshotEntry* entry = &g_snapshots[g_snapshot_count++];
    entry->frame_id = g_store_frame;
    entry->timestamp = g_store_timestamp;
    entry->mutation_index = g_mutation_count;
}

// Get mutation count
int64_t __seen_store_get_mutation_count(void) {
    return g_mutation_count;
}

// Set current frame
void __seen_store_set_frame(int64_t frame) {
    __seen_store_init();
    g_store_frame = frame;
}

// ============================================================================
// Middleware Framework Runtime (for @middleware_stack decorator)
// ============================================================================

static int64_t g_middleware_invocation_count = 0;

// Increment middleware invocation counter
void __seen_middleware_increment_count(void) {
    g_middleware_invocation_count++;
}

// Get middleware invocation count
int64_t __seen_middleware_get_count(void) {
    return g_middleware_invocation_count;
}

// ============================================================================
// CPU Feature Detection (for SIMD multi-versioned codegen)
// ============================================================================

typedef struct {
    int sse2;
    int sse3;
    int ssse3;
    int sse41;
    int sse42;
    int avx;
    int avx2;
    int fma;
    int bmi1;
    int bmi2;
    int avx512f;
    int avx512bw;
    int avx512dq;
    int avx512vl;
    int neon;
    int sve;
    int apx;
    int avx10;
    int detected;
} SeenCPUFeatures;

static SeenCPUFeatures g_cpu_features = {0};

#if defined(__x86_64__) || defined(_M_X64) || defined(__i386__)
#include <cpuid.h>

static void seen_cpu_detect_impl(void) {
    if (g_cpu_features.detected) return;
    g_cpu_features.detected = 1;

    unsigned int eax, ebx, ecx, edx;

    // Basic features (leaf 1)
    if (__get_cpuid(1, &eax, &ebx, &ecx, &edx)) {
        g_cpu_features.sse2  = (edx >> 26) & 1;
        g_cpu_features.sse3  = (ecx >> 0)  & 1;
        g_cpu_features.ssse3 = (ecx >> 9)  & 1;
        g_cpu_features.sse41 = (ecx >> 19) & 1;
        g_cpu_features.sse42 = (ecx >> 20) & 1;
        g_cpu_features.avx   = (ecx >> 28) & 1;
        g_cpu_features.fma   = (ecx >> 12) & 1;
    }

    // Extended features (leaf 7, subleaf 0)
    if (__get_cpuid_count(7, 0, &eax, &ebx, &ecx, &edx)) {
        g_cpu_features.avx2     = (ebx >> 5)  & 1;
        g_cpu_features.bmi1     = (ebx >> 3)  & 1;
        g_cpu_features.bmi2     = (ebx >> 8)  & 1;
        g_cpu_features.avx512f  = (ebx >> 16) & 1;
        g_cpu_features.avx512bw = (ebx >> 30) & 1;
        g_cpu_features.avx512dq = (ebx >> 17) & 1;
        g_cpu_features.avx512vl = (ebx >> 31) & 1;
    }

    // Extended features (leaf 7, subleaf 1) — APX and AVX10
    if (__get_cpuid_count(7, 1, &eax, &ebx, &ecx, &edx)) {
        g_cpu_features.avx10 = (edx >> 19) & 1;
        g_cpu_features.apx   = (edx >> 21) & 1;
    }
}

#elif defined(__aarch64__)

#if defined(__linux__)
#include <sys/auxv.h>
#include <asm/hwcap.h>
static void seen_cpu_detect_impl(void) {
    if (g_cpu_features.detected) return;
    g_cpu_features.detected = 1;

    unsigned long hwcap = getauxval(AT_HWCAP);
    g_cpu_features.neon = 1; // Always available on AArch64
    g_cpu_features.sve  = (hwcap & HWCAP_SVE) ? 1 : 0;
}
#elif defined(__APPLE__)
static void seen_cpu_detect_impl(void) {
    if (g_cpu_features.detected) return;
    g_cpu_features.detected = 1;
    g_cpu_features.neon = 1;  // Always available on Apple Silicon
    g_cpu_features.sve = 0;   // Apple Silicon does not support SVE
}
#else
static void seen_cpu_detect_impl(void) {
    if (g_cpu_features.detected) return;
    g_cpu_features.detected = 1;
    g_cpu_features.neon = 1;  // Assume NEON on generic AArch64
}
#endif

#else
// Fallback: no hardware detection
static void seen_cpu_detect_impl(void) {
    if (g_cpu_features.detected) return;
    g_cpu_features.detected = 1;
}
#endif

// Auto-detect at program startup
__attribute__((constructor))
static void seen_cpu_auto_detect(void) {
    seen_cpu_detect_impl();
}

void seen_cpu_detect(void) {
    seen_cpu_detect_impl();
}

int64_t seen_cpu_has_feature(SeenString name) {
    seen_cpu_detect_impl();
    if (name.len == 0 || !name.data) return 0;

    // Match feature names
    if (name.len == 4 && memcmp(name.data, "sse2", 4) == 0) return g_cpu_features.sse2;
    if (name.len == 4 && memcmp(name.data, "sse3", 4) == 0) return g_cpu_features.sse3;
    if (name.len == 5 && memcmp(name.data, "ssse3", 5) == 0) return g_cpu_features.ssse3;
    if (name.len == 5 && memcmp(name.data, "sse41", 5) == 0) return g_cpu_features.sse41;
    if (name.len == 6 && memcmp(name.data, "sse4.1", 6) == 0) return g_cpu_features.sse41;
    if (name.len == 5 && memcmp(name.data, "sse42", 5) == 0) return g_cpu_features.sse42;
    if (name.len == 6 && memcmp(name.data, "sse4.2", 6) == 0) return g_cpu_features.sse42;
    if (name.len == 3 && memcmp(name.data, "avx", 3) == 0) return g_cpu_features.avx;
    if (name.len == 4 && memcmp(name.data, "avx2", 4) == 0) return g_cpu_features.avx2;
    if (name.len == 3 && memcmp(name.data, "fma", 3) == 0) return g_cpu_features.fma;
    if (name.len == 4 && memcmp(name.data, "bmi1", 4) == 0) return g_cpu_features.bmi1;
    if (name.len == 4 && memcmp(name.data, "bmi2", 4) == 0) return g_cpu_features.bmi2;
    if (name.len == 7 && memcmp(name.data, "avx512f", 7) == 0) return g_cpu_features.avx512f;
    if (name.len == 8 && memcmp(name.data, "avx512bw", 8) == 0) return g_cpu_features.avx512bw;
    if (name.len == 8 && memcmp(name.data, "avx512dq", 8) == 0) return g_cpu_features.avx512dq;
    if (name.len == 8 && memcmp(name.data, "avx512vl", 8) == 0) return g_cpu_features.avx512vl;
    if (name.len == 4 && memcmp(name.data, "neon", 4) == 0) return g_cpu_features.neon;
    if (name.len == 3 && memcmp(name.data, "sve", 3) == 0) return g_cpu_features.sve;
    if (name.len == 3 && memcmp(name.data, "apx", 3) == 0) return g_cpu_features.apx;
    if (name.len == 5 && memcmp(name.data, "avx10", 5) == 0) return g_cpu_features.avx10;

    return 0;
}

// Returns SIMD tier: 0=scalar, 1=SSE4.2, 2=AVX2, 3=AVX-512, 4=NEON, 5=SVE
int64_t seen_cpu_simd_tier(void) {
    seen_cpu_detect_impl();

#if defined(__aarch64__)
    if (g_cpu_features.sve) return 5;
    if (g_cpu_features.neon) return 4;
    return 0;
#else
    if (g_cpu_features.avx512f && g_cpu_features.avx512bw &&
        g_cpu_features.avx512dq && g_cpu_features.avx512vl) return 3;
    if (g_cpu_features.avx2 && g_cpu_features.fma) return 2;
    if (g_cpu_features.sse42) return 1;
    return 0;
#endif
}

// ============================================================================
// 32-bit Index Arena Allocator
// ============================================================================

typedef struct SeenArena {
    char* base;
    uint32_t offset;
    uint32_t capacity;
} SeenArena;

void* seen_arena_new(int64_t capacity) {
    SeenArena* arena = (SeenArena*)malloc(sizeof(SeenArena));
    if (!arena) return NULL;
    arena->base = (char*)malloc((size_t)capacity);
    if (!arena->base) { free(arena); return NULL; }
    arena->offset = 0;
    arena->capacity = (uint32_t)capacity;
    return (void*)arena;
}

// Returns 32-bit index into arena, or 0xFFFFFFFF on failure
int64_t seen_arena_alloc(void* arenaPtr, int64_t size) {
    SeenArena* arena = (SeenArena*)arenaPtr;
    if (!arena || size <= 0) return (int64_t)0xFFFFFFFF;

    // Align to 8 bytes
    uint32_t aligned = (uint32_t)((size + 7) & ~7);
    if (arena->offset + aligned > arena->capacity) return (int64_t)0xFFFFFFFF;

    uint32_t idx = arena->offset;
    arena->offset += aligned;
    return (int64_t)idx;
}

// Convert 32-bit index to pointer
void* seen_arena_get(void* arenaPtr, int64_t index) {
    SeenArena* arena = (SeenArena*)arenaPtr;
    if (!arena || index < 0 || (uint32_t)index >= arena->offset) return NULL;
    return arena->base + (uint32_t)index;
}

void seen_arena_reset(void* arenaPtr) {
    SeenArena* arena = (SeenArena*)arenaPtr;
    if (arena) arena->offset = 0;
}

void seen_arena_free(void* arenaPtr) {
    SeenArena* arena = (SeenArena*)arenaPtr;
    if (!arena) return;
    free(arena->base);
    free(arena);
}

int64_t seen_arena_used(void* arenaPtr) {
    SeenArena* arena = (SeenArena*)arenaPtr;
    return arena ? (int64_t)arena->offset : 0;
}

int64_t seen_arena_remaining(void* arenaPtr) {
    SeenArena* arena = (SeenArena*)arenaPtr;
    return arena ? (int64_t)(arena->capacity - arena->offset) : 0;
}

// ============================================================================
// Cache-Oblivious Layout Probes
// ============================================================================

#define SEEN_PROBE_SAMPLE_RATE 64
#define SEEN_PROBE_CACHE_LINE 64

typedef struct SeenLayoutProbe {
    const char* tag;
    uint64_t access_count;
    uint64_t sampled_count;
    uint64_t sequential_count;  // stride-1 accesses
    uint64_t random_count;      // non-sequential accesses
    void* last_addr;
} SeenLayoutProbe;

void* seen_probe_new(SeenString tag) {
    SeenLayoutProbe* probe = (SeenLayoutProbe*)calloc(1, sizeof(SeenLayoutProbe));
    if (!probe) return NULL;
    // Copy tag string
    char* tag_copy = (char*)malloc(tag.len + 1);
    memcpy(tag_copy, tag.data, tag.len);
    tag_copy[tag.len] = '\0';
    probe->tag = tag_copy;
    return probe;
}

void seen_probe_access(void* probePtr, void* addr) {
    SeenLayoutProbe* probe = (SeenLayoutProbe*)probePtr;
    if (!probe) return;
    probe->access_count++;

    // Sample every Nth access to estimate cache behavior
    if ((probe->access_count & (SEEN_PROBE_SAMPLE_RATE - 1)) == 0) {
        probe->sampled_count++;
        if (probe->last_addr) {
            ptrdiff_t diff = (char*)addr - (char*)probe->last_addr;
            if (diff < 0) diff = -diff;
            if (diff <= SEEN_PROBE_CACHE_LINE) {
                probe->sequential_count++;
            } else {
                probe->random_count++;
            }
        }
        probe->last_addr = addr;
    }
}

void seen_probe_report(void* probePtr) {
    SeenLayoutProbe* probe = (SeenLayoutProbe*)probePtr;
    if (!probe) return;

    double seq_pct = 0.0;
    if (probe->sampled_count > 0) {
        seq_pct = (double)probe->sequential_count / (double)probe->sampled_count * 100.0;
    }
    fprintf(stderr, "[LayoutProbe '%s'] accesses=%lu sampled=%lu sequential=%.1f%% random=%.1f%%\n",
            probe->tag ? probe->tag : "?",
            (unsigned long)probe->access_count,
            (unsigned long)probe->sampled_count,
            seq_pct, 100.0 - seq_pct);
}

void seen_probe_free(void* probePtr) {
    SeenLayoutProbe* probe = (SeenLayoutProbe*)probePtr;
    if (!probe) return;
    free((void*)probe->tag);
    free(probe);
}

// ============================================================================
// Perf Dashboard JSON Export
// ============================================================================

void seen_perf_export_json(SeenString path) {
    seen_cpu_detect_impl();

    FILE* f = fopen(path.data ? path.data : "seen_perf.json", "w");
    if (!f) return;

    fprintf(f, "{\n");

    // CPU features
    fprintf(f, "  \"cpu_features\": {\n");
    fprintf(f, "    \"sse2\": %d,\n", g_cpu_features.sse2);
    fprintf(f, "    \"sse3\": %d,\n", g_cpu_features.sse3);
    fprintf(f, "    \"ssse3\": %d,\n", g_cpu_features.ssse3);
    fprintf(f, "    \"sse4.1\": %d,\n", g_cpu_features.sse41);
    fprintf(f, "    \"sse4.2\": %d,\n", g_cpu_features.sse42);
    fprintf(f, "    \"avx\": %d,\n", g_cpu_features.avx);
    fprintf(f, "    \"avx2\": %d,\n", g_cpu_features.avx2);
    fprintf(f, "    \"fma\": %d,\n", g_cpu_features.fma);
    fprintf(f, "    \"bmi1\": %d,\n", g_cpu_features.bmi1);
    fprintf(f, "    \"bmi2\": %d,\n", g_cpu_features.bmi2);
    fprintf(f, "    \"avx512f\": %d,\n", g_cpu_features.avx512f);
    fprintf(f, "    \"avx512bw\": %d,\n", g_cpu_features.avx512bw);
    fprintf(f, "    \"avx512dq\": %d,\n", g_cpu_features.avx512dq);
    fprintf(f, "    \"avx512vl\": %d,\n", g_cpu_features.avx512vl);
    fprintf(f, "    \"neon\": %d,\n", g_cpu_features.neon);
    fprintf(f, "    \"sve\": %d,\n", g_cpu_features.sve);
    fprintf(f, "    \"apx\": %d,\n", g_cpu_features.apx);
    fprintf(f, "    \"avx10\": %d\n", g_cpu_features.avx10);
    fprintf(f, "  },\n");

    // SIMD tier
    fprintf(f, "  \"simd_tier\": %ld\n", (long)seen_cpu_simd_tier());

    fprintf(f, "}\n");
    fclose(f);
}

// ============================================================================
// SIMD Vector Runtime Functions
// ============================================================================

#if defined(__x86_64__) || defined(_M_X64)
#include <immintrin.h>

// --- 4-wide float (SSE) ---
// SSE2 is baseline for x86_64, SSE3 needed for movehdup

__attribute__((target("sse2")))
void* seen_simd_f4_splat(double val) {
    __m128* r = (__m128*)aligned_alloc(16, sizeof(__m128));
    *r = _mm_set1_ps((float)val);
    return r;
}

__attribute__((target("sse2")))
void* seen_simd_f4_add(void* a, void* b) {
    __m128* r = (__m128*)aligned_alloc(16, sizeof(__m128));
    *r = _mm_add_ps(*(__m128*)a, *(__m128*)b);
    return r;
}

__attribute__((target("sse2")))
void* seen_simd_f4_sub(void* a, void* b) {
    __m128* r = (__m128*)aligned_alloc(16, sizeof(__m128));
    *r = _mm_sub_ps(*(__m128*)a, *(__m128*)b);
    return r;
}

__attribute__((target("sse2")))
void* seen_simd_f4_mul(void* a, void* b) {
    __m128* r = (__m128*)aligned_alloc(16, sizeof(__m128));
    *r = _mm_mul_ps(*(__m128*)a, *(__m128*)b);
    return r;
}

__attribute__((target("sse2")))
void* seen_simd_f4_div(void* a, void* b) {
    __m128* r = (__m128*)aligned_alloc(16, sizeof(__m128));
    *r = _mm_div_ps(*(__m128*)a, *(__m128*)b);
    return r;
}

__attribute__((target("sse2")))
void* seen_simd_f4_min(void* a, void* b) {
    __m128* r = (__m128*)aligned_alloc(16, sizeof(__m128));
    *r = _mm_min_ps(*(__m128*)a, *(__m128*)b);
    return r;
}

__attribute__((target("sse2")))
void* seen_simd_f4_max(void* a, void* b) {
    __m128* r = (__m128*)aligned_alloc(16, sizeof(__m128));
    *r = _mm_max_ps(*(__m128*)a, *(__m128*)b);
    return r;
}

__attribute__((target("sse3")))
double seen_simd_f4_sum(void* a) {
    __m128 v = *(__m128*)a;
    __m128 shuf = _mm_movehdup_ps(v);
    __m128 sums = _mm_add_ps(v, shuf);
    shuf = _mm_movehl_ps(shuf, sums);
    sums = _mm_add_ss(sums, shuf);
    return (double)_mm_cvtss_f32(sums);
}

__attribute__((target("sse3")))
double seen_simd_f4_dot(void* a, void* b) {
    __m128 prod = _mm_mul_ps(*(__m128*)a, *(__m128*)b);
    __m128 shuf = _mm_movehdup_ps(prod);
    __m128 sums = _mm_add_ps(prod, shuf);
    shuf = _mm_movehl_ps(shuf, sums);
    sums = _mm_add_ss(sums, shuf);
    return (double)_mm_cvtss_f32(sums);
}

__attribute__((target("sse2")))
void* seen_simd_f4_load(void* ptr) {
    __m128* r = (__m128*)aligned_alloc(16, sizeof(__m128));
    *r = _mm_loadu_ps((float*)ptr);
    return r;
}

__attribute__((target("sse2")))
void seen_simd_f4_store(void* vec, void* ptr) {
    _mm_storeu_ps((float*)ptr, *(__m128*)vec);
}

// --- 8-wide float (AVX2) ---

__attribute__((target("avx2")))
void* seen_simd_f8_splat(double val) {
    __m256* r = (__m256*)aligned_alloc(32, sizeof(__m256));
    *r = _mm256_set1_ps((float)val);
    return r;
}

__attribute__((target("avx2")))
void* seen_simd_f8_add(void* a, void* b) {
    __m256* r = (__m256*)aligned_alloc(32, sizeof(__m256));
    *r = _mm256_add_ps(*(__m256*)a, *(__m256*)b);
    return r;
}

__attribute__((target("avx2")))
void* seen_simd_f8_sub(void* a, void* b) {
    __m256* r = (__m256*)aligned_alloc(32, sizeof(__m256));
    *r = _mm256_sub_ps(*(__m256*)a, *(__m256*)b);
    return r;
}

__attribute__((target("avx2")))
void* seen_simd_f8_mul(void* a, void* b) {
    __m256* r = (__m256*)aligned_alloc(32, sizeof(__m256));
    *r = _mm256_mul_ps(*(__m256*)a, *(__m256*)b);
    return r;
}

__attribute__((target("avx2")))
void* seen_simd_f8_div(void* a, void* b) {
    __m256* r = (__m256*)aligned_alloc(32, sizeof(__m256));
    *r = _mm256_div_ps(*(__m256*)a, *(__m256*)b);
    return r;
}

__attribute__((target("avx2")))
void* seen_simd_f8_min(void* a, void* b) {
    __m256* r = (__m256*)aligned_alloc(32, sizeof(__m256));
    *r = _mm256_min_ps(*(__m256*)a, *(__m256*)b);
    return r;
}

__attribute__((target("avx2")))
void* seen_simd_f8_max(void* a, void* b) {
    __m256* r = (__m256*)aligned_alloc(32, sizeof(__m256));
    *r = _mm256_max_ps(*(__m256*)a, *(__m256*)b);
    return r;
}

__attribute__((target("avx2,sse3")))
double seen_simd_f8_sum(void* a) {
    __m256 v = *(__m256*)a;
    __m128 lo = _mm256_castps256_ps128(v);
    __m128 hi = _mm256_extractf128_ps(v, 1);
    __m128 sum4 = _mm_add_ps(lo, hi);
    __m128 shuf = _mm_movehdup_ps(sum4);
    __m128 sums = _mm_add_ps(sum4, shuf);
    shuf = _mm_movehl_ps(shuf, sums);
    sums = _mm_add_ss(sums, shuf);
    return (double)_mm_cvtss_f32(sums);
}

__attribute__((target("avx2,sse3")))
double seen_simd_f8_dot(void* a, void* b) {
    __m256 prod = _mm256_mul_ps(*(__m256*)a, *(__m256*)b);
    __m128 lo = _mm256_castps256_ps128(prod);
    __m128 hi = _mm256_extractf128_ps(prod, 1);
    __m128 sum4 = _mm_add_ps(lo, hi);
    __m128 shuf = _mm_movehdup_ps(sum4);
    __m128 sums = _mm_add_ps(sum4, shuf);
    shuf = _mm_movehl_ps(shuf, sums);
    sums = _mm_add_ss(sums, shuf);
    return (double)_mm_cvtss_f32(sums);
}

__attribute__((target("avx2")))
void* seen_simd_f8_load(void* ptr) {
    __m256* r = (__m256*)aligned_alloc(32, sizeof(__m256));
    *r = _mm256_loadu_ps((float*)ptr);
    return r;
}

__attribute__((target("avx2")))
void seen_simd_f8_store(void* vec, void* ptr) {
    _mm256_storeu_ps((float*)ptr, *(__m256*)vec);
}

// --- Auto-dispatch SIMD array operations ---

__attribute__((target("avx2,fma,sse3")))
static double seen_simd_reduce_sum_avx2(float* data, int64_t len) {
    double sum = 0.0;
    int64_t i = 0;
    __m256 acc = _mm256_setzero_ps();
    for (; i + 7 < len; i += 8) {
        __m256 v = _mm256_loadu_ps(data + i);
        acc = _mm256_add_ps(acc, v);
    }
    __m128 lo = _mm256_castps256_ps128(acc);
    __m128 hi = _mm256_extractf128_ps(acc, 1);
    __m128 sum4 = _mm_add_ps(lo, hi);
    __m128 shuf = _mm_movehdup_ps(sum4);
    __m128 sums = _mm_add_ps(sum4, shuf);
    shuf = _mm_movehl_ps(shuf, sums);
    sums = _mm_add_ss(sums, shuf);
    sum = (double)_mm_cvtss_f32(sums);
    for (; i < len; i++) sum += (double)data[i];
    return sum;
}

double seen_simd_reduce_sum(void* arr_data, int64_t len) {
    float* data = (float*)arr_data;
    if (g_cpu_features.avx2 && len >= 8) {
        return seen_simd_reduce_sum_avx2(data, len);
    }
    double sum = 0.0;
    for (int64_t i = 0; i < len; i++) sum += (double)data[i];
    return sum;
}

__attribute__((target("avx2,fma,sse3")))
static double seen_simd_dot_product_avx2(float* a, float* b, int64_t len) {
    double sum = 0.0;
    int64_t i = 0;
    __m256 acc = _mm256_setzero_ps();
    for (; i + 7 < len; i += 8) {
        __m256 va = _mm256_loadu_ps(a + i);
        __m256 vb = _mm256_loadu_ps(b + i);
        acc = _mm256_fmadd_ps(va, vb, acc);
    }
    __m128 lo = _mm256_castps256_ps128(acc);
    __m128 hi = _mm256_extractf128_ps(acc, 1);
    __m128 sum4 = _mm_add_ps(lo, hi);
    __m128 shuf = _mm_movehdup_ps(sum4);
    __m128 sums = _mm_add_ps(sum4, shuf);
    shuf = _mm_movehl_ps(shuf, sums);
    sums = _mm_add_ss(sums, shuf);
    sum = (double)_mm_cvtss_f32(sums);
    for (; i < len; i++) sum += (double)a[i] * (double)b[i];
    return sum;
}

double seen_simd_dot_product(void* a_data, void* b_data, int64_t len) {
    float* a = (float*)a_data;
    float* b = (float*)b_data;
    if (g_cpu_features.avx2 && len >= 8) {
        return seen_simd_dot_product_avx2(a, b, len);
    }
    double sum = 0.0;
    for (int64_t i = 0; i < len; i++) sum += (double)a[i] * (double)b[i];
    return sum;
}

double seen_simd_reduce_min(void* arr_data, int64_t len) {
    if (len <= 0) return 0.0;
    float* data = (float*)arr_data;
    float min_val = data[0];
    for (int64_t i = 1; i < len; i++) {
        if (data[i] < min_val) min_val = data[i];
    }
    return (double)min_val;
}

double seen_simd_reduce_max(void* arr_data, int64_t len) {
    if (len <= 0) return 0.0;
    float* data = (float*)arr_data;
    float max_val = data[0];
    for (int64_t i = 1; i < len; i++) {
        if (data[i] > max_val) max_val = data[i];
    }
    return (double)max_val;
}

void seen_simd_prefix_sum(void* arr_data, int64_t len) {
    float* data = (float*)arr_data;
    for (int64_t i = 1; i < len; i++) {
        data[i] += data[i - 1];
    }
}

#elif defined(__aarch64__)
// ARM NEON implementations for AArch64
#include <arm_neon.h>

// --- 4-wide float (NEON) ---

void* seen_simd_f4_splat(double val) {
    float32x4_t* r = (float32x4_t*)malloc(sizeof(float32x4_t));
    *r = vdupq_n_f32((float)val);
    return r;
}

void* seen_simd_f4_add(void* a, void* b) {
    float32x4_t* r = (float32x4_t*)malloc(sizeof(float32x4_t));
    *r = vaddq_f32(*(float32x4_t*)a, *(float32x4_t*)b);
    return r;
}

void* seen_simd_f4_sub(void* a, void* b) {
    float32x4_t* r = (float32x4_t*)malloc(sizeof(float32x4_t));
    *r = vsubq_f32(*(float32x4_t*)a, *(float32x4_t*)b);
    return r;
}

void* seen_simd_f4_mul(void* a, void* b) {
    float32x4_t* r = (float32x4_t*)malloc(sizeof(float32x4_t));
    *r = vmulq_f32(*(float32x4_t*)a, *(float32x4_t*)b);
    return r;
}

void* seen_simd_f4_div(void* a, void* b) {
    float32x4_t* r = (float32x4_t*)malloc(sizeof(float32x4_t));
    *r = vdivq_f32(*(float32x4_t*)a, *(float32x4_t*)b);
    return r;
}

void* seen_simd_f4_min(void* a, void* b) {
    float32x4_t* r = (float32x4_t*)malloc(sizeof(float32x4_t));
    *r = vminq_f32(*(float32x4_t*)a, *(float32x4_t*)b);
    return r;
}

void* seen_simd_f4_max(void* a, void* b) {
    float32x4_t* r = (float32x4_t*)malloc(sizeof(float32x4_t));
    *r = vmaxq_f32(*(float32x4_t*)a, *(float32x4_t*)b);
    return r;
}

double seen_simd_f4_sum(void* a) {
    float32x4_t v = *(float32x4_t*)a;
    return (double)vaddvq_f32(v);
}

double seen_simd_f4_dot(void* a, void* b) {
    float32x4_t prod = vmulq_f32(*(float32x4_t*)a, *(float32x4_t*)b);
    return (double)vaddvq_f32(prod);
}

void* seen_simd_f4_load(void* ptr) {
    float32x4_t* r = (float32x4_t*)malloc(sizeof(float32x4_t));
    *r = vld1q_f32((float*)ptr);
    return r;
}

void seen_simd_f4_store(void* vec, void* ptr) {
    vst1q_f32((float*)ptr, *(float32x4_t*)vec);
}

// --- 8-wide float (emulated with 2x NEON on AArch64) ---

void* seen_simd_f8_splat(double val) {
    float32x4_t* r = (float32x4_t*)malloc(2 * sizeof(float32x4_t));
    r[0] = vdupq_n_f32((float)val);
    r[1] = vdupq_n_f32((float)val);
    return r;
}

void* seen_simd_f8_add(void* a, void* b) {
    float32x4_t* r = (float32x4_t*)malloc(2 * sizeof(float32x4_t));
    float32x4_t* va = (float32x4_t*)a;
    float32x4_t* vb = (float32x4_t*)b;
    r[0] = vaddq_f32(va[0], vb[0]);
    r[1] = vaddq_f32(va[1], vb[1]);
    return r;
}

void* seen_simd_f8_sub(void* a, void* b) {
    float32x4_t* r = (float32x4_t*)malloc(2 * sizeof(float32x4_t));
    float32x4_t* va = (float32x4_t*)a;
    float32x4_t* vb = (float32x4_t*)b;
    r[0] = vsubq_f32(va[0], vb[0]);
    r[1] = vsubq_f32(va[1], vb[1]);
    return r;
}

void* seen_simd_f8_mul(void* a, void* b) {
    float32x4_t* r = (float32x4_t*)malloc(2 * sizeof(float32x4_t));
    float32x4_t* va = (float32x4_t*)a;
    float32x4_t* vb = (float32x4_t*)b;
    r[0] = vmulq_f32(va[0], vb[0]);
    r[1] = vmulq_f32(va[1], vb[1]);
    return r;
}

void* seen_simd_f8_div(void* a, void* b) {
    float32x4_t* r = (float32x4_t*)malloc(2 * sizeof(float32x4_t));
    float32x4_t* va = (float32x4_t*)a;
    float32x4_t* vb = (float32x4_t*)b;
    r[0] = vdivq_f32(va[0], vb[0]);
    r[1] = vdivq_f32(va[1], vb[1]);
    return r;
}

void* seen_simd_f8_min(void* a, void* b) {
    float32x4_t* r = (float32x4_t*)malloc(2 * sizeof(float32x4_t));
    float32x4_t* va = (float32x4_t*)a;
    float32x4_t* vb = (float32x4_t*)b;
    r[0] = vminq_f32(va[0], vb[0]);
    r[1] = vminq_f32(va[1], vb[1]);
    return r;
}

void* seen_simd_f8_max(void* a, void* b) {
    float32x4_t* r = (float32x4_t*)malloc(2 * sizeof(float32x4_t));
    float32x4_t* va = (float32x4_t*)a;
    float32x4_t* vb = (float32x4_t*)b;
    r[0] = vmaxq_f32(va[0], vb[0]);
    r[1] = vmaxq_f32(va[1], vb[1]);
    return r;
}

double seen_simd_f8_sum(void* a) {
    float32x4_t* v = (float32x4_t*)a;
    float32x4_t sum4 = vaddq_f32(v[0], v[1]);
    return (double)vaddvq_f32(sum4);
}

double seen_simd_f8_dot(void* a, void* b) {
    float32x4_t* va = (float32x4_t*)a;
    float32x4_t* vb = (float32x4_t*)b;
    float32x4_t p0 = vmulq_f32(va[0], vb[0]);
    float32x4_t p1 = vmulq_f32(va[1], vb[1]);
    float32x4_t sum4 = vaddq_f32(p0, p1);
    return (double)vaddvq_f32(sum4);
}

void* seen_simd_f8_load(void* ptr) {
    float32x4_t* r = (float32x4_t*)malloc(2 * sizeof(float32x4_t));
    float* fp = (float*)ptr;
    r[0] = vld1q_f32(fp);
    r[1] = vld1q_f32(fp + 4);
    return r;
}

void seen_simd_f8_store(void* vec, void* ptr) {
    float32x4_t* v = (float32x4_t*)vec;
    float* fp = (float*)ptr;
    vst1q_f32(fp, v[0]);
    vst1q_f32(fp + 4, v[1]);
}

// --- Auto-dispatch SIMD array operations (NEON) ---

double seen_simd_reduce_sum(void* arr_data, int64_t len) {
    float* data = (float*)arr_data;
    double sum = 0.0;
    int64_t i = 0;
    float32x4_t acc = vdupq_n_f32(0.0f);
    for (; i + 3 < len; i += 4) {
        float32x4_t v = vld1q_f32(data + i);
        acc = vaddq_f32(acc, v);
    }
    sum = (double)vaddvq_f32(acc);
    for (; i < len; i++) sum += (double)data[i];
    return sum;
}

double seen_simd_dot_product(void* a_data, void* b_data, int64_t len) {
    float* a = (float*)a_data;
    float* b = (float*)b_data;
    double sum = 0.0;
    int64_t i = 0;
    float32x4_t acc = vdupq_n_f32(0.0f);
    for (; i + 3 < len; i += 4) {
        float32x4_t va = vld1q_f32(a + i);
        float32x4_t vb = vld1q_f32(b + i);
        acc = vfmaq_f32(acc, va, vb);
    }
    sum = (double)vaddvq_f32(acc);
    for (; i < len; i++) sum += (double)a[i] * (double)b[i];
    return sum;
}

double seen_simd_reduce_min(void* arr_data, int64_t len) {
    if (len <= 0) return 0.0;
    float* data = (float*)arr_data;
    float min_val = data[0];
    for (int64_t i = 1; i < len; i++) if (data[i] < min_val) min_val = data[i];
    return (double)min_val;
}

double seen_simd_reduce_max(void* arr_data, int64_t len) {
    if (len <= 0) return 0.0;
    float* data = (float*)arr_data;
    float max_val = data[0];
    for (int64_t i = 1; i < len; i++) if (data[i] > max_val) max_val = data[i];
    return (double)max_val;
}

void seen_simd_prefix_sum(void* arr_data, int64_t len) {
    float* data = (float*)arr_data;
    for (int64_t i = 1; i < len; i++) data[i] += data[i - 1];
}

#else
// Scalar fallback implementations for RISC-V, WASM, and other architectures

void* seen_simd_f4_splat(double val) {
    float* r = (float*)malloc(4 * sizeof(float));
    for (int i = 0; i < 4; i++) r[i] = (float)val;
    return r;
}

static void scalar_f4_binop(void* a, void* b, void* r, int op) {
    float* fa = (float*)a; float* fb = (float*)b; float* fr = (float*)r;
    for (int i = 0; i < 4; i++) {
        switch(op) {
            case 0: fr[i] = fa[i] + fb[i]; break;
            case 1: fr[i] = fa[i] - fb[i]; break;
            case 2: fr[i] = fa[i] * fb[i]; break;
            case 3: fr[i] = fb[i] != 0.0f ? fa[i] / fb[i] : 0.0f; break;
            case 4: fr[i] = fa[i] < fb[i] ? fa[i] : fb[i]; break;
            case 5: fr[i] = fa[i] > fb[i] ? fa[i] : fb[i]; break;
        }
    }
}

void* seen_simd_f4_add(void* a, void* b) { float* r = (float*)malloc(4*sizeof(float)); scalar_f4_binop(a,b,r,0); return r; }
void* seen_simd_f4_sub(void* a, void* b) { float* r = (float*)malloc(4*sizeof(float)); scalar_f4_binop(a,b,r,1); return r; }
void* seen_simd_f4_mul(void* a, void* b) { float* r = (float*)malloc(4*sizeof(float)); scalar_f4_binop(a,b,r,2); return r; }
void* seen_simd_f4_div(void* a, void* b) { float* r = (float*)malloc(4*sizeof(float)); scalar_f4_binop(a,b,r,3); return r; }
void* seen_simd_f4_min(void* a, void* b) { float* r = (float*)malloc(4*sizeof(float)); scalar_f4_binop(a,b,r,4); return r; }
void* seen_simd_f4_max(void* a, void* b) { float* r = (float*)malloc(4*sizeof(float)); scalar_f4_binop(a,b,r,5); return r; }

double seen_simd_f4_sum(void* a) {
    float* f = (float*)a;
    return (double)(f[0] + f[1] + f[2] + f[3]);
}

double seen_simd_f4_dot(void* a, void* b) {
    float* fa = (float*)a; float* fb = (float*)b;
    return (double)(fa[0]*fb[0] + fa[1]*fb[1] + fa[2]*fb[2] + fa[3]*fb[3]);
}

void* seen_simd_f4_load(void* ptr) {
    float* r = (float*)malloc(4 * sizeof(float));
    memcpy(r, ptr, 4 * sizeof(float));
    return r;
}

void seen_simd_f4_store(void* vec, void* ptr) {
    memcpy(ptr, vec, 4 * sizeof(float));
}

void* seen_simd_f8_splat(double val) {
    float* r = (float*)malloc(8 * sizeof(float));
    for (int i = 0; i < 8; i++) r[i] = (float)val;
    return r;
}

static void scalar_f8_binop(void* a, void* b, void* r, int op) {
    float* fa = (float*)a; float* fb = (float*)b; float* fr = (float*)r;
    for (int i = 0; i < 8; i++) {
        switch(op) {
            case 0: fr[i] = fa[i] + fb[i]; break;
            case 1: fr[i] = fa[i] - fb[i]; break;
            case 2: fr[i] = fa[i] * fb[i]; break;
            case 3: fr[i] = fb[i] != 0.0f ? fa[i] / fb[i] : 0.0f; break;
            case 4: fr[i] = fa[i] < fb[i] ? fa[i] : fb[i]; break;
            case 5: fr[i] = fa[i] > fb[i] ? fa[i] : fb[i]; break;
        }
    }
}

void* seen_simd_f8_add(void* a, void* b) { float* r = (float*)malloc(8*sizeof(float)); scalar_f8_binop(a,b,r,0); return r; }
void* seen_simd_f8_sub(void* a, void* b) { float* r = (float*)malloc(8*sizeof(float)); scalar_f8_binop(a,b,r,1); return r; }
void* seen_simd_f8_mul(void* a, void* b) { float* r = (float*)malloc(8*sizeof(float)); scalar_f8_binop(a,b,r,2); return r; }
void* seen_simd_f8_div(void* a, void* b) { float* r = (float*)malloc(8*sizeof(float)); scalar_f8_binop(a,b,r,3); return r; }
void* seen_simd_f8_min(void* a, void* b) { float* r = (float*)malloc(8*sizeof(float)); scalar_f8_binop(a,b,r,4); return r; }
void* seen_simd_f8_max(void* a, void* b) { float* r = (float*)malloc(8*sizeof(float)); scalar_f8_binop(a,b,r,5); return r; }

double seen_simd_f8_sum(void* a) {
    float* f = (float*)a;
    double sum = 0.0;
    for (int i = 0; i < 8; i++) sum += (double)f[i];
    return sum;
}

double seen_simd_f8_dot(void* a, void* b) {
    float* fa = (float*)a; float* fb = (float*)b;
    double sum = 0.0;
    for (int i = 0; i < 8; i++) sum += (double)fa[i] * (double)fb[i];
    return sum;
}

void* seen_simd_f8_load(void* ptr) {
    float* r = (float*)malloc(8 * sizeof(float));
    memcpy(r, ptr, 8 * sizeof(float));
    return r;
}

void seen_simd_f8_store(void* vec, void* ptr) {
    memcpy(ptr, vec, 8 * sizeof(float));
}

double seen_simd_reduce_sum(void* arr_data, int64_t len) {
    float* data = (float*)arr_data;
    double sum = 0.0;
    for (int64_t i = 0; i < len; i++) sum += (double)data[i];
    return sum;
}

double seen_simd_dot_product(void* a_data, void* b_data, int64_t len) {
    float* a = (float*)a_data;
    float* b = (float*)b_data;
    double sum = 0.0;
    for (int64_t i = 0; i < len; i++) sum += (double)a[i] * (double)b[i];
    return sum;
}

double seen_simd_reduce_min(void* arr_data, int64_t len) {
    if (len <= 0) return 0.0;
    float* data = (float*)arr_data;
    float min_val = data[0];
    for (int64_t i = 1; i < len; i++) if (data[i] < min_val) min_val = data[i];
    return (double)min_val;
}

double seen_simd_reduce_max(void* arr_data, int64_t len) {
    if (len <= 0) return 0.0;
    float* data = (float*)arr_data;
    float max_val = data[0];
    for (int64_t i = 1; i < len; i++) if (data[i] > max_val) max_val = data[i];
    return (double)max_val;
}

void seen_simd_prefix_sum(void* arr_data, int64_t len) {
    float* data = (float*)arr_data;
    for (int64_t i = 1; i < len; i++) data[i] += data[i - 1];
}

#endif // x86_64 / aarch64 / scalar

// ============================================================================
// HashMap<K,V> Runtime - Open addressing with linear probing (SOA layout)
// Layout: { keys*, values*, states*, capacity, length, tombstones }
// SOA = Struct-of-Arrays: states[] is contiguous for cache-friendly probing
// ============================================================================

typedef struct {
    int64_t* keys;
    int64_t* values;
    uint8_t* states;     // 0=empty, 1=occupied, 2=tombstone
    int64_t capacity;
    int64_t length;
    int64_t tombstones;
} SeenHashMap;

static uint64_t hashmap_hash_int(int64_t key) {
    uint64_t h = (uint64_t)key;
    h ^= h >> 33;
    h *= 0xff51afd7ed558ccdULL;
    h ^= h >> 33;
    h *= 0xc4ceb9fe1a85ec53ULL;
    h ^= h >> 33;
    return h;
}

static void hashmap_grow(SeenHashMap* map) {
    int64_t old_cap = map->capacity;
    int64_t* old_keys = map->keys;
    int64_t* old_values = map->values;
    uint8_t* old_states = map->states;
    int64_t new_cap = old_cap * 2;
    int64_t mask = new_cap - 1;
    int64_t* new_keys = (int64_t*)malloc(new_cap * sizeof(int64_t));
    int64_t* new_values = (int64_t*)malloc(new_cap * sizeof(int64_t));
    uint8_t* new_states = (uint8_t*)calloc(new_cap, sizeof(uint8_t));
    for (int64_t i = 0; i < old_cap; i++) {
        if (old_states[i] == 1) {
            uint64_t h = hashmap_hash_int(old_keys[i]);
            int64_t idx = (int64_t)(h & (uint64_t)mask);
            while (new_states[idx] == 1) {
                idx = (idx + 1) & mask;
            }
            new_keys[idx] = old_keys[i];
            new_values[idx] = old_values[i];
            new_states[idx] = 1;
        }
    }
    free(old_keys);
    free(old_values);
    free(old_states);
    map->keys = new_keys;
    map->values = new_values;
    map->states = new_states;
    map->capacity = new_cap;
    map->tombstones = 0;
}

void* HashMap_new(void) {
    SeenHashMap* map = (SeenHashMap*)malloc(sizeof(SeenHashMap));
    map->capacity = 16;
    map->keys = (int64_t*)malloc(map->capacity * sizeof(int64_t));
    map->values = (int64_t*)malloc(map->capacity * sizeof(int64_t));
    map->states = (uint8_t*)calloc(map->capacity, sizeof(uint8_t));
    map->length = 0;
    map->tombstones = 0;
    return map;
}

void* HashMap_new_with_capacity(int64_t capacity) {
    int64_t cap = 16;
    while (cap < capacity) cap *= 2;
    SeenHashMap* map = (SeenHashMap*)malloc(sizeof(SeenHashMap));
    map->capacity = cap;
    map->keys = (int64_t*)malloc(cap * sizeof(int64_t));
    map->values = (int64_t*)malloc(cap * sizeof(int64_t));
    map->states = (uint8_t*)calloc(cap, sizeof(uint8_t));
    map->length = 0;
    map->tombstones = 0;
    return map;
}

bool HashMap_insert(void* mapPtr, int64_t key, int64_t value) {
    SeenHashMap* map = (SeenHashMap*)mapPtr;
    if ((map->length + map->tombstones) * 10 >= map->capacity * 7) {
        hashmap_grow(map);
    }
    uint64_t h = hashmap_hash_int(key);
    int64_t mask = map->capacity - 1;
    int64_t idx = (int64_t)(h & (uint64_t)mask);
    int64_t first_tombstone = -1;
    while (1) {
        if (map->states[idx] == 0) {
            int64_t ins = (first_tombstone >= 0) ? first_tombstone : idx;
            map->keys[ins] = key;
            map->values[ins] = value;
            map->states[ins] = 1;
            map->length++;
            if (first_tombstone >= 0) map->tombstones--;
            return true;
        }
        if (map->states[idx] == 2 && first_tombstone < 0) {
            first_tombstone = idx;
        }
        if (map->states[idx] == 1 && map->keys[idx] == key) {
            map->values[idx] = value;
            return false;
        }
        idx = (idx + 1) & mask;
    }
}

int64_t HashMap_get(void* mapPtr, int64_t key) {
    SeenHashMap* map = (SeenHashMap*)mapPtr;
    uint64_t h = hashmap_hash_int(key);
    int64_t mask = map->capacity - 1;
    int64_t idx = (int64_t)(h & (uint64_t)mask);
    while (1) {
        if (map->states[idx] == 0) {
            return BTreeMap_make_none();
        }
        if (map->states[idx] == 1 && map->keys[idx] == key) {
            return BTreeMap_make_some(map->values[idx]);
        }
        idx = (idx + 1) & mask;
    }
}

int64_t HashMap_getOrDefault(void* mapPtr, int64_t key, int64_t defaultValue) {
    SeenHashMap* map = (SeenHashMap*)mapPtr;
    uint64_t h = hashmap_hash_int(key);
    int64_t mask = map->capacity - 1;
    int64_t idx = (int64_t)(h & (uint64_t)mask);
    while (1) {
        if (map->states[idx] == 0) return defaultValue;
        if (map->states[idx] == 1 && map->keys[idx] == key)
            return map->values[idx];
        idx = (idx + 1) & mask;
    }
}

bool HashMap_containsKey(void* mapPtr, int64_t key) {
    SeenHashMap* map = (SeenHashMap*)mapPtr;
    uint64_t h = hashmap_hash_int(key);
    int64_t mask = map->capacity - 1;
    int64_t idx = (int64_t)(h & (uint64_t)mask);
    while (1) {
        if (map->states[idx] == 0) return false;
        if (map->states[idx] == 1 && map->keys[idx] == key) return true;
        idx = (idx + 1) & mask;
    }
}

int64_t HashMap_remove(void* mapPtr, int64_t key) {
    SeenHashMap* map = (SeenHashMap*)mapPtr;
    uint64_t h = hashmap_hash_int(key);
    int64_t mask = map->capacity - 1;
    int64_t idx = (int64_t)(h & (uint64_t)mask);
    while (1) {
        if (map->states[idx] == 0) {
            return BTreeMap_make_none();
        }
        if (map->states[idx] == 1 && map->keys[idx] == key) {
            int64_t val = map->values[idx];
            map->states[idx] = 2;
            map->length--;
            map->tombstones++;
            return BTreeMap_make_some(val);
        }
        idx = (idx + 1) & mask;
    }
}

int64_t HashMap_keys(void* mapPtr) {
    SeenHashMap* map = (SeenHashMap*)mapPtr;
    SeenVec* vec = Vec_new();
    for (int64_t i = 0; i < map->capacity; i++) {
        if (map->states[i] == 1) {
            Vec_push(vec, map->keys[i]);
        }
    }
    return (int64_t)vec;
}

int64_t HashMap_values(void* mapPtr) {
    SeenHashMap* map = (SeenHashMap*)mapPtr;
    SeenVec* vec = Vec_new();
    for (int64_t i = 0; i < map->capacity; i++) {
        if (map->states[i] == 1) {
            Vec_push(vec, map->values[i]);
        }
    }
    return (int64_t)vec;
}

int64_t HashMap_size(void* mapPtr) {
    SeenHashMap* map = (SeenHashMap*)mapPtr;
    return map->length;
}

void HashMap_clear(void* mapPtr) {
    SeenHashMap* map = (SeenHashMap*)mapPtr;
    memset(map->states, 0, map->capacity * sizeof(uint8_t));
    map->length = 0;
    map->tombstones = 0;
}

// ============================================================================
// HashMap<String, Int> Runtime (SOA layout)
// ============================================================================

typedef struct {
    SeenString* keys;
    int64_t* values;
    uint8_t* states;
    int64_t capacity;
    int64_t length;
    int64_t tombstones;
} SeenHashMapStr;

static uint64_t hashmap_hash_str(SeenString s) {
    uint64_t h = 0xcbf29ce484222325ULL;
    const char* data = s.data;
    int64_t len = s.len;
    for (int64_t i = 0; i < len; i++) {
        h ^= (uint8_t)data[i];
        h *= 0x100000001b3ULL;
    }
    return h;
}

static bool hashmap_str_eq(SeenString a, SeenString b) {
    return a.len == b.len && (a.len == 0 || memcmp(a.data, b.data, a.len) == 0);
}

static void hashmap_grow_str(SeenHashMapStr* map) {
    int64_t old_cap = map->capacity;
    SeenString* old_keys = map->keys;
    int64_t* old_values = map->values;
    uint8_t* old_states = map->states;
    int64_t new_cap = old_cap * 2;
    int64_t mask = new_cap - 1;
    SeenString* new_keys = (SeenString*)malloc(new_cap * sizeof(SeenString));
    int64_t* new_values = (int64_t*)malloc(new_cap * sizeof(int64_t));
    uint8_t* new_states = (uint8_t*)calloc(new_cap, sizeof(uint8_t));
    for (int64_t i = 0; i < old_cap; i++) {
        if (old_states[i] == 1) {
            uint64_t h = hashmap_hash_str(old_keys[i]);
            int64_t idx = (int64_t)(h & (uint64_t)mask);
            while (new_states[idx] == 1) {
                idx = (idx + 1) & mask;
            }
            new_keys[idx] = old_keys[i];
            new_values[idx] = old_values[i];
            new_states[idx] = 1;
        }
    }
    free(old_keys);
    free(old_values);
    free(old_states);
    map->keys = new_keys;
    map->values = new_values;
    map->states = new_states;
    map->capacity = new_cap;
    map->tombstones = 0;
}

void* HashMap_new_str(void) {
    SeenHashMapStr* map = (SeenHashMapStr*)malloc(sizeof(SeenHashMapStr));
    map->capacity = 16;
    map->keys = (SeenString*)malloc(map->capacity * sizeof(SeenString));
    map->values = (int64_t*)malloc(map->capacity * sizeof(int64_t));
    map->states = (uint8_t*)calloc(map->capacity, sizeof(uint8_t));
    map->length = 0;
    map->tombstones = 0;
    return map;
}

void* HashMap_new_str_with_capacity(int64_t capacity) {
    int64_t cap = 16;
    while (cap < capacity) cap *= 2;
    SeenHashMapStr* map = (SeenHashMapStr*)malloc(sizeof(SeenHashMapStr));
    map->capacity = cap;
    map->keys = (SeenString*)malloc(cap * sizeof(SeenString));
    map->values = (int64_t*)malloc(cap * sizeof(int64_t));
    map->states = (uint8_t*)calloc(cap, sizeof(uint8_t));
    map->length = 0;
    map->tombstones = 0;
    return map;
}

bool HashMap_insert_str(void* mapPtr, SeenString key, int64_t value) {
    SeenHashMapStr* map = (SeenHashMapStr*)mapPtr;
    if ((map->length + map->tombstones) * 10 >= map->capacity * 7) {
        hashmap_grow_str(map);
    }
    uint64_t h = hashmap_hash_str(key);
    int64_t mask = map->capacity - 1;
    int64_t idx = (int64_t)(h & (uint64_t)mask);
    int64_t first_tombstone = -1;
    while (1) {
        if (map->states[idx] == 0) {
            int64_t ins = (first_tombstone >= 0) ? first_tombstone : idx;
            map->keys[ins] = key;
            map->values[ins] = value;
            map->states[ins] = 1;
            map->length++;
            if (first_tombstone >= 0) map->tombstones--;
            return true;
        }
        if (map->states[idx] == 2 && first_tombstone < 0) {
            first_tombstone = idx;
        }
        if (map->states[idx] == 1 && hashmap_str_eq(map->keys[idx], key)) {
            map->values[idx] = value;
            return false;
        }
        idx = (idx + 1) & mask;
    }
}

int64_t HashMap_get_str(void* mapPtr, SeenString key) {
    SeenHashMapStr* map = (SeenHashMapStr*)mapPtr;
    uint64_t h = hashmap_hash_str(key);
    int64_t mask = map->capacity - 1;
    int64_t idx = (int64_t)(h & (uint64_t)mask);
    while (1) {
        if (map->states[idx] == 0) return BTreeMap_make_none();
        if (map->states[idx] == 1 && hashmap_str_eq(map->keys[idx], key)) {
            return BTreeMap_make_some(map->values[idx]);
        }
        idx = (idx + 1) & mask;
    }
}

int64_t HashMap_getOrDefault_str(void* mapPtr, SeenString key, int64_t defaultValue) {
    SeenHashMapStr* map = (SeenHashMapStr*)mapPtr;
    uint64_t h = hashmap_hash_str(key);
    int64_t mask = map->capacity - 1;
    int64_t idx = (int64_t)(h & (uint64_t)mask);
    while (1) {
        if (map->states[idx] == 0) return defaultValue;
        if (map->states[idx] == 1 && hashmap_str_eq(map->keys[idx], key))
            return map->values[idx];
        idx = (idx + 1) & mask;
    }
}

bool HashMap_containsKey_str(void* mapPtr, SeenString key) {
    SeenHashMapStr* map = (SeenHashMapStr*)mapPtr;
    uint64_t h = hashmap_hash_str(key);
    int64_t mask = map->capacity - 1;
    int64_t idx = (int64_t)(h & (uint64_t)mask);
    while (1) {
        if (map->states[idx] == 0) return false;
        if (map->states[idx] == 1 && hashmap_str_eq(map->keys[idx], key)) return true;
        idx = (idx + 1) & mask;
    }
}

int64_t HashMap_remove_str(void* mapPtr, SeenString key) {
    SeenHashMapStr* map = (SeenHashMapStr*)mapPtr;
    uint64_t h = hashmap_hash_str(key);
    int64_t mask = map->capacity - 1;
    int64_t idx = (int64_t)(h & (uint64_t)mask);
    while (1) {
        if (map->states[idx] == 0) return BTreeMap_make_none();
        if (map->states[idx] == 1 && hashmap_str_eq(map->keys[idx], key)) {
            int64_t val = map->values[idx];
            map->states[idx] = 2;
            map->length--;
            map->tombstones++;
            return BTreeMap_make_some(val);
        }
        idx = (idx + 1) & mask;
    }
}

int64_t HashMap_keys_str(void* mapPtr) {
    SeenHashMapStr* map = (SeenHashMapStr*)mapPtr;
    SeenVecStr* vec = Vec_new_str();
    for (int64_t i = 0; i < map->capacity; i++) {
        if (map->states[i] == 1) {
            Vec_push_str(vec, map->keys[i]);
        }
    }
    return (int64_t)vec;
}

int64_t HashMap_values_str(void* mapPtr) {
    SeenHashMapStr* map = (SeenHashMapStr*)mapPtr;
    SeenVec* vec = Vec_new();
    for (int64_t i = 0; i < map->capacity; i++) {
        if (map->states[i] == 1) {
            Vec_push(vec, map->values[i]);
        }
    }
    return (int64_t)vec;
}

int64_t HashMap_size_str(void* mapPtr) {
    SeenHashMapStr* map = (SeenHashMapStr*)mapPtr;
    return map->length;
}

void HashMap_clear_str(void* mapPtr) {
    SeenHashMapStr* map = (SeenHashMapStr*)mapPtr;
    memset(map->states, 0, map->capacity * sizeof(uint8_t));
    map->length = 0;
    map->tombstones = 0;
}

// ============================================================================
// HashMap<String, String> Runtime (SOA layout)
// ============================================================================

typedef struct {
    SeenString* keys;
    SeenString* values;
    uint8_t* states;
    int64_t capacity;
    int64_t length;
    int64_t tombstones;
} SeenHashMapStrStr;

static void hashmap_grow_str_str(SeenHashMapStrStr* map) {
    int64_t old_cap = map->capacity;
    SeenString* old_keys = map->keys;
    SeenString* old_values = map->values;
    uint8_t* old_states = map->states;
    int64_t new_cap = old_cap * 2;
    int64_t mask = new_cap - 1;
    SeenString* new_keys = (SeenString*)malloc(new_cap * sizeof(SeenString));
    SeenString* new_values = (SeenString*)malloc(new_cap * sizeof(SeenString));
    uint8_t* new_states = (uint8_t*)calloc(new_cap, sizeof(uint8_t));
    for (int64_t i = 0; i < old_cap; i++) {
        if (old_states[i] == 1) {
            uint64_t h = hashmap_hash_str(old_keys[i]);
            int64_t idx = (int64_t)(h & (uint64_t)mask);
            while (new_states[idx] == 1) {
                idx = (idx + 1) & mask;
            }
            new_keys[idx] = old_keys[i];
            new_values[idx] = old_values[i];
            new_states[idx] = 1;
        }
    }
    free(old_keys);
    free(old_values);
    free(old_states);
    map->keys = new_keys;
    map->values = new_values;
    map->states = new_states;
    map->capacity = new_cap;
    map->tombstones = 0;
}

void* HashMap_new_str_str(void) {
    SeenHashMapStrStr* map = (SeenHashMapStrStr*)malloc(sizeof(SeenHashMapStrStr));
    map->capacity = 16;
    map->keys = (SeenString*)malloc(map->capacity * sizeof(SeenString));
    map->values = (SeenString*)malloc(map->capacity * sizeof(SeenString));
    map->states = (uint8_t*)calloc(map->capacity, sizeof(uint8_t));
    map->length = 0;
    map->tombstones = 0;
    return map;
}

void* HashMap_new_str_str_with_capacity(int64_t capacity) {
    int64_t cap = 16;
    while (cap < capacity) cap *= 2;
    SeenHashMapStrStr* map = (SeenHashMapStrStr*)malloc(sizeof(SeenHashMapStrStr));
    map->capacity = cap;
    map->keys = (SeenString*)malloc(cap * sizeof(SeenString));
    map->values = (SeenString*)malloc(cap * sizeof(SeenString));
    map->states = (uint8_t*)calloc(cap, sizeof(uint8_t));
    map->length = 0;
    map->tombstones = 0;
    return map;
}

bool HashMap_insert_str_str(void* mapPtr, SeenString key, SeenString value) {
    SeenHashMapStrStr* map = (SeenHashMapStrStr*)mapPtr;
    if ((map->length + map->tombstones) * 10 >= map->capacity * 7) {
        hashmap_grow_str_str(map);
    }
    uint64_t h = hashmap_hash_str(key);
    int64_t mask = map->capacity - 1;
    int64_t idx = (int64_t)(h & (uint64_t)mask);
    int64_t first_tombstone = -1;
    while (1) {
        if (map->states[idx] == 0) {
            int64_t ins = (first_tombstone >= 0) ? first_tombstone : idx;
            map->keys[ins] = key;
            map->values[ins] = value;
            map->states[ins] = 1;
            map->length++;
            if (first_tombstone >= 0) map->tombstones--;
            return true;
        }
        if (map->states[idx] == 2 && first_tombstone < 0) {
            first_tombstone = idx;
        }
        if (map->states[idx] == 1 && hashmap_str_eq(map->keys[idx], key)) {
            map->values[idx] = value;
            return false;
        }
        idx = (idx + 1) & mask;
    }
}

int64_t HashMap_get_str_str(void* mapPtr, SeenString key) {
    SeenHashMapStrStr* map = (SeenHashMapStrStr*)mapPtr;
    uint64_t h = hashmap_hash_str(key);
    int64_t mask = map->capacity - 1;
    int64_t idx = (int64_t)(h & (uint64_t)mask);
    while (1) {
        if (map->states[idx] == 0) return BTreeMap_make_none_str();
        if (map->states[idx] == 1 && hashmap_str_eq(map->keys[idx], key)) {
            return BTreeMap_make_some_str(map->values[idx]);
        }
        idx = (idx + 1) & mask;
    }
}

SeenString HashMap_getOrDefault_str_str(void* mapPtr, SeenString key, SeenString defaultValue) {
    SeenHashMapStrStr* map = (SeenHashMapStrStr*)mapPtr;
    uint64_t h = hashmap_hash_str(key);
    int64_t mask = map->capacity - 1;
    int64_t idx = (int64_t)(h & (uint64_t)mask);
    while (1) {
        if (map->states[idx] == 0) return defaultValue;
        if (map->states[idx] == 1 && hashmap_str_eq(map->keys[idx], key))
            return map->values[idx];
        idx = (idx + 1) & mask;
    }
}

bool HashMap_containsKey_str_str(void* mapPtr, SeenString key) {
    SeenHashMapStrStr* map = (SeenHashMapStrStr*)mapPtr;
    uint64_t h = hashmap_hash_str(key);
    int64_t mask = map->capacity - 1;
    int64_t idx = (int64_t)(h & (uint64_t)mask);
    while (1) {
        if (map->states[idx] == 0) return false;
        if (map->states[idx] == 1 && hashmap_str_eq(map->keys[idx], key)) return true;
        idx = (idx + 1) & mask;
    }
}

int64_t HashMap_remove_str_str(void* mapPtr, SeenString key) {
    SeenHashMapStrStr* map = (SeenHashMapStrStr*)mapPtr;
    uint64_t h = hashmap_hash_str(key);
    int64_t mask = map->capacity - 1;
    int64_t idx = (int64_t)(h & (uint64_t)mask);
    while (1) {
        if (map->states[idx] == 0) return BTreeMap_make_none_str();
        if (map->states[idx] == 1 && hashmap_str_eq(map->keys[idx], key)) {
            SeenString val = map->values[idx];
            map->states[idx] = 2;
            map->length--;
            map->tombstones++;
            return BTreeMap_make_some_str(val);
        }
        idx = (idx + 1) & mask;
    }
}

int64_t HashMap_keys_str_str(void* mapPtr) {
    SeenHashMapStrStr* map = (SeenHashMapStrStr*)mapPtr;
    SeenVecStr* vec = Vec_new_str();
    for (int64_t i = 0; i < map->capacity; i++) {
        if (map->states[i] == 1) {
            Vec_push_str(vec, map->keys[i]);
        }
    }
    return (int64_t)vec;
}

int64_t HashMap_values_str_str(void* mapPtr) {
    SeenHashMapStrStr* map = (SeenHashMapStrStr*)mapPtr;
    SeenVecStr* vec = Vec_new_str();
    for (int64_t i = 0; i < map->capacity; i++) {
        if (map->states[i] == 1) {
            Vec_push_str(vec, map->values[i]);
        }
    }
    return (int64_t)vec;
}

int64_t HashMap_size_str_str(void* mapPtr) {
    SeenHashMapStrStr* map = (SeenHashMapStrStr*)mapPtr;
    return map->length;
}

void HashMap_clear_str_str(void* mapPtr) {
    SeenHashMapStrStr* map = (SeenHashMapStrStr*)mapPtr;
    memset(map->states, 0, map->capacity * sizeof(uint8_t));
    map->length = 0;
    map->tombstones = 0;
}

// ============================================================================
// HashMap<Int, String> Runtime (SOA layout)
// ============================================================================

typedef struct {
    int64_t* keys;
    SeenString* values;
    uint8_t* states;
    int64_t capacity;
    int64_t length;
    int64_t tombstones;
} SeenHashMapIntStr;

static void hashmap_grow_int_str(SeenHashMapIntStr* map) {
    int64_t old_cap = map->capacity;
    int64_t* old_keys = map->keys;
    SeenString* old_values = map->values;
    uint8_t* old_states = map->states;
    int64_t new_cap = old_cap * 2;
    int64_t mask = new_cap - 1;
    int64_t* new_keys = (int64_t*)malloc(new_cap * sizeof(int64_t));
    SeenString* new_values = (SeenString*)malloc(new_cap * sizeof(SeenString));
    uint8_t* new_states = (uint8_t*)calloc(new_cap, sizeof(uint8_t));
    for (int64_t i = 0; i < old_cap; i++) {
        if (old_states[i] == 1) {
            uint64_t h = hashmap_hash_int(old_keys[i]);
            int64_t idx = (int64_t)(h & (uint64_t)mask);
            while (new_states[idx] == 1) {
                idx = (idx + 1) & mask;
            }
            new_keys[idx] = old_keys[i];
            new_values[idx] = old_values[i];
            new_states[idx] = 1;
        }
    }
    free(old_keys);
    free(old_values);
    free(old_states);
    map->keys = new_keys;
    map->values = new_values;
    map->states = new_states;
    map->capacity = new_cap;
    map->tombstones = 0;
}

void* HashMap_new_int_str(void) {
    SeenHashMapIntStr* map = (SeenHashMapIntStr*)malloc(sizeof(SeenHashMapIntStr));
    map->capacity = 16;
    map->keys = (int64_t*)malloc(map->capacity * sizeof(int64_t));
    map->values = (SeenString*)malloc(map->capacity * sizeof(SeenString));
    map->states = (uint8_t*)calloc(map->capacity, sizeof(uint8_t));
    map->length = 0;
    map->tombstones = 0;
    return map;
}

void* HashMap_new_int_str_with_capacity(int64_t capacity) {
    int64_t cap = 16;
    while (cap < capacity) cap *= 2;
    SeenHashMapIntStr* map = (SeenHashMapIntStr*)malloc(sizeof(SeenHashMapIntStr));
    map->capacity = cap;
    map->keys = (int64_t*)malloc(cap * sizeof(int64_t));
    map->values = (SeenString*)malloc(cap * sizeof(SeenString));
    map->states = (uint8_t*)calloc(cap, sizeof(uint8_t));
    map->length = 0;
    map->tombstones = 0;
    return map;
}

bool HashMap_insert_int_str(void* mapPtr, int64_t key, SeenString value) {
    SeenHashMapIntStr* map = (SeenHashMapIntStr*)mapPtr;
    if ((map->length + map->tombstones) * 10 >= map->capacity * 7) {
        hashmap_grow_int_str(map);
    }
    uint64_t h = hashmap_hash_int(key);
    int64_t mask = map->capacity - 1;
    int64_t idx = (int64_t)(h & (uint64_t)mask);
    int64_t first_tombstone = -1;
    while (1) {
        if (map->states[idx] == 0) {
            int64_t ins = (first_tombstone >= 0) ? first_tombstone : idx;
            map->keys[ins] = key;
            map->values[ins] = value;
            map->states[ins] = 1;
            map->length++;
            if (first_tombstone >= 0) map->tombstones--;
            return true;
        }
        if (map->states[idx] == 2 && first_tombstone < 0) {
            first_tombstone = idx;
        }
        if (map->states[idx] == 1 && map->keys[idx] == key) {
            map->values[idx] = value;
            return false;
        }
        idx = (idx + 1) & mask;
    }
}

int64_t HashMap_get_int_str(void* mapPtr, int64_t key) {
    SeenHashMapIntStr* map = (SeenHashMapIntStr*)mapPtr;
    uint64_t h = hashmap_hash_int(key);
    int64_t mask = map->capacity - 1;
    int64_t idx = (int64_t)(h & (uint64_t)mask);
    while (1) {
        if (map->states[idx] == 0) return BTreeMap_make_none_str();
        if (map->states[idx] == 1 && map->keys[idx] == key) {
            return BTreeMap_make_some_str(map->values[idx]);
        }
        idx = (idx + 1) & mask;
    }
}

SeenString HashMap_getOrDefault_int_str(void* mapPtr, int64_t key, SeenString defaultValue) {
    SeenHashMapIntStr* map = (SeenHashMapIntStr*)mapPtr;
    uint64_t h = hashmap_hash_int(key);
    int64_t mask = map->capacity - 1;
    int64_t idx = (int64_t)(h & (uint64_t)mask);
    while (1) {
        if (map->states[idx] == 0) return defaultValue;
        if (map->states[idx] == 1 && map->keys[idx] == key)
            return map->values[idx];
        idx = (idx + 1) & mask;
    }
}

bool HashMap_containsKey_int_str(void* mapPtr, int64_t key) {
    SeenHashMapIntStr* map = (SeenHashMapIntStr*)mapPtr;
    uint64_t h = hashmap_hash_int(key);
    int64_t mask = map->capacity - 1;
    int64_t idx = (int64_t)(h & (uint64_t)mask);
    while (1) {
        if (map->states[idx] == 0) return false;
        if (map->states[idx] == 1 && map->keys[idx] == key) return true;
        idx = (idx + 1) & mask;
    }
}

int64_t HashMap_remove_int_str(void* mapPtr, int64_t key) {
    SeenHashMapIntStr* map = (SeenHashMapIntStr*)mapPtr;
    uint64_t h = hashmap_hash_int(key);
    int64_t mask = map->capacity - 1;
    int64_t idx = (int64_t)(h & (uint64_t)mask);
    while (1) {
        if (map->states[idx] == 0) return BTreeMap_make_none_str();
        if (map->states[idx] == 1 && map->keys[idx] == key) {
            SeenString val = map->values[idx];
            map->states[idx] = 2;
            map->length--;
            map->tombstones++;
            return BTreeMap_make_some_str(val);
        }
        idx = (idx + 1) & mask;
    }
}

int64_t HashMap_keys_int_str(void* mapPtr) {
    SeenHashMapIntStr* map = (SeenHashMapIntStr*)mapPtr;
    SeenVec* vec = Vec_new();
    for (int64_t i = 0; i < map->capacity; i++) {
        if (map->states[i] == 1) {
            Vec_push(vec, map->keys[i]);
        }
    }
    return (int64_t)vec;
}

int64_t HashMap_values_int_str(void* mapPtr) {
    SeenHashMapIntStr* map = (SeenHashMapIntStr*)mapPtr;
    SeenVecStr* vec = Vec_new_str();
    for (int64_t i = 0; i < map->capacity; i++) {
        if (map->states[i] == 1) {
            Vec_push_str(vec, map->values[i]);
        }
    }
    return (int64_t)vec;
}

int64_t HashMap_size_int_str(void* mapPtr) {
    SeenHashMapIntStr* map = (SeenHashMapIntStr*)mapPtr;
    return map->length;
}

void HashMap_clear_int_str(void* mapPtr) {
    SeenHashMapIntStr* map = (SeenHashMapIntStr*)mapPtr;
    memset(map->states, 0, map->capacity * sizeof(uint8_t));
    map->length = 0;
    map->tombstones = 0;
}

// ============================================================================
// LinkedList<Int> Runtime - Doubly linked list
// Layout: { LLNode* head, LLNode* tail, int64_t length }
// ============================================================================

typedef struct LLNode {
    int64_t value;
    struct LLNode* prev;
    struct LLNode* next;
} LLNode;

typedef struct {
    LLNode* head;
    LLNode* tail;
    int64_t length;
} SeenLinkedList;

void* LinkedList_new(void) {
    SeenLinkedList* ll = (SeenLinkedList*)malloc(sizeof(SeenLinkedList));
    ll->head = NULL;
    ll->tail = NULL;
    ll->length = 0;
    return ll;
}

void LinkedList_pushFront(void* llPtr, int64_t value) {
    SeenLinkedList* ll = (SeenLinkedList*)llPtr;
    LLNode* node = (LLNode*)malloc(sizeof(LLNode));
    node->value = value;
    node->prev = NULL;
    node->next = ll->head;
    if (ll->head) ll->head->prev = node;
    ll->head = node;
    if (!ll->tail) ll->tail = node;
    ll->length++;
}

void LinkedList_pushBack(void* llPtr, int64_t value) {
    SeenLinkedList* ll = (SeenLinkedList*)llPtr;
    LLNode* node = (LLNode*)malloc(sizeof(LLNode));
    node->value = value;
    node->prev = ll->tail;
    node->next = NULL;
    if (ll->tail) ll->tail->next = node;
    ll->tail = node;
    if (!ll->head) ll->head = node;
    ll->length++;
}

int64_t LinkedList_popFront(void* llPtr) {
    SeenLinkedList* ll = (SeenLinkedList*)llPtr;
    if (!ll->head) return BTreeMap_make_none();
    LLNode* node = ll->head;
    int64_t val = node->value;
    ll->head = node->next;
    if (ll->head) ll->head->prev = NULL;
    else ll->tail = NULL;
    ll->length--;
    free(node);
    return BTreeMap_make_some(val);
}

int64_t LinkedList_popBack(void* llPtr) {
    SeenLinkedList* ll = (SeenLinkedList*)llPtr;
    if (!ll->tail) return BTreeMap_make_none();
    LLNode* node = ll->tail;
    int64_t val = node->value;
    ll->tail = node->prev;
    if (ll->tail) ll->tail->next = NULL;
    else ll->head = NULL;
    ll->length--;
    free(node);
    return BTreeMap_make_some(val);
}

int64_t LinkedList_peekFront(void* llPtr) {
    SeenLinkedList* ll = (SeenLinkedList*)llPtr;
    if (!ll->head) return BTreeMap_make_none();
    return BTreeMap_make_some(ll->head->value);
}

int64_t LinkedList_peekBack(void* llPtr) {
    SeenLinkedList* ll = (SeenLinkedList*)llPtr;
    if (!ll->tail) return BTreeMap_make_none();
    return BTreeMap_make_some(ll->tail->value);
}

int64_t LinkedList_size(void* llPtr) {
    SeenLinkedList* ll = (SeenLinkedList*)llPtr;
    return ll->length;
}

bool LinkedList_isEmpty(void* llPtr) {
    SeenLinkedList* ll = (SeenLinkedList*)llPtr;
    return ll->length == 0;
}

void LinkedList_removeNode(void* llPtr, void* nodePtr) {
    SeenLinkedList* ll = (SeenLinkedList*)llPtr;
    LLNode* node = (LLNode*)nodePtr;
    if (node->prev) node->prev->next = node->next;
    else ll->head = node->next;
    if (node->next) node->next->prev = node->prev;
    else ll->tail = node->prev;
    ll->length--;
    free(node);
}

void LinkedList_clear(void* llPtr) {
    SeenLinkedList* ll = (SeenLinkedList*)llPtr;
    LLNode* cur = ll->head;
    while (cur) {
        LLNode* next = cur->next;
        free(cur);
        cur = next;
    }
    ll->head = NULL;
    ll->tail = NULL;
    ll->length = 0;
}

// ============================================================================
// LinkedList<String> Runtime
// ============================================================================

typedef struct LLNodeStr {
    SeenString value;
    struct LLNodeStr* prev;
    struct LLNodeStr* next;
} LLNodeStr;

typedef struct {
    LLNodeStr* head;
    LLNodeStr* tail;
    int64_t length;
} SeenLinkedListStr;

void* LinkedList_new_str(void) {
    SeenLinkedListStr* ll = (SeenLinkedListStr*)malloc(sizeof(SeenLinkedListStr));
    ll->head = NULL;
    ll->tail = NULL;
    ll->length = 0;
    return ll;
}

void LinkedList_pushFront_str(void* llPtr, SeenString value) {
    SeenLinkedListStr* ll = (SeenLinkedListStr*)llPtr;
    LLNodeStr* node = (LLNodeStr*)malloc(sizeof(LLNodeStr));
    node->value = value;
    node->prev = NULL;
    node->next = ll->head;
    if (ll->head) ll->head->prev = node;
    ll->head = node;
    if (!ll->tail) ll->tail = node;
    ll->length++;
}

void LinkedList_pushBack_str(void* llPtr, SeenString value) {
    SeenLinkedListStr* ll = (SeenLinkedListStr*)llPtr;
    LLNodeStr* node = (LLNodeStr*)malloc(sizeof(LLNodeStr));
    node->value = value;
    node->prev = ll->tail;
    node->next = NULL;
    if (ll->tail) ll->tail->next = node;
    ll->tail = node;
    if (!ll->head) ll->head = node;
    ll->length++;
}

int64_t LinkedList_popFront_str(void* llPtr) {
    SeenLinkedListStr* ll = (SeenLinkedListStr*)llPtr;
    if (!ll->head) return BTreeMap_make_none_str();
    LLNodeStr* node = ll->head;
    SeenString val = node->value;
    ll->head = node->next;
    if (ll->head) ll->head->prev = NULL;
    else ll->tail = NULL;
    ll->length--;
    free(node);
    return BTreeMap_make_some_str(val);
}

int64_t LinkedList_popBack_str(void* llPtr) {
    SeenLinkedListStr* ll = (SeenLinkedListStr*)llPtr;
    if (!ll->tail) return BTreeMap_make_none_str();
    LLNodeStr* node = ll->tail;
    SeenString val = node->value;
    ll->tail = node->prev;
    if (ll->tail) ll->tail->next = NULL;
    else ll->head = NULL;
    ll->length--;
    free(node);
    return BTreeMap_make_some_str(val);
}

int64_t LinkedList_peekFront_str(void* llPtr) {
    SeenLinkedListStr* ll = (SeenLinkedListStr*)llPtr;
    if (!ll->head) return BTreeMap_make_none_str();
    return BTreeMap_make_some_str(ll->head->value);
}

int64_t LinkedList_peekBack_str(void* llPtr) {
    SeenLinkedListStr* ll = (SeenLinkedListStr*)llPtr;
    if (!ll->tail) return BTreeMap_make_none_str();
    return BTreeMap_make_some_str(ll->tail->value);
}

int64_t LinkedList_size_str(void* llPtr) {
    SeenLinkedListStr* ll = (SeenLinkedListStr*)llPtr;
    return ll->length;
}

bool LinkedList_isEmpty_str(void* llPtr) {
    SeenLinkedListStr* ll = (SeenLinkedListStr*)llPtr;
    return ll->length == 0;
}

void LinkedList_clear_str(void* llPtr) {
    SeenLinkedListStr* ll = (SeenLinkedListStr*)llPtr;
    LLNodeStr* cur = ll->head;
    while (cur) {
        LLNodeStr* next = cur->next;
        free(cur);
        cur = next;
    }
    ll->head = NULL;
    ll->tail = NULL;
    ll->length = 0;
}

// ============================================================================
// Mutex (pthread-based)
// ============================================================================

typedef struct { pthread_mutex_t m; } SeenMutex;

// Allocate and initialize a new mutex. Returns opaque handle (cast to int64_t).
int64_t __MutexCreate(void) {
    SeenMutex* mtx = (SeenMutex*)malloc(sizeof(SeenMutex));
    pthread_mutex_init(&mtx->m, NULL);
    return (int64_t)(uintptr_t)mtx;
}

// Destroy and free a mutex created by __MutexCreate.
void __MutexDestroy(int64_t handle) {
    SeenMutex* mtx = (SeenMutex*)(uintptr_t)handle;
    pthread_mutex_destroy(&mtx->m);
    free(mtx);
}

// Acquire the mutex, blocking until available.
void __MutexLock(int64_t handle) {
    SeenMutex* mtx = (SeenMutex*)(uintptr_t)handle;
    pthread_mutex_lock(&mtx->m);
}

// Release the mutex.
void __MutexUnlock(int64_t handle) {
    SeenMutex* mtx = (SeenMutex*)(uintptr_t)handle;
    pthread_mutex_unlock(&mtx->m);
}

// Try to acquire the mutex without blocking. Returns 1 if acquired, 0 otherwise.
int64_t __MutexTryLock(int64_t handle) {
    SeenMutex* mtx = (SeenMutex*)(uintptr_t)handle;
    return pthread_mutex_trylock(&mtx->m) == 0 ? 1 : 0;
}

// ============================================================================
// Atomic Operations (GCC __atomic builtins, no extra library needed)
// ============================================================================
//
// Atomic operations — handle-based pattern.
// AtomicInt/AtomicBool store a heap-allocated int64_t* disguised as int64_t handle.
// All operations receive the handle (pointer-as-int) and dereference it.

// Allocate a heap int64_t for AtomicInt/AtomicBool handle pattern.
int64_t __AtomicAlloc(int64_t initial) {
    int64_t* p = (int64_t*)malloc(sizeof(int64_t));
    __atomic_store_n(p, initial, __ATOMIC_SEQ_CST);
    return (int64_t)(uintptr_t)p;
}

int64_t __AtomicLoad(int64_t handle) {
    return __atomic_load_n((int64_t*)(uintptr_t)handle, __ATOMIC_SEQ_CST);
}

void __AtomicStore(int64_t handle, int64_t new_val) {
    __atomic_store_n((int64_t*)(uintptr_t)handle, new_val, __ATOMIC_SEQ_CST);
}

int64_t __AtomicAdd(int64_t handle, int64_t delta) {
    return __atomic_fetch_add((int64_t*)(uintptr_t)handle, delta, __ATOMIC_SEQ_CST);
}

int64_t __AtomicSub(int64_t handle, int64_t delta) {
    return __atomic_fetch_sub((int64_t*)(uintptr_t)handle, delta, __ATOMIC_SEQ_CST);
}

int64_t __AtomicCompareExchange(int64_t handle, int64_t expected, int64_t desired) {
    int64_t exp = expected;
    return __atomic_compare_exchange_n((int64_t*)(uintptr_t)handle, &exp, desired,
                                        0, __ATOMIC_SEQ_CST, __ATOMIC_SEQ_CST) ? 1 : 0;
}

// Ordering-specific variants (needed by SPSC queue)
int64_t __AtomicLoadRelaxed(int64_t handle) {
    return __atomic_load_n((int64_t*)(uintptr_t)handle, __ATOMIC_RELAXED);
}

int64_t __AtomicLoadAcquire(int64_t handle) {
    return __atomic_load_n((int64_t*)(uintptr_t)handle, __ATOMIC_ACQUIRE);
}

void __AtomicStoreRelease(int64_t handle, int64_t val) {
    __atomic_store_n((int64_t*)(uintptr_t)handle, val, __ATOMIC_RELEASE);
}

// AtomicBool variants (same pattern, bool stored as i64 0/1)
int64_t __AtomicLoadBool(int64_t handle) {
    return __atomic_load_n((int64_t*)(uintptr_t)handle, __ATOMIC_SEQ_CST);
}

void __AtomicStoreBool(int64_t handle, int64_t new_val) {
    __atomic_store_n((int64_t*)(uintptr_t)handle, new_val, __ATOMIC_SEQ_CST);
}

int64_t __AtomicCompareExchangeBool(int64_t handle, int64_t expected, int64_t desired) {
    int64_t exp = expected;
    return __atomic_compare_exchange_n((int64_t*)(uintptr_t)handle, &exp, desired,
                                        0, __ATOMIC_SEQ_CST, __ATOMIC_SEQ_CST) ? 1 : 0;
}

// ============================================================================
// Thread Sleep / Yield
// ============================================================================

// Sleep the current thread for `ms` milliseconds.
void __ThreadSleep(int64_t ms) {
#ifdef _WIN32
    Sleep((DWORD)ms);
#else
    struct timespec ts;
    ts.tv_sec  = ms / 1000;
    ts.tv_nsec = (ms % 1000) * 1000000L;
    nanosleep(&ts, NULL);
#endif
}

// ============================================================================
// Async Runtime Support
// ============================================================================

// Yield the current thread to the OS scheduler.
// Used by the async runtime to avoid busy-waiting when no tasks are ready.
void __thread_yield(void) {
#ifdef _WIN32
    SwitchToThread();
#else
    sched_yield();
#endif
}

// __ThreadYield is the Seen-facing name; __thread_yield is the legacy internal name.
void __ThreadYield(void) {
#ifdef _WIN32
    SwitchToThread();
#else
    sched_yield();
#endif
}

// Return an opaque thread ID for the current thread.
int64_t __ThreadCurrent(void) {
#ifdef _WIN32
    return (int64_t)GetCurrentThreadId();
#else
    return (int64_t)(uintptr_t)pthread_self();
#endif
}

// ============================================================================
// Raw Thread Create / Join (low-level, for internal compiler use)
// ============================================================================

typedef struct {
    int64_t (*fn)(int64_t);
    int64_t arg;
} SeenThreadArg;

static void* seen_thread_runner(void* raw) {
    SeenThreadArg* a = (SeenThreadArg*)raw;
    a->fn(a->arg);
    free(a);
    return NULL;
}

// Create a new thread that calls fn(arg). Returns opaque thread handle, or 0 on failure.
// fn_ptr is cast from an int64_t function pointer (unsafe, for internal use only).
int64_t __RawThreadCreate(int64_t fn_ptr, int64_t arg) {
    pthread_t* t = (pthread_t*)malloc(sizeof(pthread_t));
    SeenThreadArg* a = (SeenThreadArg*)malloc(sizeof(SeenThreadArg));
    a->fn = (int64_t (*)(int64_t))fn_ptr;
    a->arg = arg;
    if (pthread_create(t, NULL, seen_thread_runner, a) != 0) {
        free(t);
        free(a);
        return 0;
    }
    return (int64_t)(uintptr_t)t;
}

// Join (wait for) a thread created by __RawThreadCreate. Returns 0.
int64_t __RawThreadJoin(int64_t handle) {
    pthread_t* t = (pthread_t*)(uintptr_t)handle;
    pthread_join(*t, NULL);
    free(t);
    return 0;
}

// ============================================================================
// Channel — Unix pipe-backed message passing
// ============================================================================
// Pack read_fd<<32 | write_fd into a single int64_t handle.

int64_t __ChannelCreate(void) {
    int fds[2];
#ifdef _WIN32
    if (_pipe(fds, 8192, _O_BINARY) != 0) return -1;
#else
    if (pipe(fds) != 0) return -1;
#endif
    return ((int64_t)fds[0] << 32) | (int64_t)(uint32_t)fds[1];
}

int64_t __ChannelSend(int64_t handle, int64_t val) {
    int write_fd = (int)(handle & 0xFFFFFFFF);
#ifdef _WIN32
    int n = _write(write_fd, &val, (unsigned int)sizeof(val));
    return n == (int)sizeof(val) ? 1 : 0;
#else
    ssize_t n = write(write_fd, &val, sizeof(val));
    return n == (ssize_t)sizeof(val) ? 1 : 0;
#endif
}

int64_t __ChannelReceive(int64_t handle) {
    int read_fd = (int)((uint64_t)handle >> 32);
    int64_t val = 0;
#ifdef _WIN32
    int n = _read(read_fd, &val, (unsigned int)sizeof(val));
    (void)n;
#else
    ssize_t n = read(read_fd, &val, sizeof(val));
    (void)n;
#endif
    return val;
}

void __ChannelClose(int64_t handle) {
#ifdef _WIN32
    _close((int)((uint64_t)handle >> 32));
    _close((int)(handle & 0xFFFFFFFF));
#else
    close((int)((uint64_t)handle >> 32));
    close((int)(handle & 0xFFFFFFFF));
#endif
}

// Abort the program with an error message.
// Used by Future.block() and other runtime code when an unrecoverable error occurs.
// Takes a SeenString (len + data pointer) since Seen String = %SeenString struct.
void __panic(int64_t len, char* data) {
    fprintf(stderr, "PANIC: %.*s\n", (int)len, data);
#if !defined(_WIN32) && !defined(__ANDROID__)
    // Print stack trace (backtrace not available on Windows/mingw)
    void* bt_buf[64];
    int bt_size = backtrace(bt_buf, 64);
    if (bt_size > 0) {
        fprintf(stderr, "\nStack trace:\n");
        backtrace_symbols_fd(bt_buf, bt_size, 2);  // fd 2 = stderr
    }
#endif
    abort();
}

// ---------- LLVM Coroutine Frame Wrappers ----------
//
// LLVM switched-resume coroutine frame layout (stable since LLVM 5):
//   [0..8)   resume function pointer (NULL at final suspend = done)
//   [8..16)  destroy function pointer
//   [16..)   promise area (user-defined return value storage)
//
// These C wrappers are safe for operations that only READ memory (done, promise).
// For operations that CALL function pointers (resume, destroy), use the LLVM
// intrinsics instead — coroutine lowering uses `fastcc` calling convention,
// so C wrappers would call with the wrong convention and crash.
//
// Seen code uses `llvm_coro_resume`/`llvm_coro_destroy` which the codegen
// maps to `@llvm.coro.resume`/`@llvm.coro.destroy` LLVM intrinsics.

int64_t seen_coro_done(void *handle) {
    void **frame = (void **)handle;
    return frame[0] == NULL ? 1 : 0;
}

void *seen_coro_promise(void *handle) {
    return (void *)((char *)handle + 16);
}

// Extract i64 return value from coroutine promise frame (offset 16)
// Safe: coroutine has finished (done check passed) but NOT yet destroyed.
int64_t __seen_promise_load_i64(void *handle) {
    int64_t value;
    memcpy(&value, (char *)handle + 16, 8);
    return value;
}

// Extract double (Float) return value from coroutine promise frame (offset 16)
double __seen_promise_load_f64(void *handle) {
    double value;
    memcpy(&value, (char *)handle + 16, 8);
    return value;
}

// ============================================================================
// Stack Region Allocator
// ============================================================================
// LIFO bump allocator: alloc grows a watermark, pop rewinds it.
// No per-object free — free in reverse order only.

typedef struct {
    char  *base;
    size_t capacity;
    size_t offset;   // current watermark
} SeenStackRegion;

int64_t seen_stack_region_new(int64_t capacity) {
    SeenStackRegion *sr = (SeenStackRegion *)malloc(sizeof(SeenStackRegion));
    if (!sr) return 0;
    sr->base = (char *)malloc((size_t)capacity);
    if (!sr->base) { free(sr); return 0; }
    sr->capacity = (size_t)capacity;
    sr->offset   = 0;
    return (int64_t)(uintptr_t)sr;
}

int64_t seen_stack_region_alloc(int64_t handle, int64_t size) {
    SeenStackRegion *sr = (SeenStackRegion *)(uintptr_t)handle;
    size_t aligned = ((size_t)size + 15) & ~(size_t)15;  // 16-byte align
    if (sr->offset + aligned > sr->capacity) return 0;
    char *ptr = sr->base + sr->offset;
    sr->offset += aligned;
    return (int64_t)(uintptr_t)ptr;
}

void seen_stack_region_pop(int64_t handle, int64_t size) {
    SeenStackRegion *sr = (SeenStackRegion *)(uintptr_t)handle;
    size_t aligned = ((size_t)size + 15) & ~(size_t)15;
    if (sr->offset >= aligned) sr->offset -= aligned;
    else sr->offset = 0;
}

void seen_stack_region_reset(int64_t handle) {
    SeenStackRegion *sr = (SeenStackRegion *)(uintptr_t)handle;
    sr->offset = 0;
}

void seen_stack_region_destroy(int64_t handle) {
    SeenStackRegion *sr = (SeenStackRegion *)(uintptr_t)handle;
    if (sr) { free(sr->base); free(sr); }
}

int64_t seen_stack_region_remaining(int64_t handle) {
    SeenStackRegion *sr = (SeenStackRegion *)(uintptr_t)handle;
    return (int64_t)(sr->capacity - sr->offset);
}

// ============================================================================
// Pool Region Allocator
// ============================================================================
// Fixed-size block pool using a free list. O(1) alloc and free.

typedef struct SeenPoolBlock {
    struct SeenPoolBlock *next;
} SeenPoolBlock;

typedef struct {
    char   *base;
    size_t  block_size;   // user-visible block size (aligned to 16)
    size_t  capacity;     // total number of blocks
    size_t  used;
    SeenPoolBlock *free_list;
} SeenPoolRegion;

int64_t seen_pool_region_new(int64_t block_size, int64_t count) {
    size_t bs = ((size_t)block_size + 15) & ~(size_t)15;
    if (bs < sizeof(SeenPoolBlock)) bs = sizeof(SeenPoolBlock);
    SeenPoolRegion *pr = (SeenPoolRegion *)malloc(sizeof(SeenPoolRegion));
    if (!pr) return 0;
    pr->base = (char *)malloc(bs * (size_t)count);
    if (!pr->base) { free(pr); return 0; }
    pr->block_size = bs;
    pr->capacity   = (size_t)count;
    pr->used       = 0;
    // Build free list
    pr->free_list = NULL;
    for (size_t i = (size_t)count; i > 0; --i) {
        SeenPoolBlock *blk = (SeenPoolBlock *)(pr->base + (i - 1) * bs);
        blk->next = pr->free_list;
        pr->free_list = blk;
    }
    return (int64_t)(uintptr_t)pr;
}

int64_t seen_pool_region_alloc(int64_t handle) {
    SeenPoolRegion *pr = (SeenPoolRegion *)(uintptr_t)handle;
    if (!pr->free_list) return 0;
    SeenPoolBlock *blk = pr->free_list;
    pr->free_list = blk->next;
    pr->used++;
    return (int64_t)(uintptr_t)blk;
}

void seen_pool_region_free(int64_t handle, int64_t ptr) {
    SeenPoolRegion *pr = (SeenPoolRegion *)(uintptr_t)handle;
    SeenPoolBlock *blk = (SeenPoolBlock *)(uintptr_t)ptr;
    blk->next = pr->free_list;
    pr->free_list = blk;
    if (pr->used > 0) pr->used--;
}

void seen_pool_region_destroy(int64_t handle) {
    SeenPoolRegion *pr = (SeenPoolRegion *)(uintptr_t)handle;
    if (pr) { free(pr->base); free(pr); }
}

void seen_pool_region_reset(int64_t handle) {
    SeenPoolRegion *pr = (SeenPoolRegion *)(uintptr_t)handle;
    pr->used = 0;
    pr->free_list = NULL;
    for (size_t i = pr->capacity; i > 0; --i) {
        SeenPoolBlock *blk = (SeenPoolBlock *)(pr->base + (i - 1) * pr->block_size);
        blk->next = pr->free_list;
        pr->free_list = blk;
    }
}

int64_t seen_pool_region_available(int64_t handle) {
    SeenPoolRegion *pr = (SeenPoolRegion *)(uintptr_t)handle;
    return (int64_t)(pr->capacity - pr->used);
}

// ============================================================================
// Memory-Mapped Region
// ============================================================================

#ifndef _WIN32
#include <sys/mman.h>
#include <fcntl.h>
// MAP_ANONYMOUS may not be defined on macOS with strict C standards
#ifndef MAP_ANONYMOUS
#ifdef MAP_ANON
#define MAP_ANONYMOUS MAP_ANON
#else
#define MAP_ANONYMOUS 0x1000  // macOS value
#endif
#endif
#endif

typedef struct {
    void  *addr;
    size_t length;
    int    fd;       // -1 for anonymous mappings
} SeenMappedRegion;

// flags: 0 = read-only file, 1 = read-write file, 2 = anonymous rw
int64_t seen_mapped_new(int64_t path_len, char *path_data, int64_t size, int64_t flags) {
    SeenMappedRegion *mr = (SeenMappedRegion *)malloc(sizeof(SeenMappedRegion));
    if (!mr) return 0;
    mr->length = (size_t)size;
    mr->fd = -1;

#ifdef _WIN32
    if (flags == 2) {
        // Anonymous mapping: use VirtualAlloc
        mr->addr = VirtualAlloc(NULL, mr->length, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE);
    } else {
        // File-backed mapping
        char pathbuf[4096];
        size_t plen = (size_t)path_len < 4095 ? (size_t)path_len : 4095;
        memcpy(pathbuf, path_data, plen);
        pathbuf[plen] = '\0';

        DWORD access = (flags == 1) ? GENERIC_READ | GENERIC_WRITE : GENERIC_READ;
        DWORD share = FILE_SHARE_READ;
        HANDLE hFile = CreateFileA(pathbuf, access, share, NULL, OPEN_EXISTING,
                                   FILE_ATTRIBUTE_NORMAL, NULL);
        if (hFile == INVALID_HANDLE_VALUE) { free(mr); return 0; }

        DWORD fProtect = (flags == 1) ? PAGE_READWRITE : PAGE_READONLY;
        HANDLE hMap = CreateFileMappingA(hFile, NULL, fProtect,
                                         (DWORD)(mr->length >> 32),
                                         (DWORD)(mr->length & 0xFFFFFFFF), NULL);
        if (!hMap) { CloseHandle(hFile); free(mr); return 0; }

        DWORD mapAccess = (flags == 1) ? FILE_MAP_WRITE : FILE_MAP_READ;
        mr->addr = MapViewOfFile(hMap, mapAccess, 0, 0, mr->length);
        CloseHandle(hMap);
        // Store the file handle as fd (cast for later cleanup)
        mr->fd = (int)(intptr_t)hFile;
    }
    if (!mr->addr) {
        if (mr->fd >= 0) CloseHandle((HANDLE)(intptr_t)mr->fd);
        free(mr);
        return 0;
    }
#else
    if (flags == 2) {
        // Anonymous mapping
        mr->addr = mmap(NULL, mr->length, PROT_READ | PROT_WRITE,
                         MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    } else {
        // File-backed mapping
        char pathbuf[4096];
        size_t plen = (size_t)path_len < 4095 ? (size_t)path_len : 4095;
        memcpy(pathbuf, path_data, plen);
        pathbuf[plen] = '\0';

        int oflags = (flags == 1) ? O_RDWR : O_RDONLY;
        mr->fd = open(pathbuf, oflags);
        if (mr->fd < 0) { free(mr); return 0; }

        int prot = PROT_READ | ((flags == 1) ? PROT_WRITE : 0);
        int mflags = (flags == 1) ? MAP_SHARED : MAP_PRIVATE;
        mr->addr = mmap(NULL, mr->length, prot, mflags, mr->fd, 0);
    }

    if (mr->addr == MAP_FAILED) {
        if (mr->fd >= 0) close(mr->fd);
        free(mr);
        return 0;
    }
#endif
    return (int64_t)(uintptr_t)mr;
}

int64_t seen_mapped_data(int64_t handle) {
    SeenMappedRegion *mr = (SeenMappedRegion *)(uintptr_t)handle;
    return (int64_t)(uintptr_t)mr->addr;
}

void seen_mapped_sync(int64_t handle) {
    SeenMappedRegion *mr = (SeenMappedRegion *)(uintptr_t)handle;
#ifdef _WIN32
    FlushViewOfFile(mr->addr, mr->length);
#else
    msync(mr->addr, mr->length, MS_SYNC);
#endif
}

void seen_mapped_free(int64_t handle) {
    SeenMappedRegion *mr = (SeenMappedRegion *)(uintptr_t)handle;
    if (mr) {
#ifdef _WIN32
        if (mr->addr) UnmapViewOfFile(mr->addr);
        if (mr->fd >= 0) CloseHandle((HANDLE)(intptr_t)mr->fd);
#else
        munmap(mr->addr, mr->length);
        if (mr->fd >= 0) close(mr->fd);
#endif
        free(mr);
    }
}

int64_t seen_mapped_length(int64_t handle) {
    SeenMappedRegion *mr = (SeenMappedRegion *)(uintptr_t)handle;
    return (int64_t)mr->length;
}

// ============================================================================
// RwLock (Reader-Writer Lock)
// ============================================================================

#ifdef _WIN32
// On Windows, SRWLOCK requires separate shared/exclusive unlock calls
typedef struct { SRWLOCK rw; } SeenRwLock;

int64_t seen_rwlock_new(void) {
    SeenRwLock *l = (SeenRwLock *)malloc(sizeof(SeenRwLock));
    if (!l) return 0;
    InitializeSRWLock(&l->rw);
    return (int64_t)(uintptr_t)l;
}

void seen_rwlock_read_lock(int64_t handle) {
    SeenRwLock *l = (SeenRwLock *)(uintptr_t)handle;
    AcquireSRWLockShared(&l->rw);
}

void seen_rwlock_read_unlock(int64_t handle) {
    SeenRwLock *l = (SeenRwLock *)(uintptr_t)handle;
    ReleaseSRWLockShared(&l->rw);
}

void seen_rwlock_write_lock(int64_t handle) {
    SeenRwLock *l = (SeenRwLock *)(uintptr_t)handle;
    AcquireSRWLockExclusive(&l->rw);
}

void seen_rwlock_write_unlock(int64_t handle) {
    SeenRwLock *l = (SeenRwLock *)(uintptr_t)handle;
    ReleaseSRWLockExclusive(&l->rw);
}

void seen_rwlock_destroy(int64_t handle) {
    SeenRwLock *l = (SeenRwLock *)(uintptr_t)handle;
    // SRWLOCK has no destroy function
    if (l) free(l);
}
#else
typedef struct { pthread_rwlock_t rw; } SeenRwLock;

int64_t seen_rwlock_new(void) {
    SeenRwLock *l = (SeenRwLock *)malloc(sizeof(SeenRwLock));
    if (!l) return 0;
    pthread_rwlock_init(&l->rw, NULL);
    return (int64_t)(uintptr_t)l;
}

void seen_rwlock_read_lock(int64_t handle) {
    SeenRwLock *l = (SeenRwLock *)(uintptr_t)handle;
    pthread_rwlock_rdlock(&l->rw);
}

void seen_rwlock_read_unlock(int64_t handle) {
    SeenRwLock *l = (SeenRwLock *)(uintptr_t)handle;
    pthread_rwlock_unlock(&l->rw);
}

void seen_rwlock_write_lock(int64_t handle) {
    SeenRwLock *l = (SeenRwLock *)(uintptr_t)handle;
    pthread_rwlock_wrlock(&l->rw);
}

void seen_rwlock_write_unlock(int64_t handle) {
    SeenRwLock *l = (SeenRwLock *)(uintptr_t)handle;
    pthread_rwlock_unlock(&l->rw);
}

void seen_rwlock_destroy(int64_t handle) {
    SeenRwLock *l = (SeenRwLock *)(uintptr_t)handle;
    if (l) { pthread_rwlock_destroy(&l->rw); free(l); }
}
#endif

// ============================================================================
// Barrier
// ============================================================================

#ifdef _WIN32
// Manual barrier using CRITICAL_SECTION + CONDITION_VARIABLE
typedef struct {
    CRITICAL_SECTION lock;
    CONDITION_VARIABLE cond;
    unsigned count;
    unsigned waiting;
    unsigned phase;
} SeenBarrier;

int64_t seen_barrier_new(int64_t count) {
    SeenBarrier *b = (SeenBarrier *)malloc(sizeof(SeenBarrier));
    if (!b) return 0;
    InitializeCriticalSection(&b->lock);
    InitializeConditionVariable(&b->cond);
    b->count = (unsigned)count;
    b->waiting = 0;
    b->phase = 0;
    return (int64_t)(uintptr_t)b;
}

int64_t seen_barrier_wait(int64_t handle) {
    SeenBarrier *b = (SeenBarrier *)(uintptr_t)handle;
    EnterCriticalSection(&b->lock);
    unsigned phase = b->phase;
    b->waiting++;
    if (b->waiting == b->count) {
        b->waiting = 0;
        b->phase++;
        LeaveCriticalSection(&b->lock);
        WakeAllConditionVariable(&b->cond);
        return 1; // serial thread
    }
    while (b->phase == phase) {
        SleepConditionVariableCS(&b->cond, &b->lock, INFINITE);
    }
    LeaveCriticalSection(&b->lock);
    return 0;
}

void seen_barrier_destroy(int64_t handle) {
    SeenBarrier *b = (SeenBarrier *)(uintptr_t)handle;
    if (b) {
        DeleteCriticalSection(&b->lock);
        free(b);
    }
}
#else
// macOS doesn't have pthread_barrier_t, so we provide a polyfill using mutex+cond
#ifdef __APPLE__
typedef struct {
    pthread_mutex_t mutex;
    pthread_cond_t cond;
    unsigned count;
    unsigned waiting;
    unsigned phase;
} seen_pthread_barrier_t;

static int seen_pthread_barrier_init(seen_pthread_barrier_t *b, unsigned count) {
    pthread_mutex_init(&b->mutex, NULL);
    pthread_cond_init(&b->cond, NULL);
    b->count = count;
    b->waiting = 0;
    b->phase = 0;
    return 0;
}

static int seen_pthread_barrier_wait(seen_pthread_barrier_t *b) {
    pthread_mutex_lock(&b->mutex);
    unsigned phase = b->phase;
    b->waiting++;
    if (b->waiting == b->count) {
        b->waiting = 0;
        b->phase++;
        pthread_cond_broadcast(&b->cond);
        pthread_mutex_unlock(&b->mutex);
        return 1; // serial thread
    }
    while (phase == b->phase) {
        pthread_cond_wait(&b->cond, &b->mutex);
    }
    pthread_mutex_unlock(&b->mutex);
    return 0;
}

static void seen_pthread_barrier_destroy(seen_pthread_barrier_t *b) {
    pthread_mutex_destroy(&b->mutex);
    pthread_cond_destroy(&b->cond);
}

typedef struct { seen_pthread_barrier_t b; } SeenBarrier;

int64_t seen_barrier_new(int64_t count) {
    SeenBarrier *b = (SeenBarrier *)malloc(sizeof(SeenBarrier));
    if (!b) return 0;
    seen_pthread_barrier_init(&b->b, (unsigned)count);
    return (int64_t)(uintptr_t)b;
}

int64_t seen_barrier_wait(int64_t handle) {
    SeenBarrier *b = (SeenBarrier *)(uintptr_t)handle;
    int rc = seen_pthread_barrier_wait(&b->b);
    return rc ? 1 : 0;
}

void seen_barrier_destroy(int64_t handle) {
    SeenBarrier *b = (SeenBarrier *)(uintptr_t)handle;
    if (b) { seen_pthread_barrier_destroy(&b->b); free(b); }
}
#else
typedef struct { pthread_barrier_t b; } SeenBarrier;

int64_t seen_barrier_new(int64_t count) {
    SeenBarrier *b = (SeenBarrier *)malloc(sizeof(SeenBarrier));
    if (!b) return 0;
    pthread_barrier_init(&b->b, NULL, (unsigned)count);
    return (int64_t)(uintptr_t)b;
}

int64_t seen_barrier_wait(int64_t handle) {
    SeenBarrier *b = (SeenBarrier *)(uintptr_t)handle;
    int rc = pthread_barrier_wait(&b->b);
    return (rc == PTHREAD_BARRIER_SERIAL_THREAD) ? 1 : 0;
}

void seen_barrier_destroy(int64_t handle) {
    SeenBarrier *b = (SeenBarrier *)(uintptr_t)handle;
    if (b) { pthread_barrier_destroy(&b->b); free(b); }
}
#endif
#endif

// ============================================================================
// Thread-Local Storage
// ============================================================================

#ifdef _WIN32
int64_t seen_tls_new(void) {
    DWORD key = TlsAlloc();
    if (key == TLS_OUT_OF_INDEXES) return -1;
    return (int64_t)key;
}

void seen_tls_set(int64_t key, int64_t value) {
    TlsSetValue((DWORD)key, (LPVOID)(uintptr_t)value);
}

int64_t seen_tls_get(int64_t key) {
    return (int64_t)(uintptr_t)TlsGetValue((DWORD)key);
}

void seen_tls_destroy(int64_t key) {
    TlsFree((DWORD)key);
}
#else
int64_t seen_tls_new(void) {
    pthread_key_t key;
    if (pthread_key_create(&key, NULL) != 0) return -1;
    return (int64_t)key;
}

void seen_tls_set(int64_t key, int64_t value) {
    pthread_setspecific((pthread_key_t)key, (void *)(uintptr_t)value);
}

int64_t seen_tls_get(int64_t key) {
    return (int64_t)(uintptr_t)pthread_getspecific((pthread_key_t)key);
}

void seen_tls_destroy(int64_t key) {
    pthread_key_delete((pthread_key_t)key);
}
#endif

// ============================================================================
// Work-Stealing Thread Pool
// ============================================================================
// Fixed-size pool with per-worker deques and work stealing.

#define SEEN_WS_DEQUE_CAP 4096

typedef struct {
    int64_t (*fn)(int64_t);
    int64_t  arg;
} SeenTask;

typedef struct {
    SeenTask     tasks[SEEN_WS_DEQUE_CAP];
    volatile int top;     // owner pushes/pops here
    volatile int bottom;  // thieves steal from here
    pthread_mutex_t lock; // protects steal
} SeenDeque;

typedef struct {
    SeenDeque   *deques;      // one per worker
    pthread_t   *threads;
    int          nworkers;
    volatile int shutdown;
    // Global submit queue (for external submissions)
    SeenTask     submit_queue[SEEN_WS_DEQUE_CAP];
    volatile int submit_top;
    volatile int submit_bottom;
    pthread_mutex_t submit_lock;
    pthread_cond_t  submit_cond;
} SeenWSPool;

static void deque_init(SeenDeque *d) {
    d->top = 0; d->bottom = 0;
    pthread_mutex_init(&d->lock, NULL);
}

static int deque_push(SeenDeque *d, SeenTask t) {
    int top = d->top;
    if (top - d->bottom >= SEEN_WS_DEQUE_CAP) return 0;
    d->tasks[top % SEEN_WS_DEQUE_CAP] = t;
    __atomic_store_n(&d->top, top + 1, __ATOMIC_RELEASE);
    return 1;
}

static int deque_pop(SeenDeque *d, SeenTask *out) {
    int top = __atomic_load_n(&d->top, __ATOMIC_ACQUIRE) - 1;
    __atomic_store_n(&d->top, top, __ATOMIC_RELEASE);
    int bottom = __atomic_load_n(&d->bottom, __ATOMIC_ACQUIRE);
    if (top > bottom) {
        *out = d->tasks[top % SEEN_WS_DEQUE_CAP];
        return 1;
    }
    if (top == bottom) {
        // Race with stealer
        pthread_mutex_lock(&d->lock);
        if (__atomic_load_n(&d->bottom, __ATOMIC_RELAXED) == bottom) {
            *out = d->tasks[top % SEEN_WS_DEQUE_CAP];
            __atomic_store_n(&d->bottom, bottom + 1, __ATOMIC_RELEASE);
            __atomic_store_n(&d->top, top + 1, __ATOMIC_RELEASE);
            pthread_mutex_unlock(&d->lock);
            return 1;
        }
        __atomic_store_n(&d->top, top + 1, __ATOMIC_RELEASE);
        pthread_mutex_unlock(&d->lock);
    } else {
        __atomic_store_n(&d->top, top + 1, __ATOMIC_RELEASE);
    }
    return 0;
}

static int deque_steal(SeenDeque *d, SeenTask *out) {
    pthread_mutex_lock(&d->lock);
    int bottom = __atomic_load_n(&d->bottom, __ATOMIC_ACQUIRE);
    int top = __atomic_load_n(&d->top, __ATOMIC_ACQUIRE);
    if (bottom < top) {
        *out = d->tasks[bottom % SEEN_WS_DEQUE_CAP];
        __atomic_store_n(&d->bottom, bottom + 1, __ATOMIC_RELEASE);
        pthread_mutex_unlock(&d->lock);
        return 1;
    }
    pthread_mutex_unlock(&d->lock);
    return 0;
}

static void *ws_worker(void *arg) {
    // arg encodes pool pointer in high bits, worker id in low byte
    int64_t packed = (int64_t)(uintptr_t)arg;
    SeenWSPool *pool = (SeenWSPool *)(uintptr_t)(packed & ~0xFFLL);
    int id = (int)(packed & 0xFF);
    SeenDeque *my = &pool->deques[id];
    SeenTask task;

    while (!pool->shutdown) {
        // Try own deque first
        if (deque_pop(my, &task)) {
            task.fn(task.arg);
            continue;
        }
        // Try stealing from others
        int stolen = 0;
        for (int i = 0; i < pool->nworkers && !stolen; i++) {
            if (i == id) continue;
            if (deque_steal(&pool->deques[i], &task)) {
                task.fn(task.arg);
                stolen = 1;
            }
        }
        if (stolen) continue;
        // Try global submit queue
        pthread_mutex_lock(&pool->submit_lock);
        if (pool->submit_bottom < pool->submit_top) {
            task = pool->submit_queue[pool->submit_bottom % SEEN_WS_DEQUE_CAP];
            pool->submit_bottom++;
            pthread_mutex_unlock(&pool->submit_lock);
            // Push to local deque and execute
            deque_push(my, task);
            if (deque_pop(my, &task)) task.fn(task.arg);
            continue;
        }
        // Wait for work
#ifdef _WIN32
        SleepConditionVariableCS(&pool->submit_cond, &pool->submit_lock, 1);
        LeaveCriticalSection(&pool->submit_lock);
#else
        struct timespec ts;
        clock_gettime(CLOCK_REALTIME, &ts);
        ts.tv_nsec += 1000000; // 1ms timeout
        if (ts.tv_nsec >= 1000000000) { ts.tv_sec++; ts.tv_nsec -= 1000000000; }
        pthread_cond_timedwait(&pool->submit_cond, &pool->submit_lock, &ts);
        pthread_mutex_unlock(&pool->submit_lock);
#endif
    }
    return NULL;
}

int64_t seen_ws_pool_new(int64_t nworkers) {
    int n = (int)nworkers;
    if (n <= 0) n = 4;
    if (n > 255) n = 255; // packed encoding limit
    SeenWSPool *pool = (SeenWSPool *)calloc(1, sizeof(SeenWSPool));
    if (!pool) return 0;
    pool->nworkers = n;
    pool->shutdown = 0;
    pool->submit_top = 0;
    pool->submit_bottom = 0;
    pthread_mutex_init(&pool->submit_lock, NULL);
    pthread_cond_init(&pool->submit_cond, NULL);
    pool->deques  = (SeenDeque *)calloc((size_t)n, sizeof(SeenDeque));
    pool->threads = (pthread_t *)calloc((size_t)n, sizeof(pthread_t));
    for (int i = 0; i < n; i++) deque_init(&pool->deques[i]);
    for (int i = 0; i < n; i++) {
        int64_t packed = (int64_t)(uintptr_t)pool | (int64_t)i;
        pthread_create(&pool->threads[i], NULL, ws_worker, (void *)(uintptr_t)packed);
    }
    return (int64_t)(uintptr_t)pool;
}

void seen_ws_pool_submit(int64_t pool_handle, int64_t fn_ptr, int64_t arg) {
    SeenWSPool *pool = (SeenWSPool *)(uintptr_t)pool_handle;
    SeenTask task;
    task.fn  = (int64_t (*)(int64_t))fn_ptr;
    task.arg = arg;
    pthread_mutex_lock(&pool->submit_lock);
    if (pool->submit_top - pool->submit_bottom < SEEN_WS_DEQUE_CAP) {
        pool->submit_queue[pool->submit_top % SEEN_WS_DEQUE_CAP] = task;
        pool->submit_top++;
    }
    pthread_cond_signal(&pool->submit_cond);
    pthread_mutex_unlock(&pool->submit_lock);
}

void seen_ws_pool_shutdown(int64_t pool_handle) {
    SeenWSPool *pool = (SeenWSPool *)(uintptr_t)pool_handle;
    pool->shutdown = 1;
    // Wake all workers
    pthread_mutex_lock(&pool->submit_lock);
    pthread_cond_broadcast(&pool->submit_cond);
    pthread_mutex_unlock(&pool->submit_lock);
    for (int i = 0; i < pool->nworkers; i++) {
        pthread_join(pool->threads[i], NULL);
    }
    for (int i = 0; i < pool->nworkers; i++) {
        pthread_mutex_destroy(&pool->deques[i].lock);
    }
    pthread_mutex_destroy(&pool->submit_lock);
    pthread_cond_destroy(&pool->submit_cond);
    free(pool->deques);
    free(pool->threads);
    free(pool);
}

// ============================================================================
// Parallel For
// ============================================================================

typedef struct {
    int64_t (*body)(int64_t);
    int64_t start;
    int64_t end;
} SeenPForArgs;

static void* seen_pfor_worker(void* arg) {
    SeenPForArgs* a = (SeenPForArgs*)arg;
    for (int64_t i = a->start; i < a->end; i++) {
        a->body(i);
    }
    return NULL;
}

void seen_parallel_for(int64_t start, int64_t end, int64_t fn_ptr, int64_t nthreads) {
    int64_t (*body)(int64_t) = (int64_t (*)(int64_t))(uintptr_t)fn_ptr;
    int64_t range = end - start;
    if (range <= 0) return;

    int64_t nt = nthreads;
    if (nt <= 0) {
        nt = 4; // default thread count
    }
    if (nt > range) nt = range;
    if (nt > 64) nt = 64;

    if (nt == 1) {
        // Just run sequentially
        for (int64_t i = start; i < end; i++) body(i);
        return;
    }

    pthread_t* threads = (pthread_t*)malloc(sizeof(pthread_t) * nt);
    SeenPForArgs* args = (SeenPForArgs*)malloc(sizeof(SeenPForArgs) * nt);
    int64_t chunk = range / nt;
    int64_t remainder = range % nt;
    int64_t cur = start;

    for (int64_t t = 0; t < nt; t++) {
        args[t].body = body;
        args[t].start = cur;
        args[t].end = cur + chunk + (t < remainder ? 1 : 0);
        cur = args[t].end;
        pthread_create(&threads[t], NULL, seen_pfor_worker, &args[t]);
    }
    for (int64_t t = 0; t < nt; t++) {
        pthread_join(threads[t], NULL);
    }
    free(threads);
    free(args);
}

// ============================================================================
// Lock-Free Data Structures
// ============================================================================

// MPMC AtomicQueue — CAS ring buffer (power-of-2 capacity)
// Uses GCC __atomic builtins (matching existing atomic pattern in this file)
typedef struct {
    int64_t* slots;
    volatile int64_t head;
    volatile int64_t tail;
    int64_t mask; // capacity - 1
    int64_t capacity;
} SeenAtomicQueue;

int64_t seen_atomic_queue_new(int64_t capacity) {
    // Round up to power of 2
    int64_t cap = 1;
    while (cap < capacity) cap <<= 1;
    if (cap < 16) cap = 16;

    SeenAtomicQueue* q = (SeenAtomicQueue*)calloc(1, sizeof(SeenAtomicQueue));
    q->slots = (int64_t*)calloc(cap, sizeof(int64_t));
    __atomic_store_n(&q->head, 0, __ATOMIC_RELAXED);
    __atomic_store_n(&q->tail, 0, __ATOMIC_RELAXED);
    q->mask = cap - 1;
    q->capacity = cap;
    return (int64_t)(uintptr_t)q;
}

int64_t seen_atomic_queue_push(int64_t handle, int64_t value) {
    SeenAtomicQueue* q = (SeenAtomicQueue*)(uintptr_t)handle;
    int64_t head = __atomic_load_n(&q->head, __ATOMIC_RELAXED);
    for (;;) {
        int64_t tail = __atomic_load_n(&q->tail, __ATOMIC_ACQUIRE);
        if (head - tail >= q->capacity) return 0; // full
        if (__atomic_compare_exchange_n(&q->head, &head, head + 1, 1,
                __ATOMIC_ACQ_REL, __ATOMIC_RELAXED)) {
            // Store value; use -1 as sentinel when user pushes 0
            __atomic_store_n(&q->slots[head & q->mask], value != 0 ? value : -1, __ATOMIC_RELEASE);
            return 1; // success
        }
    }
}

int64_t seen_atomic_queue_pop(int64_t handle) {
    SeenAtomicQueue* q = (SeenAtomicQueue*)(uintptr_t)handle;
    int64_t tail = __atomic_load_n(&q->tail, __ATOMIC_RELAXED);
    for (;;) {
        int64_t head = __atomic_load_n(&q->head, __ATOMIC_ACQUIRE);
        if (tail >= head) return 0; // empty
        int64_t val = __atomic_load_n(&q->slots[tail & q->mask], __ATOMIC_ACQUIRE);
        if (val == 0) return 0; // slot not yet written
        if (__atomic_compare_exchange_n(&q->tail, &tail, tail + 1, 1,
                __ATOMIC_ACQ_REL, __ATOMIC_RELAXED)) {
            __atomic_store_n(&q->slots[tail & q->mask], 0, __ATOMIC_RELEASE);
            return val == -1 ? 0 : val;
        }
    }
}

void seen_atomic_queue_destroy(int64_t handle) {
    SeenAtomicQueue* q = (SeenAtomicQueue*)(uintptr_t)handle;
    free(q->slots);
    free(q);
}

// Treiber Stack — CAS linked list
typedef struct SeenAStackNode {
    int64_t value;
    struct SeenAStackNode* next;
} SeenAStackNode;

typedef struct {
    SeenAStackNode* volatile top;
} SeenAtomicStack;

int64_t seen_atomic_stack_new(void) {
    SeenAtomicStack* s = (SeenAtomicStack*)calloc(1, sizeof(SeenAtomicStack));
    __atomic_store_n(&s->top, NULL, __ATOMIC_RELAXED);
    return (int64_t)(uintptr_t)s;
}

void seen_atomic_stack_push(int64_t handle, int64_t value) {
    SeenAtomicStack* s = (SeenAtomicStack*)(uintptr_t)handle;
    SeenAStackNode* node = (SeenAStackNode*)malloc(sizeof(SeenAStackNode));
    node->value = value;
    node->next = __atomic_load_n(&s->top, __ATOMIC_RELAXED);
    while (!__atomic_compare_exchange_n(&s->top, &node->next, node, 1,
            __ATOMIC_RELEASE, __ATOMIC_RELAXED)) {
        // retry — node->next updated by CAS
    }
}

int64_t seen_atomic_stack_pop(int64_t handle) {
    SeenAtomicStack* s = (SeenAtomicStack*)(uintptr_t)handle;
    SeenAStackNode* top = __atomic_load_n(&s->top, __ATOMIC_ACQUIRE);
    while (top != NULL) {
        if (__atomic_compare_exchange_n(&s->top, &top, top->next, 1,
                __ATOMIC_ACQ_REL, __ATOMIC_RELAXED)) {
            int64_t val = top->value;
            free(top);
            return val;
        }
        // retry — top updated by CAS
    }
    return 0; // empty
}

void seen_atomic_stack_destroy(int64_t handle) {
    SeenAtomicStack* s = (SeenAtomicStack*)(uintptr_t)handle;
    SeenAStackNode* node = __atomic_load_n(&s->top, __ATOMIC_RELAXED);
    while (node) {
        SeenAStackNode* next = node->next;
        free(node);
        node = next;
    }
    free(s);
}

// ============================================================================
// CPU Affinity
// ============================================================================

#if defined(__linux__) && !defined(_WIN32)
#include <sched.h>
#endif

int64_t seen_set_thread_affinity(int64_t core_id) {
#if defined(_WIN32)
    DWORD_PTR mask = (DWORD_PTR)1 << core_id;
    return (SetThreadAffinityMask(GetCurrentThread(), mask) != 0) ? 1 : 0;
#elif defined(__linux__) && defined(_GNU_SOURCE) && !defined(__ANDROID__)
    cpu_set_t cpuset;
    CPU_ZERO(&cpuset);
    CPU_SET((int)core_id, &cpuset);
    return (pthread_setaffinity_np(pthread_self(), sizeof(cpu_set_t), &cpuset) == 0) ? 1 : 0;
#else
    (void)core_id;
    return 0; // Not supported on this platform
#endif
}

int64_t seen_get_num_cores(void) {
#ifdef _WIN32
    SYSTEM_INFO si;
    GetSystemInfo(&si);
    return (int64_t)si.dwNumberOfProcessors;
#elif defined(_SC_NPROCESSORS_ONLN)
    return (int64_t)sysconf(_SC_NPROCESSORS_ONLN);
#else
    return 1;
#endif
}

// ============================================================================
// Bounds Checking
// ============================================================================

void seen_bounds_check(int64_t idx, int64_t len) {
    if (idx < 0 || idx >= len) {
        fprintf(stderr, "PANIC: index out of bounds: index=%lld, length=%lld\n",
                (long long)idx, (long long)len);
        abort();
    }
}

// ============================================================================
// Integer Overflow Checking
// ============================================================================

int64_t seen_checked_add(int64_t a, int64_t b) {
    int64_t r;
    if (__builtin_add_overflow(a, b, &r)) {
        fprintf(stderr, "PANIC: integer overflow in add: %lld + %lld\n",
                (long long)a, (long long)b);
        abort();
    }
    return r;
}

int64_t seen_checked_sub(int64_t a, int64_t b) {
    int64_t r;
    if (__builtin_sub_overflow(a, b, &r)) {
        fprintf(stderr, "PANIC: integer overflow in sub: %lld - %lld\n",
                (long long)a, (long long)b);
        abort();
    }
    return r;
}

int64_t seen_checked_mul(int64_t a, int64_t b) {
    int64_t r;
    if (__builtin_mul_overflow(a, b, &r)) {
        fprintf(stderr, "PANIC: integer overflow in mul: %lld * %lld\n",
                (long long)a, (long long)b);
        abort();
    }
    return r;
}

// ============================================================================
// Pointer Arithmetic
// ============================================================================

int64_t seen_ptr_add(int64_t ptr, int64_t offset, int64_t elem_size) {
    return ptr + offset * elem_size;
}

int64_t seen_ptr_deref_i64(int64_t ptr) {
    return *(int64_t*)(uintptr_t)ptr;
}

void seen_ptr_store_i64(int64_t ptr, int64_t val) {
    *(int64_t*)(uintptr_t)ptr = val;
}

double seen_ptr_deref_f64(int64_t ptr) {
    return *(double*)(uintptr_t)ptr;
}

void seen_ptr_store_f64(int64_t ptr, double val) {
    *(double*)(uintptr_t)ptr = val;
}

int64_t seen_ptr_is_null(int64_t ptr) {
    return ptr == 0 ? 1 : 0;
}

// Typed pointer operations for C interop (fixed-width types)
int32_t seen_ptr_deref_i32(int64_t ptr) {
    return *(int32_t*)(uintptr_t)ptr;
}

void seen_ptr_store_i32(int64_t ptr, int32_t val) {
    *(int32_t*)(uintptr_t)ptr = val;
}

int16_t seen_ptr_deref_i16(int64_t ptr) {
    return *(int16_t*)(uintptr_t)ptr;
}

void seen_ptr_store_i16(int64_t ptr, int16_t val) {
    *(int16_t*)(uintptr_t)ptr = val;
}

int8_t seen_ptr_deref_i8(int64_t ptr) {
    return *(int8_t*)(uintptr_t)ptr;
}

void seen_ptr_store_i8(int64_t ptr, int8_t val) {
    *(int8_t*)(uintptr_t)ptr = val;
}

float seen_ptr_deref_f32(int64_t ptr) {
    return *(float*)(uintptr_t)ptr;
}

void seen_ptr_store_f32(int64_t ptr, float val) {
    *(float*)(uintptr_t)ptr = val;
}

// ============================================================================
// Debug Assert
// ============================================================================

void seen_debug_assert_fail(int64_t line) {
    fprintf(stderr, "PANIC: debug_assert failed at line %lld\n", (long long)line);
    abort();
}

// ============================================================================
// Derive Helpers
// ============================================================================

int64_t seen_arr_clone(int64_t src) {
    if (src == 0) return 0;
    SeenArray* srcArr = (SeenArray*)(uintptr_t)src;
    int64_t len = srcArr->len;
    int64_t cap = srcArr->cap;
    if (cap < len) cap = len;
    SeenArray* dst = (SeenArray*)malloc(sizeof(SeenArray));
    dst->len = len;
    dst->cap = cap;
    dst->element_size = srcArr->element_size;
    if (cap > 0 && srcArr->data) {
        dst->data = malloc(cap * srcArr->element_size);
        memcpy(dst->data, srcArr->data, len * srcArr->element_size);
    } else {
        dst->data = NULL;
    }
    return (int64_t)(uintptr_t)dst;
}

int64_t seen_hash_int(int64_t v) {
    // FNV-1a hash for a single i64
    uint64_t hash = 14695981039346656037ULL;
    uint8_t* bytes = (uint8_t*)&v;
    for (int i = 0; i < 8; i++) {
        hash ^= bytes[i];
        hash *= 1099511628211ULL;
    }
    return (int64_t)hash;
}

int64_t seen_hash_string(SeenString s) {
    // djb2 hash over string bytes
    uint64_t hash = 5381;
    if (s.data == NULL) return (int64_t)hash;
    for (int64_t i = 0; i < s.len; i++) {
        hash = ((hash << 5) + hash) + (uint8_t)s.data[i];
    }
    return (int64_t)hash;
}

int64_t seen_hash_float(double v) {
    int64_t bits;
    memcpy(&bits, &v, sizeof(double));
    return seen_hash_int(bits);
}

int64_t seen_hash_combine(int64_t h1, int64_t h2) {
    // XOR-shift combine
    uint64_t a = (uint64_t)h1;
    uint64_t b = (uint64_t)h2;
    a ^= b + 0x9e3779b97f4a7c15ULL + (a << 6) + (a >> 2);
    return (int64_t)a;
}

int64_t seen_str_eq_derive(SeenString a, SeenString b) {
    if (a.len != b.len) return 0;
    if (a.len == 0) return 1;
    if (a.data == NULL && b.data == NULL) return 1;
    if (a.data == NULL || b.data == NULL) return 0;
    return memcmp(a.data, b.data, a.len) == 0 ? 1 : 0;
}

// ============================================================================
// Binary Serialization
// ============================================================================

typedef struct {
    uint8_t* data;
    int64_t length;
    int64_t capacity;
} SeenByteBuffer;

static SeenByteBuffer* seen_bytebuf_new(int64_t cap) {
    SeenByteBuffer* buf = (SeenByteBuffer*)malloc(sizeof(SeenByteBuffer));
    buf->data = (uint8_t*)malloc(cap > 0 ? cap : 64);
    buf->length = 0;
    buf->capacity = cap > 0 ? cap : 64;
    return buf;
}

static void seen_bytebuf_ensure(SeenByteBuffer* buf, int64_t extra) {
    if (buf->length + extra > buf->capacity) {
        int64_t newCap = buf->capacity * 2;
        if (newCap < buf->length + extra) newCap = buf->length + extra;
        buf->data = (uint8_t*)realloc(buf->data, newCap);
        buf->capacity = newCap;
    }
}

int64_t seen_binary_buffer_new(int64_t capacity) {
    return (int64_t)(uintptr_t)seen_bytebuf_new(capacity);
}

int64_t seen_binary_buffer_data(int64_t buf_handle) {
    SeenByteBuffer* buf = (SeenByteBuffer*)(uintptr_t)buf_handle;
    return (int64_t)(uintptr_t)buf->data;
}

int64_t seen_binary_buffer_length(int64_t buf_handle) {
    SeenByteBuffer* buf = (SeenByteBuffer*)(uintptr_t)buf_handle;
    return buf->length;
}

void seen_binary_write_i64(int64_t buf_handle, int64_t val) {
    SeenByteBuffer* buf = (SeenByteBuffer*)(uintptr_t)buf_handle;
    seen_bytebuf_ensure(buf, 8);
    memcpy(buf->data + buf->length, &val, 8);
    buf->length += 8;
}

int64_t seen_binary_read_i64(int64_t buf_handle, int64_t offset) {
    SeenByteBuffer* buf = (SeenByteBuffer*)(uintptr_t)buf_handle;
    int64_t val = 0;
    if (offset + 8 <= buf->length) {
        memcpy(&val, buf->data + offset, 8);
    }
    return val;
}

void seen_binary_write_f64(int64_t buf_handle, double val) {
    int64_t bits;
    memcpy(&bits, &val, 8);
    seen_binary_write_i64(buf_handle, bits);
}

double seen_binary_read_f64(int64_t buf_handle, int64_t offset) {
    int64_t bits = seen_binary_read_i64(buf_handle, offset);
    double val;
    memcpy(&val, &bits, 8);
    return val;
}

void seen_binary_write_i32(int64_t buf_handle, int32_t val) {
    SeenByteBuffer* buf = (SeenByteBuffer*)(uintptr_t)buf_handle;
    seen_bytebuf_ensure(buf, 4);
    memcpy(buf->data + buf->length, &val, 4);
    buf->length += 4;
}

int32_t seen_binary_read_i32(int64_t buf_handle, int64_t offset) {
    SeenByteBuffer* buf = (SeenByteBuffer*)(uintptr_t)buf_handle;
    int32_t val = 0;
    if (offset + 4 <= buf->length) {
        memcpy(&val, buf->data + offset, 4);
    }
    return val;
}

void seen_binary_write_str(int64_t buf_handle, SeenString s) {
    SeenByteBuffer* buf = (SeenByteBuffer*)(uintptr_t)buf_handle;
    // Length-prefix: 4 bytes for string length, then UTF-8 bytes
    int32_t slen = (int32_t)s.len;
    seen_bytebuf_ensure(buf, 4 + s.len);
    memcpy(buf->data + buf->length, &slen, 4);
    buf->length += 4;
    if (s.len > 0 && s.data) {
        memcpy(buf->data + buf->length, s.data, s.len);
        buf->length += s.len;
    }
}

SeenString seen_binary_read_str(int64_t buf_handle, int64_t offset) {
    SeenByteBuffer* buf = (SeenByteBuffer*)(uintptr_t)buf_handle;
    SeenString result = {0, NULL};
    if (offset + 4 > buf->length) return result;
    int32_t slen = 0;
    memcpy(&slen, buf->data + offset, 4);
    if (slen < 0 || offset + 4 + slen > buf->length) return result;
    char* data = (char*)malloc(slen + 1);
    memcpy(data, buf->data + offset + 4, slen);
    data[slen] = '\0';
    result.data = data;
    result.len = slen;
    return result;
}

// Big-endian variants
static uint64_t seen_bswap64(uint64_t v) {
    return ((v & 0xFF00000000000000ULL) >> 56) |
           ((v & 0x00FF000000000000ULL) >> 40) |
           ((v & 0x0000FF0000000000ULL) >> 24) |
           ((v & 0x000000FF00000000ULL) >> 8)  |
           ((v & 0x00000000FF000000ULL) << 8)  |
           ((v & 0x0000000000FF0000ULL) << 24) |
           ((v & 0x000000000000FF00ULL) << 40) |
           ((v & 0x00000000000000FFULL) << 56);
}

void seen_binary_write_i64_be(int64_t buf_handle, int64_t val) {
    int64_t swapped = (int64_t)seen_bswap64((uint64_t)val);
    seen_binary_write_i64(buf_handle, swapped);
}

int64_t seen_binary_read_i64_be(int64_t buf_handle, int64_t offset) {
    int64_t val = seen_binary_read_i64(buf_handle, offset);
    return (int64_t)seen_bswap64((uint64_t)val);
}

void seen_binary_write_f64_be(int64_t buf_handle, double val) {
    int64_t bits;
    memcpy(&bits, &val, 8);
    seen_binary_write_i64_be(buf_handle, bits);
}

double seen_binary_read_f64_be(int64_t buf_handle, int64_t offset) {
    int64_t bits = seen_binary_read_i64_be(buf_handle, offset);
    double val;
    memcpy(&val, &bits, 8);
    return val;
}

// ============================================================================
// Compression - RLE
// ============================================================================

int64_t seen_compress_rle(int64_t src_handle, int64_t srcLen, int64_t dst_handle, int64_t dstCap) {
    uint8_t* src = (uint8_t*)(uintptr_t)src_handle;
    uint8_t* dst = (uint8_t*)(uintptr_t)dst_handle;
    int64_t di = 0;
    int64_t si = 0;
    while (si < srcLen && di + 2 <= dstCap) {
        uint8_t val = src[si];
        int64_t count = 1;
        while (si + count < srcLen && src[si + count] == val && count < 255) {
            count++;
        }
        dst[di++] = (uint8_t)count;
        dst[di++] = val;
        si += count;
    }
    return di;
}

int64_t seen_decompress_rle(int64_t src_handle, int64_t srcLen, int64_t dst_handle, int64_t dstCap) {
    uint8_t* src = (uint8_t*)(uintptr_t)src_handle;
    uint8_t* dst = (uint8_t*)(uintptr_t)dst_handle;
    int64_t di = 0;
    int64_t si = 0;
    while (si + 1 < srcLen) {
        uint8_t count = src[si];
        uint8_t val = src[si + 1];
        for (int j = 0; j < count && di < dstCap; j++) {
            dst[di++] = val;
        }
        si += 2;
    }
    return di;
}

// ============================================================================
// JSON Builder
// ============================================================================

typedef struct {
    char* data;
    int64_t length;
    int64_t capacity;
    int fieldCount;
} SeenJsonBuilder;

int64_t seen_json_start_object(int64_t unused) {
    SeenJsonBuilder* jb = (SeenJsonBuilder*)malloc(sizeof(SeenJsonBuilder));
    jb->capacity = 256;
    jb->data = (char*)malloc(jb->capacity);
    jb->data[0] = '{';
    jb->length = 1;
    jb->fieldCount = 0;
    (void)unused;
    return (int64_t)(uintptr_t)jb;
}

static void jb_ensure(SeenJsonBuilder* jb, int64_t extra) {
    if (jb->length + extra >= jb->capacity) {
        jb->capacity = jb->capacity * 2 + extra;
        jb->data = (char*)realloc(jb->data, jb->capacity);
    }
}

static void jb_append(SeenJsonBuilder* jb, const char* s, int64_t len) {
    jb_ensure(jb, len);
    memcpy(jb->data + jb->length, s, len);
    jb->length += len;
}

static void jb_append_str(SeenJsonBuilder* jb, const char* s) {
    jb_append(jb, s, strlen(s));
}

static void jb_write_key(SeenJsonBuilder* jb, SeenString key) {
    if (jb->fieldCount > 0) {
        jb_append_str(jb, ", ");
    }
    jb_append_str(jb, "\"");
    if (key.data && key.len > 0) {
        jb_append(jb, key.data, key.len);
    }
    jb_append_str(jb, "\": ");
    jb->fieldCount++;
}

static const char* seen_json_skip_ws_ptr(const char* p, const char* end) {
    while (p < end && isspace((unsigned char)*p)) {
        p++;
    }
    return p;
}

static SeenString seen_json_trim_slice(const char* start, const char* end) {
    SeenString empty = {0, ""};
    if (!start || !end || start >= end) {
        return empty;
    }
    while (start < end && isspace((unsigned char)*start)) {
        start++;
    }
    while (end > start && isspace((unsigned char)*(end - 1))) {
        end--;
    }
    SeenString result = {(int64_t)(end - start), (char*)start};
    return result;
}

static const char* seen_json_skip_string_ptr(const char* p, const char* end) {
    if (!p || p >= end || *p != '"') {
        return p;
    }
    p++;
    while (p < end) {
        if (*p == '\\') {
            p++;
            if (p < end) {
                p++;
            }
            continue;
        }
        if (*p == '"') {
            return p + 1;
        }
        p++;
    }
    return end;
}

static const char* seen_json_skip_container_ptr(const char* p, const char* end, char open_ch, char close_ch) {
    if (!p || p >= end || *p != open_ch) {
        return p;
    }
    int depth = 0;
    while (p < end) {
        if (*p == '"') {
            p = seen_json_skip_string_ptr(p, end);
            continue;
        }
        if (*p == open_ch) {
            depth++;
            p++;
            continue;
        }
        if (*p == close_ch) {
            depth--;
            p++;
            if (depth == 0) {
                return p;
            }
            continue;
        }
        p++;
    }
    return end;
}

static const char* seen_json_skip_value_ptr(const char* p, const char* end) {
    p = seen_json_skip_ws_ptr(p, end);
    if (!p || p >= end) {
        return end;
    }
    if (*p == '"') {
        return seen_json_skip_string_ptr(p, end);
    }
    if (*p == '{') {
        return seen_json_skip_container_ptr(p, end, '{', '}');
    }
    if (*p == '[') {
        return seen_json_skip_container_ptr(p, end, '[', ']');
    }
    while (p < end &&
           *p != ',' &&
           *p != '}' &&
           *p != ']' &&
           !isspace((unsigned char)*p)) {
        p++;
    }
    return p;
}

SeenString seen_json_object_get(SeenString json, SeenString key) {
    SeenString empty = {0, ""};
    if (!json.data || json.len <= 0 || !key.data || key.len <= 0) {
        return empty;
    }

    const char* start = json.data;
    const char* end = json.data + json.len;
    const char* p = seen_json_skip_ws_ptr(start, end);
    if (p >= end || *p != '{') {
        return empty;
    }
    p++;

    while (p < end) {
        p = seen_json_skip_ws_ptr(p, end);
        if (p >= end || *p == '}') {
            return empty;
        }
        if (*p != '"') {
            return empty;
        }

        const char* key_start = p + 1;
        const char* scan = key_start;
        int has_escape = 0;
        while (scan < end) {
            if (*scan == '\\') {
                has_escape = 1;
                scan++;
                if (scan < end) {
                    scan++;
                }
                continue;
            }
            if (*scan == '"') {
                break;
            }
            scan++;
        }
        if (scan >= end) {
            return empty;
        }

        const char* key_end = scan;
        p = scan + 1;
        p = seen_json_skip_ws_ptr(p, end);
        if (p >= end || *p != ':') {
            return empty;
        }
        p++;

        const char* value_start = seen_json_skip_ws_ptr(p, end);
        const char* value_end = seen_json_skip_value_ptr(value_start, end);
        if (!has_escape &&
            (key_end - key_start) == key.len &&
            memcmp(key_start, key.data, (size_t)key.len) == 0) {
            return seen_json_trim_slice(value_start, value_end);
        }

        p = seen_json_skip_ws_ptr(value_end, end);
        if (p < end && *p == ',') {
            p++;
            continue;
        }
        if (p < end && *p == '}') {
            return empty;
        }
    }

    return empty;
}

int64_t seen_json_decode_int(SeenString raw) {
    if (!raw.data || raw.len <= 0) {
        return 0;
    }
    SeenString trimmed = seen_json_trim_slice(raw.data, raw.data + raw.len);
    if (!trimmed.data || trimmed.len <= 0) {
        return 0;
    }
    char* cstr = seen_str_to_cstr(trimmed);
    if (!cstr) {
        return 0;
    }
    int64_t result = strtoll(cstr, NULL, 10);
    free(cstr);
    return result;
}

double seen_json_decode_float(SeenString raw) {
    if (!raw.data || raw.len <= 0) {
        return 0.0;
    }
    SeenString trimmed = seen_json_trim_slice(raw.data, raw.data + raw.len);
    if (!trimmed.data || trimmed.len <= 0) {
        return 0.0;
    }
    char* cstr = seen_str_to_cstr(trimmed);
    if (!cstr) {
        return 0.0;
    }
    double result = strtod(cstr, NULL);
    free(cstr);
    return result;
}

SeenString seen_json_decode_string(SeenString raw) {
    SeenString empty = {0, ""};
    if (!raw.data || raw.len < 2) {
        return empty;
    }
    SeenString trimmed = seen_json_trim_slice(raw.data, raw.data + raw.len);
    if (trimmed.len < 2 || trimmed.data[0] != '"' || trimmed.data[trimmed.len - 1] != '"') {
        return empty;
    }

    char* out = (char*)malloc((size_t)trimmed.len);
    if (!out) {
        return empty;
    }

    int64_t out_len = 0;
    for (int64_t i = 1; i < trimmed.len - 1; i++) {
        char ch = trimmed.data[i];
        if (ch == '\\' && i + 1 < trimmed.len - 1) {
            i++;
            char esc = trimmed.data[i];
            switch (esc) {
                case '"': out[out_len++] = '"'; break;
                case '\\': out[out_len++] = '\\'; break;
                case '/': out[out_len++] = '/'; break;
                case 'b': out[out_len++] = '\b'; break;
                case 'f': out[out_len++] = '\f'; break;
                case 'n': out[out_len++] = '\n'; break;
                case 'r': out[out_len++] = '\r'; break;
                case 't': out[out_len++] = '\t'; break;
                case 'u':
                    if (i + 4 < trimmed.len - 1) {
                        i += 4;
                    }
                    out[out_len++] = '?';
                    break;
                default:
                    out[out_len++] = esc;
                    break;
            }
        } else {
            out[out_len++] = ch;
        }
    }

    out[out_len] = '\0';
    SeenString result = {out_len, out};
    return result;
}

int64_t seen_json_decode_bool(SeenString raw) {
    if (!raw.data || raw.len <= 0) {
        return 0;
    }
    SeenString trimmed = seen_json_trim_slice(raw.data, raw.data + raw.len);
    if (trimmed.len == 4 && memcmp(trimmed.data, "true", 4) == 0) {
        return 1;
    }
    return 0;
}

void seen_json_key_int(int64_t jb_handle, SeenString key, int64_t val) {
    SeenJsonBuilder* jb = (SeenJsonBuilder*)(uintptr_t)jb_handle;
    jb_write_key(jb, key);
    char buf[32];
    int len = snprintf(buf, sizeof(buf), "%lld", (long long)val);
    jb_append(jb, buf, len);
}

void seen_json_key_str(int64_t jb_handle, SeenString key, SeenString val) {
    SeenJsonBuilder* jb = (SeenJsonBuilder*)(uintptr_t)jb_handle;
    jb_write_key(jb, key);
    jb_append_str(jb, "\"");
    if (val.data && val.len > 0) {
        jb_append(jb, val.data, val.len);
    }
    jb_append_str(jb, "\"");
}

void seen_json_key_float(int64_t jb_handle, SeenString key, double val) {
    SeenJsonBuilder* jb = (SeenJsonBuilder*)(uintptr_t)jb_handle;
    jb_write_key(jb, key);
    char buf[64];
    int len = snprintf(buf, sizeof(buf), "%.17g", val);
    jb_append(jb, buf, len);
}

void seen_json_key_bool(int64_t jb_handle, SeenString key, int64_t val) {
    SeenJsonBuilder* jb = (SeenJsonBuilder*)(uintptr_t)jb_handle;
    jb_write_key(jb, key);
    if (val) {
        jb_append_str(jb, "true");
    } else {
        jb_append_str(jb, "false");
    }
}

void seen_json_key_null(int64_t jb_handle, SeenString key) {
    SeenJsonBuilder* jb = (SeenJsonBuilder*)(uintptr_t)jb_handle;
    jb_write_key(jb, key);
    jb_append_str(jb, "null");
}

SeenString seen_json_end_object(int64_t jb_handle) {
    SeenJsonBuilder* jb = (SeenJsonBuilder*)(uintptr_t)jb_handle;
    jb_append_str(jb, "}");
    SeenString result;
    result.len = jb->length;
    result.data = jb->data;
    // Don't free jb->data — caller owns the string
    free(jb);
    return result;
}

// ============================================================================
// Packet Runtime
// ============================================================================

void seen_packet_write_header(int64_t buf_handle, int32_t id, int32_t len) {
    SeenByteBuffer* buf = (SeenByteBuffer*)(uintptr_t)buf_handle;
    seen_bytebuf_ensure(buf, 8);
    memcpy(buf->data + buf->length, &id, 4);
    buf->length += 4;
    memcpy(buf->data + buf->length, &len, 4);
    buf->length += 4;
}

int64_t seen_packet_read_header(int64_t buf_handle, int64_t offset) {
    SeenByteBuffer* buf = (SeenByteBuffer*)(uintptr_t)buf_handle;
    if (offset + 8 > buf->length) return 0;
    int32_t id = 0, len = 0;
    memcpy(&id, buf->data + offset, 4);
    memcpy(&len, buf->data + offset + 4, 4);
    // Pack id in upper 32 bits, len in lower 32
    return ((int64_t)id << 32) | ((int64_t)(uint32_t)len);
}

// ============================================================================
// Safety Runtime
// ============================================================================

void seen_null_assign_warning(SeenString name, int64_t line) {
    if (name.data && name.len > 0) {
        fprintf(stderr, "[null-safety] Warning: assigning null to non-optional '%.*s' at line %lld\n",
                (int)name.len, name.data, (long long)line);
    } else {
        fprintf(stderr, "[null-safety] Warning: assigning null to non-optional variable at line %lld\n",
                (long long)line);
    }
}

void seen_use_after_move(SeenString name) {
    if (name.data && name.len > 0) {
        fprintf(stderr, "PANIC: use after move of '%.*s'\n", (int)name.len, name.data);
    } else {
        fprintf(stderr, "PANIC: use after move\n");
    }
    abort();
}

static int64_t seen_stack_base = 0;
static int64_t seen_stack_limit = 8 * 1024 * 1024; // 8MB default

void seen_stack_init(int64_t base) {
    seen_stack_base = base;
}

int64_t seen_stack_check(int64_t sp) {
    if (seen_stack_base == 0) return 1; // not initialized, OK
    int64_t used = seen_stack_base - sp;
    if (used < 0) used = -used;
    return (used < seen_stack_limit) ? 1 : 0;
}

void seen_stack_overflow_panic(void) {
    fprintf(stderr, "PANIC: stack overflow detected\n");
    abort();
}

// ============================================================================
// Main wrapper
// ============================================================================

// ============================================================================
// SmallVec — inline storage with heap fallback
// ============================================================================

int64_t seen_small_vec_new(int64_t inline_capacity) {
    SeenSmallVec *sv = (SeenSmallVec *)malloc(sizeof(SeenSmallVec));
    if (!sv) return 0;
    sv->data = NULL;
    memset(sv->inline_buf, 0, sizeof(sv->inline_buf));
    sv->length = 0;
    sv->inline_cap = (inline_capacity > 8) ? 8 : inline_capacity;
    sv->capacity = sv->inline_cap;
    return (int64_t)sv;
}

static void sv_ensure_capacity(SeenSmallVec *sv, int64_t needed) {
    if (needed <= sv->capacity) return;
    int64_t new_cap = sv->capacity * 2;
    if (new_cap < needed) new_cap = needed;
    if (sv->data == NULL) {
        // Transition from inline to heap
        sv->data = (int64_t *)malloc(new_cap * sizeof(int64_t));
        if (!sv->data) return;
        // Copy inline data to heap
        for (int64_t i = 0; i < sv->length; i++) {
            sv->data[i] = sv->inline_buf[i];
        }
    } else {
        sv->data = (int64_t *)realloc(sv->data, new_cap * sizeof(int64_t));
        if (!sv->data) return;
    }
    sv->capacity = new_cap;
}

static int64_t *sv_get_ptr(SeenSmallVec *sv) {
    return sv->data ? sv->data : sv->inline_buf;
}

void seen_small_vec_push_i64(int64_t handle, int64_t value) {
    SeenSmallVec *sv = (SeenSmallVec *)handle;
    if (!sv) return;
    sv_ensure_capacity(sv, sv->length + 1);
    int64_t *buf = sv_get_ptr(sv);
    buf[sv->length] = value;
    sv->length++;
}

void seen_small_vec_push_f64(int64_t handle, int64_t value) {
    seen_small_vec_push_i64(handle, value); // f64 stored as i64 bits
}

void seen_small_vec_push_str(int64_t handle, int64_t value) {
    seen_small_vec_push_i64(handle, value); // string handle is i64
}

int64_t seen_small_vec_get_i64(int64_t handle, int64_t index) {
    SeenSmallVec *sv = (SeenSmallVec *)handle;
    if (!sv || index < 0 || index >= sv->length) return 0;
    int64_t *buf = sv_get_ptr(sv);
    return buf[index];
}

int64_t seen_small_vec_get_f64(int64_t handle, int64_t index) {
    return seen_small_vec_get_i64(handle, index);
}

int64_t seen_small_vec_get_str(int64_t handle, int64_t index) {
    return seen_small_vec_get_i64(handle, index);
}

int64_t seen_small_vec_length(int64_t handle) {
    SeenSmallVec *sv = (SeenSmallVec *)handle;
    if (!sv) return 0;
    return sv->length;
}

void seen_small_vec_clear(int64_t handle) {
    SeenSmallVec *sv = (SeenSmallVec *)handle;
    if (!sv) return;
    sv->length = 0;
    // If on heap, free and go back to inline
    if (sv->data) {
        free(sv->data);
        sv->data = NULL;
        sv->capacity = sv->inline_cap;
    }
}

// ============================================================================
// Stub functions for game engine features not yet fully implemented in runtime.
// These prevent link failures when the compiler generates references to them.
// ============================================================================
#include <string.h>

int64_t seen_packed_chunk_create(int64_t size) { return (int64_t)calloc(1, (size_t)size); }
void seen_packed_chunk_free(int64_t ptr) { free((void*)ptr); }
int64_t seen_packed_chunk_get(int64_t ptr, int64_t idx) { return ((int64_t*)ptr)[idx]; }
void seen_packed_chunk_set(int64_t ptr, int64_t idx, int64_t val) { ((int64_t*)ptr)[idx] = val; }
int64_t seen_rle_compress(int64_t data, int64_t len) { (void)data; (void)len; return 0; }
int64_t seen_rle_decompress(int64_t data, int64_t len) { (void)data; (void)len; return 0; }
void seen_rle_free(int64_t ptr) { free((void*)ptr); }
int64_t seen_chunk_brick_from_packed(int64_t p, int64_t s) { (void)p; (void)s; return 0; }
int64_t seen_chunk_brick_from_rle(int64_t p, int64_t s) { (void)p; (void)s; return 0; }
int64_t seen_chunk_brick_load(int64_t p) { (void)p; return 0; }
void seen_chunk_brick_free(int64_t p) { (void)p; }
void seen_quality_log(int64_t level, int64_t msg_ptr, int64_t msg_len) { (void)level; (void)msg_ptr; (void)msg_len; }
int64_t seen_vk_get_physical_device_timestamp_period(void) { return 0; }
__attribute__((weak)) int64_t seen_vk_create_query_pool(int64_t a, int64_t b) { (void)a; (void)b; return 0; }
__attribute__((weak)) void seen_vk_cmd_write_timestamp(int64_t a, int64_t b, int64_t c) { (void)a; (void)b; (void)c; }

// Hearton engine string helpers (SeenString already typedef'd above)
SeenString hearton_int_to_str(int64_t val) {
    char buf[32]; int n = snprintf(buf, sizeof(buf), "%ld", (long)val);
    char* s = malloc(n + 1); memcpy(s, buf, n + 1);
    return (SeenString){n, s};
}
SeenString hearton_float_to_str(double val) {
    char buf[64]; int n = snprintf(buf, sizeof(buf), "%.6f", val);
    char* s = malloc(n + 1); memcpy(s, buf, n + 1);
    return (SeenString){n, s};
}
int64_t seen_string_contains(int64_t s_len, char* s_data, int64_t sub_len, char* sub_data) {
    if (sub_len == 0) return 1;
    if (s_len < sub_len) return 0;
    for (int64_t i = 0; i <= s_len - sub_len; i++) {
        if (memcmp(s_data + i, sub_data, sub_len) == 0) return 1;
    }
    return 0;
}
int64_t seen_string_token_count(int64_t s_len, char* s_data, int64_t d_len, char* d_data) {
    (void)d_len; if (s_len == 0) return 0;
    int64_t count = 1;
    for (int64_t i = 0; i < s_len; i++) {
        if (s_data[i] == d_data[0]) count++;
    }
    return count;
}
SeenString seen_string_token(int64_t s_len, char* s_data, int64_t d_len, char* d_data, int64_t idx) {
    (void)d_len; int64_t start = 0, cur = 0;
    for (int64_t i = 0; i < s_len && cur < idx; i++) {
        if (s_data[i] == d_data[0]) { start = i + 1; cur++; }
    }
    int64_t end = s_len;
    for (int64_t i = start; i < s_len; i++) {
        if (s_data[i] == d_data[0]) { end = i; break; }
    }
    int64_t len = end - start;
    char* r = malloc(len + 1); memcpy(r, s_data + start, len); r[len] = 0;
    return (SeenString){len, r};
}

// ==========================================================================
// Game engine runtime stubs — referenced by compiler's game engine modules
// These are placeholder implementations for bootstrap builds where the full
// game engine libraries (SDL, Vulkan, voxel, property) are not available.
// ==========================================================================

int64_t seen_chunk_brick_alloc(int64_t a) { (void)a; return 0; }
void seen_chunk_brick_set(int64_t a, int64_t b, int64_t c) { (void)a; (void)b; (void)c; }
void seen_packed_chunk_fill(int64_t a, int64_t b) { (void)a; (void)b; }
void seen_prop_log(int64_t a, int64_t b) { (void)a; (void)b; }
void seen_prop_warn(int64_t a, int64_t b) { (void)a; (void)b; }
void seen_console_print(int64_t a, int64_t b) { (void)a; (void)b; }
double seen_string_to_float(int64_t len, char* data) { (void)len; (void)data; return 0.0; }
int64_t seen_string_to_int(int64_t len, char* data) { (void)len; (void)data; return 0; }
double seen_math_positive_infinity(void) { return INFINITY; }
double seen_math_negative_infinity(void) { return -INFINITY; }
double seen_math_nan(void) { return NAN; }
int64_t seen_string_starts_with(int64_t s_len, char* s_data, int64_t p_len, char* p_data) {
    (void)s_len; (void)s_data; (void)p_len; (void)p_data; return 0;
}

// SDL stubs
__attribute__((weak)) int64_t seen_sdl_init(int64_t a) { (void)a; return 0; }
int64_t seen_sdl_create_window_vulkan(int64_t a, int64_t b, int64_t c, int64_t d) { (void)a; (void)b; (void)c; (void)d; return 0; }
int64_t seen_sdl_alloc_event_buffer(void) { return 0; }
__attribute__((weak)) void seen_sdl_set_relative_mouse(int64_t a) { (void)a; }

// Vulkan stubs
__attribute__((weak)) int64_t seen_vk_create_instance(int64_t a) { (void)a; return 0; }
int64_t seen_vk_create_surface(int64_t a, int64_t b) { (void)a; (void)b; return 0; }
int64_t seen_vk_pick_physical_device(int64_t a, int64_t b) { (void)a; (void)b; return 0; }
int64_t seen_vk_create_logical_device(int64_t a) { (void)a; return 0; }
int64_t seen_vk_get_graphics_queue(int64_t a) { (void)a; return 0; }
int64_t seen_vk_get_present_queue(int64_t a) { (void)a; return 0; }
__attribute__((weak)) int64_t seen_vk_create_swapchain(int64_t a, int64_t b, int64_t c) { (void)a; (void)b; (void)c; return 0; }
int64_t seen_vk_get_swapchain_image_count(int64_t a) { (void)a; return 0; }
int64_t seen_vk_create_swapchain_image_views(int64_t a, int64_t b) { (void)a; (void)b; return 0; }
int64_t seen_vk_create_render_pass(int64_t a) { (void)a; return 0; }
int64_t seen_vk_create_framebuffers(int64_t a, int64_t b, int64_t c) { (void)a; (void)b; (void)c; return 0; }
__attribute__((weak)) int64_t seen_vk_create_command_pool(int64_t a) { (void)a; return 0; }
__attribute__((weak)) int64_t seen_vk_allocate_command_buffers(int64_t a, int64_t b) { (void)a; (void)b; return 0; }
__attribute__((weak)) int64_t seen_vk_create_semaphore(int64_t a) { (void)a; return 0; }
__attribute__((weak)) int64_t seen_vk_create_fence(int64_t a) { (void)a; return 0; }
int64_t seen_vk_create_depth_resources(int64_t a, int64_t b, int64_t c) { (void)a; (void)b; (void)c; return 0; }
int64_t seen_vk_create_depth_image(int64_t a, int64_t b, int64_t c) { (void)a; (void)b; (void)c; return 0; }
int64_t seen_vk_create_depth_image_view(int64_t a) { (void)a; return 0; }
int64_t seen_vk_create_buffer(int64_t a, int64_t b, int64_t c) { (void)a; (void)b; (void)c; return 0; }
int64_t seen_vk_allocate_memory(int64_t a, int64_t b) { (void)a; (void)b; return 0; }
int64_t seen_vk_create_sampler_shadow(int64_t a) { (void)a; return 0; }
int64_t seen_vk_create_texture_atlas_from_png(int64_t a, int64_t b, int64_t c) { (void)a; (void)b; (void)c; return 0; }
int64_t seen_vk_texture_atlas_get_sampler(int64_t a) { (void)a; return 0; }
int64_t seen_vk_texture_atlas_get_view(int64_t a) { (void)a; return 0; }

// HeartOn engine stubs
int64_t hearton_split_comma(int64_t a, int64_t b) { (void)a; (void)b; return 0; }
int64_t hearton_read_file_line(int64_t a, int64_t b) { (void)a; (void)b; return 0; }

// String utility stubs
typedef struct { int64_t len; char* data; } SeenStringStub;
SeenStringStub seen_string_substring(int64_t s_len, char* s_data, int64_t start, int64_t end) {
    (void)s_len; (void)s_data; (void)start; (void)end;
    SeenStringStub r = {0, ""};
    return r;
}

// Additional stubs (batch 2)
void seen_debug_log(int64_t a, int64_t b) { (void)a; (void)b; }
void seen_perf_log_detailed(int64_t a, int64_t b, int64_t c) { (void)a; (void)b; (void)c; }
void seen_perf_set_title_ext(int64_t a, int64_t b) { (void)a; (void)b; }
void seen_perf_set_title_msg(int64_t a, int64_t b) { (void)a; (void)b; }
void seen_console_poll_line(int64_t a, int64_t b) { (void)a; (void)b; }
void seen_audio_write_samples(int64_t a, int64_t b, int64_t c) { (void)a; (void)b; (void)c; }
void seen_audio_destroy(int64_t a) { (void)a; }
int64_t seen_sdl_get_ticks_ns(void) { return 0; }
__attribute__((weak)) void seen_sdl_show_window(int64_t a) { (void)a; }
int64_t seen_vk_create_depth_only_render_pass(int64_t a) { (void)a; return 0; }
int64_t seen_vk_create_offscreen_render_pass(int64_t a) { (void)a; return 0; }
int64_t seen_vk_create_depth_resources_sampled(int64_t a, int64_t b, int64_t c) { (void)a; (void)b; (void)c; return 0; }
int64_t seen_vk_create_depth_framebuffer(int64_t a, int64_t b, int64_t c) { (void)a; (void)b; (void)c; return 0; }
int64_t seen_vk_depth_pack_get_view(int64_t a) { (void)a; return 0; }
__attribute__((weak)) int64_t seen_vk_create_descriptor_pool(int64_t a, int64_t b) { (void)a; (void)b; return 0; }
__attribute__((weak)) int64_t seen_vk_create_descriptor_set_layout(int64_t a, int64_t b) { (void)a; (void)b; return 0; }
int64_t seen_vk_create_pipeline_layout_ext(int64_t a, int64_t b) { (void)a; (void)b; return 0; }
__attribute__((weak)) int64_t seen_vk_allocate_descriptor_set(int64_t a, int64_t b) { (void)a; (void)b; return 0; }
__attribute__((weak)) void seen_vk_update_descriptor_set_buffer(int64_t a, int64_t b, int64_t c, int64_t d) { (void)a; (void)b; (void)c; (void)d; }
__attribute__((weak)) void seen_vk_update_descriptor_set_image(int64_t a, int64_t b, int64_t c, int64_t d) { (void)a; (void)b; (void)c; (void)d; }

// ==========================================================================
// Comprehensive stubs for bootstrap builds — Vulkan, SDL, voxel, audio, etc.
// These prevent link failures when the frozen compiler generates references
// to game engine symbols that aren't needed for the compiler itself.
// ==========================================================================

// --- Raw Vulkan API stubs (normally provided by -lvulkan) ---
// Keep these opt-in for bootstrap-only builds. Exporting them from the default
// runtime object can shadow the real Vulkan loader in user projects.
#ifdef SEEN_ENABLE_BOOTSTRAP_RAW_VULKAN_STUBS
__attribute__((weak)) int64_t vkBindBufferMemory(int64_t a, int64_t b, int64_t c, int64_t d) { (void)a; (void)b; (void)c; (void)d; return 0; }
__attribute__((weak)) void vkCmdEndRenderPass(int64_t a) { (void)a; }
__attribute__((weak)) void vkDestroyBuffer(int64_t a, int64_t b, int64_t c) { (void)a; (void)b; (void)c; }
__attribute__((weak)) void vkDestroyCommandPool(int64_t a, int64_t b, int64_t c) { (void)a; (void)b; (void)c; }
__attribute__((weak)) void vkDestroyDescriptorPool(int64_t a, int64_t b, int64_t c) { (void)a; (void)b; (void)c; }
__attribute__((weak)) void vkDestroyDescriptorSetLayout(int64_t a, int64_t b, int64_t c) { (void)a; (void)b; (void)c; }
__attribute__((weak)) void vkDestroyDevice(int64_t a, int64_t b) { (void)a; (void)b; }
__attribute__((weak)) void vkDestroyFence(int64_t a, int64_t b, int64_t c) { (void)a; (void)b; (void)c; }
__attribute__((weak)) void vkDestroyFramebuffer(int64_t a, int64_t b, int64_t c) { (void)a; (void)b; (void)c; }
__attribute__((weak)) void vkDestroyImage(int64_t a, int64_t b, int64_t c) { (void)a; (void)b; (void)c; }
__attribute__((weak)) void vkDestroyImageView(int64_t a, int64_t b, int64_t c) { (void)a; (void)b; (void)c; }
__attribute__((weak)) void vkDestroyInstance(int64_t a, int64_t b) { (void)a; (void)b; }
__attribute__((weak)) void vkDestroyPipeline(int64_t a, int64_t b, int64_t c) { (void)a; (void)b; (void)c; }
__attribute__((weak)) void vkDestroyPipelineLayout(int64_t a, int64_t b, int64_t c) { (void)a; (void)b; (void)c; }
__attribute__((weak)) void vkDestroyRenderPass(int64_t a, int64_t b, int64_t c) { (void)a; (void)b; (void)c; }
__attribute__((weak)) void vkDestroySampler(int64_t a, int64_t b, int64_t c) { (void)a; (void)b; (void)c; }
__attribute__((weak)) void vkDestroySemaphore(int64_t a, int64_t b, int64_t c) { (void)a; (void)b; (void)c; }
__attribute__((weak)) void vkDestroyShaderModule(int64_t a, int64_t b, int64_t c) { (void)a; (void)b; (void)c; }
__attribute__((weak)) void vkDestroySurfaceKHR(int64_t a, int64_t b, int64_t c) { (void)a; (void)b; (void)c; }
__attribute__((weak)) void vkDestroySwapchainKHR(int64_t a, int64_t b, int64_t c) { (void)a; (void)b; (void)c; }
__attribute__((weak)) int64_t vkDeviceWaitIdle(int64_t a) { (void)a; return 0; }
__attribute__((weak)) int64_t vkEndCommandBuffer(int64_t a) { (void)a; return 0; }
__attribute__((weak)) void vkFreeMemory(int64_t a, int64_t b, int64_t c) { (void)a; (void)b; (void)c; }
__attribute__((weak)) void vkUnmapMemory(int64_t a, int64_t b) { (void)a; (void)b; }
#endif

// --- Vulkan wrapper stubs ---
__attribute__((weak)) int64_t seen_vk_acquire_next_image(int64_t a, int64_t b, int64_t c) { (void)a; (void)b; (void)c; return 0; }
__attribute__((weak)) void seen_vk_begin_command_buffer(int64_t a) { (void)a; }
void seen_vk_begin_render_pass(int64_t a, int64_t b, int64_t c, int64_t d, int64_t e) { (void)a; (void)b; (void)c; (void)d; (void)e; }
void seen_vk_begin_render_pass_depth(int64_t a, int64_t b, int64_t c, int64_t d) { (void)a; (void)b; (void)c; (void)d; }
void seen_vk_cmd_bind_compute_descriptor_set(int64_t a, int64_t b, int64_t c) { (void)a; (void)b; (void)c; }
void seen_vk_cmd_bind_descriptor_set(int64_t a, int64_t b, int64_t c) { (void)a; (void)b; (void)c; }
void seen_vk_cmd_bind_index_buffer(int64_t a, int64_t b) { (void)a; (void)b; }
__attribute__((weak)) void seen_vk_cmd_bind_pipeline(int64_t a, int64_t b) { (void)a; (void)b; }
void seen_vk_cmd_bind_vertex_buffer(int64_t a, int64_t b) { (void)a; (void)b; }
void seen_vk_cmd_depth_image_barrier(int64_t a, int64_t b) { (void)a; (void)b; }
void seen_vk_cmd_dispatch_compute(int64_t a, int64_t b, int64_t c, int64_t d) { (void)a; (void)b; (void)c; (void)d; }
__attribute__((weak)) void seen_vk_cmd_draw(int64_t a, int64_t b, int64_t c) { (void)a; (void)b; (void)c; }
void seen_vk_cmd_draw_indexed(int64_t a, int64_t b, int64_t c) { (void)a; (void)b; (void)c; }
void seen_vk_cmd_draw_indexed_indirect(int64_t a, int64_t b, int64_t c, int64_t d) { (void)a; (void)b; (void)c; (void)d; }
void seen_vk_cmd_image_barrier(int64_t a, int64_t b, int64_t c, int64_t d) { (void)a; (void)b; (void)c; (void)d; }
void seen_vk_cmd_pipeline_barrier(int64_t a, int64_t b) { (void)a; (void)b; }
void seen_vk_cmd_push_constants_frag(int64_t a, int64_t b, int64_t c, int64_t d) { (void)a; (void)b; (void)c; (void)d; }
__attribute__((weak)) void seen_vk_cmd_reset_query_pool(int64_t a, int64_t b, int64_t c) { (void)a; (void)b; (void)c; }
__attribute__((weak)) void seen_vk_cmd_set_scissor(int64_t a, int64_t b, int64_t c, int64_t d, int64_t e) { (void)a; (void)b; (void)c; (void)d; (void)e; }
__attribute__((weak)) void seen_vk_cmd_set_viewport(int64_t a, int64_t b, int64_t c, int64_t d, int64_t e) { (void)a; (void)b; (void)c; (void)d; (void)e; }
void seen_vk_cmd_storage_buffer_to_indirect_barrier(int64_t a, int64_t b) { (void)a; (void)b; }
int64_t seen_vk_create_blend_pipeline(int64_t a, int64_t b, int64_t c, int64_t d) { (void)a; (void)b; (void)c; (void)d; return 0; }
int64_t seen_vk_create_compute_pipeline_with_layout(int64_t a, int64_t b, int64_t c) { (void)a; (void)b; (void)c; return 0; }
int64_t seen_vk_create_fullscreen_pipeline(int64_t a, int64_t b, int64_t c, int64_t d) { (void)a; (void)b; (void)c; (void)d; return 0; }
int64_t seen_vk_create_graphics_pipeline(int64_t a, int64_t b, int64_t c, int64_t d, int64_t e) { (void)a; (void)b; (void)c; (void)d; (void)e; return 0; }
int64_t seen_vk_create_offscreen_framebuffer(int64_t a, int64_t b, int64_t c, int64_t d) { (void)a; (void)b; (void)c; (void)d; return 0; }
int64_t seen_vk_create_offscreen_target(int64_t a, int64_t b, int64_t c) { (void)a; (void)b; (void)c; return 0; }
int64_t seen_vk_create_offscreen_target_nearest(int64_t a, int64_t b, int64_t c, int64_t d, int64_t e) { (void)a; (void)b; (void)c; (void)d; (void)e; return 0; }
int64_t seen_vk_create_overlay_color_pipeline(int64_t a, int64_t b, int64_t c, int64_t d) { (void)a; (void)b; (void)c; (void)d; return 0; }
int64_t seen_vk_create_overlay_pipeline(int64_t a, int64_t b, int64_t c, int64_t d) { (void)a; (void)b; (void)c; (void)d; return 0; }
int64_t seen_vk_create_pipeline_layout_empty(int64_t a) { (void)a; return 0; }
int64_t seen_vk_create_pipeline_layout_frag_push(int64_t a, int64_t b, int64_t c) { (void)a; (void)b; (void)c; return 0; }
int64_t seen_vk_create_storage_buffer(int64_t a, int64_t b) { (void)a; (void)b; return 0; }
int64_t seen_vk_depth_pack_get_image(int64_t a) { (void)a; return 0; }
int64_t seen_vk_depth_pack_get_sampler(int64_t a) { (void)a; return 0; }
void seen_vk_destroy_depth_pack(int64_t a) { (void)a; }
void seen_vk_destroy_depth_resources(int64_t a) { (void)a; }
void seen_vk_destroy_framebuffers(int64_t a, int64_t b) { (void)a; (void)b; }
void seen_vk_destroy_image_views(int64_t a, int64_t b) { (void)a; (void)b; }
void seen_vk_destroy_offscreen_target(int64_t a) { (void)a; }
__attribute__((weak)) void seen_vk_destroy_query_pool(int64_t a) { (void)a; }
void seen_vk_destroy_texture_atlas(int64_t a) { (void)a; }
int64_t seen_vk_get_command_buffer(int64_t a, int64_t b) { (void)a; (void)b; return 0; }
int64_t seen_vk_get_framebuffer(int64_t a, int64_t b) { (void)a; (void)b; return 0; }
int64_t seen_vk_get_last_ssbo_memory(void) { return 0; }
int64_t seen_vk_get_query_pool_results(int64_t a, int64_t b, int64_t c) { (void)a; (void)b; (void)c; return 0; }
int64_t seen_vk_get_query_result(int64_t a, int64_t b) { (void)a; (void)b; return 0; }
int64_t seen_vk_load_shader_module(int64_t a, int64_t b, int64_t c) { (void)a; (void)b; (void)c; return 0; }
int64_t seen_vk_map_memory(int64_t a, int64_t b) { (void)a; (void)b; return 0; }
int64_t seen_vk_offscreen_get_image(int64_t a) { (void)a; return 0; }
int64_t seen_vk_offscreen_get_sampler(int64_t a) { (void)a; return 0; }
int64_t seen_vk_offscreen_get_view(int64_t a) { (void)a; return 0; }
__attribute__((weak)) int64_t seen_vk_queue_present(int64_t a, int64_t b, int64_t c, int64_t d) { (void)a; (void)b; (void)c; (void)d; return 0; }
__attribute__((weak)) int64_t seen_vk_queue_submit(int64_t a, int64_t b, int64_t c, int64_t d) { (void)a; (void)b; (void)c; (void)d; return 0; }
int64_t seen_vk_recreate_swapchain(int64_t a, int64_t b, int64_t c) { (void)a; (void)b; (void)c; return 0; }
void seen_vk_reset_fence(int64_t a, int64_t b) { (void)a; (void)b; }
void seen_vk_wait_for_fence(int64_t a, int64_t b) { (void)a; (void)b; }

// --- SDL stubs ---
__attribute__((weak)) void SDL_Quit(void) {}
__attribute__((weak)) void seen_sdl_destroy_window(int64_t a) { (void)a; }
int64_t seen_sdl_event_key_scancode(int64_t a) { (void)a; return 0; }
int64_t seen_sdl_event_mouse_button(int64_t a) { (void)a; return 0; }
int64_t seen_sdl_event_mouse_dx(int64_t a) { (void)a; return 0; }
int64_t seen_sdl_event_mouse_dy(int64_t a) { (void)a; return 0; }
int64_t seen_sdl_event_type(int64_t a) { (void)a; return 0; }
int64_t seen_sdl_event_window_h(int64_t a) { (void)a; return 0; }
int64_t seen_sdl_event_window_w(int64_t a) { (void)a; return 0; }
void seen_sdl_free_event_buffer(int64_t a) { (void)a; }
__attribute__((weak)) int64_t seen_sdl_poll_event(int64_t a) { (void)a; return 0; }

// --- Audio stubs ---
int64_t seen_audio_init(int64_t a, int64_t b) { (void)a; (void)b; return 0; }

// --- Voxel/brick stubs ---
void seen_brick_pt_clear(int64_t a) { (void)a; }
void seen_brick_pt_copy_to_gpu(int64_t a, int64_t b) { (void)a; (void)b; }
int64_t seen_brick_pt_init(int64_t a) { (void)a; return 0; }
void seen_brick_pt_set(int64_t a, int64_t b, int64_t c) { (void)a; (void)b; (void)c; }
void seen_cascade2_pt_clear(int64_t a) { (void)a; }
void seen_cascade2_pt_copy_to_gpu(int64_t a, int64_t b) { (void)a; (void)b; }
int64_t seen_cascade2_pt_init(int64_t a) { (void)a; return 0; }
void seen_cascade2_pt_set(int64_t a, int64_t b, int64_t c) { (void)a; (void)b; (void)c; }
void seen_cascade3_pt_clear(int64_t a) { (void)a; }
void seen_cascade3_pt_copy_to_gpu(int64_t a, int64_t b) { (void)a; (void)b; }
int64_t seen_cascade3_pt_init(int64_t a) { (void)a; return 0; }
void seen_cascade3_pt_set(int64_t a, int64_t b, int64_t c) { (void)a; (void)b; (void)c; }
void seen_cascade4_pt_clear(int64_t a) { (void)a; }
void seen_cascade4_pt_copy_to_gpu(int64_t a, int64_t b) { (void)a; (void)b; }
int64_t seen_cascade4_pt_init(int64_t a) { (void)a; return 0; }
void seen_cascade4_pt_set(int64_t a, int64_t b, int64_t c) { (void)a; (void)b; (void)c; }
int64_t seen_chunk_brick_file_exists(int64_t a, int64_t b) { (void)a; (void)b; return 0; }
void seen_chunk_brick_save(int64_t a, int64_t b, int64_t c) { (void)a; (void)b; (void)c; }
int64_t seen_chunk_load_packed(int64_t a, int64_t b) { (void)a; (void)b; return 0; }
void seen_chunk_save(int64_t a, int64_t b) { (void)a; (void)b; }
int64_t seen_compute_brick_mip(int64_t a, int64_t b) { (void)a; (void)b; return 0; }
int64_t seen_copy_sub_brick(int64_t a, int64_t b, int64_t c, int64_t d) { (void)a; (void)b; (void)c; (void)d; return 0; }
int64_t seen_group_brick_cache_create(int64_t a) { (void)a; return 0; }
void seen_group_brick_cache_invalidate(int64_t a, int64_t b) { (void)a; (void)b; }
int64_t seen_group_brick_cache_lookup(int64_t a, int64_t b) { (void)a; (void)b; return 0; }

// --- Bootstrap-only testing / voxel classification stubs ---
int64_t seen_test_pass(void) { return 0; }
int64_t seen_test_fail(void) { return 0; }
int64_t seen_voxel_behavior_is_solid(void) { return 0; }
int64_t seen_voxel_behavior_is_transparent(void) { return 0; }

#define SEEN_BOOTSTRAP_I64_STUB(name) int64_t name(void) { return 0; }
SEEN_BOOTSTRAP_I64_STUB(seen_debug_collision_blocked)
SEEN_BOOTSTRAP_I64_STUB(seen_debug_collision_probe)
SEEN_BOOTSTRAP_I64_STUB(seen_debug_ground_column)
SEEN_BOOTSTRAP_I64_STUB(seen_debug_physics)
SEEN_BOOTSTRAP_I64_STUB(seen_debug_stepup)
SEEN_BOOTSTRAP_I64_STUB(seen_error_clear)
SEEN_BOOTSTRAP_I64_STUB(seen_error_code)
SEEN_BOOTSTRAP_I64_STUB(seen_error_message_len)
SEEN_BOOTSTRAP_I64_STUB(seen_error_message_ptr)
SEEN_BOOTSTRAP_I64_STUB(seen_error_subsystem)
SEEN_BOOTSTRAP_I64_STUB(seen_rt_atlas_clear)
SEEN_BOOTSTRAP_I64_STUB(seen_rt_atlas_upload_batch)
SEEN_BOOTSTRAP_I64_STUB(seen_shader_reload_module)
SEEN_BOOTSTRAP_I64_STUB(seen_vk_cmd_prepare_fallback_atlas)
SEEN_BOOTSTRAP_I64_STUB(seen_vk_create_debug_line_pipeline)
SEEN_BOOTSTRAP_I64_STUB(seen_vk_create_host_visible_buffer)
SEEN_BOOTSTRAP_I64_STUB(seen_vk_create_offscreen_target_sampled)
SEEN_BOOTSTRAP_I64_STUB(seen_vk_create_offscreen_target_transfer_sampled)
SEEN_BOOTSTRAP_I64_STUB(seen_vk_destroy_buffer)
SEEN_BOOTSTRAP_I64_STUB(seen_vk_device_wait_idle)
SEEN_BOOTSTRAP_I64_STUB(seen_vk_end_command_buffer)
SEEN_BOOTSTRAP_I64_STUB(seen_vk_end_render_pass)
SEEN_BOOTSTRAP_I64_STUB(seen_vk_free_memory)
SEEN_BOOTSTRAP_I64_STUB(seen_vk_get_last_buffer_memory)
SEEN_BOOTSTRAP_I64_STUB(seen_vk_get_swapchain_format)
SEEN_BOOTSTRAP_I64_STUB(seen_vk_unmap_memory)
SEEN_BOOTSTRAP_I64_STUB(seen_voxel_behavior_far_field_diag_kind)
SEEN_BOOTSTRAP_I64_STUB(seen_voxel_behavior_is_decorative_lod)
SEEN_BOOTSTRAP_I64_STUB(seen_voxel_behavior_is_landmark_ground)
SEEN_BOOTSTRAP_I64_STUB(seen_voxel_behavior_is_point_light_kind)
SEEN_BOOTSTRAP_I64_STUB(seen_voxel_behavior_is_stable_far_field)
SEEN_BOOTSTRAP_I64_STUB(seen_voxel_behavior_is_water)
SEEN_BOOTSTRAP_I64_STUB(seen_voxel_behavior_reset_defaults)
SEEN_BOOTSTRAP_I64_STUB(seen_world_to_chunk_coord)
#undef SEEN_BOOTSTRAP_I64_STUB

// --- Memory utility stubs ---
int64_t seen_mem_alloc(int64_t a) { return (int64_t)calloc(1, (size_t)a); }
void seen_mem_free(int64_t a) { free((void*)a); }
void seen_memcpy_bytes(int64_t dst, int64_t src, int64_t n) { if (dst && src && n > 0) memcpy((void*)dst, (void*)src, (size_t)n); }
void seen_memcpy_floats(int64_t dst, int64_t src, int64_t n) { if (dst && src && n > 0) memcpy((void*)dst, (void*)src, (size_t)n * sizeof(float)); }
void seen_memcpy_ints(int64_t dst, int64_t src, int64_t n) { if (dst && src && n > 0) memcpy((void*)dst, (void*)src, (size_t)n * sizeof(int64_t)); }
int64_t seen_mem_load_i32(int64_t ptr, int64_t off) { return ptr ? (int64_t)(*(int32_t*)((char*)ptr + off)) : 0; }
void seen_mem_store_f32(int64_t ptr, int64_t off, double val) { if (ptr) *(float*)((char*)ptr + off) = (float)val; }
void seen_mem_store_i32(int64_t ptr, int64_t off, int64_t val) { if (ptr) *(int32_t*)((char*)ptr + off) = (int32_t)val; }

// --- Performance/debug stubs ---
void seen_perf_10b_metrics(int64_t a, int64_t b, int64_t c) { (void)a; (void)b; (void)c; }
void seen_perf_cascade_metrics(int64_t a, int64_t b, int64_t c) { (void)a; (void)b; (void)c; }
void seen_perf_log(int64_t a, int64_t b) { (void)a; (void)b; }
void seen_perf_log_gpu(int64_t a, int64_t b, int64_t c) { (void)a; (void)b; (void)c; }
void seen_perf_log_update_phases(int64_t a, int64_t b, int64_t c, int64_t d) { (void)a; (void)b; (void)c; (void)d; }
void seen_perf_memory(int64_t a) { (void)a; }
void seen_perf_scale_metrics(int64_t a, int64_t b, int64_t c) { (void)a; (void)b; (void)c; }
void seen_perf_stats(int64_t a) { (void)a; }
void seen_perf_warn(int64_t a, int64_t b) { (void)a; (void)b; }
void seen_debug_log_load(int64_t a, int64_t b) { (void)a; (void)b; }
void seen_debug_mesh(int64_t a, int64_t b, int64_t c) { (void)a; (void)b; (void)c; }
void seen_debug_print_int(int64_t a) { (void)a; }
void seen_debug_spawn(int64_t a, int64_t b, int64_t c) { (void)a; (void)b; (void)c; }

// --- GPU request stubs ---
int64_t seen_gpu_req_get_gx(int64_t a) { (void)a; return 0; }
int64_t seen_gpu_req_get_gy(int64_t a) { (void)a; return 0; }
int64_t seen_gpu_req_get_gz(int64_t a) { (void)a; return 0; }
int64_t seen_gpu_req_get_kind(int64_t a) { (void)a; return 0; }

// --- Packed chunk extra stubs ---
int64_t seen_packed_chunk_data_ptr(int64_t a) { (void)a; return 0; }
int64_t seen_packed_chunk_is_empty(int64_t a) { (void)a; return 1; }
int64_t seen_rle_total_memory(void) { return 0; }

// --- Shader hot-reload stubs ---
int64_t seen_shader_check_modified(int64_t a) { (void)a; return 0; }
void seen_shader_register_watch(int64_t a, int64_t b) { (void)a; (void)b; }

// The actual main function should be defined in user code as seen_main
// This wrapper initializes the runtime
#ifdef SEEN_RUNTIME_MAIN
extern int seen_main(void);

int main(int argc, char** argv) {
    seen_runtime_init(argc, argv);
    return seen_main();
}
#endif
