#!/usr/bin/env bash
# Master benchmark runner for comprehensive Seen Language performance validation
# Executes all benchmarks with proper statistical rigor and honest reporting

set -e

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
PERF_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
RESULTS_DIR="$PERF_ROOT/results"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
SESSION_DIR="$RESULTS_DIR/$TIMESTAMP"

# Default parameters
ITERATIONS=30
WARMUP_ITERATIONS=5
TIMEOUT_SECONDS=300
CATEGORIES="all"
COMPETITORS="cpp,rust,zig"
TEST_SIZE="medium"
VERBOSE=false
SKIP_SETUP=false
STATISTICAL_ONLY=false
REAL_WORLD_ONLY=false

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

# Logging functions
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
    echo -e "${RED}[ERROR]${NC} $1" >&2
}

log_header() {
    echo -e "\n${CYAN}=====================================================${NC}"
    echo -e "${CYAN} $1${NC}"
    echo -e "${CYAN}=====================================================${NC}\n"
}

# Show help information
show_help() {
    cat << EOF
Seen Language Performance Validation Suite

Usage: $0 [OPTIONS]

OPTIONS:
    --iterations N          Number of benchmark iterations (default: $ITERATIONS)
    --warmup N             Number of warmup iterations (default: $WARMUP_ITERATIONS)  
    --timeout N            Timeout per benchmark in seconds (default: $TIMEOUT_SECONDS)
    --categories LIST      Comma-separated categories to test (default: all)
                          Options: lexer,parser,codegen,runtime,memory,reactive,all
    --competitors LIST     Comma-separated competitors to test (default: $COMPETITORS)
                          Options: cpp,rust,zig,c
    --test-size SIZE       Test data size (small,medium,large) (default: $TEST_SIZE)
    --real-world-only      Run only real-world application benchmarks
    --statistical-only     Skip benchmarks, only run statistical analysis
    --skip-setup           Skip environment setup and dependency checks
    --verbose              Enable verbose output and debugging
    --clean                Clean previous results and exit
    --help                 Show this help message

CATEGORIES:
    lexer      - Lexical analysis performance vs competitors
    parser     - Parser speed and memory usage
    codegen    - Code generation quality and speed  
    runtime    - Runtime performance of generated code
    memory     - Memory management overhead analysis
    reactive   - Reactive programming abstractions cost
    real_world - Real-world application benchmarks

EXAMPLES:
    $0                                    # Run all benchmarks with defaults
    $0 --iterations 50 --verbose         # More iterations with verbose output
    $0 --categories lexer,memory         # Only lexer and memory benchmarks
    $0 --real-world-only --test-size large  # Large real-world benchmarks only
    $0 --statistical-only                # Only statistical analysis of existing data
    $0 --competitors rust,zig            # Compare only against Rust and Zig

EOF
}

# Parse command line arguments
parse_arguments() {
    while [[ $# -gt 0 ]]; do
        case $1 in
            --iterations)
                ITERATIONS="$2"
                shift 2
                ;;
            --warmup)
                WARMUP_ITERATIONS="$2"
                shift 2
                ;;
            --timeout)
                TIMEOUT_SECONDS="$2"
                shift 2
                ;;
            --categories)
                CATEGORIES="$2"
                shift 2
                ;;
            --competitors)
                COMPETITORS="$2"
                shift 2
                ;;
            --test-size)
                TEST_SIZE="$2"
                shift 2
                ;;
            --real-world-only)
                REAL_WORLD_ONLY=true
                shift
                ;;
            --statistical-only)
                STATISTICAL_ONLY=true
                shift
                ;;
            --skip-setup)
                SKIP_SETUP=true
                shift
                ;;
            --verbose)
                VERBOSE=true
                shift
                ;;
            --clean)
                log_info "Cleaning previous results..."
                rm -rf "$RESULTS_DIR"
                log_success "Results directory cleaned"
                exit 0
                ;;
            --help)
                show_help
                exit 0
                ;;
            *)
                log_error "Unknown option: $1"
                echo "Use --help for usage information"
                exit 1
                ;;
        esac
    done
}

