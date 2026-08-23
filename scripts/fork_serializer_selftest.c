#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <pthread.h>
#include <signal.h>
#include <spawn.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/file.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>

extern char **environ;

enum {
    WORKER_COUNT = 3,
    LARGE_TRANSFER_BYTES = 262144,
};

struct counter_state {
    int current;
    int maximum;
};

struct worker_args {
    const char *program;
    const char *state_path;
    int mode;
    int result;
    pthread_barrier_t *barrier;
};

static int wait_for_success(pid_t pid);

static int update_counter(const char *path, int delta) {
    int fd = open(path, O_RDWR | O_CREAT, 0600);
    struct counter_state state = {0, 0};
    if (fd < 0 || flock(fd, LOCK_EX) != 0) {
        if (fd >= 0) close(fd);
        return 1;
    }
    ssize_t count = pread(fd, &state, sizeof(state), 0);
    if (count != 0 && count != (ssize_t)sizeof(state)) {
        flock(fd, LOCK_UN);
        close(fd);
        return 1;
    }
    state.current += delta;
    if (state.current < 0) state.current = 0;
    if (state.current > state.maximum) state.maximum = state.current;
    int failed = pwrite(fd, &state, sizeof(state), 0) != (ssize_t)sizeof(state);
    if (fsync(fd) != 0) failed = 1;
    flock(fd, LOCK_UN);
    close(fd);
    return failed;
}

static int child_mode(const char *state_path) {
    if (update_counter(state_path, 1) != 0) return 10;
    usleep(150000);
    if (update_counter(state_path, -1) != 0) return 11;
    return 0;
}

static int initialize_counter(const char *state_path) {
    int fd = open(state_path, O_RDWR | O_CREAT | O_TRUNC, 0600);
    struct counter_state initial = {0, 0};
    int failed = fd < 0;
    if (!failed &&
        write(fd, &initial, sizeof(initial)) != (ssize_t)sizeof(initial)) {
        failed = 1;
    }
    if (fd >= 0) close(fd);
    return failed;
}

static int verify_serial_counter(const char *state_path) {
    int fd = open(state_path, O_RDONLY);
    struct counter_state final = {0, 0};
    if (fd < 0 || read(fd, &final, sizeof(final)) != (ssize_t)sizeof(final)) {
        if (fd >= 0) close(fd);
        return 1;
    }
    close(fd);
    if (final.current != 0 || final.maximum != 1) {
        fprintf(stderr, "serializer self-test concurrency: current=%d maximum=%d\n",
                final.current, final.maximum);
        return 1;
    }
    return 0;
}

static int run_descendant_script(const char *script, const char *state_path,
                                 const char *program) {
    pid_t pid = -1;
    char *const argv[] = {(char *)script, (char *)program,
                          (char *)state_path, NULL};
    if (initialize_counter(state_path) != 0) return 1;
    if (posix_spawn(&pid, script, NULL, NULL, argv, environ) != 0) return 1;
    if (wait_for_success(pid) != 0) return 1;
    return verify_serial_counter(state_path);
}

static int emit_bytes(size_t amount) {
    char buffer[4096];
    memset(buffer, 'x', sizeof(buffer));
    while (amount > 0) {
        size_t chunk = amount < sizeof(buffer) ? amount : sizeof(buffer);
        ssize_t written = write(STDOUT_FILENO, buffer, chunk);
        if (written < 0 && errno == EINTR) continue;
        if (written <= 0) return 12;
        amount -= (size_t)written;
    }
    return 0;
}

static int consume_bytes(size_t expected) {
    char buffer[4096];
    size_t total = 0;
    for (;;) {
        ssize_t count = read(STDIN_FILENO, buffer, sizeof(buffer));
        if (count < 0 && errno == EINTR) continue;
        if (count < 0) return 13;
        if (count == 0) break;
        total += (size_t)count;
    }
    return total == expected ? 0 : 14;
}

static int wait_for_exit_code(pid_t pid, int expected) {
    int status = 0;
    if (waitpid(pid, &status, 0) != pid) return 1;
    return !WIFEXITED(status) || WEXITSTATUS(status) != expected;
}

static int wait_for_success(pid_t pid) {
    return wait_for_exit_code(pid, 0);
}

static int run_fork_worker(const struct worker_args *args) {
    pid_t pid = fork();
    if (pid < 0) return 1;
    if (pid == 0) _exit(child_mode(args->state_path));
    return wait_for_success(pid);
}

