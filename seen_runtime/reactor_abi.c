#include "reactor_abi.h"

#include <errno.h>
#include <limits.h>
#include <stdint.h>
#include <stdatomic.h>
#include <stdlib.h>
#include <string.h>

#if defined(__linux__)
#include <sys/epoll.h>
#include <sys/eventfd.h>
#include <unistd.h>
#elif defined(__APPLE__)
#include <sys/event.h>
#include <sys/time.h>
#include <unistd.h>
#elif defined(_WIN32)
#include <windows.h>
#endif

#if !defined(_WIN32)
#include <pthread.h>
#endif

#define SEEN_REACTOR_MAX_CAPACITY 1048576ULL
#define SEEN_REACTOR_WAKE_KEY UINT64_MAX

typedef struct SeenReactorRegistration {
    int64_t native_handle;
    uint64_t token;
    uint64_t generation;
    int32_t interest;
    int active;
} SeenReactorRegistration;

typedef struct SeenReactorEvent {
    uint64_t token;
    uint64_t generation;
    int32_t flags;
    int64_t native_status;
    uint64_t progress;
} SeenReactorEvent;

typedef struct SeenReactor {
    uint64_t capacity;
    int32_t backend;
    SeenReactorRegistration *registrations;
    SeenReactorEvent *events;
    void *native_events;
    uint64_t event_count;
#if defined(__linux__)
    int queue;
    int wake;
#elif defined(__APPLE__)
    int queue;
#elif defined(_WIN32)
    HANDLE queue;
#endif
} SeenReactor;

static _Atomic uint64_t seen_reactor_handles = 0;

static int32_t seen_reactor_validate_interest(int32_t interest) {
    const int32_t allowed = SEEN_REACTOR_READABLE | SEEN_REACTOR_WRITABLE |
        SEEN_REACTOR_COMPLETION;
    return interest > 0 && (interest & ~allowed) == 0;
}

static uint64_t seen_reactor_key(uint64_t slot, uint64_t generation) {
    return ((generation & UINT64_C(0xffffffff)) << 32) |
        (slot & UINT64_C(0xffffffff));
}

static uint64_t seen_reactor_key_slot(uint64_t key) {
    return key & UINT64_C(0xffffffff);
}

static uint64_t seen_reactor_key_generation(uint64_t key) {
    return key >> 32;
}

int32_t seen_reactor_backend_capability(int32_t backend) {
#if defined(__linux__)
    return backend == SEEN_REACTOR_BACKEND_EPOLL ? SEEN_REACTOR_OK :
        SEEN_REACTOR_UNSUPPORTED;
#elif defined(__APPLE__)
    return backend == SEEN_REACTOR_BACKEND_KQUEUE ? SEEN_REACTOR_OK :
        SEEN_REACTOR_UNSUPPORTED;
#elif defined(_WIN32)
    return backend == SEEN_REACTOR_BACKEND_IOCP ? SEEN_REACTOR_OK :
        SEEN_REACTOR_UNSUPPORTED;
#else
    (void)backend;
    return SEEN_REACTOR_UNSUPPORTED;
#endif
}

