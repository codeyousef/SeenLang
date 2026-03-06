#!/bin/bash
# Performance Comparison Tool
# Compares two JSONL benchmark result files and detects regressions
#
# Usage: scripts/perf_compare.sh --baseline=<file> --current=<file> [--threshold-runtime=5] [--threshold-binary=10]

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

BASELINE=""
CURRENT=""
THRESHOLD_RUNTIME=5    # % regression threshold for runtime
THRESHOLD_BINARY=10    # % regression threshold for binary size

# Parse arguments
for arg in "$@"; do
    case $arg in
        --baseline=*) BASELINE="${arg#*=}" ;;
        --current=*) CURRENT="${arg#*=}" ;;
        --threshold-runtime=*) THRESHOLD_RUNTIME="${arg#*=}" ;;
        --threshold-binary=*) THRESHOLD_BINARY="${arg#*=}" ;;
        --help|-h)
            echo "Usage: $0 --baseline=<file> --current=<file> [options]"
            echo ""
            echo "Options:"
            echo "  --baseline=<file>          Baseline JSONL results file"
            echo "  --current=<file>           Current JSONL results file"
            echo "  --threshold-runtime=<N>    Runtime regression threshold in % (default: 5)"
            echo "  --threshold-binary=<N>     Binary size regression threshold in % (default: 10)"
            exit 0
            ;;
    esac
done

if [ -z "$BASELINE" ] || [ -z "$CURRENT" ]; then
    echo -e "${RED}ERROR: Both --baseline and --current are required${NC}"
    echo "Usage: $0 --baseline=<file> --current=<file>"
    exit 1
fi

if [ ! -f "$BASELINE" ]; then
    echo -e "${RED}ERROR: Baseline file not found: $BASELINE${NC}"
    exit 1
fi

if [ ! -f "$CURRENT" ]; then
    echo -e "${RED}ERROR: Current file not found: $CURRENT${NC}"
    exit 1
fi

echo -e "${BLUE}=== Performance Comparison ===${NC}"
echo "Baseline: $BASELINE"
echo "Current:  $CURRENT"
echo "Thresholds: runtime=${THRESHOLD_RUNTIME}%, binary=${THRESHOLD_BINARY}%"
echo ""

# Header
printf "%-35s %12s %12s %10s %s\n" "Benchmark" "Baseline" "Current" "Change" "Status"
printf "%-35s %12s %12s %10s %s\n" "---------" "--------" "-------" "------" "------"

REGRESSIONS=0
IMPROVEMENTS=0
UNCHANGED=0

# Process each benchmark from current results
while IFS= read -r line; do
    # Skip summary lines
    if echo "$line" | grep -q '"_summary"'; then continue; fi

    # Extract benchmark name and current values
    NAME=$(echo "$line" | grep -oP '"name"\s*:\s*"[^"]*"' | sed 's/.*"\([^"]*\)"/\1/' | head -1)
    CURR_RUNTIME=$(echo "$line" | grep -oP '"runtime_ms"\s*:\s*[0-9.]+' | grep -oP '[0-9.]+$' || echo "")
    CURR_BINARY=$(echo "$line" | grep -oP '"binary_kb"\s*:\s*[0-9.]+' | grep -oP '[0-9.]+$' || echo "")
    STATUS=$(echo "$line" | grep -oP '"status"\s*:\s*"[^"]*"' | sed 's/.*"\([^"]*\)"/\1/' | head -1)

    if [ -z "$NAME" ] || [ "$STATUS" != "pass" ]; then continue; fi

    # Find matching baseline entry
    BASE_LINE=$(grep "\"$NAME\"" "$BASELINE" | head -1)
    if [ -z "$BASE_LINE" ]; then
        printf "%-35s %12s %12s %10s %s\n" "$NAME" "N/A" "${CURR_RUNTIME}ms" "NEW" ""
        continue
    fi

    BASE_RUNTIME=$(echo "$BASE_LINE" | grep -oP '"runtime_ms"\s*:\s*[0-9.]+' | grep -oP '[0-9.]+$' || echo "")
    BASE_BINARY=$(echo "$BASE_LINE" | grep -oP '"binary_kb"\s*:\s*[0-9.]+' | grep -oP '[0-9.]+$' || echo "")

    # Compare runtime
    if [ -n "$CURR_RUNTIME" ] && [ -n "$BASE_RUNTIME" ]; then
        # Calculate percentage change using awk
        CHANGE=$(awk "BEGIN { if ($BASE_RUNTIME > 0) printf \"%.1f\", ($CURR_RUNTIME - $BASE_RUNTIME) / $BASE_RUNTIME * 100; else print 0 }")
        CHANGE_ABS=$(awk "BEGIN { v=$CHANGE; if (v < 0) v = -v; printf \"%.1f\", v }")

        if awk "BEGIN { exit !($CHANGE > $THRESHOLD_RUNTIME) }"; then
            # Regression
            printf "%-35s %10sms %10sms %9s%% ${RED}REGRESSION${NC}\n" "$NAME" "$BASE_RUNTIME" "$CURR_RUNTIME" "+$CHANGE"
            REGRESSIONS=$((REGRESSIONS + 1))
        elif awk "BEGIN { exit !($CHANGE < -$THRESHOLD_RUNTIME) }"; then
            # Improvement
            printf "%-35s %10sms %10sms %9s%% ${GREEN}FASTER${NC}\n" "$NAME" "$BASE_RUNTIME" "$CURR_RUNTIME" "$CHANGE"
            IMPROVEMENTS=$((IMPROVEMENTS + 1))
        else
            printf "%-35s %10sms %10sms %9s%% OK\n" "$NAME" "$BASE_RUNTIME" "$CURR_RUNTIME" "$CHANGE"
            UNCHANGED=$((UNCHANGED + 1))
        fi
    fi

    # Compare binary size
    if [ -n "$CURR_BINARY" ] && [ -n "$BASE_BINARY" ]; then
        BIN_CHANGE=$(awk "BEGIN { if ($BASE_BINARY > 0) printf \"%.1f\", ($CURR_BINARY - $BASE_BINARY) / $BASE_BINARY * 100; else print 0 }")
        if awk "BEGIN { exit !($BIN_CHANGE > $THRESHOLD_BINARY) }"; then
            echo -e "  ${YELLOW}Binary size: ${BASE_BINARY}KB -> ${CURR_BINARY}KB (+${BIN_CHANGE}%)${NC}"
            REGRESSIONS=$((REGRESSIONS + 1))
        fi
    fi

done < "$CURRENT"

echo ""
echo -e "${BLUE}=== Summary ===${NC}"
echo -e "Regressions:  ${RED}$REGRESSIONS${NC}"
echo -e "Improvements: ${GREEN}$IMPROVEMENTS${NC}"
echo -e "Unchanged:    $UNCHANGED"

if [ $REGRESSIONS -gt 0 ]; then
    echo ""
    echo -e "${RED}SLO VIOLATION: $REGRESSIONS regression(s) detected${NC}"
    exit 1
fi

echo ""
echo -e "${GREEN}All benchmarks within SLO thresholds${NC}"
exit 0
