#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-release-install-payload
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0" "$@"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
PACKAGE_CLIENT="${SEEN_PACKAGE_CLIENT:-$ROOT_DIR/target/seen-build/package-client/seen-pkg}"
[ -f "$PACKAGE_CLIENT" ] && [ -x "$PACKAGE_CLIENT" ] && [ ! -L "$PACKAGE_CLIENT" ] || {
    echo 'FAIL: selected package client is unavailable or unsafe' >&2; exit 1
}
WORK_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/release-install-payload.XXXXXX")"
cleanup() {
    local status=$?
    if [ "$status" -eq 0 ] && [ -z "${SEEN_KEEP_TMP:-}" ]; then
        case "$WORK_DIR" in
            "$SEEN_ARTIFACT_ROOT"/release-install-payload.*)
                [ -d "$WORK_DIR" ] && [ ! -L "$WORK_DIR" ] &&
                    [ "$(realpath -e -- "$WORK_DIR")" = "$WORK_DIR" ] || return 1
                chmod -R u+w -- "$WORK_DIR"
                rm -rf -- "$WORK_DIR"
                ;;
            *) return 1 ;;
        esac
    else
        echo "Preserved installed-payload evidence: $WORK_DIR (status $status)" >&2
    fi
    return "$status"
}
trap cleanup EXIT
prefix="$WORK_DIR/prefix"
mkdir -p "$prefix/bin" "$prefix/lib/seen/std" "$prefix/lib/seen/runtime" "$WORK_DIR/empty"
cp -- "$COMPILER" "$prefix/bin/seen"
cp -- "$PACKAGE_CLIENT" "$prefix/bin/seen-pkg"
cp -- "$ROOT_DIR/releases/compatibility-manifest.json" "$prefix/bin/compatibility-manifest.json"
cp -r -- "$ROOT_DIR/seen_std/src/." "$prefix/lib/seen/std/"
while IFS= read -r -d '' runtime_file; do
    case "$runtime_file" in *.o|*.sig|*.a) continue ;; esac
    [ -f "$ROOT_DIR/$runtime_file" ] && [ ! -L "$ROOT_DIR/$runtime_file" ] || exit 1
    relative=${runtime_file#seen_runtime/}
    mkdir -p -- "$prefix/lib/seen/runtime/$(dirname -- "$relative")"
    cp -- "$ROOT_DIR/$runtime_file" "$prefix/lib/seen/runtime/$relative"
done < <(git -C "$ROOT_DIR" ls-files -z -- seen_runtime)
cmp -- "$COMPILER" "$prefix/bin/seen"
cmp -- "$PACKAGE_CLIENT" "$prefix/bin/seen-pkg"
cmp -- "$ROOT_DIR/releases/compatibility-manifest.json" "$prefix/bin/compatibility-manifest.json"
test -f "$prefix/lib/seen/std/json/strict.seen"
test -f "$prefix/lib/seen/std/crypto/sha256.seen"
if find "$prefix/lib/seen/runtime" -type f \( -name '*.o' -o -name '*.sig' -o -name '*.a' \) -print -quit | grep -q .; then
    echo 'FAIL: source-only staged runtime contains generated objects' >&2; exit 1
fi
# Explicit legacy upgrade debris must remain immutable and must never be linked.
printf 'stale installed object\n' > "$prefix/lib/seen/runtime/seen_runtime.o"
printf 'stale-signature\n' > "$prefix/lib/seen/runtime/seen_runtime.o.sig"
printf 'stale installed region object\n' > "$prefix/lib/seen/runtime/seen_region.o"
chmod -R a-w -- "$prefix"
prefix_digest() {
    find "$1" -type f -print0 | sort -z | xargs -0 sha256sum | sha256sum | cut -d ' ' -f1
}
before=$(prefix_digest "$prefix")
stage_project() {
    local project=$1
    mkdir -p -- "$project"
    cp -- "$ROOT_DIR/tests/misc_root_tests/seen_release_payload_api.seen" "$project/main.seen"
    cp -- "$ROOT_DIR/tests/fixtures/result-string-unwrap/packaged_readtext.seen" "$project/readtext.seen"
    printf '{}' > "$project/input.json"
    printf 'manifest-version = 1\n[project]\nname = "installed_payload_fixture"\nversion = "0.0.0"\nlanguage = "en"\n[dependencies]\n' > "$project/Seen.toml"
}
run_worker() {
    local selected_prefix=$1 project=$2 mode=$3
    local worker_scope="seen-release-install-payload-$mode"
    if [ "$mode" = positive ]; then worker_scope=seen-release-install-payload-installed; fi
    # Prove the copied compiler binding; hide checkout libraries/runtime so a
    # successful compile cannot silently consume source-tree inputs instead.
    bwrap --die-with-parent --bind / / --dev-bind /dev /dev --proc /proc --ro-bind /sys /sys \
        --ro-bind "$WORK_DIR/empty" "$ROOT_DIR/seen_std/src" \
        --ro-bind "$WORK_DIR/empty" "$ROOT_DIR/seen_runtime" -- \
        bash "$CAPPED_ENTRY" "$worker_scope" --compiler "$selected_prefix/bin/seen" -- \
        bash "$ROOT_DIR/scripts/installed_payload_fixture_worker.sh" "$selected_prefix" "$project" "$mode"
}
source_project="$WORK_DIR/source space-π"
stage_project "$source_project"
run_worker "$prefix" "$source_project" positive
[ "$before" = "$(prefix_digest "$prefix")" ] || {
    echo 'FAIL: read-only installed source payload changed during compilation' >&2; exit 1
}
# A real compiler error in a separate private source-only prefix. Never shadow
# clang, forge success, or hide a missing cache object with installed objects.
bad_prefix="$WORK_DIR/runtime-failure-prefix"
cp -a -- "$prefix" "$bad_prefix"
chmod u+w -- "$bad_prefix/lib/seen/runtime/seen_runtime.c"
printf '\n#error FEL748_INVALID_RUNTIME_SOURCE\n' >> "$bad_prefix/lib/seen/runtime/seen_runtime.c"
chmod a-w -- "$bad_prefix/lib/seen/runtime/seen_runtime.c"
bad_before=$(prefix_digest "$bad_prefix")
stage_project "$WORK_DIR/compiler-failure"
run_worker "$bad_prefix" "$WORK_DIR/compiler-failure" runtime-failure
[ "$bad_before" = "$(prefix_digest "$bad_prefix")" ] || exit 1
stage_project "$WORK_DIR/cache-failure"
printf 'blocked artifact directory\n' > "$WORK_DIR/cache-failure/.seen"
run_worker "$prefix" "$WORK_DIR/cache-failure" cache-failure
if command -v nvcc >/dev/null 2>&1 && command -v nvidia-smi >/dev/null 2>&1 &&
    timeout --kill-after=5s 30s nvidia-smi --query-gpu=compute_cap --format=csv,noheader | grep -Fxq '8.9'; then
    env -u SEEN_CAPPED_REGRESSION_ACTIVE -u SEEN_COMPILER_SOURCE_ROOT -u SEEN_PACKAGE_CLIENT \
        COMPILER="$prefix/bin/seen" SEEN_BIN="$prefix/bin/seen" \
        bash "$ROOT_DIR/tests/misc_root_tests/seen_bundled_cuda_link.sh"
else
    echo 'SKIP: installed bundled CUDA execution requires nvcc and an sm_89 GPU'
fi
[ "$before" = "$(prefix_digest "$prefix")" ] || {
    echo 'FAIL: installed source payload changed during negative or hardware checks' >&2; exit 1
}
echo 'PASS: attested read-only source-only installed payload, local caches and real failure controls'
