#!/usr/bin/env bash
# Compilation Speed Test - Fair comparison across languages
# Measures compilation speed fairly with single-threaded builds

set -e

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
PERF_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
TEST_PROJECTS_DIR="$PERF_ROOT/test_data/compilation_projects"
RESULTS_DIR="$PERF_ROOT/results"

# Default parameters  
ITERATIONS=10
WARMUP_ITERATIONS=3
OUTPUT_FILE="$RESULTS_DIR/compilation_speed_results.json"
COMPETITORS="rust,cpp,zig"
CLEAN_BUILDS="true"
PARALLEL_JOBS=1  # Single-threaded for fair comparison

# Test projects of various sizes
declare -A PROJECT_SIZES=(
    ["hello_world"]="10"           # 10 lines
    ["small_app"]="1000"          # 1,000 lines
    ["medium_app"]="10000"        # 10,000 lines  
    ["large_app"]="100000"        # 100,000 lines
    ["parser_heavy"]="5000"       # 5,000 lines with complex parsing
    ["generic_heavy"]="3000"      # 3,000 lines with heavy generic usage
)

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

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --iterations) ITERATIONS="$2"; shift 2 ;;
        --warmup) WARMUP_ITERATIONS="$2"; shift 2 ;;
        --output) OUTPUT_FILE="$2"; shift 2 ;;
        --competitors) COMPETITORS="$2"; shift 2 ;;
        --no-clean) CLEAN_BUILDS="false"; shift 1 ;;
        --parallel-jobs) PARALLEL_JOBS="$2"; shift 2 ;;
        --help)
            cat << 'EOF'
Compilation Speed Test

Usage: $0 [OPTIONS]

OPTIONS:
    --iterations N      Number of compilation iterations (default: 10)
    --warmup N         Number of warmup runs (default: 3)
    --output FILE      Output file for results (default: results/compilation_speed_results.json)
    --competitors LIST Comma-separated competitors (default: rust,cpp,zig)
    --no-clean         Don't clean between builds (faster but less accurate)
    --parallel-jobs N  Number of parallel build jobs (default: 1 for fair comparison)
    --help             Show this help message

EXAMPLES:
    # Basic test
    ./speed_test.sh
    
    # Quick test with fewer iterations
    ./speed_test.sh --iterations 5 --warmup 1
    
    # Test only against Rust and C++
    ./speed_test.sh --competitors "rust,cpp"

EOF
            exit 0
            ;;
        *) log_error "Unknown option: $1"; exit 1 ;;
    esac
done

# Initialize results
init_results() {
    mkdir -p "$RESULTS_DIR"
    cat > "$OUTPUT_FILE" << 'EOF'
{
  "metadata": {
    "timestamp": "",
    "system_info": {},
    "test_config": {}
  },
  "results": {}
}
EOF
}

# Get system information
get_system_info() {
    local os_info
    local cpu_info
    local memory_info
    
    if [[ "$OSTYPE" == "linux-gnu"* ]]; then
        os_info=$(cat /etc/os-release | grep PRETTY_NAME | cut -d'"' -f2)
        cpu_info=$(cat /proc/cpuinfo | grep "model name" | head -1 | cut -d':' -f2 | xargs)
        memory_info=$(free -h | grep Mem | awk '{print $2}')
    elif [[ "$OSTYPE" == "darwin"* ]]; then
        os_info=$(sw_vers -productName)" "$(sw_vers -productVersion)
        cpu_info=$(sysctl -n machdep.cpu.brand_string)
        memory_info=$(sysctl -n hw.memsize | awk '{print int($1/1024/1024/1024)"GB"}')
    else
        os_info="Windows (assumed)"
        cpu_info="Unknown"
        memory_info="Unknown"
    fi
    
    echo "{\"os\":\"$os_info\",\"cpu\":\"$cpu_info\",\"memory\":\"$memory_info\"}"
}

