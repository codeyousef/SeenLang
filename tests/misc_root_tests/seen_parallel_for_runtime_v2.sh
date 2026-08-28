#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ARTIFACT_ROOT="${SEEN_TEST_ARTIFACT_ROOT:-$ROOT_DIR/.seen/agent-tools/tests}"
mkdir -p "$ARTIFACT_ROOT"
TMP_DIR="$(mktemp -d "$ARTIFACT_ROOT/seen-pfor-v2.XXXXXX")"
cleanup() {
    case "$TMP_DIR" in
        "$ARTIFACT_ROOT"/*) rm -rf -- "$TMP_DIR" ;;
        *) echo "refusing to remove unexpected test path: $TMP_DIR" >&2 ;;
    esac
}
trap cleanup EXIT

case "${SEEN_MAIN_VMEM_KB:-}" in
    ''|*[!0-9]*|0)
        echo "SEEN_MAIN_VMEM_KB must be a positive explicit memory cap" >&2
        exit 2
        ;;
esac

cat >"$TMP_DIR/test.c" <<'EOF'
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

typedef int64_t (*Body)(int64_t, void *);
typedef void (*DropError)(int64_t);

int64_t seen_parallel_for_v2(int64_t, int64_t, Body, void *, DropError,
                             int64_t);
void seen_parallel_for(int64_t, int64_t, int64_t, int64_t);
void seen_test_parallel_for_fail_create_after(int64_t);
void seen_test_parallel_for_fail_allocations(int64_t);

typedef struct {
    int counts[64];
    int drops;
} TestEnv;

static TestEnv *active_env;

static int64_t test_body(int64_t index, void *raw_env) {
    TestEnv *env = (TestEnv *)raw_env;
    __atomic_fetch_add(&env->counts[index], 1, __ATOMIC_RELAXED);
    if (index == 3 || index == 17 || index == 31) return 1000 + index;
    return 0;
}

static int64_t success_body(int64_t index, void *raw_env) {
    TestEnv *env = (TestEnv *)raw_env;
    __atomic_fetch_add(&env->counts[index], 1, __ATOMIC_RELAXED);
    return 0;
}

static void drop_error(int64_t error) {
    (void)error;
    __atomic_fetch_add(&active_env->drops, 1, __ATOMIC_RELAXED);
}

static int64_t compat_body(int64_t index) {
    __atomic_fetch_add(&active_env->counts[index], 1, __ATOMIC_RELAXED);
    return index;
}

static void clear_env(TestEnv *env) {
    for (int i = 0; i < 64; ++i) env->counts[i] = 0;
    env->drops = 0;
}

static void require_exactly_once(TestEnv *env, int start, int end,
                                 const char *label) {
    for (int i = 0; i < 64; ++i) {
        int expected = i >= start && i < end ? 1 : 0;
        if (env->counts[i] != expected) {
            fprintf(stderr, "%s: index %d ran %d times (expected %d)\n",
                    label, i, env->counts[i], expected);
            exit(1);
        }
    }
}

static void require_at_most_once(TestEnv *env, const char *label) {
    for (int i = 0; i < 64; ++i) {
        if (env->counts[i] < 0 || env->counts[i] > 1) {
            fprintf(stderr, "%s: index %d ran %d times\n", label, i,
                    env->counts[i]);
            exit(1);
        }
    }
}

int main(void) {
    TestEnv env;
    active_env = &env;

    clear_env(&env);
    int64_t winner = seen_parallel_for_v2(2, 40, test_body, &env,
                                          drop_error, 8);
    require_exactly_once(&env, 2, 40, "normal");
    if (winner != 1003 || env.drops != 2) {
        fprintf(stderr, "normal: winner=%lld drops=%d\n",
                (long long)winner, env.drops);
        return 1;
    }

    clear_env(&env);
    seen_test_parallel_for_fail_create_after(2);
    winner = seen_parallel_for_v2(5, 29, success_body, &env, drop_error, 8);
    seen_test_parallel_for_fail_create_after(-1);
    require_at_most_once(&env, "partial-create");
    if (winner != -7102) return 1;

    clear_env(&env);
    seen_test_parallel_for_fail_allocations(1);
    winner = seen_parallel_for_v2(7, 33, success_body, &env, drop_error, 8);
    seen_test_parallel_for_fail_allocations(0);
    require_at_most_once(&env, "allocation-failure");
    if (winner != -7101) return 1;

    clear_env(&env);
    seen_parallel_for(4, 19, (int64_t)(uintptr_t)compat_body, 4);
    require_exactly_once(&env, 4, 19, "compatibility-wrapper");

    puts("parallel_for runtime v2 tests passed");
    return 0;
}
EOF

(
    ulimit -v "$SEEN_MAIN_VMEM_KB"
    clang -std=c11 -O1 -pthread -DSEEN_TEST_PFOR_FAILURE_INJECTION \
        -I "$ROOT_DIR/seen_runtime" \
        "$TMP_DIR/test.c" "$ROOT_DIR/seen_runtime/seen_runtime.c" \
        -ldl -lm -o "$TMP_DIR/test"
)

"$TMP_DIR/test"
