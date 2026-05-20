#!/usr/bin/env bash
# Shared build-performance helpers for Seen build, package, and release scripts.

seen_build_trace_script="${seen_build_trace_script:-seen-build}"
seen_build_trace_enabled=0
seen_build_trace_path=""
seen_build_trace_summary_printed=0
seen_build_trace_phases=()
seen_build_trace_durations=()
seen_build_trace_statuses=()

seen_build_trace_init() {
    seen_build_trace_script="${1:-seen-build}"
    seen_build_trace_path="${SEEN_TRACE_BUILD:-}"
    if [ -z "$seen_build_trace_path" ] && [ -n "${SEEN_BUILD_TRACE:-}" ]; then
        seen_build_trace_path="$SEEN_BUILD_TRACE"
        SEEN_TRACE_BUILD="$seen_build_trace_path"
        export SEEN_TRACE_BUILD
    fi
    if [ -n "$seen_build_trace_path" ]; then
        mkdir -p "$(dirname "$seen_build_trace_path")" 2>/dev/null || true
        seen_build_trace_enabled=1
        printf '' >> "$seen_build_trace_path"
    fi
}

seen_build_now_ms() {
    local now
    now=$(date +%s%3N 2>/dev/null || true)
    case "$now" in
        ''|*[!0-9]*)
            now=$(date +%s 2>/dev/null || echo 0)
            printf '%s000\n' "$now"
            ;;
        *)
            printf '%s\n' "$now"
            ;;
    esac
}

seen_build_json_escape() {
    printf '%s' "$1" | sed \
        -e 's/\\/\\\\/g' \
        -e 's/"/\\"/g' \
        -e ':a;N;$!ba;s/\n/\\n/g'
}

seen_build_trace_step_start() {
    seen_build_now_ms
}

seen_build_trace_step_end() {
    local phase="$1"
    local start_ms="$2"
    local status="${3:-ok}"
    local detail="${4:-}"
    local end_ms duration escaped_phase escaped_status escaped_detail

    end_ms=$(seen_build_now_ms)
    if [ -z "$start_ms" ]; then
        start_ms="$end_ms"
    fi
    duration=$((end_ms - start_ms))
    if [ "$duration" -lt 0 ] 2>/dev/null; then
        duration=0
    fi

    seen_build_trace_phases+=("$phase")
    seen_build_trace_durations+=("$duration")
    seen_build_trace_statuses+=("$status")

    if [ "$seen_build_trace_enabled" = "1" ]; then
        escaped_phase=$(seen_build_json_escape "$phase")
        escaped_status=$(seen_build_json_escape "$status")
        escaped_detail=$(seen_build_json_escape "$detail")
        printf '{"ts_ms":%s,"script":"%s","phase":"%s","status":"%s","duration_ms":%s,"detail":"%s"}\n' \
            "$end_ms" \
            "$(seen_build_json_escape "$seen_build_trace_script")" \
            "$escaped_phase" \
            "$escaped_status" \
            "$duration" \
            "$escaped_detail" >> "$seen_build_trace_path"
    fi
}

seen_build_trace_event() {
    local phase="$1"
    local status="${2:-ok}"
    local detail="${3:-}"
    local now escaped_phase escaped_status escaped_detail

    now=$(seen_build_now_ms)
    if [ "$seen_build_trace_enabled" = "1" ]; then
        escaped_phase=$(seen_build_json_escape "$phase")
        escaped_status=$(seen_build_json_escape "$status")
        escaped_detail=$(seen_build_json_escape "$detail")
        printf '{"ts_ms":%s,"script":"%s","phase":"%s","status":"%s","duration_ms":0,"detail":"%s"}\n' \
            "$now" \
            "$(seen_build_json_escape "$seen_build_trace_script")" \
            "$escaped_phase" \
            "$escaped_status" \
            "$escaped_detail" >> "$seen_build_trace_path"
    fi
}

seen_build_trace_summary() {
    if [ "$seen_build_trace_summary_printed" = "1" ]; then
        return 0
    fi
    seen_build_trace_summary_printed=1
    if [ "${#seen_build_trace_phases[@]}" -eq 0 ]; then
        return 0
    fi

    echo ""
    echo "Build timing summary:"
    local i phase duration status seconds
    i=0
    while [ "$i" -lt "${#seen_build_trace_phases[@]}" ]; do
        phase="${seen_build_trace_phases[$i]}"
        duration="${seen_build_trace_durations[$i]}"
        status="${seen_build_trace_statuses[$i]}"
        seconds=$((duration / 1000))
        printf '  %-38s %5ss  %s\n' "$phase" "$seconds" "$status"
        i=$((i + 1))
    done
    if [ "$seen_build_trace_enabled" = "1" ]; then
        echo "Trace: $seen_build_trace_path"
    fi
}

seen_build_positive_integer() {
    case "${1:-}" in
        ''|*[!0-9]*) return 1 ;;
        *) [ "$1" -gt 0 ] 2>/dev/null ;;
    esac
}

