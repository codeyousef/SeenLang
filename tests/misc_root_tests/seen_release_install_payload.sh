#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
HARD_SCOPE="$ROOT_DIR/scripts/run_in_hard_memory_scope.sh"

if [ "${SEEN_MEMORY_GUARD_IN_SCOPE:-0}" != 1 ]; then
    echo "RESOURCE STOP: installed release payload test requires a hard-memory scope" >&2
    exit 126
fi
"$HARD_SCOPE" --label "Installed release payload read-back" --verify-only -- >/dev/null
[ "${SEEN_LOW_MEMORY:-0}" = 1 ] && [ "${SEEN_JOBS:-0}" = 1 ] &&
    [ "${SEEN_OPT_JOBS:-0}" = 1 ] || {
    echo "RESOURCE STOP: installed release payload test requires serial settings" >&2
    exit 126
}

PAYLOAD_VMEM_KB=8388608
if [ "${SEEN_MAIN_VMEM_KB:?}" -lt "$PAYLOAD_VMEM_KB" ]; then
    PAYLOAD_VMEM_KB="$SEEN_MAIN_VMEM_KB"
fi
ulimit -S -v "$PAYLOAD_VMEM_KB" || {
    echo "RESOURCE STOP: could not apply installed-payload VMEM cap" >&2
    exit 126
}
export SEEN_MAIN_VMEM_KB="$PAYLOAD_VMEM_KB"
export SEEN_MEMORY_LIMIT_BYTES=$((PAYLOAD_VMEM_KB * 1024))

COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
PACKAGE_CLIENT="${SEEN_PACKAGE_CLIENT_BIN:-$ROOT_DIR/tools/seen-pkg/bin/seen-pkg}"
WORK_DIR="$(mktemp -d /tmp/release-install-payload.XXXXXX)"
case "$WORK_DIR" in
    /tmp/release-install-payload.*) ;;
    *) echo "ERROR: unsafe installed-payload work path: $WORK_DIR" >&2; exit 1 ;;
esac
cleanup() {
    chmod -R u+w "$WORK_DIR/prefix" 2>/dev/null || true
    rm -rf -- "$WORK_DIR"
}
trap cleanup EXIT

mkdir -p "$WORK_DIR/prefix/bin" "$WORK_DIR/prefix/lib/seen/std" \
    "$WORK_DIR/prefix/lib/seen/runtime" "$WORK_DIR/source"
cp "$COMPILER" "$WORK_DIR/prefix/bin/seen"
cp "$PACKAGE_CLIENT" "$WORK_DIR/prefix/bin/seen-pkg"
cp "$ROOT_DIR/releases/compatibility-manifest.json" \
    "$WORK_DIR/prefix/bin/compatibility-manifest.json"
