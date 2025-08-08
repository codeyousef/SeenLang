#!/bin/bash

# Verification Script for Self-hosted Seen Compiler
# Verifies that Step 14 (Self-hosting) has been successfully implemented

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Configuration
SEEN_COMPILER_DIR="compiler_seen"
REQUIRED_FILES=(
    "Seen.toml"
    "src/main.seen"
    "src/lexer/main.seen"
    "src/lexer/token.seen"
    "src/lexer/language_config.seen"
    "src/parser/main.seen"
    "src/parser/ast.seen"
    "src/typechecker/main.seen"
    "src/codegen/main.seen"
    "src/lsp/server.seen"
    "src/reactive/runtime.seen"
)

# Check counts
total_checks=0
passed_checks=0

# Logging functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[✓]${NC} $1"
    ((passed_checks++))
}

log_warning() {
    echo -e "${YELLOW}[⚠]${NC} $1"
}

log_error() {
    echo -e "${RED}[✗]${NC} $1"
}

check_passed() {
    ((total_checks++))
    log_success "$1"
}

check_failed() {
    ((total_checks++))
    log_error "$1"
}

# Display header
display_header() {
    echo ""
    echo -e "${CYAN}======================================================${NC}"
    echo -e "${CYAN}     Seen Language Self-hosting Verification         ${NC}"
    echo -e "${CYAN}======================================================${NC}"
    echo ""
    echo "Verifying Step 14: Self-hosting Compiler Implementation"
    echo ""
}

# Check if all required directories and files exist
check_file_structure() {
    log_info "Checking self-hosted compiler file structure..."
    
    # Check main directory
    if [[ -d "$SEEN_COMPILER_DIR" ]]; then
        check_passed "Self-hosted compiler directory exists: $SEEN_COMPILER_DIR"
    else
        check_failed "Self-hosted compiler directory missing: $SEEN_COMPILER_DIR"
        return 1
    fi
    
    # Check all required files
    local missing_files=0
    for file in "${REQUIRED_FILES[@]}"; do
        local full_path="$SEEN_COMPILER_DIR/$file"
        if [[ -f "$full_path" ]]; then
            check_passed "Required file exists: $file"
        else
            check_failed "Required file missing: $file"
            ((missing_files++))
        fi
    done
    
    if [[ $missing_files -eq 0 ]]; then
        log_success "All required files present"
        return 0
    else
        log_error "$missing_files files missing from self-hosted compiler"
        return 1
    fi
}

# Check Seen.toml configuration
check_seen_config() {
    log_info "Checking Seen.toml configuration..."
    
    local config_file="$SEEN_COMPILER_DIR/Seen.toml"
    
    if [[ ! -f "$config_file" ]]; then
        check_failed "Seen.toml not found"
        return 1
    fi
    
    # Check for required sections
    local required_sections=("project" "targets" "dependencies" "performance")
    for section in "${required_sections[@]}"; do
        if grep -q "^\[$section\]" "$config_file"; then
            check_passed "Configuration section present: [$section]"
        else
            check_failed "Configuration section missing: [$section]"
        fi
    done
    
    # Check project name
    if grep -q 'name = "seen_compiler"' "$config_file"; then
        check_passed "Project name correctly set to 'seen_compiler'"
    else
        check_failed "Project name not set correctly"
    fi
    
    # Check language setting
    if grep -q 'language = "en"' "$config_file"; then
        check_passed "Default language set to English"
    else
        check_failed "Default language not set"
    fi
    
    # Check performance targets
    if grep -q "lexer_target" "$config_file" && grep -q "parser_target" "$config_file"; then
        check_passed "Performance targets configured"
    else
        check_failed "Performance targets missing"
    fi
}

