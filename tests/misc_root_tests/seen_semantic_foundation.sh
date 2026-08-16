#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd -P)"
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen-dev}"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-semantic-foundation
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != "1" ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- \
        bash "$0" "$@"
fi
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
ARTIFACT_ROOT="$SEEN_ARTIFACT_ROOT"

if [ -z "$ARTIFACT_ROOT" ] || [ ! -d "$ARTIFACT_ROOT" ] ||
    [ -L "$ARTIFACT_ROOT" ]; then
    echo "FAIL: SEEN_ARTIFACT_ROOT must name an existing project-local directory" >&2
    exit 1
fi
if [ ! -x "$COMPILER" ]; then
    echo "FAIL: compiler is not executable: $COMPILER" >&2
    exit 1
fi

run_compiler() {
    bash "$ATTESTED_SEEN" "$COMPILER" "$@"
}

assert_check_stayed_frontend_only() {
    local log_path=$1
    if grep -Fq "Optimization pipeline:" "$log_path" ||
        grep -Fq "Build succeeded" "$log_path" ||
        grep -Fq "[Pass 2]" "$log_path"; then

        echo "FAIL: seen check entered LLVM/codegen" >&2
        tail -n 120 "$log_path" >&2 || true
        exit 1
    fi
}

WORK_DIR="$(mktemp -d "$ARTIFACT_ROOT/semantic-foundation.XXXXXX")"
cleanup() {
    local status=$?
    case "$WORK_DIR" in
        "$ARTIFACT_ROOT"/semantic-foundation.*)
            if [ -d "$WORK_DIR" ] && [ ! -L "$WORK_DIR" ]; then
                # Package views are intentionally read-only. Restore write
                # permission only on this test-owned tree without following
                # links, then remove the exact validated work directory.
                find -P "$WORK_DIR" -type f -exec chmod u+w -- {} + \
                    2>/dev/null || true
                find -P "$WORK_DIR" -depth -type d -exec chmod u+w -- {} + \
                    2>/dev/null || true
                rm -rf -- "$WORK_DIR" || true
            fi
            ;;
    esac
    return "$status"
}
trap cleanup EXIT

LEFT_DIR="$WORK_DIR/left"
RIGHT_DIR="$WORK_DIR/right"
CONSUMER_DIR="$WORK_DIR/consumer"
mkdir -p "$LEFT_DIR/src/models" "$RIGHT_DIR/src/models" \
    "$CONSUMER_DIR/src"

cat >"$LEFT_DIR/Seen.toml" <<'EOF'
manifest-version = 1

[project]
name = "left_models"
version = "0.1.0"

[dependencies]
EOF

cat >"$RIGHT_DIR/Seen.toml" <<'EOF'
manifest-version = 1

[project]
name = "right_models"
version = "0.1.0"

[dependencies]
EOF

cat >"$LEFT_DIR/src/models/user.seen" <<'EOF'
pub class Shared {
    value: Int
}
EOF

cat >"$LEFT_DIR/src/models/generic.seen" <<'EOF'
pub class Envelope<T> {
    value: T
}

pub class Matrix<T, const N: Int, comptime Mode: Int> {
    value: T
}

pub class Bundle<Head, ...Tail> {
    value: Head
}
EOF

cat >"$LEFT_DIR/src/models/broken.seen" <<'EOF'
pub fun importedBroken() r: Int {
    return missingImportedBinding
}
EOF

cat >"$RIGHT_DIR/src/models/user.seen" <<'EOF'
pub class Shared {
    label: String
}
EOF

cat >"$CONSUMER_DIR/Seen.toml" <<EOF
manifest-version = 1

[project]
name = "semantic_consumer"
version = "0.1.0"

[dependencies]
left = { path = "../left" }
right = { path = "../right" }
EOF

cat >"$CONSUMER_DIR/src/main.seen" <<'EOF'
import left.models.user.{Shared as LeftShared}
import right.models.user.{Shared as RightShared}

class Pair {
    left: LeftShared
    right: RightShared
}

fun main() r: Int {
    return 0
}
EOF

CHECK_SUCCESS_LOG="$WORK_DIR/check-success.log"
if ! run_compiler check "$CONSUMER_DIR/src/main.seen" \
    >"$CHECK_SUCCESS_LOG" 2>&1; then
    echo "FAIL: seen check rejected package-qualified imported types" >&2
    tail -n 120 "$CHECK_SUCCESS_LOG" >&2 || true
    exit 1
fi
if ! grep -Fq "[OK] Check passed" "$CHECK_SUCCESS_LOG"; then
    echo "FAIL: successful seen check did not report completion" >&2
    tail -n 120 "$CHECK_SUCCESS_LOG" >&2 || true
    exit 1
fi
assert_check_stayed_frontend_only "$CHECK_SUCCESS_LOG"

cat >"$CONSUMER_DIR/src/imported_invalid.seen" <<'EOF'
import left.models.broken.{importedBroken}

fun main() r: Int {
    return 0
}
EOF

CHECK_IMPORTED_LOG="$WORK_DIR/check-imported-invalid.log"
if run_compiler check "$CONSUMER_DIR/src/imported_invalid.seen" \
    >"$CHECK_IMPORTED_LOG" 2>&1; then
    echo "FAIL: seen check skipped semantic validation of an imported module" >&2
    exit 1
