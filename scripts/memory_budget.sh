#!/usr/bin/env bash
# Linux memory.max quantizes bytes to whole pages. Preserve upper bounds by
# admitting whole-page KiB limits before publishing compiler allocation budgets.
seen_memory_page_floor_kb() {
    [ "$#" -eq 2 ] || return 2
    local limit_kb=$1 page_bytes=$2 page_kb aligned_kb
    [[ "$limit_kb" =~ ^[1-9][0-9]{0,15}$ ]] &&
        [[ "$page_bytes" =~ ^[1-9][0-9]{0,8}$ ]] || return 2
    [ "$page_bytes" -ge 1024 ] && [ "$((page_bytes % 1024))" -eq 0 ] || return 2
    page_kb=$((page_bytes / 1024))
    aligned_kb=$((limit_kb / page_kb * page_kb))
    [ "$aligned_kb" -gt 0 ] || return 2
    printf '%s\n' "$aligned_kb"
}
