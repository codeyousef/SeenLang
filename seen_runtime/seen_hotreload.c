// Seen Hot Reload Runtime Implementation
// Provides runtime code reloading using dlopen/dlsym

#ifdef __linux__
#define _GNU_SOURCE
#endif
#include "seen_hotreload.h"
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <dlfcn.h>
#include <time.h>
#include <sys/stat.h>

// ============================================================================
// Global State
// ============================================================================

static SeenHotModule* g_hot_modules[SEEN_HOTRELOAD_MAX_MODULES];
static int g_hot_module_count = 0;
static int g_hotreload_initialized = 0;
static char g_last_error[512] = "";

// File watching state
#define MAX_WATCHES 64
typedef struct {
    char path[SEEN_HOTRELOAD_MAX_PATH];
    time_t last_mtime;
    void (*callback)(void*);
    void* user_data;
    int active;
} FileWatch;

static FileWatch g_watches[MAX_WATCHES];
static int g_watch_count = 0;

// ============================================================================
// Initialization
// ============================================================================

void __seen_hotreload_init(void) {
    if (g_hotreload_initialized) return;
    g_hotreload_initialized = 1;

    memset(g_hot_modules, 0, sizeof(g_hot_modules));
    g_hot_module_count = 0;
    g_last_error[0] = '\0';

    memset(g_watches, 0, sizeof(g_watches));
    g_watch_count = 0;
}

void __seen_hotreload_cleanup(void) {
    // Unload all modules
    for (int i = 0; i < g_hot_module_count; i++) {
        if (g_hot_modules[i]) {
            __seen_hotreload_unload(g_hot_modules[i]);
        }
    }
    g_hot_module_count = 0;
    g_hotreload_initialized = 0;
}

// ============================================================================
// Module Loading
// ============================================================================

SeenHotModule* __seen_hotreload_load(SeenStringHR path) {
    return __seen_hotreload_load_named(path, path);
}

SeenHotModule* __seen_hotreload_load_named(SeenStringHR path, SeenStringHR name) {
    __seen_hotreload_init();

    if (g_hot_module_count >= SEEN_HOTRELOAD_MAX_MODULES) {
        snprintf(g_last_error, sizeof(g_last_error), "Maximum number of modules reached");
        return NULL;
    }

    // Convert path to C string
    char* cpath = (char*)malloc(path.len + 1);
    if (!cpath) {
        snprintf(g_last_error, sizeof(g_last_error), "Out of memory");
        return NULL;
    }
    memcpy(cpath, path.data, path.len);
    cpath[path.len] = '\0';

    // Open shared library
    void* handle = dlopen(cpath, RTLD_NOW | RTLD_LOCAL);
    if (!handle) {
        snprintf(g_last_error, sizeof(g_last_error), "dlopen failed: %s", dlerror());
        free(cpath);
        return NULL;
    }

    // Allocate module structure
    SeenHotModule* module = (SeenHotModule*)calloc(1, sizeof(SeenHotModule));
    if (!module) {
        dlclose(handle);
        free(cpath);
        snprintf(g_last_error, sizeof(g_last_error), "Out of memory");
        return NULL;
    }

    // Initialize module
    strncpy(module->path, cpath, SEEN_HOTRELOAD_MAX_PATH - 1);
    module->path[SEEN_HOTRELOAD_MAX_PATH - 1] = '\0';

    if (name.data && name.len > 0 && name.len < 255) {
        memcpy(module->name, name.data, name.len);
        module->name[name.len] = '\0';
    } else {
        // Use path as name
        strncpy(module->name, cpath, 255);
        module->name[255] = '\0';
    }

    module->handle = handle;
    module->version = 1;
    module->load_timestamp = time(NULL);
    module->active = 1;
    module->function_count = 0;

    // Register module
    g_hot_modules[g_hot_module_count++] = module;

    free(cpath);

    fprintf(stderr, "[Hot Reload] Loaded module '%s' (v%ld)\n", module->name, (long)module->version);

    return module;
}

