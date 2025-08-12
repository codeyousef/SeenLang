# TDD Infrastructure Setup Script for Seen Language
# This script sets up comprehensive testing infrastructure with coverage reporting

Write-Host "Setting up TDD Infrastructure for Seen Language" -ForegroundColor Green
Write-Host "=================================================" -ForegroundColor Green

# Check if Rust is installed
if (!(Get-Command "cargo" -ErrorAction SilentlyContinue)) {
    Write-Host "Error: Rust/Cargo not found. Please install Rust first." -ForegroundColor Red
    exit 1
}

# Install required tools for testing and coverage
Write-Host "Installing testing and coverage tools..." -ForegroundColor Yellow

# Install cargo-tarpaulin for coverage
cargo install cargo-tarpaulin

# Install cargo-nextest for faster test execution
cargo install cargo-nextest

# Install cargo-watch for continuous testing
cargo install cargo-watch

# Install cargo-audit for security auditing
cargo install cargo-audit

# Install cargo-deny for dependency checking
cargo install cargo-deny

# Install cargo-outdated for dependency updates
cargo install cargo-outdated

# Create test configuration files
Write-Host "Creating test configuration files..." -ForegroundColor Yellow

# Create nextest configuration
$nextestConfig = @"
[profile.default]
retries = 2
threads = "num-cpus"
test-threads = "num-cpus"

[profile.ci]
retries = 3
threads = 1
test-threads = 1
slow-timeout = { period = "60s", terminate-after = 2 }

[profile.coverage]
retries = 0
threads = 1
test-threads = 1
"@

New-Item -Path ".config" -ItemType Directory -Force | Out-Null
$nextestConfig | Out-File -FilePath ".config/nextest.toml" -Encoding UTF8

# Create tarpaulin configuration
$tarpaulinConfig = @"
[tool.tarpaulin]
exclude-files = [
    "*/tests/*",
    "*/benches/*",
    "*/examples/*",
    "target/*"
]
ignore-panics = true
ignore-tests = true
out = ["Html", "Xml", "Lcov"]
output-dir = "target/coverage"
run-types = ["Tests", "Doctests"]
timeout = 120
"@

$tarpaulinConfig | Out-File -FilePath "tarpaulin.toml" -Encoding UTF8

# Create GitHub Actions workflow for CI
Write-Host "Creating CI/CD configuration..." -ForegroundColor Yellow

$ciWorkflow = @"
name: CI

on:
  push:
    branches: [ main, develop ]
  pull_request:
    branches: [ main, develop ]

env:
  CARGO_TERM_COLOR: always
  RUST_BACKTRACE: 1

jobs:
  test:
    name: Test Suite
    runs-on: `${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, windows-latest, macos-latest]
        rust: [stable, beta]
    
    steps:
    - uses: actions/checkout@v4
    
    - name: Install Rust
      uses: dtolnay/rust-toolchain@master
      with:
        toolchain: `${{ matrix.rust }}
        components: rustfmt, clippy
    
    - name: Cache dependencies
      uses: actions/cache@v4
      with:
        path: |
          ~/.cargo/registry
          ~/.cargo/git
          target
        key: `${{ runner.os }}-cargo-`${{ hashFiles('**/Cargo.lock') }}
    
    - name: Install nextest
      uses: taiki-e/install-action@nextest
    
    - name: Run tests
      run: cargo nextest run --profile ci
    
    - name: Run doctests
      run: cargo test --doc
    
    - name: Check formatting
      run: cargo fmt --all -- --check
    
    - name: Run clippy
      run: cargo clippy --all-targets --all-features -- -D warnings

  coverage:
    name: Coverage
    runs-on: ubuntu-latest
    
    steps:
    - uses: actions/checkout@v4
    
    - name: Install Rust
      uses: dtolnay/rust-toolchain@stable
    
    - name: Install tarpaulin
      uses: taiki-e/install-action@cargo-tarpaulin
    
    - name: Generate coverage
      run: cargo tarpaulin --verbose --workspace --timeout 120 --out xml
    
    - name: Upload coverage to Codecov
      uses: codecov/codecov-action@v4
      with:
        file: ./cobertura.xml
        fail_ci_if_error: true

  audit:
    name: Security Audit
    runs-on: ubuntu-latest
    
    steps:
    - uses: actions/checkout@v4
    
    - name: Install Rust
      uses: dtolnay/rust-toolchain@stable
    
    - name: Install cargo-audit
      uses: taiki-e/install-action@cargo-audit
    
    - name: Run audit
      run: cargo audit
