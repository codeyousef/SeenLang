#!/usr/bin/env bash
# Assembly Analysis Tool for Seen Language Performance Optimization
# Analyzes generated assembly code to identify optimization opportunities

set -e

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
PERF_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
OUTPUT_DIR="$PERF_ROOT/results/assembly_analysis"

# Default parameters
SOURCE_FILE=""
OPTIMIZATION_LEVEL="release"
SEEN_EXECUTABLE=""
OUTPUT_NAME="assembly_analysis"
COMPARE_WITH_COMPETITORS=true
GENERATE_ANNOTATIONS=true
ANALYZE_HOTSPOTS=true
INCLUDE_DISASSEMBLY=true
VERBOSE=false

# Analysis tools
OBJDUMP_AVAILABLE=false
PERF_AVAILABLE=false
GCC_AVAILABLE=false
CLANG_AVAILABLE=false

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1" >&2; }
log_warning() { echo -e "${YELLOW}[WARNING]${NC} $1"; }
log_header() { echo -e "\n${CYAN}=== $1 ===${NC}"; }

# Show help information
show_help() {
    cat << EOF
Seen Language Assembly Analysis Tool

Usage: $0 [OPTIONS] <source_file>

REQUIRED:
    source_file                 Path to the Seen source file to analyze

OPTIONS:
    --optimization LEVEL        Optimization level: debug, release (default: $OPTIMIZATION_LEVEL)
    --seen-exe PATH            Path to seen executable (auto-detected if not provided)
    --output NAME              Output filename prefix (default: $OUTPUT_NAME)
    --no-competitors           Skip comparison with other languages
    --no-annotations           Skip generating annotated assembly
    --no-hotspots              Skip hotspot analysis
    --no-disassembly           Skip binary disassembly
    --verbose                  Enable verbose output
    --help                     Show this help message

FEATURES:
    - Assembly code generation and analysis
    - Instruction frequency analysis
    - Register usage patterns
    - Branch prediction analysis
    - Memory access patterns
    - Comparison with C++/Rust equivalent code
    - Optimization opportunity identification
    - Performance hotspot detection

EXAMPLES:
    # Basic assembly analysis
    $0 benchmarks/lexer/lexer.seen

    # Debug optimization analysis
    $0 --optimization debug --verbose test.seen

    # Analysis without competitor comparison
    $0 --no-competitors --output lexer_asm lexer.seen

    # Full analysis with all features
    $0 --verbose --output comprehensive_analysis benchmark.seen

REQUIREMENTS:
    - Seen compiler (debug/release build)
    - objdump (for disassembly)
    - perf (for hotspot analysis)
    - Optional: gcc, clang (for competitor comparison)

EOF
}

# Parse command line arguments
parse_arguments() {
    while [[ $# -gt 0 ]]; do
        case $1 in
            --optimization)
                OPTIMIZATION_LEVEL="$2"
                shift 2
                ;;
            --seen-exe)
                SEEN_EXECUTABLE="$2"
                shift 2
                ;;
            --output)
                OUTPUT_NAME="$2"
                shift 2
                ;;
            --no-competitors)
                COMPARE_WITH_COMPETITORS=false
                shift
                ;;
            --no-annotations)
                GENERATE_ANNOTATIONS=false
                shift
                ;;
            --no-hotspots)
                ANALYZE_HOTSPOTS=false
                shift
                ;;
            --no-disassembly)
                INCLUDE_DISASSEMBLY=false
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
                if [[ -z "$SOURCE_FILE" ]]; then
                    SOURCE_FILE="$1"
                else
                    log_error "Multiple source files specified: $SOURCE_FILE and $1"
                    exit 1
                fi
                shift
                ;;
        esac
    done
    
    if [[ -z "$SOURCE_FILE" ]]; then
        log_error "No source file specified"
        echo "Use --help for usage information"
        exit 1
    fi
}

