#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -P)
COMPILER=${COMPILER:-${SEEN_BIN:-$ROOT_DIR/compiler_seen/target/seen}}
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-core-004e
FIXTURES="$ROOT_DIR/tests/fixtures/core-004e"
POLICY="$ROOT_DIR/seen_std/src/determinism/policy.seen"
CONTEXT="$ROOT_DIR/seen_std/src/determinism/context.seen"
MANIFEST="$ROOT_DIR/seen_std/Seen.toml"
RUNTIME="$ROOT_DIR/seen_runtime/seen_runtime.c"
RUNTIME_FIXTURE="$FIXTURES/runtime_contract.c"

fail() {
    echo "FAIL: CORE-004E deterministic context: $*" >&2
    exit 1
}

test -x "$COMPILER" || fail "compiler is unavailable"
test -f "$CAPPED_ENTRY" && test ! -L "$CAPPED_ENTRY" ||
    fail "capped regression entry is unavailable or unsafe"
test -f "$POLICY" || fail "native policy module is missing"
test -f "$CONTEXT" || fail "native context module is missing"
test -f "$FIXTURES/context_contract.seen" || fail "native fixture is missing"
test -f "$RUNTIME_FIXTURE" || fail "runtime fixture is missing"
test -f "$RUNTIME" || fail "runtime source is missing"
test -f "$FIXTURES/CORE-004E_ffi_bypass_rejected/main.seen" || \
    fail "foreign-call rejection fixture is missing"

python3 -m json.tool "$FIXTURES/cases.json" >/dev/null || fail fixture-json
python3 - "$FIXTURES/cases.json" <<'PY' || exit 1
import json
import sys

expected = [
    "CORE-004E_epoch_happy",
    "CORE-004E_seed_happy",
    "CORE-004E_missing_epoch",
    "CORE-004E_invalid_seed",
    "CORE-004E_env_denied",
    "CORE-004E_locale_pinned",
    "CORE-004E_external_input_denied",
    "CORE-004E_ffi_bypass_rejected",
    "CORE-004E_run_parity",
    "CORE-004E_cancel",
    "CORE-004E_installed_payload",
]
with open(sys.argv[1], "r", encoding="utf-8") as handle:
    document = json.load(handle)
names = [case.get("name") for case in document.get("cases", [])]
if document.get("schema") != "seen-core-004e-fixtures-v1":
    raise SystemExit("fixture schema mismatch")
if names != expected or len(set(names)) != len(names):
    raise SystemExit("fixture name/order mismatch")
for case in document["cases"]:
    if not case.get("kind") or not case.get("expected"):
        raise SystemExit("fixture expectation is incomplete")
PY

python3 -c \
    'import sys,tomllib; d=tomllib.load(open(sys.argv[1],"rb")); m=d["project"]["modules"]; assert "src/determinism/policy.seen" in m; assert "src/determinism/context.seen" in m' \
    "$MANIFEST" || fail manifest-wiring

for code in \
    core.004e.cancelled core.004e.timeout core.004e.limit \
    core.004e.invalid core.004e.missing_epoch core.004e.invalid_epoch \
    core.004e.invalid_seed core.004e.env_denied \
    core.004e.external_input_denied core.004e.policy_conflict; do
    rg -Fq "$code" "$POLICY" "$CONTEXT" || fail "missing $code"
done
rg -Fq 'SEEN_ERROR_MAX_MESSAGE_BYTES: Int = 4096' \
    "$ROOT_DIR/seen_std/src/core/error.seen" || fail error-message-bound
rg -Fq 'SEEN_ERROR_MAX_CAUSES: Int = 8' \
    "$ROOT_DIR/seen_std/src/core/error.seen" || fail error-cause-bound
rg -Fq '@nondeterministic' "$CONTEXT" || fail process-boundary
rg -Fq 'fun deterministicContextFromProcess' "$CONTEXT" || \
    fail process-snapshot
rg -Fq 'func.isExtern' "$ROOT_DIR/compiler_seen/src/typechecker/determinism.seen" || \
    fail foreign-effect-collection
rg -Fq '"unsafe-ffi"' "$ROOT_DIR/compiler_seen/src/typechecker/determinism.seen" || \
    fail foreign-effect-diagnostic
python3 - "$RUNTIME" <<'PY' || exit 1
import pathlib
import sys

source = pathlib.Path(sys.argv[1]).read_text(encoding="utf-8")
start = source.index("int32_t seen_deterministic_path_beneath(")
end = source.index("int32_t seen_time_sleep_nanos(", start)
implementation = source[start:end]
windows = implementation.split("#ifdef _WIN32", 1)[1].split("#else", 1)[0]
if "return -2;" not in windows:
    raise SystemExit("Windows path containment must remain explicitly unsupported")
