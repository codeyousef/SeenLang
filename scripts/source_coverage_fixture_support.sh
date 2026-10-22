#!/usr/bin/env bash
# Shared coverage export policy; each caller owns its fixture and source scope.
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
    llvm-profdata merge -sparse "${profiles[@]}" -o "$prefix.profdata" || return
    llvm-cov export "${objects[@]}" -instr-profile "$prefix.profdata" >"$prefix-coverage.json" || return
    python3 "$ROOT_DIR/scripts/check_source_coverage_thresholds.py" "$prefix-coverage.json" \
        "${sources[@]}" >"$prefix-coverage-summary.json"
}