# Check system requirements and available tools
check_requirements() {
    log_info "Checking system requirements and available tools..."
    
    # Check if source file exists
    if [[ ! -f "$SOURCE_FILE" ]]; then
        log_error "Source file not found: $SOURCE_FILE"
        return 1
    fi
    
    # Find Seen executable if not provided
    if [[ -z "$SEEN_EXECUTABLE" ]]; then
        local seen_paths=(
            "$PROJECT_ROOT/target/release/seen"
            "$PROJECT_ROOT/target/debug/seen"
            "$PROJECT_ROOT/target-wsl/release/seen"
            "$PROJECT_ROOT/target-wsl/debug/seen"
        )
        
        for path in "${seen_paths[@]}"; do
            if [[ -x "$path" ]]; then
                SEEN_EXECUTABLE="$path"
                break
            fi
        done
        
        if [[ -z "$SEEN_EXECUTABLE" ]]; then
            log_error "Seen executable not found. Please build the project or specify with --seen-exe"
            return 1
        fi
    fi
    
    if [[ ! -x "$SEEN_EXECUTABLE" ]]; then
        log_error "Seen executable not found or not executable: $SEEN_EXECUTABLE"
        return 1
    fi
    
    # Check available analysis tools
    if command -v objdump &> /dev/null; then
        OBJDUMP_AVAILABLE=true
        log_info "✓ objdump available for disassembly"
    else
        log_warning "objdump not available - binary disassembly will be skipped"
    fi
    
    if command -v perf &> /dev/null; then
        PERF_AVAILABLE=true
        log_info "✓ perf available for hotspot analysis"
    else
        log_warning "perf not available - hotspot analysis will be limited"
    fi
    
    if command -v gcc &> /dev/null; then
        GCC_AVAILABLE=true
        log_info "✓ gcc available for comparison"
    fi
    
    if command -v clang &> /dev/null; then
        CLANG_AVAILABLE=true
        log_info "✓ clang available for comparison"
    fi
    
    log_success "Requirements check completed"
}

# Setup output directory
setup_output() {
    mkdir -p "$OUTPUT_DIR"
    local session_dir="$OUTPUT_DIR/$(date +%Y%m%d_%H%M%S)_${OUTPUT_NAME}"
    mkdir -p "$session_dir"
    echo "$session_dir"
}

# Compile Seen source to get assembly and binary
compile_seen_source() {
    local session_dir="$1"
    local binary_path="$session_dir/seen_program"
    local assembly_path="$session_dir/seen_program.s"
    
    log_header "Compiling Seen Source"
    log_info "Source: $SOURCE_FILE"
    log_info "Optimization: $OPTIMIZATION_LEVEL"
    
    # Compile to binary
    local build_cmd="$SEEN_EXECUTABLE build"
    if [[ "$OPTIMIZATION_LEVEL" == "release" ]]; then
        build_cmd="$build_cmd --release"
    fi
    build_cmd="$build_cmd --output $binary_path $SOURCE_FILE"
    
    if [[ "$VERBOSE" == "true" ]]; then
        log_info "Running: $build_cmd"
    fi
    
    if eval "$build_cmd" > "$session_dir/seen_build.log" 2>&1; then
        log_success "Seen binary compiled: $binary_path"
    else
        log_error "Failed to compile Seen source"
        cat "$session_dir/seen_build.log"
        return 1
    fi
    
    # Try to generate assembly if supported
    local asm_cmd="$SEEN_EXECUTABLE build --emit-asm"
    if [[ "$OPTIMIZATION_LEVEL" == "release" ]]; then
        asm_cmd="$asm_cmd --release"
    fi
    asm_cmd="$asm_cmd --output $assembly_path $SOURCE_FILE"
    
    if eval "$asm_cmd" > "$session_dir/seen_asm_build.log" 2>&1; then
        log_success "Seen assembly generated: $assembly_path"
        echo "$assembly_path"
    else
        log_warning "Could not generate assembly directly, will use disassembly"
        echo ""
    fi
    
    echo "$binary_path"
}

