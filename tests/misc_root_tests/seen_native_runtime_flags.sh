#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-native-runtime-flags
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0" "$@"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
if [ "${1:-}" = --worker ]; then
    [ "$#" -eq 3 ] || exit 2
    case "$2" in "$SEEN_ARTIFACT_ROOT"/native-runtime-flags.*/consumer) ;; *) exit 2 ;; esac
    TMP_DIR="$(mktemp -d "$2/native-runtime-flags.XXXXXX")"
else
    TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/native-runtime-flags.paths with spaces.XXXXXX")"
fi
FIXTURE_ARTIFACT_PREFIX=native-runtime-flags
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
fail() { echo "FAIL: native runtime flags: $*" >&2; exit 1; }

if [ "${1:-}" != --worker ]; then
    [ "$#" -eq 0 ] || fail 'unexpected arguments'
    command -v clang >/dev/null || fail 'Clang is required'
    command -v llvm-profdata >/dev/null || fail 'llvm-profdata is required'
    export FEL748_REAL_PKG_CONFIG
    FEL748_REAL_PKG_CONFIG=$(command -v pkg-config) || fail 'pkg-config is required'
    "$FEL748_REAL_PKG_CONFIG" --exists vulkan || fail 'real Vulkan development package is required'
    "$FEL748_REAL_PKG_CONFIG" --libs vulkan >"$TMP_DIR/vulkan.libs" || fail 'real Vulkan link metadata is required'
    # Always prune copied toolchains, even when small diagnostic logs are kept.
    finish() {
        local status=$?
        set +e
        if [ -d "$TMP_DIR/installed" ]; then chmod -R u+w "$TMP_DIR/installed"; fi
        rm -rf -- "$TMP_DIR/installed" "$TMP_DIR/checkout"
        (exit "$status")
        cleanup
    }
    trap finish EXIT
    python3 "$ROOT_DIR/tests/fixtures/native-runtime-flags/prepare.py" \
        --repo "$ROOT_DIR" --compiler "$COMPILER" --destination "$TMP_DIR"
    for layout in checkout installed; do
        selected="$TMP_DIR/checkout/compiler_seen/target/seen"
        if [ "$layout" = installed ]; then selected="$TMP_DIR/installed/bin/seen"; fi
        COMPILER="$selected" bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$selected" -- \
            bash "$0" --worker "$TMP_DIR/consumer" "$layout"
    done
    echo 'PASS: native runtime relative flags, environment search paths, PGO, and read-only installed consumers'
    exit 0
fi

[ "$#" -eq 3 ] || fail 'worker requires consumer and layout'
CALLER=$2
LAYOUT=$3
[ -d "$CALLER" ] && [ -f "$CALLER/Seen.toml" ] || fail 'consumer is unavailable'
case "$CALLER" in "$SEEN_ARTIFACT_ROOT"/native-runtime-flags.*/consumer) ;; *) fail 'unsafe consumer path' ;; esac
unset SEEN_COMPILER_SOURCE_ROOT SEEN_PACKAGE_CLIENT SEEN_DATA_PATH SEEN_DETERMINISTIC SEEN_DETERMINISTIC_COMPILER_REEXEC
export FEL748_CFLAGS_FILE="$CALLER/controlled.cflags"
# Keep the verified compiler/tool guard authoritative; adapt only pkg-config.
export PATH="${PATH%%:*}:$CALLER/adapter:${PATH#*:}"
run_compiler() {
    bash "$ATTESTED_SEEN" --consumer-context "$CALLER" "$COMPILER" "$@"
}
cd -- "$CALLER"
# Shared compiled fixtures are frozen consumers, including dependency-free ones.
# Materialize their real locks with the selected layout's package client.
for project in "$CALLER" "$CALLER/pgo"; do
    if ! bash "$ATTESTED_SEEN" "$COMPILER" pkg fetch "$project" >"$TMP_DIR/package-fetch.log" 2>&1; then
        cat "$TMP_DIR/package-fetch.log" >&2
        fail "$LAYOUT/package-lock-preparation"
    fi
