#!/usr/bin/env bash
# Flamegraph Generation Script for Seen Language Performance Analysis
# Generates visual flamegraphs to identify performance bottlenecks

set -e

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
PERF_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
OUTPUT_DIR="$PERF_ROOT/results/profiling"
FLAMEGRAPH_DIR="$SCRIPT_DIR/FlameGraph"

# Default parameters
BENCHMARK_EXECUTABLE=""
BENCHMARK_ARGS=""
DURATION=30
FREQUENCY=99
OUTPUT_NAME="flamegraph"
PROFILE_TYPE="cpu"
SEEN_EXECUTABLE=""
INCLUDE_KERNEL=false
VERBOSE=false

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1" >&2; }
log_warning() { echo -e "${YELLOW}[WARNING]${NC} $1"; }

# Show help information
show_help() {
    cat << EOF
Seen Language Flamegraph Generator

Usage: $0 [OPTIONS] <executable>

REQUIRED:
    executable                  Path to the benchmark executable to profile

OPTIONS:
    --args ARGS                Arguments to pass to the executable
    --duration N               Profiling duration in seconds (default: $DURATION)
    --frequency N              Sampling frequency in Hz (default: $FREQUENCY)
    --output NAME              Output filename prefix (default: $OUTPUT_NAME)
    --type TYPE                Profile type: cpu, memory, cache (default: $PROFILE_TYPE)
    --seen-exe PATH            Path to seen executable for language-specific profiling
    --include-kernel           Include kernel symbols in flamegraph
    --verbose                  Enable verbose output
    --help                     Show this help message

EXAMPLES:
    # Profile a benchmark executable
    $0 --duration 60 --output lexer_bench ./benchmarks/lexer/lexer_benchmark

    # Profile with specific arguments
    $0 --args "--iterations 1000" --output parser_test ./parser_benchmark

    # Memory profiling
    $0 --type memory --duration 30 ./memory_benchmark

    # Cache profiling with kernel symbols
    $0 --type cache --include-kernel --output cache_analysis ./benchmark

REQUIREMENTS:
    - perf (Linux performance tools)
    - FlameGraph tools (automatically downloaded if missing)
    - Root privileges may be required for some profiling modes

EOF
}

# Parse command line arguments
parse_arguments() {
    while [[ $# -gt 0 ]]; do
        case $1 in
            --args)
                BENCHMARK_ARGS="$2"
                shift 2
                ;;
            --duration)
                DURATION="$2"
                shift 2
                ;;
            --frequency)
                FREQUENCY="$2"
                shift 2
                ;;
            --output)
                OUTPUT_NAME="$2"
                shift 2
                ;;
            --type)
                PROFILE_TYPE="$2"
                shift 2
                ;;
            --seen-exe)
                SEEN_EXECUTABLE="$2"
                shift 2
                ;;
            --include-kernel)
                INCLUDE_KERNEL=true
                shift
                ;;
            --verbose)
                VERBOSE=true
                shift
                ;;
            --help)
                show_help
                exit 0
                ;;
            -*)
                log_error "Unknown option: $1"
                echo "Use --help for usage information"
                exit 1
                ;;
            *)
                if [[ -z "$BENCHMARK_EXECUTABLE" ]]; then
                    BENCHMARK_EXECUTABLE="$1"
                else
                    log_error "Multiple executables specified: $BENCHMARK_EXECUTABLE and $1"
                    exit 1
                fi
                shift
                ;;
        esac
    done
    
    if [[ -z "$BENCHMARK_EXECUTABLE" ]]; then
        log_error "No executable specified"
        echo "Use --help for usage information"
        exit 1
    fi
}