"@

New-Item -Path ".github/workflows" -ItemType Directory -Force | Out-Null
$ciWorkflow | Out-File -FilePath ".github/workflows/ci.yml" -Encoding UTF8

# Create test runner scripts
Write-Host "Creating test runner scripts..." -ForegroundColor Yellow

$testRunnerScript = @"
#!/usr/bin/env pwsh
# Comprehensive test runner for Seen Language

param(
    [string]`$Component = "all",
    [switch]`$Coverage,
    [switch]`$Watch,
    [switch]`$Parallel,
    [string]`$Filter = ""
)

Write-Host "Seen Language Test Runner" -ForegroundColor Green
Write-Host "=========================" -ForegroundColor Green

if (`$Coverage) {
    Write-Host "Running tests with coverage..." -ForegroundColor Yellow
    if (`$Component -eq "all") {
        cargo tarpaulin --workspace --timeout 120 --out Html --output-dir target/coverage
    } else {
        cargo tarpaulin --package seen_`$Component --timeout 120 --out Html --output-dir target/coverage
    }
} elseif (`$Watch) {
    Write-Host "Running tests in watch mode..." -ForegroundColor Yellow
    if (`$Component -eq "all") {
        cargo watch -x "nextest run"
    } else {
        cargo watch -x "nextest run --package seen_`$Component"
    }
} elseif (`$Parallel) {
    Write-Host "Running tests in parallel..." -ForegroundColor Yellow
    if (`$Component -eq "all") {
        cargo nextest run --profile default
    } else {
        cargo nextest run --package seen_`$Component --profile default
    }
} else {
    Write-Host "Running standard tests..." -ForegroundColor Yellow
    if (`$Component -eq "all") {
        if (`$Filter -ne "") {
            cargo test `$Filter
        } else {
            cargo test
        }
    } else {
        if (`$Filter -ne "") {
            cargo test --package seen_`$Component `$Filter
        } else {
            cargo test --package seen_`$Component
        }
    }
}

Write-Host "Test execution completed." -ForegroundColor Green
"@

$testRunnerScript | Out-File -FilePath "scripts/run_tests.ps1" -Encoding UTF8

# Create quality gates script
$qualityGatesScript = @"
#!/usr/bin/env pwsh
# Quality gates for Seen Language development

Write-Host "Running Quality Gates" -ForegroundColor Green
Write-Host "=====================" -ForegroundColor Green

