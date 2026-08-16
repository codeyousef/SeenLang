#define _GNU_SOURCE
#include <dlfcn.h>
#include <errno.h>
#include <limits.h>
#include <pthread.h>
#include <signal.h>
#include <spawn.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>

#define SERIALIZER_TARGET_ENV "SEEN_FORK_SERIALIZER_TARGET"
#define SERIALIZER_ROOT_PID_ENV "SEEN_FORK_SERIALIZER_ROOT_PID"
#ifndef SERIALIZER_STATUS_CAPACITY
#define SERIALIZER_STATUS_CAPACITY 4096
#endif

typedef pid_t (*fork_fn)(void);
typedef pid_t (*waitpid_fn)(pid_t, int *, int);
typedef int (*posix_spawn_fn)(pid_t *, const char *,
                             const posix_spawn_file_actions_t *,
                             const posix_spawnattr_t *, char *const[],
                             char *const[]);
typedef FILE *(*popen_fn)(const char *, const char *);
typedef int (*pclose_fn)(FILE *);

enum active_child_kind {
    ACTIVE_CHILD_NONE = 0,
    ACTIVE_CHILD_PID,
    ACTIVE_CHILD_POPEN,
    ACTIVE_CHILD_POISONED,
};

struct saved_wait_status {
    pid_t pid;
    int status;
};

static struct saved_wait_status saved_statuses[SERIALIZER_STATUS_CAPACITY];
static size_t saved_status_count = 0;
static pthread_mutex_t spawn_lock = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t spawn_condition = PTHREAD_COND_INITIALIZER;
static enum active_child_kind active_child = ACTIVE_CHILD_NONE;
static pid_t active_pid = -1;
static FILE *active_popen_stream = NULL;
static pthread_t active_popen_owner;
static int active_popen_owner_valid = 0;
static int serializer_poison_status = ECHILD;
static pid_t serializer_root_pid = -1;
static int serializer_target_active = 0;
static int serializer_initialization_status = ENOSYS;
static __thread int descendant_passthrough = 0;
static __thread int inside_popen = 0;

static fork_fn real_fork = NULL;
static waitpid_fn real_waitpid = NULL;
static posix_spawn_fn real_posix_spawn = NULL;
static posix_spawn_fn real_posix_spawnp = NULL;
static popen_fn real_popen = NULL;
static pclose_fn real_pclose = NULL;

static int parse_positive_pid(const char *text, pid_t *value) {
    char *end = NULL;
    long parsed;

    if (text == NULL || *text == '\0') return 0;
    errno = 0;
    parsed = strtol(text, &end, 10);
    if (errno != 0 || end == text || *end != '\0' || parsed <= 0 ||
        parsed > INT_MAX) {
        return 0;
    }
    *value = (pid_t)parsed;
    return 1;
}

static int target_matches_current_executable(const char *target) {
    char canonical_target[PATH_MAX];
    char current_executable[PATH_MAX];
    struct stat target_status;
    struct stat current_status;
    ssize_t length;

    if (target == NULL || realpath(target, canonical_target) == NULL) return 0;
    length = readlink("/proc/self/exe", current_executable,
                      sizeof(current_executable) - 1);
    if (length <= 0 || (size_t)length >= sizeof(current_executable) - 1) {
        return 0;
    }
    current_executable[length] = '\0';
    if (strcmp(canonical_target, current_executable) != 0 ||
        stat(canonical_target, &target_status) != 0 ||
        stat("/proc/self/exe", &current_status) != 0) {
        return 0;
    }
    return target_status.st_dev == current_status.st_dev &&
           target_status.st_ino == current_status.st_ino;
}

static void serializer_atfork_child(void) {
    /* Never touch mutex state copied from another process/TID in the child. */
    descendant_passthrough = 1;
    inside_popen = 0;
}