int __seen_hotreload_reload(SeenHotModule* module, SeenStringHR new_path) {
    if (!module || !module->active) {
        snprintf(g_last_error, sizeof(g_last_error), "Invalid module");
        return 0;
    }

    // Convert path to C string
    char* cpath = (char*)malloc(new_path.len + 1);
    if (!cpath) {
        snprintf(g_last_error, sizeof(g_last_error), "Out of memory");
        return 0;
    }
    memcpy(cpath, new_path.data, new_path.len);
    cpath[new_path.len] = '\0';

    // Save old handle
    void* old_handle = module->handle;

    // Load new library
    void* new_handle = dlopen(cpath, RTLD_NOW | RTLD_LOCAL);
    if (!new_handle) {
        snprintf(g_last_error, sizeof(g_last_error), "dlopen failed: %s", dlerror());
        free(cpath);
        return 0;
    }

    // Update function pointers
    for (int i = 0; i < module->function_count; i++) {
        void* new_fn = dlsym(new_handle, module->functions[i].name);
        if (new_fn) {
            module->functions[i].fn_ptr = new_fn;
        } else {
            fprintf(stderr, "[Hot Reload] Warning: Function '%s' not found in new module\n",
                    module->functions[i].name);
        }
    }

    // Close old library
    if (old_handle) {
        dlclose(old_handle);
    }

    // Update module state
    module->handle = new_handle;
    module->version++;
    module->load_timestamp = time(NULL);
    strncpy(module->path, cpath, SEEN_HOTRELOAD_MAX_PATH - 1);

    free(cpath);

    fprintf(stderr, "[Hot Reload] Reloaded module '%s' (v%ld)\n", module->name, (long)module->version);

    return 1;
}

void __seen_hotreload_unload(SeenHotModule* module) {
    if (!module) return;

    if (module->handle) {
        dlclose(module->handle);
        module->handle = NULL;
    }

    module->active = 0;
    module->function_count = 0;

    // Remove from registry
    for (int i = 0; i < g_hot_module_count; i++) {
        if (g_hot_modules[i] == module) {
            // Shift remaining modules
            for (int j = i; j < g_hot_module_count - 1; j++) {
                g_hot_modules[j] = g_hot_modules[j + 1];
            }
            g_hot_module_count--;
            break;
        }
    }

    fprintf(stderr, "[Hot Reload] Unloaded module '%s'\n", module->name);

    free(module);
}

SeenHotModule* __seen_hotreload_find(SeenStringHR name) {
    if (!name.data || name.len == 0) return NULL;

    for (int i = 0; i < g_hot_module_count; i++) {
        SeenHotModule* mod = g_hot_modules[i];
        if (mod && mod->active) {
            if (strlen(mod->name) == (size_t)name.len &&
                memcmp(mod->name, name.data, name.len) == 0) {
                return mod;
            }
        }
    }

    return NULL;
}

// ============================================================================
// Function Lookup
// ============================================================================

void* __seen_hotreload_get_function(SeenHotModule* module, SeenStringHR name) {
    if (!module || !module->handle || !name.data || name.len == 0) {
        return NULL;
    }

    // Convert name to C string
    char cname[256];
    int copy_len = (name.len < 255) ? (int)name.len : 255;
    memcpy(cname, name.data, copy_len);
    cname[copy_len] = '\0';

    // Look in function table first
    for (int i = 0; i < module->function_count; i++) {
        if (strcmp(module->functions[i].name, cname) == 0) {
            return module->functions[i].fn_ptr;
        }
    }

    // Try dlsym
    void* fn = dlsym(module->handle, cname);
    if (fn) {
        // Cache in function table
        __seen_hotreload_register_function(module, name, fn);
    }

    return fn;
}

int __seen_hotreload_register_function(SeenHotModule* module, SeenStringHR name, void* fn_ptr) {
    if (!module || module->function_count >= SEEN_HOTRELOAD_MAX_FUNCTIONS) {
        return 0;
    }

    // Check if already registered
    char cname[256];
    int copy_len = (name.len < 127) ? (int)name.len : 127;
    memcpy(cname, name.data, copy_len);
    cname[copy_len] = '\0';

    for (int i = 0; i < module->function_count; i++) {
        if (strcmp(module->functions[i].name, cname) == 0) {
            module->functions[i].fn_ptr = fn_ptr;
            return 1;
        }
    }

    // Add new entry
    strncpy(module->functions[module->function_count].name, cname, 127);
    module->functions[module->function_count].fn_ptr = fn_ptr;
    module->function_count++;

    return 1;
}

