// Seen Hot Reload Runtime
// Provides runtime code reloading for @hot_reload decorator
// Uses dlopen/dlsym for dynamic library loading

#ifndef SEEN_HOTRELOAD_H
#define SEEN_HOTRELOAD_H

#include <stdint.h>
#include <stddef.h>

// ============================================================================
// Type Definitions (matches seen_runtime.h)
// ============================================================================

// Duplicate definitions for standalone compilation
// These must match the definitions in seen_runtime.h
typedef struct SeenStringHR {
    int64_t len;
    char* data;
} SeenStringHR;

typedef struct SeenArrayHR {
    int64_t len;
    int64_t cap;
    int64_t element_size;
    void* data;
} SeenArrayHR;

// ============================================================================
// Configuration
// ============================================================================

// Maximum path length for module names
#define SEEN_HOTRELOAD_MAX_PATH 512

// Maximum number of modules that can be loaded
#define SEEN_HOTRELOAD_MAX_MODULES 64

// Maximum number of functions per module
#define SEEN_HOTRELOAD_MAX_FUNCTIONS 256

// ============================================================================
// Hot Module Structure
// ============================================================================

// A loaded hot-reloadable module
typedef struct SeenHotModule {
    char name[256];             // Module name
    char path[SEEN_HOTRELOAD_MAX_PATH];  // Path to shared library
    void* handle;               // dlopen handle
    int64_t version;            // Reload count
    int64_t load_timestamp;     // Unix timestamp of last load
    int active;                 // Whether module is active

    // Function table for patching
    struct {
        char name[128];
        void* fn_ptr;
    } functions[SEEN_HOTRELOAD_MAX_FUNCTIONS];
    int function_count;
} SeenHotModule;

// ============================================================================
// Module Loading Functions
// ============================================================================

// Load a hot-reloadable module from a shared library path
// Returns pointer to module on success, NULL on failure
SeenHotModule* __seen_hotreload_load(SeenStringHR path);

// Load module with a name (for lookup)
SeenHotModule* __seen_hotreload_load_named(SeenStringHR path, SeenStringHR name);

// Reload a module from a new (or same) path
// Returns 1 on success, 0 on failure
int __seen_hotreload_reload(SeenHotModule* module, SeenStringHR new_path);

// Unload a module
void __seen_hotreload_unload(SeenHotModule* module);

// Find a loaded module by name
SeenHotModule* __seen_hotreload_find(SeenStringHR name);

// ============================================================================
// Function Lookup
// ============================================================================

// Get a function pointer by name from a module
void* __seen_hotreload_get_function(SeenHotModule* module, SeenStringHR name);

// Register a function in a module's function table
int __seen_hotreload_register_function(SeenHotModule* module, SeenStringHR name, void* fn_ptr);

/*
 * Typed dlsym trampolines for hosts that cannot safely call raw function
 * pointers from Seen code.
 */
int64_t __seen_hotreload_call_i64(SeenHotModule* module, SeenStringHR name);
int64_t __seen_hotreload_call_i64_ptr(SeenHotModule* module, SeenStringHR name, void* arg0);

// ============================================================================
// State Serialization
// ============================================================================

// Serialize object state to a byte array
// The state pointer should point to the start of the object's data
// size is the total size in bytes
SeenArrayHR* __seen_hotreload_serialize_state(void* state, int64_t size);

// Deserialize state from a byte array back to memory
// Returns a newly allocated buffer with the deserialized data
void* __seen_hotreload_deserialize_state(SeenArrayHR* data);

// Copy state from old to new object (for preserving state across reload)
void __seen_hotreload_copy_state(void* dest, void* src, int64_t size);

// ============================================================================
// File Watching (Optional)
// ============================================================================

// Start watching a file for changes
// callback is called with user_data when file changes
// Returns a watch ID, or -1 on failure
int __seen_hotreload_watch_file(SeenStringHR path, void (*callback)(void*), void* user_data);

// Stop watching a file
void __seen_hotreload_stop_watch(int watch_id);

// Poll for file changes (non-blocking)
// Returns number of callbacks triggered
int __seen_hotreload_poll(void);

// ============================================================================
// Utility Functions
// ============================================================================

// Get module version (reload count)
int64_t __seen_hotreload_get_version(SeenHotModule* module);

// Get last load timestamp
int64_t __seen_hotreload_get_timestamp(SeenHotModule* module);

// Check if module is valid/active
int __seen_hotreload_is_active(SeenHotModule* module);

// Get last error message
SeenStringHR __seen_hotreload_get_error(void);

// Initialize hot reload subsystem
void __seen_hotreload_init(void);

// Cleanup hot reload subsystem
void __seen_hotreload_cleanup(void);

#endif // SEEN_HOTRELOAD_H