int32_t seen_reactor_create(uint64_t capacity, uint64_t *out_handle,
                            int32_t *out_backend) {
    if (!out_handle || !out_backend || capacity == 0 ||
        capacity > SEEN_REACTOR_MAX_CAPACITY ||
        capacity > SIZE_MAX / sizeof(SeenReactorRegistration) ||
        capacity > SIZE_MAX / sizeof(SeenReactorEvent))
        return capacity == 0 || capacity > SEEN_REACTOR_MAX_CAPACITY ?
            SEEN_REACTOR_LIMIT : SEEN_REACTOR_INVALID;
    *out_handle = 0;
    *out_backend = 0;
    SeenReactor *reactor = (SeenReactor *)calloc(1, sizeof(*reactor));
    if (!reactor) return SEEN_REACTOR_UNAVAILABLE;
    reactor->registrations = (SeenReactorRegistration *)calloc(
        (size_t)capacity, sizeof(*reactor->registrations));
    reactor->events = (SeenReactorEvent *)calloc(
        (size_t)capacity, sizeof(*reactor->events));
#if defined(__linux__)
    reactor->native_events = calloc((size_t)capacity,
                                    sizeof(struct epoll_event));
#elif defined(__APPLE__)
    reactor->native_events = calloc((size_t)capacity, sizeof(struct kevent));
#elif defined(_WIN32)
    reactor->native_events = calloc((size_t)capacity,
                                    sizeof(OVERLAPPED_ENTRY));
#endif
    if (!reactor->registrations || !reactor->events ||
        !reactor->native_events) {
        free(reactor->native_events);
        free(reactor->events);
        free(reactor->registrations);
        free(reactor);
        return SEEN_REACTOR_UNAVAILABLE;
    }
    reactor->capacity = capacity;
#if defined(__linux__)
    reactor->backend = SEEN_REACTOR_BACKEND_EPOLL;
    reactor->queue = epoll_create1(EPOLL_CLOEXEC);
    reactor->wake = eventfd(0, EFD_CLOEXEC | EFD_NONBLOCK);
    if (reactor->queue < 0 || reactor->wake < 0) goto unavailable;
    struct epoll_event wake_event;
    memset(&wake_event, 0, sizeof(wake_event));
    wake_event.events = EPOLLIN;
    wake_event.data.u64 = SEEN_REACTOR_WAKE_KEY;
    if (epoll_ctl(reactor->queue, EPOLL_CTL_ADD, reactor->wake,
                  &wake_event) != 0) goto unavailable;
#elif defined(__APPLE__)
    reactor->backend = SEEN_REACTOR_BACKEND_KQUEUE;
    reactor->queue = kqueue();
    if (reactor->queue < 0) goto unavailable;
    struct kevent wake_event;
    EV_SET(&wake_event, 1, EVFILT_USER, EV_ADD | EV_CLEAR, 0, 0,
           (void *)(uintptr_t)SEEN_REACTOR_WAKE_KEY);
    if (kevent(reactor->queue, &wake_event, 1, NULL, 0, NULL) != 0)
        goto unavailable;
#elif defined(_WIN32)
    reactor->backend = SEEN_REACTOR_BACKEND_IOCP;
    reactor->queue = CreateIoCompletionPort(INVALID_HANDLE_VALUE, NULL, 0, 1);
    if (!reactor->queue) goto unavailable;
#else
    free(reactor->events);
    free(reactor->registrations);
    free(reactor);
    return SEEN_REACTOR_UNSUPPORTED;
#endif
    *out_handle = (uint64_t)(uintptr_t)reactor;
    *out_backend = reactor->backend;
    atomic_fetch_add_explicit(&seen_reactor_handles, 1, memory_order_relaxed);
    return SEEN_REACTOR_OK;

#if defined(__linux__) || defined(__APPLE__) || defined(_WIN32)
unavailable:
#if defined(__linux__)
    if (reactor->wake >= 0) close(reactor->wake);
    if (reactor->queue >= 0) close(reactor->queue);
#elif defined(__APPLE__)
    if (reactor->queue >= 0) close(reactor->queue);
#elif defined(_WIN32)
    if (reactor->queue) CloseHandle(reactor->queue);
#endif
    free(reactor->native_events);
    free(reactor->events);
    free(reactor->registrations);
    free(reactor);
    return SEEN_REACTOR_UNAVAILABLE;
#endif
}

static int64_t seen_reactor_find_slot(SeenReactor *reactor,
                                      uint64_t token) {
    for (uint64_t i = 0; i < reactor->capacity; ++i)
        if (reactor->registrations[i].active &&
            reactor->registrations[i].token == token) return (int64_t)i;
    return -1;
}