done
prune_case() {
    local name=$1 profile=$2
    rm -rf -- "$TMP_DIR/$name-$profile" "$TMP_DIR/$name-$profile-ir" \
        "$CALLER/.seen" "$CALLER/.seen_cache" "$CALLER/pgo/.seen" "$CALLER/pgo/.seen_cache"
}
finish_worker() {
    local status=$?
    set +e
    rm -rf -- "$TMP_DIR"/*-fast "$TMP_DIR"/*-release "$TMP_DIR"/*-ir \
        "$CALLER/.seen" "$CALLER/.seen_cache" "$CALLER/pgo/.seen" "$CALLER/pgo/.seen_cache" \
        "$CALLER/pgo-profiles"
    (exit "$status")
    cleanup
}
trap finish_worker EXIT
run_native_case() {
    local name=$1
    unset CPATH C_INCLUDE_PATH CPLUS_INCLUDE_PATH OBJC_INCLUDE_PATH
    export CPATH="$CALLER/environment first"
    export FEL748_CFLAGS_FILE="$CALLER/controlled.cflags"
    case "$name" in
        pkg-config) ;;
        pkg-config-joined-include) export FEL748_CFLAGS_FILE="$CALLER/joined-include.cflags" ;;
        pkg-config-joined-imacros) export FEL748_CFLAGS_FILE="$CALLER/joined-imacros.cflags" ;;
        forced-search-include|forced-search-include-joined|forced-search-imacros|forced-search-imacros-joined|isystem-after|isystem-after-joined|framework-sysroot|framework-sysroot-joined|framework-sysroot-absolute|framework-sysroot-absolute-joined)
            export FEL748_CFLAGS_FILE="$CALLER/$name.cflags" ;;
        forced-search-include-env)
            export FEL748_CFLAGS_FILE="$CALLER/$name.cflags"
            export CPATH='forced headers:environment first' ;;
        forced-search-imacros-env)
            export FEL748_CFLAGS_FILE="$CALLER/$name.cflags"
            export C_INCLUDE_PATH='macro headers' ;;
        env-cpath-relative) export CPATH='environment first:environment second'; export C_INCLUDE_PATH='environment second' ;;
        env-cinclude-relative) unset CPATH; export C_INCLUDE_PATH='environment first:environment second' ;;
        env-cpath-empty) export CPATH='' ;;
        env-cinclude-empty) unset CPATH; export C_INCLUDE_PATH='' ;;
        env-cpath-empty-first) export CPATH=':environment second' ;;
        env-cpath-empty-middle) export CPATH='missing::environment second' ;;
        env-cpath-empty-last) export CPATH='missing:' ;;
        env-cinclude-empty-first) unset CPATH; export C_INCLUDE_PATH=':environment second' ;;
        env-cinclude-empty-middle) unset CPATH; export C_INCLUDE_PATH='missing::environment second' ;;
        env-cinclude-empty-last) unset CPATH; export C_INCLUDE_PATH='missing:' ;;
        cplus-ignored) export CPLUS_INCLUDE_PATH='cxx only::missing' ;;
        *) fail "unknown native case: $name" ;;
    esac
    # Every case is cold: inherited include environments must reach real Clang,
    # not succeed by reusing a runtime object built under another environment.
    rm -rf -- "$CALLER/.seen" "$CALLER/.seen_cache"
    case "$name" in
        env-cpath-empty|env-cinclude-empty)
            # Clang ignores an entirely empty variable; only an empty element
            # in a nonempty list adds cwd. Do not invent that search directory.
            local status=0
            compile_fixture fast "$name" "$CALLER/main.seen" || status=$?
            [ "$status" -eq 1 ] || fail "$LAYOUT/$name/empty-variable-created-search-path"
            [ ! -e "$TMP_DIR/$name-fast" ] || fail "$LAYOUT/$name/published-output"
            ;;
        *) compile_case fast "$name" "$CALLER/main.seen" || fail "$LAYOUT/$name" ;;
    esac
    prune_case "$name" fast
    printf 'PASS: %s/%s\n' "$LAYOUT" "$name"
}
run_pgo_case() {
    unset CPATH C_INCLUDE_PATH CPLUS_INCLUDE_PATH OBJC_INCLUDE_PATH
    # The CLI requires canonical relative profile names. The caller/root paths
    # still contain spaces, exercising the resolved native Clang path quoting.
    mkdir -p -- "$CALLER/pgo-profiles"
    local raw="$CALLER/pgo-profiles/default.profraw"
    local profile='pgo-profiles/default.profdata'
    FIXTURE_COMPILE_FLAGS=(--pgo-generate)
    compile_fixture release pgo-generate "$CALLER/pgo/main.seen" || fail "$LAYOUT/pgo-generate"
    LLVM_PROFILE_FILE="$raw" run_fixture release pgo-generate || fail "$LAYOUT/pgo-training"
    llvm-profdata merge -sparse --num-threads=1 "$raw" -o "$CALLER/$profile" >"$TMP_DIR/pgo-merge.log" 2>&1 || fail "$LAYOUT/pgo-merge"
    llvm-profdata show --all-functions --counts "$CALLER/$profile" >"$TMP_DIR/pgo-counters.log" || fail "$LAYOUT/pgo-profile"
    prune_case pgo-generate release
    FIXTURE_COMPILE_FLAGS=(--pgo-use "$profile")
    compile_case release pgo-use "$CALLER/pgo/main.seen" || fail "$LAYOUT/relative-pgo-use"
    prune_case pgo-use release
    # A successful metadata echo is insufficient: corrupt the consumed file and
    # require the actual compiler/Clang pipeline to reject it without an output.
    printf 'not an LLVM profile\n' >"$CALLER/$profile"
    if compile_fixture release pgo-corrupt "$CALLER/pgo/main.seen"; then
        fail "$LAYOUT/corrupt-relative-profile-accepted"
    fi
    [ ! -e "$TMP_DIR/pgo-corrupt-release" ] || fail "$LAYOUT/corrupt-profile-published-output"
    prune_case pgo-corrupt release
    rm -rf -- "$CALLER/pgo-profiles"
    FIXTURE_COMPILE_FLAGS=()
    printf 'PASS: %s/relative-pgo-consumed-and-corruption-rejected\n' "$LAYOUT"
}
selected_case=${SEEN_NATIVE_INTEROP_CASE:-all}
if [ "$selected_case" = all ]; then
    for name in pkg-config pkg-config-joined-include pkg-config-joined-imacros \
        forced-search-include forced-search-include-joined \
        forced-search-imacros forced-search-imacros-joined \
        forced-search-include-env forced-search-imacros-env \
        isystem-after isystem-after-joined framework-sysroot framework-sysroot-joined \
        framework-sysroot-absolute framework-sysroot-absolute-joined \
        env-cpath-relative env-cinclude-relative env-cpath-empty env-cinclude-empty \
        env-cpath-empty-first env-cpath-empty-middle env-cpath-empty-last \
        env-cinclude-empty-first env-cinclude-empty-middle env-cinclude-empty-last cplus-ignored; do
        run_native_case "$name"
    done
    run_pgo_case
elif [ "$selected_case" = pgo ]; then
    run_pgo_case
else
    run_native_case "$selected_case"
fi