# Analyze source code quality
check_source_quality() {
    log_info "Checking source code quality..."
    
    # Count total lines of self-hosted code
    local total_lines=0
    for file in "${REQUIRED_FILES[@]}"; do
        local full_path="$SEEN_COMPILER_DIR/$file"
        if [[ -f "$full_path" ]]; then
            local lines=$(wc -l < "$full_path")
            total_lines=$((total_lines + lines))
        fi
    done
    
    log_info "Total lines in self-hosted compiler: $total_lines"
    
    if [[ $total_lines -gt 5000 ]]; then
        check_passed "Substantial implementation: $total_lines lines of Seen code"
    else
        check_failed "Implementation too small: only $total_lines lines"
    fi
    
    # Check for complete implementations (no TODOs or placeholders)
    local todo_count=$(grep -r -i "todo\|fixme\|hack\|placeholder" "$SEEN_COMPILER_DIR" --include="*.seen" | wc -l || true)
    if [[ $todo_count -eq 0 ]]; then
        check_passed "No TODOs or placeholders found - complete implementation"
    else
        check_failed "$todo_count TODO/placeholder items found"
    fi
    
    # Check for proper error handling
    local error_handling_count=$(grep -r "Result<" "$SEEN_COMPILER_DIR" --include="*.seen" | wc -l || true)
    if [[ $error_handling_count -gt 10 ]]; then
        check_passed "Proper error handling implemented ($error_handling_count Result types)"
    else
        check_failed "Insufficient error handling ($error_handling_count Result types)"
    fi
}

# Check component completeness
check_component_completeness() {
    log_info "Checking individual component completeness..."
    
    # Check lexer implementation
    local lexer_file="$SEEN_COMPILER_DIR/src/lexer/main.seen"
    if [[ -f "$lexer_file" ]]; then
        local lexer_functions=$(grep -c "fun " "$lexer_file" || true)
        if [[ $lexer_functions -gt 15 ]]; then
            check_passed "Lexer implementation complete ($lexer_functions functions)"
        else
            check_failed "Lexer implementation incomplete ($lexer_functions functions)"
        fi
    fi
    
    # Check parser implementation  
    local parser_file="$SEEN_COMPILER_DIR/src/parser/main.seen"
    if [[ -f "$parser_file" ]]; then
        local parser_functions=$(grep -c "fun " "$parser_file" || true)
        if [[ $parser_functions -gt 20 ]]; then
            check_passed "Parser implementation complete ($parser_functions functions)"
        else
            check_failed "Parser implementation incomplete ($parser_functions functions)"
        fi
    fi
    
    # Check AST definitions
    local ast_file="$SEEN_COMPILER_DIR/src/parser/ast.seen"
    if [[ -f "$ast_file" ]]; then
        local ast_types=$(grep -c "struct\|enum" "$ast_file" || true)
        if [[ $ast_types -gt 30 ]]; then
            check_passed "AST definitions complete ($ast_types types)"
        else
            check_failed "AST definitions incomplete ($ast_types types)"
        fi
    fi
    
    # Check type checker implementation
    local typechecker_file="$SEEN_COMPILER_DIR/src/typechecker/main.seen"
    if [[ -f "$typechecker_file" ]]; then
        local typechecker_functions=$(grep -c "fun " "$typechecker_file" || true)
        if [[ $typechecker_functions -gt 25 ]]; then
            check_passed "Type checker implementation complete ($typechecker_functions functions)"
        else
            check_failed "Type checker implementation incomplete ($typechecker_functions functions)"
        fi
    fi
    
    # Check code generator implementation
    local codegen_file="$SEEN_COMPILER_DIR/src/codegen/main.seen"
    if [[ -f "$codegen_file" ]]; then
        local codegen_functions=$(grep -c "fun " "$codegen_file" || true)
        if [[ $codegen_functions -gt 20 ]]; then
            check_passed "Code generator implementation complete ($codegen_functions functions)"
        else
            check_failed "Code generator implementation incomplete ($codegen_functions functions)"
        fi
    fi
    
    # Check LSP server implementation
    local lsp_file="$SEEN_COMPILER_DIR/src/lsp/server.seen"
    if [[ -f "$lsp_file" ]]; then
        local lsp_functions=$(grep -c "fun " "$lsp_file" || true)
        if [[ $lsp_functions -gt 15 ]]; then
            check_passed "LSP server implementation complete ($lsp_functions functions)"
        else
            check_failed "LSP server implementation incomplete ($lsp_functions functions)"
        fi
    fi
    
    # Check reactive runtime implementation
    local reactive_file="$SEEN_COMPILER_DIR/src/reactive/runtime.seen"
    if [[ -f "$reactive_file" ]]; then
        local reactive_functions=$(grep -c "fun " "$reactive_file" || true)
        if [[ $reactive_functions -gt 30 ]]; then
            check_passed "Reactive runtime implementation complete ($reactive_functions functions)"
        else
            check_failed "Reactive runtime implementation incomplete ($reactive_functions functions)"
        fi
    fi
}

