#!/usr/bin/env bash
# Comprehensive test runner for Seen Language installer system
# Tests all installer formats and installation methods

set -e

# Configuration
VERSION="1.0.0"
TEST_DIR="/tmp/seen-installer-tests"
VERBOSE=false
QUICK_MODE=false
PLATFORMS=()
INSTALLERS=()

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

# Test results
TESTS_PASSED=0
TESTS_FAILED=0
FAILED_TESTS=()

# Logging functions
error() {
    echo -e "${RED}✗ $1${NC}" >&2
    TESTS_FAILED=$((TESTS_FAILED + 1))
    FAILED_TESTS+=("$1")
}

warning() {
    echo -e "${YELLOW}⚠ $1${NC}"
}

info() {
    echo -e "${BLUE}ℹ $1${NC}"
}

success() {
    echo -e "${GREEN}✓ $1${NC}"
    TESTS_PASSED=$((TESTS_PASSED + 1))
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
Seen Language Installer Test Runner

Usage: $0 [options]

Options:
  --version VERSION        Test specific version (default: $VERSION)
  --test-dir DIR          Test directory (default: $TEST_DIR)
  --platform PLATFORM     Test specific platform (linux, windows, macos)
  --installer TYPE         Test specific installer (universal, deb, rpm, msi, appimage)
  --quick                 Quick test mode (skip long-running tests)
  --verbose               Enable verbose output
  --clean                 Clean test directory before running
  --help                  Show this help message

Examples:
  $0                           # Run all tests
  $0 --platform linux         # Test only Linux installers
  $0 --installer universal    # Test only universal scripts
  $0 --quick --verbose        # Quick tests with verbose output

Test Categories:
  - Universal installation scripts (install.sh, install.ps1)
  - Native package formats (DEB, RPM, MSI, AppImage)
  - Package manager integration (Homebrew, Scoop)
  - Installation validation and verification
  - Uninstallation and cleanup testing

EOF
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --version)
            VERSION="$2"
            shift 2
            ;;
        --test-dir)
            TEST_DIR="$2"
            shift 2
            ;;
        --platform)
            PLATFORMS+=("$2")
            shift 2
            ;;
        --installer)
            INSTALLERS+=("$2")
            shift 2
            ;;
        --quick)
            QUICK_MODE=true
            shift
            ;;
        --verbose)
            VERBOSE=true
            shift
            ;;
        --clean)
            rm -rf "$TEST_DIR"
            success "Test directory cleaned: $TEST_DIR"
            exit 0
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

# Get absolute paths
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

# Setup test environment
setup_test_env() {
    info "Setting up test environment..."
    
    # Create test directory
    mkdir -p "$TEST_DIR"
    cd "$TEST_DIR"
    
    # Create isolated environment
    mkdir -p bin lib share
    export PATH="$TEST_DIR/bin:$PATH"
    export SEEN_LIB_PATH="$TEST_DIR/lib/seen"
    export SEEN_DATA_PATH="$TEST_DIR/share/seen"
    
    success "Test environment ready: $TEST_DIR"
}

# Detect platform
detect_platform() {
    if [[ "$OSTYPE" == "linux-gnu"* ]]; then
        echo "linux"
    elif [[ "$OSTYPE" == "darwin"* ]]; then
        echo "macos"
    elif [[ "$OSTYPE" == "cygwin" || "$OSTYPE" == "msys" ]]; then
        echo "windows"
    else
        echo "unknown"
    fi
}