`$exitCode = 0

# Run tests
Write-Host "1. Running test suite..." -ForegroundColor Yellow
cargo nextest run --profile ci
if (`$LASTEXITCODE -ne 0) {
    Write-Host "❌ Tests failed" -ForegroundColor Red
    `$exitCode = 1
} else {
    Write-Host "✅ Tests passed" -ForegroundColor Green
}

# Check formatting
Write-Host "2. Checking code formatting..." -ForegroundColor Yellow
cargo fmt --all -- --check
if (`$LASTEXITCODE -ne 0) {
    Write-Host "❌ Code formatting check failed" -ForegroundColor Red
    `$exitCode = 1
} else {
    Write-Host "✅ Code formatting check passed" -ForegroundColor Green
}

# Run clippy
Write-Host "3. Running clippy lints..." -ForegroundColor Yellow
cargo clippy --all-targets --all-features -- -D warnings
if (`$LASTEXITCODE -ne 0) {
    Write-Host "❌ Clippy lints failed" -ForegroundColor Red
    `$exitCode = 1
} else {
    Write-Host "✅ Clippy lints passed" -ForegroundColor Green
}

# Run security audit
Write-Host "4. Running security audit..." -ForegroundColor Yellow
cargo audit
if (`$LASTEXITCODE -ne 0) {
    Write-Host "❌ Security audit failed" -ForegroundColor Red
    `$exitCode = 1
} else {
    Write-Host "✅ Security audit passed" -ForegroundColor Green
}

# Check for TODO/FIXME/panic! in production code
Write-Host "5. Checking for forbidden patterns..." -ForegroundColor Yellow
`$forbiddenPatterns = @("TODO", "FIXME", "panic!", "unimplemented!", "unreachable!")
`$foundForbidden = `$false

foreach (`$pattern in `$forbiddenPatterns) {
    `$matches = Select-String -Path "*/src/**/*.rs" -Pattern `$pattern -Exclude "*test*" 2>$null
    if (`$matches) {
        Write-Host "❌ Found forbidden pattern '`$pattern':" -ForegroundColor Red
        `$matches | ForEach-Object { Write-Host "  `$_" -ForegroundColor Red }
        `$foundForbidden = `$true
    }
}

if (`$foundForbidden) {
    `$exitCode = 1
} else {
    Write-Host "✅ No forbidden patterns found" -ForegroundColor Green
}

# Generate coverage report
Write-Host "6. Generating coverage report..." -ForegroundColor Yellow
cargo tarpaulin --workspace --timeout 120 --out Html --output-dir target/coverage --ignore-tests
if (`$LASTEXITCODE -ne 0) {
    Write-Host "❌ Coverage generation failed" -ForegroundColor Red
    `$exitCode = 1
} else {
    Write-Host "✅ Coverage report generated" -ForegroundColor Green
    Write-Host "Coverage report available at: target/coverage/tarpaulin-report.html" -ForegroundColor Cyan
}

if (`$exitCode -eq 0) {
    Write-Host "🎉 All quality gates passed!" -ForegroundColor Green
} else {
    Write-Host "💥 Some quality gates failed!" -ForegroundColor Red
}

exit `$exitCode
"@

$qualityGatesScript | Out-File -FilePath "scripts/quality_gates.ps1" -Encoding UTF8

# Create benchmark runner
$benchmarkScript = @"
#!/usr/bin/env pwsh
# Benchmark runner for Seen Language

Write-Host "Seen Language Benchmark Runner" -ForegroundColor Green
Write-Host "==============================" -ForegroundColor Green

# Run criterion benchmarks
Write-Host "Running performance benchmarks..." -ForegroundColor Yellow
cargo bench

# Generate benchmark report
Write-Host "Benchmark results available at: target/criterion/report/index.html" -ForegroundColor Cyan
"@

$benchmarkScript | Out-File -FilePath "scripts/run_benchmarks.ps1" -Encoding UTF8

Write-Host "TDD Infrastructure setup completed!" -ForegroundColor Green
Write-Host "" -ForegroundColor White
Write-Host "Available commands:" -ForegroundColor Cyan
Write-Host "  ./scripts/run_tests.ps1                    - Run all tests" -ForegroundColor White
Write-Host "  ./scripts/run_tests.ps1 -Coverage          - Run tests with coverage" -ForegroundColor White
Write-Host "  ./scripts/run_tests.ps1 -Watch             - Run tests in watch mode" -ForegroundColor White
Write-Host "  ./scripts/run_tests.ps1 -Component lexer   - Run tests for specific component" -ForegroundColor White
Write-Host "  ./scripts/quality_gates.ps1                - Run all quality checks" -ForegroundColor White
Write-Host "  ./scripts/run_benchmarks.ps1               - Run performance benchmarks" -ForegroundColor White
Write-Host "" -ForegroundColor White
Write-Host "Next steps:" -ForegroundColor Yellow
Write-Host "1. Run 'cargo build' to verify workspace setup" -ForegroundColor White
Write-Host "2. Run './scripts/run_tests.ps1' to execute initial tests" -ForegroundColor White
Write-Host "3. Start implementing features using TDD methodology" -ForegroundColor White
"@

$setupScript | Out-File -FilePath "scripts/setup_tdd_infrastructure.ps1" -Encoding UTF8