static int run_spawn_worker(const struct worker_args *args) {
    pid_t pid = -1;
    char *const argv[] = {(char *)args->program, "--child",
                          (char *)args->state_path, NULL};
    int result = posix_spawn(&pid, args->program, NULL, NULL, argv, environ);
    if (result != 0) return 1;
    return wait_for_success(pid);
}

static int run_spawnp_worker(const struct worker_args *args) {
    pid_t pid = -1;
    char *const argv[] = {(char *)args->program, "--child",
                          (char *)args->state_path, NULL};
    int result = posix_spawnp(&pid, args->program, NULL, NULL, argv, environ);
    if (result != 0) return 1;
    return wait_for_success(pid);
}

static int shell_quote(const char *input, char *output, size_t output_size) {
    size_t used = 0;
    if (output_size < 3) return 1;
    output[used++] = '\'';
    for (const char *cursor = input; *cursor != '\0'; ++cursor) {
        if (*cursor == '\'') {
            const char escaped[] = "'\\''";
            if (used + sizeof(escaped) - 1 >= output_size) return 1;
            memcpy(output + used, escaped, sizeof(escaped) - 1);
            used += sizeof(escaped) - 1;
        } else {
            if (used + 1 >= output_size) return 1;
            output[used++] = *cursor;
        }
    }
    if (used + 2 > output_size) return 1;
    output[used++] = '\'';
    output[used] = '\0';
    return 0;
}

static int run_popen_worker(const struct worker_args *args) {
    char quoted_program[4096];
    char quoted_state[4096];
    char command[8300];
    if (shell_quote(args->program, quoted_program, sizeof(quoted_program)) != 0 ||
        shell_quote(args->state_path, quoted_state, sizeof(quoted_state)) != 0) {
        return 1;
    }
    int length = snprintf(command, sizeof(command), "%s --child %s",
                          quoted_program, quoted_state);
    if (length < 0 || (size_t)length >= sizeof(command)) return 1;
    FILE *stream = popen(command, "r");
    if (stream == NULL) return 1;
    return pclose(stream) != 0;
}

static int run_large_capture(const struct worker_args *args) {
    char quoted_program[4096];
    char command[4200];
    char buffer[4096];
    size_t total = 0;
    if (shell_quote(args->program, quoted_program, sizeof(quoted_program)) != 0) {
        return 1;
    }
    int length = snprintf(command, sizeof(command), "%s --emit-bytes %d",
                          quoted_program, LARGE_TRANSFER_BYTES);
    if (length < 0 || (size_t)length >= sizeof(command)) return 1;
    FILE *stream = popen(command, "r");
    if (stream == NULL) return 1;
    while (!feof(stream)) {
        size_t count = fread(buffer, 1, sizeof(buffer), stream);
        total += count;
        if (count == 0 && ferror(stream)) {
            pclose(stream);
            return 1;
        }
    }
    if (pclose(stream) != 0) return 1;
    return total == LARGE_TRANSFER_BYTES ? 0 : 1;
}

static int run_large_pipeline(const struct worker_args *args) {
    char quoted_program[4096];
    char command[8300];
    if (shell_quote(args->program, quoted_program, sizeof(quoted_program)) != 0) {
        return 1;
    }
    int length = snprintf(command, sizeof(command),
                          "%s --emit-bytes %d | %s --consume-bytes %d",
                          quoted_program, LARGE_TRANSFER_BYTES,
                          quoted_program, LARGE_TRANSFER_BYTES);
    if (length < 0 || (size_t)length >= sizeof(command)) return 1;
    FILE *stream = popen(command, "r");
    if (stream == NULL) return 1;
    return pclose(stream) != 0;
}

static int run_large_command_substitution(const struct worker_args *args) {
    char quoted_program[4096];
    char command[4300];
    if (shell_quote(args->program, quoted_program, sizeof(quoted_program)) != 0) {
        return 1;
    }
    int length = snprintf(command, sizeof(command),
                          "captured=$(%s --emit-bytes %d); "
                          "test \"${#captured}\" -eq %d",
                          quoted_program, LARGE_TRANSFER_BYTES,
                          LARGE_TRANSFER_BYTES);
    if (length < 0 || (size_t)length >= sizeof(command)) return 1;
    FILE *stream = popen(command, "r");
    if (stream == NULL) return 1;
    return pclose(stream) != 0;
}

static int drain_exactly(int fd, size_t expected) {
    char buffer[4096];
    size_t total = 0;
    for (;;) {
        ssize_t count = read(fd, buffer, sizeof(buffer));
        if (count < 0 && errno == EINTR) continue;
        if (count < 0) return 1;
        if (count == 0) break;
        total += (size_t)count;
    }
    return total == expected ? 0 : 1;
}