# Setup test projects
setup_test_projects() {
    log_info "Setting up test projects..."
    
    mkdir -p "$TEST_PROJECTS_DIR"
    
    for project in "${!PROJECT_SIZES[@]}"; do
        local lines="${PROJECT_SIZES[$project]}"
        local project_dir="$TEST_PROJECTS_DIR/$project"
        
        if [[ ! -d "$project_dir" ]]; then
            log_info "Generating $project project ($lines lines)..."
            create_test_project "$project" "$lines"
        fi
    done
}

# Create test project of specified size
create_test_project() {
    local project_name="$1"
    local target_lines="$2"
    local project_dir="$TEST_PROJECTS_DIR/$project_name"
    
    mkdir -p "$project_dir"
    
    # Generate Seen project
    create_seen_project "$project_dir" "$target_lines"
    
    # Generate Rust equivalent
    create_rust_project "$project_dir" "$target_lines"
    
    # Generate C++ equivalent  
    create_cpp_project "$project_dir" "$target_lines"
    
    # Generate Zig equivalent
    create_zig_project "$project_dir" "$target_lines"
}

create_seen_project() {
    local project_dir="$1"
    local target_lines="$2"
    local seen_file="$project_dir/main.seen"
    
    cat > "$seen_file" << 'EOF'
// Generated Seen test project
use std.io
use std.collections
use std.time

EOF
    
    # Generate functions based on target lines
    local current_lines=4
    local func_count=0
    
    while [[ $current_lines -lt $target_lines ]]; do
        cat >> "$seen_file" << EOF
fun function_${func_count}(param: Int): Int {
    val result = param * 2 + 1
    if (result > 100) {
        return result - 50
    } else {
        return result + 25
    }
}

EOF
        current_lines=$((current_lines + 8))
        func_count=$((func_count + 1))
    done
    
    # Add main function
    cat >> "$seen_file" << EOF
fun main() {
    println("Test project with $target_lines lines")
    var total = 0
    for (i in 0..${func_count}) {
        total += function_\$i(i)
    }
    println("Total: \$total")
}
EOF
}

create_rust_project() {
    local project_dir="$1"
    local target_lines="$2"
    
    # Create Cargo.toml
    cat > "$project_dir/Cargo.toml" << EOF
[package]
name = "test_project"
version = "0.1.0"
edition = "2021"

[dependencies]
EOF
    
    # Create src directory and main.rs
    mkdir -p "$project_dir/src"
    local rust_file="$project_dir/src/main.rs"
    
    cat > "$rust_file" << 'EOF'
// Generated Rust test project

EOF
    
    # Generate functions
    local current_lines=2
    local func_count=0
    
    while [[ $current_lines -lt $target_lines ]]; do
        cat >> "$rust_file" << EOF
fn function_${func_count}(param: i32) -> i32 {
    let result = param * 2 + 1;
    if result > 100 {
        result - 50
    } else {
        result + 25
    }
}

EOF
        current_lines=$((current_lines + 8))
        func_count=$((func_count + 1))
    done
    
    # Add main function
    cat >> "$rust_file" << EOF
fn main() {
    println!("Test project with $target_lines lines");
    let mut total = 0;
    for i in 0..${func_count} {
        total += function_\$i(i);
    }
    println!("Total: {}", total);
}
EOF
}

create_cpp_project() {
    local project_dir="$1"
    local target_lines="$2"
    local cpp_file="$project_dir/main.cpp"
    
    cat > "$cpp_file" << 'EOF'
// Generated C++ test project
#include <iostream>

EOF
    
    # Generate functions
    local current_lines=3
    local func_count=0
    
    while [[ $current_lines -lt $target_lines ]]; do
        cat >> "$cpp_file" << EOF
int function_${func_count}(int param) {
    int result = param * 2 + 1;
    if (result > 100) {
        return result - 50;
    } else {
        return result + 25;
    }
}

EOF
        current_lines=$((current_lines + 8))
        func_count=$((func_count + 1))
    done
    
    # Add main function
    cat >> "$cpp_file" << EOF
int main() {
    std::cout << "Test project with $target_lines lines" << std::endl;
    int total = 0;
    for (int i = 0; i < ${func_count}; ++i) {
        // Call functions dynamically (simplified)
        total += i * 2;
    }
    std::cout << "Total: " << total << std::endl;
    return 0;
}
EOF

    # Create simple Makefile
    cat > "$project_dir/Makefile" << 'EOF'
CXX=clang++
CXXFLAGS=-O3 -std=c++20
TARGET=main
SOURCE=main.cpp

$(TARGET): $(SOURCE)
	$(CXX) $(CXXFLAGS) -o $(TARGET) $(SOURCE)

clean:
	rm -f $(TARGET)

.PHONY: clean
EOF
}