cp -r "$ROOT_DIR/seen_std/src/"* "$WORK_DIR/prefix/lib/seen/std/"
while IFS= read -r -d '' runtime_file; do
    case "$runtime_file" in
        *.o|*.sig|*.a) continue ;;
    esac
    runtime_relative=${runtime_file#seen_runtime/}
    mkdir -p "$WORK_DIR/prefix/lib/seen/runtime/$(dirname "$runtime_relative")"
    cp "$ROOT_DIR/$runtime_file" \
        "$WORK_DIR/prefix/lib/seen/runtime/$runtime_relative"
done < <(git -C "$ROOT_DIR" ls-files -z -- seen_runtime)

# Simulate an upgrade from a legacy payload. These files are immutable stale
# inputs and must never be selected by the installed compiler.
printf 'stale installed object\n' > \
    "$WORK_DIR/prefix/lib/seen/runtime/seen_runtime.o"
printf 'stale-signature\n' > \
    "$WORK_DIR/prefix/lib/seen/runtime/seen_runtime.o.sig"
printf 'stale installed region object\n' > \
    "$WORK_DIR/prefix/lib/seen/runtime/seen_region.o"

prefix_digest() {
    find "$WORK_DIR/prefix" -type f -print0 | sort -z | \
        xargs -0 sha256sum | sha256sum | awk '{print $1}'
}
chmod -R a-w "$WORK_DIR/prefix"
PREFIX_DIGEST_BEFORE=$(prefix_digest)

cmp -s "$WORK_DIR/prefix/bin/compatibility-manifest.json" \
    "$ROOT_DIR/releases/compatibility-manifest.json"
test -f "$WORK_DIR/prefix/lib/seen/std/json/strict.seen"
test -f "$WORK_DIR/prefix/lib/seen/std/crypto/sha256.seen"
"$WORK_DIR/prefix/bin/seen-pkg" --expect-version 0.18.0 version >/dev/null
cp "$ROOT_DIR/tests/misc_root_tests/seen_release_payload_api.seen" \
    "$WORK_DIR/source/main.seen"
mkdir -p "$WORK_DIR/source/.seen/agent-tools"

(cd "$WORK_DIR/source" && env -u SEEN_COMPILER_SOURCE_ROOT -u SEEN_PACKAGE_CLIENT \
    SEEN_PROJECT_ROOT="$WORK_DIR/source" \
    SEEN_ARTIFACT_ROOT="$WORK_DIR/source/.seen/agent-tools" \
    "$WORK_DIR/prefix/bin/seen" check main.seen)
for mode in fast release; do
    for temperature in cold warm; do
        output="$WORK_DIR/source/payload-$mode-$temperature"
        compile_flags=(--no-fork --jobs 1 --opt-jobs 1 --target-cpu x86-64)
        if [ "$mode" = fast ]; then
            compile_flags+=(--fast)
        else
            compile_flags+=(--release --lto thin)
        fi
        (cd "$WORK_DIR/source" && \
            env -u SEEN_COMPILER_SOURCE_ROOT -u SEEN_PACKAGE_CLIENT \
            SEEN_PROJECT_ROOT="$WORK_DIR/source" \
            SEEN_ARTIFACT_ROOT="$WORK_DIR/source/.seen/agent-tools" \
            "$WORK_DIR/prefix/bin/seen" compile main.seen "$output" \
            "${compile_flags[@]}")
        "$output"
    done
done

find "$WORK_DIR/source/.seen/agent-tools" -type f \
    -path '*/runtime-objects/*/*.o' \
    -print -quit | grep -q . || {
    echo "FAIL: installed compiler did not create project-local runtime objects" >&2
    exit 1
}

FAIL_BIN="$WORK_DIR/fail-bin"
mkdir -p "$FAIL_BIN"
printf '#!/usr/bin/env bash\nexit 42\n' > "$FAIL_BIN/clang"
chmod +x "$FAIL_BIN/clang"
mkdir -p "$WORK_DIR/compiler-failure"
cp "$ROOT_DIR/tests/misc_root_tests/seen_release_payload_api.seen" \
    "$WORK_DIR/compiler-failure/main.seen"
set +e
compiler_failure_output=$(cd "$WORK_DIR/compiler-failure" && \
    env -u SEEN_COMPILER_SOURCE_ROOT -u SEEN_PACKAGE_CLIENT \
    SEEN_PROJECT_ROOT="$WORK_DIR/compiler-failure" \
    SEEN_ARTIFACT_ROOT="$WORK_DIR/compiler-failure/.seen/agent-tools" \
    PATH="$FAIL_BIN:$PATH" "$WORK_DIR/prefix/bin/seen" compile main.seen \
    failed-output --fast --no-cache --no-fork --jobs 1 --opt-jobs 1 2>&1)
compiler_failure_status=$?
set -e
if [ "$compiler_failure_status" -eq 0 ] || \
    [ -e "$WORK_DIR/compiler-failure/failed-output" ] || \
    grep -Fq 'Build succeeded' <<<"$compiler_failure_output"; then
    echo "FAIL: runtime compiler failure did not fail closed" >&2
    printf '%s\n' "$compiler_failure_output" >&2
    exit 1
fi

mkdir -p "$WORK_DIR/cache-failure"
cp "$ROOT_DIR/tests/misc_root_tests/seen_release_payload_api.seen" \
    "$WORK_DIR/cache-failure/main.seen"
printf 'blocked artifact root\n' > "$WORK_DIR/cache-failure/.seen"
set +e
cache_failure_output=$(cd "$WORK_DIR/cache-failure" && \
    env -u SEEN_COMPILER_SOURCE_ROOT -u SEEN_PACKAGE_CLIENT \
    SEEN_PROJECT_ROOT="$WORK_DIR/cache-failure" \
    SEEN_ARTIFACT_ROOT="$WORK_DIR/cache-failure/.seen/agent-tools" \
    "$WORK_DIR/prefix/bin/seen" compile main.seen failed-output \
    --fast --no-cache --no-fork --jobs 1 --opt-jobs 1 2>&1)
cache_failure_status=$?
set -e
if [ "$cache_failure_status" -eq 0 ] || \
    [ -e "$WORK_DIR/cache-failure/failed-output" ] || \
    grep -Fq 'Build succeeded' <<<"$cache_failure_output"; then
    echo "FAIL: runtime cache failure did not fail closed" >&2
    printf '%s\n' "$cache_failure_output" >&2
    exit 1
fi

PREFIX_DIGEST_AFTER=$(prefix_digest)
[ "$PREFIX_DIGEST_BEFORE" = "$PREFIX_DIGEST_AFTER" ] || {
    echo "FAIL: installed source payload changed during compilation" >&2
    exit 1
}
echo "PASS: installed layout provides a self-contained Seen 0.18.0 payload"
