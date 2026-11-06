#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -P)
COMPILER=${COMPILER:-${SEEN_BIN:-$ROOT_DIR/compiler_seen/target/seen}}
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-core-004e
FIXTURES="$ROOT_DIR/tests/fixtures/core-004e"
POLICY="$ROOT_DIR/seen_std/src/determinism/policy.seen"
CONTEXT="$ROOT_DIR/seen_std/src/determinism/context.seen"
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
test -f "$FIXTURES/arguments_contract.seen" || fail "argument fixture is missing"
test -f "$RUNTIME_FIXTURE" || fail "runtime fixture is missing"
test -f "$RUNTIME" || fail "runtime source is missing"
test -f "$FIXTURES/CORE-004E_ffi_bypass_rejected/main.seen" || \
    fail "foreign-call rejection fixture is missing"

python3 -m json.tool "$FIXTURES/cases.json" >/dev/null || fail fixture-json
python3 - "$FIXTURES/cases.json" <<'PY' || exit 1
import json
import sys

required = {
    ("native-runtime", "1700000000"),
    ("native-runtime", "xorshift64-v1"),
    ("native-error", "core.004e.missing_epoch"),
    ("native-error", "core.004e.invalid_seed"),
    ("native-error", "core.004e.env_denied"),
    ("native-runtime", "C.UTF-8/UTF-8"),
    ("native-error", "core.004e.external_input_denied"),
    ("compiler-rejection", "core.004d.unsafe-ffi"),
    ("repeat-run", "identical-output"),
    ("native-error", "core.004e.cancelled"),
    ("installed-layout", "lib/seen/std/determinism/context.seen"),
}
with open(sys.argv[1], "r", encoding="utf-8") as handle:
    document = json.load(handle)
if document.get("schema") != "seen-core-004e-fixtures-v1":
    raise SystemExit("fixture schema mismatch")
if not required.issubset({(case.get("kind"), case.get("expected"))
                          for case in document.get("cases", [])}):
    raise SystemExit("fixture semantic expectations are incomplete")
for case in document["cases"]:
    if not case.get("kind") or not case.get("expected"):
        raise SystemExit("fixture expectation is incomplete")
PY


if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != "1" ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- \
        bash "$0" "$@"