int32_t seen_reactor_register(uint64_t handle, int64_t native_handle,
                              int32_t interest, uint64_t token,
                              uint64_t generation) {
    SeenReactor *reactor = (SeenReactor *)(uintptr_t)handle;
    if (!reactor || native_handle < 0 || token == 0 || generation == 0 ||
        generation > UINT32_MAX || !seen_reactor_validate_interest(interest))
        return SEEN_REACTOR_INVALID;
    if (seen_reactor_find_slot(reactor, token) >= 0)
        return SEEN_REACTOR_INVALID;
    uint64_t slot = reactor->capacity;
    for (uint64_t i = 0; i < reactor->capacity; ++i)
        if (!reactor->registrations[i].active) { slot = i; break; }
    if (slot == reactor->capacity || slot > UINT32_MAX)
        return SEEN_REACTOR_LIMIT;
    uint64_t key = seen_reactor_key(slot, generation);
#if defined(__linux__)
    struct epoll_event event;
    memset(&event, 0, sizeof(event));
    if (interest & SEEN_REACTOR_READABLE) event.events |= EPOLLIN;
    if (interest & SEEN_REACTOR_WRITABLE) event.events |= EPOLLOUT;
    event.events |= EPOLLERR | EPOLLHUP;
    event.data.u64 = key;
    if (epoll_ctl(reactor->queue, EPOLL_CTL_ADD, (int)native_handle,
                  &event) != 0)
        return errno == ENOMEM || errno == ENOSPC ? SEEN_REACTOR_LIMIT :
            SEEN_REACTOR_UNAVAILABLE;
#elif defined(__APPLE__)
    struct kevent changes[2];
    int count = 0;
    if (interest & SEEN_REACTOR_READABLE)
        EV_SET(&changes[count++], (uintptr_t)native_handle, EVFILT_READ,
               EV_ADD | EV_CLEAR, 0, 0, (void *)(uintptr_t)key);
    if (interest & SEEN_REACTOR_WRITABLE)
        EV_SET(&changes[count++], (uintptr_t)native_handle, EVFILT_WRITE,
               EV_ADD | EV_CLEAR, 0, 0, (void *)(uintptr_t)key);
    if ((interest & SEEN_REACTOR_COMPLETION) || count == 0)
        return SEEN_REACTOR_UNSUPPORTED;
    if (kevent(reactor->queue, changes, count, NULL, 0, NULL) != 0)
        return errno == ENOMEM ? SEEN_REACTOR_LIMIT : SEEN_REACTOR_UNAVAILABLE;
#elif defined(_WIN32)
    if (!(interest & SEEN_REACTOR_COMPLETION)) return SEEN_REACTOR_UNSUPPORTED;
    if (!CreateIoCompletionPort((HANDLE)(uintptr_t)native_handle,
                                reactor->queue, (ULONG_PTR)key, 0))
        return SEEN_REACTOR_UNAVAILABLE;
#else
    return SEEN_REACTOR_UNSUPPORTED;
#endif
    reactor->registrations[slot].native_handle = native_handle;
    reactor->registrations[slot].token = token;
    reactor->registrations[slot].generation = generation;
    reactor->registrations[slot].interest = interest;
    reactor->registrations[slot].active = 1;
    return SEEN_REACTOR_OK;
}

int32_t seen_reactor_deregister(uint64_t handle, int64_t native_handle,
                                uint64_t token, uint64_t generation) {
    SeenReactor *reactor = (SeenReactor *)(uintptr_t)handle;
    if (!reactor || token == 0 || generation == 0)
        return SEEN_REACTOR_INVALID;
    int64_t slot = seen_reactor_find_slot(reactor, token);
    if (slot < 0) return SEEN_REACTOR_NOT_FOUND;
    SeenReactorRegistration *registration = &reactor->registrations[slot];
    if (registration->native_handle != native_handle ||
        registration->generation != generation)
        return SEEN_REACTOR_NOT_FOUND;
#if defined(__linux__)
    if (epoll_ctl(reactor->queue, EPOLL_CTL_DEL, (int)native_handle, NULL) != 0 &&
        errno != ENOENT && errno != EBADF)
        return SEEN_REACTOR_UNAVAILABLE;
#elif defined(__APPLE__)
    struct kevent changes[2];
    int count = 0;
    if (registration->interest & SEEN_REACTOR_READABLE)
        EV_SET(&changes[count++], (uintptr_t)native_handle, EVFILT_READ,
               EV_DELETE, 0, 0, NULL);
    if (registration->interest & SEEN_REACTOR_WRITABLE)
        EV_SET(&changes[count++], (uintptr_t)native_handle, EVFILT_WRITE,
               EV_DELETE, 0, 0, NULL);
    if (count && kevent(reactor->queue, changes, count, NULL, 0, NULL) != 0 &&
        errno != ENOENT) return SEEN_REACTOR_UNAVAILABLE;
#elif defined(_WIN32)
    /* IOCP association cannot be removed. Generation filtering makes all
       late completions stale, and Seen owns the OVERLAPPED lifetime. */
#endif
    registration->active = 0;
    return SEEN_REACTOR_OK;
}

