#!/bin/bash
# Main benchmark runner script for Seen language performance validation

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
BENCHMARK_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$BENCHMARK_DIR")"
RESULTS_DIR="$BENCHMARK_DIR/results"
REPORTS_DIR="$BENCHMARK_DIR/reports"
ITERATIONS=100
MODE="jit"  # Default mode

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --mode)
            MODE="$2"
            shift 2
            ;;
        --iterations)
            ITERATIONS="$2"
            shift 2
            ;;
        --category)
            CATEGORY="$2"
            shift 2
            ;;
        --validate)
            VALIDATE=true
            shift
            ;;
        --compare)
            COMPARE=true
            BASELINE="$2"
            shift 2
            ;;
        --help)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --mode [jit|aot]       Execution mode (default: jit)"
            echo "  --iterations N         Number of iterations (default: 100)"
            echo "  --category NAME        Run specific category only"
            echo "  --validate            Validate performance claims"
            echo "  --compare BASELINE    Compare against baseline file"
            echo "  --help                Show this help message"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

# Create directories
mkdir -p "$RESULTS_DIR"
mkdir -p "$REPORTS_DIR"

echo -e "${BLUE}=== Seen Language Performance Benchmark Suite ===${NC}"
echo -e "Mode: ${GREEN}$MODE${NC}"
echo -e "Iterations: ${GREEN}$ITERATIONS${NC}"
echo ""

# Function to run benchmarks
run_benchmark_category() {
    local category=$1
    local seen_binary=$2
    
    echo -e "${YELLOW}Running $category benchmarks...${NC}"
    
    case $category in
        microbenchmarks)
            "$seen_binary" run "$BENCHMARK_DIR/microbenchmarks/arithmetic_ops.seen" \
                --mode "$MODE" \
                --iterations "$ITERATIONS" \
                --output "$RESULTS_DIR/${category}_${MODE}.json"
            
            "$seen_binary" run "$BENCHMARK_DIR/microbenchmarks/memory_ops.seen" \
                --mode "$MODE" \
                --iterations "$ITERATIONS" \
                --append "$RESULTS_DIR/${category}_${MODE}.json"
            
            "$seen_binary" run "$BENCHMARK_DIR/microbenchmarks/string_ops.seen" \
                --mode "$MODE" \
                --iterations "$ITERATIONS" \
                --append "$RESULTS_DIR/${category}_${MODE}.json"
            ;;
        
        systems)
            "$seen_binary" run "$BENCHMARK_DIR/systems/threading_benchmarks.seen" \
                --mode "$MODE" \
                --iterations "$ITERATIONS" \
                --output "$RESULTS_DIR/${category}_${MODE}.json"
            ;;
        
        real_world)
            "$seen_binary" run "$BENCHMARK_DIR/real_world/web_server.seen" \
                --mode "$MODE" \
                --iterations "$((ITERATIONS / 10))" \
                --output "$RESULTS_DIR/${category}_${MODE}.json"
            
            "$seen_binary" run "$BENCHMARK_DIR/real_world/json_parser.seen" \
                --mode "$MODE" \
                --iterations "$ITERATIONS" \
                --append "$RESULTS_DIR/${category}_${MODE}.json"
            ;;
        
        *)
            echo -e "${RED}Unknown category: $category${NC}"
            return 1
            ;;
    esac
    
    echo -e "${GREEN}✓ $category benchmarks completed${NC}"
}

# Build Seen benchmarks
echo -e "${BLUE}Building Seen benchmarks...${NC}"
cd "$PROJECT_ROOT"

# Check if Seen compiler exists
if [ ! -f "target/release/seen" ]; then
    echo -e "${YELLOW}Seen compiler not found, building...${NC}"
    cargo build --release --bin seen
fi

SEEN_BIN="$PROJECT_ROOT/target/release/seen"

# Build competitor benchmarks
echo -e "${BLUE}Building competitor benchmarks...${NC}"

# Rust
if command -v cargo &> /dev/null; then
    echo "Building Rust benchmarks..."
    cd "$BENCHMARK_DIR/competitors/rust"
    cargo build --release --quiet
    cd "$PROJECT_ROOT"
else
    echo -e "${YELLOW}Rust not found, skipping Rust benchmarks${NC}"
fi

