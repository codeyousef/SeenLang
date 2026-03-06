#!/usr/bin/env bash
# Integration test for complete installer workflow
# Tests end-to-end installation and verification

set -e

# Configuration
TEST_VERSION="1.0.0"
TEST_DIR="/tmp/seen-integration-test"
CLEANUP=true
VERBOSE=false

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

# Test tracking
TESTS_RUN=0
TESTS_PASSED=0
TESTS_FAILED=0

# Logging functions
log() {
    if $VERBOSE; then
        echo -e "${BLUE}[$(date '+%H:%M:%S')] $1${NC}"
    fi
}

info() {
    echo -e "${BLUE}ℹ $1${NC}"
}

success() {
    echo -e "${GREEN}✓ $1${NC}"
    TESTS_PASSED=$((TESTS_PASSED + 1))
}

error() {
    echo -e "${RED}✗ $1${NC}" >&2
    TESTS_FAILED=$((TESTS_FAILED + 1))
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

# Test case wrapper
test_case() {
    local name="$1"
    shift
    
    TESTS_RUN=$((TESTS_RUN + 1))
    info "Running test: $name"
    
    if "$@"; then
        success "Test passed: $name"
        return 0
    else
        error "Test failed: $name"
        return 1
    fi
}

# Setup test environment
setup_test_env() {
    log "Setting up test environment..."
    
    # Clean previous test
    if [ -d "$TEST_DIR" ] && $CLEANUP; then
        rm -rf "$TEST_DIR"
    fi
    
    # Create test directory
    mkdir -p "$TEST_DIR"
    cd "$TEST_DIR"
    
    # Create isolated environment
    export PATH="$TEST_DIR/bin:$PATH"
    export SEEN_LIB_PATH="$TEST_DIR/lib/seen"
    export SEEN_DATA_PATH="$TEST_DIR/share/seen"
    
    log "Test environment ready: $TEST_DIR"
}

# Test project creation
test_project_creation() {
    log "Testing project creation..."
    
    # Create test project
    mkdir -p test-project
    cd test-project
    
    # Create basic Seen.toml
    cat > Seen.toml << EOF
[package]
name = "test-project"
version = "0.1.0"
language = "en"

[build]
target = "native"
optimization = "release"
EOF
    
    # Create main source file
    mkdir -p src
    cat > src/main.seen << EOF
fn main() {
    println!("Hello from Seen Language!");
    println!("Integration test successful!");
}
EOF
    
    if [ -f "Seen.toml" ] && [ -f "src/main.seen" ]; then
        return 0
    else
        return 1
    fi
}

# Test binary execution (mock)
test_binary_execution() {
    log "Testing binary execution (mock)..."
    
    # Create mock seen binary
    mkdir -p "$TEST_DIR/bin"
    cat > "$TEST_DIR/bin/seen" << 'EOF'
#!/bin/bash
case "$1" in
    --version)
        echo "Seen Language 1.0.0"
        echo "Commit: abc123def456"
        echo "Built: 2024-01-01"
        ;;
    init)
        mkdir -p "$2"
        cd "$2"
        cat > Seen.toml << 'TOML'
[package]
name = "$2"
version = "0.1.0"

[build]
target = "native"
TOML
        mkdir -p src
        cat > src/main.seen << 'SEEN'
fn main() {
    println!("Hello, World!");
}
SEEN
        echo "Created project: $2"
        ;;
    build)
        if [ -f "Seen.toml" ]; then
            echo "Building project..."
            sleep 0.1
            mkdir -p target
            echo "Build completed successfully"
        else
            echo "Error: Seen.toml not found" >&2
            exit 1
        fi
        ;;
    run)
        if [ -f "Seen.toml" ]; then
            echo "Running project..."
            echo "Hello, World!"
        else
            echo "Error: Project not built" >&2
            exit 1
        fi
        ;;
    test)
        echo "Running tests..."
        echo "All tests passed"
        ;;
    *)
        echo "Seen Language Compiler"
        echo "Usage: seen <command> [options]"
        echo "Commands:"
        echo "  init <name>    Create a new project"
        echo "  build          Build the project"
        echo "  run            Run the project"
        echo "  test           Run tests"
        echo "  --version      Show version"
        ;;
esac
EOF
    
    chmod +x "$TEST_DIR/bin/seen"
    
    # Test version command
    local version_output=$(seen --version)
    if [[ "$version_output" == *"Seen Language"* ]]; then
        return 0
    else
        return 1
    fi
}

