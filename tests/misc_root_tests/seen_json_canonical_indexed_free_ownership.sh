#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-json-canonical-indexed-free-ownership
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"

if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi

COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
bash "$CAPPED" --verify-active "$SCOPE" --compiler "$COMPILER"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
export SEEN_COMPILER_SOURCE_ROOT="$ROOT_DIR"
WORK="$(mktemp -d "$SEEN_ARTIFACT_ROOT/json-class-ownership.XXXXXX")"

for profile in fast release; do
    flags=(--no-cache --frozen --target-cpu=x86-64 --jobs 1 --opt-jobs 1)
    if [ "$profile" = fast ]; then
        flags+=(--fast)
    else
        flags+=(--release --lto=thin)
    fi
    for fixture in json_canonical_ownership indexed_class_free_ownership; do
        source="$ROOT_DIR/tests/fixtures/$fixture.seen"
        output="$WORK/$fixture-$profile"
        ir_dir="$WORK/$fixture-$profile-ir"
        log="$WORK/$fixture-$profile.log"
        timeout --foreground --kill-after=10s 900s \
            bash "$ATTESTED_SEEN" "$COMPILER" check "$source" \
            >"$log" 2>&1 || {
                tail -c 32768 "$log" >&2
                echo "FAIL: $profile $fixture frontend check" >&2
                exit 1
            }
        timeout --foreground --kill-after=10s 900s \
            bash "$ATTESTED_SEEN" "$COMPILER" compile "$source" "$output" \
            "${flags[@]}" --emit-module-ir-dir "$ir_dir" \
            >>"$log" 2>&1 || {
                tail -c 32768 "$log" >&2
                echo "FAIL: $profile $fixture compilation" >&2
                exit 1
            }
        timeout --foreground --kill-after=5s 120s "$output" \
            >>"$log" 2>&1 || {
                tail -c 32768 "$log" >&2
                echo "FAIL: $profile $fixture execution" >&2
                exit 1
            }
    done
    grep -Fxq 'PASS: canonical JSON allocator baseline' \
        "$WORK/json_canonical_ownership-$profile.log"
    grep -Fxq 'PASS: indexed class free restores accounting' \
        "$WORK/indexed_class_free_ownership-$profile.log"
    ir_dir="$WORK/indexed_class_free_ownership-$profile-ir"
    if rg --no-ignore -q '@ProbeOwner_free' "$ir_dir"; then
        echo "FAIL: $profile indexed class free retained unresolved method" >&2
        exit 1
    fi
    rg --no-ignore -q 'call void @seen_pool_free\(ptr .*i64 8\)' "$ir_dir" || {
        echo "FAIL: $profile indexed class free omitted accounted release" >&2
        exit 1
    }
done

echo 'PASS: fast and release/ThinLTO canonical JSON and indexed class ownership'
