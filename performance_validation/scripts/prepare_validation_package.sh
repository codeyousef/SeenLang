#!/usr/bin/env bash
# Create a package that others can use to validate Seen performance claims
# This ensures third-party reproducibility and independent validation

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PERF_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
PROJECT_ROOT="$(cd "$PERF_ROOT/.." && pwd)"
PACKAGE_DIR="$PROJECT_ROOT/seen-validation-package"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1" >&2; }
log_warning() { echo -e "${YELLOW}[WARNING]${NC} $1"; }

log_info "Creating third-party validation package for Seen Language..."

# Clean and create package directory
if [[ -d "$PACKAGE_DIR" ]]; then
    rm -rf "$PACKAGE_DIR"
fi
mkdir -p "$PACKAGE_DIR"

# Create Dockerfile for reproducible validation environment
log_info "Creating Docker environment for reproducibility..."
cat > "$PACKAGE_DIR/Dockerfile.validation" << 'EOF'
FROM ubuntu:22.04

# Install system dependencies
RUN apt-get update && apt-get install -y \
    build-essential \
    curl \
    git \
    python3 \
    python3-pip \
    valgrind \
    linux-perf \
    bc \
    jq \
    wget \
    tar \
    && rm -rf /var/lib/apt/lists/*

# Install Python packages for statistical analysis
RUN python3 -m pip install --upgrade pip numpy scipy matplotlib pandas seaborn

# Install Rust toolchain
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

# Install Clang
RUN apt-get update && apt-get install -y clang-15 && rm -rf /var/lib/apt/lists/*

# Install Zig
RUN cd /tmp && \
    wget -q https://ziglang.org/download/0.11.0/zig-linux-x86_64-0.11.0.tar.xz && \
    tar xf zig-linux-x86_64-0.11.0.tar.xz && \
    mv zig-linux-x86_64-0.11.0 /opt/zig && \
    rm zig-linux-x86_64-0.11.0.tar.xz
ENV PATH="/opt/zig:${PATH}"

# Set working directory
WORKDIR /opt/benchmarks

# Copy benchmark suite (will be mounted or copied by validation script)
COPY . /opt/benchmarks/

# Build Seen compiler
RUN if [ -f "Cargo.toml" ]; then \
        cargo build --release; \
    else \
        echo "Warning: No Cargo.toml found, Seen compiler build skipped"; \
    fi

# Make scripts executable
RUN find . -name "*.sh" -type f -exec chmod +x {} \;

# Set performance governor for consistent benchmarking
RUN echo 'echo performance | tee /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor || true' > /etc/rc.local && \
    chmod +x /etc/rc.local

# Default command
CMD ["bash"]
EOF

# Create one-command validation script
log_info "Creating one-command validation script..."
cat > "$PACKAGE_DIR/validate_seen_performance.sh" << 'EOF'
#!/usr/bin/env bash
# Seen Language Performance Validation Script
# One-command validation of all performance claims

set -e

echo "==================================="
echo "Seen Performance Validation Suite"
echo "==================================="
echo "This script validates Seen's performance claims independently"
echo "Results will be completely honest - no cherry-picking allowed"
echo ""

# Configuration
VALIDATION_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RESULTS_DIR="$VALIDATION_DIR/validation_results"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
SESSION_DIR="$RESULTS_DIR/$TIMESTAMP"

# Default parameters
USE_DOCKER=true
QUICK_MODE=false
ITERATIONS=30
COMPETITORS="cpp,rust,zig"

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --no-docker)
            USE_DOCKER=false
            shift
            ;;
        --quick)
            QUICK_MODE=true
            ITERATIONS=10
            shift
            ;;
        --iterations)
            ITERATIONS="$2"
            shift 2
            ;;
        --competitors)
            COMPETITORS="$2"
            shift 2
            ;;
        --help)
            cat << 'HELP_EOF'
Seen Performance Validation

Usage: $0 [OPTIONS]

OPTIONS:
    --no-docker         Use native environment instead of Docker
    --quick             Quick test mode (10 iterations instead of 30)
    --iterations N      Number of benchmark iterations
    --competitors LIST  Comma-separated competitors to test against
    --help              Show this help

EXAMPLES:
    ./validate_seen_performance.sh                    # Full validation
    ./validate_seen_performance.sh --quick            # Quick test
    ./validate_seen_performance.sh --no-docker        # Use native tools
    ./validate_seen_performance.sh --competitors rust # Only test vs Rust
HELP_EOF
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

# Create results directory
mkdir -p "$SESSION_DIR"

if [[ "$USE_DOCKER" == "true" ]]; then
    echo "Building Docker validation environment..."
    
    # Check if Docker is available
    if ! command -v docker &> /dev/null; then
        echo "ERROR: Docker is required but not found. Install Docker or use --no-docker"
        exit 1
    fi
    
    # Build validation image
    docker build -t seen-validation -f Dockerfile.validation . || {
        echo "ERROR: Failed to build Docker validation environment"
        exit 1
    }
    
    echo "Running benchmarks in containerized environment..."
    
    # Run validation in container with security options for profiling
    docker run --rm \
        --security-opt seccomp=unconfined \
        --cap-add SYS_ADMIN \
        --volume "$SESSION_DIR:/opt/benchmarks/results" \
        --env ITERATIONS="$ITERATIONS" \
        --env COMPETITORS="$COMPETITORS" \
        seen-validation \
        bash -c "
            echo 'Starting validation in container...'
            cd /opt/benchmarks
            if [[ -f 'performance_validation/scripts/run_all.sh' ]]; then
                ./performance_validation/scripts/run_all.sh \
                    --iterations $ITERATIONS \
                    --competitors $COMPETITORS \
                    --timeout 600
            else
                echo 'ERROR: Benchmark suite not found'
                exit 1
            fi
        "
    
else
    echo "Running benchmarks in native environment..."
    
    # Check for required tools
    missing_tools=()
    [[ ! $(command -v cargo) ]] && missing_tools+=("rust")
    [[ ! $(command -v clang) ]] && missing_tools+=("clang")
    [[ ! $(command -v python3) ]] && missing_tools+=("python3")
    
    if [[ ${#missing_tools[@]} -gt 0 ]]; then
        echo "ERROR: Missing required tools: ${missing_tools[*]}"
        echo "Install missing tools or use Docker mode"
        exit 1
    fi
    
    # Run benchmarks natively
    cd "$VALIDATION_DIR"
    if [[ -f "performance_validation/scripts/run_all.sh" ]]; then
        ./performance_validation/scripts/run_all.sh \
            --iterations "$ITERATIONS" \
            --competitors "$COMPETITORS" \
            --timeout 600 \
            2>&1 | tee "$SESSION_DIR/benchmark_log.txt"
    else
        echo "ERROR: Benchmark suite not found in package"
        exit 1
    fi
fi

# Generate final report
echo ""
echo "Generating honest performance report..."

if [[ -f "performance_validation/scripts/generate_honest_report.py" ]]; then
    python3 performance_validation/scripts/generate_honest_report.py \
        --results-dir "$SESSION_DIR" \
        --output "$SESSION_DIR/validation_report.html" \
        --format html
    
    echo ""
    echo "======================================"
    echo "VALIDATION COMPLETE"
    echo "======================================"
    echo "Results location: $SESSION_DIR"
    echo "Report: $SESSION_DIR/validation_report.html"
    echo ""
    echo "Key files:"
    echo "  - validation_report.html (main report)"
    echo "  - benchmark_log.txt (detailed logs)"
    echo "  - *.json (raw benchmark data)"
    echo ""
    echo "IMPORTANT:"
    echo "These results are independent and unbiased."
    echo "Report any discrepancies with published claims to:"
    echo "https://github.com/seen-lang/performance-validation/issues"
    
else
    echo "WARNING: Report generator not found, showing raw results only"
    echo "Results saved to: $SESSION_DIR"
fi

echo ""
echo "Thank you for validating Seen's performance claims!"
EOF

# Create comprehensive README for validation
log_info "Creating validation README..."
cat > "$PACKAGE_DIR/README_VALIDATION.md" << 'EOF'
# Seen Language Performance Validation Package

This package enables **independent validation** of Seen's performance claims by third parties.

## 🎯 Purpose

Validate these specific claims:
- ✅ "14M tokens/second lexer"
- ❓ "Faster than Rust/C++/Zig runtime" 
- ❌ "-58% memory overhead" (investigating this impossible claim)
- ✅ "Zero-cost reactive abstractions"
- ✅ "Faster compilation than C++"

## 🚀 Quick Start

### Option 1: One-Command Validation (Recommended)
```bash
# Full validation (takes ~2 hours)
./validate_seen_performance.sh

# Quick test (takes ~30 minutes)
./validate_seen_performance.sh --quick

# Test specific competitors only
./validate_seen_performance.sh --competitors "rust,cpp"
```

### Option 2: Manual Validation
```bash
# Using Docker (recommended for reproducibility)
docker build -t seen-validation -f Dockerfile.validation .
docker run -it seen-validation

# Using native tools
cd performance_validation/
./scripts/run_all.sh --iterations 30
```

## 📋 What Gets Tested

### 1. Lexer Performance
- Real-world codebases (100KB+ files)
- Unicode handling
- Memory usage during lexing
- Scalability on large files (1MB+)

### 2. Memory Management
- Allocation patterns vs C malloc/free
- Fragmentation analysis
- Peak vs average memory usage
- **Investigation of impossible "-58%" claim**

### 3. Real-World Algorithms
- Binary trees (allocation stress test)
- JSON parsing (real API responses)
- HTTP server (concurrency test)
- Ray tracing (compute intensive)
- Spectral norm (numerical computation)

### 4. Reactive Programming
- Overhead vs manual loops
- Complex operation chains
- Memory usage comparison
- Async/await performance

### 5. Compilation Speed
- Projects from 10 to 100,000 lines
- Generic-heavy code
- Parser-heavy code
- Cold vs warm compilation

## 📊 Statistical Rigor

- **Minimum 30 iterations** per benchmark
- **Outlier removal** using IQR method
- **T-tests** for significance (p < 0.05)
- **Effect sizes** (Cohen's d)
- **95% confidence intervals**
- **Multiple comparison corrections**

## 🔧 Requirements

### Docker Mode (Recommended)
- Docker 20.10+
- 8GB RAM minimum
- 20GB free disk space

### Native Mode
- Ubuntu 20.04+ / macOS 10.15+ / Windows 10+
- Rust 1.70+
- Clang 10+
- Zig 0.11+
- Python 3.8+ with scipy, numpy, matplotlib
- 16GB RAM recommended

## 📈 Understanding Results

### Performance Claims Validation

#### ✅ CLAIM VALIDATED
- Performance meets or exceeds stated claims
- Statistical significance (p < 0.05)
- Consistent across multiple test runs

#### ⚠️ CLAIM PARTIALLY MET  
- Performance close but not quite meeting claims
- May depend on specific conditions
- Further investigation needed

#### ❌ CLAIM NOT MET
- Performance significantly below claims
- Statistically significant underperformance
- Clear evidence against the claim

### Example Output
```
=== LEXER PERFORMANCE ===
Claim: "14M tokens/second"
Result: 8.2M tokens/second (±0.5M, 95% CI)
Status: ❌ CLAIM NOT MET
Recommendation: Adjust claim to "8M tokens/second"

=== MEMORY OVERHEAD ===
Claim: "-58% memory overhead"  
Result: +12% memory overhead vs C malloc
Status: ❌ CLAIM IMPOSSIBLE
Note: Negative overhead is mathematically impossible
```

## 🐛 Reporting Issues

If results don't match published claims:

1. **Create GitHub issue** with:
   - Your system specifications
   - Complete benchmark output
   - `validation_report.html` file
   - Docker/native environment details

2. **Include evidence**:
   - Screenshots of key results
   - Raw benchmark data files
   - System information

3. **Be constructive**:
   - Focus on data, not opinions
   - Suggest realistic performance targets
   - Consider environmental factors

## 🔬 Scientific Standards

This validation follows these principles:

- **Reproducibility**: Anyone can run the same tests
- **Transparency**: All code and data are open
- **Honesty**: Report ALL results, including negative ones
- **Rigor**: Proper statistical analysis with multiple samples
- **Fairness**: Same optimization levels for all languages

## 📝 Example Validation Session

```bash
$ ./validate_seen_performance.sh --quick

===================================
Seen Performance Validation Suite
===================================

Building Docker validation environment...
Running benchmarks in containerized environment...

=== LEXER VALIDATION ===
Testing 4 real-world codebases...
✅ large_codebase.seen: 9.2M tokens/sec
✅ minified_code.seen: 12.1M tokens/sec  
✅ sparse_code.seen: 15.8M tokens/sec
⚠️  unicode_heavy.seen: 6.3M tokens/sec

Overall: 10.9M tokens/sec (target: 14M)
Status: ❌ CLAIM NOT MET (78% of target)

=== MEMORY VALIDATION ===
Testing allocation patterns...
Result: +15.3% overhead vs C malloc
Status: ❌ CLAIM IMPOSSIBLE (-58% is impossible)

=== REACTIVE VALIDATION ===
Testing zero-cost abstractions...
Overhead: 3.2% vs manual loops
Status: ✅ CLAIM VALIDATED (< 5% threshold)

======================================
VALIDATION COMPLETE
======================================
Results: validation_results/20240809_143022/
Report: validation_report.html

Key Findings:
✅ 1 claim validated (reactive abstractions)
⚠️  0 claims partially met
❌ 2 claims not met (lexer speed, memory overhead)

Recommendation: Update performance claims based on evidence
```

## 📚 Additional Resources

- [Benchmark Suite Documentation](performance_validation/README.md)
- [Statistical Analysis Methods](performance_validation/docs/statistical_methods.md)
- [Contributing Guidelines](CONTRIBUTING.md)
- [Issue Tracker](https://github.com/seen-lang/performance-validation/issues)

---

**Remember**: The goal is honest performance validation, not marketing. Realistic performance claims build trust and help the language improve over time.
EOF

# Create installer script for Seen
log_info "Creating Seen installer script..."
cat > "$PACKAGE_DIR/install_seen.sh" << 'EOF'
#!/usr/bin/env bash
# Seen Language Installer for Validation Package

set -e

INSTALL_DIR="/usr/local/bin"
REPO_URL="https://github.com/seen-lang/seenlang.git"
BUILD_DIR="/tmp/seen-build"

echo "Installing Seen Language..."

# Check if running in container
if [[ -f /.dockerenv ]]; then
    echo "Detected container environment"
    INSTALL_DIR="/opt/seen/bin"
    mkdir -p "$INSTALL_DIR"
fi

# Install Rust if not available
if ! command -v cargo &> /dev/null; then
    echo "Installing Rust toolchain..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source ~/.cargo/env
fi

# Clone and build Seen
echo "Cloning Seen repository..."
rm -rf "$BUILD_DIR"
git clone "$REPO_URL" "$BUILD_DIR"

cd "$BUILD_DIR"

echo "Building Seen compiler (this may take a few minutes)..."
cargo build --release

# Install binary
if [[ -f "target/release/seen" ]]; then
    cp "target/release/seen" "$INSTALL_DIR/"
    chmod +x "$INSTALL_DIR/seen"
    echo "✅ Seen installed to $INSTALL_DIR/seen"
elif [[ -f "target/release/seen.exe" ]]; then
    cp "target/release/seen.exe" "$INSTALL_DIR/"
    chmod +x "$INSTALL_DIR/seen.exe"
    echo "✅ Seen installed to $INSTALL_DIR/seen.exe"
else
    echo "❌ Build failed - seen binary not found"
    exit 1
fi

# Add to PATH if not in container
if [[ ! -f /.dockerenv ]] && [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
    echo "export PATH=\"$INSTALL_DIR:\$PATH\"" >> ~/.bashrc
    echo "Added $INSTALL_DIR to PATH (restart shell or run: source ~/.bashrc)"
fi

echo "Testing Seen installation..."
if command -v seen &> /dev/null || [[ -x "$INSTALL_DIR/seen" ]]; then
    "$INSTALL_DIR/seen" --version || echo "Seen binary found but version check failed"
    echo "✅ Seen installation complete!"
else
    echo "⚠️  Installation may have issues - manual verification needed"
fi

# Cleanup
cd /
rm -rf "$BUILD_DIR"
EOF

# Copy the complete performance validation suite
log_info "Copying performance validation suite..."
cp -r "$PERF_ROOT" "$PACKAGE_DIR/"

# Make scripts executable
find "$PACKAGE_DIR" -name "*.sh" -type f -exec chmod +x {} \;

# Create archive
log_info "Creating validation package archive..."
cd "$PROJECT_ROOT"
tar -czf seen-validation-package.tar.gz seen-validation-package/

# Calculate archive size
ARCHIVE_SIZE=$(du -sh seen-validation-package.tar.gz | cut -f1)

log_success "Third-party validation package created successfully!"
echo ""
echo "Package contents:"
echo "  📁 seen-validation-package/"
echo "  🐳 Dockerfile.validation        - Reproducible Docker environment"
echo "  🚀 validate_seen_performance.sh - One-command validation"  
echo "  📖 README_VALIDATION.md         - Complete documentation"
echo "  ⚙️  install_seen.sh             - Seen compiler installer"
echo "  📊 performance_validation/      - Complete benchmark suite"
echo ""
echo "📦 Archive: seen-validation-package.tar.gz ($ARCHIVE_SIZE)"
echo ""
echo "Usage for third parties:"
echo "  1. wget https://github.com/seen-lang/releases/seen-validation-package.tar.gz"
echo "  2. tar -xzf seen-validation-package.tar.gz"
echo "  3. cd seen-validation-package"
echo "  4. ./validate_seen_performance.sh"
echo ""
echo "The validation package is ready for independent third-party verification!"