seen_build_cpu_count() {
    local cpus=""
    cpus=$(getconf _NPROCESSORS_ONLN 2>/dev/null || true)
    if ! seen_build_positive_integer "$cpus"; then
        cpus=$(nproc 2>/dev/null || true)
    fi
    if ! seen_build_positive_integer "$cpus"; then
        cpus=1
    fi
    printf '%s\n' "$cpus"
}

seen_build_hash_file() {
    local file="$1"
    if [ -f "$file" ]; then
        sha256sum "$file" 2>/dev/null | awk '{print $1}'
    fi
}

seen_build_hash_paths() {
    local path
    for path in "$@"; do
        [ -e "$path" ] || continue
        if [ -f "$path" ]; then
            sha256sum "$path"
        else
            find "$path" -type f -print0 | sort -z | xargs -0 sha256sum 2>/dev/null
        fi
    done | sha256sum | awk '{print $1}'
}

seen_build_derive_jobs() {
    local main_kb="${1:-}"
    local opt_kb="${2:-}"
    local cpus jobs opt_jobs

    cpus=$(seen_build_cpu_count)
    jobs=$cpus
    opt_jobs=$cpus

    if seen_build_positive_integer "$main_kb"; then
        # Compiler IR workers inherit a large registry snapshot. Use a
        # conservative 6 GiB-per-worker floor under explicit VM caps so the
        # memory guard remains a safety net instead of the normal scheduler.
        jobs=$((main_kb / 6291456))
        if [ "$jobs" -lt 1 ]; then
            jobs=1
        fi
        if [ "$jobs" -gt "$cpus" ]; then
            jobs=$cpus
        fi
    fi
    if [ "$jobs" -gt 8 ]; then
        jobs=8
    fi

    if seen_build_positive_integer "$opt_kb"; then
        opt_jobs=$((opt_kb / 1048576))
        if [ "$opt_jobs" -lt 1 ]; then
            opt_jobs=1
        fi
        if [ "$opt_jobs" -gt "$jobs" ]; then
            opt_jobs=$jobs
        fi
    fi
    if [ "$opt_jobs" -gt 4 ]; then
        opt_jobs=4
    fi

    printf '%s %s\n' "$jobs" "$opt_jobs"
}

seen_build_write_full_release_stamp() {
    local root="$1"
    local compiler="${2:-$root/compiler_seen/target/seen}"
    local stamp_dir="$root/target/seen-build"
    local stamp="$stamp_dir/full-release.stamp"
    local compiler_hash commit branch version

    mkdir -p "$stamp_dir"
    compiler_hash=$(seen_build_hash_file "$compiler")
    commit=$(git -C "$root" rev-parse HEAD 2>/dev/null || echo unknown)
    branch=$(git -C "$root" rev-parse --abbrev-ref HEAD 2>/dev/null || echo unknown)
    version=$("$compiler" --version 2>/dev/null | head -1 || echo unknown)
    {
        printf 'stamp_version=1\n'
        printf 'created_at=%s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
        printf 'branch=%s\n' "$branch"
        printf 'commit=%s\n' "$commit"
        printf 'compiler=%s\n' "$compiler"
        printf 'compiler_sha256=%s\n' "$compiler_hash"
        printf 'seen_version=%s\n' "$version"
        printf 'release_cpu_baseline=%s\n' "${SEEN_RELEASE_CPU_BASELINE:-native}"
    } > "$stamp"
    seen_build_trace_event "full release stamp" "ok" "$stamp"
}

seen_build_require_full_release_stamp() {
    local root="$1"
    local compiler="$2"
    local stamp="$root/target/seen-build/full-release.stamp"
    local max_age="${SEEN_RELEASE_FULL_STAMP_MAX_AGE_SECONDS:-86400}"
    local now stamp_time age expected_hash actual_hash

    if [ "${SEEN_RELEASE_SKIP_FULL_STAMP:-0}" = "1" ]; then
        echo "WARNING: full release stamp check skipped by SEEN_RELEASE_SKIP_FULL_STAMP=1." >&2
        return 0
    fi
    if [ ! -f "$stamp" ]; then
        echo "Error: release upload requires a recent full verification stamp." >&2
        echo "Run a memory-capped full rebuild first: scripts/safe_rebuild.sh --tier full" >&2
        return 1
    fi

    now=$(date +%s)
    stamp_time=$(stat -c %Y "$stamp" 2>/dev/null || stat -f %m "$stamp" 2>/dev/null || echo 0)
    age=$((now - stamp_time))
    if [ "$age" -gt "$max_age" ]; then
        echo "Error: full verification stamp is stale (${age}s old, max ${max_age}s)." >&2
        echo "Run scripts/safe_rebuild.sh --tier full before uploading." >&2
        return 1
    fi

    expected_hash=$(awk -F= '/^compiler_sha256=/ {print $2; exit}' "$stamp")
    actual_hash=$(seen_build_hash_file "$compiler")
    if [ -z "$expected_hash" ] || [ "$expected_hash" != "$actual_hash" ]; then
        echo "Error: full verification stamp does not match the compiler selected for upload." >&2
        echo "Run scripts/safe_rebuild.sh --tier full with the same compiler artifact." >&2
        return 1
    fi
}
