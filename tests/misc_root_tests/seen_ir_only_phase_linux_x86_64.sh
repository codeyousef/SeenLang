#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-ir-only-phase
COMPILER="${COMPILER:-${SEEN_BIN:-$ROOT_DIR/compiler_seen/target/seen}}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/ir-only-phase.XXXXXX")"
FIXTURE_ARTIFACT_PREFIX=ir-only-phase
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"
SOURCE="$ROOT_DIR/tests/fixtures/test-002a/ir_only_phase_probe.seen"
run_compiler() { (cd "$TMP_DIR"; bash "$ATTESTED_SEEN" "$COMPILER" "$@"); }
fail() { printf 'FAIL: raw IR phase: %s\n' "$*" >&2; exit 1; }

# None of these paths exists. Successful raw IR must not depend on a link SDK.
export SEEN_LINUX_ARM64_SYSROOT="$TMP_DIR/missing-arm-sdk"
export SEEN_LINUX_RISCV64_SYSROOT="$TMP_DIR/missing-riscv-sdk"
export RISCV64_SYSROOT="$TMP_DIR/missing-riscv-sdk"
export ANDROID_NDK_HOME="$TMP_DIR/missing-android-sdk"
export ANDROID_NDK_ROOT="$ANDROID_NDK_HOME"

native_snapshot() {
    find "$SEEN_ARTIFACT_ROOT" -type f \( -name '*.o' -o -name '*.bc' \
        -o -name '*.opt.ll' -o -name '*.opt.status' \) -print0 | LC_ALL=C sort -z
}
check_ir() {
    local target=$1 profile=$2 cache=$3 name="$1-$2-$3" triple
    local input=${4:-$SOURCE}
    local flags=(--fast) cache_flags=() cpu_flags=()
    if [ "$profile" = release ]; then flags=(--release --lto=thin); fi
    if [ "$cache" = cold ] || [ "$cache" = generated-cold ]; then cache_flags=(--no-cache); fi
    if [ "$target" = linux-x86_64 ]; then cpu_flags=(--target-cpu=x86-64); fi
    native_snapshot > "$TMP_DIR/$name.before"
    run_compiler compile "$input" "$TMP_DIR/$name-output" --frozen \
        "${flags[@]}" "${cache_flags[@]}" "${cpu_flags[@]}" --target="$target" \
        --emit-module-ir-dir "$TMP_DIR/$name-ir" --stop-after-ir \
        --object-manifest "$TMP_DIR/$name.objects" > "$TMP_DIR/00-active.log" 2>&1
    cp -- "$TMP_DIR/00-active.log" "$TMP_DIR/$name.compile.log"
    native_snapshot > "$TMP_DIR/$name.after"
    cmp -- "$TMP_DIR/$name.before" "$TMP_DIR/$name.after" || fail "$name emitted native objects or optimizer output"
    [ ! -e "$TMP_DIR/$name-output" ] && [ ! -e "$TMP_DIR/$name.objects" ] || fail "$name published a native output"
    if rg -q '\[Pass 2b\]|\[3\] Linking' "$TMP_DIR/$name.compile.log"; then
        fail "$name entered optimizer/object or link phase"
    fi
    grep -Fq '[3] Stopping after module IR emission' "$TMP_DIR/$name.compile.log"
    case "$target" in
        linux-x86_64) triple='x86_64-unknown-linux-gnu' ;;
        linux-arm64) triple='aarch64-unknown-linux-gnu' ;;
        linux-riscv64) triple='riscv64-unknown-linux-gnu' ;;
        android-arm64) triple='aarch64-linux-android' ;;
        *) fail "unrecognized test target $target" ;;
    esac
    local modules=("$TMP_DIR/$name-ir"/*.ll) ir
    [ "${#modules[@]}" -gt 0 ] && [ "${#modules[@]}" -le 128 ] || fail "$name raw module count"
    for ir in "${modules[@]}"; do
        [ -f "$ir" ] && [ ! -L "$ir" ] && [ "$(wc -c < "$ir")" -le 16777216 ] || fail "$name raw module missing or oversized"
        grep -Fq "target triple = \"$triple" "$ir" || fail "$name changed target to host"
        timeout --foreground --kill-after=5s 60s opt -passes=verify -disable-output "$ir"
    done
    printf 'PASS: raw IR target=%s profile=%s cache=%s modules=%s\n' "$target" "$profile" "$cache" "${#modules[@]}"
}

for profile in fast release; do
    flags=(--fast)
    if [ "$profile" = release ]; then flags=(--release --lto=thin); fi
    # Seed a genuine successful object cache for the identical native input.
    run_compiler compile "$SOURCE" "$TMP_DIR/seed-$profile" --frozen \
        "${flags[@]}" --target=linux-x86_64 --target-cpu=x86-64 > "$TMP_DIR/00-active.log" 2>&1
    cp -- "$TMP_DIR/00-active.log" "$TMP_DIR/seed-$profile.compile.log"
    timeout --foreground --kill-after=5s 120s "$TMP_DIR/seed-$profile"
    run_compiler compile "$SOURCE" "$TMP_DIR/warm-$profile" --frozen \
        "${flags[@]}" --target=linux-x86_64 --target-cpu=x86-64 > "$TMP_DIR/00-active.log" 2>&1
    cp -- "$TMP_DIR/00-active.log" "$TMP_DIR/warm-$profile.compile.log"
    grep -Eq 'source cache hit|IR cache hit|Complete \([1-9][0-9]*/[0-9]+ cached\)' \
        "$TMP_DIR/warm-$profile.compile.log" || fail "$profile object cache was not warmed"
    timeout --foreground --kill-after=5s 120s "$TMP_DIR/warm-$profile"
    for cache in cold warm; do check_ir linux-x86_64 "$profile" "$cache"; done