__attribute__((constructor)) static void serializer_initialize(void) {
    const char *target = getenv(SERIALIZER_TARGET_ENV);
    const char *root_text = getenv(SERIALIZER_ROOT_PID_ENV);
    pid_t inherited_root = -1;
    char root_buffer[32];
    int target_matches = target_matches_current_executable(target);

    if (target_matches) {
        if (root_text == NULL || *root_text == '\0') {
            serializer_root_pid = getpid();
            serializer_target_active = 1;
            snprintf(root_buffer, sizeof(root_buffer), "%ld",
                     (long)serializer_root_pid);
            if (setenv(SERIALIZER_ROOT_PID_ENV, root_buffer, 1) != 0) {
                serializer_initialization_status =
                    errno != 0 ? errno : EINVAL;
            }
        } else if (parse_positive_pid(root_text, &inherited_root)) {
            serializer_root_pid = inherited_root;
            serializer_target_active = serializer_root_pid == getpid();
        } else {
            /* A malformed root identity on the intended target fails closed. */
            serializer_root_pid = getpid();
            serializer_target_active = 1;
            serializer_initialization_status = EINVAL;
        }
    } else if (target != NULL && *target != '\0' &&
               (root_text == NULL || *root_text == '\0')) {
        /* An intended initial target that cannot be proven must not run open. */
        serializer_root_pid = getpid();
        serializer_target_active = 1;
        serializer_initialization_status = EACCES;
    }

    real_fork = (fork_fn)dlsym(RTLD_NEXT, "fork");
    real_waitpid = (waitpid_fn)dlsym(RTLD_NEXT, "waitpid");
    real_posix_spawn = (posix_spawn_fn)dlsym(RTLD_NEXT, "posix_spawn");
    real_posix_spawnp = (posix_spawn_fn)dlsym(RTLD_NEXT, "posix_spawnp");
    real_popen = (popen_fn)dlsym(RTLD_NEXT, "popen");
    real_pclose = (pclose_fn)dlsym(RTLD_NEXT, "pclose");
    if (real_fork == NULL || real_waitpid == NULL || real_posix_spawn == NULL ||
        real_posix_spawnp == NULL || real_popen == NULL || real_pclose == NULL) {
        serializer_initialization_status = ENOSYS;
        return;
    }
    if (serializer_target_active &&
        serializer_initialization_status != ENOSYS) {
        /* Preserve a setenv/root-identity failure recorded above. */
        return;
    }
    serializer_initialization_status = pthread_atfork(NULL, NULL,
                                                      serializer_atfork_child);
}

static int serializer_is_active(void) {
    return serializer_target_active && !descendant_passthrough &&
           serializer_root_pid == getpid();
}

static int cached_pid_exists_locked(pid_t pid) {
    size_t index;
    for (index = 0; index < saved_status_count; ++index) {
        if (saved_statuses[index].pid == pid) return 1;
    }
    return 0;
}

static int save_wait_status_locked(pid_t pid, int status) {
    if (saved_status_count >=
        sizeof(saved_statuses) / sizeof(saved_statuses[0])) {
        return EAGAIN;
    }
    if (cached_pid_exists_locked(pid)) return EEXIST;
    saved_statuses[saved_status_count].pid = pid;
    saved_statuses[saved_status_count].status = status;
    saved_status_count += 1;
    return 0;
}

static pid_t take_saved_wait_status_locked(pid_t pid, int *status) {
    size_t index;
    for (index = 0; index < saved_status_count; ++index) {
        if (pid == saved_statuses[index].pid || pid == -1) {
            pid_t matched_pid = saved_statuses[index].pid;
            if (status != NULL) *status = saved_statuses[index].status;
            saved_status_count -= 1;
            saved_statuses[index] = saved_statuses[saved_status_count];
            return matched_pid;
        }
    }
    return 0;
}

