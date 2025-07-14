#!/usr/bin/env bash

# Script to run code coverage analysis for the Seen language project

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Default values
OUTPUT_FORMAT="html"
OPEN_REPORT=false
CLEAN_FIRST=false
PACKAGE=""
VERBOSE=false

# Help function
show_help() {
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  -f, --format FORMAT    Output format: html, xml, lcov, json (default: html)"
    echo "  -o, --open            Open HTML report in browser after generation"
    echo "  -c, --clean           Clean previous coverage data before running"
    echo "  -p, --package PKG     Run coverage for specific package only"
    echo "  -v, --verbose         Verbose output"
    echo "  -h, --help            Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0                    # Run with default settings"
    echo "  $0 -f lcov           # Generate LCOV format for CI"
    echo "  $0 -p seen_lexer -o  # Run for lexer only and open report"
    echo "  $0 -c -o             # Clean, run all, and open report"
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -f|--format)
            OUTPUT_FORMAT="$2"
            shift 2
            ;;
        -o|--open)
            OPEN_REPORT=true
            shift
            ;;
        -c|--clean)
            CLEAN_FIRST=true
            shift
            ;;
        -p|--package)
            PACKAGE="$2"
            shift 2
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

# Check if tarpaulin is installed
if ! command -v cargo-tarpaulin &> /dev/null; then
    echo -e "${YELLOW}cargo-tarpaulin is not installed. Installing...${NC}"
    cargo install cargo-tarpaulin
fi

# Clean previous coverage data if requested
if [ "$CLEAN_FIRST" = true ]; then
    echo -e "${YELLOW}Cleaning previous coverage data...${NC}"
    rm -rf target/coverage
    cargo clean
fi

# Build the tarpaulin command
TARPAULIN_CMD="cargo tarpaulin"

# Add format option
case $OUTPUT_FORMAT in
    html)
        TARPAULIN_CMD="$TARPAULIN_CMD --out Html"
        ;;
    xml)
        TARPAULIN_CMD="$TARPAULIN_CMD --out Xml"
        ;;
    lcov)
        TARPAULIN_CMD="$TARPAULIN_CMD --out Lcov"
        ;;
    json)
        TARPAULIN_CMD="$TARPAULIN_CMD --out Json"
        ;;
    *)
        echo -e "${RED}Invalid format: $OUTPUT_FORMAT${NC}"
        exit 1
        ;;
esac

# Add package option if specified
if [ -n "$PACKAGE" ]; then
    TARPAULIN_CMD="$TARPAULIN_CMD --packages $PACKAGE"
else
    TARPAULIN_CMD="$TARPAULIN_CMD --workspace"
fi

# Add verbose flag if requested
if [ "$VERBOSE" = true ]; then
    TARPAULIN_CMD="$TARPAULIN_CMD --verbose"
fi

# Add common options
TARPAULIN_CMD="$TARPAULIN_CMD --all-features --timeout 300 --output-dir target/coverage"

# Run tarpaulin
echo -e "${GREEN}Running code coverage analysis...${NC}"
echo -e "${YELLOW}Command: $TARPAULIN_CMD${NC}"
eval $TARPAULIN_CMD

# Check if coverage succeeded
if [ $? -eq 0 ]; then
    echo -e "${GREEN}Coverage analysis completed successfully!${NC}"
    
    # Show coverage summary
    if [ -f "target/coverage/tarpaulin-report.json" ]; then
        echo -e "\n${GREEN}Coverage Summary:${NC}"
        # Extract and display coverage percentage from JSON
        # (This would require jq or similar tool)
    fi
    
    # Open report if requested
    if [ "$OPEN_REPORT" = true ] && [ "$OUTPUT_FORMAT" = "html" ]; then
        if [ -f "target/coverage/tarpaulin-report.html" ]; then
            echo -e "${GREEN}Opening coverage report in browser...${NC}"
            if command -v xdg-open &> /dev/null; then
                xdg-open target/coverage/tarpaulin-report.html
            elif command -v open &> /dev/null; then
                open target/coverage/tarpaulin-report.html
            elif command -v start &> /dev/null; then
                start target/coverage/tarpaulin-report.html
            else
                echo -e "${YELLOW}Could not open browser. Report available at: target/coverage/tarpaulin-report.html${NC}"
            fi
        fi
    fi
else
    echo -e "${RED}Coverage analysis failed!${NC}"
    exit 1
fi

# Show report location
echo -e "\n${GREEN}Coverage reports generated in: target/coverage/${NC}"
case $OUTPUT_FORMAT in
    html)
        echo "  - HTML report: target/coverage/tarpaulin-report.html"
        ;;
    xml)
        echo "  - XML report: target/coverage/cobertura.xml"
        ;;
    lcov)
        echo "  - LCOV report: target/coverage/lcov.info"
        ;;
    json)
        echo "  - JSON report: target/coverage/tarpaulin-report.json"
        ;;
esac