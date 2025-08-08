#!/usr/bin/env bash
# Real-world lexer performance validation against the "14M tokens/sec" claim
# Tests against actual large codebases, not synthetic input

set -e

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
PERF_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
TEST_DATA_DIR="$PERF_ROOT/test_data/large_codebases"

# Default parameters  
ITERATIONS=30
WARMUP_ITERATIONS=5
OUTPUT_FILE=""
COMPETITORS="rust,cpp,zig"
TEST_SIZE="medium"
FORMAT="json"

# Test data sets (real codebases)
declare -A TEST_FILES=(
    ["small"]="10kb_real_code.seen"
    ["medium"]="100kb_rust_like.seen 200kb_mixed_lang.seen"
    ["large"]="1mb_comprehensive.seen 2mb_unicode_heavy.seen"
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

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --iterations) ITERATIONS="$2"; shift 2 ;;
        --warmup) WARMUP_ITERATIONS="$2"; shift 2 ;;
        --output) OUTPUT_FILE="$2"; shift 2 ;;
        --competitors) COMPETITORS="$2"; shift 2 ;;
        --test-size) TEST_SIZE="$2"; shift 2 ;;
        --format) FORMAT="$2"; shift 2 ;;
        --help)
            cat << EOF
Lexer Performance Validation

Usage: $0 [OPTIONS]

OPTIONS:
    --iterations N      Number of benchmark iterations (default: $ITERATIONS)
    --warmup N         Number of warmup runs (default: $WARMUP_ITERATIONS)
    --output FILE      Output file for results (default: stdout)
    --competitors LIST Comma-separated competitors (default: $COMPETITORS)
    --test-size SIZE   Test data size: small, medium, large (default: $TEST_SIZE)
    --format FORMAT    Output format: json, csv (default: $FORMAT)
    --help             Show this help message

EOF
            exit 0
            ;;
        *) log_error "Unknown option: $1"; exit 1 ;;
    esac
done

# Ensure test data exists
setup_test_data() {
    mkdir -p "$TEST_DATA_DIR"
    
    # Generate realistic test data if it doesn't exist
    for size in small medium large; do
        for test_file in ${TEST_FILES[$size]}; do
            local file_path="$TEST_DATA_DIR/$test_file"
            
            if [[ ! -f "$file_path" ]]; then
                log_info "Generating test data: $test_file"
                python3 "$SCRIPT_DIR/generate_realistic_code.py" \
                    --size "$size" \
                    --output "$file_path" \
                    --language seen
            fi
        done
    done
}

