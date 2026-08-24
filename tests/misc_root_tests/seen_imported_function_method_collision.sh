#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -P)"
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-imported-function-method-collision
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != "1" ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- \
        bash "$0" "$@"
fi
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/seen-import-method-collision.XXXXXX")"

cleanup() {
    local status=$?
    case "$TMP_DIR" in
        "$SEEN_ARTIFACT_ROOT"/seen-import-method-collision.*)
            if [ -d "$TMP_DIR" ] && [ ! -L "$TMP_DIR" ] &&
                [ "$(dirname -- "$TMP_DIR")" = "$SEEN_ARTIFACT_ROOT" ]; then
                rm -rf -- "$TMP_DIR"
            else
                echo "ERROR: refusing to clean unsafe collision regression path: $TMP_DIR" >&2
                status=1
            fi
            ;;
        *) status=1 ;;
    esac
    trap - EXIT
    exit "$status"
}
trap cleanup EXIT

run_logged() {
    local label=$1
    local log=$2
    shift 2
    local status=0

    "$@" >"$log" 2>&1 || status=$?
    if [ "$status" -ne 0 ]; then
        echo "FAIL: $label exited with status $status" >&2
        tail -c 32768 "$log" >&2 || true
        return "$status"
    fi
}

mkdir -p "$TMP_DIR/target"
cat >"$TMP_DIR/math_utils.seen" <<'SEEN'
type Scalar = Float

@trait
pub class ScalarTrait {
    fun value(this: ScalarTrait) r: Int
}

pub class ScalarBase {
}

pub fun relu(value: Float) r: Float {
    if value < 0.0 { return 0.0 }
    return value
}

pub fun takesInt(value: Int) r: Int {
    return value
}

pub fun aliasRelu(value: Scalar) r: Scalar {
    return value
}

pub fun acceptsBase(value: ScalarBase) r: Int {
    return 7
}

pub fun acceptsDyn(value: dyn ScalarTrait) r: Int {
    return 9
}
SEEN
cat >"$TMP_DIR/main.seen" <<'SEEN'
import math_utils.{relu, acceptsBase, ScalarBase}

class ScalarChild extends ScalarBase {
}

class Tensor {
    var values: Array<Float>

    static fun one(value: Float) r: Tensor {
        let values = Array<Float>()
        values.push(value)
        return Tensor{values: values}
    }

    fun relu() r: Tensor {
        let out = Array<Float>()
        out.push(relu(this.values[0]))
        return Tensor{values: out}
    }
}

fun main() r: Int {
    let negative = Tensor.one(-2.0).relu()
    if negative.values[0] < -0.1 or negative.values[0] > 0.1 { return 1 }
    let positive = Tensor.one(3.5).relu()
    if positive.values[0] < 3.4 or positive.values[0] > 3.6 { return 2 }
    let promoted = relu(4)
    if promoted < 3.9 or promoted > 4.1 { return 3 }
    if acceptsBase(ScalarChild{}) != 7 { return 4 }
    return 0
}
SEEN
cat >"$TMP_DIR/valid_alias.seen" <<'SEEN'
import math_utils.{aliasRelu}

// This caller-local alias deliberately collides with math_utils.Scalar. The
// imported signature must be checked in its own module and must not be
// rewritten through this unrelated alias.
type Scalar = Int

fun validAliasedCall() r: Float {
    return aliasRelu(2.5)
}

fun main() r: Int { return 0 }
SEEN
cat >"$TMP_DIR/invalid.seen" <<'SEEN'
import math_utils.{relu}

class Tensor {
    fun relu() r: Tensor { return this }

    fun invalidGlobalCall() r: Float {
        return relu("not-a-float")
    }
}

fun main() r: Int { return 0 }
SEEN
cat >"$TMP_DIR/invalid_numeric.seen" <<'SEEN'
import math_utils.{takesInt}

fun invalidNumericCall() r: Int {
    return takesInt(1.5)
}

