// seen_compat_win32.h — Windows (mingw-w64) compatibility layer for Seen Runtime
//
// This header is included ONLY when _WIN32 is defined. It provides POSIX-compatible
// wrappers and typedefs so that seen_runtime.c compiles with mingw-w64.
//
// mingw-w64 already provides GCC builtins (__builtin_expect, __atomic_*, __attribute__),
// so those are NOT wrapped here.

#ifndef SEEN_COMPAT_WIN32_H
#define SEEN_COMPAT_WIN32_H

#ifdef _WIN32

// ============================================================================
// Windows Headers
// ============================================================================

// Prevent min/max macros from windows.h
#ifndef NOMINMAX
#define NOMINMAX
#endif
#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif

#include <windows.h>
#include <io.h>        // _open, _read, _write, _close, _pipe
#include <fcntl.h>     // _O_BINARY, O_RDONLY, O_RDWR
#include <direct.h>    // _mkdir
#include <process.h>   // _getpid, _popen, _pclose
#include <malloc.h>    // _aligned_malloc, _aligned_free, _aligned_realloc
#include <sys/stat.h>  // _stat

// ============================================================================
// POSIX type stubs (mingw-w64 may or may not provide these)
// ============================================================================

// Include sys/types.h which typically provides pid_t and ssize_t on mingw-w64
#include <sys/types.h>

// ============================================================================
// Process functions
// ============================================================================

// fork/waitpid/getpid: seen_runtime.c uses #ifdef _WIN32 to handle these
// directly (returning -1 for fork, using GetCurrentProcessId() for getpid, etc.).
// No POSIX process wrappers are defined here.

// popen/pclose
#define popen  _popen
#define pclose _pclose

// ============================================================================
// Environment functions
// ============================================================================

static inline int setenv(const char *name, const char *value, int overwrite) {
    if (!overwrite) {
        // Check if already set
        if (getenv(name) != NULL) return 0;
    }
    return _putenv_s(name, value);
}

static inline int unsetenv(const char *name) {
    // Setting to empty string removes on Windows via _putenv
    char buf[4096];
    snprintf(buf, sizeof(buf), "%s=", name);
    return _putenv(buf);
}

// environ — not directly available; stub for posix_spawn usage
// (posix_spawn is replaced with system() on Windows, so this is unused)

// ============================================================================
// Threading — pthread-compatible wrappers over Win32 threads
// ============================================================================

// --- pthread_t ---
typedef HANDLE pthread_t;

// --- pthread_attr_t (unused, stub) ---
typedef int pthread_attr_t;

// --- pthread_mutex_t via CRITICAL_SECTION ---
typedef CRITICAL_SECTION pthread_mutex_t;
typedef int pthread_mutexattr_t;

static inline int pthread_mutex_init(pthread_mutex_t *m, const pthread_mutexattr_t *attr) {
    (void)attr;
    InitializeCriticalSection(m);
    return 0;
}

static inline int pthread_mutex_destroy(pthread_mutex_t *m) {
    DeleteCriticalSection(m);
    return 0;
}

static inline int pthread_mutex_lock(pthread_mutex_t *m) {
    EnterCriticalSection(m);
    return 0;
}

static inline int pthread_mutex_unlock(pthread_mutex_t *m) {
    LeaveCriticalSection(m);
    return 0;
}

static inline int pthread_mutex_trylock(pthread_mutex_t *m) {
    return TryEnterCriticalSection(m) ? 0 : 1;
}

// --- pthread_rwlock_t via SRWLOCK ---
typedef SRWLOCK pthread_rwlock_t;
typedef int pthread_rwlockattr_t;

static inline int pthread_rwlock_init(pthread_rwlock_t *rw, const pthread_rwlockattr_t *attr) {
    (void)attr;
    InitializeSRWLock(rw);
    return 0;
}

static inline int pthread_rwlock_rdlock(pthread_rwlock_t *rw) {
    AcquireSRWLockShared(rw);
    return 0;
}

static inline int pthread_rwlock_wrlock(pthread_rwlock_t *rw) {
    AcquireSRWLockExclusive(rw);
    return 0;
}