# Check system requirements
check_requirements() {
    log_info "Checking system requirements..."
    
    # Check if perf is available
    if ! command -v perf &> /dev/null; then
        log_error "perf is required but not installed"
        log_error "On Ubuntu/Debian: sudo apt-get install linux-perf"
        log_error "On CentOS/RHEL: sudo yum install perf"
        return 1
    fi
    
    # Check perf permissions
    if [[ ! -r /proc/sys/kernel/perf_event_paranoid ]]; then
        log_warning "Cannot read perf_event_paranoid setting"
    else
        local paranoid_level=$(cat /proc/sys/kernel/perf_event_paranoid)
        if [[ $paranoid_level -gt 1 ]]; then
            log_warning "perf_event_paranoid is set to $paranoid_level"
            log_warning "You may need to run: echo 1 | sudo tee /proc/sys/kernel/perf_event_paranoid"
            log_warning "Or run this script with sudo for full profiling capabilities"
        fi
    fi
    
    # Check if executable exists and is executable
    if [[ ! -f "$BENCHMARK_EXECUTABLE" ]]; then
        log_error "Executable not found: $BENCHMARK_EXECUTABLE"
        return 1
    fi
    
    if [[ ! -x "$BENCHMARK_EXECUTABLE" ]]; then
        log_error "File is not executable: $BENCHMARK_EXECUTABLE"
        return 1
    fi
    
    # Check for flamegraph tools
    if [[ ! -d "$FLAMEGRAPH_DIR" ]]; then
        log_info "FlameGraph tools not found, downloading..."
        download_flamegraph_tools
    fi
    
    log_success "System requirements check passed"
}

# Download FlameGraph tools
download_flamegraph_tools() {
    log_info "Downloading FlameGraph tools..."
    
    cd "$SCRIPT_DIR"
    
    if command -v git &> /dev/null; then
        git clone https://github.com/brendangregg/FlameGraph.git
    else
        log_warning "git not available, downloading as tar archive..."
        if command -v curl &> /dev/null; then
            curl -L https://github.com/brendangregg/FlameGraph/archive/master.tar.gz | tar -xz
            mv FlameGraph-master FlameGraph
        elif command -v wget &> /dev/null; then
            wget -O- https://github.com/brendangregg/FlameGraph/archive/master.tar.gz | tar -xz
            mv FlameGraph-master FlameGraph
        else
            log_error "Neither git, curl, nor wget available to download FlameGraph tools"
            log_error "Please install FlameGraph tools manually from: https://github.com/brendangregg/FlameGraph"
            return 1
        fi
    fi
    
    if [[ -d "$FLAMEGRAPH_DIR" ]]; then
        log_success "FlameGraph tools downloaded successfully"
    else
        log_error "Failed to download FlameGraph tools"
        return 1
    fi
}

# Setup output directory
setup_output() {
    mkdir -p "$OUTPUT_DIR"
    local session_dir="$OUTPUT_DIR/$(date +%Y%m%d_%H%M%S)_${OUTPUT_NAME}"
    mkdir -p "$session_dir"
    echo "$session_dir"
}

# Generate CPU flamegraph
generate_cpu_flamegraph() {
    local session_dir="$1"
    local perf_data="$session_dir/${OUTPUT_NAME}.perf.data"
    local perf_out="$session_dir/${OUTPUT_NAME}.perf.out"
    local flamegraph_svg="$session_dir/${OUTPUT_NAME}_cpu.svg"
    
    log_info "Starting CPU profiling for ${DURATION}s at ${FREQUENCY}Hz..."
    
    # Build perf command
    local perf_cmd="perf record"
    perf_cmd="$perf_cmd -F $FREQUENCY"
    perf_cmd="$perf_cmd -g"
    perf_cmd="$perf_cmd -o $perf_data"
    
    if [[ "$INCLUDE_KERNEL" == "false" ]]; then
        perf_cmd="$perf_cmd --exclude-kernel"
    fi
    
    perf_cmd="$perf_cmd -- timeout ${DURATION}s $BENCHMARK_EXECUTABLE $BENCHMARK_ARGS"
    
    if [[ "$VERBOSE" == "true" ]]; then
        log_info "Running: $perf_cmd"
    fi
    
    # Run profiling
    eval $perf_cmd > "$session_dir/perf_record.log" 2>&1
    
    if [[ ! -f "$perf_data" ]]; then
        log_error "Failed to generate perf data file"
        return 1
    fi
    
    log_info "Processing perf data to generate flamegraph..."
    
    # Convert perf data to flamegraph format
    perf script -i "$perf_data" > "$perf_out" 2>/dev/null
    
    if [[ ! -s "$perf_out" ]]; then
        log_error "Failed to extract stack traces from perf data"
        return 1
    fi
    
    # Generate flamegraph
    "$FLAMEGRAPH_DIR/stackcollapse-perf.pl" "$perf_out" | \
    "$FLAMEGRAPH_DIR/flamegraph.pl" \
        --title "CPU Flamegraph - $OUTPUT_NAME" \
        --subtitle "$(basename "$BENCHMARK_EXECUTABLE") - ${DURATION}s @ ${FREQUENCY}Hz" \
        --width 1200 \
        --height 800 \
        > "$flamegraph_svg"
    
    if [[ -f "$flamegraph_svg" ]] && [[ -s "$flamegraph_svg" ]]; then
        log_success "CPU flamegraph generated: $flamegraph_svg"
        
        # Generate summary statistics
        generate_perf_summary "$perf_data" "$session_dir/${OUTPUT_NAME}_cpu_summary.txt"
        
        return 0
    else
        log_error "Failed to generate flamegraph SVG"
        return 1
    fi
}

