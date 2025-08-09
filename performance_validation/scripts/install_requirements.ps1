# Install Required Tools for Performance Validation on Windows
# This script checks for and installs missing compilers and tools

param(
    [switch]$Force,
    [switch]$Help,
    [string]$Tools = ""
)

if ($Help) {
    Write-Host @"
Install Requirements Script

This script checks for and installs required tools for performance validation:
- Rust (rustc, cargo)
- C++ (clang or MSVC)
- Zig
- Python packages (numpy, scipy, matplotlib, pandas)
- Git
- CMake

Usage: .\install_requirements.ps1 [OPTIONS]

OPTIONS:
    -Force    Force reinstall even if tools exist
    -Help     Show this help

EXAMPLES:
    .\install_requirements.ps1
    .\install_requirements.ps1 -Force
"@
    exit 0
}

# Color codes
$GREEN = "Green"
$RED = "Red"
$YELLOW = "Yellow"
$BLUE = "Blue"

function Log-Info($Message) {
    Write-Host "[INFO] $Message" -ForegroundColor $BLUE
}

function Log-Success($Message) {
    Write-Host "[SUCCESS] $Message" -ForegroundColor $GREEN
}

function Log-Error($Message) {
    Write-Host "[ERROR] $Message" -ForegroundColor $RED
}

function Log-Warning($Message) {
    Write-Host "[WARNING] $Message" -ForegroundColor $YELLOW
}

# Check if running as administrator
$currentPrincipal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
$isAdmin = $currentPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)

if (-not $isAdmin) {
    Log-Warning "Not running as administrator. Some installations may fail."
    Log-Info "Restart PowerShell as Administrator for best results."
}

# Function to check if a command exists
function Test-Command($Command) {
    try {
        Get-Command $Command -ErrorAction Stop | Out-Null
        return $true
    } catch {
        return $false
    }
}

# Function to install Chocolatey if not present
function Install-Chocolatey {
    if (-not (Test-Command "choco")) {
        Log-Info "Installing Chocolatey package manager..."
        try {
            Set-ExecutionPolicy Bypass -Scope Process -Force
            [System.Net.ServicePointManager]::SecurityProtocol = [System.Net.ServicePointManager]::SecurityProtocol -bor 3072
            Invoke-Expression ((New-Object System.Net.WebClient).DownloadString('https://chocolatey.org/install.ps1'))
            
            # Refresh environment
            $env:Path = [System.Environment]::GetEnvironmentVariable("Path","Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path","User")
            
            Log-Success "Chocolatey installed successfully"
            return $true
        } catch {
            Log-Error "Failed to install Chocolatey: $_"
            return $false
        }
    }
    return $true
}

# Function to install Scoop if not present (alternative to Chocolatey)
function Install-Scoop {
    if (-not (Test-Command "scoop")) {
        Log-Info "Installing Scoop package manager..."
        try {
            Set-ExecutionPolicy RemoteSigned -Scope CurrentUser -Force
            Invoke-RestMethod -Uri https://get.scoop.sh | Invoke-Expression
            
            # Refresh environment
            $env:Path = [System.Environment]::GetEnvironmentVariable("Path","Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path","User")
            
            Log-Success "Scoop installed successfully"
            return $true
        } catch {
            Log-Error "Failed to install Scoop: $_"
            return $false
        }
    }
    return $true
}

# Check and install Git
function Install-Git {
    if ($Force -or -not (Test-Command "git")) {
        Log-Info "Installing Git..."
        
        if (Test-Command "choco") {
            choco install git -y
        } elseif (Test-Command "scoop") {
            scoop install git
        } else {
            Log-Error "No package manager available. Please install Git manually from https://git-scm.com"
            return $false
        }
        
        # Refresh environment
        $env:Path = [System.Environment]::GetEnvironmentVariable("Path","Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path","User")
        
        if (Test-Command "git") {
            Log-Success "Git installed successfully"
            return $true
        } else {
            Log-Error "Git installation failed"
            return $false
        }
    } else {
        $version = git --version
        Log-Success "Git already installed: $version"
        return $true
    }
}

# Check and install Rust
function Install-Rust {
    if ($Force -or -not (Test-Command "rustc")) {
        Log-Info "Installing Rust..."
        
        # Download and run rustup-init
        $rustupUrl = "https://win.rustup.rs/x86_64"
        $rustupPath = "$env:TEMP\rustup-init.exe"
        
        try {
            Invoke-WebRequest -Uri $rustupUrl -OutFile $rustupPath
            Start-Process -FilePath $rustupPath -ArgumentList "-y", "--default-toolchain", "stable" -Wait
            
            # Add to PATH
            $env:Path = "$env:USERPROFILE\.cargo\bin;" + $env:Path
            
            if (Test-Command "rustc") {
                Log-Success "Rust installed successfully"
                return $true
            } else {
                Log-Error "Rust installation failed"
                return $false
            }
        } catch {
            Log-Error "Failed to install Rust: $_"
            return $false
        }
    } else {
        $version = rustc --version
        Log-Success "Rust already installed: $version"
        return $true
    }
}