# C++
if command -v g++ &> /dev/null; then
    echo "Building C++ benchmarks..."
    g++ -O3 -march=native -std=c++20 \
        "$BENCHMARK_DIR/competitors/cpp/arithmetic_bench.cpp" \
        -o "$BENCHMARK_DIR/competitors/cpp/arithmetic_bench"
else
    echo -e "${YELLOW}g++ not found, skipping C++ benchmarks${NC}"
fi

# Zig
if command -v zig &> /dev/null; then
    echo "Building Zig benchmarks..."
    cd "$BENCHMARK_DIR/competitors/zig"
    zig build-exe arithmetic_bench.zig -O ReleaseFast
    cd "$PROJECT_ROOT"
else
    echo -e "${YELLOW}Zig not found, skipping Zig benchmarks${NC}"
fi

echo ""

# Run benchmarks
if [ -n "$CATEGORY" ]; then
    # Run specific category
    run_benchmark_category "$CATEGORY" "$SEEN_BIN"
else
    # Run all categories
    for category in microbenchmarks systems real_world; do
        run_benchmark_category "$category" "$SEEN_BIN"
        echo ""
    done
fi

# Run competitor benchmarks
echo -e "${BLUE}Running competitor benchmarks...${NC}"

if [ -f "$BENCHMARK_DIR/competitors/rust/target/release/arithmetic_bench" ]; then
    echo "Running Rust benchmarks..."
    "$BENCHMARK_DIR/competitors/rust/target/release/arithmetic_bench" \
        > "$RESULTS_DIR/rust_results.txt"
fi

if [ -f "$BENCHMARK_DIR/competitors/cpp/arithmetic_bench" ]; then
    echo "Running C++ benchmarks..."
    "$BENCHMARK_DIR/competitors/cpp/arithmetic_bench" \
        > "$RESULTS_DIR/cpp_results.txt"
fi

if [ -f "$BENCHMARK_DIR/competitors/zig/arithmetic_bench" ]; then
    echo "Running Zig benchmarks..."
    "$BENCHMARK_DIR/competitors/zig/arithmetic_bench" \
        > "$RESULTS_DIR/zig_results.txt"
fi

echo ""

# Validate performance claims if requested
if [ "$VALIDATE" = true ]; then
    echo -e "${BLUE}Validating performance claims...${NC}"
    "$SEEN_BIN" run "$BENCHMARK_DIR/harness/runner.seen" \
        --validate-claims \
        --input-dir "$RESULTS_DIR" \
        --output "$REPORTS_DIR/validation_report.json"
    
    # Check validation results
    if grep -q '"validation_status": "validated"' "$REPORTS_DIR/validation_report.json"; then
        echo -e "${GREEN}✓ Performance claims validated!${NC}"
    else
        echo -e "${YELLOW}⚠ Some performance claims could not be validated${NC}"
        cat "$REPORTS_DIR/validation_report.json"
    fi
fi

# Compare with baseline if requested
if [ "$COMPARE" = true ] && [ -n "$BASELINE" ]; then
    echo -e "${BLUE}Comparing with baseline...${NC}"
    "$SEEN_BIN" run "$BENCHMARK_DIR/harness/runner.seen" \
        --compare "$BASELINE" "$RESULTS_DIR/microbenchmarks_${MODE}.json" \
        --output "$REPORTS_DIR/comparison_report.json"
    
    # Check for regressions
    if grep -q '"regression_detected": true' "$REPORTS_DIR/comparison_report.json"; then
        echo -e "${RED}✗ Performance regression detected!${NC}"
        cat "$REPORTS_DIR/comparison_report.json"
        exit 1
    else
        echo -e "${GREEN}✓ No performance regressions detected${NC}"
    fi
fi

# Generate final report
echo -e "${BLUE}Generating comprehensive report...${NC}"
"$SEEN_BIN" run "$BENCHMARK_DIR/harness/reporter.seen" \
    --input-dir "$RESULTS_DIR" \
    --output-dir "$REPORTS_DIR" \
    --formats json,markdown,html

echo ""
echo -e "${GREEN}=== Benchmark Suite Completed ===${NC}"
echo -e "Results saved to: ${BLUE}$RESULTS_DIR${NC}"
echo -e "Reports saved to: ${BLUE}$REPORTS_DIR${NC}"
echo ""

# Display summary
if [ -f "$REPORTS_DIR/benchmark_report.md" ]; then
    echo -e "${BLUE}Executive Summary:${NC}"
    head -n 30 "$REPORTS_DIR/benchmark_report.md"
fi