fun main() r: Int { return 0 }
SEEN
cat >"$TMP_DIR/invalid_class.seen" <<'SEEN'
import math_utils.{relu}

class Tensor {
    var values: Array<Float>
}

fun invalidClassCall() r: Float {
    return relu(Tensor{values: Array<Float>()})
}

fun main() r: Int { return 0 }
SEEN
cat >"$TMP_DIR/invalid_alias.seen" <<'SEEN'
import math_utils.{aliasRelu}

// This unrelated caller binding must never control the imported parameter's
// alias. math_utils.Scalar is Float, so this call is an E015 mismatch.
type Scalar = Int

fun invalidAliasedCall() r: Float {
    return aliasRelu("not-a-float")
}

fun main() r: Int { return 0 }
SEEN
cat >"$TMP_DIR/invalid_dyn.seen" <<'SEEN'
import math_utils.{acceptsDyn}

fun invalidDynamicTraitCall() r: Int {
    return acceptsDyn(1.5)
}

fun main() r: Int { return 0 }
SEEN

run_logged "valid imported-function semantic check" "$TMP_DIR/check.log" \
    timeout --foreground --kill-after=10s 600s \
    bash "$ATTESTED_SEEN" "$COMPILER" check "$TMP_DIR/main.seen"
run_logged "valid module-scoped alias semantic check" \
    "$TMP_DIR/valid-alias-check.log" \
    timeout --foreground --kill-after=10s 600s \
    bash "$ATTESTED_SEEN" "$COMPILER" check "$TMP_DIR/valid_alias.seen"
run_logged "valid imported-function compile" "$TMP_DIR/compile.log" \
    timeout --foreground --kill-after=10s 600s \
    bash "$ATTESTED_SEEN" "$COMPILER" compile "$TMP_DIR/main.seen" \
    "$TMP_DIR/target/collision" --fast --no-cache
run_logged "valid imported-function executable" "$TMP_DIR/run.log" \
    timeout --foreground --kill-after=5s 30s "$TMP_DIR/target/collision"

invalid_status=0
timeout --foreground --kill-after=10s 600s \
    bash "$ATTESTED_SEEN" "$COMPILER" check "$TMP_DIR/invalid.seen" \
    >"$TMP_DIR/invalid-check.log" 2>&1 || invalid_status=$?
if [ "$invalid_status" -eq 0 ]; then
    echo "FAIL: invalid imported-function argument passed semantic check" >&2
    tail -c 32768 "$TMP_DIR/invalid-check.log" >&2 || true
    exit 1
fi
if [ "$invalid_status" -ne 1 ]; then
    echo "FAIL: invalid imported-function check exited with unexpected status $invalid_status" >&2
    tail -c 32768 "$TMP_DIR/invalid-check.log" >&2 || true
    exit 1
fi
if grep -Eiq 'segmentation fault|invalid llvm|llvm error|internal compiler error|traceback|panicked|signal [0-9]+' \
    "$TMP_DIR/invalid-check.log"; then
    echo "FAIL: invalid imported-function call escaped to LLVM/codegen failure" >&2
    tail -c 32768 "$TMP_DIR/invalid-check.log" >&2 || true
    exit 1
fi
grep -Eiq 'error.*E015.*argument 1 to imported function' \
    "$TMP_DIR/invalid-check.log" || {
    echo "FAIL: invalid imported-function call lacked E015" >&2
    tail -c 32768 "$TMP_DIR/invalid-check.log" >&2 || true
    exit 1
}
grep -Fq 'type `String`, expected `Float`' \
    "$TMP_DIR/invalid-check.log" || {
    echo "FAIL: imported-function mismatch diagnostic lacked actual/expected types" >&2
    tail -c 32768 "$TMP_DIR/invalid-check.log" >&2 || true
    exit 1
}
grep -Fq 'invalid.seen' "$TMP_DIR/invalid-check.log" || {
    echo "FAIL: invalid imported-function diagnostic lacked source context" >&2
    tail -c 32768 "$TMP_DIR/invalid-check.log" >&2 || true
    exit 1
}

