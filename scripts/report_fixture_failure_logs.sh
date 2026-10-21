#!/usr/bin/env bash
# Best-effort, bounded diagnostics for failed nested regression harnesses.
# Callers retain and return their original exit status.
seen_report_fixture_failure_logs() {
    local fixture_dir=$1
    local fixture_log
    local fixture_root="${SEEN_ARTIFACT_ROOT:-}"
    [ -n "$fixture_root" ] || return 0
    case "$fixture_dir" in
        "$fixture_root"/*) ;;
        *) return 0 ;;
    esac
    [ -d "$fixture_dir" ] && [ ! -L "$fixture_dir" ] || return 0
    # The outer gate retains a bounded tail. Printing successful logs after
    # the failure could erase the actual diagnostic from that tail. Prefer
    # the active log; legacy harnesses use their most recently written log.
    while IFS= read -r -d '' fixture_log; do
        [ -f "$fixture_log" ] && [ ! -L "$fixture_log" ] || continue
        printf '\nFixture log tail: %s (at most 16384 bytes)\n' "$fixture_log" >&2
        tail -c 16384 -- "$fixture_log" >&2 || true
        if [ -n "${ROOT_DIR:-}" ]; then
            python3 "$ROOT_DIR/scripts/report_ir_failure_context.py" "$fixture_log" "$ROOT_DIR" >&2 || true
        fi
    done < <(python3 - "$fixture_dir" <<'PYLOG'
import os
import stat
import sys
from pathlib import Path

candidates = []
try:
    for index, path in enumerate(Path(sys.argv[1]).iterdir()):
        if index >= 1024:
            break
        if path.suffix != ".log":
            continue
        info = path.lstat()
        if stat.S_ISREG(info.st_mode):
            candidates.append((path.name == "00-active.log", info.st_mtime_ns, path.name, path))
    if candidates:
        sys.stdout.buffer.write(os.fsencode(max(candidates)[3]) + b"\0")
except OSError:
    pass  # Diagnostics never replace the original gate failure.
PYLOG
    )
    return 0
}