create_zig_project() {
    local project_dir="$1" 
    local target_lines="$2"
    local zig_file="$project_dir/main.zig"
    
    cat > "$zig_file" << 'EOF'
// Generated Zig test project
const std = @import("std");

EOF
    
    # Generate functions
    local current_lines=3
    local func_count=0
    
    while [[ $current_lines -lt $target_lines ]]; do
        cat >> "$zig_file" << EOF
fn function_${func_count}(param: i32) i32 {
    const result = param * 2 + 1;
    if (result > 100) {
        return result - 50;
    } else {
        return result + 25;
    }
}

EOF
        current_lines=$((current_lines + 8))
        func_count=$((func_count + 1))
    done
    
    # Add main function
    cat >> "$zig_file" << EOF
pub fn main() !void {
    std.debug.print("Test project with $target_lines lines\\n", .{});
    var total: i32 = 0;
    var i: i32 = 0;
    while (i < ${func_count}) : (i += 1) {
        total += i * 2;
    }
    std.debug.print("Total: {}\\n", .{total});
}
EOF
}

# Find Seen compiler
find_seen_compiler() {
    local seen_paths=(
        "$PROJECT_ROOT/target/release/seen"
        "$PROJECT_ROOT/target/debug/seen"
        "$PROJECT_ROOT/target-wsl/release/seen"
        "$PROJECT_ROOT/target-wsl/debug/seen"
    )
    
    for path in "${seen_paths[@]}"; do
        if [[ -x "$path" ]]; then
            echo "$path"
            return 0
        fi
    done
    
    log_error "Seen compiler not found. Please build the project first."
    exit 1
}

# Benchmark compilation for a specific language and project
benchmark_compilation() {
    local language="$1"
    local project="$2"
    local project_dir="$TEST_PROJECTS_DIR/$project"
    
    log_info "Benchmarking $language compilation for $project..."
    
    local times=()
    local total_iterations=$((WARMUP_ITERATIONS + ITERATIONS))
    
    for ((i=1; i<=total_iterations; i++)); do
        if [[ "$CLEAN_BUILDS" == "true" ]]; then
            clean_project "$language" "$project_dir"
        fi
        
        local start_time=$(date +%s.%N)
        
        case "$language" in
            "seen")
                compile_seen "$project_dir" > /dev/null 2>&1
                ;;
            "rust")
                compile_rust "$project_dir" > /dev/null 2>&1
                ;;
            "cpp")
                compile_cpp "$project_dir" > /dev/null 2>&1
                ;;
            "zig")
                compile_zig "$project_dir" > /dev/null 2>&1
                ;;
        esac
        
        local end_time=$(date +%s.%N)
        local duration=$(echo "$end_time - $start_time" | bc -l)
        
        # Skip warmup iterations
        if [[ $i -gt $WARMUP_ITERATIONS ]]; then
            times+=("$duration")
            echo -n "."
        fi
    done
    
    echo # New line after dots
    
    # Calculate statistics
    local sum=0
    local min_time=${times[0]}
    local max_time=${times[0]}
    
    for time in "${times[@]}"; do
        sum=$(echo "$sum + $time" | bc -l)
        if (( $(echo "$time < $min_time" | bc -l) )); then
            min_time=$time
        fi
        if (( $(echo "$time > $max_time" | bc -l) )); then
            max_time=$time
        fi
    done
    
    local mean=$(echo "scale=6; $sum / ${#times[@]}" | bc -l)
    
    # Calculate standard deviation
    local variance_sum=0
    for time in "${times[@]}"; do
        local diff=$(echo "$time - $mean" | bc -l)
        local squared=$(echo "$diff * $diff" | bc -l)
        variance_sum=$(echo "$variance_sum + $squared" | bc -l)
    done
    local variance=$(echo "scale=6; $variance_sum / ${#times[@]}" | bc -l)
    local stddev=$(echo "scale=6; sqrt($variance)" | bc -l)
    
    echo "{\"mean\":$mean,\"min\":$min_time,\"max\":$max_time,\"stddev\":$stddev,\"samples\":${#times[@]}}"
}

