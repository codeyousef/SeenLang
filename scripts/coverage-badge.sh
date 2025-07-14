#!/usr/bin/env bash

# Script to generate coverage badge for README

set -e

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

# Check if grcov is installed
if ! command -v grcov &> /dev/null; then
    echo -e "${YELLOW}grcov is not installed. Installing...${NC}"
    cargo install grcov
fi

# Run tests with coverage instrumentation
echo -e "${GREEN}Running tests with coverage instrumentation...${NC}"
export CARGO_INCREMENTAL=0
export RUSTFLAGS="-Cinstrument-coverage"
export LLVM_PROFILE_FILE="target/coverage/prof/seen-%p-%m.profraw"

# Create coverage directory
mkdir -p target/coverage/prof

# Build and test
cargo build --workspace
cargo test --workspace

# Generate coverage report
echo -e "${GREEN}Generating coverage report...${NC}"
grcov . \
    --binary-path ./target/debug/deps \
    --source-dir . \
    --output-type lcov \
    --branch \
    --ignore-not-existing \
    --ignore "*/.cargo/*" \
    --ignore "*/tests/*" \
    --ignore "*/benches/*" \
    --ignore "*/target/*" \
    --output-path target/coverage/lcov.info

# Extract coverage percentage
if [ -f "target/coverage/lcov.info" ]; then
    # Calculate line coverage percentage
    LINES_FOUND=$(grep -E "^DA:" target/coverage/lcov.info | wc -l)
    LINES_HIT=$(grep -E "^DA:[0-9]+,[1-9]" target/coverage/lcov.info | wc -l)
    
    if [ $LINES_FOUND -gt 0 ]; then
        COVERAGE=$(echo "scale=1; $LINES_HIT * 100 / $LINES_FOUND" | bc)
        echo -e "${GREEN}Coverage: ${COVERAGE}%${NC}"
        
        # Determine badge color based on coverage
        if (( $(echo "$COVERAGE >= 80" | bc -l) )); then
            COLOR="brightgreen"
        elif (( $(echo "$COVERAGE >= 60" | bc -l) )); then
            COLOR="yellow"
        else
            COLOR="red"
        fi
        
        # Generate badge URL
        BADGE_URL="https://img.shields.io/badge/coverage-${COVERAGE}%25-${COLOR}"
        echo -e "${GREEN}Badge URL: ${BADGE_URL}${NC}"
        
        # Update README if it exists
        if [ -f "README.md" ]; then
            # Check if badge already exists in README
            if grep -q "img.shields.io/badge/coverage" README.md; then
                # Update existing badge
                sed -i "s|https://img.shields.io/badge/coverage-[0-9.]*%25-[a-z]*|${BADGE_URL}|g" README.md
                echo -e "${GREEN}Updated coverage badge in README.md${NC}"
            else
                echo -e "${YELLOW}Coverage badge not found in README.md${NC}"
                echo "Add the following to your README.md:"
                echo "![Coverage](${BADGE_URL})"
            fi
        fi
    else
        echo -e "${RED}No coverage data found${NC}"
    fi
else
    echo -e "${RED}Failed to generate coverage report${NC}"
fi

# Clean up
unset CARGO_INCREMENTAL
unset RUSTFLAGS
unset LLVM_PROFILE_FILE