fi
if ! grep -Fq "E_BINDING_UNKNOWN" "$CHECK_IMPORTED_LOG" ||
    ! grep -Fq "broken.seen" "$CHECK_IMPORTED_LOG"; then

    echo "FAIL: imported-module semantic error was not attributed precisely" >&2
    tail -n 120 "$CHECK_IMPORTED_LOG" >&2 || true
    exit 1
fi
assert_check_stayed_frontend_only "$CHECK_IMPORTED_LOG"

SUCCESS_LOG="$WORK_DIR/success.log"
if ! run_compiler compile "$CONSUMER_DIR/src/main.seen" \
    "$WORK_DIR/semantic-success" --fast --no-cache \
    >"$SUCCESS_LOG" 2>&1; then
    echo "FAIL: package-qualified same-relative modules did not coexist" >&2
    tail -n 120 "$SUCCESS_LOG" >&2 || true
    exit 1
fi

cat >"$CONSUMER_DIR/src/unimported.seen" <<'EOF'
import left.models.user.{Shared as LeftShared}
import right.models.user.{Shared as RightShared}

class Broken {
    value: Shared
}

fun main() r: Int {
    return 0
}
EOF

CHECK_UNKNOWN_LOG="$WORK_DIR/check-unimported.log"
if run_compiler check "$CONSUMER_DIR/src/unimported.seen" \
    >"$CHECK_UNKNOWN_LOG" 2>&1; then
    echo "FAIL: seen check accepted an unimported package type" >&2
    exit 1
fi
if ! grep -Fq "E_TYPE_UNKNOWN" "$CHECK_UNKNOWN_LOG"; then
    echo "FAIL: seen check did not report E_TYPE_UNKNOWN" >&2
    tail -n 120 "$CHECK_UNKNOWN_LOG" >&2 || true
    exit 1
fi
assert_check_stayed_frontend_only "$CHECK_UNKNOWN_LOG"

FAILURE_LOG="$WORK_DIR/unimported.log"
if run_compiler compile "$CONSUMER_DIR/src/unimported.seen" \
    "$WORK_DIR/semantic-unimported" --fast --no-cache \
    >"$FAILURE_LOG" 2>&1; then
    echo "FAIL: unimported same-leaf package type bypassed import boundaries" >&2
    exit 1
fi
if ! grep -Fq "E_TYPE_UNKNOWN" "$FAILURE_LOG"; then
    echo "FAIL: unimported package type did not produce E_TYPE_UNKNOWN" >&2
    tail -n 120 "$FAILURE_LOG" >&2 || true
    exit 1
fi

cat >"$CONSUMER_DIR/src/bad_generic_arity.seen" <<'EOF'
import left.models.generic.{Envelope, Matrix, Bundle}

class BrokenArity {
    value: Envelope<Int, String>
    matrix: Matrix<Int, 4>
    bundle: Bundle
}

fun main() r: Int {
    return 0
}
EOF

CHECK_ARITY_LOG="$WORK_DIR/check-generic-arity.log"
if run_compiler check "$CONSUMER_DIR/src/bad_generic_arity.seen" \
    >"$CHECK_ARITY_LOG" 2>&1; then
    echo "FAIL: seen check accepted invalid user generic arity" >&2
    exit 1
fi
if ! grep -Fq "E_TYPE_ARITY" "$CHECK_ARITY_LOG" ||
    ! grep -Fq "expects 1 argument(s), found 2" "$CHECK_ARITY_LOG" ||
    ! grep -Fq "expects 3 argument(s), found 2" "$CHECK_ARITY_LOG" ||
    ! grep -Fq "expects at least 1 argument(s), found 0" \
        "$CHECK_ARITY_LOG"; then

    echo "FAIL: seen check did not report precise generic arity" >&2
    tail -n 120 "$CHECK_ARITY_LOG" >&2 || true
    exit 1
fi
assert_check_stayed_frontend_only "$CHECK_ARITY_LOG"

# Compile the same deliberately invalid source to preserve check/compile
# diagnostic parity; Pass 1b rejects it before LLVM lowering.
GENERIC_ARITY_LOG="$WORK_DIR/generic-arity.log"
if run_compiler compile "$CONSUMER_DIR/src/bad_generic_arity.seen" \
    "$WORK_DIR/generic-arity-out" --fast --no-cache \
    >"$GENERIC_ARITY_LOG" 2>&1; then
    echo "FAIL: surplus user-defined generic arguments compiled" >&2
    exit 1
fi
if ! grep -Fq "E_TYPE_ARITY" "$GENERIC_ARITY_LOG" ||
    ! grep -Fq "expects 1 argument(s), found 2" "$GENERIC_ARITY_LOG" ||
    ! grep -Fq "expects 3 argument(s), found 2" "$GENERIC_ARITY_LOG" ||
    ! grep -Fq "expects at least 1 argument(s), found 0" \
        "$GENERIC_ARITY_LOG"; then
    echo "FAIL: user-defined generic arity did not fail precisely" >&2
    tail -n 120 "$GENERIC_ARITY_LOG" >&2 || true
    exit 1
fi

echo "PASS: check/compile semantic parity, package identities, and generic arity"
