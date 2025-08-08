#!/usr/bin/env bash
# Master test runner for Seen Language installer system
# Runs all test suites in sequence

set -e

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VERBOSE=false
QUICK_MODE=false
CLEANUP=true
EXIT_ON_FAILURE=true

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

# Test results
SUITES_RUN=0
SUITES_PASSED=0
SUITES_FAILED=0
FAILED_SUITES=()

# Logging functions
info() {
    echo -e "${BLUE}ℹ $1${NC}"
}

success() {
    echo -e "${GREEN}✓ $1${NC}"
}

error() {
    echo -e "${RED}✗ $1${NC}" >&2
}

warning() {
    echo -e "${YELLOW}⚠ $1${NC}"
}

header() {
    echo ""
    echo -e "${CYAN}===============================================${NC}"
    echo -e "${CYAN}  $1${NC}"
    echo -e "${CYAN}===============================================${NC}"
    echo ""
}

show_help() {
    cat << EOF
Seen Language Installer - Master Test Runner

Usage: $0 [options]

Options:
  --verbose           Enable verbose output
  --quick             Quick test mode (skip integration tests)
  --no-cleanup        Don't clean up test files
  --continue-on-fail  Continue running tests even if one suite fails
  --help              Show this help message

Test Suites:
  1. Unit Tests      - Basic functionality and syntax validation
  2. Integration     - End-to-end installation workflow
  3. Performance     - Installation speed and resource usage (optional)

Examples:
  $0                      # Run all tests
  $0 --quick              # Run only unit tests
  $0 --verbose            # Run with detailed output
  $0 --continue-on-fail   # Don't exit on first failure

EOF
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --verbose)
            VERBOSE=true
            shift
            ;;
        --quick)
            QUICK_MODE=true
            shift
            ;;
        --no-cleanup)
            CLEANUP=false
            shift
            ;;
        --continue-on-fail)
            EXIT_ON_FAILURE=false
            shift
            ;;
        --help)
            show_help
            exit 0
            ;;
        *)
            error "Unknown option: $1. Use --help for usage information."
            exit 1
            ;;
    esac
done

# Run a test suite
run_test_suite() {
    local suite_name="$1"
    local script_path="$2"
    shift 2
    local args=("$@")
    
    SUITES_RUN=$((SUITES_RUN + 1))
    
    header "Running $suite_name"
    
    if [ ! -f "$script_path" ]; then
        error "Test script not found: $script_path"
        SUITES_FAILED=$((SUITES_FAILED + 1))
        FAILED_SUITES+=("$suite_name (script not found)")
        return 1
    fi
    
    if [ ! -x "$script_path" ]; then
        error "Test script not executable: $script_path"
        SUITES_FAILED=$((SUITES_FAILED + 1))
        FAILED_SUITES+=("$suite_name (not executable)")
        return 1
    fi
    
    local start_time=$(date +%s)
    
    if "$script_path" "${args[@]}"; then
        local end_time=$(date +%s)
        local duration=$((end_time - start_time))
        success "$suite_name completed successfully in ${duration}s"
        SUITES_PASSED=$((SUITES_PASSED + 1))
        return 0
    else
        local end_time=$(date +%s)
        local duration=$((end_time - start_time))
        error "$suite_name failed after ${duration}s"
        SUITES_FAILED=$((SUITES_FAILED + 1))
        FAILED_SUITES+=("$suite_name")
        return 1
    fi
}

# Build test arguments
build_args() {
    local args=()
    
    if $VERBOSE; then
        args+=("--verbose")
    fi
    
    if $QUICK_MODE; then
        args+=("--quick")
    fi
    
    if ! $CLEANUP; then
        args+=("--no-cleanup")
    fi
    
    echo "${args[@]}"
}