static inline int pthread_rwlock_unlock(pthread_rwlock_t *rw) {
    // SRWLOCK requires knowing which mode was acquired.
    // We try exclusive first; if it fails, try shared.
    // However, Win32 SRWLOCK has separate release functions.
    // Since Seen code always pairs read_lock/read_unlock and write_lock/write_unlock,
    // we handle this by providing separate unlock helpers called from seen_runtime.c.
    // This generic unlock is a stub — seen_runtime.c uses the specific variants below.
    (void)rw;
    return 0;
}

static inline void pthread_rwlock_unlock_shared(pthread_rwlock_t *rw) {
    ReleaseSRWLockShared(rw);
}

static inline void pthread_rwlock_unlock_exclusive(pthread_rwlock_t *rw) {
    ReleaseSRWLockExclusive(rw);
}

static inline int pthread_rwlock_destroy(pthread_rwlock_t *rw) {
    // SRWLOCK has no destroy function — it's a static struct
    (void)rw;
    return 0;
}

// --- pthread_cond_t via CONDITION_VARIABLE ---
typedef CONDITION_VARIABLE pthread_cond_t;
typedef int pthread_condattr_t;

static inline int pthread_cond_init(pthread_cond_t *c, const pthread_condattr_t *attr) {
    (void)attr;
    InitializeConditionVariable(c);
    return 0;
}

static inline int pthread_cond_signal(pthread_cond_t *c) {
    WakeConditionVariable(c);
    return 0;
}

static inline int pthread_cond_broadcast(pthread_cond_t *c) {
    WakeAllConditionVariable(c);
    return 0;
}

static inline int pthread_cond_destroy(pthread_cond_t *c) {
    // CONDITION_VARIABLE has no destroy function
    (void)c;
    return 0;
}

// pthread_cond_timedwait: convert absolute timespec to relative ms for SleepConditionVariableCS
// Note: the mutex passed here must be a CRITICAL_SECTION (our pthread_mutex_t).
static inline int pthread_cond_timedwait(pthread_cond_t *c, pthread_mutex_t *m,
                                         const struct timespec *abstime) {
    // Convert absolute time to relative milliseconds
    DWORD ms = 1; // default 1ms if we can't compute
    if (abstime) {
        // Use GetSystemTimeAsFileTime instead of clock_gettime for reliability on mingw
        FILETIME ft;
        GetSystemTimeAsFileTime(&ft);
        ULARGE_INTEGER uli;
        uli.LowPart = ft.dwLowDateTime;
        uli.HighPart = ft.dwHighDateTime;
        // Convert from 100ns intervals since 1601 to seconds since epoch
        uint64_t now_100ns = uli.QuadPart - 116444736000000000ULL;
        uint64_t now_ms = now_100ns / 10000;
        uint64_t abs_ms = (uint64_t)abstime->tv_sec * 1000 + (uint64_t)abstime->tv_nsec / 1000000;
        if (abs_ms > now_ms) {
            ms = (DWORD)(abs_ms - now_ms);
        } else {
            ms = 0;
        }
    }
    BOOL ok = SleepConditionVariableCS(c, m, ms);
    return ok ? 0 : (GetLastError() == ERROR_TIMEOUT ? ETIMEDOUT : -1);
}

// --- pthread_create / pthread_join ---
typedef struct {
    void *(*start_routine)(void *);
    void *arg;
} seen_win32_thread_arg;

static DWORD WINAPI seen_win32_thread_func(LPVOID lpParam) {
    seen_win32_thread_arg *ta = (seen_win32_thread_arg *)lpParam;
    void *(*fn)(void *) = ta->start_routine;
    void *arg = ta->arg;
    free(ta);
    fn(arg);
    return 0;
}

static inline int pthread_create(pthread_t *thread, const pthread_attr_t *attr,
                                 void *(*start_routine)(void *), void *arg) {
    (void)attr;
    seen_win32_thread_arg *ta = (seen_win32_thread_arg *)malloc(sizeof(seen_win32_thread_arg));
    if (!ta) return -1;
    ta->start_routine = start_routine;
    ta->arg = arg;
    HANDLE h = CreateThread(NULL, 0, seen_win32_thread_func, ta, 0, NULL);
    if (h == NULL) {
        free(ta);
        return -1;
    }
    *thread = h;
    return 0;
}

