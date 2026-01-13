// Seen Runtime Library Implementation
// Provides C implementations of Seen standard library functions

#define _POSIX_C_SOURCE 200809L

#include "seen_runtime.h"
#include <errno.h>
#include <sys/wait.h>
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
// File I/O Functions
// ============================================================================

SeenString readText(SeenString path) {
    // Null-terminate the path
    char* cpath = (char*)malloc(path.len + 1);
    memcpy(cpath, path.data, path.len);
    cpath[path.len] = 0;

    FILE* f = fopen(cpath, "r");
    free(cpath);

    if (!f) {
        SeenString empty = { 0, "" };
        return empty;
    }

    // Get file size
    fseek(f, 0, SEEK_END);
    long size = ftell(f);
    fseek(f, 0, SEEK_SET);

    // Read content
    char* data = (char*)malloc(size + 1);
    size_t read = fread(data, 1, size, f);
    data[read] = 0;
    fclose(f);

    SeenString result = { read, data };
    return result;
}

bool writeText(SeenString path, SeenString content) {
    // Null-terminate the path
    char* cpath = (char*)malloc(path.len + 1);
    memcpy(cpath, path.data, path.len);
    cpath[path.len] = 0;

    FILE* f = fopen(cpath, "w");
    free(cpath);

    if (!f) {
        return false;
    }

    size_t written = fwrite(content.data, 1, content.len, f);
    fclose(f);

    return written == (size_t)content.len;
}

// ============================================================================
// Process Functions
// ============================================================================

CommandResult runCommand(SeenString cmd) {
    CommandResult result = { false, { 0, "" }, -1 };

    // Null-terminate the command
    char* ccmd = (char*)malloc(cmd.len + 1);
    memcpy(ccmd, cmd.data, cmd.len);
    ccmd[cmd.len] = 0;

    // Run command and capture output
    FILE* pipe = popen(ccmd, "r");
    free(ccmd);

    if (!pipe) {
        return result;
    }

    // Read output
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
    result.exitCode = WEXITSTATUS(status);
    result.success = (result.exitCode == 0);
    result.output.len = length;
    result.output.data = output;

    return result;
}

bool commandWasSuccessful(CommandResult result) {
    return result.success;
}

// ============================================================================
// Environment Functions
// ============================================================================

SeenArray args(void) {
    SeenArray arr = seen_arr_new_str();

    for (int i = 0; i < g_argc; i++) {
        SeenString arg = seen_str_copy(g_argv[i]);
        seen_arr_push_str(&arr, arg);
    }

    return arr;
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
// Bootstrap Stub Functions
// These are placeholders for the actual compiler implementation
// In a full bootstrap, these would be replaced with compiled Seen code
// ============================================================================

FrontendResult* run_frontend(SeenString source, SeenString filename, SeenString lang) {
    // Stub implementation - returns failure
    static FrontendResult result;
    result.success = false;
    result.diagnostics.len = 0;
    result.diagnostics.cap = 0;
    result.diagnostics.data = NULL;
    result.program = NULL;

    // For now, print that we need the real implementation
    printf("ERROR: run_frontend stub called - need full compiler implementation\n");
    printf("  Source length: %ld bytes\n", (long)source.len);
    printf("  Filename: %.*s\n", (int)filename.len, filename.data);

    return &result;
}

void* CGenerator_new(void) {
    // Stub implementation
    printf("ERROR: CGenerator_new stub called - need full compiler implementation\n");
    return NULL;
}

SeenString CGenerator_generate(void* gen, void* program) {
    // Stub implementation
    printf("ERROR: CGenerator_generate stub called - need full compiler implementation\n");
    SeenString empty = { 0, "" };
    return empty;
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