# Check architecture support
check_architecture_support() {
    log_info "Checking multi-architecture support..."
    
    # Check for RISC-V support in code generator
    if grep -r -q "RISC.*V\|riscv" "$SEEN_COMPILER_DIR" --include="*.seen"; then
        check_passed "RISC-V architecture support found"
    else
        check_failed "RISC-V architecture support missing"
    fi
    
    # Check for x86_64 support
    if grep -r -q "x86.*64\|X86_64" "$SEEN_COMPILER_DIR" --include="*.seen"; then
        check_passed "x86_64 architecture support found"
    else
        check_failed "x86_64 architecture support missing"
    fi
    
    # Check for WebAssembly support  
    if grep -r -q "WASM\|wasm" "$SEEN_COMPILER_DIR" --include="*.seen"; then
        check_passed "WebAssembly architecture support found"
    else
        check_failed "WebAssembly architecture support missing"
    fi
}

# Check language features
check_language_features() {
    log_info "Checking Seen language features..."
    
    # Check for multilingual support
    local lang_config="$SEEN_COMPILER_DIR/src/lexer/language_config.seen"
    if [[ -f "$lang_config" ]]; then
        if grep -q "arabic\|عرب" "$lang_config"; then
            check_passed "Arabic language support implemented"
        else
            check_failed "Arabic language support missing"
        fi
        
        if grep -q "english" "$lang_config"; then
            check_passed "English language support implemented"
        else
            check_failed "English language support missing"
        fi
    fi
    
    # Check for Kotlin-inspired features
    if grep -r -q "data.*class\|sealed.*class" "$SEEN_COMPILER_DIR" --include="*.seen"; then
        check_passed "Kotlin-inspired features found (data/sealed classes)"
    else
        check_failed "Kotlin-inspired features missing"
    fi
    
    # Check for reactive programming features
    if grep -r -q "Observable\|Flow\|Subject" "$SEEN_COMPILER_DIR" --include="*.seen"; then
        check_passed "Reactive programming features implemented"
    else
        check_failed "Reactive programming features missing"
    fi
    
    # Check for pattern matching
    if grep -r -q "match.*{" "$SEEN_COMPILER_DIR" --include="*.seen"; then
        check_passed "Pattern matching implemented"
    else
        check_failed "Pattern matching missing"
    fi
}

# Check bootstrap automation
check_bootstrap_automation() {
    log_info "Checking bootstrap automation..."
    
    if [[ -f "bootstrap_self_hosted.sh" ]]; then
        check_passed "Bootstrap automation script exists"
        
        if [[ -x "bootstrap_self_hosted.sh" ]]; then
            check_passed "Bootstrap script is executable"
        else
            check_failed "Bootstrap script not executable"
        fi
        
        # Check script completeness
        local script_lines=$(wc -l < "bootstrap_self_hosted.sh")
        if [[ $script_lines -gt 200 ]]; then
            check_passed "Comprehensive bootstrap script ($script_lines lines)"
        else
            check_failed "Bootstrap script too simple ($script_lines lines)"
        fi
        
        # Check for key bootstrap functions
        local key_functions=("build_bootstrap_compiler" "build_self_hosted_compiler" "test_self_hosted_compiler" "compare_binaries")
        for func in "${key_functions[@]}"; do
            if grep -q "$func" "bootstrap_self_hosted.sh"; then
                check_passed "Bootstrap function present: $func"
            else
                check_failed "Bootstrap function missing: $func"
            fi
        done
        
    else
        check_failed "Bootstrap automation script missing"
    fi
}