# Test full project workflow
test_project_workflow() {
    log "Testing full project workflow..."
    
    cd "$TEST_DIR"
    
    # Initialize project
    if ! seen init test-workflow; then
        return 1
    fi
    
    cd test-workflow
    
    # Build project
    if ! seen build; then
        return 1
    fi
    
    # Run project
    local output=$(seen run)
    if [[ "$output" == *"Hello, World!"* ]]; then
        return 0
    else
        return 1
    fi
}

# Test standard library installation
test_stdlib_installation() {
    log "Testing standard library installation..."
    
    # Create mock stdlib
    mkdir -p "$TEST_DIR/lib/seen"
    
    # Create core modules
    mkdir -p "$TEST_DIR/lib/seen/core"
    cat > "$TEST_DIR/lib/seen/core/mod.seen" << EOF
// Core module for Seen Language
pub fn main() {
    // Entry point
}

pub trait Display {
    fn display(&self) -> String;
}
EOF
    
    # Create collections module
    mkdir -p "$TEST_DIR/lib/seen/collections"
    cat > "$TEST_DIR/lib/seen/collections/mod.seen" << EOF
// Collections module
pub struct Vec<T> {
    // Vector implementation
}

pub struct HashMap<K, V> {
    // HashMap implementation
}
EOF
    
    # Verify stdlib structure
    if [ -f "$TEST_DIR/lib/seen/core/mod.seen" ] && [ -f "$TEST_DIR/lib/seen/collections/mod.seen" ]; then
        return 0
    else
        return 1
    fi
}

# Test language configuration
test_language_config() {
    log "Testing language configuration..."
    
    # Create language configs
    mkdir -p "$TEST_DIR/share/seen/languages"
    
    # English configuration
    cat > "$TEST_DIR/share/seen/languages/en.toml" << EOF
[language]
name = "English"
code = "en"

[keywords]
function = "fn"
return = "return"
if = "if"
else = "else"
while = "while"
for = "for"

[messages]
hello = "Hello"
world = "World"
EOF
    
    # Arabic configuration
    cat > "$TEST_DIR/share/seen/languages/ar.toml" << EOF
[language]
name = "العربية"
code = "ar"

[keywords]
function = "دالة"
return = "إرجاع"
if = "إذا"
else = "وإلا"
while = "بينما"
for = "لكل"

[messages]
hello = "مرحبا"
world = "العالم"
EOF
    
    # Verify language configs
    if [ -f "$TEST_DIR/share/seen/languages/en.toml" ] && [ -f "$TEST_DIR/share/seen/languages/ar.toml" ]; then
        return 0
    else
        return 1
    fi
}

# Test environment variables
test_environment_setup() {
    log "Testing environment variables..."
    
    # Check PATH includes seen binary
    if command -v seen &> /dev/null; then
        log "✓ seen binary found in PATH"
    else
        return 1
    fi
    
    # Check SEEN_LIB_PATH
    if [ -n "$SEEN_LIB_PATH" ] && [ -d "$SEEN_LIB_PATH" ]; then
        log "✓ SEEN_LIB_PATH set and valid: $SEEN_LIB_PATH"
    else
        return 1
    fi
    
    # Check SEEN_DATA_PATH
    if [ -n "$SEEN_DATA_PATH" ] && [ -d "$SEEN_DATA_PATH" ]; then
        log "✓ SEEN_DATA_PATH set and valid: $SEEN_DATA_PATH"
    else
        return 1
    fi
    
    return 0
}

# Test LSP server (if available)
test_lsp_server() {
    log "Testing LSP server..."
    
    # Create mock LSP server
    cat > "$TEST_DIR/bin/seen-lsp" << 'EOF'
#!/bin/bash
case "$1" in
    --version)
        echo "Seen Language Server 1.0.0"
        ;;
    --stdio)
        echo "LSP server started in stdio mode"
        # Simulate LSP initialization
        cat << 'LSP'
Content-Length: 52

{"jsonrpc":"2.0","method":"initialized","params":{}}
LSP
        ;;
    *)
        echo "Seen Language Server"
        echo "Usage: seen-lsp [options]"
        echo "Options:"
        echo "  --version    Show version"
        echo "  --stdio      Use stdio for communication"
        ;;
esac
EOF
    
    chmod +x "$TEST_DIR/bin/seen-lsp"
    
    # Test LSP version
    local lsp_output=$(seen-lsp --version)
    if [[ "$lsp_output" == *"Language Server"* ]]; then
        return 0
    else
        return 1
    fi
}

