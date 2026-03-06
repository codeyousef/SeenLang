#!/bin/bash
# Performance Bisect Tool
# Wraps `git bisect run` with a single benchmark + threshold for regression hunting
#
# Usage: scripts/perf_bisect.sh <benchmark_file> <threshold_ms> <good_commit> <bad_commit>
# Example: scripts/perf_bisect.sh 05_nbody 200 abc1234 def5678

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m'

if [ $# -lt 4 ]; then
    echo "Usage: $0 <benchmark_file> <threshold_ms> <good_commit> <bad_commit>"
    echo ""
    echo "Arguments:"
    echo "  benchmark_file   Benchmark name without .seen (e.g., 05_nbody)"
    echo "  threshold_ms     Maximum acceptable runtime in milliseconds"
    echo "  good_commit      Known good commit hash"
    echo "  bad_commit       Known bad commit hash (usually HEAD)"
    echo ""
    echo "Example:"
    echo "  $0 05_nbody 200 abc1234 HEAD"
    exit 1
fi

BENCH_FILE="$1"
THRESHOLD_MS="$2"
GOOD_COMMIT="$3"
BAD_COMMIT="$4"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
SEEN_FILE="$ROOT_DIR/benchmarks/production/${BENCH_FILE}.seen"

if [ ! -f "$SEEN_FILE" ]; then
    echo -e "${RED}ERROR: Benchmark not found: $SEEN_FILE${NC}"
    exit 1
fi

# Create the bisect test script
BISECT_SCRIPT=$(mktemp /tmp/seen_bisect_XXXXXX.sh)
cat > "$BISECT_SCRIPT" << 'INNEREOF'
#!/bin/bash
set -e
ROOT_DIR="ROOTDIR_PLACEHOLDER"
BENCH_FILE="BENCH_PLACEHOLDER"
THRESHOLD_MS="THRESHOLD_PLACEHOLDER"

cd "$ROOT_DIR"

# Rebuild compiler
if [ -f scripts/safe_rebuild.sh ]; then
    ./scripts/safe_rebuild.sh 2>&1 | tail -3
else
    ./bootstrap/stage1_frozen compile compiler_seen/src/main_compiler.seen compiler_seen/target/seen 2>&1 | tail -1
fi

COMPILER="$ROOT_DIR/compiler_seen/target/seen"
if [ ! -f "$COMPILER" ]; then
    echo "Compiler build failed, skip this commit"
    exit 125  # git bisect skip
fi

SEEN_FILE="$ROOT_DIR/benchmarks/production/${BENCH_FILE}.seen"
OUTPUT_BIN="/tmp/bisect_bench"

# Compile benchmark
if ! "$COMPILER" compile "$SEEN_FILE" "$OUTPUT_BIN" 2>&1 | tail -1 | grep -q "Build succeeded"; then
    echo "Compilation failed, skip"
    exit 125
fi

# Run and extract time
OUTPUT=$(timeout 120 "$OUTPUT_BIN" 2>&1 || true)
MIN_TIME=$(echo "$OUTPUT" | grep -oP "(?<=Min time: )[0-9.]+" || echo "$OUTPUT" | grep -oP "(?<=Time: )[0-9.]+" || echo "")
rm -f "$OUTPUT_BIN"

if [ -z "$MIN_TIME" ]; then
    echo "No timing output, skip"
    exit 125
fi

echo "Runtime: ${MIN_TIME}ms (threshold: ${THRESHOLD_MS}ms)"

# Compare: exit 0 = good (below threshold), exit 1 = bad (above threshold)
awk "BEGIN { exit ($MIN_TIME > $THRESHOLD_MS) ? 1 : 0 }"
INNEREOF

# Fill in placeholders
sed -i "s|ROOTDIR_PLACEHOLDER|$ROOT_DIR|g" "$BISECT_SCRIPT"
sed -i "s|BENCH_PLACEHOLDER|$BENCH_FILE|g" "$BISECT_SCRIPT"
sed -i "s|THRESHOLD_PLACEHOLDER|$THRESHOLD_MS|g" "$BISECT_SCRIPT"
chmod +x "$BISECT_SCRIPT"

echo -e "${BLUE}=== Performance Bisect ===${NC}"
echo "Benchmark:  $BENCH_FILE"
echo "Threshold:  ${THRESHOLD_MS}ms"
echo "Good:       $GOOD_COMMIT"
echo "Bad:        $BAD_COMMIT"
echo "Script:     $BISECT_SCRIPT"
echo ""

cd "$ROOT_DIR"
git bisect start "$BAD_COMMIT" "$GOOD_COMMIT"
git bisect run "$BISECT_SCRIPT"
git bisect reset

rm -f "$BISECT_SCRIPT"
echo -e "${GREEN}Bisect complete${NC}"