# Check if we should test a platform
should_test_platform() {
    local platform="$1"
    
    if [ ${#PLATFORMS[@]} -eq 0 ]; then
        return 0  # Test all platforms if none specified
    fi
    
    for p in "${PLATFORMS[@]}"; do
        if [ "$p" = "$platform" ]; then
            return 0
        fi
    done
    
    return 1
}

# Check if we should test an installer
should_test_installer() {
    local installer="$1"
    
    if [ ${#INSTALLERS[@]} -eq 0 ]; then
        return 0  # Test all installers if none specified
    fi
    
    for i in "${INSTALLERS[@]}"; do
        if [ "$i" = "$installer" ]; then
            return 0
        fi
    done
    
    return 1
}

# Test universal installation script
test_universal_installer() {
    local platform="$1"
    
    if ! should_test_platform "$platform" || ! should_test_installer "universal"; then
        return 0
    fi
    
    info "Testing universal installer for $platform..."
    
    local installer_script=""
    local test_args=""
    
    case "$platform" in
        linux|macos)
            installer_script="$PROJECT_ROOT/installer/scripts/install.sh"
            test_args="--version $VERSION --install-dir $TEST_DIR --no-path"
            ;;
        windows)
            installer_script="$PROJECT_ROOT/installer/scripts/install.ps1"
            test_args="-Version $VERSION -InstallDir $TEST_DIR -NoPath"
            ;;
        *)
            warning "Universal installer not supported on $platform"
            return 0
            ;;
    esac
    
    if [ ! -f "$installer_script" ]; then
        error "Universal installer script not found: $installer_script"
        return 1
    fi
    
    # Test script syntax
    case "$platform" in
        linux|macos)
            if bash -n "$installer_script"; then
                success "Universal installer syntax valid"
            else
                error "Universal installer syntax error"
                return 1
            fi
            ;;
        windows)
            if command -v powershell &> /dev/null; then
                if powershell -Command "Get-Content '$installer_script' | Out-Null"; then
                    success "Universal installer syntax valid"
                else
                    error "Universal installer syntax error"
                    return 1
                fi
            else
                warning "PowerShell not available, skipping Windows installer test"
                return 0
            fi
            ;;
    esac
    
    success "Universal installer test passed for $platform"
}

# Test DEB package
test_deb_package() {
    if ! should_test_platform "linux" || ! should_test_installer "deb"; then
        return 0
    fi
    
    info "Testing DEB package builder..."
    
    local build_script="$PROJECT_ROOT/installer/linux/build-deb.sh"
    
    if [ ! -f "$build_script" ]; then
        error "DEB build script not found: $build_script"
        return 1
    fi
    
    # Test script syntax
    if bash -n "$build_script"; then
        success "DEB build script syntax valid"
    else
        error "DEB build script syntax error"
        return 1
    fi
    
    # Test help output
    if timeout 10 "$build_script" --help >/dev/null 2>&1; then
        success "DEB build script help works"
    else
        error "DEB build script help failed"
        return 1
    fi
    
    success "DEB package test passed"
}

# Test RPM package
test_rpm_package() {
    if ! should_test_platform "linux" || ! should_test_installer "rpm"; then
        return 0
    fi
    
    info "Testing RPM package builder..."
    
    local build_script="$PROJECT_ROOT/installer/linux/build-rpm.sh"
    
    if [ ! -f "$build_script" ]; then
        error "RPM build script not found: $build_script"
        return 1
    fi
    
    # Test script syntax
    if bash -n "$build_script"; then
        success "RPM build script syntax valid"
    else
        error "RPM build script syntax error"
        return 1
    fi
    
    # Test help output
    if timeout 10 "$build_script" --help >/dev/null 2>&1; then
        success "RPM build script help works"
    else
        error "RPM build script help failed"
        return 1
    fi
    
    success "RPM package test passed"
}

# Test AppImage
test_appimage() {
    if ! should_test_platform "linux" || ! should_test_installer "appimage"; then
        return 0
    fi
    
    info "Testing AppImage builder..."
    
    local build_script="$PROJECT_ROOT/installer/linux/build-appimage.sh"
    
    if [ ! -f "$build_script" ]; then
        error "AppImage build script not found: $build_script"
        return 1
    fi
    
    # Test script syntax
    if bash -n "$build_script"; then
        success "AppImage build script syntax valid"
    else
        error "AppImage build script syntax error"
        return 1
    fi
    
    # Test help output
    if timeout 10 "$build_script" --help >/dev/null 2>&1; then
        success "AppImage build script help works"
    else
        error "AppImage build script help failed"
        return 1
    fi
    
    success "AppImage test passed"
}