fi
COMPILER=${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
ATTESTED_SEEN=${SEEN_ATTESTED_COMPILER_RUNNER:?}
ARTIFACT_ROOT=${SEEN_ARTIFACT_ROOT:?}
if [ -n "${SEEN_PACKAGE_CLIENT:-}" ]; then
    PACKAGE_CLIENT=$SEEN_PACKAGE_CLIENT
else
    PACKAGE_CLIENT=""
    for candidate in "$(dirname "$COMPILER")/seen-pkg" \
        "$ROOT_DIR/bin/seen-pkg" "$ROOT_DIR/compiler_seen/target/seen-pkg" \
        "$ROOT_DIR/tools/seen-pkg/bin/seen-pkg"; do
        if [ -f "$candidate" ] && [ -x "$candidate" ] && [ ! -L "$candidate" ]; then
            PACKAGE_CLIENT=$candidate
            break
        fi
    done
fi
[ -f "$PACKAGE_CLIENT" ] && [ -x "$PACKAGE_CLIENT" ] && \
    [ ! -L "$PACKAGE_CLIENT" ] || fail "selected package client is unavailable or unsafe"
COMPATIBILITY="$ROOT_DIR/releases/compatibility-manifest.json"
[ -f "$COMPATIBILITY" ] && [ ! -L "$COMPATIBILITY" ] || \
    fail "package compatibility manifest is unavailable or unsafe"
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
    "$WORK_DIR/consumer/.seen/agent-tools" "$WORK_DIR/path-root/child" \
    "$WORK_DIR/path-root-sibling" "$WORK_DIR/path-outside"
ln -s "$WORK_DIR/path-outside" "$WORK_DIR/path-root/escape"
ln -s "$WORK_DIR/path-root/child" "$WORK_DIR/path-root/inside-link"
cp "$POLICY" "$WORK_DIR/source/determinism/policy.seen"
cp "$CONTEXT" "$WORK_DIR/source/determinism/context.seen"
cp "$FIXTURES/context_contract.seen" "$WORK_DIR/source/main.seen"
cp "$COMPILER" "$WORK_DIR/prefix/bin/seen"
cp "$PACKAGE_CLIENT" "$WORK_DIR/prefix/bin/seen-pkg"
cp "$COMPATIBILITY" "$WORK_DIR/prefix/bin/compatibility-manifest.json"
cp "$FIXTURES/context_contract.seen" "$WORK_DIR/consumer/main.seen"
cat >"$WORK_DIR/consumer/Seen.toml" <<'TOML'
manifest-version = 1

[project]
name = "core_004e_installed_consumer"
version = "0.1.0"
language = "en"
modules = ["main.seen"]
TOML
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
env -i "$WORK_DIR/runtime-contract" --disabled-views \
    >"$WORK_DIR/runtime-disabled.out" 2>"$WORK_DIR/runtime-disabled.err" || \
    fail runtime-disabled-views
env -i SEEN_DETERMINISTIC=1 SOURCE_DATE_EPOCH=1700000000 \
    SEEN_DETERMINISTIC_SEED=1101 SEEN_HASH_SEED=1101 \
    CORE_004E_VISIBLE= LANG=C.UTF-8 LC_ALL=C.UTF-8 TZ=UTC \
    "$WORK_DIR/runtime-contract" --empty-view \
    >"$WORK_DIR/runtime-empty.out" 2>"$WORK_DIR/runtime-empty.err" || \
    fail runtime-present-empty-view
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
if maximum_environment_result.returncode != 0:
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
if maximum_arguments.returncode != 0:
    raise SystemExit(
        "maximum application-argument snapshot failed: "
        f"{maximum_arguments.returncode}: {maximum_arguments.stderr!r}")
PY

bash "$ATTESTED_SEEN" "$COMPILER" check "$WORK_DIR/source/main.seen" \
    >"$WORK_DIR/source-check.log" 2>&1 || {
    tail -n 120 "$WORK_DIR/source-check.log" >&2 || true
    fail source-check
}

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
    --fast --no-cache --no-fork \
    >"$WORK_DIR/compile.log" 2>&1 || {
    tail -n 160 "$WORK_DIR/compile.log" >&2 || true
    fail native-compile
}
for repetition in 1 2; do
    env -i SEEN_DETERMINISTIC=1 SOURCE_DATE_EPOCH=1700000000 \
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

for mode in fast thin-lto; do
    flags=(--fast)
    if [ "$mode" = thin-lto ]; then
        flags=(--release --lto=thin)
    fi
    bash "$ATTESTED_SEEN" "$COMPILER" compile \
        "$FIXTURES/arguments_contract.seen" "$WORK_DIR/arguments-$mode" \
        "${flags[@]}" --no-cache --no-fork \
        >"$WORK_DIR/arguments-$mode-compile.log" 2>&1 || {
        tail -n 160 "$WORK_DIR/arguments-$mode-compile.log" >&2 || true
        fail "arguments-$mode-compile"
    }
    python3 - "$ROOT_DIR" "$WORK_DIR" "$mode" "$COMPILER" <<'PY' || fail "arguments-$mode-contract"
import pathlib
import re
import subprocess
import sys

root = pathlib.Path(sys.argv[1])
work = pathlib.Path(sys.argv[2])
mode = sys.argv[3]
compiler = pathlib.Path(sys.argv[4])
sys.path.insert(0, str(root / "scripts"))
import core_004e_crash_witness as crash
observations = []
for label in ("short", "long"):
    argv, environment = crash.invocation(work, mode, label)
    prepared = None
    try:
        prepared = crash.prepare(root, work, mode, label, compiler)
    except (OSError, ValueError):
        print("CORE-004E crash identity snapshot unavailable", file=sys.stderr)
    result = subprocess.run(
        argv, env=environment, capture_output=True, timeout=120)
    if result.returncode < 0 and prepared is not None:
        try:
            crash.retain(root, work, mode, label, prepared, result.returncode)
        except (OSError, ValueError):
            # Evidence failure must never hide or replace the original failure.
            print("CORE-004E crash witness publication unavailable", file=sys.stderr)
    (work / f"arguments-{mode}-{label}.out").write_bytes(result.stdout)
    (work / f"arguments-{mode}-{label}.err").write_bytes(result.stderr)
    # The fixture exits nonzero unless the public view keeps argument zero and
    # the missing high-index grant returns the typed core.004e.invalid error.
    match = re.fullmatch(
        rb"ARGUMENT_VIEW=kept\nINVALID_RETAINED_BYTES=([0-9]+)\n",
        result.stdout)
    if result.returncode != 0 or result.stderr or match is None:
        raise SystemExit(
            f"{mode}/{label} argument contract failed: {result.returncode}: "
            f"{result.stdout!r}: {result.stderr!r}")
    observations.append(int(match.group(1)))
if observations[0] != observations[1]:
    raise SystemExit(
        f"{mode} unrelated argv changed failed-context retained bytes: "
        f"short={observations[0]}, long={observations[1]}")
print(f"PASS: CORE-004E argument retention ({mode}: "
      f"short={observations[0]}, long={observations[1]})")
PY
done

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
cmp -s "$PACKAGE_CLIENT" "$WORK_DIR/prefix/bin/seen-pkg" || \
    fail installed-package-client-identity
cmp -s "$COMPATIBILITY" "$WORK_DIR/prefix/bin/compatibility-manifest.json" || \
    fail installed-compatibility-identity
chmod -R a-w "$WORK_DIR/prefix"
prefix_digest() {
    python3 - "$WORK_DIR/prefix" <<'PY'
import hashlib
import pathlib
import stat
import sys

prefix = pathlib.Path(sys.argv[1])
digest = hashlib.sha256()
for path in [prefix, *sorted(prefix.rglob("*"))]:
    mode = path.lstat().st_mode
    if path.is_symlink() or not (stat.S_ISDIR(mode) or stat.S_ISREG(mode)):
        raise SystemExit(f"unsafe installed payload entry: {path}")
    if mode & 0o222:
        raise SystemExit(f"writable installed payload entry: {path}")
    digest.update(str(path.relative_to(prefix)).encode() + b"\0")
    digest.update(str(mode).encode() + b"\0")
    if stat.S_ISREG(mode):
        digest.update(hashlib.sha256(path.read_bytes()).digest())
print(digest.hexdigest())
PY
}
prefix_before=$(prefix_digest) || fail installed-prefix-snapshot
# A copied executable needs its own canonical compiler binding. Re-enter the
# capped runner rather than replacing the parent's attestation markers.
bash "$CAPPED_ENTRY" "$SCOPE-installed" \
    --compiler "$WORK_DIR/prefix/bin/seen" -- \
    bash -c '
        set -euo pipefail
        runner=$1
        compiler=$2
        consumer=$3
        cd "$consumer"
        bash "$runner" --consumer-context "$consumer" "$compiler" \
            check "$consumer/main.seen"
        bash "$runner" --consumer-context "$consumer" "$compiler" \
            compile "$consumer/main.seen" "$consumer/context-contract" \
            --fast --no-cache --no-fork
    ' bash "$ATTESTED_SEEN" "$WORK_DIR/prefix/bin/seen" "$WORK_DIR/consumer" \
    >"$WORK_DIR/installed-check.log" 2>&1 || {
    tail -n 160 "$WORK_DIR/installed-check.log" >&2 || true
    fail installed-compiler-check
}
env -i SEEN_DETERMINISTIC=1 SOURCE_DATE_EPOCH=1700000000 \
    SEEN_DETERMINISTIC_SEED=1101 SEEN_HASH_SEED=1101 \
    CORE_004E_VISIBLE=granted-value LANG=C.UTF-8 LC_ALL=C.UTF-8 TZ=UTC \
    "$WORK_DIR/consumer/context-contract" >"$WORK_DIR/installed-run.out" \
    2>"$WORK_DIR/installed-run.err" || {
    cat "$WORK_DIR/installed-run.out" >&2 || true
    cat "$WORK_DIR/installed-run.err" >&2 || true
    fail installed-native-contract
}
cmp -s "$WORK_DIR/run-1.out" "$WORK_DIR/installed-run.out" || \
    fail installed-native-output
cmp -s "$WORK_DIR/run-1.err" "$WORK_DIR/installed-run.err" || \
    fail installed-native-error
prefix_after=$(prefix_digest) || fail installed-prefix-resnapshot
[ "$prefix_before" = "$prefix_after" ] || fail installed-prefix-mutated

echo "PASS: CORE-004E_ffi_bypass_rejected (compiler rejection)"
echo "PASS: CORE-004E_installed_payload (read-only installed compiler payload)"
echo "PASS: CORE-004E deterministic execution-context native contract"
