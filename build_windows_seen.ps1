# Build Windows Seen Compiler
# Run this from Windows PowerShell

Write-Host "Building Windows Seen Compiler..." -ForegroundColor Green

# Check if we have gcc available through WSL or Windows
$gccPath = ""
if (Get-Command gcc -ErrorAction SilentlyContinue) {
    $gccPath = "gcc"
} elseif (Get-Command wsl -ErrorAction SilentlyContinue) {
    $gccPath = "wsl gcc"
} else {
    Write-Host "Error: GCC not found. Please install either:" -ForegroundColor Red
    Write-Host "1. MinGW-w64 for Windows" -ForegroundColor Yellow
    Write-Host "2. Or use WSL with gcc installed" -ForegroundColor Yellow
    exit 1
}

# Compile the bootstrap C code
Write-Host "Compiling bootstrap C code..." -ForegroundColor Cyan

if ($gccPath -eq "wsl gcc") {
    wsl gcc -o /mnt/d/Projects/Rust/seenlang/seen_compiler.exe /mnt/d/Projects/Rust/seenlang/bootstrap_main.c
} else {
    & $gccPath -o seen_compiler.exe bootstrap_main.c
}

if (Test-Path "seen_compiler.exe") {
    Write-Host "✅ Windows compiler built successfully!" -ForegroundColor Green
    Write-Host "Testing compiler..." -ForegroundColor Cyan
    .\seen_compiler.exe --version
} else {
    Write-Host "❌ Failed to build Windows compiler" -ForegroundColor Red
}

Write-Host "Build complete!" -ForegroundColor Green