int32_t seen_reactor_wakeup(uint64_t handle) {
    SeenReactor *reactor = (SeenReactor *)(uintptr_t)handle;
    if (!reactor) return SEEN_REACTOR_INVALID;
#if defined(__linux__)
    uint64_t value = 1;
    ssize_t result = write(reactor->wake, &value, sizeof(value));
    if (result == (ssize_t)sizeof(value) ||
        (result < 0 && errno == EAGAIN)) return SEEN_REACTOR_OK;
    return SEEN_REACTOR_UNAVAILABLE;
#elif defined(__APPLE__)
    struct kevent event;
    EV_SET(&event, 1, EVFILT_USER, 0, NOTE_TRIGGER, 0, NULL);
    return kevent(reactor->queue, &event, 1, NULL, 0, NULL) == 0 ?
        SEEN_REACTOR_OK : SEEN_REACTOR_UNAVAILABLE;
#elif defined(_WIN32)
    return PostQueuedCompletionStatus(reactor->queue, 0,
        (ULONG_PTR)SEEN_REACTOR_WAKE_KEY, NULL) ? SEEN_REACTOR_OK :
        SEEN_REACTOR_UNAVAILABLE;
#else
    return SEEN_REACTOR_UNSUPPORTED;
#endif
}

static int seen_reactor_timeout_ms(int64_t nanoseconds) {
    if (nanoseconds < 0) return -1;
    if (nanoseconds == 0) return 0;
    uint64_t rounded = ((uint64_t)nanoseconds + 999999ULL) / 1000000ULL;
    return rounded > INT_MAX ? INT_MAX : (int)rounded;
}

static int seen_reactor_store_event(SeenReactor *reactor, uint64_t key,
                                    int32_t flags, int64_t status,
                                    uint64_t progress) {
    if (key == SEEN_REACTOR_WAKE_KEY) {
        SeenReactorEvent *out = &reactor->events[reactor->event_count++];
        out->token = 0;
        out->generation = 0;
        out->flags = SEEN_REACTOR_WAKEUP;
        out->native_status = status;
        out->progress = progress;
        return 1;
    }
    uint64_t slot = seen_reactor_key_slot(key);
    uint64_t generation = seen_reactor_key_generation(key);
    if (slot >= reactor->capacity) return 0;
    SeenReactorRegistration *registration = &reactor->registrations[slot];
    if (!registration->active || registration->generation != generation)
        return 0;
    SeenReactorEvent *out = &reactor->events[reactor->event_count++];
    out->token = registration->token;
    out->generation = generation;
    out->flags = flags;
    out->native_status = status;
    out->progress = progress;
    return 1;
}