# Check test environment
check_environment() {
    info "Checking test environment..."
    
    # Check if we're in the right directory
    if [ ! -d "$SCRIPT_DIR/../scripts" ] || [ ! -d "$SCRIPT_DIR/../windows" ]; then
        error "Test must be run from installer directory"
        exit 1
    fi
    
    # Check for required tools
    local required_tools=("bash")
    local missing_tools=()
    
    for tool in "${required_tools[@]}"; do
        if ! command -v "$tool" &> /dev/null; then
            missing_tools+=("$tool")
        fi
    done
    
    if [ ${#missing_tools[@]} -gt 0 ]; then
        error "Missing required tools: ${missing_tools[*]}"
        exit 1
    fi
    
    # Check optional tools
    local optional_tools=("jq" "yamllint" "xmllint" "ruby" "powershell")
    local missing_optional=()
    
    for tool in "${optional_tools[@]}"; do
        if ! command -v "$tool" &> /dev/null; then
            missing_optional+=("$tool")
        fi
    done
    
    if [ ${#missing_optional[@]} -gt 0 ]; then
        warning "Optional tools missing (some tests may be skipped): ${missing_optional[*]}"
    fi
    
    success "Environment check passed"
}

# Show system information
show_system_info() {
    header "System Information"
    
    info "Operating System: $(uname -s) $(uname -r)"
    info "Architecture: $(uname -m)"
    info "Shell: $SHELL"
    info "User: $(whoami)"
    info "Working Directory: $(pwd)"
    info "Test Directory: $SCRIPT_DIR"
    
    # Show available disk space
    local disk_info=$(df -h "$SCRIPT_DIR" 2>/dev/null | tail -1 | awk '{print $4}')
    if [ -n "$disk_info" ]; then
        info "Available Disk Space: $disk_info"
    fi
    
    # Show memory info if available
    if [ -f /proc/meminfo ]; then
        local mem_info=$(grep MemAvailable /proc/meminfo 2>/dev/null | awk '{printf "%.1f GB", $2/1024/1024}')
        if [ -n "$mem_info" ]; then
            info "Available Memory: $mem_info"
        fi
    fi
}

# Run all test suites
run_all_tests() {
    local test_args=($(build_args))
    local suite_failed=false
    
    # 1. Unit Tests (test-runner.sh)
    if ! run_test_suite "Unit Tests" "$SCRIPT_DIR/test-runner.sh" "${test_args[@]}"; then
        suite_failed=true
        if $EXIT_ON_FAILURE; then
            return 1
        fi
    fi
    
    # 2. Integration Tests (integration-test.sh) - skip in quick mode
    if ! $QUICK_MODE; then
        if ! run_test_suite "Integration Tests" "$SCRIPT_DIR/integration-test.sh" "${test_args[@]}"; then
            suite_failed=true
            if $EXIT_ON_FAILURE; then
                return 1
            fi
        fi
    else
        info "Skipping integration tests (quick mode)"
    fi
    
    # 3. Performance Tests (optional, if present)
    local perf_test="$SCRIPT_DIR/performance-test.sh"
    if [ -f "$perf_test" ] && ! $QUICK_MODE; then
        if ! run_test_suite "Performance Tests" "$perf_test" "${test_args[@]}"; then
            warning "Performance tests failed (non-critical)"
        fi
    fi
    
    if $suite_failed; then
        return 1
    else
        return 0
    fi
}

# Show final results
show_results() {
    header "Test Results Summary"
    
    info "Test suites run: $SUITES_RUN"
    success "Test suites passed: $SUITES_PASSED"
    
    if [ $SUITES_FAILED -gt 0 ]; then
        error "Test suites failed: $SUITES_FAILED"
        echo ""
        info "Failed suites:"
        for suite in "${FAILED_SUITES[@]}"; do
            echo -e "  ${RED}✗${NC} $suite"
        done
        echo ""
        error "❌ Overall test result: FAILED"
        return 1
    else
        success "✅ Overall test result: PASSED"
        echo ""
        success "🎉 All test suites completed successfully!"
        echo ""
        success "The Seen Language installer system is ready for production use."
        return 0
    fi
}

# Generate test report
generate_report() {
    local report_file="$SCRIPT_DIR/test-report-$(date +%Y%m%d-%H%M%S).txt"
    
    {
        echo "Seen Language Installer Test Report"
        echo "Generated: $(date)"
        echo "System: $(uname -s) $(uname -r) $(uname -m)"
        echo ""
        echo "Test Configuration:"
        echo "  Verbose: $VERBOSE"
        echo "  Quick Mode: $QUICK_MODE"
        echo "  Cleanup: $CLEANUP"
        echo "  Exit on Failure: $EXIT_ON_FAILURE"
        echo ""
        echo "Results:"
        echo "  Suites Run: $SUITES_RUN"
        echo "  Suites Passed: $SUITES_PASSED"
        echo "  Suites Failed: $SUITES_FAILED"
        echo ""
        if [ ${#FAILED_SUITES[@]} -gt 0 ]; then
            echo "Failed Suites:"
            for suite in "${FAILED_SUITES[@]}"; do
                echo "  - $suite"
            done
            echo ""
        fi
        echo "Overall Result: $([ $SUITES_FAILED -eq 0 ] && echo "PASSED" || echo "FAILED")"
    } > "$report_file"
    
    info "Test report saved: $report_file"
}

# Cleanup on exit
cleanup() {
    if $CLEANUP; then
        # Clean up any temporary files
        find /tmp -name "seen-*-test*" -type d -mtime -1 -exec rm -rf {} + 2>/dev/null || true
    fi
}

# Handle script interruption
trap cleanup EXIT INT TERM

# Main execution
main() {
    header "Seen Language Installer - Master Test Runner"
    
    # Show system information
    if $VERBOSE; then
        show_system_info
    fi
    
    # Check environment
    check_environment
    
    # Run all test suites
    local start_time=$(date +%s)
    
    if run_all_tests; then
        local end_time=$(date +%s)
        local total_duration=$((end_time - start_time))
        success "All tests completed in ${total_duration}s"
        
        if show_results; then
            generate_report
            exit 0
        else
            generate_report
            exit 1
        fi
    else
        local end_time=$(date +%s)
        local total_duration=$((end_time - start_time))
        error "Tests failed after ${total_duration}s"
        
        show_results
        generate_report
        exit 1
    fi
}

# Run main function
main "$@"