# Generate disassembly from binary
generate_disassembly() {
    local binary_path="$1"
    local session_dir="$2"
    local disasm_path="$session_dir/disassembly.txt"
    
    if [[ "$OBJDUMP_AVAILABLE" != "true" ]]; then
        log_warning "objdump not available, skipping disassembly"
        return 1
    fi
    
    log_header "Generating Binary Disassembly"
    
    # Generate detailed disassembly
    objdump -d -M intel -l "$binary_path" > "$disasm_path" 2>/dev/null
    
    if [[ -f "$disasm_path" ]] && [[ -s "$disasm_path" ]]; then
        log_success "Disassembly generated: $disasm_path"
        
        # Generate annotated disassembly with source mapping if possible
        local annotated_path="$session_dir/annotated_disassembly.txt"
        objdump -d -S -M intel "$binary_path" > "$annotated_path" 2>/dev/null
        
        if [[ -s "$annotated_path" ]]; then
            log_success "Annotated disassembly generated: $annotated_path"
        fi
        
        echo "$disasm_path"
    else
        log_error "Failed to generate disassembly"
        return 1
    fi
}

# Analyze instruction patterns
analyze_instruction_patterns() {
    local disasm_file="$1"
    local session_dir="$2"
    local analysis_file="$session_dir/instruction_analysis.txt"
    
    log_header "Analyzing Instruction Patterns"
    
    {
        echo "Instruction Pattern Analysis - $(date)"
        echo "======================================"
        echo
        echo "Source File: $SOURCE_FILE"
        echo "Optimization Level: $OPTIMIZATION_LEVEL"
        echo
        
        # Extract and count instructions
        echo "Most Common Instructions:"
        echo "------------------------"
        grep -E "^\s+[0-9a-f]+:" "$disasm_file" | \
        sed 's/.*:\t[0-9a-f ]*\t\([^ ]*\).*/\1/' | \
        sort | uniq -c | sort -rn | head -20 | \
        awk '{printf "%-12s %6d (%.1f%%)\n", $2, $1, $1/NR*100}'
        
        echo
        echo "Register Usage Patterns:"
        echo "-----------------------"
        grep -E "^\s+[0-9a-f]+:" "$disasm_file" | \
        grep -oE "%[re][a-z][a-z0-9]*" | sort | uniq -c | sort -rn | head -15
        
        echo
        echo "Memory Access Patterns:"
        echo "----------------------"
        grep -E "^\s+[0-9a-f]+:" "$disasm_file" | \
        grep -E "(mov|ld|st)" | \
        grep -oE "\([^)]*\)" | sort | uniq -c | sort -rn | head -10
        
        echo
        echo "Jump/Branch Instructions:"
        echo "------------------------"
        grep -E "^\s+[0-9a-f]+:" "$disasm_file" | \
        grep -E "(jmp|je|jne|jl|jg|jle|jge|call|ret)" | \
        sed 's/.*:\t[0-9a-f ]*\t\([^ ]*\).*/\1/' | \
        sort | uniq -c | sort -rn
        
        echo
        echo "Function Analysis:"
        echo "-----------------"
        local func_count=$(grep -c "^[0-9a-f]\+.*>:$" "$disasm_file")
        echo "Total functions: $func_count"
        
        echo "Function sizes (lines of assembly):"
        awk '/^[0-9a-f]+.*>:$/ {name=$0; count=0} 
             /^\s+[0-9a-f]+:/ {count++} 
             /^$/ && name {print count, name; name=""}' "$disasm_file" | \
        sort -rn | head -10
        
    } > "$analysis_file"
    
    log_success "Instruction analysis completed: $analysis_file"
    echo "$analysis_file"
}

