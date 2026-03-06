#!/usr/bin/env bash
# R4 Release Playbook: Final validation gate before Rust removal
# Orchestrates all verification steps and reports pass/fail summary.
#
# Usage:
#   bash scripts/r4_release_playbook.sh            # full run
#   bash scripts/r4_release_playbook.sh --dry-run   # print steps without executing
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT"

DRY_RUN=0
if [[ "${1:-}" == "--dry-run" ]]; then
    DRY_RUN=1
fi

echo "╔══════════════════════════════════════════════════════════════════╗"
echo "║          R4 Release Playbook: Rust Removal Validation          ║"
echo "╚══════════════════════════════════════════════════════════════════╝"
echo ""
if [[ $DRY_RUN -eq 1 ]]; then
    echo "  MODE: --dry-run (no scripts will be executed)"
    echo ""
fi

# Gate results: 0 = not run, 1 = pass, 2 = fail
R1_RESULT=0
D2_RESULT=0
S1_RESULT=0
B1_RESULT=0

run_gate() {
    local gate_name="$1"
    local description="$2"
    local script="$3"

    echo "┌──────────────────────────────────────────────────────────────────┐"
    echo "│ $gate_name: $description"
    echo "└──────────────────────────────────────────────────────────────────┘"

    if [[ $DRY_RUN -eq 1 ]]; then
        echo "  [dry-run] Would execute: bash $script"
        echo ""
        return 0
    fi

    echo "  Executing: bash $script"
    echo ""
    if bash "$script"; then
        echo ""
        echo "  => $gate_name PASSED"
        echo ""
        return 0
    else
        echo ""
        echo "  => $gate_name FAILED"
        echo ""
        return 1
    fi
}

# ── Gate R1: Rust removal readiness ──────────────────────────────────────
if run_gate "R1" "Rust removal readiness" "scripts/verify_rust_needed.sh"; then
    R1_RESULT=1
else
    R1_RESULT=2
fi

# ── Gate D2: Determinism (stage3 == stage4 fixed-point) ──────────────────
if run_gate "D2" "Fixed-point determinism (stage3 == stage4)" "scripts/validate_d2_determinism.sh"; then
    D2_RESULT=1
else
    D2_RESULT=2
fi

# ── Gate S1: Seen-only bootstrap smoke tests ─────────────────────────────
if run_gate "S1" "Seen-only bootstrap smoke tests" "scripts/run_bootstrap_seen_only.sh"; then
    S1_RESULT=1
else
    S1_RESULT=2
fi

# ── Gate B1: Full bootstrap verification + Rust symbol check ────────────
if run_gate "B1" "Full bootstrap verification (triple + Rust symbol check)" "scripts/verify_bootstrap.sh"; then
    B1_RESULT=1
else
    B1_RESULT=2
fi

# ── Summary ──────────────────────────────────────────────────────────────
status_icon() {
    case "$1" in
        0) echo "⬚ SKIPPED" ;;
        1) echo "✅ PASS" ;;
        2) echo "❌ FAIL" ;;
    esac
}

echo "╔══════════════════════════════════════════════════════════════════╗"
echo "║                  R4 RELEASE PLAYBOOK SUMMARY                   ║"
echo "╚══════════════════════════════════════════════════════════════════╝"
echo ""
echo "  R1  Rust removal readiness          $(status_icon $R1_RESULT)"
echo "  D2  Fixed-point determinism          $(status_icon $D2_RESULT)"
echo "  S1  Seen-only bootstrap              $(status_icon $S1_RESULT)"
echo "  B1  Full bootstrap verification      $(status_icon $B1_RESULT)"
echo ""

if [[ $DRY_RUN -eq 1 ]]; then
    echo "╔══════════════════════════════════════════════════════════════════╗"
    echo "║  DRY RUN COMPLETE: All gates listed. Re-run without --dry-run  ║"
    echo "║  to execute the full validation pipeline.                      ║"
    echo "╚══════════════════════════════════════════════════════════════════╝"
    exit 0
fi

if [[ $R1_RESULT -eq 1 && $D2_RESULT -eq 1 && $S1_RESULT -eq 1 && $B1_RESULT -eq 1 ]]; then
    echo "╔══════════════════════════════════════════════════════════════════╗"
    echo "║                                                                ║"
    echo "║   ✅ R4 RELEASE READY: All gates passed                       ║"
    echo "║                                                                ║"
    echo "║   The Rust compiler can be safely removed.                     ║"
    echo "║                                                                ║"
    echo "╚══════════════════════════════════════════════════════════════════╝"
    exit 0
else
    echo "╔══════════════════════════════════════════════════════════════════╗"
    echo "║                                                                ║"
    echo "║   ❌ R4 NOT READY: One or more gates failed                   ║"
    echo "║                                                                ║"
    echo "║   Resolve failures above before removing Rust.                 ║"
    echo "║                                                                ║"
    echo "╚══════════════════════════════════════════════════════════════════╝"
    exit 1
fi
