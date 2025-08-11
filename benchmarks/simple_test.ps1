# Simple test script to verify Seen compiler and benchmarks work
param(
    [switch]$Verbose
)

Write-Host "=== Seen Benchmark System Test ===" -ForegroundColor Cyan
Write-Host ""

# Check Seen compiler
$SeenExe = Join-Path (Split-Path -Parent $PSScriptRoot) "target\release\seen.exe"
if (!(Test-Path $SeenExe)) {
    $SeenExe = Join-Path (Split-Path -Parent $PSScriptRoot) "target\debug\seen.exe"
}

if (Test-Path $SeenExe) {
    Write-Host "[OK] Found Seen compiler: $SeenExe" -ForegroundColor Green
    
    # Test running a simple Seen file
    $TestFile = Join-Path (Split-Path -Parent $PSScriptRoot) "compiler_seen\simple_test.seen"
    if (Test-Path $TestFile) {
        Write-Host "[OK] Found test file: $TestFile" -ForegroundColor Green
        Write-Host ""
        Write-Host "Running test file..." -ForegroundColor Yellow
        & $SeenExe run $TestFile
        if ($LASTEXITCODE -eq 0) {
            Write-Host "[OK] Test file executed successfully" -ForegroundColor Green
        } else {
            Write-Host "[WARNING] Test file execution failed with code: $LASTEXITCODE" -ForegroundColor Yellow
        }
    } else {
        Write-Host "[WARNING] Test file not found: $TestFile" -ForegroundColor Yellow
    }
} else {
    Write-Host "[ERROR] Seen compiler not found!" -ForegroundColor Red
    Write-Host "Expected at: $SeenExe" -ForegroundColor Yellow
}

Write-Host ""

# Test Rust competitor build
Write-Host "Testing Rust competitor build..." -ForegroundColor Cyan
$RustDir = Join-Path $PSScriptRoot "competitors\rust"
if (Test-Path (Join-Path $RustDir "Cargo.toml")) {
    Write-Host "[OK] Cargo.toml found" -ForegroundColor Green
    
    Push-Location $RustDir
    if (Get-Command cargo -ErrorAction SilentlyContinue) {
        Write-Host "Building Rust benchmark..." -ForegroundColor Yellow
        cargo build --release 2>&1 | Out-String | Write-Host
        if (Test-Path "target\release\arithmetic_bench.exe") {
            Write-Host "[OK] Rust benchmark built successfully" -ForegroundColor Green
            
            # Run the benchmark
            Write-Host "Running Rust benchmark..." -ForegroundColor Yellow
            .\target\release\arithmetic_bench.exe
        } else {
            Write-Host "[ERROR] Failed to build Rust benchmark" -ForegroundColor Red
        }
    } else {
        Write-Host "[WARNING] Cargo not found - install Rust from https://rustup.rs/" -ForegroundColor Yellow
    }
    Pop-Location
} else {
    Write-Host "[ERROR] Cargo.toml not found in $RustDir" -ForegroundColor Red
}

Write-Host ""
Write-Host "=== Test Complete ===" -ForegroundColor Cyan