PY
rg -Fq 'Windows path policy fail closed' \
    "$ROOT_DIR/docs/architecture/native-boundaries.json" || \
    fail windows-path-containment-ledger

if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != "1" ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- \
        bash "$0" "$@"
fi
COMPILER=${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
ATTESTED_SEEN=${SEEN_ATTESTED_COMPILER_RUNNER:?}
ARTIFACT_ROOT=${SEEN_ARTIFACT_ROOT:?}
WORK_DIR=$(mktemp -d "$ARTIFACT_ROOT/core-004e.XXXXXX")
cleanup() {
    local status=$?
    if [ "$status" -eq 0 ]; then
        case "$WORK_DIR" in
            "$ARTIFACT_ROOT"/core-004e.*)
                chmod -R u+w "$WORK_DIR/prefix" 2>/dev/null || true
                rm -rf -- "$WORK_DIR"
                ;;
            *) return 1 ;;
        esac
    else
        echo "Preserved failed CORE-004E artifacts: $WORK_DIR" >&2
    fi
    return "$status"
}
trap cleanup EXIT

mkdir -p "$WORK_DIR/source/determinism" "$WORK_DIR/prefix/bin" \
    "$WORK_DIR/prefix/lib/seen/std" "$WORK_DIR/prefix/lib/seen/runtime" \
    "$WORK_DIR/source/.seen/agent-tools" "$WORK_DIR/path-root/child" \
    "$WORK_DIR/path-root-sibling" "$WORK_DIR/path-outside"
