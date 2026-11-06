#!/usr/bin/env bash
# Exercise a copied compiler only after its own capped binding is established.
set -euo pipefail
ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/.." && pwd -P)"
[ "$#" -eq 3 ] || exit 64
prefix=$1
project=$2
mode=$3
case "$mode" in
    positive) SCOPE=seen-release-install-payload-installed ;;
    runtime-failure) SCOPE=seen-release-install-payload-runtime-failure ;;
    cache-failure) SCOPE=seen-release-install-payload-cache-failure ;;
    *) exit 64 ;;
esac
for directory in "$prefix" "$project"; do
    case "$directory" in
        "${SEEN_ARTIFACT_ROOT:?}"/release-install-payload.*/*) ;;
        *) echo 'FAIL: installed fixture escaped its owned artifact directory' >&2; exit 126 ;;
    esac
    [ -d "$directory" ] && [ ! -L "$directory" ] &&
        [ "$(realpath -e -- "$directory")" = "$directory" ] || exit 126
done
COMPILER="$prefix/bin/seen"
bash "$ROOT_DIR/scripts/run_capped_regression.sh" --verify-active "$SCOPE" --compiler "$COMPILER"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
run_installed() (
    cd -- "$project"
    bash "$ATTESTED_SEEN" --consumer-context "$project" "$COMPILER" "$@"
)
run_binary() {
    local binary=$1
    bash "$ROOT_DIR/scripts/check_x86_executable_baseline.sh" x86-64 "$binary"
    (cd -- "$project" && timeout --foreground --kill-after=5s 120s "$binary")
}
if [ "$mode" != positive ]; then
    status=0
    run_installed compile "$project/main.seen" "$project/failed-output" \
        --fast --no-cache --target-cpu=x86-64 >"$project/failure.log" 2>&1 || status=$?
    if [ "$status" -eq 0 ] || [ -e "$project/failed-output" ] ||
        grep -Fq 'Build succeeded' "$project/failure.log"; then
        echo "FAIL: installed $mode did not fail closed" >&2
        exit 1
    fi
    if [ "$mode" = runtime-failure ]; then
        grep -Fq FEL748_INVALID_RUNTIME_SOURCE "$project/failure.log"
    else
        grep -Fq 'could not create project artifact directory' "$project/failure.log"
    fi
    printf 'PASS: installed %s returns %s without an executable or false success\n' "$mode" "$status"
    exit 0
fi
reject_context() {
    local name=$1 diagnostic=$2 status=0
    shift 2
    bash "$ATTESTED_SEEN" "$@" > "$project/guard-$name.log" 2>&1 || status=$?
    [ "$status" -eq 126 ] && grep -Fq "$diagnostic" "$project/guard-$name.log" || {
        echo "FAIL: installed context guard $name returned $status" >&2
        tail -c 4000 "$project/guard-$name.log" >&2
        exit 1
    }
}
reject_context root 'consumer project must stay inside the attested artifact root' \
    --consumer-context "$SEEN_ARTIFACT_ROOT" "$COMPILER" check "$project/main.seen"
ln -s . "$project/context-link"
reject_context symlink 'consumer project must be an existing canonical directory' \
    --consumer-context "$project/context-link" "$COMPILER" check "$project/main.seen"
reject_context source 'consumer source escapes its project' \
    --consumer-context "$project" "$COMPILER" check "$ROOT_DIR/tests/fixtures/test-002a/failure-probe/main.seen"
reject_context output 'consumer output escapes its project' \
    --consumer-context "$project" "$COMPILER" compile "$project/main.seen" "$project-parent-output"
[ ! -e "$project-parent-output" ] || exit 1
ln -s main.seen "$project/source-link.seen"
reject_context source-link 'consumer source must be a canonical regular file' \
    --consumer-context "$project" "$COMPILER" check "$project/source-link.seen"
reject_context modes 'consumer context cannot combine invocation modes' \
    --deterministic-environment --consumer-context "$project" "$COMPILER" check "$project/main.seen"
reject_context project-control 'consumer project contains control characters' \
    --consumer-context "$project"$'\n' "$COMPILER" check "$project/main.seen"
reject_context source-control 'consumer source contains control characters' \
    --consumer-context "$project" "$COMPILER" check "$project/main"$'\n'".seen"
reject_context output-control 'consumer output contains control characters' \
    --consumer-context "$project" "$COMPILER" compile "$project/main.seen" "$project/output"$'\r'".bin"
reject_context command 'consumer context supports only check and compile' \
    --consumer-context "$project" "$COMPILER" version
reject_context noncanonical 'consumer project must be an existing canonical directory' \
    --consumer-context "$project/../$(basename -- "$project")" "$COMPILER" check "$project/main.seen"
printf 'untouched output sentinel\n' > "$project/output-target"
ln -s output-target "$project/output-link"
reject_context output-link 'consumer output must not be a symlink' \
    --consumer-context "$project" "$COMPILER" compile "$project/main.seen" "$project/output-link"
grep -Fxq 'untouched output sentinel' "$project/output-target"
expected_version=$(python3 - "$prefix/bin/compatibility-manifest.json" <<'PY'
import json
import sys
value = json.load(open(sys.argv[1]))
compiler = value['components']['compiler']['version']
assert compiler == value['components']['package_client']['version']
print(compiler)
PY
)
timeout --foreground --kill-after=5s 30s "$prefix/bin/seen-pkg" \
    --expect-version "$expected_version" version --machine > "$project/package-client.json"
run_installed check "$project/main.seen" > "$project/frontend.log" 2>&1
for profile in fast release; do
    cache="$project/.seen/agent-tools/runtime-objects"
    if [ -e "$cache" ]; then
        [ -d "$cache" ] && [ ! -L "$cache" ] || exit 126
        [ ! -e "$project/runtime-objects-before-$profile" ] || exit 126
        mv -- "$cache" "$project/runtime-objects-before-$profile"
    fi
    flags=(--target-cpu=x86-64)
    if [ "$profile" = fast ]; then flags+=(--fast); else flags+=(--release --lto=thin); fi
    for temperature in cold warm; do
        output="$project/payload-$profile-$temperature"
        run_installed compile "$project/main.seen" "$output" "${flags[@]}" \
            > "$project/$profile-$temperature.compile.log" 2>&1
        run_binary "$output" > "$project/$profile-$temperature.run.log" 2>&1
        grep -Fq 'PASS: installed release standard-library payload' "$project/$profile-$temperature.run.log"
        find "$cache" -type f -name '*.o' -print -quit | grep -q .
    done
done
run_installed check "$project/readtext.seen" > "$project/readtext-frontend.log" 2>&1
for profile in fast release; do
    output="$project/readtext-$profile"
    ir_dir="$project/readtext-$profile-ir"
    flags=(--target-cpu=x86-64 --emit-module-ir-dir "$ir_dir")
    if [ "$profile" = fast ]; then flags+=(--fast); else flags+=(--release --lto=thin); fi
    run_installed compile "$project/readtext.seen" "$output" "${flags[@]}" \
        > "$project/readtext-$profile.compile.log" 2>&1
    run_binary "$output" > "$project/readtext-$profile.run.log" 2>&1
    if rg --no-ignore -q '@ptr_length' "$ir_dir"; then
        echo "FAIL: installed readText $profile emitted ptr_length" >&2
        exit 1
    fi
    rg --no-ignore -q 'call i64 @seen_length\(%SeenString ' "$ir_dir"
done
echo 'PASS: copied installed compiler frontend, cold/warm fast/ThinLTO, portable CPU and typed readText'