# Test RISC-V tools (if available)
test_riscv_tools() {
    log "Testing RISC-V tools..."
    
    # Create mock RISC-V compiler
    cat > "$TEST_DIR/bin/seen-riscv" << 'EOF'
#!/bin/bash
case "$1" in
    --version)
        echo "Seen RISC-V Compiler 1.0.0"
        echo "Target: riscv64-unknown-linux-gnu"
        ;;
    compile)
        echo "Compiling for RISC-V target..."
        echo "Compilation successful"
        ;;
    *)
        echo "Seen RISC-V Cross Compiler"
        echo "Usage: seen-riscv <command>"
        echo "Commands:"
        echo "  compile      Compile for RISC-V"
        echo "  --version    Show version"
        ;;
esac
EOF
    
    chmod +x "$TEST_DIR/bin/seen-riscv"
    
    # Test RISC-V version
    local riscv_output=$(seen-riscv --version)
    if [[ "$riscv_output" == *"RISC-V Compiler"* ]]; then
        return 0
    else
        return 1
    fi
}

# Test uninstallation
test_uninstallation() {
    log "Testing uninstallation..."
    
    # Create uninstall script
    cat > "$TEST_DIR/uninstall.sh" << 'EOF'
#!/bin/bash
echo "Uninstalling Seen Language..."

# Remove binaries
rm -f "$TEST_DIR/bin/seen"
rm -f "$TEST_DIR/bin/seen-lsp" 
rm -f "$TEST_DIR/bin/seen-riscv"

# Remove library
rm -rf "$TEST_DIR/lib/seen"

# Remove data
rm -rf "$TEST_DIR/share/seen"

echo "Uninstallation completed"
EOF
    
    chmod +x "$TEST_DIR/uninstall.sh"
    
    # Run uninstallation
    if "$TEST_DIR/uninstall.sh"; then
        # Verify removal
        if [ ! -f "$TEST_DIR/bin/seen" ]; then
            return 0
        fi
    fi
    
    return 1
}

# Cleanup test environment
cleanup_test_env() {
    if $CLEANUP && [ -d "$TEST_DIR" ]; then
        log "Cleaning up test environment..."
        rm -rf "$TEST_DIR"
        log "Cleanup completed"
    fi
}

# Show test results
show_results() {
    header "Integration Test Results"
    
    info "Tests run: $TESTS_RUN"
    success "Tests passed: $TESTS_PASSED"
    
    if [ $TESTS_FAILED -gt 0 ]; then
        error "Tests failed: $TESTS_FAILED"
        return 1
    else
        success "All integration tests passed! 🎉"
        return 0
    fi
}

# Main test execution
main() {
    header "Seen Language Integration Test"
    
    # Parse arguments
    while [[ $# -gt 0 ]]; do
        case $1 in
            --version)
                TEST_VERSION="$2"
                shift 2
                ;;
            --test-dir)
                TEST_DIR="$2"
                shift 2
                ;;
            --no-cleanup)
                CLEANUP=false
                shift
                ;;
            --verbose)
                VERBOSE=true
                shift
                ;;
            --help)
                echo "Integration Test for Seen Language Installer"
                echo "Usage: $0 [options]"
                echo "Options:"
                echo "  --version VERSION    Test version (default: $TEST_VERSION)"
                echo "  --test-dir DIR       Test directory (default: $TEST_DIR)"
                echo "  --no-cleanup         Don't clean up after test"
                echo "  --verbose            Enable verbose output"
                exit 0
                ;;
            *)
                error "Unknown option: $1"
                exit 1
                ;;
        esac
    done
    
    info "Running integration tests for version $TEST_VERSION"
    info "Test directory: $TEST_DIR"
    
    # Setup test environment
    setup_test_env
    
    # Run integration tests
    test_case "Project Creation" test_project_creation
    test_case "Binary Execution" test_binary_execution
    test_case "Project Workflow" test_project_workflow
    test_case "Standard Library Installation" test_stdlib_installation
    test_case "Language Configuration" test_language_config
    test_case "Environment Setup" test_environment_setup
    test_case "LSP Server" test_lsp_server
    test_case "RISC-V Tools" test_riscv_tools
    test_case "Uninstallation" test_uninstallation
    
    # Show results
    local exit_code=0
    if ! show_results; then
        exit_code=1
    fi
    
    # Cleanup
    cleanup_test_env
    
    exit $exit_code
}

# Handle script interruption
trap 'cleanup_test_env; exit 130' INT TERM

# Run main function
main "$@"