done
for target in linux-arm64 linux-riscv64 android-arm64; do
    for cache in cold enabled; do check_ir "$target" fast "$cache"; done
done

# Generated test mains intentionally produce module objects, not an executable.
# Warm that real cache, then require raw IR to bypass both cache and child-object
# fast paths without losing the generated source's declarations.
generated="$TMP_DIR/probe.test_main.seen"
cp -- "$SOURCE" "$generated"
for pass in seed warm; do
    run_compiler compile "$generated" "$TMP_DIR/generated-$pass" --frozen \
        --fast --target=linux-x86_64 --target-cpu=x86-64 > "$TMP_DIR/00-active.log" 2>&1
    cp -- "$TMP_DIR/00-active.log" "$TMP_DIR/generated-$pass.compile.log"
    grep -Fq '[3] Skipping final link for generated test main; module objects are ready' "$TMP_DIR/00-active.log"
done
if rg -q '\[Pass 2\]|\[Pass 2b\]' "$TMP_DIR/generated-warm.compile.log"; then
    fail 'generated test object cache was not warmed'
fi
check_ir linux-x86_64 fast generated-cold "$generated"
check_ir linux-x86_64 fast generated-warm "$generated"

# IR-only does not relax the existing Linux-x86-64 explicit-LTO policy.
status=0
run_compiler compile "$SOURCE" "$TMP_DIR/unsupported-lto-output" \
    --release --lto=thin --target=linux-arm64 --stop-after-ir \
    --emit-module-ir-dir "$TMP_DIR/unsupported-lto-ir" > "$TMP_DIR/00-active.log" 2>&1 || status=$?
[ "$status" -eq 1 ] || fail "cross-target LTO policy returned $status"
grep -Fq 'core.rel.003.platform' "$TMP_DIR/00-active.log"
[ ! -e "$TMP_DIR/unsupported-lto-output" ] || fail 'unsupported LTO produced an executable'
[ ! -e "$TMP_DIR/unsupported-lto-ir" ] || fail 'unsupported LTO published raw IR'

# Publication failure must not fall through into optimization or native linking.
printf 'not a directory\n' > "$TMP_DIR/blocked-ir-output"
native_snapshot > "$TMP_DIR/blocked-publication.before"
status=0
run_compiler compile "$SOURCE" "$TMP_DIR/blocked-native-output" --no-cache \
    --frozen --fast --target=linux-x86_64 --target-cpu=x86-64 \
    --stop-after-ir --emit-module-ir-dir "$TMP_DIR/blocked-ir-output" \
    > "$TMP_DIR/00-active.log" 2>&1 || status=$?
[ "$status" -eq 1 ] || fail "blocked raw IR publication returned $status"
grep -Fq 'failed to emit module IR directory' "$TMP_DIR/00-active.log"
[ ! -e "$TMP_DIR/blocked-native-output" ] || fail 'failed IR publication produced an executable'
native_snapshot > "$TMP_DIR/blocked-publication.after"
cmp -- "$TMP_DIR/blocked-publication.before" "$TMP_DIR/blocked-publication.after" || fail 'failed IR publication emitted native objects'
if rg -q '\[Pass 2b\]|\[3\] Linking|Stopping after module IR emission|Build succeeded' "$TMP_DIR/00-active.log"; then
    fail 'failed IR publication continued or reported success'
fi

# An intentionally empty SDK is a negative input, never a success workaround.
mkdir "$TMP_DIR/invalid-empty-sdk"
status=0
SEEN_LINUX_ARM64_SYSROOT="$TMP_DIR/invalid-empty-sdk" \
    run_compiler compile "$SOURCE" "$TMP_DIR/missing-sdk-output" --no-cache \
    --frozen --fast --target=linux-arm64 > "$TMP_DIR/00-active.log" 2>&1 || status=$?
[ "$status" -eq 1 ] || fail "ordinary cross-compilation with invalid SDK returned $status"
[ ! -e "$TMP_DIR/missing-sdk-output" ] || fail 'invalid SDK produced an executable'
if grep -Fq 'Build succeeded' "$TMP_DIR/00-active.log"; then fail 'invalid SDK reported success'; fi
echo 'PASS: raw IR precedes native phases and preserves semantic/SDK rejection'