invalid_compile_status=0
timeout --foreground --kill-after=10s 600s \
    bash "$ATTESTED_SEEN" "$COMPILER" compile "$TMP_DIR/invalid.seen" \
    "$TMP_DIR/target/invalid" --fast --no-cache \
    >"$TMP_DIR/invalid-compile.log" 2>&1 || invalid_compile_status=$?
if [ "$invalid_compile_status" -ne 1 ]; then
    echo "FAIL: invalid imported-function compile exited with status $invalid_compile_status" >&2
    tail -c 32768 "$TMP_DIR/invalid-compile.log" >&2 || true
    exit 1
fi
if grep -Eiq 'segmentation fault|invalid llvm|llvm error|optimizer|internal compiler error|traceback|panicked|signal [0-9]+' \
    "$TMP_DIR/invalid-compile.log"; then

    echo "FAIL: invalid imported-function compile reached LLVM/optimizer failure" >&2
    tail -c 32768 "$TMP_DIR/invalid-compile.log" >&2 || true
    exit 1
fi
grep -Eiq 'error.*E015.*argument 1 to imported function' \
    "$TMP_DIR/invalid-compile.log" || {
    echo "FAIL: invalid imported-function compile lacked E015" >&2
    tail -c 32768 "$TMP_DIR/invalid-compile.log" >&2 || true
    exit 1
}
if [ -e "$TMP_DIR/target/invalid" ]; then
    echo "FAIL: invalid imported-function compile left an executable" >&2
    exit 1
fi

numeric_status=0
timeout --foreground --kill-after=10s 600s \
    bash "$ATTESTED_SEEN" "$COMPILER" check "$TMP_DIR/invalid_numeric.seen" \
    >"$TMP_DIR/invalid-numeric-check.log" 2>&1 || numeric_status=$?
if [ "$numeric_status" -ne 1 ]; then
    echo "FAIL: Float-to-Int imported call exited with status $numeric_status" >&2
    tail -c 32768 "$TMP_DIR/invalid-numeric-check.log" >&2 || true
    exit 1
fi
if grep -Eiq 'segmentation fault|invalid llvm|llvm error|internal compiler error|traceback|panicked|signal [0-9]+' \
    "$TMP_DIR/invalid-numeric-check.log"; then

    echo "FAIL: Float-to-Int imported call escaped to LLVM/codegen failure" >&2
    tail -c 32768 "$TMP_DIR/invalid-numeric-check.log" >&2 || true
    exit 1
fi
grep -Eiq 'error.*E015.*argument 1 to imported function' \
    "$TMP_DIR/invalid-numeric-check.log" || {
    echo "FAIL: Float-to-Int imported call lacked E015" >&2
    tail -c 32768 "$TMP_DIR/invalid-numeric-check.log" >&2 || true
    exit 1
}
grep -Fq 'type `Float`, expected `Int`' \
    "$TMP_DIR/invalid-numeric-check.log" || {
    echo "FAIL: Float-to-Int diagnostic lacked actual/expected types" >&2
    tail -c 32768 "$TMP_DIR/invalid-numeric-check.log" >&2 || true
    exit 1
}

class_status=0
timeout --foreground --kill-after=10s 600s \
    bash "$ATTESTED_SEEN" "$COMPILER" check "$TMP_DIR/invalid_class.seen" \
    >"$TMP_DIR/invalid-class-check.log" 2>&1 || class_status=$?
if [ "$class_status" -ne 1 ]; then
    echo "FAIL: class-to-Float imported call exited with status $class_status" >&2
    tail -c 32768 "$TMP_DIR/invalid-class-check.log" >&2 || true
    exit 1
fi
if grep -Eiq 'segmentation fault|invalid llvm|llvm error|internal compiler error|traceback|panicked|signal [0-9]+' \
    "$TMP_DIR/invalid-class-check.log"; then

    echo "FAIL: class-to-Float imported call escaped to LLVM/codegen failure" >&2
    tail -c 32768 "$TMP_DIR/invalid-class-check.log" >&2 || true
    exit 1