int32_t seen_reactor_poll(uint64_t handle, int64_t timeout_nanoseconds,
                          uint64_t max_events, uint64_t *out_count) {
    SeenReactor *reactor = (SeenReactor *)(uintptr_t)handle;
    if (!reactor || !out_count || max_events == 0 ||
        max_events > reactor->capacity) return max_events == 0 ||
        (reactor && max_events > reactor->capacity) ? SEEN_REACTOR_LIMIT :
        SEEN_REACTOR_INVALID;
    reactor->event_count = 0;
    *out_count = 0;
#if defined(__linux__)
    struct epoll_event *native_events =
        (struct epoll_event *)reactor->native_events;
    int count;
    do { count = epoll_wait(reactor->queue, native_events, (int)max_events,
                            seen_reactor_timeout_ms(timeout_nanoseconds)); }
    while (count < 0 && errno == EINTR);
    if (count < 0) return SEEN_REACTOR_UNAVAILABLE;
    for (int i = 0; i < count && reactor->event_count < max_events; ++i) {
        uint64_t key = native_events[i].data.u64;
        int32_t flags = 0;
        if (key == SEEN_REACTOR_WAKE_KEY) {
            uint64_t value;
            while (read(reactor->wake, &value, sizeof(value)) > 0) {}
        } else {
            if (native_events[i].events & EPOLLIN) flags |= SEEN_REACTOR_READABLE;
            if (native_events[i].events & EPOLLOUT) flags |= SEEN_REACTOR_WRITABLE;
            if (native_events[i].events & EPOLLERR) flags |= SEEN_REACTOR_ERROR;
            if (native_events[i].events & EPOLLHUP) flags |= SEEN_REACTOR_HANGUP;
        }
        seen_reactor_store_event(reactor, key, flags, 0, 0);
    }
#elif defined(__APPLE__)
    struct kevent *native_events = (struct kevent *)reactor->native_events;
    struct timespec timeout;
    struct timespec *timeout_pointer = NULL;
    if (timeout_nanoseconds >= 0) {
        timeout.tv_sec = (time_t)(timeout_nanoseconds / 1000000000LL);
        timeout.tv_nsec = (long)(timeout_nanoseconds % 1000000000LL);
        timeout_pointer = &timeout;
    }
    int count;
    do { count = kevent(reactor->queue, NULL, 0, native_events,
                        (int)max_events, timeout_pointer); }
    while (count < 0 && errno == EINTR);
    if (count < 0) return SEEN_REACTOR_UNAVAILABLE;
    for (int i = 0; i < count && reactor->event_count < max_events; ++i) {
        uint64_t key = native_events[i].filter == EVFILT_USER ?
            SEEN_REACTOR_WAKE_KEY : (uint64_t)(uintptr_t)native_events[i].udata;
        int32_t flags = native_events[i].filter == EVFILT_READ ?
            SEEN_REACTOR_READABLE : native_events[i].filter == EVFILT_WRITE ?
            SEEN_REACTOR_WRITABLE : 0;
        if (native_events[i].flags & EV_ERROR) flags |= SEEN_REACTOR_ERROR;
        if (native_events[i].flags & EV_EOF) flags |= SEEN_REACTOR_HANGUP;
        seen_reactor_store_event(reactor, key, flags,
            (int64_t)native_events[i].data, 0);
    }
#elif defined(_WIN32)
    OVERLAPPED_ENTRY *native_events =
        (OVERLAPPED_ENTRY *)reactor->native_events;
    ULONG count = 0;
    BOOL ok = GetQueuedCompletionStatusEx(reactor->queue, native_events,
        (ULONG)max_events, &count,
        (DWORD)seen_reactor_timeout_ms(timeout_nanoseconds), FALSE);
    if (!ok && GetLastError() != WAIT_TIMEOUT) {
        return SEEN_REACTOR_UNAVAILABLE;
    }
    for (ULONG i = 0; i < count && reactor->event_count < max_events; ++i)
        seen_reactor_store_event(reactor,
            (uint64_t)native_events[i].lpCompletionKey,
            SEEN_REACTOR_COMPLETION, 0,
            (uint64_t)native_events[i].dwNumberOfBytesTransferred);
#else
    return SEEN_REACTOR_UNSUPPORTED;
#endif
    *out_count = reactor->event_count;
    return SEEN_REACTOR_OK;
}