# Check system requirements and dependencies
check_environment() {
    if [[ "$SKIP_SETUP" == "true" ]]; then
        log_info "Skipping environment setup"
        return 0
    fi
    
    log_header "Environment Setup and Validation"
    
    # Check required tools
    local required_tools=("python3" "git" "make" "cmake")
    local missing_tools=()
    
    for tool in "${required_tools[@]}"; do
        if ! command -v "$tool" &> /dev/null; then
            missing_tools+=("$tool")
        fi
    done
    
    if [[ ${#missing_tools[@]} -gt 0 ]]; then
        log_error "Missing required tools: ${missing_tools[*]}"
        log_error "Please install missing tools and try again"
        return 1
    fi
    
    # Check Python dependencies
    if ! python3 -c "import numpy, scipy, matplotlib, pandas, seaborn" &> /dev/null; then
        log_warning "Installing required Python packages..."
        pip3 install numpy scipy matplotlib pandas seaborn
    fi
    
    # Verify Seen compiler
    if [[ ! -x "$PROJECT_ROOT/target-wsl/debug/seen" ]] && [[ ! -x "$PROJECT_ROOT/target/debug/seen" ]]; then
        log_warning "Seen compiler not found, building..."
        cd "$PROJECT_ROOT"
        if [[ -d "target-wsl" ]]; then
            env CARGO_TARGET_DIR=target-wsl cargo build --release --bin seen
        else
            cargo build --release --bin seen
        fi
    fi
    
    # Check competitor languages if requested
    IFS=',' read -ra COMP_ARRAY <<< "$COMPETITORS"
    for comp in "${COMP_ARRAY[@]}"; do
        case $comp in
            cpp)
                if ! command -v clang++ &> /dev/null && ! command -v g++ &> /dev/null; then
                    log_warning "C++ compiler not found, some benchmarks will be skipped"
                fi
                ;;
            rust)
                if ! command -v rustc &> /dev/null; then
                    log_warning "Rust compiler not found, installing..."
                    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
                    source ~/.cargo/env
                fi
                ;;
            zig)
                if ! command -v zig &> /dev/null; then
                    log_warning "Zig compiler not found, some benchmarks will be skipped"
                fi
                ;;
            c)
                if ! command -v clang &> /dev/null && ! command -v gcc &> /dev/null; then
                    log_warning "C compiler not found, some benchmarks will be skipped"
                fi
                ;;
        esac
    done
    
    log_success "Environment validation completed"
}

# Setup benchmark session
setup_session() {
    log_header "Setting Up Benchmark Session"
    
    # Create session directory
    mkdir -p "$SESSION_DIR"
    mkdir -p "$SESSION_DIR"/{raw_data,logs,metadata}
    
    # Record system information
    cat > "$SESSION_DIR/metadata/system_info.json" << EOF
{
    "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
    "hostname": "$(hostname)",
    "os": "$(uname -s)",
    "kernel": "$(uname -r)",
    "architecture": "$(uname -m)",
    "cpu_info": $(cat /proc/cpuinfo 2>/dev/null | grep "model name" | head -1 | cut -d: -f2 | sed 's/^[ \t]*//' | jq -R . || echo '"Unknown"'),
    "memory_total": $(free -b 2>/dev/null | awk '/^Mem:/{print $2}' || echo "0"),
    "benchmark_config": {
        "iterations": $ITERATIONS,
        "warmup_iterations": $WARMUP_ITERATIONS,
        "timeout_seconds": $TIMEOUT_SECONDS,
        "categories": "$CATEGORIES",
        "competitors": "$COMPETITORS",
        "test_size": "$TEST_SIZE"
    }
}
EOF
    
    # Record compiler versions
    local versions_file="$SESSION_DIR/metadata/compiler_versions.json"
    echo "{" > "$versions_file"
    
    # Seen version
    if [[ -x "$PROJECT_ROOT/target-wsl/debug/seen" ]]; then
        SEEN_VERSION=$("$PROJECT_ROOT/target-wsl/debug/seen" --version 2>/dev/null | head -1 || echo "Unknown")
    elif [[ -x "$PROJECT_ROOT/target/debug/seen" ]]; then
        SEEN_VERSION=$("$PROJECT_ROOT/target/debug/seen" --version 2>/dev/null | head -1 || echo "Unknown")
    else
        SEEN_VERSION="Not found"
    fi
    echo "  \"seen\": \"$SEEN_VERSION\"," >> "$versions_file"
    
    # Other compilers
    echo "  \"rust\": \"$(rustc --version 2>/dev/null || echo 'Not installed')\"," >> "$versions_file"
    echo "  \"clang\": \"$(clang --version 2>/dev/null | head -1 || echo 'Not installed')\"," >> "$versions_file"
    echo "  \"gcc\": \"$(gcc --version 2>/dev/null | head -1 || echo 'Not installed')\"," >> "$versions_file"
    echo "  \"zig\": \"$(zig version 2>/dev/null || echo 'Not installed')\"" >> "$versions_file"
    echo "}" >> "$versions_file"
    
    log_success "Session setup completed: $SESSION_DIR"
}

