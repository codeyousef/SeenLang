#if defined(__linux__) && !defined(_GNU_SOURCE)
#define _GNU_SOURCE
#endif
#include "executor_process_abi.h"
#if defined(__linux__) && defined(__x86_64__)
#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <signal.h>
#include <spawn.h>
#include <stddef.h>
#include <stdatomic.h>
#include <sys/syscall.h>
#include <sys/wait.h>
#include <unistd.h>

static _Atomic uint64_t seen_async_process_children;
static int32_t seen_async_process_errno(int error) {
    if (error == EINVAL || error == EBADF || error == ENOENT || error == ENOEXEC) return 1;
    if (error == EAGAIN || error == ENOMEM || error == EMFILE || error == ENFILE) return 2;
    if (error == ENOSYS || error == EOPNOTSUPP) return 4;
    if (error == EINTR) return 5;
    return 3;
}
static int seen_async_process_valid_fd(int64_t descriptor) { return descriptor >= 0 && descriptor <= INT_MAX; }
int32_t seen_async_process_pipe(int32_t parent_write, int64_t *read_fd, int64_t *write_fd) {
    if (!read_fd || !write_fd || *read_fd != -1 || *write_fd != -1 ||
        (parent_write != 0 && parent_write != 1)) return 1;
    int descriptors[2];
    if (pipe2(descriptors, O_CLOEXEC) != 0) return seen_async_process_errno(errno);
    for (int i = 0; i < 2; i++) {
        if (descriptors[i] < 3) {
            int duplicate = fcntl(descriptors[i], F_DUPFD_CLOEXEC, 3);
            if (duplicate < 0) { int error = errno; close(descriptors[0]); close(descriptors[1]); return seen_async_process_errno(error); }
            close(descriptors[i]); descriptors[i] = duplicate;
        }
    }
    int endpoint = parent_write ? 1 : 0;
    int flags = fcntl(descriptors[endpoint], F_GETFL);
    if (flags < 0 || fcntl(descriptors[endpoint], F_SETFL, flags | O_NONBLOCK) != 0) {
        int error = errno; close(descriptors[0]); close(descriptors[1]); return seen_async_process_errno(error);
    }
    *read_fd = descriptors[0]; *write_fd = descriptors[1]; return 0;
}
int32_t seen_async_process_spawn(const void *program, const void *arguments, const void *environment,
    int64_t input, int64_t output, int64_t error, int64_t *pid) {
    if (!program || !arguments || !environment || !pid || *pid != -1 ||
        !seen_async_process_valid_fd(input) || !seen_async_process_valid_fd(output) ||
        !seen_async_process_valid_fd(error)) return 1;
    struct sigaction child_action;
    if (sigaction(SIGCHLD, NULL, &child_action) != 0) return seen_async_process_errno(errno);
    if (child_action.sa_handler == SIG_IGN || (child_action.sa_flags & SA_NOCLDWAIT)) return 4;
    posix_spawn_file_actions_t actions; posix_spawnattr_t attributes;
    int status = posix_spawn_file_actions_init(&actions);
    if (status) return seen_async_process_errno(status);
    status = posix_spawnattr_init(&attributes);
    if (status) { posix_spawn_file_actions_destroy(&actions); return seen_async_process_errno(status); }
    sigset_t empty, defaults; sigemptyset(&empty); sigemptyset(&defaults); sigaddset(&defaults, SIGPIPE);
    status = posix_spawnattr_setsigmask(&attributes, &empty);
    if (!status) status = posix_spawnattr_setsigdefault(&attributes, &defaults);
    if (!status) status = posix_spawnattr_setflags(&attributes, POSIX_SPAWN_SETSIGMASK | POSIX_SPAWN_SETSIGDEF);
    if (!status) status = posix_spawn_file_actions_adddup2(&actions, (int)input, STDIN_FILENO);
    if (!status) status = posix_spawn_file_actions_adddup2(&actions, (int)output, STDOUT_FILENO);
    if (!status) status = posix_spawn_file_actions_adddup2(&actions, (int)error, STDERR_FILENO);
    if (!status) status = posix_spawn_file_actions_addclosefrom_np(&actions, 3);
    pid_t child = -1;
    if (!status) status = posix_spawn(&child, program, &actions, &attributes,
        (char *const *)arguments, (char *const *)environment);
    posix_spawnattr_destroy(&attributes); posix_spawn_file_actions_destroy(&actions);
    if (status) return seen_async_process_errno(status);
    *pid = child; atomic_fetch_add_explicit(&seen_async_process_children, 1, memory_order_relaxed); return 0;
}
int32_t seen_async_process_pidfd(int64_t pid, int64_t *descriptor) {
    if (pid <= 0 || pid > INT_MAX || !descriptor || *descriptor != -1) return 1;
    long result = syscall(SYS_pidfd_open, (pid_t)pid, 0U);
    if (result < 0) return seen_async_process_errno(errno);
    *descriptor = result; return 0;
}
int32_t seen_async_process_signal(int64_t descriptor, int32_t signal) {
    if (!seen_async_process_valid_fd(descriptor) || (signal != SIGTERM && signal != SIGKILL)) return 1;
    if (syscall(SYS_pidfd_send_signal, (int)descriptor, signal, NULL, 0U) == 0 || errno == ESRCH) return 0;
    return seen_async_process_errno(errno);
}
int32_t seen_async_process_kill_unreaped(int64_t pid) {
    if (pid <= 0 || pid > INT_MAX) return 1;
    // The Seen caller must still own this unreaped child; waitpid ownership
    // prevents PID reuse. Normal cancellation uses pidfd_send_signal instead.
    if (kill((pid_t)pid, SIGKILL) == 0 || errno == ESRCH) return 0;
    return seen_async_process_errno(errno);
}
int32_t seen_async_process_wait(int64_t pid, int32_t blocking, int32_t *done,
    int32_t *exit_code, int32_t *signal) {
    if (pid <= 0 || pid > INT_MAX || (blocking != 0 && blocking != 1) || !done || !exit_code || !signal) return 1;
    *done = 0; *exit_code = 0; *signal = 0; int status = 0;
    pid_t result = waitpid((pid_t)pid, &status, blocking ? 0 : WNOHANG);
    if (result < 0) return seen_async_process_errno(errno);
    if (result == 0) return 0;
    if (!WIFEXITED(status) && !WIFSIGNALED(status)) return 3;
    *done = 1;
    atomic_fetch_sub_explicit(&seen_async_process_children, 1, memory_order_relaxed);
    if (WIFEXITED(status)) *exit_code = WEXITSTATUS(status);
    else *signal = WTERMSIG(status);
    return 0;
}
uint64_t seen_async_process_live_children(void) {
    return atomic_load_explicit(&seen_async_process_children, memory_order_relaxed);
}

#else
int32_t seen_async_process_pipe(int32_t p,int64_t*r,int64_t*w) {(void)p;(void)r;(void)w;return 4;}
int32_t seen_async_process_spawn(const void*p,const void*a,const void*e,int64_t i,int64_t o,int64_t r,int64_t*c) {(void)p;(void)a;(void)e;(void)i;(void)o;(void)r;(void)c;return 4;}
int32_t seen_async_process_pidfd(int64_t p,int64_t*d) {(void)p;(void)d;return 4;}
int32_t seen_async_process_signal(int64_t d,int32_t s) {(void)d;(void)s;return 4;}
int32_t seen_async_process_kill_unreaped(int64_t p) {(void)p;return 4;}
int32_t seen_async_process_wait(int64_t p,int32_t b,int32_t*d,int32_t*e,int32_t*s) {(void)p;(void)b;(void)d;(void)e;(void)s;return 4;}
uint64_t seen_async_process_live_children(void) {return 0;}
#endif