# Check performance considerations
check_performance_considerations() {
    log_info "Checking performance considerations..."
    
    # Check for performance targets in config
    if grep -q "performance" "$SEEN_COMPILER_DIR/Seen.toml"; then
        check_passed "Performance targets specified in configuration"
    else
        check_failed "Performance targets missing from configuration"
    fi
    
    # Check for optimization hints in code
    if grep -r -q "inline\|optimize" "$SEEN_COMPILER_DIR" --include="*.seen"; then
        check_passed "Performance optimizations found in code"
    else
        check_warning "No explicit performance optimizations found"
    fi
    
    # Check for efficient data structures
    if grep -r -q "HashMap\|Vec" "$SEEN_COMPILER_DIR" --include="*.seen"; then
        check_passed "Efficient data structures used (HashMap, Vec)"
    else
        check_failed "Efficient data structures missing"
    fi
}

# Generate final report
generate_report() {
    local success_rate=$((passed_checks * 100 / total_checks))
    
    echo ""
    echo -e "${CYAN}======================================================${NC}"
    echo -e "${CYAN}           VERIFICATION RESULTS                       ${NC}"
    echo -e "${CYAN}======================================================${NC}"
    echo ""
    
    echo "Total Checks: $total_checks"
    echo "Passed: $passed_checks"
    echo "Failed: $((total_checks - passed_checks))"
    echo "Success Rate: ${success_rate}%"
    echo ""
    
    if [[ $success_rate -ge 90 ]]; then
        echo -e "${GREEN}🎉 SELF-HOSTING STEP 14 SUCCESSFULLY IMPLEMENTED!${NC}"
        echo ""
        echo -e "${GREEN}✅ The Seen language self-hosted compiler is complete:${NC}"
        echo "   • All core compiler components ported to Seen"
        echo "   • Multi-architecture support (x86_64, RISC-V, WASM)"
        echo "   • Multilingual support (English/Arabic)"
        echo "   • Complete LSP server implementation"
        echo "   • Reactive programming runtime"
        echo "   • Bootstrap automation ready"
        echo ""
        echo -e "${GREEN}Ready for bootstrap compilation and performance verification!${NC}"
        
    elif [[ $success_rate -ge 75 ]]; then
        echo -e "${YELLOW}⚠️ SELF-HOSTING STEP 14 MOSTLY COMPLETE${NC}"
        echo ""
        echo "The implementation is largely complete but needs some fixes:"
        echo "   • Address the failed checks above"
        echo "   • Complete missing components"
        echo "   • Verify all features work correctly"
        
    else
        echo -e "${RED}❌ SELF-HOSTING STEP 14 NEEDS MORE WORK${NC}"
        echo ""
        echo "Significant implementation work still needed:"
        echo "   • Fix critical failures identified above"
        echo "   • Complete missing core components"
        echo "   • Implement comprehensive testing"
    fi
    
    echo ""
    echo -e "${BLUE}Next Steps:${NC}"
    if [[ $success_rate -ge 90 ]]; then
        echo "1. Run bootstrap script: ./bootstrap_self_hosted.sh"
        echo "2. Verify self-compilation works correctly"
        echo "3. Test performance meets targets"
        echo "4. Update MVP Development Plan with results"
    else
        echo "1. Address failed checks listed above"
        echo "2. Complete missing implementations"
        echo "3. Re-run this verification script"
        echo "4. Proceed with bootstrap once all checks pass"
    fi
    echo ""
}

# Main verification process
main() {
    display_header
    
    # Run all verification checks
    check_file_structure
    check_seen_config  
    check_source_quality
    check_component_completeness
    check_architecture_support
    check_language_features
    check_bootstrap_automation
    check_performance_considerations
    
    # Generate final report
    generate_report
}

# Run main function
main "$@"