# Check and install C++ compiler
function Install-Cpp {
    if ($Force -or (-not (Test-Command "clang++") -and -not (Test-Command "cl"))) {
        Log-Info "Installing C++ compiler (Clang)..."
        
        if (Test-Command "choco") {
            choco install llvm -y
        } elseif (Test-Command "scoop") {
            scoop install llvm
        } else {
            Log-Warning "Please install Visual Studio with C++ support or LLVM/Clang manually"
            return $false
        }
        
        # Refresh environment
        $env:Path = [System.Environment]::GetEnvironmentVariable("Path","Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path","User")
        
        if (Test-Command "clang++") {
            Log-Success "Clang++ installed successfully"
            return $true
        } elseif (Test-Command "cl") {
            Log-Success "MSVC compiler found"
            return $true
        } else {
            Log-Error "C++ compiler installation failed"
            return $false
        }
    } else {
        if (Test-Command "clang++") {
            $version = clang++ --version | Select-Object -First 1
            Log-Success "Clang++ already installed: $version"
        } elseif (Test-Command "cl") {
            Log-Success "MSVC compiler found"
        }
        return $true
    }
}

# Check and install Zig
function Install-Zig {
    if ($Force -or -not (Test-Command "zig")) {
        Log-Info "Installing Zig..."
        
        if (Test-Command "choco") {
            choco install zig -y
        } elseif (Test-Command "scoop") {
            scoop bucket add main
            scoop install zig
        } else {
            # Manual download
            Log-Info "Downloading Zig manually..."
            $zigUrl = "https://ziglang.org/download/0.11.0/zig-windows-x86_64-0.11.0.zip"
            $zigZip = "$env:TEMP\zig.zip"
            $zigDir = "$env:LOCALAPPDATA\zig"
            
            try {
                Invoke-WebRequest -Uri $zigUrl -OutFile $zigZip
                Expand-Archive -Path $zigZip -DestinationPath $zigDir -Force
                
                # Add to PATH
                $zigBin = Get-ChildItem -Path $zigDir -Filter "zig.exe" -Recurse | Select-Object -First 1
                if ($zigBin) {
                    $zigPath = $zigBin.DirectoryName
                    [System.Environment]::SetEnvironmentVariable("Path", "$zigPath;" + [System.Environment]::GetEnvironmentVariable("Path", "User"), "User")
                    $env:Path = "$zigPath;" + $env:Path
                }
                
                if (Test-Command "zig") {
                    Log-Success "Zig installed successfully"
                    return $true
                }
            } catch {
                Log-Error "Failed to install Zig: $_"
                return $false
            }
        }
        
        # Refresh environment
        $env:Path = [System.Environment]::GetEnvironmentVariable("Path","Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path","User")
        
        if (Test-Command "zig") {
            Log-Success "Zig installed successfully"
            return $true
        } else {
            Log-Error "Zig installation failed"
            return $false
        }
    } else {
        $version = zig version
        Log-Success "Zig already installed: $version"
        return $true
    }
}

# Check and install Python packages
function Install-PythonPackages {
    if (-not (Test-Command "python")) {
        Log-Error "Python not found. Please install Python 3.8+ first"
        return $false
    }
    
    Log-Info "Installing Python packages for statistical analysis..."
    
    $packages = @("numpy", "scipy", "matplotlib", "pandas", "seaborn")
    $failed = @()
    
    foreach ($package in $packages) {
        try {
            Log-Info "Installing $package..."
            python -m pip install --upgrade $package 2>&1 | Out-Null
            Log-Success "$package installed"
        } catch {
            Log-Warning "Failed to install $package"
            $failed += $package
        }
    }
    
    if ($failed.Count -eq 0) {
        Log-Success "All Python packages installed successfully"
        return $true
    } else {
        Log-Warning "Some packages failed to install: $($failed -join ', ')"
        Log-Info "Try: python -m pip install $($failed -join ' ')"
        return $false
    }
}