int32_t seen_reactor_event(uint64_t handle, uint64_t index,
                           uint64_t *out_token, uint64_t *out_generation,
                           int32_t *out_flags, int64_t *out_native_status,
                           uint64_t *out_progress) {
    SeenReactor *reactor = (SeenReactor *)(uintptr_t)handle;
    if (!reactor || index >= reactor->event_count || !out_token ||
        !out_generation || !out_flags || !out_native_status || !out_progress)
        return SEEN_REACTOR_INVALID;
    SeenReactorEvent *event = &reactor->events[index];
    *out_token = event->token;
    *out_generation = event->generation;
    *out_flags = event->flags;
    *out_native_status = event->native_status;
    *out_progress = event->progress;
    return SEEN_REACTOR_OK;
}

int32_t seen_reactor_close(uint64_t *handle) {
    if (!handle) return SEEN_REACTOR_INVALID;
    if (*handle == 0) return SEEN_REACTOR_OK;
    SeenReactor *reactor = (SeenReactor *)(uintptr_t)*handle;
#if defined(__linux__)
    close(reactor->wake);
    close(reactor->queue);
#elif defined(__APPLE__)
    close(reactor->queue);
#elif defined(_WIN32)
    CloseHandle(reactor->queue);
#endif
    free(reactor->native_events);
    free(reactor->events);
    free(reactor->registrations);
    free(reactor);
    *handle = 0;
    atomic_fetch_sub_explicit(&seen_reactor_handles, 1, memory_order_relaxed);
    return SEEN_REACTOR_OK;
}

uint64_t seen_reactor_live_handles(void) {
    return atomic_load_explicit(&seen_reactor_handles, memory_order_relaxed);
}

int32_t seen_reactor_native_handle_close(int64_t native_handle) {
    if (native_handle < 0) return SEEN_REACTOR_INVALID;
#if defined(_WIN32)
    return CloseHandle((HANDLE)(uintptr_t)native_handle) ? SEEN_REACTOR_OK :
        SEEN_REACTOR_UNAVAILABLE;
#elif defined(__linux__) || defined(__APPLE__)
    return close((int)native_handle) == 0 ? SEEN_REACTOR_OK :
        SEEN_REACTOR_UNAVAILABLE;
#else
    return SEEN_REACTOR_UNSUPPORTED;
#endif
}

typedef int64_t (*SeenBlockingFunction)(int64_t);
typedef struct SeenBlockingTask {
    uint64_t token;
    SeenBlockingFunction function;
    int64_t argument;
} SeenBlockingTask;
typedef struct SeenBlockingCompletion {
    uint64_t token;
    int64_t result;
} SeenBlockingCompletion;

#if !defined(_WIN32)
typedef struct SeenBlockingPool {
    pthread_mutex_t mutex;
    pthread_cond_t ready;
    pthread_t *threads;
    SeenBlockingTask *tasks;
    SeenBlockingCompletion *completions;
    uint64_t worker_count;
    uint64_t capacity;
    uint64_t completion_capacity;
    uint64_t task_head, task_count, active_count;
    uint64_t completion_head, completion_count;
    int shutting_down;
} SeenBlockingPool;

static _Atomic uint64_t seen_blocking_workers = 0;

static void *seen_blocking_worker(void *opaque) {
    SeenBlockingPool *pool = (SeenBlockingPool *)opaque;
    for (;;) {
        pthread_mutex_lock(&pool->mutex);
        while (!pool->shutting_down && pool->task_count == 0)
            pthread_cond_wait(&pool->ready, &pool->mutex);
        if (pool->shutting_down && pool->task_count == 0) {
            pthread_mutex_unlock(&pool->mutex);
            return NULL;
        }
        SeenBlockingTask task = pool->tasks[pool->task_head];
        pool->task_head = (pool->task_head + 1) % pool->capacity;
        pool->task_count--;
        pool->active_count++;
        pthread_mutex_unlock(&pool->mutex);
        int64_t result = task.function(task.argument);
        pthread_mutex_lock(&pool->mutex);
        pool->active_count--;
        uint64_t slot = (pool->completion_head + pool->completion_count) %
            pool->completion_capacity;
        pool->completions[slot].token = task.token;
        pool->completions[slot].result = result;
        pool->completion_count++;
        pthread_mutex_unlock(&pool->mutex);
    }
}
#else
typedef struct SeenBlockingPool { int unused; } SeenBlockingPool;
static _Atomic uint64_t seen_blocking_workers = 0;
#endif