static int reap_active_pid_locked(void) {
    int status = 0;
    pid_t result;
    int saved_errno;

    if (active_child != ACTIVE_CHILD_PID || active_pid <= 0) return 0;
    if (saved_status_count >=
            sizeof(saved_statuses) / sizeof(saved_statuses[0]) ||
        cached_pid_exists_locked(active_pid)) {
        return EAGAIN;
    }
    do {
        result = real_waitpid(active_pid, &status, 0);
    } while (result < 0 && errno == EINTR);
    if (result != active_pid) return errno != 0 ? errno : ECHILD;
    saved_errno = save_wait_status_locked(active_pid, status);
    if (saved_errno != 0) return saved_errno;
    active_child = ACTIVE_CHILD_NONE;
    active_pid = -1;
    return 0;
}

static int reject_reused_pid_locked(pid_t pid) {
    int ignored_status = 0;
    pid_t wait_result;

    if (!cached_pid_exists_locked(pid)) return 0;
    while (kill(pid, SIGKILL) != 0 && errno == EINTR) {
    }
    do {
        wait_result = real_waitpid(pid, &ignored_status, 0);
    } while (wait_result < 0 && errno == EINTR);
    active_child = ACTIVE_CHILD_POISONED;
    serializer_poison_status = EEXIST;
    return EEXIST;
}

/* Returns with spawn_lock held on success. */
static int prepare_top_level_spawn(void) {
    int result = pthread_mutex_lock(&spawn_lock);
    if (result != 0) return result;
    while (active_child == ACTIVE_CHILD_POPEN) {
        if (active_popen_owner_valid &&
            pthread_equal(active_popen_owner, pthread_self())) {
            pthread_mutex_unlock(&spawn_lock);
            return EDEADLK;
        }
        result = pthread_cond_wait(&spawn_condition, &spawn_lock);
        if (result != 0) {
            pthread_mutex_unlock(&spawn_lock);
            return result;
        }
    }
    if (active_child == ACTIVE_CHILD_POISONED) {
        result = serializer_poison_status;
        pthread_mutex_unlock(&spawn_lock);
        return result;
    }
    result = reap_active_pid_locked();
    if (result != 0) {
        pthread_mutex_unlock(&spawn_lock);
        return result;
    }
    return 0;
}

pid_t fork(void) {
    pid_t pid;
    int result;

    if (real_fork == NULL) {
        errno = ENOSYS;
        return -1;
    }
    if (!serializer_is_active() || inside_popen) return real_fork();
    if (serializer_initialization_status != 0) {
        errno = serializer_initialization_status;
        return -1;
    }
    result = prepare_top_level_spawn();
    if (result != 0) {
        errno = result;
        return -1;
    }
    pid = real_fork();
    if (pid == 0) return 0;
    if (pid > 0) {
        result = reject_reused_pid_locked(pid);
        if (result != 0) {
            pthread_mutex_unlock(&spawn_lock);
            errno = result;
            return -1;
        }
        active_child = ACTIVE_CHILD_PID;
        active_pid = pid;
    }
    pthread_mutex_unlock(&spawn_lock);
    return pid;
}

static int serialized_posix_spawn(posix_spawn_fn spawn_function, pid_t *pid,
                                  const char *path,
                                  const posix_spawn_file_actions_t *actions,
                                  const posix_spawnattr_t *attributes,
                                  char *const argv[], char *const envp[]) {
    int result;

    if (spawn_function == NULL) return ENOSYS;
    if (!serializer_is_active() || inside_popen) {
        return spawn_function(pid, path, actions, attributes, argv, envp);
    }
    if (serializer_initialization_status != 0) {
        return serializer_initialization_status;
    }
    if (pid == NULL) return EINVAL;
    result = prepare_top_level_spawn();
    if (result != 0) return result;
    result = spawn_function(pid, path, actions, attributes, argv, envp);
    if (result == 0 && *pid > 0) {
        result = reject_reused_pid_locked(*pid);
        if (result == 0) {
            active_child = ACTIVE_CHILD_PID;
            active_pid = *pid;
        }
    } else if (result == 0) {
        active_child = ACTIVE_CHILD_POISONED;
        serializer_poison_status = ECHILD;
        result = ECHILD;
    }
    pthread_mutex_unlock(&spawn_lock);
    return result;
}

