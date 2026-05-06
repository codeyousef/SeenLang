#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
OPT_VMEM_KB="${SEEN_OPT_VMEM_KB:-2097152}"

require_cmd() {
    local name="$1"
    if ! command -v "$name" >/dev/null 2>&1; then
        echo "ERROR: required prebuild gate command not found: $name" >&2
        exit 1
    fi
}

run_with_opt_cap() {
    (
        ulimit -v "$OPT_VMEM_KB" 2>/dev/null || true
        "$@"
    )
}

sweep_saved_ll_dir() {
    local source_dir="${SEEN_PREFLIGHT_LL_DIR:-}"
    if [ -z "$source_dir" ]; then
        return 0
    fi
    if [ ! -d "$source_dir" ]; then
        echo "ERROR: SEEN_PREFLIGHT_LL_DIR does not exist: $source_dir" >&2
        exit 1
    fi

    local work_dir
    work_dir="$(mktemp -d /tmp/seen_preflight_ll.XXXXXX)"

    local count=0
    local ll
    for ll in "$source_dir"/seen_module_*.ll; do
        [ -f "$ll" ] || continue
        [[ "$ll" == *.opt.ll ]] && continue
        count=$((count + 1))
        local copy="$work_dir/$(basename "$ll")"
        cp "$ll" "$copy"
        run_with_opt_cap python3 "$SCRIPT_DIR/fix_ir.py" "$copy"
        run_with_opt_cap llvm-as "$copy" -o /dev/null
    done

    if [ "$count" -eq 0 ]; then
        echo "ERROR: no seen_module_*.ll files found in SEEN_PREFLIGHT_LL_DIR=$source_dir" >&2
        rm -rf "$work_dir"
        exit 1
    fi
    echo "PASS: preflight swept $count saved Stage2 .ll file(s)"
    rm -rf "$work_dir"
}

cd "$REPO_ROOT"

require_cmd python3
require_cmd bash
require_cmd llvm-as
require_cmd opt

echo "Prebuild gates: Python and shell syntax..."
python3 -m py_compile "$SCRIPT_DIR/fix_ir.py" "$SCRIPT_DIR/check_codegen_abi_boundaries.py"
bash -n "$SCRIPT_DIR/safe_rebuild.sh" \
    "$SCRIPT_DIR/recovery_opt.sh" \
    "$SCRIPT_DIR/seen_prebuild_gates.sh" \
    "$REPO_ROOT/tests/misc_root_tests/seen_fix_ir_stage2_patterns.sh" \
    "$REPO_ROOT/tests/misc_root_tests/seen_codegen_abi_preflight.sh"

if [ "${SEEN_SKIP_CODEGEN_ABI_PREFLIGHT:-0}" != "1" ]; then
    echo "Prebuild gates: codegen ABI/import/cycle checks..."
    python3 "$SCRIPT_DIR/check_codegen_abi_boundaries.py" "$REPO_ROOT"
else
    echo "Prebuild gates: codegen ABI checks skipped by SEEN_SKIP_CODEGEN_ABI_PREFLIGHT=1"
fi

echo "Prebuild gates: codegen ABI regression fixtures..."
bash "$REPO_ROOT/tests/misc_root_tests/seen_codegen_abi_preflight.sh"

echo "Prebuild gates: Stage2 IR repair patterns under ${OPT_VMEM_KB} KiB cap..."
run_with_opt_cap bash "$REPO_ROOT/tests/misc_root_tests/seen_fix_ir_stage2_patterns.sh"

sweep_saved_ll_dir

echo "PASS: prebuild gates"