// ============================================================================
// State Serialization
// ============================================================================

SeenArrayHR* __seen_hotreload_serialize_state(void* state, int64_t size) {
    // Allocate array
    SeenArrayHR* arr = (SeenArrayHR*)malloc(sizeof(SeenArrayHR));
    if (!arr) return NULL;

    arr->len = 0;
    arr->cap = size > 0 ? size : 8;
    arr->element_size = sizeof(int64_t);
    arr->data = malloc(arr->cap * sizeof(int64_t));

    if (!arr->data) {
        free(arr);
        return NULL;
    }

    if (!state || size <= 0) return arr;

    // Store state as bytes using i64 values
    unsigned char* bytes = (unsigned char*)state;
    int64_t* data = (int64_t*)arr->data;
    for (int64_t i = 0; i < size; i++) {
        data[i] = (int64_t)bytes[i];
    }
    arr->len = size;

    return arr;
}

void* __seen_hotreload_deserialize_state(SeenArrayHR* data) {
    if (!data || data->len == 0) return NULL;

    void* state = malloc(data->len);
    if (!state) return NULL;

    unsigned char* bytes = (unsigned char*)state;
    int64_t* src = (int64_t*)data->data;
    for (int64_t i = 0; i < data->len; i++) {
        bytes[i] = (unsigned char)src[i];
    }

    return state;
}

void __seen_hotreload_copy_state(void* dest, void* src, int64_t size) {
    if (dest && src && size > 0) {
        memcpy(dest, src, size);
    }
}

// ============================================================================
// File Watching
// ============================================================================

static time_t get_file_mtime(const char* path) {
    struct stat st;
    if (stat(path, &st) == 0) {
        return st.st_mtime;
    }
    return 0;
}

int __seen_hotreload_watch_file(SeenStringHR path, void (*callback)(void*), void* user_data) {
    if (g_watch_count >= MAX_WATCHES) return -1;

    // Convert path to C string
    char cpath[SEEN_HOTRELOAD_MAX_PATH];
    int copy_len = (path.len < SEEN_HOTRELOAD_MAX_PATH - 1) ? (int)path.len : SEEN_HOTRELOAD_MAX_PATH - 1;
    memcpy(cpath, path.data, copy_len);
    cpath[copy_len] = '\0';

    // Find a free slot
    int slot = -1;
    for (int i = 0; i < MAX_WATCHES; i++) {
        if (!g_watches[i].active) {
            slot = i;
            break;
        }
    }

    if (slot < 0) return -1;

    // Set up watch
    strncpy(g_watches[slot].path, cpath, SEEN_HOTRELOAD_MAX_PATH - 1);
    g_watches[slot].last_mtime = get_file_mtime(cpath);
    g_watches[slot].callback = callback;
    g_watches[slot].user_data = user_data;
    g_watches[slot].active = 1;

    g_watch_count++;

    return slot;
}

void __seen_hotreload_stop_watch(int watch_id) {
    if (watch_id < 0 || watch_id >= MAX_WATCHES) return;

    if (g_watches[watch_id].active) {
        g_watches[watch_id].active = 0;
        g_watch_count--;
    }
}

int __seen_hotreload_poll(void) {
    int triggered = 0;

    for (int i = 0; i < MAX_WATCHES; i++) {
        if (!g_watches[i].active) continue;

        time_t current_mtime = get_file_mtime(g_watches[i].path);
        if (current_mtime != g_watches[i].last_mtime && current_mtime != 0) {
            g_watches[i].last_mtime = current_mtime;
            if (g_watches[i].callback) {
                g_watches[i].callback(g_watches[i].user_data);
                triggered++;
            }
        }
    }

    return triggered;
}

// ============================================================================
// Utility Functions
// ============================================================================

int64_t __seen_hotreload_get_version(SeenHotModule* module) {
    if (!module) return 0;
    return module->version;
}

int64_t __seen_hotreload_get_timestamp(SeenHotModule* module) {
    if (!module) return 0;
    return module->load_timestamp;
}

int __seen_hotreload_is_active(SeenHotModule* module) {
    if (!module) return 0;
    return module->active;
}

SeenStringHR __seen_hotreload_get_error(void) {
    SeenStringHR result;
    result.len = strlen(g_last_error);
    result.data = g_last_error;
    return result;
}
