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
C13_MODULE_CONST_BIND_SRC="$ROOT_DIR/tests/fixtures/seen_fixes/c13_module_const_local_bind_entry.seen"
C12_SRC="$ROOT_DIR/tests/fixtures/seen_fixes/c12_direct_entry_missing_user_decl.seen"
C12_MODULE_CONST_BIND_SRC="$ROOT_DIR/tests/fixtures/seen_fixes/c12_module_const_local_bind.seen"
C14_SHADOWED_BRANCH_SRC="$ROOT_DIR/tests/fixtures/seen_fixes/c14_shadowed_branch_locals.seen"

cleanup_seen_artifacts() {
    rm -rf "$ROOT_DIR/.seen_cache" /tmp/seen_ir_cache "$TMP_ROOT"
    rm -f /tmp/seen_module_*.ll /tmp/seen_module_*.o /tmp/seen_module_*.opt.ll \
        /tmp/seen_module_*.polly.ll /tmp/seen_module_*.opt.status /tmp/seen_module_*.opt.log
    rm -rf /tmp/seen_recovery.*
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

run_compile_in_dir() {
    local project_dir="$1"
    local source_file="$2"
    local output_file="$3"
    local log_file="$4"

    if [[ "$BUILD_CMD" == "build" ]]; then
        (
            cd "$project_dir" &&
            timeout 120 "$COMPILER" build "$source_file" -o "$output_file" --fast >"$log_file" 2>&1
        )
    else
        (
            cd "$project_dir" &&
            timeout 120 "$COMPILER" compile "$source_file" "$output_file" --fast >"$log_file" 2>&1
        )
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

run_recovery_partial_failure_case() {
    local recovery_log="$TMP_ROOT/recovery_partial_failure.log"

    cleanup_seen_artifacts

    local idx=0
    while [[ "$idx" -lt 31 ]]; do
        cat >"/tmp/seen_module_${idx}.ll" <<EOF
source_filename = "seen_module_${idx}"
target triple = "x86_64-unknown-linux-gnu"

define i64 @module_${idx}() {
entry:
  ret i64 0
}
EOF
        idx=$((idx + 1))
    done

    cat >"/tmp/seen_module_31.ll" <<'EOF'
source_filename = "seen_module_31"
target triple = "x86_64-unknown-linux-gnu"

define i64 @broken() {
entry:
  %x = call i64 @missing(i64 1)
  ret i64 %x
}
EOF

    set +e
    bash "$ROOT_DIR/scripts/recovery_opt.sh" "$ROOT_DIR/scripts" "$ROOT_DIR/scripts" --skip-fixups \
        >"$recovery_log" 2>&1
    local status=$?
    set -e

    if [[ "$status" -eq 0 ]]; then
        echo "FAIL: recovery_opt.sh accepted a partial object set after optimizer failure"
        cat "$recovery_log"
        exit 1
    fi

    if grep -q '^RECOVERY_DIR=' "$recovery_log"; then
        echo "FAIL: recovery_opt.sh exposed a recovery directory after partial failure"
        cat "$recovery_log"
        exit 1
    fi

    echo "PASS: recovery_opt rejects partial object sets"
}

run_toml_project_modules_case() {
    local project_dir="$TMP_ROOT/toml_project_modules"
    local output_file="$project_dir/toml_project_modules"
    local log_file="$project_dir/toml_project_modules.log"

    cleanup_seen_artifacts
    mkdir -p "$project_dir/engine" "$project_dir/game"

    cat >"$project_dir/Seen.toml" <<'EOF'
[project]
name = "toml-project-modules"
version = "0.1.0"
language = "en"
modules = [
    "engine/core.seen",
    "game/main.seen"
]
EOF

    cat >"$project_dir/engine/core.seen" <<'EOF'
class PropHandles {
    var gravity: Int

    static fun new() r: PropHandles {
        return PropHandles { gravity: 7 }
    }
}

class Engine {
    var propHandles: PropHandles

    static fun new() r: Engine {
        return Engine { propHandles: PropHandles.new() }
    }
}
EOF

    cat >"$project_dir/game/main.seen" <<'EOF'
fun main() r: Int {
    let engine = Engine.new()
    if engine.propHandles.gravity == 7 {
        return 0
    }
    return 1
}
EOF

    if ! run_compile_in_dir "$project_dir" "game/main.seen" "$output_file" "$log_file"; then
        echo "FAIL: Seen.toml [project].modules case did not compile"
        cat "$log_file"
        exit 1
    fi

    if ! "$output_file" >/dev/null 2>&1; then
        echo "FAIL: Seen.toml [project].modules binary exited non-zero"
        cat "$log_file"
        exit 1
    fi

    echo "PASS: Seen.toml project modules are included in compilation"
}

run_build_entry_seed_case() {
    local project_dir="$TMP_ROOT/build_entry_seed"
    local output_file="$project_dir/build_entry_seed"
    local log_file="$project_dir/build_entry_seed.log"

    cleanup_seen_artifacts
    mkdir -p "$project_dir/engine" "$project_dir/game"

    cat >"$project_dir/Seen.toml" <<'EOF'
[project]
name = "demo"
version = "0.1.0"
language = "en"

[build]
entry = "main.seen"
EOF

    cat >"$project_dir/main.seen" <<'EOF'
import demo::engine::core
EOF

    cat >"$project_dir/engine/core.seen" <<'EOF'
class Engine {
    var value: Int

    static fun new() r: Engine {
        return Engine { value: 7 }
    }
}
EOF

    cat >"$project_dir/game/main.seen" <<'EOF'
fun main() r: Int {
    let engine = Engine.new()
    if engine.value == 7 {
        return 0
    }
    return 1
}
EOF

    if ! run_compile_in_dir "$project_dir" "game/main.seen" "$output_file" "$log_file"; then
        echo "FAIL: Seen.toml [build].entry imports were not seeded into compilation"
        cat "$log_file"
        exit 1
    fi

    if ! "$output_file" >/dev/null 2>&1; then
        echo "FAIL: Seen.toml [build].entry seeded binary exited non-zero"
        cat "$log_file"
        exit 1
    fi

    echo "PASS: Seen.toml build entry imports are seeded into compilation"
}

run_build_entry_main_fallback_case() {
    local project_dir="$TMP_ROOT/build_entry_main_fallback"
    local output_file="$project_dir/build_entry_main_fallback"
    local log_file="$project_dir/build_entry_main_fallback.log"

    cleanup_seen_artifacts
    mkdir -p "$project_dir/game"

    cat >"$project_dir/Seen.toml" <<'EOF'
[project]
name = "demo"
version = "0.1.0"
language = "en"

[build]
entry = "main.seen"
EOF

    cat >"$project_dir/main.seen" <<'EOF'
import demo::game::game

fun main() r: Int {
    return runGame()
}
EOF

    cat >"$project_dir/game/game.seen" <<'EOF'
fun runGame() r: Int {
    return 0
}
EOF

    if ! run_compile_in_dir "$project_dir" "game/game.seen" "$output_file" "$log_file"; then
        echo "FAIL: Seen.toml [build].entry main was not added for non-main input"
        cat "$log_file"
        exit 1
    fi

    if ! "$output_file" >/dev/null 2>&1; then
        echo "FAIL: Seen.toml [build].entry fallback binary exited non-zero"
        cat "$log_file"
        exit 1
    fi

    echo "PASS: Seen.toml build entry main is added for non-main input"
}

run_missing_import_failure_case() {
    local project_dir="$TMP_ROOT/missing_import_failure"
    local output_file="$project_dir/missing_import_failure"
    local log_file="$project_dir/missing_import_failure.log"

    cleanup_seen_artifacts
    mkdir -p "$project_dir"

    cat >"$project_dir/main.seen" <<'EOF'
import missing_module.{MissingDecl}

fun main() r: Void {
}
EOF

    set +e
    run_compile_in_dir "$project_dir" "main.seen" "$output_file" "$log_file"
    local status=$?
    set -e

    if [[ "$status" -eq 0 ]]; then
        echo "FAIL: missing imported module compiled successfully"
        cat "$log_file"
        exit 1
    fi

    if ! grep -q 'could not read module' "$log_file"; then
        echo "FAIL: missing imported module did not surface a read error"
        cat "$log_file"
        exit 1
    fi

    echo "PASS: missing imported modules fail compilation"
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
run_success_case "C13 module-const local bind" "$C13_MODULE_CONST_BIND_SRC" "$TMP_ROOT/c13_module_const_local_bind" "$TMP_ROOT/c13_module_const_local_bind.log"
run_success_case "C14 shadowed branch locals" "$C14_SHADOWED_BRANCH_SRC" "$TMP_ROOT/c14_shadowed_branch_locals" "$TMP_ROOT/c14_shadowed_branch_locals.log"
run_success_case "C12 module-const local bind" "$C12_MODULE_CONST_BIND_SRC" "$TMP_ROOT/c12_module_const_local_bind" "$TMP_ROOT/c12_module_const_local_bind.log"
run_c12_case
run_recovery_partial_failure_case
run_toml_project_modules_case
run_build_entry_seed_case
run_build_entry_main_fallback_case
run_missing_import_failure_case

cleanup_seen_artifacts
echo "=== Seen fix regression checks passed ==="
