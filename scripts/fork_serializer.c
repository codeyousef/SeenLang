#define _GNU_SOURCE
#include <dlfcn.h>
#include <errno.h>
#include <stddef.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>

typedef pid_t (*fork_fn)(void);
typedef pid_t (*waitpid_fn)(pid_t, int *, int);

struct saved_wait_status {
    pid_t pid;
    int status;
};

static struct saved_wait_status saved_statuses[4096];
static size_t saved_status_count = 0;

static fork_fn real_fork_fn(void) {
    static fork_fn fn = NULL;
    if (fn == NULL) {
        fn = (fork_fn)dlsym(RTLD_NEXT, "fork");
    }
    return fn;
}

static waitpid_fn real_waitpid_fn(void) {
    static waitpid_fn fn = NULL;
    if (fn == NULL) {
        fn = (waitpid_fn)dlsym(RTLD_NEXT, "waitpid");
    }
    return fn;
}

static void save_wait_status(pid_t pid, int status) {
    if (saved_status_count >= sizeof(saved_statuses) / sizeof(saved_statuses[0])) {
        return;
    }
    saved_statuses[saved_status_count].pid = pid;
    saved_statuses[saved_status_count].status = status;
    saved_status_count += 1;
}

static pid_t take_saved_wait_status(pid_t pid, int *status) {
    size_t i = 0;
    while (i < saved_status_count) {
        int matches = 0;
        if (pid == saved_statuses[i].pid || pid == -1) {
            matches = 1;
        }
        if (matches) {
            pid_t matched_pid = saved_statuses[i].pid;
            if (status != NULL) {
                *status = saved_statuses[i].status;
            }
            saved_status_count -= 1;
            saved_statuses[i] = saved_statuses[saved_status_count];
            return matched_pid;
        }
        i += 1;
    }
    return 0;
}

pid_t fork(void) {
    fork_fn real_fork = real_fork_fn();
    if (real_fork == NULL) {
        errno = ENOSYS;
        return -1;
    }

    pid_t pid = real_fork();
    if (pid > 0) {
        int status = 0;
        waitpid_fn real_waitpid = real_waitpid_fn();
        if (real_waitpid == NULL) {
            return pid;
        }
        pid_t waited = real_waitpid(pid, &status, 0);
        if (waited == pid) {
            save_wait_status(pid, status);
        }
    }
    return pid;
}

pid_t waitpid(pid_t pid, int *status, int options) {
    if (options == 0) {
        pid_t saved_pid = take_saved_wait_status(pid, status);
        if (saved_pid != 0) {
            return saved_pid;
        }
    }

    waitpid_fn real_waitpid = real_waitpid_fn();
    if (real_waitpid == NULL) {
        errno = ENOSYS;
        return -1;
    }
    return real_waitpid(pid, status, options);
}