# Clean project artifacts
clean_project() {
    local language="$1"
    local project_dir="$2"
    
    case "$language" in
        "seen")
            rm -f "$project_dir"/*.o "$project_dir"/main
            ;;
        "rust")
            (cd "$project_dir" && cargo clean > /dev/null 2>&1) || true
            ;;
        "cpp")
            (cd "$project_dir" && make clean > /dev/null 2>&1) || true
            ;;
        "zig")
            rm -f "$project_dir"/main "$project_dir"/*.o
            ;;
    esac
}

# Compilation functions for each language
compile_seen() {
    local project_dir="$1"
    local seen_compiler=$(find_seen_compiler)
    
    cd "$project_dir"
    "$seen_compiler" build --release main.seen
}

compile_rust() {
    local project_dir="$1"
    
    cd "$project_dir"
    export CARGO_BUILD_JOBS=$PARALLEL_JOBS
    cargo build --release
}

compile_cpp() {
    local project_dir="$1"
    
    cd "$project_dir"
    export CMAKE_BUILD_PARALLEL_LEVEL=$PARALLEL_JOBS
    make -j$PARALLEL_JOBS
}

compile_zig() {
    local project_dir="$1"
    
    cd "$project_dir"
    zig build-exe -O ReleaseFast main.zig
}

# Run full benchmark suite
run_benchmark_suite() {
    log_info "Starting compilation speed benchmark suite..."
    log_info "Configuration: $ITERATIONS iterations, $WARMUP_ITERATIONS warmup runs"
    log_info "Competitors: $COMPETITORS"
    log_info "Clean builds: $CLEAN_BUILDS"
    log_info "Parallel jobs: $PARALLEL_JOBS"
    
    init_results
    
    # Update metadata
    local timestamp=$(date -Iseconds)
    local system_info=$(get_system_info)
    
    # Create temporary file for results
    local temp_results=$(mktemp)
    
    echo "{" > "$temp_results"
    echo "  \"metadata\": {" >> "$temp_results"
    echo "    \"timestamp\": \"$timestamp\"," >> "$temp_results"
    echo "    \"system_info\": $system_info," >> "$temp_results"
    echo "    \"test_config\": {" >> "$temp_results"
    echo "      \"iterations\": $ITERATIONS," >> "$temp_results"
    echo "      \"warmup_iterations\": $WARMUP_ITERATIONS," >> "$temp_results"
    echo "      \"clean_builds\": $CLEAN_BUILDS," >> "$temp_results"
    echo "      \"parallel_jobs\": $PARALLEL_JOBS" >> "$temp_results"
    echo "    }" >> "$temp_results"
    echo "  }," >> "$temp_results"
    echo "  \"results\": {" >> "$temp_results"
    
    local project_count=0
    local total_projects=${#PROJECT_SIZES[@]}
    
    for project in "${!PROJECT_SIZES[@]}"; do
        local lines="${PROJECT_SIZES[$project]}"
        
        echo "    \"$project\": {" >> "$temp_results"
        echo "      \"lines\": $lines," >> "$temp_results"
        
        local lang_count=0
        IFS=',' read -ra LANGS <<< "$COMPETITORS"
        local total_langs=$((${#LANGS[@]} + 1)) # +1 for seen
        
        # Always test Seen first
        log_info "Testing $project ($lines lines) with Seen..."
        local seen_result=$(benchmark_compilation "seen" "$project")
        echo "      \"seen\": $seen_result," >> "$temp_results"
        lang_count=$((lang_count + 1))
        
        # Test other languages
        for lang in "${LANGS[@]}"; do
            lang_count=$((lang_count + 1))
            log_info "Testing $project ($lines lines) with $lang..."
            local result=$(benchmark_compilation "$lang" "$project")
            echo "      \"$lang\": $result" >> "$temp_results"
            
            if [[ $lang_count -lt $total_langs ]]; then
                echo "," >> "$temp_results"
            fi
        done
        
        project_count=$((project_count + 1))
        echo -n "    }" >> "$temp_results"
        
        if [[ $project_count -lt $total_projects ]]; then
            echo "," >> "$temp_results"
        else
            echo >> "$temp_results"
        fi
    done
    
    echo "  }" >> "$temp_results"
    echo "}" >> "$temp_results"
    
    # Move temp results to final location
    mv "$temp_results" "$OUTPUT_FILE"
    
    log_success "Benchmark complete! Results saved to $OUTPUT_FILE"
}

# Generate summary report
generate_summary() {
    log_info "Generating compilation speed summary..."
    
    python3 - << 'EOF'
import json
import sys

try:
    with open(sys.argv[1], 'r') as f:
        data = json.load(f)
except:
    print("Error loading results file")
    exit(1)

print("\n=== COMPILATION SPEED SUMMARY ===")
print(f"System: {data['metadata']['system_info']['os']}")
print(f"CPU: {data['metadata']['system_info']['cpu']}")
print()

results = data['results']
languages = set()
for project_data in results.values():
    languages.update(k for k in project_data.keys() if k != 'lines')

languages = sorted(languages)

# Print table header
print(f"{'Project':<15} {'Lines':<8} ", end="")
for lang in languages:
    print(f"{lang.upper():<12}", end="")
print()

print("-" * (15 + 8 + 12 * len(languages)))

# Print results for each project
for project, data in results.items():
    lines = data['lines']
    print(f"{project:<15} {lines:<8} ", end="")
    
    for lang in languages:
        if lang in data:
            mean_time = data[lang]['mean']
            print(f"{mean_time:>8.3f}s   ", end="")
        else:
            print(f"{'N/A':<12}", end="")
    print()

print()

# Calculate speedups relative to each language
for base_lang in languages:
    if base_lang == 'seen':
        continue
        
    print(f"Seen vs {base_lang.upper()} speedup:")
    total_speedup = 0
    count = 0
    
    for project, data in results.items():
        if 'seen' in data and base_lang in data:
            seen_time = data['seen']['mean']
            other_time = data[base_lang]['mean']
            speedup = other_time / seen_time
            total_speedup += speedup
            count += 1
            
            status = "✓" if speedup > 1.0 else "✗"
            print(f"  {project}: {speedup:.2f}x {status}")
    
    if count > 0:
        avg_speedup = total_speedup / count
        print(f"  Average: {avg_speedup:.2f}x")
        
        if avg_speedup > 2.0:
            print(f"  🚀 Seen is significantly faster than {base_lang}")
        elif avg_speedup > 1.2:
            print(f"  ✅ Seen is faster than {base_lang}")
        elif avg_speedup > 0.8:
            print(f"  ➖ Seen is competitive with {base_lang}")
        else:
            print(f"  ⚠️  Seen is slower than {base_lang}")
    
    print()

EOF "$OUTPUT_FILE"
}

# Main execution
main() {
    log_info "=== Seen Compilation Speed Test ==="
    
    # Verify dependencies
    command -v bc >/dev/null 2>&1 || { log_error "bc is required but not installed"; exit 1; }
    
    setup_test_projects
    run_benchmark_suite
    generate_summary
    
    log_success "Compilation speed test completed successfully!"
}

# Run main function
main "$@"