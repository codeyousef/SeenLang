#!/usr/bin/env pwsh
# Comprehensive test runner for Seen Language

param(
    [string]$Component = "all",
    [switch]$Coverage,
    [switch]$Watch,
    [switch]$Parallel,
    [string]$Filter = ""
)

Write-Host "Seen Language Test Runner" -ForegroundColor Green
Write-Host "=========================" -ForegroundColor Green

if ($Coverage) {
    Write-Host "Running tests with coverage..." -ForegroundColor Yellow
    if ($Component -eq "all") {
        cargo tarpaulin --workspace --timeout 120 --out Html --output-dir target/coverage
    } else {
        cargo tarpaulin --package seen_$Component --timeout 120 --out Html --output-dir target/coverage
    }
} elseif ($Watch) {
    Write-Host "Running tests in watch mode..." -ForegroundColor Yellow
    if ($Component -eq "all") {
        cargo watch -x "nextest run"
    } else {
        cargo watch -x "nextest run --package seen_$Component"
    }
} elseif ($Parallel) {
    Write-Host "Running tests in parallel..." -ForegroundColor Yellow
    if ($Component -eq "all") {
        cargo nextest run --profile default
    } else {
        cargo nextest run --package seen_$Component --profile default
    }
} else {
    Write-Host "Running standard tests..." -ForegroundColor Yellow
    if ($Component -eq "all") {
        if ($Filter -ne "") {
            cargo test $Filter
        } else {
            cargo test
        }
    } else {
        if ($Filter -ne "") {
            cargo test --package seen_$Component $Filter
        } else {
            cargo test --package seen_$Component
        }
    }
}

Write-Host "Test execution completed." -ForegroundColor Green