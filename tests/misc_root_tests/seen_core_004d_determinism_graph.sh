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
TMP_DIR="$WORK_DIR"
FIXTURE_ARTIFACT_PREFIX=core-004d
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"

cleanup() {
    local status=$?
    trap - EXIT
    if [ "$status" -eq 0 ] && [ -z "${SEEN_KEEP_TMP:-}" ]; then
        if ! chmod -R u+rwX -- "$WORK_DIR" || ! rm -rf -- "$WORK_DIR"; then
            echo "FAIL: could not remove CORE-004D generated artifacts: $WORK_DIR" >&2
            status=1
        fi
    else
        echo "Preserved CORE-004D artifacts: $WORK_DIR (status $status)" >&2
        if [ "$status" -ne 0 ]; then
            seen_report_fixture_failure_logs "$WORK_DIR"
        fi
    fi
    exit "$status"
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

# Inferred imported return types and chained field receivers are independent of
# declaration discovery order. Each program below is actually checked/compiled.
for order in main-first support-first; do
    typed_root="$WORK_DIR/typed-$order"
    mkdir -p "$typed_root"
    cp "$FIXTURES/CORE-004D_typed_receivers/main.seen" "$typed_root/main.seen"
    cp "$FIXTURES/CORE-004D_typed_receivers/support.seen" "$typed_root/support.seen"
    modules='"main.seen", "support.seen"'
    if [ "$order" = support-first ]; then modules='"support.seen", "main.seen"'; fi
    printf 'manifest-version = 1\n[project]\nname = "typed_receivers"\nversion = "0.1.0"\nlanguage = "en"\nmodules = [%s]\n' \
        "$modules" >"$typed_root/Seen.toml"
    expect_check_success "CORE-004D_typed_$order" "$typed_root/main.seen"
    run_compiler compile "$typed_root/main.seen" "$typed_root/program" \
        --deterministic --no-cache --no-fork >"$typed_root/compile.log" 2>&1
    "$typed_root/program"
done

# String.split is a resolved String intrinsic with an Array<String> result;
# chained element methods exercise that result identity at the CLI boundary.
split_root="$WORK_DIR/typed-split"
mkdir -p "$split_root"
cp "$FIXTURES/CORE-004D_typed_receivers/split.seen" "$split_root/main.seen"
printf 'manifest-version = 1\n[project]\nname = "string_split"\nversion = "0.1.0"\nlanguage = "en"\nmodules = ["main.seen"]\n' \
    >"$split_root/Seen.toml"
expect_check_success CORE-004D_typed_split "$split_root/main.seen"
run_compiler compile "$split_root/main.seen" "$split_root/program" \
    --deterministic --no-cache --no-fork >"$split_root/compile.log" 2>&1
"$split_root/program"

for negative in unresolved spoof split_unresolved split_spoof; do
    negative_root="$WORK_DIR/typed-$negative"
    mkdir -p "$negative_root"
    cp "$FIXTURES/CORE-004D_typed_receivers/$negative.seen" "$negative_root/main.seen"
    printf 'manifest-version = 1\n[project]\nname = "receiver_negative"\nversion = "0.1.0"\nlanguage = "en"\nmodules = ["main.seen"]\n' \
        >"$negative_root/Seen.toml"
    code=core.004d.unresolved-call
    if [ "$negative" = spoof ] || [ "$negative" = split_spoof ]; then
        code=core.004d.nondeterministic-effect
    fi
    expect_check_failure "CORE-004D_typed_$negative" "$negative_root/main.seen" \
        "$code" "typed-$negative/main.seen"
done

# Generic arguments belong to the caller even when the declaring module has a
# different type with the same spelling. Exercise both fields and method returns.
for order in main-first support-first; do
    generic_root="$WORK_DIR/generic-$order"
    mkdir -p "$generic_root"
    for source in main support caller_types; do
        cp "$FIXTURES/CORE-004D_generic_receivers/$source.seen" "$generic_root/$source.seen"
    done
    modules='"main.seen", "support.seen", "caller_types.seen"'
    if [ "$order" = support-first ]; then
        modules='"support.seen", "caller_types.seen", "main.seen"'
    fi
    printf 'manifest-version = 1\n[project]\nname = "generic_receivers"\nversion = "0.1.0"\nlanguage = "en"\nmodules = [%s]\n' \
        "$modules" >"$generic_root/Seen.toml"
    expect_check_success "CORE-004D_generic_$order" "$generic_root/main.seen"
    run_compiler compile "$generic_root/main.seen" "$generic_root/program" \
        --deterministic --no-cache --no-fork >"$generic_root/compile.log" 2>&1
    "$generic_root/program"
done

# Concrete aggregate shapes use the normal class lifecycle and method ABI.
# Cover function boundaries, alias-equivalent demands, overwrites, adjacent
# fields, multiple shapes, and the unchanged word-erased template together.
for order in main-first support-first; do
    aggregate_root="$WORK_DIR/aggregate-$order"
    mkdir -p "$aggregate_root"
    cp "$FIXTURES/CORE-004D_generic_receivers/aggregate_values.seen" "$aggregate_root/main.seen"
    cp "$FIXTURES/CORE-004D_generic_receivers/support.seen" "$aggregate_root/support.seen"
    modules='"main.seen", "support.seen"'
    if [ "$order" = support-first ]; then modules='"support.seen", "main.seen"'; fi
    printf 'manifest-version = 1\n[project]\nname = "aggregate_receivers"\nversion = "0.1.0"\nlanguage = "en"\nmodules = [%s]\n' \
        "$modules" >"$aggregate_root/Seen.toml"
    expect_check_success "CORE-004D_aggregate_$order" "$aggregate_root/main.seen"
    run_compiler compile "$aggregate_root/main.seen" "$aggregate_root/program" \
        --deterministic --no-cache --no-fork >"$aggregate_root/compile.log" 2>&1
    "$aggregate_root/program"
done

for fixture in unsupported_aggregate_function.seen \
    unsupported_inferred_aggregate_function.seen \
    unsupported_explicit_descriptor_body.seen \
    unsupported_inferred_descriptor_body.seen; do
    aggregate_negative="$WORK_DIR/${fixture%.seen}"
    mkdir -p "$aggregate_negative"
    cp "$FIXTURES/CORE-004D_generic_receivers/$fixture" "$aggregate_negative/main.seen"
    printf 'manifest-version = 1\n[project]\nname = "aggregate_function_negative"\nversion = "0.1.0"\nlanguage = "en"\nmodules = ["main.seen"]\n' \
        >"$aggregate_negative/Seen.toml"
    aggregate_status=0
    run_compiler compile "$aggregate_negative/main.seen" "$aggregate_negative/program" \
        --deterministic --no-cache --no-fork >"$aggregate_negative/compile.log" 2>&1 || aggregate_status=$?
    if [ "$aggregate_status" -ne 1 ] || [ -e "$aggregate_negative/program" ] ||
        ! grep -Fq '[E_GENERIC_AGGREGATE]' "$aggregate_negative/compile.log"; then
        echo "FAIL: $fixture must reject with E_GENERIC_AGGREGATE without a native artifact"
        cat "$aggregate_negative/compile.log" >&2
        exit 1
    fi
done

stable_root="$WORK_DIR/word-stable-functions"
mkdir -p "$stable_root"
cp "$FIXTURES/CORE-004D_generic_receivers/word_stable_functions.seen" "$stable_root/main.seen"
printf 'manifest-version = 1\n[project]\nname = "word_stable_functions"\nversion = "0.1.0"\nlanguage = "en"\nmodules = ["main.seen"]\n' \
    >"$stable_root/Seen.toml"
expect_check_success CORE-004D_word_stable_functions "$stable_root/main.seen"
run_compiler compile "$stable_root/main.seen" "$stable_root/program" \
    --deterministic --no-cache --no-fork >"$stable_root/compile.log" 2>&1
"$stable_root/program"

forward_root="$WORK_DIR/inferred-descriptor-forwarding"
mkdir -p "$forward_root"
cp "$FIXTURES/CORE-004D_generic_receivers/inferred_descriptor_forwarding.seen" "$forward_root/main.seen"
printf 'manifest-version = 1\n[project]\nname = "inferred_descriptor_forwarding"\nversion = "0.1.0"\nlanguage = "en"\nmodules = ["main.seen"]\n' \
    >"$forward_root/Seen.toml"
expect_check_success CORE-004D_inferred_descriptor_forwarding "$forward_root/main.seen"
for profile in fast release undefined address; do
    compile_case "$profile" inferred-forwarding "$forward_root/main.seen"
done

for order in main-first support-first; do
    repair_root="$WORK_DIR/class-repairs-$order"
    mkdir -p "$repair_root"
    cp "$FIXTURES/CORE-004D_generic_receivers/class_repairs.seen" "$repair_root/main.seen"
    cp "$FIXTURES/CORE-004D_generic_receivers/class_repair_support.seen" "$repair_root/class_repair_support.seen"
    modules='"main.seen", "class_repair_support.seen"'
    if [ "$order" = support-first ]; then modules='"class_repair_support.seen", "main.seen"'; fi
    printf 'manifest-version = 1\n[project]\nname = "class_repairs"\nversion = "0.1.0"\nlanguage = "en"\nmodules = [%s]\n' \
        "$modules" >"$repair_root/Seen.toml"
    expect_check_success "CORE-004D_class_repairs_$order" "$repair_root/main.seen"
    for profile in fast release undefined address; do
        compile_case "$profile" "class-repairs-$order" "$repair_root/main.seen"
    done
done

# Separate owning manifests prevent project-wide visibility from hiding a lost
# caller type identity during dependency-template substitution.
dependency_root="$WORK_DIR/class-repairs-dependency"
mkdir -p "$dependency_root/consumer/src" "$dependency_root/provider/src"
cp "$FIXTURES/CORE-004D_generic_receivers/cross_package_class_repairs.seen" \
    "$dependency_root/consumer/src/main.seen"
cp "$FIXTURES/CORE-004D_generic_receivers/class_repair_support.seen" \
    "$dependency_root/provider/src/class_repair_support.seen"
cat >"$dependency_root/consumer/Seen.toml" <<'EOF'
manifest-version = 1
[project]
name = "class_repairs_consumer"
version = "0.1.0"
language = "en"
modules = ["src/main.seen"]
[dependencies]
provider = { path = "../provider" }
EOF
cat >"$dependency_root/provider/Seen.toml" <<'EOF'
manifest-version = 1
[project]
name = "class_repairs_provider"
version = "0.1.0"
language = "en"
modules = ["src/class_repair_support.seen"]
EOF
expect_check_success CORE-004D_dependency_class_repairs "$dependency_root/consumer/src/main.seen"
for profile in fast release undefined address; do
    compile_case "$profile" dependency-class-repairs "$dependency_root/consumer/src/main.seen"
done

callback_root="$WORK_DIR/named-callbacks"
mkdir -p "$callback_root"
cp "$FIXTURES/CORE-004D_generic_receivers/named_callbacks.seen" "$callback_root/main.seen"
printf 'manifest-version = 1\n[project]\nname = "aggregate_named_callbacks"\nversion = "0.1.0"\nlanguage = "en"\nmodules = ["main.seen"]\n' \
    >"$callback_root/Seen.toml"
run_compiler check "$callback_root/main.seen" >"$callback_root/check.log" 2>&1
for profile in fast release undefined address; do
    compile_case "$profile" named-callbacks "$callback_root/main.seen"
done

structural_root="$WORK_DIR/structural-receivers"
mkdir -p "$structural_root"
cp "$FIXTURES/CORE-004D_generic_receivers/structural.seen" "$structural_root/main.seen"
cp "$FIXTURES/CORE-004D_generic_receivers/structural_support.seen" "$structural_root/structural_support.seen"
printf 'manifest-version = 1\n[project]\nname = "structural_receivers"\nversion = "0.1.0"\nlanguage = "en"\nmodules = ["main.seen", "structural_support.seen"]\n' \
    >"$structural_root/Seen.toml"
expect_check_success CORE-004D_structural_receivers "$structural_root/main.seen"

# Boundary annotations on a producer cannot suppress the caller's inferred
# floating-point or unordered-collection type effects.
for effect in float hash; do
    effect_root="$WORK_DIR/canonical-$effect"
    mkdir -p "$effect_root"
    cp "$FIXTURES/CORE-004D_generic_receivers/negative.seen" "$effect_root/main.seen"
    cp "$FIXTURES/CORE-004D_generic_receivers/${effect}_support.seen" "$effect_root/support.seen"
    printf 'manifest-version = 1\n[project]\nname = "canonical_negative"\nversion = "0.1.0"\nlanguage = "en"\nmodules = ["main.seen", "support.seen"]\n' \
        >"$effect_root/Seen.toml"
    code=core.004d.floating-point
    if [ "$effect" = hash ]; then code=core.004d.unordered-collection; fi
    expect_check_failure "CORE-004D_canonical_$effect" "$effect_root/main.seen" \
        "$code" "canonical-$effect/main.seen"
    grep -Eq '= declaration: .*::main$' "$WORK_DIR/CORE-004D_canonical_$effect.log"
done

# A package can spell a module like a stdlib intrinsic owner, but cannot acquire
# the compiler's trusted source provenance or bypass unsafe-FFI rejection.
default_spoof="$WORK_DIR/default-spoof/seen_std/src/core"
mkdir -p "$default_spoof"
cat >"$default_spoof/option.seen" <<'SEEN'
extern fun __default<T>() r: T
fun main() r: Int {
    return __default<Int>()
}
SEEN
expect_check_failure CORE-004D_default_identity_spoof "$default_spoof/option.seen" \
    core.004d.unsafe-ffi default-spoof/seen_std/src/core/option.seen

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
