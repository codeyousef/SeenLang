#!/usr/bin/env bash
# Compile host SDK canaries and the conditional frozen loader bridge only after
# the platform runner verifies aggregate limits and private project scratch.
set -euo pipefail
ROOT_DIR=$(cd -P -- "${BASH_SOURCE[0]%/*}/.." && pwd -P)
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-ci-native-host-inputs
fail() { echo "ci-host-provision: core.001a.invalid: $*" >&2; exit 1; }
[ "$#" -eq 1 ] || fail 'one private compatibility directory is required'
COMPAT_ROOT=$1
if [ "${SEEN_CAPPED_PLATFORM_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" --platform "$SCOPE" -- bash "$0" "$@"
fi
bash "$CAPPED_ENTRY" --verify-platform-active "$SCOPE"
source "$ROOT_DIR/scripts/artifact_root.sh"
seen_artifact_root_init "$ROOT_DIR" || fail 'artifact root validation failed'
case "$COMPAT_ROOT" in
    "$ROOT_DIR"/.seen/toolchains/ci-bootstrap-compat.*) ;;
    *) fail 'compatibility output escaped its project-local directory' ;;
esac
seen_artifact_assert_safe_relative_path "${COMPAT_ROOT#"$ROOT_DIR"/}" || fail 'unsafe compatibility path'
seen_artifact_assert_no_symlink_components "$ROOT_DIR" "${COMPAT_ROOT#"$ROOT_DIR"/}" || fail 'unsafe compatibility components'
[ -d "$COMPAT_ROOT" ] && [ ! -L "$COMPAT_ROOT" ] &&
    [ "$(cd -P -- "$COMPAT_ROOT" && pwd -P)" = "$COMPAT_ROOT" ] || fail 'compatibility directory is not canonical'
for output in "$COMPAT_ROOT/libSDL3.so.0" "$COMPAT_ROOT/libSDL3.so"; do
    [ ! -e "$output" ] && [ ! -L "$output" ] || fail 'compatibility output already exists'
done
TMP_DIR=$(mktemp -d "${SEEN_ARTIFACT_ROOT:?}/ci-native-host.XXXXXX")
cleanup() {
    local status=$?
    trap - EXIT
    if [ "$status" -eq 0 ] && [ -z "${SEEN_KEEP_TMP:-}" ]; then
        case "$TMP_DIR" in "$SEEN_ARTIFACT_ROOT"/ci-native-host.*) rm -rf -- "$TMP_DIR" ;; *) status=1 ;; esac
    else
        printf 'Native host input artifacts retained: %s\n' "$TMP_DIR" >&2
    fi
    exit "$status"
}
trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM
printf '%s\n' \
    'define ptr @seen_ci_gep_nuw(ptr %base, i64 %index) {' \
    'entry:' \
    '  %result = getelementptr inbounds nuw i64, ptr %base, i64 %index' \
    '  ret ptr %result' \
    '}' | llvm-as -o "$TMP_DIR/gep-nuw.bc" - ||
    fail "pinned LLVM does not accept Seen's required GEP no-wrap IR dialect"
printf '%s\n' 'int main(void) { return 0; }' | clang \
    -x c - -o "$TMP_DIR/instrumentation" -fuse-ld=lld -fprofile-instr-generate \
    -fcoverage-mapping -fsanitize=undefined -fno-omit-frame-pointer -g ||
    fail 'pinned LLVM instrumentation runtimes cannot compile and link'
[ -f /usr/aarch64-linux-gnu/include/stdint.h ] &&
    [ -f /usr/aarch64-linux-gnu/lib/crt1.o ] || fail 'ARM64 libc sysroot is unavailable'
printf '%s\n' '#include <stdint.h>' \
    'uint32_t seen_ci_arm64(uint32_t value) { return value + 1; }' | \
    clang --target=aarch64-unknown-linux-gnu --sysroot=/usr/aarch64-linux-gnu \
    -x c -c - -o "$TMP_DIR/arm64.o" || fail 'ARM64 libc sysroot cannot compile a target object'
LC_ALL=C readelf --file-header "$TMP_DIR/arm64.o" |
    grep -Eq 'Machine:[[:space:]]+AArch64' || fail 'ARM64 probe produced the wrong object architecture'
needs_sdl3_bridge=0
# The frozen 0.10.1 seeds carry an unused SDL3 DT_NEEDED entry. This loader-only
# bridge is admissible only when neither seed imports any SDL symbol; rebuilt
# compilers remove the unused dependency through their ordinary linker policy.
frozen_index=0
for frozen in "$ROOT_DIR/bootstrap/stage1_frozen" "$ROOT_DIR/bootstrap/stage1_frozen_v3"; do
    [ -f "$frozen" ] && [ ! -L "$frozen" ] || fail "frozen bootstrap input is missing or unsafe: $frozen"
    readelf -d "$frozen" > "$TMP_DIR/frozen-$frozen_index.dynamic"
    if grep -Fq 'libSDL3.so.0' "$TMP_DIR/frozen-$frozen_index.dynamic"; then
        needs_sdl3_bridge=1
        nm -D --undefined-only "$frozen" > "$TMP_DIR/frozen-$frozen_index.undefined"
        if grep -Eq '(^|[[:space:]])SDL_[[:alnum:]_]+' "$TMP_DIR/frozen-$frozen_index.undefined"; then
            fail 'frozen bootstrap imports SDL3 symbols; loader bridge refused'
        fi
    fi
    frozen_index=$((frozen_index + 1))
done
if [ "$needs_sdl3_bridge" -eq 1 ]; then
    printf '%s\n' 'void seen_ci_sdl3_loader_bridge(void) {}' |
        cc -x c -shared -Wl,-soname,libSDL3.so.0 - -o "$COMPAT_ROOT/libSDL3.so.0" ||
        fail 'could not build the frozen-bootstrap SDL3 loader bridge'
    ln -s libSDL3.so.0 "$COMPAT_ROOT/libSDL3.so" || fail 'could not expose the frozen-bootstrap SDL3 linker name'
    [ -f "$COMPAT_ROOT/libSDL3.so.0" ] && [ -L "$COMPAT_ROOT/libSDL3.so" ] ||
        fail 'SDL3 loader bridge failed read-back'
fi
printf 'PASS: scoped native SDK inputs and conditional frozen loader bridge\n'
