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
C13_HELPER_GLOBAL_SRC="$ROOT_DIR/tests/fixtures/seen_fixes/c13_imported_helper_global_state_entry.seen"
C13_MODULE_CONST_BIND_SRC="$ROOT_DIR/tests/fixtures/seen_fixes/c13_module_const_local_bind_entry.seen"
PACKAGE_LOCAL_DUP_CONST_SRC="$ROOT_DIR/tests/fixtures/seen_fixes/package_local_duplicate_const_entry.seen"
C12_SRC="$ROOT_DIR/tests/fixtures/seen_fixes/c12_direct_entry_missing_user_decl.seen"
C12_HELPER_GLOBAL_SRC="$ROOT_DIR/tests/fixtures/seen_fixes/c12_helper_global_state.seen"
C12_MODULE_CONST_BIND_SRC="$ROOT_DIR/tests/fixtures/seen_fixes/c12_module_const_local_bind.seen"
C14_SHADOWED_BRANCH_SRC="$ROOT_DIR/tests/fixtures/seen_fixes/c14_shadowed_branch_locals.seen"
EFFECT_OK_SRC="$ROOT_DIR/tests/fixtures/current_limitations/effect_capability_ok.seen"
CAPABILITY_WRONG_EFFECT_SRC="$ROOT_DIR/tests/fixtures/current_limitations/capability_missing_effect.seen"
METHOD_WRONG_EFFECT_SRC="$ROOT_DIR/tests/fixtures/current_limitations/effect_method_wrong.seen"
IMPORTED_CAPABILITY_MISSING_SRC="$ROOT_DIR/tests/fixtures/current_limitations/capability_imported_effect_missing_entry.seen"
IMPORTED_CAPABILITY_ALIAS_SRC="$ROOT_DIR/tests/fixtures/current_limitations/capability_imported_effect_alias_entry.seen"
IMPORTED_CAPABILITY_OK_SRC="$ROOT_DIR/tests/fixtures/current_limitations/capability_imported_effect_ok_entry.seen"
SEND_INVALID_SRC="$ROOT_DIR/tests/fixtures/current_limitations/send_annotation_invalid_field.seen"
SYNC_INVALID_SRC="$ROOT_DIR/tests/fixtures/current_limitations/sync_annotation_invalid_field.seen"
SEND_IMPORTED_INVALID_SRC="$ROOT_DIR/tests/fixtures/current_limitations/send_annotation_imported_invalid_entry.seen"
SYNC_IMPORTED_INVALID_SRC="$ROOT_DIR/tests/fixtures/current_limitations/sync_annotation_imported_invalid_entry.seen"
SEALED_CROSS_MODULE_ENTRY_SRC="$ROOT_DIR/tests/fixtures/current_limitations/sealed_cross_module_entry.seen"
SEALED_ALIAS_ENTRY_SRC="$ROOT_DIR/tests/fixtures/current_limitations/sealed_alias_entry.seen"
SEALED_SAME_MODULE_OK_SRC="$ROOT_DIR/tests/fixtures/current_limitations/sealed_same_module_ok.seen"
WHEN_ENUM_NON_EXHAUSTIVE_SRC="$ROOT_DIR/tests/fixtures/current_limitations/when_enum_non_exhaustive.seen"
WHEN_ENUM_EXHAUSTIVE_OK_SRC="$ROOT_DIR/tests/fixtures/current_limitations/when_enum_exhaustive_ok.seen"
WHEN_ENUM_ELSE_OK_SRC="$ROOT_DIR/tests/fixtures/current_limitations/when_enum_else_ok.seen"
UNRESOLVED_FREE_CALL_SRC="$ROOT_DIR/tests/fixtures/current_limitations/unresolved_free_call.seen"
BOOL_RETURN_COERCION_SRC="$ROOT_DIR/tests/codegen/test_bool_return_int_coercion_regression.seen"
BOOL_HELPER_LOGICAL_SRC="$ROOT_DIR/tests/codegen/test_bool_helper_logical_regression.seen"
FLOAT32_PTR_DEREF_CAST_SRC="$ROOT_DIR/tests/codegen/test_float32_ptr_deref_cast_regression.seen"
NESTED_PROPERTY_RECEIVER_SRC="$ROOT_DIR/tests/codegen/test_nested_property_receiver_regression.seen"
FEL22_TYPED_STORE_RETURN_SRC="$ROOT_DIR/tests/codegen/test_fel22_typed_store_return_regression.seen"
PTR_I64_ALIAS_SRC="$ROOT_DIR/tests/codegen/test_ptr_i64_alias_regression.seen"
RUNTIME_FILE_BYTES_SRC="$ROOT_DIR/tests/codegen/test_runtime_file_bytes_regression.seen"
UINT32_GLOBAL_INIT_SRC="$ROOT_DIR/tests/codegen/test_uint32_global_init_regression.seen"
HIGH_ARITY_PARAMS_SRC="$ROOT_DIR/tests/codegen/test_high_arity_params_regression.seen"
NESTED_CONTINUE_HIGH_ARITY_SRC="$ROOT_DIR/tests/codegen/test_nested_continue_high_arity_regression.seen"

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

run_check() {
    local source_file="$1"
    local log_file="$2"

    timeout 120 "$COMPILER" check "$source_file" --language en >"$log_file" 2>&1
}