fi
grep -Eiq 'error.*E015.*argument 1 to imported function' \
    "$TMP_DIR/invalid-class-check.log" || {
    echo "FAIL: class-to-Float imported call lacked E015" >&2
    tail -c 32768 "$TMP_DIR/invalid-class-check.log" >&2 || true
    exit 1
}
grep -Fq 'type `Tensor`, expected `Float`' \
    "$TMP_DIR/invalid-class-check.log" || {
    echo "FAIL: class-to-Float diagnostic lacked actual/expected types" >&2
    tail -c 32768 "$TMP_DIR/invalid-class-check.log" >&2 || true
    exit 1
}

alias_status=0
timeout --foreground --kill-after=10s 600s \
    bash "$ATTESTED_SEEN" "$COMPILER" check "$TMP_DIR/invalid_alias.seen" \
    >"$TMP_DIR/invalid-alias-check.log" 2>&1 || alias_status=$?
if [ "$alias_status" -ne 1 ]; then
    echo "FAIL: imported alias mismatch exited with status $alias_status" >&2
    tail -c 32768 "$TMP_DIR/invalid-alias-check.log" >&2 || true
    exit 1
fi
if grep -Eiq 'segmentation fault|invalid llvm|llvm error|internal compiler error|traceback|panicked|signal [0-9]+' \
    "$TMP_DIR/invalid-alias-check.log"; then

    echo "FAIL: imported alias mismatch escaped to LLVM/codegen failure" >&2
    tail -c 32768 "$TMP_DIR/invalid-alias-check.log" >&2 || true
    exit 1
fi
grep -Eiq 'error.*E015.*argument 1 to imported function' \
    "$TMP_DIR/invalid-alias-check.log" || {
    echo "FAIL: imported alias mismatch lacked E015" >&2
    tail -c 32768 "$TMP_DIR/invalid-alias-check.log" >&2 || true
    exit 1
}
grep -Fq 'type `String`, expected `Scalar`' \
    "$TMP_DIR/invalid-alias-check.log" || {
    echo "FAIL: imported alias diagnostic lacked source-level types" >&2
    tail -c 32768 "$TMP_DIR/invalid-alias-check.log" >&2 || true
    exit 1
}

dyn_status=0
timeout --foreground --kill-after=10s 600s \
    bash "$ATTESTED_SEEN" "$COMPILER" check "$TMP_DIR/invalid_dyn.seen" \
    >"$TMP_DIR/invalid-dyn-check.log" 2>&1 || dyn_status=$?
if [ "$dyn_status" -ne 1 ]; then
    echo "FAIL: scalar-to-dyn imported call exited with status $dyn_status" >&2
    tail -c 32768 "$TMP_DIR/invalid-dyn-check.log" >&2 || true
    exit 1
fi
if grep -Eiq 'segmentation fault|invalid llvm|llvm error|internal compiler error|traceback|panicked|signal [0-9]+' \
    "$TMP_DIR/invalid-dyn-check.log"; then

    echo "FAIL: scalar-to-dyn imported call escaped to LLVM/codegen failure" >&2
    tail -c 32768 "$TMP_DIR/invalid-dyn-check.log" >&2 || true
    exit 1
fi
grep -Eiq 'error.*E015.*argument 1 to imported function' \
    "$TMP_DIR/invalid-dyn-check.log" || {
    echo "FAIL: scalar-to-dyn imported call lacked E015" >&2
    tail -c 32768 "$TMP_DIR/invalid-dyn-check.log" >&2 || true
    exit 1
}
grep -Fq 'type `Float`, expected `dyn ScalarTrait`' \
    "$TMP_DIR/invalid-dyn-check.log" || {
    echo "FAIL: scalar-to-dyn diagnostic lacked source-level types" >&2
    tail -c 32768 "$TMP_DIR/invalid-dyn-check.log" >&2 || true
    exit 1
}

echo "PASS: imported global function remains distinct from same-named method"