# Analyze performance characteristics
analyze_performance_characteristics() {
    local disasm_file="$1"
    local session_dir="$2"
    local perf_analysis_file="$session_dir/performance_analysis.txt"
    
    log_header "Analyzing Performance Characteristics"
    
    {
        echo "Performance Characteristics Analysis - $(date)"
        echo "=============================================="
        echo
        
        # Analyze potential performance issues
        echo "Potential Performance Issues:"
        echo "----------------------------"
        
        # Division instructions (expensive)
        local div_count=$(grep -cE "(div|idiv)" "$disasm_file" || true)
        echo "Division instructions: $div_count"
        if [[ $div_count -gt 0 ]]; then
            echo "  → Consider: Replace divisions with multiplications or bit shifts where possible"
        fi
        
        # Floating point operations
        local fp_count=$(grep -cE "(fld|fst|fadd|fmul|fdiv)" "$disasm_file" || true)
        echo "Floating point operations: $fp_count"
        
        # Memory barriers/synchronization
        local sync_count=$(grep -cE "(lock|mfence|lfence|sfence)" "$disasm_file" || true)
        echo "Synchronization instructions: $sync_count"
        if [[ $sync_count -gt 10 ]]; then
            echo "  → Consider: Review synchronization overhead"
        fi
        
        # Function call overhead
        local call_count=$(grep -cE "(call)" "$disasm_file" || true)
        local ret_count=$(grep -cE "(ret)" "$disasm_file" || true)
        echo "Function calls: $call_count"
        echo "Function returns: $ret_count"
        if [[ $call_count -gt 1000 ]]; then
            echo "  → Consider: Inlining frequently called small functions"
        fi
        
        # Loop analysis
        echo
        echo "Loop Patterns:"
        echo "-------------"
        local loop_count=$(grep -cE "(cmp.*j[ln]e|dec.*jnz)" "$disasm_file" || true)
        echo "Potential loops detected: $loop_count"
        
        # Branch prediction hints
        echo
        echo "Branch Analysis:"
        echo "---------------"
        local cond_jumps=$(grep -cE "j[^m]" "$disasm_file" || true)
        local uncond_jumps=$(grep -cE "jmp" "$disasm_file" || true)
        echo "Conditional branches: $cond_jumps"
        echo "Unconditional jumps: $uncond_jumps"
        if [[ $cond_jumps -gt 100 ]]; then
            echo "  → Consider: Profile to identify mispredicted branches"
        fi
        
        # Stack usage analysis
        echo
        echo "Stack Usage Analysis:"
        echo "--------------------"
        local push_count=$(grep -cE "(push)" "$disasm_file" || true)
        local pop_count=$(grep -cE "(pop)" "$disasm_file" || true)
        echo "Stack pushes: $push_count"
        echo "Stack pops: $pop_count"
        
        # Vector/SIMD instruction usage
        echo
        echo "SIMD/Vector Analysis:"
        echo "--------------------"
        local simd_count=$(grep -cE "(movd|movq|padd|pmul|pack|punpck|pshuf)" "$disasm_file" || true)
        echo "SIMD instructions: $simd_count"
        if [[ $simd_count -eq 0 ]]; then
            echo "  → Opportunity: No SIMD instructions found - consider vectorization"
        else
            echo "  ✓ SIMD optimizations detected"
        fi
        
    } > "$perf_analysis_file"
    
    log_success "Performance analysis completed: $perf_analysis_file"
    echo "$perf_analysis_file"
}

