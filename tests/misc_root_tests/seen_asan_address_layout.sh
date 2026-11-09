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
                [ "$status" = 0 ] || exit 1
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
# Scoped GPU driver leak attribution: an SDK-only control bounds the run, but a
# leak allocated by the fixture itself, or any ASan error, is never accepted.
for result in 0 1; do
    printf 'int main(void) { return %s; }\n' "$result" > "$work/driver-control-$result.c"
    (set -x; clang -std=c11 -O0 -g -fno-omit-frame-pointer -fsanitize=address \
        "$work/driver-control-$result.c" -o "$work/driver-control-$result") >> "$work/compile.log" 2>&1
done
driver_case() {
    local expected=$1 profile=$2 layout=$3 control=$4 mode=$5 status=0
    timeout --foreground --kill-after=5s 30s bash "$ROOT_DIR/scripts/run_asan_in_hard_memory_scope.sh" \
        --address-layout "$layout" --driver-leak-profile "$profile" --driver-leak-control "$control" \
        --target-root "$work" --compile-log "$work/compile.log" -- "$work/control" "$mode" \
        > "$work/driver-$profile-$mode.log" 2>&1 || status=$?
    [ "$status" = "$expected" ] || { echo "FAIL: driver $profile $mode exited $status, expected $expected" >&2; exit 1; }
}
driver_case 0 cuda cuda "$work/driver-control-0" clean
rg -Fq 'gpu-driver-leak-attribution backend=cuda fixture=0B/0 control=0B/0' "$work/driver-cuda-clean.log"
driver_case 23 vulkan default "$work/driver-control-0" leak
rg -Fq '37 byte(s) leaked in 1 allocation(s)' "$work/driver-vulkan-leak.log"
rg -Fq 'not an approved vulkan driver module' "$work/driver-vulkan-leak.log"
driver_case 134 cuda cuda "$work/driver-control-0" heap
rg -Fq 'heap-buffer-overflow' "$work/driver-cuda-heap.log"
driver_case 126 cuda cuda "$work/driver-control-1" clean
driver_case 126 cuda cuda "$work/control" clean
driver_case 126 cuda cuda "$ROOT_DIR/scripts/run_asan_in_hard_memory_scope.sh" clean
for args in '--driver-leak-profile cuda' "--driver-leak-control $work/driver-control-0" \
    "--driver-leak-profile metal --driver-leak-control $work/driver-control-0" \
    "--sanitizer thread --driver-leak-profile cuda --driver-leak-control $work/driver-control-0"; do
    status=0
    read -r -a options <<< "$args"
    bash "$ROOT_DIR/scripts/run_asan_in_hard_memory_scope.sh" "${options[@]}" \
        --target-root "$work" --compile-log "$work/compile.log" -- "$work/control" clean > "$work/rejected.log" 2>&1 || status=$?
    [ "$status" = 126 ] || { echo "FAIL: accepted invalid driver options: $args" >&2; exit 1; }
done
echo "PASS: both ASan layouts retain heap/UAF/stack/global/leak detection; CUDA gap mappings retain poison checks; scoped driver attribution rejects fixture leaks and errors: $work"
