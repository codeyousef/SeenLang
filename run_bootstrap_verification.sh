#!/bin/bash

# 3-Stage Bootstrap Verification Script
# Performs the complete self-hosting verification for the Seen compiler
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LOG_FILE="bootstrap_results.log"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
NC='\033[0m' # No Color

log() {
    echo -e "$1" | tee -a "$LOG_FILE"
}

log_header() {
    echo ""
    echo "=============================================="
    echo "$1"
    echo "=============================================="
    echo ""
}

compile_c_to_executable() {
    local c_file="$1"
    local exe_file="$2"
    
    log "   Compiling C code to executable..."
    log "   Command: clang $c_file -o $exe_file"
    
    if clang "$c_file" -o "$exe_file" 2>&1 | tee -a "$LOG_FILE"; then
        if [ -f "$exe_file" ]; then
            chmod +x "$exe_file"
            log "   ✅ Successfully created executable: $exe_file"
            return 0
        else
            log "   ❌ Executable was not created"
            return 1
        fi
    else
        log "   ❌ C compilation failed"
        return 1
    fi
}

# Record start time
START_TIME=$(date +%s)
log_header "🚀 STARTING 3-STAGE BOOTSTRAP VERIFICATION"
log "$(date): Starting bootstrap verification"
log "Working directory: $SCRIPT_DIR"

# Pre-verification checks
log_header "📋 PRE-VERIFICATION CHECKS"

if [ ! -f "compiler_seen/src/main_compiler.seen" ]; then
    log "${RED}❌ FATAL: Seen compiler source not found!${NC}"
    exit 1
fi

if [ ! -f "target/release/seen_cli" ]; then
    log "${RED}❌ FATAL: Rust bootstrap compiler not built!${NC}"
    log "Please run: cargo build --release"
    exit 1
fi

if ! command -v clang &> /dev/null; then
    log "${RED}❌ FATAL: clang not found! Required for C compilation${NC}"
    exit 1
fi

log "${GREEN}✅ Seen compiler source found${NC}"
log "${GREEN}✅ Rust bootstrap compiler ready${NC}"
log "${GREEN}✅ clang compiler available${NC}"

# Clean up any previous bootstrap files
log "🧹 Cleaning up previous bootstrap attempts..."
rm -f stage1_compiler stage2_compiler stage3_compiler
rm -f stage1_compiler.exe stage2_compiler.exe stage3_compiler.exe
rm -f stage1_compiler.c stage2_compiler.c stage3_compiler.c
rm -f *.o

log_header "🎯 STAGE 1: Rust Compiler → Seen Compiler"
log "${BLUE}Using Rust bootstrap compiler to compile Seen compiler${NC}"

STAGE1_START=$(date +%s)
log "Command: cargo run -p seen_cli --release -- build compiler_seen/src/main_compiler.seen -o stage1_compiler.c"

if cargo run -p seen_cli --release -- build compiler_seen/src/main_compiler.seen -o stage1_compiler.c 2>&1 | tee -a "$LOG_FILE"; then
    if [ -f "stage1_compiler.c" ]; then
        log "   ✅ Generated C code: stage1_compiler.c"
        
        # Compile C to executable
        if compile_c_to_executable "stage1_compiler.c" "stage1_compiler"; then
            STAGE1_END=$(date +%s)
            STAGE1_DURATION=$((STAGE1_END - STAGE1_START))
            
            STAGE1_SIZE=$(wc -c < "stage1_compiler")
            STAGE1_HASH=$(sha256sum "stage1_compiler" | cut -d' ' -f1)
            
            log "${GREEN}✅ STAGE 1 SUCCESS${NC}"
            log "   Duration: ${STAGE1_DURATION}s"
            log "   Binary: stage1_compiler"
            log "   Size: ${STAGE1_SIZE} bytes"
            log "   SHA256: $STAGE1_HASH"
        else
            log "${RED}❌ STAGE 1 FAILED: C compilation failed${NC}"
            exit 1
        fi
    else
        log "${RED}❌ STAGE 1 FAILED: No C code generated${NC}"
        exit 1
    fi
else
    log "${RED}❌ STAGE 1 FAILED: Seen compilation error${NC}"
    exit 1
fi

log_header "🎯 STAGE 2: Stage 1 Compiler → Seen Compiler (Self-Compilation #1)"
log "${BLUE}Using Stage 1 compiler to compile Seen compiler${NC}"

STAGE2_START=$(date +%s)
log "Command: ./stage1_compiler build compiler_seen/src/main_compiler.seen -o stage2_compiler.c"

if "./stage1_compiler" build compiler_seen/src/main_compiler.seen -o stage2_compiler.c 2>&1 | tee -a "$LOG_FILE"; then
    if [ -f "stage2_compiler.c" ]; then
        log "   ✅ Generated C code: stage2_compiler.c"
        
        # Compile C to executable
        if compile_c_to_executable "stage2_compiler.c" "stage2_compiler"; then
            STAGE2_END=$(date +%s)
            STAGE2_DURATION=$((STAGE2_END - STAGE2_START))
            
            STAGE2_SIZE=$(wc -c < "stage2_compiler")
            STAGE2_HASH=$(sha256sum "stage2_compiler" | cut -d' ' -f1)
            
            log "${GREEN}✅ STAGE 2 SUCCESS${NC}"
            log "   Duration: ${STAGE2_DURATION}s"
            log "   Binary: stage2_compiler"
            log "   Size: ${STAGE2_SIZE} bytes"
            log "   SHA256: $STAGE2_HASH"
        else
            log "${RED}❌ STAGE 2 FAILED: C compilation failed${NC}"
            exit 1
        fi
    else
        log "${RED}❌ STAGE 2 FAILED: No C code generated${NC}"
        exit 1
    fi
