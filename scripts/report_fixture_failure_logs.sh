#!/usr/bin/env bash
# Best-effort, bounded diagnostics for failed nested regression harnesses.
# Callers retain and return their original exit status.
seen_report_fixture_failure_logs() {
    local fixture_dir=$1
    local fixture_log fixture_count=0
    case "$fixture_dir" in
        "${SEEN_ARTIFACT_ROOT:?}"/*) ;;
        *) return 0 ;;
    esac
    [ -d "$fixture_dir" ] && [ ! -L "$fixture_dir" ] || return 0
    for fixture_log in "$fixture_dir"/*.log; do
        [ -f "$fixture_log" ] && [ ! -L "$fixture_log" ] || continue
        printf '\nFixture log tail: %s (at most 16384 bytes)\n' "$fixture_log" >&2
        tail -c 16384 -- "$fixture_log" >&2 || true
        fixture_count=$((fixture_count + 1))
        [ "$fixture_count" -lt 8 ] || break
    done
    return 0
}