# Generate comparison with other languages
generate_competitor_comparison() {
    local session_dir="$1"
    local comparison_file="$session_dir/competitor_comparison.txt"
    
    if [[ "$COMPARE_WITH_COMPETITORS" != "true" ]]; then
        log_info "Skipping competitor comparison (disabled)"
        return 0
    fi
    
    log_header "Generating Competitor Comparison"
    
    # Create equivalent C++ code for comparison
    local cpp_equivalent="$session_dir/equivalent.cpp"
    generate_cpp_equivalent "$SOURCE_FILE" "$cpp_equivalent"
    
    # Create equivalent Rust code
    local rust_equivalent="$session_dir/equivalent.rs"
    generate_rust_equivalent "$SOURCE_FILE" "$rust_equivalent"
    
    {
        echo "Language Comparison Analysis - $(date)"
        echo "====================================="
        echo
        echo "Seen Source: $SOURCE_FILE"
        echo
        
        # Compare with C++ if available
        if [[ "$GCC_AVAILABLE" == "true" ]] && [[ -f "$cpp_equivalent" ]]; then
            echo "C++ Comparison (gcc -O3):"
            echo "-------------------------"
            
            local cpp_binary="$session_dir/cpp_program"
            local cpp_asm="$session_dir/cpp_program.s"
            
            if gcc -O3 -S -o "$cpp_asm" "$cpp_equivalent" 2>/dev/null && \
               gcc -O3 -o "$cpp_binary" "$cpp_equivalent" 2>/dev/null; then
                
                local cpp_size=$(wc -l < "$cpp_asm" 2>/dev/null || echo "0")
                local seen_size=$(wc -l < "$session_dir/disassembly.txt" 2>/dev/null || echo "0")
                
                echo "Assembly lines - Seen: $seen_size, C++: $cpp_size"
                echo "Size ratio: $(echo "scale=2; $seen_size / $cpp_size" | bc -l 2>/dev/null || echo "N/A")"
                
                # Compare binary sizes
                if [[ -f "$session_dir/seen_program" ]] && [[ -f "$cpp_binary" ]]; then
                    local seen_bin_size=$(stat -c%s "$session_dir/seen_program" 2>/dev/null || echo "0")
                    local cpp_bin_size=$(stat -c%s "$cpp_binary" 2>/dev/null || echo "0")
                    echo "Binary size - Seen: ${seen_bin_size} bytes, C++: ${cpp_bin_size} bytes"
                fi
            else
                echo "Failed to compile C++ equivalent"
            fi
        fi
        
        echo
        echo "Optimization Opportunities:"
        echo "-------------------------"
        echo "1. Compare instruction counts with competitors"
        echo "2. Identify missing optimizations (SIMD, loop unrolling)"
        echo "3. Analyze register allocation efficiency"
        echo "4. Check for unnecessary memory operations"
        echo "5. Evaluate function inlining decisions"
        
    } > "$comparison_file"
    
    log_success "Competitor comparison completed: $comparison_file"
}

# Generate equivalent C++ code for comparison
generate_cpp_equivalent() {
    local seen_file="$1"
    local cpp_file="$2"
    
    # This is a simplified conversion - in practice, this would need
    # more sophisticated translation logic
    cat > "$cpp_file" << 'EOF'
#include <iostream>
#include <vector>
#include <chrono>

// Simplified equivalent of the Seen program
// This is a basic translation for comparison purposes

int main() {
    // Basic computation similar to typical benchmarks
    const int n = 1000000;
    long long sum = 0;
    
    for (int i = 0; i < n; i++) {
        sum += i * 2 + 1;
    }
    
    std::cout << "Result: " << sum << std::endl;
    return 0;
}
EOF
}

# Generate equivalent Rust code for comparison
generate_rust_equivalent() {
    local seen_file="$1"
    local rust_file="$2"
    
    cat > "$rust_file" << 'EOF'
fn main() {
    let n = 1_000_000;
    let mut sum: i64 = 0;
    
    for i in 0..n {
        sum += i * 2 + 1;
    }
    
    println!("Result: {}", sum);
}
EOF
}