int32_t seen_blocking_pool_create(uint64_t workers, uint64_t queue_capacity,
                                  uint64_t *out_handle) {
    if (!out_handle || workers == 0 || workers > 64 || queue_capacity == 0 ||
        queue_capacity > SEEN_REACTOR_MAX_CAPACITY)
        return workers == 0 || workers > 64 || queue_capacity == 0 ||
            queue_capacity > SEEN_REACTOR_MAX_CAPACITY ? SEEN_REACTOR_LIMIT :
            SEEN_REACTOR_INVALID;
    *out_handle = 0;
#if defined(_WIN32)
    return SEEN_REACTOR_UNSUPPORTED;
#else
    SeenBlockingPool *pool = (SeenBlockingPool *)calloc(1, sizeof(*pool));
    if (!pool) return SEEN_REACTOR_UNAVAILABLE;
    pool->threads = (pthread_t *)calloc((size_t)workers, sizeof(pthread_t));
    pool->tasks = (SeenBlockingTask *)calloc((size_t)queue_capacity,
                                             sizeof(*pool->tasks));
    pool->completion_capacity = queue_capacity + workers;
    pool->completions = (SeenBlockingCompletion *)calloc(
        (size_t)pool->completion_capacity, sizeof(*pool->completions));
    if (!pool->threads || !pool->tasks || !pool->completions) goto pool_fail;
    pool->worker_count = workers;
    pool->capacity = queue_capacity;
    if (pthread_mutex_init(&pool->mutex, NULL) != 0) goto pool_fail;
    if (pthread_cond_init(&pool->ready, NULL) != 0) {
        pthread_mutex_destroy(&pool->mutex);
        goto pool_fail;
    }
    uint64_t created = 0;
    for (; created < workers; ++created)
        if (pthread_create(&pool->threads[created], NULL,
                           seen_blocking_worker, pool) != 0) break;
    if (created != workers) {
        pthread_mutex_lock(&pool->mutex);
        pool->shutting_down = 1;
        pthread_cond_broadcast(&pool->ready);
        pthread_mutex_unlock(&pool->mutex);
        for (uint64_t i = 0; i < created; ++i) pthread_join(pool->threads[i], NULL);
        pthread_cond_destroy(&pool->ready);
        pthread_mutex_destroy(&pool->mutex);
        goto pool_fail;
    }
    atomic_fetch_add_explicit(&seen_blocking_workers, workers,
                              memory_order_relaxed);
    *out_handle = (uint64_t)(uintptr_t)pool;
    return SEEN_REACTOR_OK;
pool_fail:
    free(pool->completions);
    free(pool->tasks);
    free(pool->threads);
    free(pool);
    return SEEN_REACTOR_UNAVAILABLE;
#endif
}

int32_t seen_blocking_pool_submit(uint64_t handle, uint64_t token,
                                  int64_t function_address, int64_t argument) {
#if defined(_WIN32)
    (void)handle; (void)token; (void)function_address; (void)argument;
    return SEEN_REACTOR_UNSUPPORTED;
#else
    SeenBlockingPool *pool = (SeenBlockingPool *)(uintptr_t)handle;
    if (!pool || token == 0 || function_address == 0) return SEEN_REACTOR_INVALID;
    pthread_mutex_lock(&pool->mutex);
    if (pool->shutting_down) {
        pthread_mutex_unlock(&pool->mutex);
        return SEEN_REACTOR_BUSY;
    }
    if (pool->task_count >= pool->capacity ||
        pool->task_count + pool->active_count + pool->completion_count >=
            pool->completion_capacity) {
        pthread_mutex_unlock(&pool->mutex);
        return SEEN_REACTOR_LIMIT;
    }
    uint64_t slot = (pool->task_head + pool->task_count) % pool->capacity;
    pool->tasks[slot].token = token;
    pool->tasks[slot].function = (SeenBlockingFunction)(uintptr_t)function_address;
    pool->tasks[slot].argument = argument;
    pool->task_count++;
    pthread_cond_signal(&pool->ready);
    pthread_mutex_unlock(&pool->mutex);
    return SEEN_REACTOR_OK;
#endif
}

