#!/usr/bin/env pwsh
# Quality gates for Seen Language development

Write-Host "Running Quality Gates" -ForegroundColor Green
Write-Host "=====================" -ForegroundColor Green

$exitCode = 0

# Run tests
Write-Host "1. Running test suite..." -ForegroundColor Yellow
cargo nextest run --profile ci
if ($LASTEXITCODE -ne 0) {
    Write-Host "❌ Tests failed" -ForegroundColor Red
    $exitCode = 1
} else {
    Write-Host "✅ Tests passed" -ForegroundColor Green
}

# Check formatting
Write-Host "2. Checking code formatting..." -ForegroundColor Yellow
cargo fmt --all -- --check
if ($LASTEXITCODE -ne 0) {
    Write-Host "❌ Code formatting check failed" -ForegroundColor Red
    $exitCode = 1
} else {
    Write-Host "✅ Code formatting check passed" -ForegroundColor Green
}

# Run clippy
Write-Host "3. Running clippy lints..." -ForegroundColor Yellow
cargo clippy --all-targets --all-features -- -D warnings
if ($LASTEXITCODE -ne 0) {
    Write-Host "❌ Clippy lints failed" -ForegroundColor Red
    $exitCode = 1
} else {
    Write-Host "✅ Clippy lints passed" -ForegroundColor Green
}

# Check for forbidden patterns
Write-Host "4. Checking for forbidden patterns..." -ForegroundColor Yellow
$forbiddenPatterns = @("TODO", "FIXME", "panic!", "unimplemented!", "unreachable!")
$foundForbidden = $false

foreach ($pattern in $forbiddenPatterns) {
    $matches = Select-String -Path "*/src/**/*.rs" -Pattern $pattern -Exclude "*test*" 2>$null
    if ($matches) {
        Write-Host "❌ Found forbidden pattern '$pattern':" -ForegroundColor Red
        $matches | ForEach-Object { Write-Host "  $_" -ForegroundColor Red }
        $foundForbidden = $true
    }
}

if ($foundForbidden) {
    $exitCode = 1
} else {
    Write-Host "✅ No forbidden patterns found" -ForegroundColor Green
}

if ($exitCode -eq 0) {
    Write-Host "🎉 All quality gates passed!" -ForegroundColor Green
} else {
    Write-Host "💥 Some quality gates failed!" -ForegroundColor Red
}

exit $exitCode