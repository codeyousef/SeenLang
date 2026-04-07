#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
COMPILER="$ROOT_DIR/compiler_seen/target/seen"
TMP_ROOT="/tmp/seen_fix_regressions"

D15_EMPTY_SRC="$ROOT_DIR/tests/fixtures/seen_fixes/d15_empty_class_import.seen"
D15_FUNCTION_SRC="$ROOT_DIR/tests/fixtures/seen_fixes/d15_function_only_import.seen"
C13_GENERIC_SRC="$ROOT_DIR/tests/fixtures/seen_fixes/c13_imported_generic_store_entry.seen"
C13_ENGINE_SRC="$ROOT_DIR/tests/fixtures/seen_fixes/c13_cross_module_engine_entry.seen"
C13_GLOBAL_CLASS_SRC="$ROOT_DIR/tests/fixtures/seen_fixes/c13_module_level_class_state_entry.seen"
C13_GLOBAL_GAME_SRC="$ROOT_DIR/tests/fixtures/seen_fixes/c13_module_level_game_entry.seen"
C12_SRC="$ROOT_DIR/tests/fixtures/seen_fixes/c12_direct_entry_missing_user_decl.seen"

cleanup_seen_artifacts() {
    rm -rf "$ROOT_DIR/.seen_cache" /tmp/seen_ir_cache "$TMP_ROOT"
    rm -f /tmp/seen_module_*.ll /tmp/seen_module_*.o /tmp/seen_module_*.opt.ll
    mkdir -p "$TMP_ROOT"
}

run_compile() {
    local source_file="$1"
    local output_file="$2"
    local log_file="$3"

    if [[ "$BUILD_CMD" == "build" ]]; then
        timeout 120 "$COMPILER" build "$source_file" -o "$output_file" --fast >"$log_file" 2>&1
    else
        timeout 120 "$COMPILER" compile "$source_file" "$output_file" --fast >"$log_file" 2>&1
    fi
}

run_success_case() {
    local label="$1"
    local source_file="$2"
    local output_file="$3"
    local log_file="$4"

    cleanup_seen_artifacts
    if ! run_compile "$source_file" "$output_file" "$log_file"; then
        echo "FAIL: $label compile failed"
        cat "$log_file"
        exit 1
    fi

    if ! "$output_file" >/dev/null 2>&1; then
        echo "FAIL: $label binary exited non-zero"
        cat "$log_file"
        exit 1
    fi

    echo "PASS: $label"
}

run_c12_case() {
    local output_file="$TMP_ROOT/c12_direct_entry_missing_user_decl"
    local log_file="$TMP_ROOT/c12_direct_entry_missing_user_decl.log"
    local ir_file="/tmp/seen_module_0.ll"

    cleanup_seen_artifacts

    set +e
    run_compile "$C12_SRC" "$output_file" "$log_file"
    local status=$?
    set -e

    if grep -q "use of undefined value '@unresolvedHelper'" "$log_file"; then
        echo "FAIL: C12 still fails in opt with an undefined LLVM value"
        cat "$log_file"
        exit 1
    fi

    if [[ ! -f "$ir_file" ]]; then
        echo "FAIL: C12 did not leave LLVM IR to inspect"
        cat "$log_file"
        exit 1
    fi

    if ! grep -Eq 'declare i64 @"?unresolvedHelper"?\(i64\) nounwind' "$ir_file"; then
        echo "FAIL: C12 did not emit the late unresolvedHelper declaration"
        grep -n "unresolvedHelper" "$ir_file" || true
        exit 1
    fi

    if [[ "$status" -eq 0 ]]; then
        echo "PASS: C12 direct-entry compile succeeded with a late user declaration"
        return
    fi

    if grep -Eq 'undefined (reference|symbol).*(unresolvedHelper)|Undefined symbols for architecture.*unresolvedHelper|ld\.lld: error: undefined symbol: unresolvedHelper|clang: error: linker command failed' "$log_file"; then
        echo "PASS: C12 progressed past optimizer failure and reached unresolved-external linking"
        return
    fi

    echo "FAIL: C12 failed unexpectedly after the late declare fix"
    cat "$log_file"
    exit 1
}

if [[ ! -x "$COMPILER" ]]; then
    echo "Compiler binary not found or not executable: $COMPILER" >&2
    exit 1
fi

CLI_HELP="$("$COMPILER" 2>&1 || true)"
BUILD_CMD="compile"
if grep -q 'seen build <' <<< "$CLI_HELP"; then
    BUILD_CMD="build"
fi

echo "=== Seen fix regression checks ==="

run_success_case "D15 empty-class import" "$D15_EMPTY_SRC" "$TMP_ROOT/d15_empty_class_import" "$TMP_ROOT/d15_empty_class_import.log"
run_success_case "D15 function-only control" "$D15_FUNCTION_SRC" "$TMP_ROOT/d15_function_only_import" "$TMP_ROOT/d15_function_only_import.log"
run_success_case "C13 imported generic store" "$C13_GENERIC_SRC" "$TMP_ROOT/c13_imported_generic_store" "$TMP_ROOT/c13_imported_generic_store.log"
run_success_case "C13 cross-module engine" "$C13_ENGINE_SRC" "$TMP_ROOT/c13_cross_module_engine" "$TMP_ROOT/c13_cross_module_engine.log"
run_success_case "C13 module-level class state" "$C13_GLOBAL_CLASS_SRC" "$TMP_ROOT/c13_module_level_class_state" "$TMP_ROOT/c13_module_level_class_state.log"
run_success_case "C13 module-level game state" "$C13_GLOBAL_GAME_SRC" "$TMP_ROOT/c13_module_level_game" "$TMP_ROOT/c13_module_level_game.log"
run_c12_case

cleanup_seen_artifacts
echo "=== Seen fix regression checks passed ==="