run_compile_with_path_override() {
    local path_prefix="$1"
    local source_file="$2"
    local output_file="$3"
    local log_file="$4"

    if [[ "$BUILD_CMD" == "build" ]]; then
        PATH="$path_prefix:$PATH" timeout 120 "$COMPILER" build "$source_file" -o "$output_file" --fast >"$log_file" 2>&1
    else
        PATH="$path_prefix:$PATH" timeout 120 "$COMPILER" compile "$source_file" "$output_file" --fast >"$log_file" 2>&1
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

run_compile_no_cache_no_fork() {
    local source_file="$1"
    local output_file="$2"
    local log_file="$3"

    if [[ "$BUILD_CMD" == "build" ]]; then
        timeout 120 "$COMPILER" build "$source_file" -o "$output_file" --fast --no-cache --no-fork >"$log_file" 2>&1
    else
        timeout 120 "$COMPILER" compile "$source_file" "$output_file" --fast --no-cache --no-fork >"$log_file" 2>&1
    fi
}

run_success_case_with_ir_check() {
    local label="$1"
    local source_file="$2"
    local output_file="$3"
    local log_file="$4"
    local expected_ir_pattern="$5"
    local rejected_ir_pattern="$6"

    cleanup_seen_artifacts
    if ! run_compile_no_cache_no_fork "$source_file" "$output_file" "$log_file"; then
        echo "FAIL: $label compile failed"
        cat "$log_file"
        exit 1
    fi

    if [[ ! -f /tmp/seen_module_0.ll ]]; then
        echo "FAIL: $label did not leave LLVM IR to inspect"
        cat "$log_file"
        exit 1
    fi

    if ! grep -Eq "$expected_ir_pattern" /tmp/seen_module_0.ll; then
        echo "FAIL: $label did not emit the expected LLVM IR"
        grep -n "PropertyRegistry_registerFloat" /tmp/seen_module_0.ll || true
        exit 1
    fi

    if [[ -n "$rejected_ir_pattern" ]] && grep -Eq "$rejected_ir_pattern" /tmp/seen_module_0.ll; then
        echo "FAIL: $label emitted rejected LLVM IR"
        grep -n "PropertyRegistry_registerFloat" /tmp/seen_module_0.ll || true
        exit 1
    fi

    if ! "$output_file" >/dev/null 2>&1; then
        echo "FAIL: $label binary exited non-zero"
        cat "$log_file"
        exit 1
    fi

    echo "PASS: $label"
}

run_check_success_case() {
    local label="$1"
    local source_file="$2"
    local log_file="$3"

    cleanup_seen_artifacts
    if ! run_check "$source_file" "$log_file"; then
        echo "FAIL: $label check failed"
        cat "$log_file"
        exit 1
    fi

    echo "PASS: $label"
}

run_check_failure_case() {
    local label="$1"
    local source_file="$2"
    local log_file="$3"
    local expected_pattern="$4"

    cleanup_seen_artifacts

    set +e
    run_check "$source_file" "$log_file"
    local status=$?
    set -e

    if [[ "$status" -eq 0 ]]; then
        echo "FAIL: $label unexpectedly passed"
        cat "$log_file"
        exit 1
    fi

    if ! grep -Eq "$expected_pattern" "$log_file"; then
        echo "FAIL: $label did not report the expected diagnostic"
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

setup_fake_llvm_dir() {
    local kind="$1"
    local fake_dir="$2"
    local fake_log="$3"
    local real_opt
    local real_llc
    local real_link

    real_opt="$(command -v opt)"
    real_llc="$(command -v llc)"
    real_link="$(command -v llvm-link)"

    mkdir -p "$fake_dir"
    ln -sf "$real_link" "$fake_dir/llvm-link"

    cat >"$fake_dir/opt" <<EOF
#!/usr/bin/env bash
set -euo pipefail
echo "\$*" >> "$fake_log"
case "$kind" in
  opt)
    if [[ "\$*" == *"/tmp/seen_module_1.ll"* && "\$*" != *"--thinlto-bc"* ]]; then
      echo "forced opt failure for module 1" >&2
      exit 1
    fi
    ;;
  object)
    if [[ "\$*" == *"--thinlto-bc"* && "\$*" == *"/tmp/seen_module_1.opt.ll"* ]]; then
      echo "forced object emission failure for module 1" >&2
      exit 1
    fi
    ;;
esac
exec "$real_opt" "\$@"
EOF
    chmod +x "$fake_dir/opt"

    cat >"$fake_dir/llc" <<EOF
#!/usr/bin/env bash
set -euo pipefail
echo "\$*" >> "$fake_log"
case "$kind" in
  object)
    if [[ "\$*" == *"/tmp/seen_module_1.opt.ll"* ]]; then
      echo "forced object emission failure for module 1" >&2
      exit 1
    fi
    ;;
esac
exec "$real_llc" "\$@"
EOF
    chmod +x "$fake_dir/llc"
}