else
    log "${RED}❌ STAGE 2 FAILED: Seen compilation error${NC}"
    exit 1
fi

log_header "🎯 STAGE 3: Stage 2 Compiler → Seen Compiler (Self-Compilation #2)"
log "${BLUE}Using Stage 2 compiler to compile Seen compiler${NC}"

STAGE3_START=$(date +%s)
log "Command: ./stage2_compiler build compiler_seen/src/main_compiler.seen -o stage3_compiler.c"

if "./stage2_compiler" build compiler_seen/src/main_compiler.seen -o stage3_compiler.c 2>&1 | tee -a "$LOG_FILE"; then
    if [ -f "stage3_compiler.c" ]; then
        log "   ✅ Generated C code: stage3_compiler.c"
        
        # Compile C to executable
        if compile_c_to_executable "stage3_compiler.c" "stage3_compiler"; then
            STAGE3_END=$(date +%s)
            STAGE3_DURATION=$((STAGE3_END - STAGE3_START))
            
            STAGE3_SIZE=$(wc -c < "stage3_compiler")
            STAGE3_HASH=$(sha256sum "stage3_compiler" | cut -d' ' -f1)
            
            log "${GREEN}✅ STAGE 3 SUCCESS${NC}"
            log "   Duration: ${STAGE3_DURATION}s"
            log "   Binary: stage3_compiler"
            log "   Size: ${STAGE3_SIZE} bytes"
            log "   SHA256: $STAGE3_HASH"
        else
            log "${RED}❌ STAGE 3 FAILED: C compilation failed${NC}"
            exit 1
        fi
    else
        log "${RED}❌ STAGE 3 FAILED: No C code generated${NC}"
        exit 1
    fi
else
    log "${RED}❌ STAGE 3 FAILED: Seen compilation error${NC}"
    exit 1
fi

log_header "🔍 VERIFICATION: Comparing Stage 2 and Stage 3 Binaries"
log "${PURPLE}Checking if Stage 2 and Stage 3 produce identical output${NC}"

# Compare file sizes
log "Stage 2 size: ${STAGE2_SIZE} bytes"
log "Stage 3 size: ${STAGE3_SIZE} bytes"

if [ "$STAGE2_SIZE" != "$STAGE3_SIZE" ]; then
    log "${YELLOW}⚠️  WARNING: Stage 2 and Stage 3 have different sizes${NC}"
    log "This might indicate a non-deterministic build or compilation differences"
fi

# Compare hashes
log "Stage 2 SHA256: $STAGE2_HASH"
log "Stage 3 SHA256: $STAGE3_HASH"

TOTAL_END=$(date +%s)
TOTAL_DURATION=$((TOTAL_END - START_TIME))

if [ "$STAGE2_HASH" = "$STAGE3_HASH" ]; then
    log_header "🎉 BOOTSTRAP VERIFICATION SUCCESSFUL!"
    log "${GREEN}✅ SELF-HOSTING ACHIEVED! 🎉${NC}"
    log ""
    log "${GREEN}Stage 2 and Stage 3 compilers are IDENTICAL!${NC}"
    log "Hash: $STAGE2_HASH"
    log "Size: ${STAGE2_SIZE} bytes"
    log ""
    log "📊 TIMING SUMMARY:"
    log "   Stage 1 (Rust → Seen):     ${STAGE1_DURATION}s"
    log "   Stage 2 (Self-compile #1): ${STAGE2_DURATION}s"
    log "   Stage 3 (Self-compile #2): ${STAGE3_DURATION}s"
    log "   Total bootstrap time:       ${TOTAL_DURATION}s"
    log ""
    log "${GREEN}The Seen compiler can now compile itself!${NC}"
    log "${GREEN}The language implementation is complete and self-hosting.${NC}"
    
    # Create final self-hosted compiler
    log "📦 Creating final self-hosted compiler..."
    cp "stage2_compiler" "seen_compiler_self_hosted"
    log "${GREEN}✅ Created: seen_compiler_self_hosted${NC}"
    
    exit 0
else
    log_header "❌ BOOTSTRAP VERIFICATION FAILED"
    log "${RED}✗ Stage 2 and Stage 3 compilers are DIFFERENT!${NC}"
    log ""
    log "This indicates that the compiler is not yet deterministic or"
    log "there may be remaining implementation issues that need to be"
    log "resolved before achieving true self-hosting."
    log ""
    log "Stage 2 hash: $STAGE2_HASH"
    log "Stage 3 hash: $STAGE3_HASH"
    log ""
    log "📊 TIMING SUMMARY:"
    log "   Stage 1 (Rust → Seen):     ${STAGE1_DURATION}s"
    log "   Stage 2 (Self-compile #1): ${STAGE2_DURATION}s"
    log "   Stage 3 (Self-compile #2): ${STAGE3_DURATION}s"
    log "   Total bootstrap time:       ${TOTAL_DURATION}s"
    
    exit 1
fi