int posix_spawn(pid_t *pid, const char *path,
                const posix_spawn_file_actions_t *actions,
                const posix_spawnattr_t *attributes, char *const argv[],
                char *const envp[]) {
    return serialized_posix_spawn(real_posix_spawn, pid, path, actions,
                                  attributes, argv, envp);
}

int posix_spawnp(pid_t *pid, const char *file,
                 const posix_spawn_file_actions_t *actions,
                 const posix_spawnattr_t *attributes, char *const argv[],
                 char *const envp[]) {
    return serialized_posix_spawn(real_posix_spawnp, pid, file, actions,
                                  attributes, argv, envp);
}

FILE *popen(const char *command, const char *type) {
    FILE *stream;
    int result;

    if (real_popen == NULL) {
        errno = ENOSYS;
        return NULL;
    }
    if (!serializer_is_active() || inside_popen) {
        return real_popen(command, type);
    }
    result = prepare_top_level_spawn();
    if (result != 0) {
        errno = result;
        return NULL;
    }
    inside_popen += 1;
    stream = real_popen(command, type);
    inside_popen -= 1;
    if (stream != NULL) {
        active_child = ACTIVE_CHILD_POPEN;
        active_popen_stream = stream;
        active_popen_owner = pthread_self();
        active_popen_owner_valid = 1;
    }
    pthread_mutex_unlock(&spawn_lock);
    return stream;
}

int pclose(FILE *stream) {
    int result;
    int pclose_errno = 0;

    if (real_pclose == NULL) {
        errno = ENOSYS;
        return -1;
    }
    if (!serializer_is_active() || inside_popen) return real_pclose(stream);
    if (pthread_mutex_lock(&spawn_lock) != 0) {
        errno = EAGAIN;
        return -1;
    }
    if (active_child != ACTIVE_CHILD_POPEN ||
        active_popen_stream != stream) {
        pthread_mutex_unlock(&spawn_lock);
        errno = EINVAL;
        return -1;
    }
    inside_popen += 1;
    result = real_pclose(stream);
    pclose_errno = errno;
    inside_popen -= 1;
    if (result == -1) {
        active_child = ACTIVE_CHILD_POISONED;
        serializer_poison_status = pclose_errno != 0 ? pclose_errno : ECHILD;
    } else {
        active_child = ACTIVE_CHILD_NONE;
    }
    active_popen_stream = NULL;
    active_popen_owner_valid = 0;
    pthread_cond_broadcast(&spawn_condition);
    pthread_mutex_unlock(&spawn_lock);
    if (result == -1) errno = pclose_errno;
    return result;
}

pid_t waitpid(pid_t pid, int *status, int options) {
    pid_t result;
    int local_status = 0;
    int *wait_status = status != NULL ? status : &local_status;

    if (real_waitpid == NULL) {
        errno = ENOSYS;
        return -1;
    }
    if (!serializer_is_active() || inside_popen) {
        return real_waitpid(pid, status, options);
    }
    if (pthread_mutex_lock(&spawn_lock) != 0) {
        errno = EAGAIN;
        return -1;
    }
    result = take_saved_wait_status_locked(pid, status);
    if (result != 0) {
        pthread_mutex_unlock(&spawn_lock);
        return result;
    }
    result = real_waitpid(pid, wait_status, options);
    if (result > 0 && active_child == ACTIVE_CHILD_PID &&
        result == active_pid &&
        (WIFEXITED(*wait_status) || WIFSIGNALED(*wait_status))) {
        active_child = ACTIVE_CHILD_NONE;
        active_pid = -1;
    }
    pthread_mutex_unlock(&spawn_lock);
    return result;
}
