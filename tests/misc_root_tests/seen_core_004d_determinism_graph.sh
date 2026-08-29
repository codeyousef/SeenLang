#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -P)
COMPILER=${COMPILER:-${SEEN_BIN:-$ROOT_DIR/compiler_seen/target/seen}}
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-core-004d

if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != "1" ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- \
        bash "$0" "$@"
fi

COMPILER=${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
ATTESTED_SEEN=${SEEN_ATTESTED_COMPILER_RUNNER:?}
ARTIFACT_ROOT=${SEEN_ARTIFACT_ROOT:?}
FIXTURES="$ROOT_DIR/tests/fixtures/core-004d"
WORK_DIR=$(mktemp -d "$ARTIFACT_ROOT/core-004d.XXXXXX")

cleanup() {
    local status=$?
    if [ "$status" -eq 0 ]; then
        if ! chmod -R u+rwX -- "$WORK_DIR"; then
            echo "FAIL: could not make CORE-004D generated artifacts removable: $WORK_DIR" >&2
            return 1
        fi
        if ! rm -rf -- "$WORK_DIR"; then
            echo "FAIL: could not remove CORE-004D generated artifacts: $WORK_DIR" >&2
            return 1
        fi
    else
        echo "Preserved failed CORE-004D artifacts: $WORK_DIR" >&2
    fi
    return "$status"
}
trap cleanup EXIT

run_compiler() {
    SEEN_DETERMINISTIC=1 SOURCE_DATE_EPOCH=1700000000 \
        SEEN_DETERMINISTIC_SEED=1101 SEEN_HASH_SEED=1101 \
        LANG=C.UTF-8 LC_ALL=C.UTF-8 TZ=UTC \
        bash "$ATTESTED_SEEN" --deterministic-environment "$COMPILER" "$@"
}

expect_check_success() {
    local label=$1
    local source=$2
    local log="$WORK_DIR/$label.log"
    if ! run_compiler check "$source" --deterministic >"$log" 2>&1; then
        echo "FAIL: $label was rejected" >&2
        tail -n 120 "$log" >&2 || true
        exit 1
    fi
    grep -Fq '[OK] Check passed' "$log"
    echo "PASS: $label"
}

expect_check_failure() {
    local label=$1
    local source=$2
    local code=$3
    local location_fragment=$4
    local log="$WORK_DIR/$label.log"
    if run_compiler check "$source" --deterministic >"$log" 2>&1; then
        echo "FAIL: $label unexpectedly passed" >&2
        exit 1
    fi
    grep -Fq "error[$code]" "$log"
    grep -Fq "$location_fragment" "$log"
    grep -Fq '= declaration:' "$log"
    grep -Fq '= effect path:' "$log"
    grep -Fq '= help:' "$log"
    echo "PASS: $label"
}

expect_check_success \
    CORE-004D_graph_happy \
    "$FIXTURES/CORE-004D_graph_happy/main.seen"

expect_check_failure \
    CORE-004D_import_reject \
    "$FIXTURES/CORE-004D_import_reject/main.seen" \
    core.004d.nondeterministic-effect \
    imported_noise.seen

expect_check_failure \
    CORE-004D_initializer_reject \
    "$FIXTURES/CORE-004D_import_reject/initializer.seen" \
    core.004d.nondeterministic-effect \
    CORE-004D_import_reject/initializer.seen
grep -Fq '__seen_internal_module_init__' \
    "$WORK_DIR/CORE-004D_initializer_reject.log"

PACKAGE_COPY="$WORK_DIR/package-reject"
cp -R -- "$FIXTURES/CORE-004D_package_reject" "$PACKAGE_COPY"
expect_check_failure \
    CORE-004D_package_reject \
    "$PACKAGE_COPY/consumer/src/main.seen" \
    core.004d.nondeterministic-effect \
    noise.seen

expect_check_failure \
    CORE-004D_alias_reject \
    "$FIXTURES/CORE-004D_alias_reject/main.seen" \
    core.004d.floating-point \
    CORE-004D_alias_reject/main.seen

expect_check_failure \
    CORE-004D_generic_reject \
    "$FIXTURES/CORE-004D_generic_reject/main.seen" \
    core.004d.nondeterministic-effect \
    CORE-004D_generic_reject/main.seen

expect_check_failure \
    CORE-004D_closure_reject \
    "$FIXTURES/CORE-004D_closure_reject/main.seen" \
    core.004d.nondeterministic-effect \
    CORE-004D_closure_reject/main.seen

expect_check_failure \
    CORE-004D_effect_path \
    "$FIXTURES/CORE-004D_effect_path/main.seen" \
    core.004d.nondeterministic-effect \
    leaf.seen
grep -Fq 'middleValue' "$WORK_DIR/CORE-004D_effect_path.log"
grep -Fq 'leafValue' "$WORK_DIR/CORE-004D_effect_path.log"

expect_check_failure \
    CORE-004D_trait_reject \
    "$FIXTURES/CORE-004D_effect_path/trait.seen" \
    core.004d.nondeterministic-effect \
    CORE-004D_effect_path/trait.seen
grep -Fq 'ValueSource.value' "$WORK_DIR/CORE-004D_trait_reject.log"
grep -Fq 'NoisySource' "$WORK_DIR/CORE-004D_trait_reject.log"

expect_check_success \
    CORE-004D_annotation_boundary \
    "$FIXTURES/CORE-004D_annotation_boundary/main.seen"

expect_check_failure \
    CORE-004D_annotation_invalid \
    "$FIXTURES/CORE-004D_annotation_boundary/invalid.seen" \
    core.004d.annotation-invalid \
    CORE-004D_annotation_boundary/invalid.seen

expect_check_failure \
    CORE-004D_cycle_limit \
    "$FIXTURES/CORE-004D_cycle_limit/main.seen" \
    core.004d.nondeterministic-effect \
    CORE-004D_cycle_limit/main.seen
if grep -Fq 'core.004d.limit' "$WORK_DIR/CORE-004D_cycle_limit.log"; then
    echo 'FAIL: bounded recursive graph incorrectly exhausted the limit' >&2
    exit 1
fi
if [ "$(wc -l <"$WORK_DIR/CORE-004D_cycle_limit.log")" -gt 120 ]; then
    echo 'FAIL: recursive graph emitted unbounded diagnostics' >&2
    exit 1
fi

PARITY_SOURCE="$FIXTURES/CORE-004D_mode_parity/main.seen"
CHECK_LOG="$WORK_DIR/CORE-004D_mode_parity-check.log"
COMPILE_LOG="$WORK_DIR/CORE-004D_mode_parity-compile.log"
RUN_LOG="$WORK_DIR/CORE-004D_mode_parity-run.log"
if run_compiler check "$PARITY_SOURCE" --deterministic \
    >"$CHECK_LOG" 2>&1; then
    echo 'FAIL: mode-parity check unexpectedly passed' >&2
    exit 1
fi
if run_compiler compile "$PARITY_SOURCE" "$WORK_DIR/parity-output" \
    --deterministic --no-cache --no-fork \
    >"$COMPILE_LOG" 2>&1; then
    echo 'FAIL: mode-parity compile unexpectedly passed' >&2
    exit 1
fi
if run_compiler run "$PARITY_SOURCE" --deterministic --no-cache \
    >"$RUN_LOG" 2>&1; then
    echo 'FAIL: mode-parity run unexpectedly passed' >&2
    exit 1
fi
test ! -e "$WORK_DIR/parity-output"

for log in "$CHECK_LOG" "$COMPILE_LOG" "$RUN_LOG"; do
    grep -Fq 'error[core.004d.nondeterministic-effect]' "$log"
    grep -Fq 'parity_dep.seen:' "$log"
done

check_code=$(grep -m1 -o 'core\.004d\.[a-z-]*' "$CHECK_LOG")
compile_code=$(grep -m1 -o 'core\.004d\.[a-z-]*' "$COMPILE_LOG")
run_code=$(grep -m1 -o 'core\.004d\.[a-z-]*' "$RUN_LOG")
[ "$check_code" = "$compile_code" ]
[ "$check_code" = "$run_code" ]

check_location=$(grep -m1 -- '--> .*parity_dep.seen:' "$CHECK_LOG" | sed 's/.*parity_dep/parity_dep/')
compile_location=$(grep -m1 -- '--> .*parity_dep.seen:' "$COMPILE_LOG" | sed 's/.*parity_dep/parity_dep/')
run_location=$(grep -m1 -- '--> .*parity_dep.seen:' "$RUN_LOG" | sed 's/.*parity_dep/parity_dep/')
[ "$check_location" = "$compile_location" ]
[ "$check_location" = "$run_location" ]
echo 'PASS: CORE-004D_mode_parity'

echo 'PASS: CORE-004D resolved determinism graph contracts'