static inline int pthread_join(pthread_t thread, void **retval) {
    (void)retval;
    WaitForSingleObject(thread, INFINITE);
    CloseHandle(thread);
    return 0;
}

static inline pthread_t pthread_self(void) {
    // Return a pseudo-handle; good enough for ID comparison
    return GetCurrentThread();
}

// --- pthread_key_t (Thread-Local Storage) ---
typedef DWORD pthread_key_t;

static inline int pthread_key_create(pthread_key_t *key, void (*destructor)(void *)) {
    (void)destructor; // Windows TLS doesn't support destructors in this simple wrapper
    DWORD k = TlsAlloc();
    if (k == TLS_OUT_OF_INDEXES) return -1;
    *key = k;
    return 0;
}

static inline int pthread_setspecific(pthread_key_t key, const void *value) {
    return TlsSetValue(key, (LPVOID)value) ? 0 : -1;
}

static inline void *pthread_getspecific(pthread_key_t key) {
    return TlsGetValue(key);
}

static inline int pthread_key_delete(pthread_key_t key) {
    return TlsFree(key) ? 0 : -1;
}

// --- pthread_barrier_t (manual implementation) ---
typedef struct {
    CRITICAL_SECTION lock;
    CONDITION_VARIABLE cond;
    unsigned count;     // total threads needed
    unsigned waiting;   // threads currently waiting
    unsigned phase;     // to distinguish successive barrier waits
} pthread_barrier_t;

typedef int pthread_barrierattr_t;

#ifndef PTHREAD_BARRIER_SERIAL_THREAD
#define PTHREAD_BARRIER_SERIAL_THREAD (-1)
#endif

static inline int pthread_barrier_init(pthread_barrier_t *b, const pthread_barrierattr_t *attr,
                                       unsigned count) {
    (void)attr;
    InitializeCriticalSection(&b->lock);
    InitializeConditionVariable(&b->cond);
    b->count = count;
    b->waiting = 0;
    b->phase = 0;
    return 0;
}

static inline int pthread_barrier_wait(pthread_barrier_t *b) {
    EnterCriticalSection(&b->lock);
    unsigned phase = b->phase;
    b->waiting++;
    if (b->waiting == b->count) {
        // Last thread: release all
        b->waiting = 0;
        b->phase++;
        LeaveCriticalSection(&b->lock);
        WakeAllConditionVariable(&b->cond);
        return PTHREAD_BARRIER_SERIAL_THREAD;
    }
    // Wait until phase changes
    while (b->phase == phase) {
        SleepConditionVariableCS(&b->cond, &b->lock, INFINITE);
    }
    LeaveCriticalSection(&b->lock);
    return 0;
}

static inline int pthread_barrier_destroy(pthread_barrier_t *b) {
    DeleteCriticalSection(&b->lock);
    // CONDITION_VARIABLE has no destroy
    return 0;
}

// --- pthread_setaffinity_np (stub on Windows) ---
// Windows uses SetThreadAffinityMask instead; the actual code already #ifdef's __linux__
// so this is just a fallback to prevent link errors.

// ============================================================================
// Sleep / Yield
// ============================================================================

// nanosleep — provided by mingw-w64 11+ via <pthread_time.h>.
// Older versions (< 11) need a fallback.
#if defined(__MINGW64_VERSION_MAJOR) && __MINGW64_VERSION_MAJOR < 11
static inline int nanosleep(const struct timespec *req, struct timespec *rem) {
    (void)rem;
    DWORD ms = (DWORD)(req->tv_sec * 1000 + req->tv_nsec / 1000000);
    if (ms == 0 && req->tv_nsec > 0) ms = 1;
    Sleep(ms);
    return 0;
}
#endif

// sched_yield
static inline int sched_yield(void) {
    SwitchToThread();
    return 0;
}

