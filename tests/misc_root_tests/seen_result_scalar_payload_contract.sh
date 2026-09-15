#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-result-scalar-payload
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"

if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
FIXTURE_DIR="$ROOT_DIR/tests/fixtures/fel-1550"
WORK_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/result-scalars.XXXXXX")"

cleanup() {
    local status=$?
    case "$WORK_DIR" in
        "$SEEN_ARTIFACT_ROOT"/result-scalars.*)
            if [ "$status" -eq 0 ] && [ -z "${SEEN_KEEP_TMP:-}" ]; then
                [ -d "$WORK_DIR" ] && [ ! -L "$WORK_DIR" ] &&
                    rm -rf -- "$WORK_DIR"
            else
                echo "Preserved Result scalar artifacts: $WORK_DIR" >&2
            fi
            ;;
        *) status=1 ;;
    esac
    exit "$status"
}
trap cleanup EXIT

run_compiler() {
    bash "$ATTESTED_SEEN" "$COMPILER" "$@"
}

for fixture in same_module cross_module; do
    for profile in fast release; do
        source_file="$FIXTURE_DIR/$fixture.seen"
        output="$WORK_DIR/$fixture-$profile"
        ir_dir="$WORK_DIR/ir-$fixture-$profile"
        log="$WORK_DIR/$fixture-$profile.log"
        flags=(--no-cache --frozen --target-cpu=x86-64 --jobs 1 --opt-jobs 1
            --emit-module-ir-dir "$ir_dir")
        if [ "$profile" = release ]; then
            flags+=(--release --lto=thin)
        else
            flags+=(--fast)
        fi
        run_compiler check "$source_file" >"$log" 2>&1
        run_compiler compile "$source_file" "$output" "${flags[@]}" \
            >>"$log" 2>&1
        timeout --foreground --kill-after=5s 60s "$output" \
            >>"$log" 2>&1
    done
done

PROVIDER_IR="$(grep -El 'define .*@makeInt32\(' \
    "$WORK_DIR"/ir-cross_module-release/*.ll | head -1)"
CONSUMER_IR="$(grep -El 'define .*@main\(' \
    "$WORK_DIR"/ir-cross_module-release/*.ll | head -1)"
[ -n "$PROVIDER_IR" ] && [ -n "$CONSUMER_IR" ]

grep -Eq 'bitcast double .* to i64' "$PROVIDER_IR"
grep -Eq 'bitcast float .* to i32' "$PROVIDER_IR"
[ "$(grep -Ec 'zext i32 .* to i64' "$PROVIDER_IR")" -ge 3 ]
[ "$(grep -Ec 'zext i16 .* to i64' "$PROVIDER_IR")" -ge 2 ]
[ "$(grep -Ec 'zext i8 .* to i64' "$PROVIDER_IR")" -ge 2 ]
grep -Eq 'zext i1 .* to i64' "$PROVIDER_IR"
grep -Eq 'ptrtoint ptr .* to i64' "$PROVIDER_IR"

grep -Eq 'bitcast i64 .* to double' "$CONSUMER_IR"
grep -Eq 'trunc i64 .* to i32' "$CONSUMER_IR"
grep -Eq 'bitcast i32 .* to float' "$CONSUMER_IR"
if grep -Eq 'bitcast i64 .* to float' "$PROVIDER_IR" "$CONSUMER_IR"; then
    echo "FAIL: Float32 Result payload used a width-invalid bitcast" >&2
    exit 1
fi
if grep -Eq 'call i64 @Ok\(i(8|16|32) ' "$PROVIDER_IR"; then
    echo "FAIL: fixed-width Result constructor emitted a mismatched call" >&2
    exit 1
fi

echo "PASS: FEL-1550 Result scalar payload ABI contract"