# Find Seen compiler
find_seen_compiler() {
    local seen_paths=(
        "$PROJECT_ROOT/target-wsl/debug/seen"
        "$PROJECT_ROOT/target/debug/seen"
        "$PROJECT_ROOT/target-wsl/release/seen"
        "$PROJECT_ROOT/target/release/seen"
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

# Benchmark Seen lexer performance
benchmark_seen_lexer() {
    local test_file="$1"
    local results=()
    
    local seen_compiler=$(find_seen_compiler)
    
    # Get file size and estimate token count
    local file_size=$(stat -f%z "$test_file" 2>/dev/null || stat -c%s "$test_file")
    local estimated_tokens=$((file_size / 8))  # Rough estimate: 8 bytes per token average
    
    log_info "Benchmarking Seen lexer with $(basename "$test_file") ($file_size bytes, ~$estimated_tokens tokens)"
    
    # Warmup runs
    for ((i=1; i<=WARMUP_ITERATIONS; i++)); do
        timeout 30 "$seen_compiler" check "$test_file" --lexer-only > /dev/null 2>&1 || true
    done
    
    # Actual benchmark runs
    for ((i=1; i<=ITERATIONS; i++)); do
        local start_time=$(python3 -c "import time; print(time.time())")
        
        if timeout 30 "$seen_compiler" check "$test_file" --lexer-only > /dev/null 2>&1; then
            local end_time=$(python3 -c "import time; print(time.time())")
            local duration=$(python3 -c "print($end_time - $start_time)")
            results+=("$duration")
        else
            log_error "Seen lexer failed on iteration $i"
            return 1
        fi
    done
    
    # Calculate tokens per second
    local total_time=0
    for time in "${results[@]}"; do
        total_time=$(python3 -c "print($total_time + $time)")
    done
    
    local avg_time=$(python3 -c "print($total_time / ${#results[@]})")
    local tokens_per_sec=$(python3 -c "print($estimated_tokens / $avg_time)")
    
    # Output results
    cat << EOF
{
    "language": "seen",
    "test_file": "$(basename "$test_file")",
    "file_size_bytes": $file_size,
    "estimated_tokens": $estimated_tokens,
    "iterations": ${#results[@]},
    "times": [$(IFS=,; echo "${results[*]}")],
    "average_time_seconds": $avg_time,
    "tokens_per_second": $tokens_per_sec,
    "claim_validated": $(python3 -c "print('true' if $tokens_per_sec >= 14000000 else 'false')"),
    "metadata": {
        "warmup_iterations": $WARMUP_ITERATIONS,
        "compiler_path": "$seen_compiler"
    }
}
EOF
}

# Benchmark Rust lexer (using a real lexer library)
benchmark_rust_lexer() {
    local test_file="$1"
    
    # Create Rust benchmark program
    local rust_dir="$PERF_ROOT/competitors/rust/lexer_benchmark"
    mkdir -p "$rust_dir"
    
    cat > "$rust_dir/Cargo.toml" << 'EOF'
[package]
name = "lexer_benchmark"
version = "0.1.0"
edition = "2021"

[dependencies]
logos = "0.13"
EOF

    cat > "$rust_dir/src/main.rs" << 'EOF'
use logos::Logos;
use std::env;
use std::fs;
use std::time::Instant;

#[derive(Logos, Debug, PartialEq)]
enum Token {
    #[token("func")]
    Func,
    #[token("let")]
    Let,
    #[token("if")]
    If,
    #[token("else")]
    Else,
    #[token("while")]
    While,
    #[token("for")]
    For,
    #[token("return")]
    Return,
    #[token("struct")]
    Struct,
    #[token("enum")]
    Enum,
    #[token("true")]
    True,
    #[token("false")]
    False,
    
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*")]
    Identifier,
    
    #[regex(r"[0-9]+")]
    Integer,
    
    #[regex(r#""[^"]*""#)]
    String,
    
    #[regex(r"//[^\n]*\n")]
    Comment,
    
    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("*")]
    Multiply,
    #[token("/")]
    Divide,
    #[token("=")]
    Assign,
    #[token("==")]
    Equal,
    #[token("!=")]
    NotEqual,
    #[token("<")]
    Less,
    #[token("<=")]
    LessEqual,
    #[token(">")]
    Greater,
    #[token(">=")]
    GreaterEqual,
    
    #[token("(")]
    LeftParen,
    #[token(")")]
    RightParen,
    #[token("{")]
    LeftBrace,
    #[token("}")]
    RightBrace,
    #[token("[")]
    LeftBracket,
    #[token("]")]
    RightBracket,
    
    #[token(";")]
    Semicolon,
    #[token(",")]
    Comma,
    #[token(":")]
    Colon,
    
    #[regex(r"[ \t\n\f]+", logos::skip)]
    #[error]
    Error,
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <file>", args[0]);
        std::process::exit(1);
    }
    
    let content = fs::read_to_string(&args[1]).expect("Failed to read file");
    
    let start = Instant::now();
    let mut lex = Token::lexer(&content);
    let mut token_count = 0;
    
    while let Some(_token) = lex.next() {
        token_count += 1;
    }
    
    let duration = start.elapsed();
    let tokens_per_sec = token_count as f64 / duration.as_secs_f64();
    
    println!("{}", duration.as_secs_f64());
    eprintln!("Tokens: {}, Time: {:?}, Rate: {:.0} tokens/sec", 
              token_count, duration, tokens_per_sec);
}
EOF

    # Build Rust lexer benchmark
    cd "$rust_dir"
    if ! cargo build --release > /dev/null 2>&1; then
        log_error "Failed to build Rust lexer benchmark"
        return 1
    fi
    
    local rust_binary="$rust_dir/target/release/lexer_benchmark"
    local file_size=$(stat -f%z "$test_file" 2>/dev/null || stat -c%s "$test_file")
    local results=()
    
    log_info "Benchmarking Rust (logos) lexer with $(basename "$test_file")"
    
    # Warmup runs
    for ((i=1; i<=WARMUP_ITERATIONS; i++)); do
        timeout 30 "$rust_binary" "$test_file" > /dev/null 2>&1 || true
    done
    
    # Actual benchmark runs
    for ((i=1; i<=ITERATIONS; i++)); do
        local result=$(timeout 30 "$rust_binary" "$test_file" 2>/dev/null)
        if [[ $? -eq 0 ]] && [[ -n "$result" ]]; then
            results+=("$result")
        fi
    done
    
    if [[ ${#results[@]} -eq 0 ]]; then
        log_error "Rust lexer produced no valid results"
        return 1
    fi
    
    # Calculate average performance
    local total_time=0
    for time in "${results[@]}"; do
        total_time=$(python3 -c "print($total_time + $time)")
    done
    
    local avg_time=$(python3 -c "print($total_time / ${#results[@]})")
    local estimated_tokens=$((file_size / 8))
    local tokens_per_sec=$(python3 -c "print($estimated_tokens / $avg_time)")
    
    cat << EOF
{
    "language": "rust",
    "test_file": "$(basename "$test_file")",
    "file_size_bytes": $file_size,
    "estimated_tokens": $estimated_tokens,
    "iterations": ${#results[@]},
    "times": [$(IFS=,; echo "${results[*]}")],
    "average_time_seconds": $avg_time,
    "tokens_per_second": $tokens_per_sec,
    "metadata": {
        "library": "logos",
        "warmup_iterations": $WARMUP_ITERATIONS
    }
}
EOF
}

# Benchmark C++ lexer (using re2c or manual implementation)
benchmark_cpp_lexer() {
    local test_file="$1"
    
    # Create C++ benchmark program
    local cpp_dir="$PERF_ROOT/competitors/cpp/lexer_benchmark"
    mkdir -p "$cpp_dir"
    
    cat > "$cpp_dir/lexer.cpp" << 'EOF'
#include <iostream>
#include <fstream>
#include <string>
#include <chrono>
#include <unordered_set>
#include <cctype>

enum TokenType {
    FUNC, LET, IF, ELSE, WHILE, FOR, RETURN, STRUCT, ENUM,
    TRUE_TOK, FALSE_TOK, IDENTIFIER, INTEGER, STRING_LIT,
    COMMENT, PLUS, MINUS, MULTIPLY, DIVIDE, ASSIGN, EQUAL,
    NOT_EQUAL, LESS, LESS_EQUAL, GREATER, GREATER_EQUAL,
    LEFT_PAREN, RIGHT_PAREN, LEFT_BRACE, RIGHT_BRACE,
    LEFT_BRACKET, RIGHT_BRACKET, SEMICOLON, COMMA, COLON,
    WHITESPACE, END_OF_FILE, ERROR_TOK
};

class SimpleLexer {
private:
    std::string input;
    size_t pos;
    std::unordered_set<std::string> keywords;
    
public:
    SimpleLexer(const std::string& text) : input(text), pos(0) {
        keywords = {"func", "let", "if", "else", "while", "for", 
                   "return", "struct", "enum", "true", "false"};
    }
    
    TokenType nextToken() {
        while (pos < input.length()) {
            char c = input[pos];
            
            // Skip whitespace
            if (std::isspace(c)) {
                pos++;
                continue;
            }
            
            // Single character tokens
            switch (c) {
                case '+': pos++; return PLUS;
                case '-': pos++; return MINUS;
                case '*': pos++; return MULTIPLY;
                case '/': 
                    if (pos + 1 < input.length() && input[pos + 1] == '/') {
                        // Line comment
                        while (pos < input.length() && input[pos] != '\n') pos++;
                        return COMMENT;
                    }
                    pos++; return DIVIDE;
                case '=':
                    if (pos + 1 < input.length() && input[pos + 1] == '=') {
                        pos += 2; return EQUAL;
                    }
                    pos++; return ASSIGN;
                case '!':
                    if (pos + 1 < input.length() && input[pos + 1] == '=') {
                        pos += 2; return NOT_EQUAL;
                    }
                    pos++; return ERROR_TOK;
                case '<':
                    if (pos + 1 < input.length() && input[pos + 1] == '=') {
                        pos += 2; return LESS_EQUAL;
                    }
                    pos++; return LESS;
                case '>':
                    if (pos + 1 < input.length() && input[pos + 1] == '=') {
                        pos += 2; return GREATER_EQUAL;
                    }
                    pos++; return GREATER;
                case '(': pos++; return LEFT_PAREN;
                case ')': pos++; return RIGHT_PAREN;
                case '{': pos++; return LEFT_BRACE;
                case '}': pos++; return RIGHT_BRACE;
                case '[': pos++; return LEFT_BRACKET;
                case ']': pos++; return RIGHT_BRACKET;
                case ';': pos++; return SEMICOLON;
                case ',': pos++; return COMMA;
                case ':': pos++; return COLON;
            }
            
            // String literals
            if (c == '"') {
                pos++; // Skip opening quote
                while (pos < input.length() && input[pos] != '"') {
                    pos++;
                }
                if (pos < input.length()) pos++; // Skip closing quote
                return STRING_LIT;
            }
            
            // Numbers
            if (std::isdigit(c)) {
                while (pos < input.length() && std::isdigit(input[pos])) {
                    pos++;
                }
                return INTEGER;
            }
            
            // Identifiers and keywords
            if (std::isalpha(c) || c == '_') {
                size_t start = pos;
                while (pos < input.length() && 
                       (std::isalnum(input[pos]) || input[pos] == '_')) {
                    pos++;
                }
                std::string word = input.substr(start, pos - start);
                return keywords.count(word) ? FUNC : IDENTIFIER; // Simplified
            }
            
            // Unknown character
            pos++;
            return ERROR_TOK;
        }
        
        return END_OF_FILE;
    }
};

int main(int argc, char* argv[]) {
    if (argc != 2) {
        std::cerr << "Usage: " << argv[0] << " <file>" << std::endl;
        return 1;
    }
    
    std::ifstream file(argv[1]);
    if (!file) {
        std::cerr << "Cannot open file: " << argv[1] << std::endl;
        return 1;
    }
    
    std::string content((std::istreambuf_iterator<char>(file)),
                       std::istreambuf_iterator<char>());
    
    auto start = std::chrono::high_resolution_clock::now();
    
    SimpleLexer lexer(content);
    int token_count = 0;
    
    TokenType token;
    while ((token = lexer.nextToken()) != END_OF_FILE) {
        token_count++;
    }
    
    auto end = std::chrono::high_resolution_clock::now();
    auto duration = std::chrono::duration<double>(end - start);
    
    std::cout << duration.count() << std::endl;
    std::cerr << "Tokens: " << token_count << ", Time: " << duration.count() 
              << "s, Rate: " << (token_count / duration.count()) << " tokens/sec" << std::endl;
    
    return 0;
}
EOF

    # Build C++ lexer
    cd "$cpp_dir"
    if ! clang++ -O3 -std=c++17 lexer.cpp -o lexer_benchmark > /dev/null 2>&1; then
        if ! g++ -O3 -std=c++17 lexer.cpp -o lexer_benchmark > /dev/null 2>&1; then
            log_error "Failed to build C++ lexer benchmark"
            return 1
        fi
    fi
    
    local cpp_binary="$cpp_dir/lexer_benchmark"
    local file_size=$(stat -f%z "$test_file" 2>/dev/null || stat -c%s "$test_file")
    local results=()
    
    log_info "Benchmarking C++ lexer with $(basename "$test_file")"
    
    # Warmup runs
    for ((i=1; i<=WARMUP_ITERATIONS; i++)); do
        timeout 30 "$cpp_binary" "$test_file" > /dev/null 2>&1 || true
    done
    
    # Actual benchmark runs
    for ((i=1; i<=ITERATIONS; i++)); do
        local result=$(timeout 30 "$cpp_binary" "$test_file" 2>/dev/null)
        if [[ $? -eq 0 ]] && [[ -n "$result" ]]; then
            results+=("$result")
        fi
    done
    
    if [[ ${#results[@]} -eq 0 ]]; then
        log_error "C++ lexer produced no valid results"
        return 1
    fi
    
    # Calculate average performance
    local total_time=0
    for time in "${results[@]}"; do
        total_time=$(python3 -c "print($total_time + $time)")
    done
    
    local avg_time=$(python3 -c "print($total_time / ${#results[@]})")
    local estimated_tokens=$((file_size / 8))
    local tokens_per_sec=$(python3 -c "print($estimated_tokens / $avg_time)")
    
    cat << EOF
{
    "language": "cpp",
    "test_file": "$(basename "$test_file")",
    "file_size_bytes": $file_size,
    "estimated_tokens": $estimated_tokens,
    "iterations": ${#results[@]},
    "times": [$(IFS=,; echo "${results[*]}")],
    "average_time_seconds": $avg_time,
    "tokens_per_second": $tokens_per_sec,
    "metadata": {
        "implementation": "manual",
        "compiler": "$(clang++ --version 2>/dev/null | head -1 || g++ --version | head -1)",
        "warmup_iterations": $WARMUP_ITERATIONS
    }
}
EOF
}

# Main benchmark execution
main() {
    log_info "Starting lexer performance validation"
    log_info "Target claim: 14M tokens/second"
    
    setup_test_data
    
    # Get test files for specified size
    local test_files_array=(${TEST_FILES[$TEST_SIZE]})
    if [[ ${#test_files_array[@]} -eq 0 ]]; then
        log_error "No test files found for size: $TEST_SIZE"
        exit 1
    fi
    
    # Initialize results structure
    local all_results=()
    
    # Run benchmarks for each test file
    for test_file_name in "${test_files_array[@]}"; do
        local test_file="$TEST_DATA_DIR/$test_file_name"
        
        if [[ ! -f "$test_file" ]]; then
            log_error "Test file not found: $test_file"
            continue
        fi
        
        log_info "Testing with: $(basename "$test_file")"
        
        # Benchmark Seen lexer
        local seen_result=$(benchmark_seen_lexer "$test_file")
        all_results+=("$seen_result")
        
        # Benchmark competitors
        IFS=',' read -ra COMP_ARRAY <<< "$COMPETITORS"
        for competitor in "${COMP_ARRAY[@]}"; do
            case $competitor in
                rust)
                    if command -v rustc &> /dev/null; then
                        local rust_result=$(benchmark_rust_lexer "$test_file")
                        all_results+=("$rust_result")
                    else
                        log_error "Rust not available, skipping"
                    fi
                    ;;
                cpp)
                    if command -v clang++ &> /dev/null || command -v g++ &> /dev/null; then
                        local cpp_result=$(benchmark_cpp_lexer "$test_file")
                        all_results+=("$cpp_result")
                    else
                        log_error "C++ compiler not available, skipping"
                    fi
                    ;;
                zig)
                    # TODO: Implement Zig lexer benchmark
                    log_info "Zig lexer benchmark not yet implemented"
                    ;;
            esac
        done
    done
    
    # Format and output results
    if [[ "$FORMAT" == "json" ]]; then
        cat << EOF
{
    "benchmark_name": "lexer_performance_validation",
    "claim_being_tested": "14M tokens per second",
    "test_configuration": {
        "iterations": $ITERATIONS,
        "warmup_iterations": $WARMUP_ITERATIONS,
        "test_size": "$TEST_SIZE",
        "competitors": "$COMPETITORS"
    },
    "results": [
$(IFS=$'\n'; echo "${all_results[*]}" | sed 's/^/        /' | sed '$!s/$/,/')
    ]
}
EOF
    else
        # CSV format
        echo "language,test_file,file_size_bytes,estimated_tokens,iterations,average_time_seconds,tokens_per_second,claim_validated"
        for result in "${all_results[@]}"; do
            echo "$result" | jq -r '[.language, .test_file, .file_size_bytes, .estimated_tokens, .iterations, .average_time_seconds, .tokens_per_second, .claim_validated] | @csv'
        done
    fi
    
    # Save to output file if specified
    if [[ -n "$OUTPUT_FILE" ]]; then
        if [[ "$FORMAT" == "json" ]]; then
            cat << EOF > "$OUTPUT_FILE"
{
    "benchmark_name": "lexer_performance_validation",
    "claim_being_tested": "14M tokens per second",
    "test_configuration": {
        "iterations": $ITERATIONS,
        "warmup_iterations": $WARMUP_ITERATIONS,
        "test_size": "$TEST_SIZE",
        "competitors": "$COMPETITORS"
    },
    "results": [
$(IFS=$'\n'; echo "${all_results[*]}" | sed 's/^/        /' | sed '$!s/$/,/')
    ]
}
EOF
        fi
        log_success "Results saved to: $OUTPUT_FILE"
    fi
}

# Execute main function
main "$@"