static int run_fork_pipe_pressure(void) {
    int descriptors[2];
    if (pipe(descriptors) != 0) return 1;
    pid_t pid = fork();
    if (pid < 0) {
        close(descriptors[0]);
        close(descriptors[1]);
        return 1;
    }
    if (pid == 0) {
        close(descriptors[0]);
        if (dup2(descriptors[1], STDOUT_FILENO) < 0) _exit(50);
        close(descriptors[1]);
        _exit(emit_bytes(LARGE_TRANSFER_BYTES));
    }
    close(descriptors[1]);
    int failed = drain_exactly(descriptors[0], LARGE_TRANSFER_BYTES);
    close(descriptors[0]);
    return failed || wait_for_success(pid);
}

static int run_spawn_pipe_pressure(const struct worker_args *args) {
    int descriptors[2];
    pid_t pid = -1;
    posix_spawn_file_actions_t actions;
    char amount[32];
    char *const argv[] = {(char *)args->program, "--emit-bytes", amount, NULL};

    snprintf(amount, sizeof(amount), "%d", LARGE_TRANSFER_BYTES);
    if (pipe(descriptors) != 0) {
        return 1;
    }
    if (posix_spawn_file_actions_init(&actions) != 0) {
        close(descriptors[0]);
        close(descriptors[1]);
        return 1;
    }
    if (posix_spawn_file_actions_addclose(&actions, descriptors[0]) != 0 ||
        posix_spawn_file_actions_adddup2(&actions, descriptors[1],
                                        STDOUT_FILENO) != 0 ||
        posix_spawn_file_actions_addclose(&actions, descriptors[1]) != 0) {
        posix_spawn_file_actions_destroy(&actions);
        close(descriptors[0]);
        close(descriptors[1]);
        return 1;
    }
    int result = posix_spawn(&pid, args->program, &actions, NULL, argv, environ);
    posix_spawn_file_actions_destroy(&actions);
    close(descriptors[1]);
    if (result != 0) {
        close(descriptors[0]);
        return 1;
    }
    int failed = drain_exactly(descriptors[0], LARGE_TRANSFER_BYTES);
    close(descriptors[0]);
    return failed || wait_for_success(pid);
}

static int run_descendant_siblings(void) {
    pid_t first = fork();
    if (first < 0) return 1;
    if (first == 0) _exit(21);
    pid_t second = fork();
    if (second < 0) return 1;
    if (second == 0) _exit(22);
    return wait_for_exit_code(first, 21) || wait_for_exit_code(second, 22);
}

static int run_nested_operations(const struct worker_args *args) {
    if (run_spawn_worker(args) != 0) return 31;
    if (run_spawnp_worker(args) != 0) return 32;
    if (run_popen_worker(args) != 0) return 33;
    if (run_large_capture(args) != 0) return 34;
    if (run_large_pipeline(args) != 0) return 35;
    if (run_large_command_substitution(args) != 0) return 36;
    if (run_descendant_siblings() != 0) return 37;
    return 0;
}

static int run_nested_fork_worker(const struct worker_args *args) {
    pid_t pid = fork();
    if (pid < 0) return 1;
    if (pid == 0) _exit(run_nested_operations(args));
    return wait_for_success(pid);
}

static void *worker_main(void *opaque) {
    struct worker_args *args = opaque;
    int barrier_status = pthread_barrier_wait(args->barrier);
    if (barrier_status != 0 && barrier_status != PTHREAD_BARRIER_SERIAL_THREAD) {
        args->result = 1;
        return NULL;
    }
    if (args->mode == 0) args->result = run_fork_worker(args);
    else if (args->mode == 1) args->result = run_spawn_worker(args);
    else args->result = run_popen_worker(args);
    return NULL;
}

static int test_cached_status_and_two_forks(void) {
    int status = 0;
    pid_t first = fork();
    if (first < 0) return 1;
    if (first == 0) _exit(41);
    pid_t second = fork();
    if (second < 0) return 1;
    if (second == 0) _exit(42);
    if (waitpid(-1, &status, WNOHANG) != first ||
        !WIFEXITED(status) || WEXITSTATUS(status) != 41) {
        return 1;
    }
    if (wait_for_exit_code(second, 42) != 0) return 1;

    pid_t third = fork();
    if (third < 0) return 1;
    if (third == 0) _exit(43);
    pid_t fourth = fork();
    if (fourth < 0) return 1;
    if (fourth == 0) _exit(44);
    if (wait_for_exit_code(fourth, 44) != 0) return 1;
    return wait_for_exit_code(third, 43);
}