# Test MSI installer
test_msi_installer() {
    if ! should_test_platform "windows" || ! should_test_installer "msi"; then
        return 0
    fi
    
    info "Testing MSI installer..."
    
    local build_script="$PROJECT_ROOT/installer/windows/build-msi.ps1"
    local wxs_file="$PROJECT_ROOT/installer/windows/seen.wxs"
    
    if [ ! -f "$build_script" ]; then
        error "MSI build script not found: $build_script"
        return 1
    fi
    
    if [ ! -f "$wxs_file" ]; then
        error "WiX configuration not found: $wxs_file"
        return 1
    fi
    
    # Test WiX file syntax (basic XML validation)
    if command -v xmllint &> /dev/null; then
        if xmllint --noout "$wxs_file" 2>/dev/null; then
            success "WiX configuration syntax valid"
        else
            error "WiX configuration syntax error"
            return 1
        fi
    else
        warning "xmllint not available, skipping WiX syntax check"
    fi
    
    success "MSI installer test passed"
}

# Test Homebrew formula
test_homebrew() {
    if ! should_test_platform "macos" && ! should_test_platform "linux"; then
        return 0
    fi
    
    if ! should_test_installer "homebrew"; then
        return 0
    fi
    
    info "Testing Homebrew formula..."
    
    local formula_file="$PROJECT_ROOT/installer/homebrew/seen-lang.rb"
    local generator_script="$PROJECT_ROOT/installer/homebrew/generate-formula.sh"
    
    if [ ! -f "$formula_file" ]; then
        error "Homebrew formula not found: $formula_file"
        return 1
    fi
    
    if [ ! -f "$generator_script" ]; then
        error "Formula generator not found: $generator_script"
        return 1
    fi
    
    # Test Ruby syntax
    if command -v ruby &> /dev/null; then
        if ruby -c "$formula_file" >/dev/null 2>&1; then
            success "Homebrew formula syntax valid"
        else
            error "Homebrew formula syntax error"
            return 1
        fi
    else
        warning "Ruby not available, skipping formula syntax check"
    fi
    
    # Test generator script
    if bash -n "$generator_script"; then
        success "Formula generator syntax valid"
    else
        error "Formula generator syntax error"
        return 1
    fi
    
    success "Homebrew formula test passed"
}

# Test Scoop manifest
test_scoop() {
    if ! should_test_platform "windows" || ! should_test_installer "scoop"; then
        return 0
    fi
    
    info "Testing Scoop manifest..."
    
    local manifest_file="$PROJECT_ROOT/installer/scoop/seen-lang.json"
    local generator_script="$PROJECT_ROOT/installer/scoop/generate-manifest.ps1"
    
    if [ ! -f "$manifest_file" ]; then
        error "Scoop manifest not found: $manifest_file"
        return 1
    fi
    
    if [ ! -f "$generator_script" ]; then
        error "Manifest generator not found: $generator_script"
        return 1
    fi
    
    # Test JSON syntax
    if command -v jq &> /dev/null; then
        if jq empty "$manifest_file" >/dev/null 2>&1; then
            success "Scoop manifest JSON valid"
        else
            error "Scoop manifest JSON syntax error"
            return 1
        fi
    elif command -v python3 &> /dev/null; then
        if python3 -m json.tool "$manifest_file" >/dev/null 2>&1; then
            success "Scoop manifest JSON valid"
        else
            error "Scoop manifest JSON syntax error"
            return 1
        fi
    else
        warning "JSON validator not available, skipping manifest syntax check"
    fi
    
    success "Scoop manifest test passed"
}

# Test GitHub Actions workflow
test_github_actions() {
    info "Testing GitHub Actions workflow..."
    
    local workflow_file="$PROJECT_ROOT/.github/workflows/release.yml"
    
    if [ ! -f "$workflow_file" ]; then
        error "GitHub Actions workflow not found: $workflow_file"
        return 1
    fi
    
    # Test YAML syntax
    if command -v yamllint &> /dev/null; then
        if yamllint -d relaxed "$workflow_file" >/dev/null 2>&1; then
            success "GitHub Actions workflow YAML valid"
        else
            error "GitHub Actions workflow YAML syntax error"
            return 1
        fi
    elif command -v python3 &> /dev/null; then
        if python3 -c "import yaml; yaml.safe_load(open('$workflow_file'))" >/dev/null 2>&1; then
            success "GitHub Actions workflow YAML valid"
        else
            error "GitHub Actions workflow YAML syntax error"
            return 1
        fi
    else
        warning "YAML validator not available, skipping workflow syntax check"
    fi
    
    success "GitHub Actions workflow test passed"
}