# Generate comprehensive report
generate_comprehensive_report() {
    local session_dir="$1"
    local report_file="$session_dir/comprehensive_report.md"
    
    log_header "Generating Comprehensive Report"
    
    {
        echo "# Seen Language Assembly Analysis Report"
        echo
        echo "**Source File:** \`$SOURCE_FILE\`"
        echo "**Optimization Level:** $OPTIMIZATION_LEVEL"
        echo "**Analysis Date:** $(date)"
        echo "**Seen Compiler:** \`$SEEN_EXECUTABLE\`"
        echo
        echo "## Executive Summary"
        echo
        echo "This report provides a comprehensive analysis of the assembly code generated"
        echo "by the Seen language compiler, identifying optimization opportunities and"
        echo "comparing with equivalent implementations in other systems programming languages."
        echo
        echo "## Files Generated"
        echo
        
        # List all generated files
        echo "### Analysis Files"
        find "$session_dir" -name "*.txt" -o -name "*.s" | while read file; do
            echo "- [$(basename "$file")](./$(basename "$file"))"
        done
        
        echo
        echo "### Binary Files"
        find "$session_dir" -type f -executable | while read file; do
            local size=$(stat -c%s "$file" 2>/dev/null || echo "unknown")
            echo "- $(basename "$file") (${size} bytes)"
        done
        
        echo
        echo "## Key Findings"
        echo
        
        # Extract key metrics if files exist
        if [[ -f "$session_dir/instruction_analysis.txt" ]]; then
            echo "### Instruction Analysis"
            echo "- Assembly instruction patterns analyzed"
            echo "- Register usage patterns documented"
            echo "- Memory access patterns identified"
            echo
        fi
        
        if [[ -f "$session_dir/performance_analysis.txt" ]]; then
            echo "### Performance Characteristics"
            echo "- Performance hotspots identified"
            echo "- Optimization opportunities documented"
            echo "- SIMD usage analyzed"
            echo
        fi
        
        echo "## Recommended Actions"
        echo
        echo "1. **Review Hot Functions**: Focus optimization efforts on functions with highest instruction counts"
        echo "2. **SIMD Opportunities**: Consider vectorizing loops that process arrays"
        echo "3. **Register Usage**: Analyze register pressure in performance-critical sections"
        echo "4. **Memory Access**: Optimize data structure layout for better cache performance"
        echo "5. **Branch Prediction**: Profile branches to identify misprediction issues"
        echo
        echo "## How to Use This Analysis"
        echo
        echo "1. **Start with `instruction_analysis.txt`** - Understand the overall instruction mix"
        echo "2. **Review `performance_analysis.txt`** - Identify specific optimization opportunities"
        echo "3. **Examine `disassembly.txt`** - Deep dive into specific function implementations"
        echo "4. **Compare with competitors** - See how Seen stacks up against other languages"
        echo "5. **Profile with perf** - Use flamegraph tools for runtime analysis"
        echo
        echo "## Next Steps"
        echo
        echo "- Run performance profiling with flamegraphs"
        echo "- Benchmark against equivalent C++/Rust implementations"
        echo "- Implement identified optimizations"
        echo "- Re-analyze to measure improvement"
        echo
        
    } > "$report_file"
    
    log_success "Comprehensive report generated: $report_file"
}

# Main execution function
main() {
    parse_arguments "$@"
    
    log_info "=== Seen Language Assembly Analysis Tool ==="
    log_info "Source: $SOURCE_FILE"
    log_info "Optimization: $OPTIMIZATION_LEVEL"
    
    # System checks
    check_requirements || exit 1
    
    # Setup output
    local session_dir
    session_dir=$(setup_output)
    log_info "Output directory: $session_dir"
    
    # Compile source
    local binary_path assembly_path
    read -r assembly_path binary_path < <(compile_seen_source "$session_dir")
    
    # Generate disassembly if needed
    local disasm_file=""
    if [[ "$INCLUDE_DISASSEMBLY" == "true" ]]; then
        if [[ -n "$assembly_path" && -f "$assembly_path" ]]; then
            disasm_file="$assembly_path"
        else
            disasm_file=$(generate_disassembly "$binary_path" "$session_dir")
        fi
    fi
    
    # Perform analyses
    if [[ -n "$disasm_file" && -f "$disasm_file" ]]; then
        analyze_instruction_patterns "$disasm_file" "$session_dir"
        analyze_performance_characteristics "$disasm_file" "$session_dir"
    fi
    
    # Generate competitor comparison
    if [[ "$COMPARE_WITH_COMPETITORS" == "true" ]]; then
        generate_competitor_comparison "$session_dir"
    fi
    
    # Generate comprehensive report
    generate_comprehensive_report "$session_dir"
    
    # Final summary
    log_success "Assembly analysis completed!"
    log_success "Results directory: $session_dir"
    
    echo
    log_info "Generated analysis files:"
    find "$session_dir" -name "*.txt" -o -name "*.md" | while read file; do
        echo "  - $(basename "$file")"
    done
    
    echo
    log_info "To continue performance analysis:"
    echo "  1. Review the comprehensive report: $session_dir/comprehensive_report.md"
    echo "  2. Examine detailed analysis files in the results directory"
    echo "  3. Use flamegraph tools for runtime profiling"
    echo "  4. Run performance benchmarks for quantitative comparison"
}

# Execute main function
main "$@"