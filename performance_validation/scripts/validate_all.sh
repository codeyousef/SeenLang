#!/bin/bash
# Complete third-party validation script for Seen language performance claims
# This script ensures reproducible and unbiased performance validation

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
RESULTS_DIR="$PROJECT_ROOT/results"
TIMESTAMP=$(date +"%Y%m%d_%H%M%S")
SESSION_DIR="$RESULTS_DIR/$TIMESTAMP"

# Validation parameters
ITERATIONS=${ITERATIONS:-100}
WARMUP=${WARMUP:-10}
TEST_SIZE=${TEST_SIZE:-large}
CONFIDENCE_LEVEL=0.95

# Create session directory
mkdir -p "$SESSION_DIR"
mkdir -p "$SESSION_DIR/raw_data"
mkdir -p "$SESSION_DIR/analysis"
mkdir -p "$SESSION_DIR/validation"

echo -e "${BLUE}================================================${NC}"
echo -e "${BLUE} Seen Language Third-Party Performance Validation${NC}"
echo -e "${BLUE}================================================${NC}"
echo ""
echo "Session: $TIMESTAMP"
echo "Iterations: $ITERATIONS"
echo "Test size: $TEST_SIZE"
echo ""

# Function to log messages
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# System information
log_info "Recording system information..."
{
    echo "System Information"
    echo "=================="
    echo "Date: $(date)"
    echo "Hostname: $(hostname)"
    echo "OS: $(uname -a)"
    echo "CPU: $(lscpu | grep 'Model name' | cut -d: -f2 | xargs)"
    echo "Memory: $(free -h | grep Mem | awk '{print $2}')"
    echo "Kernel: $(uname -r)"
    echo ""
    echo "Compiler Versions"
    echo "================="
    echo "GCC: $(gcc --version | head -n1)"
    echo "Clang: $(clang --version | head -n1)"
    echo "Rust: $(rustc --version)"
    echo "Zig: $(zig version)"
    echo "Go: $(go version)"
} > "$SESSION_DIR/system_info.txt"

# Function to run benchmark category
run_benchmark() {
    local category=$1
    local script=$2
    
    log_info "Running $category benchmarks..."
    
    if [ -f "$PROJECT_ROOT/benchmarks/$category/$script" ]; then
        cd "$PROJECT_ROOT/benchmarks/$category"
        
        if [ -x "$script" ]; then
            ./"$script" \
                --iterations "$ITERATIONS" \
                --warmup "$WARMUP" \
                --output "$SESSION_DIR/raw_data/${category}_results.json" \
                2>&1 | tee "$SESSION_DIR/raw_data/${category}.log"
            
            if [ $? -eq 0 ]; then
                log_success "$category benchmark completed"
            else
                log_error "$category benchmark failed"
            fi
        else
            log_warning "$script is not executable, trying with bash..."
            bash "$script" \
                --iterations "$ITERATIONS" \
                --warmup "$WARMUP" \
                --output "$SESSION_DIR/raw_data/${category}_results.json" \
                2>&1 | tee "$SESSION_DIR/raw_data/${category}.log"
        fi
    else
        log_warning "$category benchmark script not found"
    fi
}

# Run all benchmarks
log_info "Starting benchmark suite..."

# Core language benchmarks
run_benchmark "lexer" "run_real_benchmark.sh"
run_benchmark "parser" "run_real_benchmark.sh"
run_benchmark "codegen" "run_benchmark.sh"
run_benchmark "runtime" "run_benchmark.sh"
run_benchmark "memory" "run_benchmark.sh"
run_benchmark "reactive" "run_real_benchmark.sh"

# Real-world application benchmarks
log_info "Running real-world benchmarks..."
for app in json_parser http_server ray_tracer compression regex_engine; do
    if [ -d "$PROJECT_ROOT/real_world/$app" ]; then
        run_benchmark "../real_world/$app" "run_benchmark.sh"
    fi
done

# Statistical analysis
log_info "Performing statistical analysis..."
cd "$PROJECT_ROOT"
python3 scripts/statistical_analysis.py \
    "$SESSION_DIR/raw_data" \
    --output "$SESSION_DIR/analysis" \
    --min-samples 25 \
    --plot \
    2>&1 | tee "$SESSION_DIR/analysis/statistical_analysis.log"

# Validate claims
log_info "Validating performance claims..."
python3 scripts/validate_claims.py \
    --benchmark-data "$SESSION_DIR/analysis/statistical_analysis.json" \
    --output "$SESSION_DIR/validation/claims_validation.json" \
    --verbose \
    2>&1 | tee "$SESSION_DIR/validation/claims_validation.log"

# Generate comprehensive report
log_info "Generating validation report..."
python3 scripts/report_generator.py \
    --data-dir "$SESSION_DIR" \
    --output "$SESSION_DIR/performance_validation_report.md" \
    --format markdown \
    --include-plots \
    --honest-mode \
    2>&1 | tee "$SESSION_DIR/report_generation.log"

# Create HTML version
python3 scripts/report_generator.py \
    --data-dir "$SESSION_DIR" \
    --output "$SESSION_DIR/performance_validation_report.html" \
    --format html \
    --include-plots \
    --honest-mode \
    2>&1 | tee -a "$SESSION_DIR/report_generation.log"

# Create summary
log_info "Creating validation summary..."
{
    echo "Third-Party Validation Summary"
    echo "=============================="
    echo ""
    echo "Session: $TIMESTAMP"
    echo "Date: $(date)"
    echo ""
    
    # Extract key results from claims validation
    if [ -f "$SESSION_DIR/validation/claims_validation.json" ]; then
        echo "Claims Validation Results:"
        python3 -c "
import json
with open('$SESSION_DIR/validation/claims_validation.json') as f:
    data = json.load(f)
    if 'summary' in data:
        for key, value in data['summary'].items():
            print(f'  {key}: {value}')
"
    fi
    
    echo ""
    echo "Full report available at:"
    echo "  $SESSION_DIR/performance_validation_report.md"
    echo "  $SESSION_DIR/performance_validation_report.html"
    
} > "$SESSION_DIR/VALIDATION_SUMMARY.txt"

# Create latest symlink
ln -sfn "$SESSION_DIR" "$RESULTS_DIR/latest"

# Display summary
cat "$SESSION_DIR/VALIDATION_SUMMARY.txt"

log_success "Validation complete! Results saved to $SESSION_DIR"

# Return exit code based on validation results
if grep -q "\"all_claims_validated\": true" "$SESSION_DIR/validation/claims_validation.json" 2>/dev/null; then
    log_success "All performance claims validated successfully!"
    exit 0
else
    log_warning "Some performance claims could not be validated"
    exit 1
fi