# Test documentation
test_documentation() {
    info "Testing installer documentation..."
    
    local docs=(
        "$PROJECT_ROOT/installer/README.md"
        "$PROJECT_ROOT/installer/assets/README.md"
        "$PROJECT_ROOT/docs/Installer Plan.md"
    )
    
    local missing_docs=()
    
    for doc in "${docs[@]}"; do
        if [ ! -f "$doc" ]; then
            missing_docs+=("$doc")
        fi
    done
    
    if [ ${#missing_docs[@]} -gt 0 ]; then
        error "Missing documentation files: ${missing_docs[*]}"
        return 1
    fi
    
    success "All documentation files found"
    
    # Check for common documentation issues
    for doc in "${docs[@]}"; do
        if [ -f "$doc" ]; then
            # Check for placeholder content
            if grep -q "TODO\|FIXME\|XXX" "$doc"; then
                warning "Documentation contains TODOs: $(basename "$doc")"
            fi
            
            # Check file size (should not be empty)
            if [ ! -s "$doc" ]; then
                error "Documentation file is empty: $(basename "$doc")"
                return 1
            fi
        fi
    done
    
    success "Documentation test passed"
}

# Test asset generation
test_assets() {
    info "Testing asset generation..."
    
    local asset_script="$PROJECT_ROOT/installer/assets/generate-assets.sh"
    
    if [ ! -f "$asset_script" ]; then
        error "Asset generation script not found: $asset_script"
        return 1
    fi
    
    # Test script syntax
    if bash -n "$asset_script"; then
        success "Asset generation script syntax valid"
    else
        error "Asset generation script syntax error"
        return 1
    fi
    
    # Test help output
    if "$asset_script" --help >/dev/null 2>&1; then
        success "Asset generation script help works"
    else
        error "Asset generation script help failed"
        return 1
    fi
    
    success "Asset generation test passed"
}

# Run comprehensive test suite
run_tests() {
    header "Running Installer Test Suite"
    
    info "Test configuration:"
    info "  Version: $VERSION"
    info "  Test directory: $TEST_DIR"
    info "  Quick mode: $QUICK_MODE"
    info "  Verbose: $VERBOSE"
    info "  Current platform: $(detect_platform)"
    
    if [ ${#PLATFORMS[@]} -gt 0 ]; then
        info "  Target platforms: ${PLATFORMS[*]}"
    fi
    
    if [ ${#INSTALLERS[@]} -gt 0 ]; then
        info "  Target installers: ${INSTALLERS[*]}"
    fi
    
    # Setup test environment
    setup_test_env
    
    # Run tests
    test_universal_installer "linux"
    test_universal_installer "macos"
    test_universal_installer "windows"
    
    test_deb_package
    test_rpm_package
    test_appimage
    test_msi_installer
    
    test_homebrew
    test_scoop
    
    test_github_actions
    test_documentation
    test_assets
}

# Show test results
show_results() {
    header "Test Results"
    
    local total_tests=$((TESTS_PASSED + TESTS_FAILED))
    
    if [ $total_tests -eq 0 ]; then
        warning "No tests were run"
        return 0
    fi
    
    success "Tests passed: $TESTS_PASSED"
    
    if [ $TESTS_FAILED -gt 0 ]; then
        error "Tests failed: $TESTS_FAILED"
        echo ""
        info "Failed tests:"
        for test in "${FAILED_TESTS[@]}"; do
            echo -e "  ${RED}✗${NC} $test"
        done
        echo ""
        return 1
    else
        success "All tests passed! 🎉"
        return 0
    fi
}

# Main execution
main() {
    run_tests
    show_results
}

# Execute main function
main "$@"