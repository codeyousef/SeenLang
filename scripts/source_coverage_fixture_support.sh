#!/usr/bin/env bash
# Shared coverage export policy; each caller owns its fixture and source scope.
seen_source_coverage_tool() (
    # Scope admission belongs to the calling harness. Keep the profiler's
    # address-space ceiling separate from that aggregate hard-memory limit.
    local limit_kb=${SEEN_OPT_VMEM_KB:-}
    case "$limit_kb" in
        ''|*[!0-9]*) echo 'source coverage: missing helper VMEM limit' >&2; return 126 ;;
    esac
    if [ "$limit_kb" -le 0 ] || [ "$limit_kb" -gt 2097152 ]; then
        echo 'source coverage: invalid helper VMEM limit' >&2
        return 126
    fi
    ulimit -S -v "$limit_kb" || return 126
    local active_kb
    active_kb=$(ulimit -S -v) || return 126
    case "$active_kb" in ''|*[!0-9]*) return 126 ;; esac
    [ "$active_kb" -le "$limit_kb" ] || return 126
    timeout --foreground --kill-after=5s 120s "$@"
)

seen_verify_source_coverage() {
    local prefix=$1
    shift
    local profiles=() objects=() sources=()
    while [ "$#" -gt 0 ]; do
        case "$1" in
            --fixture)
                [ "$#" -ge 3 ] || return 2
                objects+=(-object "$2"); profiles+=("$3"); shift 3
                ;;
            --source)
                [ "$#" -ge 2 ] || return 2
                sources+=(--source "$2"); shift 2
                ;;
            *) echo "invalid source coverage argument: $1" >&2; return 2 ;;
        esac
    done
    [ "${#profiles[@]}" -gt 0 ] && [ "${#sources[@]}" -gt 0 ] || return 2
    seen_source_coverage_tool llvm-profdata merge -sparse --num-threads=1 \
        "${profiles[@]}" -o "$prefix.profdata" || return
    seen_source_coverage_tool llvm-cov export --num-threads=1 "${objects[@]}" \
        -instr-profile "$prefix.profdata" >"$prefix-coverage.json" || return
    python3 "$ROOT_DIR/scripts/check_source_coverage_thresholds.py" "$prefix-coverage.json" \
        "${sources[@]}" >"$prefix-coverage-summary.json"
}