run_real_compiler_failure_case() {
    local label="$1"
    local kind="$2"
    local expected_driver_error="$3"
    local expected_forced_error="$4"
    local output_file="$TMP_ROOT/c13_forced_${kind}_failure"
    local log_file="$TMP_ROOT/c13_forced_${kind}_failure.log"
    local fake_dir="$TMP_ROOT/fake_llvm_${kind}"
    local fake_log="$TMP_ROOT/fake_llvm_${kind}.log"

    cleanup_seen_artifacts
    setup_fake_llvm_dir "$kind" "$fake_dir" "$fake_log"

    set +e
    run_compile_with_path_override "$fake_dir" "$C13_ENGINE_SRC" "$output_file" "$log_file"
    local status=$?
    set -e

    if [[ "$status" -eq 0 ]]; then
        echo "FAIL: $label unexpectedly passed"
        cat "$log_file"
        exit 1
    fi

    if [[ -f "$output_file" ]]; then
        echo "FAIL: $label produced a success-shaped output binary"
        cat "$log_file"
        exit 1
    fi

    if ! grep -q "$expected_driver_error" "$log_file"; then
        echo "FAIL: $label did not report the expected compiler-side failure"
        cat "$log_file"
        exit 1
    fi

    if ! grep -q "$expected_forced_error" "$log_file"; then
        echo "FAIL: $label did not surface the injected LLVM failure"
        cat "$log_file"
        exit 1
    fi

    if grep -q 'Build succeeded ->' "$log_file"; then
        echo "FAIL: $label still printed a success-shaped build message"
        cat "$log_file"
        exit 1
    fi

    echo "PASS: $label"
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

run_large_manifest_scanner_case() {
    local project_dir="$TMP_ROOT/large_manifest_scanner"
    local output_file="$project_dir/large_manifest_scanner"
    local log_file="$project_dir/large_manifest_scanner.log"

    cleanup_seen_artifacts
    mkdir -p "$project_dir/hearton/src/generated"

    {
        echo '[project]'
        echo 'name = "large-manifest-scanner"'
        echo 'version = "0.1.0"'
        echo 'language = "en"'
        echo 'description = "large manifest scanner regression"'
        echo ''
        echo 'modules = ['
        echo '    "main.seen",'
        for i in $(seq 1 56); do
            printf '    "hearton/src/generated/module_%03d.seen",\n' "$i"
        done
        echo ']'
        echo ''
        echo '[build]'
        echo 'entry = "main.seen"'
    } >"$project_dir/Seen.toml"

    cat >"$project_dir/main.seen" <<'EOF'
fun main() r: Int {
    return 0
}
EOF

    for i in $(seq 1 56); do
        local module_file
        module_file=$(printf '%s/hearton/src/generated/module_%03d.seen' "$project_dir" "$i")
        {
            printf 'fun generatedHelper%03d() r: Int {\n' "$i"
            printf '    return %d\n' "$i"
            printf '}\n'
        } >"$module_file"
    done

    (
        cd "$project_dir" &&
        timeout 120 "$COMPILER" compile "main.seen" "$output_file" --fast --no-cache --no-fork >"$log_file" 2>&1
    ) || {
        echo "FAIL: large Seen.toml scanner case did not compile"
        cat "$log_file"
        exit 1
    }

    if grep -q 'free(): invalid size' "$log_file"; then
        echo "FAIL: large Seen.toml scanner case aborted in manifest handling"
        cat "$log_file"
        exit 1
    fi

    if ! grep -q 'Large module graph' "$log_file"; then
        echo "FAIL: large Seen.toml scanner case did not use bounded preflight path"
        cat "$log_file"
        exit 1
    fi

    if ! "$output_file" >/dev/null 2>&1; then
        echo "FAIL: large Seen.toml scanner binary exited non-zero"
        cat "$log_file"
        exit 1
    fi

    echo "PASS: large Seen.toml manifests use bounded scanner path"
}

run_small_manifest_allocator_stability_case() {
    local project_dir="$TMP_ROOT/small_manifest_allocator_stability"
    local output_file="$project_dir/small_manifest_allocator_stability"
    local log_file="$project_dir/small_manifest_allocator_stability.log"

    cleanup_seen_artifacts
    mkdir -p "$project_dir/modules"

    {
        echo '[project]'
        echo 'name = "small-manifest-allocator-stability"'
        echo 'version = "0.1.0"'
        echo 'language = "en"'
        echo ''
        echo 'modules = ['
        echo '    "main.seen",'
        for i in $(seq 1 12); do
            printf '    "modules/helper_%02d.seen",\n' "$i"
        done
        echo ']'
        echo ''
        echo '[build]'
        echo 'entry = "main.seen"'
    } >"$project_dir/Seen.toml"

    cat >"$project_dir/main.seen" <<'EOF'
fun main() r: Int {
    return 0
}
EOF

    for i in $(seq 1 12); do
        local module_file
        module_file=$(printf '%s/modules/helper_%02d.seen' "$project_dir" "$i")
        {
            printf 'fun smallManifestHelper%02d() r: Int {\n' "$i"
            printf '    return %d\n' "$i"
            printf '}\n'
        } >"$module_file"
    done

    if ! run_compile_in_dir "$project_dir" "main.seen" "$output_file" "$log_file"; then
        echo "FAIL: small Seen.toml allocator stability case did not compile"
        cat "$log_file"
        exit 1
    fi

    if grep -q 'free(): invalid size' "$log_file"; then
        echo "FAIL: small Seen.toml allocator stability case aborted after preflight"
        cat "$log_file"
        exit 1
    fi

    if grep -q 'Large module graph' "$log_file"; then
        echo "FAIL: small Seen.toml allocator stability case skipped whole-project preflight"
        cat "$log_file"
        exit 1
    fi

    if ! "$output_file" >/dev/null 2>&1; then
        echo "FAIL: small Seen.toml allocator stability binary exited non-zero"
        cat "$log_file"
        exit 1
    fi

    echo "PASS: small Seen.toml preflight releases validation state cleanly"
}

run_scalar_get_guard_case() {
    local source_file="$TMP_ROOT/scalar_get_guard.seen"
    local output_file="$TMP_ROOT/scalar_get_guard"
    local log_file="$TMP_ROOT/scalar_get_guard.log"

    cleanup_seen_artifacts

    cat >"$source_file" <<'EOF'
fun main() r: Int {
    let value = 5
    let ignored = value.get(0)
    return 0
}
EOF

    if ! run_compile "$source_file" "$output_file" "$log_file"; then
        echo "FAIL: scalar get guard case did not compile"
        cat "$log_file"
        exit 1
    fi

    if grep -q '@Int_get' "$log_file" /tmp/seen_module_*.ll 2>/dev/null; then
        echo "FAIL: scalar get guard still emitted Int_get"
        cat "$log_file"
        grep -n '@Int_get' /tmp/seen_module_*.ll 2>/dev/null || true
        exit 1
    fi

    if ! "$output_file" >/dev/null 2>&1; then
        echo "FAIL: scalar get guard binary exited non-zero"
        cat "$log_file"
        exit 1
    fi

    echo "PASS: scalar get receivers do not lower to Int_get"
}

run_local_system_dependency_case() {
    local project_dir="$TMP_ROOT/local_system_dependency"
    local outside_dir="$TMP_ROOT/local_system_dependency_outside"
    local output_file="$TMP_ROOT/local_system_dependency_bin"
    local log_file="$TMP_ROOT/local_system_dependency.log"
    local source_file="$project_dir/main.seen"

    cleanup_seen_artifacts
    mkdir -p "$project_dir/native/lib" "$outside_dir"

    cat >"$project_dir/Seen.toml" <<'EOF'
[project]
name = "local-system-dependency"
version = "0.1.0"
language = "en"

[dependencies]
hearton_shim = { system = true, path = "native/lib" }
EOF

    cat >"$project_dir/native/hearton_shim.c" <<'EOF'
#include <stdint.h>

int64_t hearton_bonus(int64_t base) {
    return base + 7;
}
EOF

    if ! clang -shared -fPIC "$project_dir/native/hearton_shim.c" -o "$project_dir/native/lib/libhearton_shim.so" >/dev/null 2>&1; then
        echo "FAIL: local system dependency shim did not build"
        exit 1
    fi

    cat >"$project_dir/main.seen" <<'EOF'
extern fun hearton_bonus(base: Int) r: Int

fun main() r: Int {
    if hearton_bonus(35) == 42 {
        return 0
    }
    return 1
}
EOF

    set +e
    if [[ "$BUILD_CMD" == "build" ]]; then
        (
            cd "$outside_dir" &&
            env -u LIBRARY_PATH -u LD_LIBRARY_PATH timeout 120 "$COMPILER" build "$source_file" -o "$output_file" --fast >"$log_file" 2>&1
        )
    else
        (
            cd "$outside_dir" &&
            env -u LIBRARY_PATH -u LD_LIBRARY_PATH timeout 120 "$COMPILER" compile "$source_file" "$output_file" --fast >"$log_file" 2>&1
        )
    fi
    local status=$?
    set -e

    if [[ "$status" -ne 0 ]]; then
        echo "FAIL: local system dependency did not compile without LIBRARY_PATH"
        cat "$log_file"
        exit 1
    fi

    if ! env -u LIBRARY_PATH -u LD_LIBRARY_PATH "$output_file" >/dev/null 2>&1; then
        echo "FAIL: local system dependency binary still needs runtime library env overrides"
        cat "$log_file"
        exit 1
    fi

    echo "PASS: Seen.toml local system dependency paths link and run"
}

run_c12_abs_path_project_case() {
    local project_dir="$TMP_ROOT/c12_abs_path_project"
    local outside_dir="$TMP_ROOT/c12_abs_path_outside"
    local output_file="$TMP_ROOT/c12_abs_path_project_bin"
    local log_file="$TMP_ROOT/c12_abs_path_project.log"
    local source_file="$project_dir/game.seen"

    cleanup_seen_artifacts
    mkdir -p "$project_dir" "$outside_dir"

    cat >"$project_dir/Seen.toml" <<'EOF'
[project]
name = "c12repro"
version = "0.1.0"
language = "en"

modules = [
    "support.seen",
    "game.seen"
]

[build]
entry = "game.seen"
optimize = "speed"
EOF

    cat >"$project_dir/support.seen" <<'EOF'
class PropertyRegistry {
    var count: Int
    var floatValues: Array<Float>

    static fun new() r: PropertyRegistry {
        return PropertyRegistry {
            count: 0,
            floatValues: Array<Float>()
        }
    }

    fun registerFloat(default_: Float) r: Int {
        let handle = this.count
        this.floatValues.push(default_)
        this.count = this.count + 1
        return handle
    }
}

class PropHandles {
    var gravity: Int

    static fun new() r: PropHandles {
        return PropHandles { gravity: -1 }
    }
}

class PresetStack {
    var registry: PropertyRegistry
    var stageHandles: Array<Int>
    var stageFloatValues: Array<Float>

    static fun new(registry: PropertyRegistry) r: PresetStack {
        return PresetStack {
            registry: registry,
            stageHandles: Array<Int>(),
            stageFloatValues: Array<Float>()
        }
    }

    fun beginPreset() {
        this.stageHandles = Array<Int>()
        this.stageFloatValues = Array<Float>()
    }

    fun stageFloat(handle: Int, value: Float) {
        this.stageHandles.push(handle)
        this.stageFloatValues.push(value)
    }
}

class Engine {
    var props: PropertyRegistry
    var propHandles: PropHandles

    static fun new() r: Engine {
        return Engine {
            props: PropertyRegistry.new(),
            propHandles: PropHandles.new()
        }
    }

    fun initProperties() {
        this.propHandles.gravity = this.props.registerFloat(-32.0)
    }
}
EOF

    cat >"$project_dir/game.seen" <<'EOF'
fun runGame() {
    let engine = Engine.new()
    engine.initProperties()

    var presetStack = PresetStack.new(engine.props)
    presetStack.beginPreset()
    presetStack.stageFloat(engine.propHandles.gravity, -32.0)
}

fun main() r: Int {
    runGame()
    return 0
}
EOF

    set +e
    if [[ "$BUILD_CMD" == "build" ]]; then
        (
            cd "$outside_dir" &&
            timeout 120 "$COMPILER" build "$source_file" -o "$output_file" >"$log_file" 2>&1
        )
    else
        (
            cd "$outside_dir" &&
            timeout 120 "$COMPILER" compile "$source_file" "$output_file" >"$log_file" 2>&1
        )
    fi
    local status=$?
    set -e

    if [[ "$status" -ne 0 ]]; then
        echo "FAIL: C12 absolute-path direct-entry project module did not compile"
        cat "$log_file"
        exit 1
    fi

    if ! grep -q 'Found 2 modules' "$log_file"; then
        echo "FAIL: C12 absolute-path direct-entry project module did not load Seen.toml project modules"
        cat "$log_file"
        exit 1
    fi

    if ! "$output_file" >/dev/null 2>&1; then
        echo "FAIL: C12 absolute-path direct-entry project module binary exited non-zero"
        cat "$log_file"
        exit 1
    fi

    echo "PASS: C12 absolute-path direct-entry project module"
}

run_non_member_abs_path_case() {
    local project_dir="$TMP_ROOT/non_member_abs_path_project"
    local outside_dir="$TMP_ROOT/non_member_abs_path_outside"
    local output_file="$TMP_ROOT/non_member_abs_path_bin"
    local log_file="$TMP_ROOT/non_member_abs_path.log"
    local source_file="$project_dir/tests/standalone.seen"

    cleanup_seen_artifacts
    mkdir -p "$project_dir/tests" "$outside_dir"

    cat >"$project_dir/Seen.toml" <<'EOF'
[project]
name = "non-member-project"
version = "0.1.0"
language = "en"
modules = [
    "missing.seen"
]
EOF

    cat >"$project_dir/tests/standalone.seen" <<'EOF'
fun main() r: Int {
    return 0
}
EOF

    set +e
    if [[ "$BUILD_CMD" == "build" ]]; then
        (
            cd "$outside_dir" &&
            timeout 120 "$COMPILER" build "$source_file" -o "$output_file" >"$log_file" 2>&1
        )
    else
        (
            cd "$outside_dir" &&
            timeout 120 "$COMPILER" compile "$source_file" "$output_file" >"$log_file" 2>&1
        )
    fi
    local status=$?
    set -e

    if [[ "$status" -ne 0 ]]; then
        echo "FAIL: non-member absolute-path input should not load Seen.toml project modules"
        cat "$log_file"
        exit 1
    fi

    if grep -q 'could not read module' "$log_file"; then
        echo "FAIL: non-member absolute-path input still tried to load Seen.toml project modules"
        cat "$log_file"
        exit 1
    fi

    if ! grep -q 'Found 1 modules' "$log_file"; then
        echo "FAIL: non-member absolute-path input unexpectedly changed module discovery"
        cat "$log_file"
        exit 1
    fi

    if ! "$output_file" >/dev/null 2>&1; then
        echo "FAIL: non-member absolute-path binary exited non-zero"
        cat "$log_file"
        exit 1
    fi

    echo "PASS: non-member absolute-path input stays standalone"
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

run_root_main_build_entry_isolation_case() {
    local project_dir="$TMP_ROOT/root_main_build_entry_isolation"
    local output_file="$project_dir/root_main_build_entry_isolation"
    local log_file="$project_dir/root_main_build_entry_isolation.log"

    cleanup_seen_artifacts
    mkdir -p "$project_dir"

    cat >"$project_dir/Seen.toml" <<'EOF'
[project]
name = "demo"
version = "0.1.0"
language = "en"

[build]
entry = "main.seen"
EOF

    cat >"$project_dir/main.seen" <<'EOF'
import demo::missing::core
EOF

    cat >"$project_dir/bug_repro.seen" <<'EOF'
fun main() r: Int {
    return 0
}
EOF

    if ! run_compile_in_dir "$project_dir" "bug_repro.seen" "$output_file" "$log_file"; then
        echo "FAIL: root-level standalone main should ignore Seen.toml build-entry seeding"
        cat "$log_file"
        exit 1
    fi

    if grep -q 'Seeding imports from Seen.toml build entry' "$log_file"; then
        echo "FAIL: root-level standalone main still seeded build-entry imports"
        cat "$log_file"
        exit 1
    fi

    if grep -q 'could not read module' "$log_file"; then
        echo "FAIL: root-level standalone main still tried to resolve build-entry imports"
        cat "$log_file"
        exit 1
    fi

    if ! grep -q 'Found 1 modules' "$log_file"; then
        echo "FAIL: root-level standalone main unexpectedly changed module discovery"
        cat "$log_file"
        exit 1
    fi

    if ! "$output_file" >/dev/null 2>&1; then
        echo "FAIL: root-level standalone main binary exited non-zero"
        cat "$log_file"
        exit 1
    fi

    echo "PASS: root-level standalone main stays isolated from build entry"
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

run_legacy_whole_module_import_case() {
    local project_dir="$TMP_ROOT/legacy_whole_module_import"
    local output_file="$project_dir/legacy_whole_module_import"
    local log_file="$project_dir/legacy_whole_module_import.log"

    cleanup_seen_artifacts
    mkdir -p "$project_dir/resolver_visibility_repro"

    cat >"$project_dir/Seen.toml" <<'EOF'
[project]
name = "resolver_visibility_repro"
version = "0.1.0"
language = "en"
description = "Package import aggregator visibility repro"

modules = [
    "resolver_visibility_repro/helper.seen",
    "resolver_visibility_repro/consumer.seen",
]

[build]
entry = "main.seen"
EOF

    cat >"$project_dir/resolver_visibility_repro/helper.seen" <<'EOF'
fun helperValue() r: Int {
    return 41
}
EOF

    cat >"$project_dir/resolver_visibility_repro/consumer.seen" <<'EOF'
fun consumerValue() r: Int {
    return helperValue() + 1
}
EOF

    cat >"$project_dir/main.seen" <<'EOF'
import resolver_visibility_repro::helper
import resolver_visibility_repro::consumer

fun main() r: Int {
    let value = consumerValue()
    if value == 42 {
        return 0
    }
    return 1
}
EOF

    if ! run_compile_in_dir "$project_dir" "main.seen" "$output_file" "$log_file"; then
        echo "FAIL: legacy whole-module imports did not compile"
        cat "$log_file"
        exit 1
    fi

    if grep -q 'unresolved function' "$log_file"; then
        echo "FAIL: legacy whole-module imports still reported unresolved functions"
        cat "$log_file"
        exit 1
    fi

    if ! "$output_file" >/dev/null 2>&1; then
        echo "FAIL: legacy whole-module import binary exited non-zero"
        cat "$log_file"
        exit 1
    fi

    echo "PASS: legacy whole-module imports expose top-level functions"
}

run_named_import_control_case() {
    local project_dir="$TMP_ROOT/named_import_control"
    local output_file="$project_dir/named_import_control"
    local log_file="$project_dir/named_import_control.log"

    cleanup_seen_artifacts
    mkdir -p "$project_dir/resolver_visibility_repro"

    cat >"$project_dir/Seen.toml" <<'EOF'
[project]
name = "resolver_visibility_repro"
version = "0.1.0"
language = "en"
description = "Curly import control"

modules = [
    "resolver_visibility_repro/helper.seen",
    "resolver_visibility_repro/consumer.seen",
]

[build]
entry = "main.seen"
EOF

    cat >"$project_dir/resolver_visibility_repro/helper.seen" <<'EOF'
fun helperValue() r: Int {
    return 41
}
EOF

    cat >"$project_dir/resolver_visibility_repro/consumer.seen" <<'EOF'
import resolver_visibility_repro.helper.{helperValue}

fun consumerValue() r: Int {
    return helperValue() + 1
}
EOF

    cat >"$project_dir/main.seen" <<'EOF'
import resolver_visibility_repro.consumer.{consumerValue}

fun main() r: Int {
    let value = consumerValue()
    if value == 42 {
        return 0
    }
    return 1
}
EOF

    if ! run_compile_in_dir "$project_dir" "main.seen" "$output_file" "$log_file"; then
        echo "FAIL: explicit named imports did not compile"
        cat "$log_file"
        exit 1
    fi

    if ! "$output_file" >/dev/null 2>&1; then
        echo "FAIL: explicit named import binary exited non-zero"
        cat "$log_file"
        exit 1
    fi

    echo "PASS: explicit named imports remain supported"
}

run_package_explicit_src_import_case() {
    local package_dir="$TMP_ROOT/seen_input"
    local project_dir="$TMP_ROOT/package_explicit_src_import"
    local output_file="$project_dir/package_explicit_src_import"
    local log_file="$project_dir/package_explicit_src_import.log"

    cleanup_seen_artifacts
    mkdir -p "$package_dir/src" "$project_dir"

    cat >"$package_dir/Seen.toml" <<'EOF'
[project]
name = "seen_input"
version = "0.1.0"
language = "en"
EOF

    cat >"$package_dir/src/keyboard.seen" <<'EOF'
class Keyboard {
    var key: Int

    static fun new() r: Keyboard {
        return Keyboard { key: 42 }
    }
}
EOF

    cat >"$project_dir/Seen.toml" <<'EOF'
[project]
name = "package_explicit_src_import"
version = "0.1.0"
language = "en"

[dependencies]
seen_input = { path = "../seen_input" }
EOF

    cat >"$project_dir/main.seen" <<'EOF'
import seen_input::src::keyboard.{Keyboard}

fun main() r: Int {
    let keyboard = Keyboard.new()
    if keyboard.key == 42 {
        return 0
    }
    return 1
}
EOF

    if ! run_compile_in_dir "$project_dir" "main.seen" "$output_file" "$log_file"; then
        echo "FAIL: package import with explicit src segment did not compile"
        cat "$log_file"
        exit 1
    fi

    if grep -q ':/src/keyboard.seen\|/src/src/keyboard.seen' "$log_file"; then
        echo "FAIL: package import with explicit src resolved to the wrong path"
        cat "$log_file"
        exit 1
    fi

    if ! "$output_file" >/dev/null 2>&1; then
        echo "FAIL: package import with explicit src binary exited non-zero"
        cat "$log_file"
        exit 1
    fi

    echo "PASS: package imports with explicit src resolve under package source root"
}

run_package_local_dotted_import_case() {
    local package_dir="$TMP_ROOT/seen_audio"
    local project_dir="$TMP_ROOT/package_local_dotted_import"
    local output_file="$project_dir/package_local_dotted_import"
    local log_file="$project_dir/package_local_dotted_import.log"

    cleanup_seen_artifacts
    mkdir -p "$package_dir/src/platform/linux" "$project_dir"

    cat >"$package_dir/Seen.toml" <<'EOF'
[project]
name = "seen_audio"
version = "0.1.0"
language = "en"
EOF

    cat >"$package_dir/src/platform/linux/sdl3.seen" <<'EOF'
fun audioPlatformValue() r: Int {
    return 41
}
EOF

    cat >"$package_dir/src/runtime.seen" <<'EOF'
import platform.linux.sdl3.{audioPlatformValue}

fun runtimeValue() r: Int {
    return audioPlatformValue() + 1
}
EOF

    cat >"$project_dir/Seen.toml" <<'EOF'
[project]
name = "package_local_dotted_import"
version = "0.1.0"
language = "en"

[dependencies]
seen_audio = { path = "../seen_audio" }
EOF

    cat >"$project_dir/main.seen" <<'EOF'
import seen_audio::src::runtime.{runtimeValue}

fun main() r: Int {
    if runtimeValue() == 42 {
        return 0
    }
    return 1
}
EOF

    if ! run_compile_in_dir "$project_dir" "main.seen" "$output_file" "$log_file"; then
        echo "FAIL: package-local dotted import did not compile"
        cat "$log_file"
        exit 1
    fi

    if grep -q '/src/\./linux/sdl3.seen\|:/src/.*sdl3.seen' "$log_file"; then
        echo "FAIL: package-local dotted import resolved to the wrong path"
        cat "$log_file"
        exit 1
    fi

    if ! "$output_file" >/dev/null 2>&1; then
        echo "FAIL: package-local dotted import binary exited non-zero"
        cat "$log_file"
        exit 1
    fi

    echo "PASS: package-local dotted imports resolve from package source root"
}

run_manifest_sibling_runtime_memory_case() {
    local project_dir="$TMP_ROOT/manifest_sibling_runtime_memory"
    local output_file="$project_dir/manifest_sibling_runtime_memory"
    local log_file="$project_dir/manifest_sibling_runtime_memory.log"

    cleanup_seen_artifacts
    mkdir -p "$project_dir/seen_runtime/src" "$project_dir/seen_voxel/src"

    cat >"$project_dir/Seen.toml" <<'EOF'
[project]
name = "manifest_sibling_runtime_memory"
version = "0.1.0"
language = "en"

modules = [
    "seen_runtime/src/memory.seen",
    "seen_voxel/src/runtime.seen",
]
EOF

    cat >"$project_dir/seen_runtime/src/memory.seen" <<'EOF'
extern fun seen_mem_alloc(size: Int) r: Int
EOF

    cat >"$project_dir/seen_voxel/src/runtime.seen" <<'EOF'
fun runtimeValue() r: Int {
    let ptr = seen_mem_alloc(8)
    if ptr != 0 {
        return 42
    }
    return 41
}
EOF

    cat >"$project_dir/main.seen" <<'EOF'
import seen_voxel::src::runtime.{runtimeValue}

fun main() r: Int {
    if runtimeValue() == 42 {
        return 0
    }
    return 1
}
EOF

    if ! run_compile_in_dir "$project_dir" "main.seen" "$output_file" "$log_file"; then
        echo "FAIL: manifest sibling runtime memory case did not compile"
        cat "$log_file"
        exit 1
    fi

    if grep -q '/seen_voxel/src/seen_runtime/src/memory.seen\|:/src/seen_runtime/src/memory.seen' "$log_file"; then
        echo "FAIL: implicit runtime memory import resolved relative to sibling package source"
        cat "$log_file"
        exit 1
    fi

    if ! "$output_file" >/dev/null 2>&1; then
        echo "FAIL: manifest sibling runtime memory binary exited non-zero"
        cat "$log_file"
        exit 1
    fi

    echo "PASS: implicit runtime memory imports resolve from project root"
}

run_package_whole_module_visibility_case() {
    local package_dir="$TMP_ROOT/seen_whole"
    local project_dir="$TMP_ROOT/package_whole_module_visibility"
    local output_file="$project_dir/package_whole_module_visibility"
    local log_file="$project_dir/package_whole_module_visibility.log"

    cleanup_seen_artifacts
    mkdir -p "$package_dir/src" "$project_dir"

    cat >"$package_dir/Seen.toml" <<'EOF'
[project]
name = "seen_whole"
version = "0.1.0"
language = "en"
EOF

    cat >"$package_dir/src/helper.seen" <<'EOF'
fun helperValue() r: Int {
    return 42
}
EOF

    cat >"$project_dir/Seen.toml" <<'EOF'
[project]
name = "package_whole_module_visibility"
version = "0.1.0"
language = "en"

[dependencies]
seen_whole = { path = "../seen_whole" }
EOF

    cat >"$project_dir/main.seen" <<'EOF'
import seen_whole::src::helper

fun main() r: Int {
    if helperValue() == 42 {
        return 0
    }
    return 1
}
EOF

    if ! run_compile_in_dir "$project_dir" "main.seen" "$output_file" "$log_file"; then
        echo "FAIL: whole-module package import did not compile"
        cat "$log_file"
        exit 1
    fi

    if ! "$output_file" >/dev/null 2>&1; then
        echo "FAIL: whole-module package import binary exited non-zero"
        cat "$log_file"
        exit 1
    fi

    echo "PASS: whole-module package imports keep symbols visible"
}

run_manifest_companion_module_visibility_case() {
    local package_dir="$TMP_ROOT/seen_facade"
    local project_dir="$TMP_ROOT/manifest_companion_module_visibility"
    local output_file="$project_dir/manifest_companion_module_visibility"
    local log_file="$project_dir/manifest_companion_module_visibility.log"

    cleanup_seen_artifacts
    mkdir -p "$package_dir/src" "$project_dir"

    cat >"$package_dir/Seen.toml" <<'EOF'
[project]
name = "seen_facade"
version = "0.1.0"
language = "en"

modules = [
    "src/shared_type.seen",
    "src/facade_helper.seen",
    "src/facade.seen",
]
EOF

    cat >"$package_dir/src/shared_type.seen" <<'EOF'
class SharedType {
    static fun answer() r: Int {
        return 42
    }
}
EOF

    cat >"$package_dir/src/facade_helper.seen" <<'EOF'
fun facadeHelper(value: Int) r: Int {
    return value
}
EOF

    cat >"$package_dir/src/facade.seen" <<'EOF'
fun facadeValue() r: Int {
    return facadeHelper(SharedType.answer())
}
EOF

    cat >"$project_dir/Seen.toml" <<'EOF'
[project]
name = "manifest_companion_module_visibility"
version = "0.1.0"
language = "en"

[dependencies]
seen_facade = { path = "../seen_facade" }
EOF

    cat >"$project_dir/main.seen" <<'EOF'
import seen_facade::src::facade.{facadeValue}

fun main() r: Int {
    if facadeValue() == 42 {
        return 0
    }
    return 1
}
EOF

    if ! run_compile_in_dir "$project_dir" "main.seen" "$output_file" "$log_file"; then
        echo "FAIL: manifest companion module visibility case did not compile"
        cat "$log_file"
        exit 1
    fi

    if ! "$output_file" >/dev/null 2>&1; then
        echo "FAIL: manifest companion module visibility binary exited non-zero"
        cat "$log_file"
        exit 1
    fi

    echo "PASS: manifest companion modules keep facade helper symbols visible"
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
run_success_case "C13 imported helper global state" "$C13_HELPER_GLOBAL_SRC" "$TMP_ROOT/c13_imported_helper_global_state" "$TMP_ROOT/c13_imported_helper_global_state.log"
run_success_case "C13 module-const local bind" "$C13_MODULE_CONST_BIND_SRC" "$TMP_ROOT/c13_module_const_local_bind" "$TMP_ROOT/c13_module_const_local_bind.log"
run_success_case "package-local duplicate immutable constants link cleanly" "$PACKAGE_LOCAL_DUP_CONST_SRC" "$TMP_ROOT/package_local_duplicate_const" "$TMP_ROOT/package_local_duplicate_const.log"
run_success_case "C14 shadowed branch locals" "$C14_SHADOWED_BRANCH_SRC" "$TMP_ROOT/c14_shadowed_branch_locals" "$TMP_ROOT/c14_shadowed_branch_locals.log"
run_success_case "C12 helper global state" "$C12_HELPER_GLOBAL_SRC" "$TMP_ROOT/c12_helper_global_state" "$TMP_ROOT/c12_helper_global_state.log"
run_success_case "C12 module-const local bind" "$C12_MODULE_CONST_BIND_SRC" "$TMP_ROOT/c12_module_const_local_bind" "$TMP_ROOT/c12_module_const_local_bind.log"
run_c12_abs_path_project_case
run_non_member_abs_path_case
run_real_compiler_failure_case "C13 real compiler opt failure stops before link" "opt" "Error: optimization failed for module 1" "forced opt failure for module 1"
run_real_compiler_failure_case "C13 real compiler object failure stops before link" "object" "Error: object emission failed for module 1" "forced object emission failure for module 1"
run_check_success_case "effect(FileToken) allows restricted call" "$EFFECT_OK_SRC" "$TMP_ROOT/effect_capability_ok.log"
run_check_failure_case "effect(NetToken) rejects file capability use" "$CAPABILITY_WRONG_EFFECT_SRC" "$TMP_ROOT/capability_wrong_effect.log" 'Missing capability token for restricted operation|requires @using\(FileToken\)'
run_check_failure_case "method effect(NetToken) rejects file capability use" "$METHOD_WRONG_EFFECT_SRC" "$TMP_ROOT/method_wrong_effect.log" 'Missing capability token for restricted operation|requires @using\(FileToken\)'
run_check_failure_case "imported effect(FileToken) propagates to caller" "$IMPORTED_CAPABILITY_MISSING_SRC" "$TMP_ROOT/imported_capability_missing.log" 'Calling imported function that requires capability token|requires @using\(FileToken\)'
run_check_failure_case "aliased imported effect(FileToken) propagates to caller" "$IMPORTED_CAPABILITY_ALIAS_SRC" "$TMP_ROOT/imported_capability_alias.log" 'Calling imported function that requires capability token|requires @using\(FileToken\)'
run_check_success_case "imported effect(FileToken) stays allowed with caller capability" "$IMPORTED_CAPABILITY_OK_SRC" "$TMP_ROOT/imported_capability_ok.log"
run_check_failure_case "@send rejects non-send field" "$SEND_INVALID_SRC" "$TMP_ROOT/send_invalid.log" '@send class .* cannot contain field .* without @send'
run_check_failure_case "@sync rejects non-sync field" "$SYNC_INVALID_SRC" "$TMP_ROOT/sync_invalid.log" '@sync class .* cannot contain field .* without @sync'
run_check_failure_case "@send rejects imported non-send field" "$SEND_IMPORTED_INVALID_SRC" "$TMP_ROOT/send_imported_invalid.log" '@send class .* cannot contain field .* without @send'
run_check_failure_case "@sync rejects imported non-sync field" "$SYNC_IMPORTED_INVALID_SRC" "$TMP_ROOT/sync_imported_invalid.log" '@sync class .* cannot contain field .* without @sync'
run_check_failure_case "sealed cross-module inheritance is rejected" "$SEALED_CROSS_MODULE_ENTRY_SRC" "$TMP_ROOT/sealed_cross_module.log" 'sealed class .* cannot be extended outside'
run_check_failure_case "sealed alias-import inheritance is rejected" "$SEALED_ALIAS_ENTRY_SRC" "$TMP_ROOT/sealed_alias.log" 'sealed class .* cannot be extended outside'
run_success_case "sealed same-module inheritance stays allowed" "$SEALED_SAME_MODULE_OK_SRC" "$TMP_ROOT/sealed_same_module_ok" "$TMP_ROOT/sealed_same_module_ok.log"
run_check_failure_case "enum matches must be exhaustive without else" "$WHEN_ENUM_NON_EXHAUSTIVE_SRC" "$TMP_ROOT/when_enum_non_exhaustive.log" 'non-exhaustive match on enum'
run_check_success_case "enum matches stay allowed when all variants are covered" "$WHEN_ENUM_EXHAUSTIVE_OK_SRC" "$TMP_ROOT/when_enum_exhaustive_ok.log"
run_check_success_case "enum matches stay allowed with else arm" "$WHEN_ENUM_ELSE_OK_SRC" "$TMP_ROOT/when_enum_else_ok.log"
run_check_failure_case "unresolved free function calls are diagnosed" "$UNRESOLVED_FREE_CALL_SRC" "$TMP_ROOT/unresolved_free_call.log" 'unresolved function `chunkInBounds`'
run_success_case "Bool returns coerce Int predicates to i1" "$BOOL_RETURN_COERCION_SRC" "$TMP_ROOT/bool_return_coercion" "$TMP_ROOT/bool_return_coercion.log"
run_success_case "Bool helper logical conditions stay verifier-clean" "$BOOL_HELPER_LOGICAL_SRC" "$TMP_ROOT/bool_helper_logical" "$TMP_ROOT/bool_helper_logical.log"
run_success_case "Float32 pointer deref casts directly to Int" "$FLOAT32_PTR_DEREF_CAST_SRC" "$TMP_ROOT/float32_ptr_deref_cast" "$TMP_ROOT/float32_ptr_deref_cast.log"
run_success_case_with_ir_check "nested property method receivers preserve object pointer" "$NESTED_PROPERTY_RECEIVER_SRC" "$TMP_ROOT/nested_property_receiver" "$TMP_ROOT/nested_property_receiver.log" 'call void @PropertyRegistry_registerFloat' '= call i64 @PropertyRegistry_registerFloat'
run_success_case "FEL-22 typed stores and Float returns stay verifier-clean" "$FEL22_TYPED_STORE_RETURN_SRC" "$TMP_ROOT/fel22_typed_store_return" "$TMP_ROOT/fel22_typed_store_return.log"
run_success_case "ptr_deref_i64 and ptr_store_i64 lower to runtime builtins" "$PTR_I64_ALIAS_SRC" "$TMP_ROOT/ptr_i64_alias" "$TMP_ROOT/ptr_i64_alias.log"
run_success_case "runtime file byte arrays use pointer ABI" "$RUNTIME_FILE_BYTES_SRC" "$TMP_ROOT/runtime_file_bytes" "$TMP_ROOT/runtime_file_bytes.log"
run_success_case "UInt32 top-level globals extend into module init stores" "$UINT32_GLOBAL_INIT_SRC" "$TMP_ROOT/uint32_global_init" "$TMP_ROOT/uint32_global_init.log"
run_scalar_get_guard_case
run_success_case "9+ parameter functions parse without corruption" "$HIGH_ARITY_PARAMS_SRC" "$TMP_ROOT/high_arity_params" "$TMP_ROOT/high_arity_params.log"
run_success_case "nested continue high-arity functions compile" "$NESTED_CONTINUE_HIGH_ARITY_SRC" "$TMP_ROOT/nested_continue_high_arity" "$TMP_ROOT/nested_continue_high_arity.log"
run_c12_case
run_recovery_partial_failure_case
run_toml_project_modules_case
run_large_manifest_scanner_case
run_small_manifest_allocator_stability_case
run_local_system_dependency_case
run_build_entry_seed_case
run_root_main_build_entry_isolation_case
run_build_entry_main_fallback_case
run_legacy_whole_module_import_case
run_named_import_control_case
run_package_explicit_src_import_case
run_package_local_dotted_import_case
run_manifest_sibling_runtime_memory_case
run_package_whole_module_visibility_case
run_manifest_companion_module_visibility_case
run_missing_import_failure_case

cleanup_seen_artifacts
echo "=== Seen fix regression checks passed ==="
