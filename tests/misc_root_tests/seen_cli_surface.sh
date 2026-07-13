#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
COMPILER="${SEEN_BIN:-$ROOT_DIR/compiler_seen/target/seen}"
VERSION="${SEEN_EXPECTED_VERSION:-0.9.4}"

fail() {
    echo "FAIL: $*" >&2
    exit 1
}

if [[ ! -x "$COMPILER" ]]; then
    fail "Compiler binary not found or not executable: $COMPILER"
fi

expect_success_contains() {
    local label="$1"
    local expected="$2"
    shift 2

    local output
    if ! output="$("$@" 2>&1)"; then
        echo "$output" >&2
        fail "$label: expected success"
    fi
    if ! grep -Fq "$expected" <<<"$output"; then
        echo "$output" >&2
        fail "$label: expected output to contain '$expected'"
    fi
}

expect_failure_contains() {
    local label="$1"
    local expected="$2"
    shift 2

    local output
    set +e
    output="$("$@" 2>&1)"
    local status=$?
    set -e

    if [[ "$status" -eq 0 ]]; then
        echo "$output" >&2
        fail "$label: expected failure"
    fi
    if ! grep -Fq "$expected" <<<"$output"; then
        echo "$output" >&2
        fail "$label: expected output to contain '$expected'"
    fi
}

expect_success_contains "--version" "Seen $VERSION" "$COMPILER" --version
expect_success_contains "-v" "Seen $VERSION" "$COMPILER" -v
expect_success_contains "--help" "seen compile" "$COMPILER" --help
expect_success_contains "-h" "seen compile" "$COMPILER" -h

help_output="$("$COMPILER" --help 2>&1)"
if grep -Fq "seen build <" <<<"$help_output"; then
    echo "$help_output" >&2
    fail "--help advertised legacy seen build usage"
fi

expect_failure_contains "build unsupported" "not supported by the shipped compiler" "$COMPILER" build
expect_failure_contains "init unsupported" "not supported by the shipped compiler" "$COMPILER" init demo
expect_failure_contains "fmt unsupported" "not supported by the shipped compiler" "$COMPILER" fmt file.seen
expect_failure_contains "format unsupported" "not supported by the shipped compiler" "$COMPILER" format file.seen
expect_failure_contains "clean unsupported" "not supported by the shipped compiler" "$COMPILER" clean
expect_failure_contains "test unsupported" "not supported by the shipped compiler" "$COMPILER" test
expect_failure_contains "c backend unsupported" "Supported backend: llvm" "$COMPILER" compile file.seen out --backend=c

echo "CLI surface checks passed"