# Generate memory flamegraph
generate_memory_flamegraph() {
    local session_dir="$1"
    local perf_data="$session_dir/${OUTPUT_NAME}_mem.perf.data"
    local perf_out="$session_dir/${OUTPUT_NAME}_mem.perf.out"
    local flamegraph_svg="$session_dir/${OUTPUT_NAME}_memory.svg"
    
    log_info "Starting memory profiling for ${DURATION}s..."
    
    # Memory profiling requires different perf events
    local perf_cmd="perf record"
    perf_cmd="$perf_cmd -e cycles:u,cache-misses:u,page-faults:u"
    perf_cmd="$perf_cmd -g"
    perf_cmd="$perf_cmd -o $perf_data"
    perf_cmd="$perf_cmd -- timeout ${DURATION}s $BENCHMARK_EXECUTABLE $BENCHMARK_ARGS"
    
    if [[ "$VERBOSE" == "true" ]]; then
        log_info "Running: $perf_cmd"
    fi
    
    eval $perf_cmd > "$session_dir/perf_record_mem.log" 2>&1
    
    if [[ ! -f "$perf_data" ]]; then
        log_error "Failed to generate memory perf data"
        return 1
    fi
    
    log_info "Processing memory profiling data..."
    
    perf script -i "$perf_data" > "$perf_out" 2>/dev/null
    
    "$FLAMEGRAPH_DIR/stackcollapse-perf.pl" "$perf_out" | \
    "$FLAMEGRAPH_DIR/flamegraph.pl" \
        --title "Memory Flamegraph - $OUTPUT_NAME" \
        --subtitle "$(basename "$BENCHMARK_EXECUTABLE") - Memory Events" \
        --colors mem \
        --width 1200 \
        --height 800 \
        > "$flamegraph_svg"
    
    if [[ -f "$flamegraph_svg" ]] && [[ -s "$flamegraph_svg" ]]; then
        log_success "Memory flamegraph generated: $flamegraph_svg"
        
        # Generate memory-specific summary
        generate_memory_summary "$perf_data" "$session_dir/${OUTPUT_NAME}_memory_summary.txt"
        
        return 0
    else
        log_error "Failed to generate memory flamegraph"
        return 1
    fi
}