ln -s "$WORK_DIR/path-outside" "$WORK_DIR/path-root/escape"
ln -s "$WORK_DIR/path-root/child" "$WORK_DIR/path-root/inside-link"
cp "$POLICY" "$WORK_DIR/source/determinism/policy.seen"
cp "$CONTEXT" "$WORK_DIR/source/determinism/context.seen"
cp "$FIXTURES/context_contract.seen" "$WORK_DIR/source/main.seen"
cp "$COMPILER" "$WORK_DIR/prefix/bin/seen"
cp -r "$ROOT_DIR/seen_std/src/"* "$WORK_DIR/prefix/lib/seen/std/"
while IFS= read -r -d '' runtime_file; do
    case "$runtime_file" in *.o|*.sig|*.a) continue ;; esac
    relative=${runtime_file#seen_runtime/}
    mkdir -p "$WORK_DIR/prefix/lib/seen/runtime/$(dirname "$relative")"
    cp "$ROOT_DIR/$runtime_file" \
        "$WORK_DIR/prefix/lib/seen/runtime/$relative"
done < <(git -C "$ROOT_DIR" ls-files -z -- seen_runtime)

clang -std=c11 -O1 -I "$ROOT_DIR/seen_runtime" \
    "$RUNTIME_FIXTURE" "$RUNTIME" -pthread -ldl -lm \
    -o "$WORK_DIR/runtime-contract" >"$WORK_DIR/runtime-compile.log" \
    2>&1 || {
    tail -n 160 "$WORK_DIR/runtime-compile.log" >&2 || true
    fail runtime-contract-compile
}
env -i SEEN_DETERMINISTIC=1 SOURCE_DATE_EPOCH=1700000000 \
    SEEN_DETERMINISTIC_SEED=1101 SEEN_HASH_SEED=1101 \
    CORE_004E_VISIBLE=granted-value LANG=C.UTF-8 LC_ALL=C.UTF-8 TZ=UTC \
    "$WORK_DIR/runtime-contract" captured-argument \
    "$WORK_DIR/path-root" "$WORK_DIR/path-root/child" \
    "$WORK_DIR/path-root-sibling" "$WORK_DIR/path-root/escape" \
    "$WORK_DIR/path-root/inside-link" "$WORK_DIR/path-root/missing" \
    >"$WORK_DIR/runtime.out" 2>"$WORK_DIR/runtime.err" || {
    cat "$WORK_DIR/runtime.out" >&2 || true
    cat "$WORK_DIR/runtime.err" >&2 || true
    fail runtime-contract
}
grep -Fq 'PASS: CORE-004E bounded runtime snapshot and path containment' \
    "$WORK_DIR/runtime.out" || fail runtime-contract-result

set +e
env -i SEEN_DETERMINISTIC=1 SOURCE_DATE_EPOCH=invalid \
    SEEN_DETERMINISTIC_SEED=1101 SEEN_HASH_SEED=1101 \
    LANG=C.UTF-8 LC_ALL=C.UTF-8 TZ=UTC \
    "$WORK_DIR/runtime-contract" captured-argument \
    "$WORK_DIR/path-root" "$WORK_DIR/path-root/child" \
    "$WORK_DIR/path-root-sibling" "$WORK_DIR/path-root/escape" \
    "$WORK_DIR/path-root/inside-link" "$WORK_DIR/path-root/missing" \
    >"$WORK_DIR/runtime-invalid.out" 2>"$WORK_DIR/runtime-invalid.err"
runtime_invalid_status=$?
set -e
[ "$runtime_invalid_status" -eq 70 ] || fail runtime-invalid-status
grep -Fq 'core.004e.invalid' "$WORK_DIR/runtime-invalid.err" || \
    fail runtime-invalid-diagnostic

python3 - "$WORK_DIR/runtime-contract" "$WORK_DIR" <<'PY' || \
    fail runtime-input-bounds
import pathlib
import subprocess
import sys

binary = pathlib.Path(sys.argv[1])
work = pathlib.Path(sys.argv[2])
base_environment = {
    "SEEN_DETERMINISTIC": "1",
    "SOURCE_DATE_EPOCH": "1700000000",
    "SEEN_DETERMINISTIC_SEED": "1101",
    "SEEN_HASH_SEED": "1101",
    "LC_ALL": "C.UTF-8",
    "TZ": "UTC",
    "CORE_004E_VISIBLE": "granted-value",
}
path_arguments = [
    str(binary), "captured-argument", str(work / "path-root"),
    str(work / "path-root" / "child"), str(work / "path-root-sibling"),
    str(work / "path-root" / "escape"),
    str(work / "path-root" / "inside-link"),
    str(work / "path-root" / "missing"),
]

def run(arguments: list[str], environment: dict[str, str]):
    return subprocess.run(arguments, env=environment, check=False,
                          capture_output=True, text=True)

def expect_invalid(label: str, arguments: list[str],
                   environment: dict[str, str]) -> None:
    completed = run(arguments, environment)
    if (completed.returncode != 70 or
            "core.004e.invalid" not in completed.stderr):
        raise SystemExit(
            f"{label} did not fail closed: {completed.returncode}: "
            f"{completed.stderr!r}")

over_bound_environment = dict(base_environment)
over_bound_environment.update({f"CORE_004E_EXTRA_{index}": "x"
                               for index in range(122)})
if len(over_bound_environment) != 129:
    raise SystemExit("environment-bound fixture is not exact")
expect_invalid("over-bound environment", path_arguments,
               over_bound_environment)

maximum_environment = dict(base_environment)
maximum_environment.update({f"CORE_004E_EXTRA_{index}": "x"
                            for index in range(121)})
if len(maximum_environment) != 128:
    raise SystemExit("maximum environment fixture is not exact")
maximum_environment_result = run(path_arguments, maximum_environment)
if (maximum_environment_result.returncode != 0 or
        "PASS: CORE-004E bounded runtime snapshot and path containment" not in
        maximum_environment_result.stdout):
    raise SystemExit(
        "maximum environment snapshot failed: "
        f"{maximum_environment_result.returncode}: "
        f"{maximum_environment_result.stderr!r}")

long_environment = dict(base_environment)
long_environment["CORE_004E_TOO_LARGE"] = "x" * 4097
expect_invalid("over-bound environment value", path_arguments,
               long_environment)
long_environment_name = dict(base_environment)
long_environment_name["X" * 129] = "x"
expect_invalid("over-bound environment name", path_arguments,
               long_environment_name)
expect_invalid("over-bound argument value", [str(binary), "x" * 4097],
               base_environment)
expect_invalid("over-bound argument count",
               [str(binary), "--max-arguments"] + ["a"] * 256,
               base_environment)

maximum_arguments = run(
    [str(binary), "--max-arguments"] + ["a"] * 255,
    base_environment)
if (maximum_arguments.returncode != 0 or
        "PASS: CORE-004E maximum application-argument snapshot" not in
        maximum_arguments.stdout):
    raise SystemExit(
        "maximum application-argument snapshot failed: "
        f"{maximum_arguments.returncode}: {maximum_arguments.stderr!r}")
PY

bash "$ATTESTED_SEEN" "$COMPILER" check "$WORK_DIR/source/main.seen" \
    >"$WORK_DIR/source-check.log" 2>&1 || {
    tail -n 120 "$WORK_DIR/source-check.log" >&2 || true
    fail source-check
}
grep -Fq '[OK] Check passed' "$WORK_DIR/source-check.log" || \
    fail source-check-result

set +e
SEEN_DETERMINISTIC=1 SOURCE_DATE_EPOCH=1700000000 \
    SEEN_DETERMINISTIC_SEED=1101 SEEN_HASH_SEED=1101 \
    LANG=C.UTF-8 LC_ALL=C.UTF-8 TZ=UTC \
    bash "$ATTESTED_SEEN" --deterministic-environment "$COMPILER" check \
    "$FIXTURES/CORE-004E_ffi_bypass_rejected/main.seen" --deterministic \
    >"$WORK_DIR/ffi-bypass.log" 2>&1
ffi_status=$?
set -e
[ "$ffi_status" -ne 0 ] || fail ffi-bypass-accepted
grep -Fq 'core.004d.unsafe-ffi' "$WORK_DIR/ffi-bypass.log" || {
    cat "$WORK_DIR/ffi-bypass.log" >&2 || true
    fail ffi-bypass-diagnostic
}

bash "$ATTESTED_SEEN" "$COMPILER" compile \
    "$WORK_DIR/source/main.seen" "$WORK_DIR/context-contract" \
    --fast --no-cache --no-fork --jobs 1 --opt-jobs 1 \
    >"$WORK_DIR/compile.log" 2>&1 || {
    tail -n 160 "$WORK_DIR/compile.log" >&2 || true
    fail native-compile
}
for repetition in 1 2; do
    SEEN_DETERMINISTIC=1 SOURCE_DATE_EPOCH=1700000000 \
        SEEN_DETERMINISTIC_SEED=1101 SEEN_HASH_SEED=1101 \
        CORE_004E_VISIBLE=granted-value LANG=C.UTF-8 LC_ALL=C.UTF-8 TZ=UTC \
        "$WORK_DIR/context-contract" >"$WORK_DIR/run-$repetition.out" \
        2>"$WORK_DIR/run-$repetition.err" || {
        cat "$WORK_DIR/run-$repetition.out" >&2 || true
        cat "$WORK_DIR/run-$repetition.err" >&2 || true
        fail "native-run-$repetition"
    }
done
cmp -s "$WORK_DIR/run-1.out" "$WORK_DIR/run-2.out" || fail run-parity-output
cmp -s "$WORK_DIR/run-1.err" "$WORK_DIR/run-2.err" || fail run-parity-error

test -f "$WORK_DIR/prefix/lib/seen/std/determinism/policy.seen" || \
    fail installed-policy
test -f "$WORK_DIR/prefix/lib/seen/std/determinism/context.seen" || \
    fail installed-context
cmp -s "$POLICY" \
    "$WORK_DIR/prefix/lib/seen/std/determinism/policy.seen" || \
    fail installed-policy-identity
cmp -s "$CONTEXT" \
    "$WORK_DIR/prefix/lib/seen/std/determinism/context.seen" || \
    fail installed-context-identity
chmod -R a-w "$WORK_DIR/prefix"
prefix_before=$(find "$WORK_DIR/prefix" -type f -print0 | sort -z | \
    xargs -0 sha256sum | sha256sum | awk '{print $1}')
(cd "$WORK_DIR/source" && \
    SEEN_PROJECT_ROOT="$WORK_DIR/source" \
    SEEN_ARTIFACT_ROOT="$WORK_DIR/source/.seen/agent-tools" \
    "$WORK_DIR/prefix/bin/seen" check main.seen) \
    >"$WORK_DIR/installed-check.log" 2>&1 || {
    tail -n 120 "$WORK_DIR/installed-check.log" >&2 || true
    fail installed-compiler-check
}
prefix_after=$(find "$WORK_DIR/prefix" -type f -print0 | sort -z | \
    xargs -0 sha256sum | sha256sum | awk '{print $1}')
[ "$prefix_before" = "$prefix_after" ] || fail installed-prefix-mutated

for name in \
    CORE-004E_epoch_happy CORE-004E_seed_happy CORE-004E_missing_epoch \
    CORE-004E_invalid_seed CORE-004E_env_denied CORE-004E_locale_pinned \
    CORE-004E_external_input_denied CORE-004E_run_parity CORE-004E_cancel; do
    rg -Fq "$name" "$FIXTURES/context_contract.seen" || fail "$name"
    echo "PASS: $name (source fixture)"
done
echo "PASS: CORE-004E_ffi_bypass_rejected (compiler rejection)"
echo "PASS: CORE-004E_installed_payload (read-only installed compiler payload)"
echo "PASS: CORE-004E deterministic execution-context static contract"