static int test_wnohang_retains_active(void) {
    int status = 0;
    pid_t pid = fork();
    if (pid < 0) return 1;
    if (pid == 0) {
        usleep(500000);
        _exit(0);
    }
    if (waitpid(pid, &status, WNOHANG) != 0) return 1;
    return wait_for_success(pid);
}

static int test_stopped_and_signaled_status(void) {
    int status = 0;
    pid_t stopped = fork();
    if (stopped < 0) return 1;
    if (stopped == 0) {
        raise(SIGSTOP);
        _exit(0);
    }
    if (waitpid(stopped, &status, WUNTRACED) != stopped ||
        !WIFSTOPPED(status)) {
        return 1;
    }
    if (kill(stopped, SIGCONT) != 0 || wait_for_success(stopped) != 0) return 1;

    pid_t signaled = fork();
    if (signaled < 0) return 1;
    if (signaled == 0) {
        for (;;) pause();
    }
    if (kill(signaled, SIGTERM) != 0 ||
        waitpid(signaled, &status, 0) != signaled) {
        return 1;
    }
    return !WIFSIGNALED(status) || WTERMSIG(status) != SIGTERM;
}

static int test_cache_capacity_failure(void) {
    int status = 0;
    pid_t first = fork();
    if (first < 0) return 1;
    if (first == 0) _exit(61);
    pid_t second = fork();
    if (second < 0) return 1;
    if (second == 0) _exit(62);
    pid_t third = fork();
    if (third < 0) return 1;
    if (third == 0) _exit(63);

    errno = 0;
    pid_t refused = fork();
    if (refused != -1 || errno != EAGAIN) return 1;
    if (wait_for_exit_code(first, 61) != 0 ||
        wait_for_exit_code(second, 62) != 0 ||
        wait_for_exit_code(third, 63) != 0) {
        return 1;
    }
    return waitpid(-1, &status, WNOHANG) == -1 && errno == ECHILD ? 0 : 1;
}

int main(int argc, char **argv) {
    if (argc == 3 && strcmp(argv[1], "--child") == 0) {
        return child_mode(argv[2]);
    }
    if (argc == 3 && strcmp(argv[1], "--emit-bytes") == 0) {
        return emit_bytes((size_t)strtoull(argv[2], NULL, 10));
    }
    if (argc == 3 && strcmp(argv[1], "--consume-bytes") == 0) {
        return consume_bytes((size_t)strtoull(argv[2], NULL, 10));
    }
    if (argc == 2 && strcmp(argv[1], "--cache-overflow") == 0) {
        return test_cache_capacity_failure();
    }
    if (argc == 4 && strcmp(argv[1], "--descendant-script") == 0) {
        return run_descendant_script(argv[2], argv[3], argv[0]);
    }
    if (argc != 2) return 2;

    const char *state_path = argv[1];
    if (initialize_counter(state_path) != 0) return 3;

    pthread_t threads[WORKER_COUNT];
    pthread_barrier_t barrier;
    struct worker_args args[WORKER_COUNT];
    if (pthread_barrier_init(&barrier, NULL, WORKER_COUNT) != 0) return 4;
    for (int index = 0; index < WORKER_COUNT; ++index) {
        args[index].program = argv[0];
        args[index].state_path = state_path;
        args[index].mode = index;
        args[index].result = 1;
        args[index].barrier = &barrier;
        if (pthread_create(&threads[index], NULL, worker_main, &args[index]) != 0) {
            return 4;
        }
    }
    for (int index = 0; index < WORKER_COUNT; ++index) {
        if (pthread_join(threads[index], NULL) != 0 || args[index].result != 0) {
            return 5;
        }
    }
    pthread_barrier_destroy(&barrier);

    if (run_nested_fork_worker(&args[0]) != 0) return 16;
    if (run_spawnp_worker(&args[0]) != 0) return 22;
    if (run_large_capture(&args[0]) != 0) return 17;
    if (run_large_pipeline(&args[0]) != 0) return 18;
    if (run_large_command_substitution(&args[0]) != 0) return 19;
    if (run_fork_pipe_pressure() != 0) return 20;
    if (run_spawn_pipe_pressure(&args[0]) != 0) return 21;
    if (test_cached_status_and_two_forks() != 0) return 8;
    if (test_wnohang_retains_active() != 0) return 9;
    if (test_stopped_and_signaled_status() != 0) return 15;

    return verify_serial_counter(state_path) == 0 ? 0 : 7;
}