# Generate cache flamegraph
generate_cache_flamegraph() {
    local session_dir="$1"
    local perf_data="$session_dir/${OUTPUT_NAME}_cache.perf.data"
    local perf_out="$session_dir/${OUTPUT_NAME}_cache.perf.out"
    local flamegraph_svg="$session_dir/${OUTPUT_NAME}_cache.svg"
    
    log_info "Starting cache profiling for ${DURATION}s..."
    
    # Cache profiling events
    local perf_cmd="perf record"
    perf_cmd="$perf_cmd -e cache-references:u,cache-misses:u,L1-dcache-loads:u,L1-dcache-load-misses:u"
    perf_cmd="$perf_cmd -g"
    perf_cmd="$perf_cmd -o $perf_data"
    perf_cmd="$perf_cmd -- timeout ${DURATION}s $BENCHMARK_EXECUTABLE $BENCHMARK_ARGS"
    
    if [[ "$VERBOSE" == "true" ]]; then
        log_info "Running: $perf_cmd"
    fi
    
    eval $perf_cmd > "$session_dir/perf_record_cache.log" 2>&1
    
    if [[ ! -f "$perf_data" ]]; then
        log_error "Failed to generate cache perf data"
        return 1
    fi
    
    log_info "Processing cache profiling data..."
    
    perf script -i "$perf_data" > "$perf_out" 2>/dev/null
    
    "$FLAMEGRAPH_DIR/stackcollapse-perf.pl" "$perf_out" | \
    "$FLAMEGRAPH_DIR/flamegraph.pl" \
        --title "Cache Flamegraph - $OUTPUT_NAME" \
        --subtitle "$(basename "$BENCHMARK_EXECUTABLE") - Cache Events" \
        --colors blue \
        --width 1200 \
        --height 800 \
        > "$flamegraph_svg"
    
    if [[ -f "$flamegraph_svg" ]] && [[ -s "$flamegraph_svg" ]]; then
        log_success "Cache flamegraph generated: $flamegraph_svg"
        
        # Generate cache-specific summary
        generate_cache_summary "$perf_data" "$session_dir/${OUTPUT_NAME}_cache_summary.txt"
        
        return 0
    else
        log_error "Failed to generate cache flamegraph"
        return 1
    fi
}

# Generate performance summary from perf data
generate_perf_summary() {
    local perf_data="$1"
    local summary_file="$2"
    
    log_info "Generating performance summary..."
    
    {
        echo "Performance Summary - $(date)"
        echo "================================="
        echo
        echo "Executable: $BENCHMARK_EXECUTABLE"
        echo "Arguments: $BENCHMARK_ARGS"
        echo "Duration: ${DURATION}s"
        echo "Frequency: ${FREQUENCY}Hz"
        echo "Profile Type: $PROFILE_TYPE"
        echo
        
        # Basic perf report
        echo "Top Functions by CPU Usage:"
        echo "----------------------------"
        perf report -i "$perf_data" --stdio -n --sort=overhead,symbol 2>/dev/null | head -20
        
        echo
        echo "Call Graph Summary:"
        echo "------------------"
        perf report -i "$perf_data" --stdio -g --sort=overhead 2>/dev/null | head -30
        
    } > "$summary_file"
    
    if [[ -f "$summary_file" ]]; then
        log_success "Performance summary saved: $summary_file"
    fi
}

# Generate memory-specific summary
generate_memory_summary() {
    local perf_data="$1"
    local summary_file="$2"
    
    {
        echo "Memory Performance Summary - $(date)"
        echo "===================================="
        echo
        echo "Executable: $BENCHMARK_EXECUTABLE"
        echo "Arguments: $BENCHMARK_ARGS"
        echo "Duration: ${DURATION}s"
        echo
        
        # Memory events summary
        echo "Memory Events Summary:"
        echo "---------------------"
        perf report -i "$perf_data" --stdio -n --sort=overhead,symbol --event=cache-misses 2>/dev/null | head -15
        
        echo
        echo "Page Fault Analysis:"
        echo "-------------------"
        perf report -i "$perf_data" --stdio -n --sort=overhead,symbol --event=page-faults 2>/dev/null | head -10
        
    } > "$summary_file"
}

# Generate cache-specific summary
generate_cache_summary() {
    local perf_data="$1"
    local summary_file="$2"
    
    {
        echo "Cache Performance Summary - $(date)"
        echo "==================================="
        echo
        echo "Executable: $BENCHMARK_EXECUTABLE"
        echo "Arguments: $BENCHMARK_ARGS"
        echo "Duration: ${DURATION}s"
        echo
        
        # Cache statistics
        echo "Cache Performance Analysis:"
        echo "--------------------------"
        perf report -i "$perf_data" --stdio -n --sort=overhead,symbol --event=cache-misses 2>/dev/null | head -15
        
        echo
        echo "L1 Cache Analysis:"
        echo "-----------------"
        perf report -i "$perf_data" --stdio -n --sort=overhead,symbol --event=L1-dcache-load-misses 2>/dev/null | head -10
        
    } > "$summary_file"
}