// ============================================================================
// Memory-mapped I/O
// ============================================================================
// seen_mapped_* in seen_runtime.c use Win32 APIs directly via #ifdef _WIN32
// (VirtualAlloc, CreateFileMapping, MapViewOfFile, etc.).
// No POSIX mmap constants or wrappers are needed here.

// ============================================================================
// Pipe / fd I/O
// ============================================================================

// pipe: __ChannelCreate in seen_runtime.c uses _pipe directly via #ifdef _WIN32.
// Do NOT define a pipe wrapper to avoid shadowing issues with 'pipe' variable names.

// POSIX fd I/O: do NOT define read/write/close as macros since "read" is used
// as a variable name in seen_runtime.c. Instead, the Channel functions use
// explicit _read/_write/_close calls guarded by #ifdef _WIN32 in seen_runtime.c.

// ============================================================================
// File system
// ============================================================================

// mkdir: __CreateDirectory in seen_runtime.c uses _mkdir directly via #ifdef _WIN32.
// Do NOT define a mkdir macro to avoid expansion issues in POSIX code paths.

// ============================================================================
// Aligned allocation
// ============================================================================

// On Windows (mingw), aligned_alloc is not reliably available.
// Use _aligned_malloc instead. IMPORTANT: memory allocated with _aligned_malloc
// MUST be freed with _aligned_free, not free().
static inline void *seen_win32_aligned_alloc(size_t alignment, size_t size) {
    if (size == 0) size = alignment;
    return _aligned_malloc(size, alignment);
}
#define aligned_alloc(a, s) seen_win32_aligned_alloc(a, s)

// Macro to free aligned memory — must use _aligned_free on Windows
#define seen_aligned_free(p) _aligned_free(p)

// _aligned_realloc is available on Windows
static inline void *seen_win32_aligned_realloc(void *ptr, size_t new_size, size_t alignment) {
    return _aligned_realloc(ptr, new_size, alignment);
}

// ============================================================================
// Backtrace (not available in mingw — stubs)
// ============================================================================

static inline int backtrace(void **buffer, int size) {
    (void)buffer; (void)size;
    return 0;
}

static inline void backtrace_symbols_fd(void *const *buffer, int size, int fd) {
    (void)buffer; (void)size; (void)fd;
}

// ============================================================================
// _exit and sysconf
// ============================================================================

// _exit is available in mingw via <stdlib.h>

// sysconf stub for _SC_NPROCESSORS_ONLN
#ifndef _SC_NPROCESSORS_ONLN
#define _SC_NPROCESSORS_ONLN 84
#endif

static inline long sysconf(int name) {
    if (name == _SC_NPROCESSORS_ONLN) {
        SYSTEM_INFO si;
        GetSystemInfo(&si);
        return (long)si.dwNumberOfProcessors;
    }
    return -1;
}

// ============================================================================
// clock_gettime (mingw-w64 usually provides it, but provide a fallback)
// ============================================================================

#ifndef CLOCK_MONOTONIC
#define CLOCK_MONOTONIC 1
#endif
#ifndef CLOCK_REALTIME
#define CLOCK_REALTIME 0
#endif

// mingw-w64 typically provides clock_gettime in <time.h>.
// If it's missing, uncomment the fallback below:
//
// static inline int clock_gettime(int clk_id, struct timespec *tp) {
//     LARGE_INTEGER freq, counter;
//     QueryPerformanceFrequency(&freq);
//     QueryPerformanceCounter(&counter);
//     tp->tv_sec = (time_t)(counter.QuadPart / freq.QuadPart);
//     tp->tv_nsec = (long)((counter.QuadPart % freq.QuadPart) * 1000000000LL / freq.QuadPart);
//     return 0;
// }

// ============================================================================
// posix_spawn — not available on Windows, replaced in seen_runtime.c
// ============================================================================

// No wrapper needed — __ExecuteProgram uses system() on Windows (handled in .c file)

// ============================================================================
// errno values that may be missing
// ============================================================================

#ifndef ETIMEDOUT
#define ETIMEDOUT 110
#endif

#endif // _WIN32
#endif // SEEN_COMPAT_WIN32_H