# Check and install CMake
function Install-CMake {
    if ($Force -or -not (Test-Command "cmake")) {
        Log-Info "Installing CMake..."
        
        if (Test-Command "choco") {
            choco install cmake -y
        } elseif (Test-Command "scoop") {
            scoop install cmake
        } else {
            Log-Warning "Please install CMake manually from https://cmake.org"
            return $false
        }
        
        # Refresh environment
        $env:Path = [System.Environment]::GetEnvironmentVariable("Path","Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path","User")
        
        if (Test-Command "cmake") {
            Log-Success "CMake installed successfully"
            return $true
        } else {
            Log-Error "CMake installation failed"
            return $false
        }
    } else {
        $version = cmake --version | Select-Object -First 1
        Log-Success "CMake already installed: $version"
        return $true
    }
}

# Main installation process
Write-Host ""
Write-Host "==================================" -ForegroundColor Cyan
Write-Host " Performance Validation Setup" -ForegroundColor Cyan
Write-Host "==================================" -ForegroundColor Cyan
Write-Host ""

# Parse tools list if provided
$toolsToInstall = @()
if ($Tools) {
    $toolsToInstall = $Tools -split ',' | ForEach-Object { $_.Trim().ToLower() }
    Log-Info "Installing specific tools: $($toolsToInstall -join ', ')"
}

# Try to install package managers first
$hasPackageManager = $false
if (Test-Command "choco") {
    Log-Success "Chocolatey found"
    $hasPackageManager = $true
} elseif (Test-Command "scoop") {
    Log-Success "Scoop found"
    $hasPackageManager = $true
} else {
    Log-Info "No package manager found. Attempting to install Scoop..."
    if (Install-Scoop) {
        $hasPackageManager = $true
    } else {
        Log-Warning "Could not install package manager. Some tools may need manual installation."
    }
}

# Install tools based on what's requested or install all if no specific tools provided
$results = @{}

if ($toolsToInstall.Count -eq 0 -or $toolsToInstall -contains "git") {
    $results.Git = Install-Git
}

if ($toolsToInstall.Count -eq 0 -or $toolsToInstall -contains "rustc" -or $toolsToInstall -contains "rust") {
    $results.Rust = Install-Rust
}

if ($toolsToInstall.Count -eq 0 -or $toolsToInstall -contains "clang++" -or $toolsToInstall -contains "g++" -or $toolsToInstall -contains "cpp") {
    $results.Cpp = Install-Cpp
}

if ($toolsToInstall.Count -eq 0 -or $toolsToInstall -contains "zig") {
    $results.Zig = Install-Zig
}

if ($toolsToInstall.Count -eq 0 -or $toolsToInstall -contains "python") {
    $results.Python = Install-PythonPackages
}

if ($toolsToInstall.Count -eq 0 -or $toolsToInstall -contains "cmake") {
    $results.CMake = Install-CMake
}

# Summary
Write-Host ""
Write-Host "==================================" -ForegroundColor Cyan
Write-Host " Installation Summary" -ForegroundColor Cyan
Write-Host "==================================" -ForegroundColor Cyan

$allSuccess = $true
foreach ($tool in $results.Keys) {
    if ($results[$tool]) {
        Write-Host "✅ $tool" -ForegroundColor Green
    } else {
        Write-Host "❌ $tool" -ForegroundColor Red
        $allSuccess = $false
    }
}

Write-Host ""

if ($allSuccess) {
    Log-Success "All requirements installed successfully!"
    Log-Info "You may need to restart your terminal for PATH changes to take effect"
} else {
    Log-Warning "Some installations failed. Please install them manually or run as Administrator"
}

# Create a verification script
$verifyScript = @'
# Verify all tools are installed
Write-Host "Verifying installations..." -ForegroundColor Blue

$tools = @{
    "Git" = "git --version"
    "Rust" = "rustc --version"
    "Cargo" = "cargo --version"
    "C++ (Clang)" = "clang++ --version"
    "Zig" = "zig version"
    "Python" = "python --version"
    "CMake" = "cmake --version"
}

$missing = @()
foreach ($tool in $tools.Keys) {
    try {
        $result = Invoke-Expression $tools[$tool] 2>&1
        Write-Host "✅ $tool : $($result | Select-Object -First 1)" -ForegroundColor Green
    } catch {
        Write-Host "❌ $tool : Not found" -ForegroundColor Red
        $missing += $tool
    }
}

if ($missing.Count -eq 0) {
    Write-Host "`nAll tools verified successfully!" -ForegroundColor Green
} else {
    Write-Host "`nMissing tools: $($missing -join ', ')" -ForegroundColor Red
}
'@

$verifyScript | Out-File -FilePath "verify_installation.ps1" -Encoding UTF8
Log-Info "Created verify_installation.ps1 - run this to check all tools"

Write-Host ""
Log-Info "Next steps:"
Log-Info "1. Run .\verify_installation.ps1 to verify all tools"
Log-Info "2. Restart your terminal if needed"
Log-Info "3. Run .\scripts\run_all.ps1 to start benchmarking"