# Run benchmarks for a specific category
run_category_benchmarks() {
    local category="$1"
    local category_dir="$PERF_ROOT/benchmarks/$category"
    local output_dir="$SESSION_DIR/raw_data/$category"
    
    if [[ ! -d "$category_dir" ]]; then
        log_warning "Category directory not found: $category_dir"
        return 0
    fi
    
    log_header "Running $category Benchmarks"
    mkdir -p "$output_dir"
    
    # Find all benchmark executables or scripts
    local benchmarks=($(find "$category_dir" -name "*.sh" -executable -o -name "benchmark_*" -executable | sort))
    
    if [[ ${#benchmarks[@]} -eq 0 ]]; then
        log_warning "No benchmarks found in $category_dir"
        return 0
    fi
    
    for benchmark in "${benchmarks[@]}"; do
        local benchmark_name=$(basename "$benchmark" | sed 's/\.[^.]*$//')
        log_info "Running benchmark: $benchmark_name"
        
        # Create benchmark-specific output file
        local bench_output="$output_dir/${benchmark_name}_results.json"
        local bench_log="$SESSION_DIR/logs/${category}_${benchmark_name}.log"
        
        # Run benchmark with timeout and capture output
        if timeout "$TIMEOUT_SECONDS" "$benchmark" \
            --iterations "$ITERATIONS" \
            --warmup "$WARMUP_ITERATIONS" \
            --output "$bench_output" \
            --competitors "$COMPETITORS" \
            --test-size "$TEST_SIZE" \
            --format json > "$bench_log" 2>&1; then
            
            log_success "Completed: $benchmark_name"
            
            if [[ "$VERBOSE" == "true" ]]; then
                log_info "Results written to: $bench_output"
            fi
        else
            log_error "Failed or timed out: $benchmark_name (see $bench_log)"
        fi
    done
}

# Run real-world application benchmarks
run_real_world_benchmarks() {
    local real_world_dir="$PERF_ROOT/real_world"
    local output_dir="$SESSION_DIR/raw_data/real_world"
    
    log_header "Running Real-World Application Benchmarks"
    mkdir -p "$output_dir"
    
    local applications=("json_parser" "http_server" "ray_tracer" "compression" "regex_engine")
    
    for app in "${applications[@]}"; do
        local app_dir="$real_world_dir/$app"
        
        if [[ ! -d "$app_dir" ]]; then
            log_warning "Real-world benchmark not found: $app"
            continue
        fi
        
        log_info "Running real-world benchmark: $app"
        
        local app_output="$output_dir/${app}_results.json"
        local app_log="$SESSION_DIR/logs/real_world_${app}.log"
        
        # Run the application benchmark
        if timeout "$TIMEOUT_SECONDS" "$app_dir/run_benchmark.sh" \
            --iterations "$ITERATIONS" \
            --output "$app_output" \
            --competitors "$COMPETITORS" \
            --test-size "$TEST_SIZE" \
            --format json > "$app_log" 2>&1; then
            
            log_success "Completed real-world benchmark: $app"
        else
            log_error "Failed real-world benchmark: $app (see $app_log)"
        fi
    done
}

# Perform statistical analysis on collected data
run_statistical_analysis() {
    log_header "Statistical Analysis"
    
    local analysis_output="$SESSION_DIR/analysis"
    mkdir -p "$analysis_output"
    
    # Run comprehensive statistical analysis
    log_info "Performing rigorous statistical analysis..."
    
    if python3 "$SCRIPT_DIR/statistical_analysis.py" \
        "$SESSION_DIR/raw_data" \
        --output "$analysis_output" \
        --min-samples 25 \
        --plot \
        > "$SESSION_DIR/logs/statistical_analysis.log" 2>&1; then
        
        log_success "Statistical analysis completed"
        
        # Copy analysis summary to main results
        if [[ -f "$analysis_output/statistical_analysis.json" ]]; then
            cp "$analysis_output/statistical_analysis.json" "$SESSION_DIR/"
        fi
    else
        log_error "Statistical analysis failed (see logs/statistical_analysis.log)"
        return 1
    fi
}

# Generate comprehensive performance report
generate_report() {
    log_header "Generating Performance Report"
    
    local report_output="$SESSION_DIR/performance_report.html"
    local report_log="$SESSION_DIR/logs/report_generation.log"
    
    log_info "Generating comprehensive HTML report..."
    
    if python3 "$SCRIPT_DIR/report_generator.py" \
        --data-dir "$SESSION_DIR" \
        --output "$report_output" \
        --include-plots \
        --honest-mode \
        > "$report_log" 2>&1; then
        
        log_success "Performance report generated: $report_output"
        
        # Also generate a markdown summary
        python3 "$SCRIPT_DIR/generate_markdown_summary.py" \
            "$SESSION_DIR/statistical_analysis.json" \
            > "$SESSION_DIR/PERFORMANCE_SUMMARY.md"
            
        log_success "Markdown summary: $SESSION_DIR/PERFORMANCE_SUMMARY.md"
    else
        log_error "Report generation failed (see logs/report_generation.log)"
    fi
}

# Validate performance claims against benchmark data
validate_claims() {
    log_header "Validating Performance Claims"
    
    log_info "Checking benchmark results against published claims..."
    
    local claims_output="$SESSION_DIR/claims_validation.json"
    local claims_log="$SESSION_DIR/logs/claims_validation.log"
    
    python3 "$SCRIPT_DIR/validate_claims.py" \
        --benchmark-data "$SESSION_DIR/statistical_analysis.json" \
        --output "$claims_output" \
        --verbose > "$claims_log" 2>&1
    
    if [[ $? -eq 0 ]]; then
        log_success "Claims validation completed"
    else
        log_warning "Some performance claims could not be validated (see logs)"
    fi
}

# Main execution function
main() {
    parse_arguments "$@"
    
    log_header "Seen Language Performance Validation Suite"
    log_info "Starting comprehensive performance benchmarking..."
    log_info "Session: $TIMESTAMP"
    
    # Only run statistical analysis if requested
    if [[ "$STATISTICAL_ONLY" == "true" ]]; then
        log_info "Running statistical analysis only"
        
        # Find latest results directory
        local latest_dir=$(find "$RESULTS_DIR" -maxdepth 1 -type d -name "20*" | sort | tail -1)
        if [[ -z "$latest_dir" ]]; then
            log_error "No previous benchmark results found"
            exit 1
        fi
        
        SESSION_DIR="$latest_dir"
        run_statistical_analysis
        generate_report
        validate_claims
        exit 0
    fi
    
    # Full benchmark run
    check_environment || exit 1
    setup_session
    
    # Determine which categories to run
    local categories_to_run=()
    if [[ "$REAL_WORLD_ONLY" == "true" ]]; then
        log_info "Running real-world benchmarks only"
        run_real_world_benchmarks
    else
        if [[ "$CATEGORIES" == "all" ]]; then
            categories_to_run=("lexer" "parser" "codegen" "runtime" "memory" "reactive")
        else
            IFS=',' read -ra categories_to_run <<< "$CATEGORIES"
        fi
        
        # Run category benchmarks
        for category in "${categories_to_run[@]}"; do
            run_category_benchmarks "$category"
        done
        
        # Also run real-world benchmarks unless specifically excluded
        if [[ "$CATEGORIES" == "all" ]]; then
            run_real_world_benchmarks
        fi
    fi
    
    # Analysis and reporting
    run_statistical_analysis || exit 1
    generate_report
    validate_claims
    
    # Final summary
    log_header "Benchmark Session Complete"
    log_success "Results directory: $SESSION_DIR"
    log_success "Performance report: $SESSION_DIR/performance_report.html"
    log_success "Statistical analysis: $SESSION_DIR/statistical_analysis.json"
    
    # Show quick summary
    if [[ -f "$SESSION_DIR/PERFORMANCE_SUMMARY.md" ]]; then
        echo -e "\n${CYAN}Quick Performance Summary:${NC}"
        head -20 "$SESSION_DIR/PERFORMANCE_SUMMARY.md"
        echo -e "\nSee full report for detailed analysis: $SESSION_DIR/performance_report.html"
    fi
    
    log_info "Benchmark validation completed successfully!"
}

# Execute main function with all arguments
main "$@"