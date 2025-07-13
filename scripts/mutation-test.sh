#!/usr/bin/env bash

# Script to run mutation testing for the Seen language project

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Default values
PACKAGE=""
CHECK_ONLY=false
JOBS=""
TIMEOUT=""
VERBOSE=false
GENERATE_REPORT=true

# Help function
show_help() {
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  -p, --package PKG     Run mutations for specific package only"
    echo "  -c, --check           Check if tests can catch mutants (dry run)"
    echo "  -j, --jobs NUM        Number of parallel jobs"
    echo "  -t, --timeout SECS    Timeout for each mutant (seconds)"
    echo "  -n, --no-report       Don't generate HTML report"
    echo "  -v, --verbose         Verbose output"
    echo "  -h, --help            Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0                    # Run mutation testing on all packages"
    echo "  $0 -p seen_lexer      # Run only on lexer package"
    echo "  $0 -c -j 8            # Check mode with 8 parallel jobs"
    echo "  $0 -t 60              # Set 60 second timeout per mutant"
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -p|--package)
            PACKAGE="$2"
            shift 2
            ;;
        -c|--check)
            CHECK_ONLY=true
            shift
            ;;
        -j|--jobs)
            JOBS="$2"
            shift 2
            ;;
        -t|--timeout)
            TIMEOUT="$2"
            shift 2
            ;;
        -n|--no-report)
            GENERATE_REPORT=false
            shift
            ;;
        -v|--verbose)
            VERBOSE=true
            shift
            ;;
        -h|--help)
            show_help
            exit 0
            ;;
        *)
            echo -e "${RED}Unknown option: $1${NC}"
            show_help
            exit 1
            ;;
    esac
done

# Check if cargo-mutants is installed
if ! command -v cargo-mutants &> /dev/null; then
    echo -e "${YELLOW}cargo-mutants is not installed. Installing...${NC}"
    cargo install cargo-mutants
fi

# First, ensure all tests pass
echo -e "${GREEN}Running tests to ensure they pass...${NC}"
if [ -n "$PACKAGE" ]; then
    cargo test -p "$PACKAGE" --all-features
else
    cargo test --all --all-features
fi

# Build the mutants command
MUTANTS_CMD="cargo mutants"

# Add package option if specified
if [ -n "$PACKAGE" ]; then
    MUTANTS_CMD="$MUTANTS_CMD --package $PACKAGE"
fi

# Add check mode if requested
if [ "$CHECK_ONLY" = true ]; then
    MUTANTS_CMD="$MUTANTS_CMD --check"
fi

# Add jobs option if specified
if [ -n "$JOBS" ]; then
    MUTANTS_CMD="$MUTANTS_CMD --jobs $JOBS"
fi

# Add timeout option if specified
if [ -n "$TIMEOUT" ]; then
    MUTANTS_CMD="$MUTANTS_CMD --timeout $TIMEOUT"
fi

# Add verbose flag if requested
if [ "$VERBOSE" = true ]; then
    MUTANTS_CMD="$MUTANTS_CMD --verbose"
fi

# Create output directory
mkdir -p target/mutants

# Run mutation testing
echo -e "${GREEN}Running mutation testing...${NC}"
echo -e "${YELLOW}Command: $MUTANTS_CMD${NC}"
echo -e "${BLUE}This may take a while...${NC}"

# Run and capture output
if eval $MUTANTS_CMD > target/mutants/mutants.log 2>&1; then
    echo -e "${GREEN}Mutation testing completed!${NC}"
else
    echo -e "${RED}Mutation testing failed! Check target/mutants/mutants.log for details${NC}"
    tail -20 target/mutants/mutants.log
    exit 1
fi

# Parse results
if [ -f "target/mutants/mutants.log" ]; then
    echo -e "\n${GREEN}Mutation Testing Results:${NC}"
    echo "================================"
    
    # Extract summary from log
    TOTAL=$(grep -E "Generated [0-9]+ mutants" target/mutants/mutants.log | grep -oE "[0-9]+" | head -1 || echo "0")
    CAUGHT=$(grep -E "Caught [0-9]+" target/mutants/mutants.log | grep -oE "[0-9]+" | head -1 || echo "0")
    MISSED=$(grep -E "Missed [0-9]+" target/mutants/mutants.log | grep -oE "[0-9]+" | head -1 || echo "0")
    TIMEOUT=$(grep -E "Timeout [0-9]+" target/mutants/mutants.log | grep -oE "[0-9]+" | head -1 || echo "0")
    
    if [ "$TOTAL" -gt 0 ]; then
        SCORE=$(echo "scale=1; $CAUGHT * 100 / $TOTAL" | bc)
        
        echo "Total mutants:    $TOTAL"
        echo "Caught:          $CAUGHT"
        echo "Missed:          $MISSED"
        echo "Timeout:         $TIMEOUT"
        echo "Mutation score:  ${SCORE}%"
        
        # Show quality indicator
        if (( $(echo "$SCORE >= 80" | bc -l) )); then
            echo -e "\n${GREEN}✓ Excellent mutation coverage!${NC}"
        elif (( $(echo "$SCORE >= 60" | bc -l) )); then
            echo -e "\n${YELLOW}⚠ Good mutation coverage, but could be improved${NC}"
        else
            echo -e "\n${RED}✗ Poor mutation coverage - more tests needed${NC}"
        fi
    fi
    
    # Show missed mutants if any
    if [ "$MISSED" -gt 0 ]; then
        echo -e "\n${YELLOW}Missed Mutants:${NC}"
        echo "---------------"
        grep -A 2 "MISSED:" target/mutants/mutants.log | head -20
        echo ""
        echo "Run with -v flag for more details on missed mutants"
    fi
fi

# Generate HTML report if requested
if [ "$GENERATE_REPORT" = true ] && [ -f "mutants.out/caught.txt" ]; then
    echo -e "\n${GREEN}Generating HTML report...${NC}"
    
    # Create simple HTML report
    cat > target/mutants/report.html << EOF
<!DOCTYPE html>
<html>
<head>
    <title>Mutation Testing Report - Seen Language</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 20px; }
        .summary { background: #f0f0f0; padding: 15px; border-radius: 5px; }
        .good { color: green; }
        .warning { color: orange; }
        .bad { color: red; }
        .mutant { margin: 10px 0; padding: 10px; background: #f8f8f8; }
        .caught { border-left: 3px solid green; }
        .missed { border-left: 3px solid red; }
    </style>
</head>
<body>
    <h1>Mutation Testing Report</h1>
    <div class="summary">
        <h2>Summary</h2>
        <p>Total Mutants: $TOTAL</p>
        <p>Caught: <span class="good">$CAUGHT</span></p>
        <p>Missed: <span class="bad">$MISSED</span></p>
        <p>Timeout: <span class="warning">$TIMEOUT</span></p>
        <p>Mutation Score: <strong>${SCORE}%</strong></p>
    </div>
    
    <h2>Details</h2>
    <p>See <code>mutants.out/</code> directory for detailed results.</p>
</body>
</html>
EOF
    
    echo -e "${GREEN}HTML report generated at: target/mutants/report.html${NC}"
fi

# Show output locations
echo -e "\n${GREEN}Output files:${NC}"
echo "  - Log file: target/mutants/mutants.log"
echo "  - Detailed results: mutants.out/"
if [ "$GENERATE_REPORT" = true ]; then
    echo "  - HTML report: target/mutants/report.html"
fi

# Exit with appropriate code
if [ "$MISSED" -gt 0 ]; then
    exit 1
else
    exit 0
fi