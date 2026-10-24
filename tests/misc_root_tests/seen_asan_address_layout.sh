#!/usr/bin/env bash
# Prove CUDA-compatible shadow layout retains access and leak diagnostics.
set -euo pipefail
ROOT_DIR=$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)
scope=seen-asan-address-layout
if [ "${SEEN_CAPPED_PLATFORM_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$ROOT_DIR/scripts/run_capped_regression.sh" --platform "$scope" -- bash "$0"
fi
bash "$ROOT_DIR/scripts/run_capped_regression.sh" --verify-platform-active "$scope"
TMP_DIR=$(mktemp -d "$SEEN_ARTIFACT_ROOT/asan-address-layout.XXXXXX")
work=$TMP_DIR
ulimit -c 0
(set -x; clang -std=c11 -O0 -g -march=x86-64 -fno-omit-frame-pointer -fsanitize=address \
    "$ROOT_DIR/tests/production_gpu_runtime/asan_address_layout_contract.c" -o "$work/control") > "$work/compile.log" 2>&1
for layout in default cuda; do
    modes=(clean heap use-after-free stack global leak)
    if [ "$layout" = cuda ]; then modes+=(gap-clean gap-poison); fi
    for mode in "${modes[@]}"; do
        status=0
        timeout --foreground --kill-after=5s 30s bash "$ROOT_DIR/scripts/run_asan_in_hard_memory_scope.sh" \
            --address-layout "$layout" --target-root "$work" --compile-log "$work/compile.log" \
            -- "$work/control" "$mode" > "$work/$layout-$mode.stdout" 2> "$work/$layout-$mode.stderr" || status=$?
        cat -- "$work/$layout-$mode.stdout" "$work/$layout-$mode.stderr" > "$work/$layout-$mode.log"
        printf '%s\n' "$status" > "$work/$layout-$mode.status"
        case "$mode" in
            clean|gap-clean)
                [ "$status" = 0 ] && rg -Fq 'PASS: ASan address-layout control' "$work/$layout-$mode.log" || exit 1
                # Protocol consumers own stdout. Wrapper readback and sanitizer
                # setup must remain visible, but only on the diagnostic stream.
                python3 - "$work/$layout-$mode.stdout" <<'PY'
from pathlib import Path
import sys
actual = Path(sys.argv[1]).read_bytes()
expected = b"PASS: ASan address-layout control\n"
if actual != expected:
    raise SystemExit(f"FAIL: sanitizer wrapper changed fixture stdout: {actual!r}")
PY
                rg -Fq 'sanitizer-vmem soft_kb=' "$work/$layout-$mode.stderr"
                rg -Fq "sanitizer-address-layout=$layout " "$work/$layout-$mode.stderr"
                ;;
            *)
                case "$status" in 23|134) ;; *) echo "FAIL: $layout $mode exited $status, not a sanitizer finding" >&2; exit 1 ;; esac
                case "$mode" in
                    heap) finding=heap-buffer-overflow ;;
                    use-after-free) finding=heap-use-after-free ;;
                    stack) finding=stack-buffer-overflow ;;
                    global) finding=global-buffer-overflow ;;
                    leak) finding='37 byte(s) leaked in 1 allocation(s)' ;;
                    gap-poison) finding=use-after-poison ;;
                esac
                rg -Fq "$finding" "$work/$layout-$mode.log" || exit 1
                ;;
        esac
    done
done
for args in '--address-layout invalid' '--sanitizer thread --address-layout cuda'; do
    status=0
    # Fixed test-owned words only, not untrusted command text.
    read -r -a options <<< "$args"
    bash "$ROOT_DIR/scripts/run_asan_in_hard_memory_scope.sh" "${options[@]}" \
        --target-root "$work" --compile-log "$work/compile.log" -- "$work/control" clean > "$work/rejected.log" 2>&1 || status=$?
    [ "$status" = 126 ] || exit 1
done
echo "PASS: both ASan layouts retain heap/UAF/stack/global/leak detection; CUDA gap mappings retain poison checks: $work"