# Generate comparison report
generate_comparison_report() {
    local session_dir="$1"
    local comparison_file="$session_dir/${OUTPUT_NAME}_comparison.md"
    
    log_info "Generating comparison report..."
    
    {
        echo "# Performance Analysis Report"
        echo
        echo "**Executable:** \`$(basename "$BENCHMARK_EXECUTABLE")\`"
        echo "**Arguments:** \`$BENCHMARK_ARGS\`"
        echo "**Duration:** ${DURATION}s"
        echo "**Date:** $(date)"
        echo
        echo "## Flamegraphs Generated"
        echo
        
        if [[ -f "$session_dir/${OUTPUT_NAME}_cpu.svg" ]]; then
            echo "- 🔥 [CPU Flamegraph]($(basename "${OUTPUT_NAME}_cpu.svg"))"
        fi
        
        if [[ -f "$session_dir/${OUTPUT_NAME}_memory.svg" ]]; then
            echo "- 💾 [Memory Flamegraph]($(basename "${OUTPUT_NAME}_memory.svg"))"
        fi
        
        if [[ -f "$session_dir/${OUTPUT_NAME}_cache.svg" ]]; then
            echo "- 🏆 [Cache Flamegraph]($(basename "${OUTPUT_NAME}_cache.svg"))"
        fi
        
        echo
        echo "## Analysis Summary"
        echo
        echo "### Key Findings"
        echo "- **Hot Functions:** Check the widest sections in the CPU flamegraph"
        echo "- **Memory Issues:** Look for cache misses and page faults in memory flamegraph"
        echo "- **Optimization Targets:** Focus on functions with high self-time"
        echo
        echo "### How to Read Flamegraphs"
        echo "- **Width:** Time spent in function (wider = more time)"
        echo "- **Height:** Call stack depth"
        echo "- **Color:** Different functions (click to zoom/search)"
        echo "- **Plateau:** Functions that don't call others (good optimization targets)"
        echo
        echo "### Next Steps"
        echo "1. Identify the widest sections (hot functions)"
        echo "2. Look for optimization opportunities in self-time functions"
        echo "3. Check for unexpected function calls or memory patterns"
        echo "4. Compare with baseline measurements"
        echo
        
    } > "$comparison_file"
    
    log_success "Comparison report generated: $comparison_file"
}

# Main execution function
main() {
    parse_arguments "$@"
    
    log_info "=== Seen Language Flamegraph Generator ==="
    log_info "Profiling: $BENCHMARK_EXECUTABLE"
    log_info "Type: $PROFILE_TYPE"
    log_info "Duration: ${DURATION}s"
    
    # System checks
    check_requirements || exit 1
    
    # Setup output directory
    local session_dir
    session_dir=$(setup_output)
    log_info "Output directory: $session_dir"
    
    # Generate flamegraphs based on profile type
    case "$PROFILE_TYPE" in
        cpu)
            generate_cpu_flamegraph "$session_dir" || exit 1
            ;;
        memory)
            generate_memory_flamegraph "$session_dir" || exit 1
            ;;
        cache)
            generate_cache_flamegraph "$session_dir" || exit 1
            ;;
        all)
            log_info "Generating all flamegraph types..."
            generate_cpu_flamegraph "$session_dir"
            generate_memory_flamegraph "$session_dir"
            generate_cache_flamegraph "$session_dir"
            ;;
        *)
            log_error "Unknown profile type: $PROFILE_TYPE"
            log_error "Supported types: cpu, memory, cache, all"
            exit 1
            ;;
    esac
    
    # Generate comparison report
    generate_comparison_report "$session_dir"
    
    # Final summary
    log_success "Flamegraph generation completed!"
    log_success "Results directory: $session_dir"
    
    # List generated files
    echo
    log_info "Generated files:"
    find "$session_dir" -name "*.svg" -o -name "*.txt" -o -name "*.md" | while read file; do
        echo "  - $(basename "$file")"
    done
    
    echo
    log_info "Open the .svg files in a web browser to view interactive flamegraphs"
}

# Execute main function
main "$@"