int32_t seen_blocking_pool_cancel(uint64_t handle, uint64_t token) {
#if defined(_WIN32)
    (void)handle; (void)token;
    return SEEN_REACTOR_UNSUPPORTED;
#else
    SeenBlockingPool *pool = (SeenBlockingPool *)(uintptr_t)handle;
    if (!pool || token == 0) return SEEN_REACTOR_INVALID;
    pthread_mutex_lock(&pool->mutex);
    uint64_t found = pool->capacity;
    for (uint64_t i = 0; i < pool->task_count; ++i) {
        uint64_t slot = (pool->task_head + i) % pool->capacity;
        if (pool->tasks[slot].token == token) { found = i; break; }
    }
    if (found == pool->capacity) {
        pthread_mutex_unlock(&pool->mutex);
        return SEEN_REACTOR_NOT_FOUND;
    }
    for (uint64_t i = found; i + 1 < pool->task_count; ++i) {
        uint64_t destination = (pool->task_head + i) % pool->capacity;
        uint64_t source = (pool->task_head + i + 1) % pool->capacity;
        pool->tasks[destination] = pool->tasks[source];
    }
    pool->task_count--;
    pthread_mutex_unlock(&pool->mutex);
    return SEEN_REACTOR_OK;
#endif
}

int32_t seen_blocking_pool_completion(uint64_t handle, uint64_t *out_token,
                                      int64_t *out_result) {
#if defined(_WIN32)
    (void)handle; (void)out_token; (void)out_result;
    return SEEN_REACTOR_UNSUPPORTED;
#else
    SeenBlockingPool *pool = (SeenBlockingPool *)(uintptr_t)handle;
    if (!pool || !out_token || !out_result) return SEEN_REACTOR_INVALID;
    pthread_mutex_lock(&pool->mutex);
    if (pool->completion_count == 0) {
        pthread_mutex_unlock(&pool->mutex);
        return SEEN_REACTOR_NOT_FOUND;
    }
    *out_token = pool->completions[pool->completion_head].token;
    *out_result = pool->completions[pool->completion_head].result;
    pool->completion_head = (pool->completion_head + 1) %
        pool->completion_capacity;
    pool->completion_count--;
    pthread_mutex_unlock(&pool->mutex);
    return SEEN_REACTOR_OK;
#endif
}

int32_t seen_blocking_pool_close(uint64_t *handle) {
    if (!handle) return SEEN_REACTOR_INVALID;
    if (*handle == 0) return SEEN_REACTOR_OK;
#if defined(_WIN32)
    return SEEN_REACTOR_UNSUPPORTED;
#else
    SeenBlockingPool *pool = (SeenBlockingPool *)(uintptr_t)*handle;
    pthread_mutex_lock(&pool->mutex);
    pool->shutting_down = 1;
    pthread_cond_broadcast(&pool->ready);
    pthread_mutex_unlock(&pool->mutex);
    for (uint64_t i = 0; i < pool->worker_count; ++i)
        pthread_join(pool->threads[i], NULL);
    atomic_fetch_sub_explicit(&seen_blocking_workers, pool->worker_count,
                              memory_order_relaxed);
    pthread_cond_destroy(&pool->ready);
    pthread_mutex_destroy(&pool->mutex);
    free(pool->completions);
    free(pool->tasks);
    free(pool->threads);
    free(pool);
    *handle = 0;
    return SEEN_REACTOR_OK;
#endif
}

uint64_t seen_blocking_pool_live_workers(void) {
    return atomic_load_explicit(&seen_blocking_workers, memory_order_relaxed);
}
