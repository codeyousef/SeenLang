# Install LLVM for Seen Language on Windows
# LLVM is required for the Seen compiler to generate native binaries

param(
    [string]$Version = "20.1.0",
    [switch]$Help
)

if ($Help) {
    Write-Host "Install LLVM for Seen Language"
    Write-Host ""
    Write-Host "Usage: .\install-llvm.ps1 [-Version <version>]"
    Write-Host ""
    Write-Host "This script installs LLVM using winget (preferred) or provides"
    Write-Host "manual installation instructions."
    exit 0
}

Write-Host "Seen Language - LLVM Installer" -ForegroundColor Cyan
Write-Host ""

# Check if LLVM is already installed
$clang = Get-Command clang -ErrorAction SilentlyContinue
if ($clang) {
    $version = & clang --version 2>&1 | Select-Object -First 1
    Write-Host "LLVM is already installed: $version" -ForegroundColor Green

    # Check version
    $opt = Get-Command opt -ErrorAction SilentlyContinue
    $lld = Get-Command lld-link -ErrorAction SilentlyContinue

    if (-not $opt) {
        Write-Host "Warning: 'opt' not found in PATH. Full LLVM installation may be needed." -ForegroundColor Yellow
    }
    if (-not $lld) {
        Write-Host "Warning: 'lld-link' not found in PATH. LLD linker may not be installed." -ForegroundColor Yellow
    }

    exit 0
}

# Try winget first
$winget = Get-Command winget -ErrorAction SilentlyContinue
if ($winget) {
    Write-Host "Installing LLVM $Version via winget..." -ForegroundColor Blue
    try {
        winget install LLVM.LLVM --version $Version --accept-package-agreements --accept-source-agreements
        Write-Host ""
        Write-Host "LLVM installed successfully!" -ForegroundColor Green
        Write-Host "Please restart your terminal for PATH changes to take effect." -ForegroundColor Yellow
        exit 0
    } catch {
        Write-Host "winget installation failed. Trying alternative..." -ForegroundColor Yellow
    }
}

# Provide manual instructions
Write-Host "Automatic installation not available." -ForegroundColor Yellow
Write-Host ""
Write-Host "Please install LLVM manually:" -ForegroundColor White
Write-Host "  1. Download LLVM from: https://github.com/llvm/llvm-project/releases" -ForegroundColor Gray
Write-Host "     Look for: LLVM-$Version-win64.exe" -ForegroundColor Gray
Write-Host "  2. Run the installer" -ForegroundColor Gray
Write-Host "  3. IMPORTANT: Check 'Add LLVM to the system PATH' during installation" -ForegroundColor Gray
Write-Host ""
Write-Host "Or install via winget:" -ForegroundColor White
Write-Host "  winget install LLVM.LLVM" -ForegroundColor Gray
Write-Host ""
Write-Host "After installing, restart your terminal and verify:" -ForegroundColor White
Write-Host "  clang --version" -ForegroundColor Gray
Write-Host "  opt --version" -ForegroundColor Gray
