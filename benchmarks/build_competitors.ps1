# Build script for competitor benchmarks on Windows
# Builds Rust, C++, and Zig implementations

param(
    [switch]$Clean,
    [switch]$Release = $true,
    [switch]$Verbose
)

$ErrorActionPreference = "Stop"

# Setup paths
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$CompetitorsDir = Join-Path $ScriptDir "competitors"

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "   BUILDING COMPETITOR BENCHMARKS" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# Function to check if command exists
function Test-Command($Command) {
    try {
        Get-Command $Command -ErrorAction Stop | Out-Null
        return $true
    } catch {
        return $false
    }
}

# Function to find MSVC
function Find-MSVC {
    # Try vswhere first
    $vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
    if (Test-Path $vswhere) {
        $installPath = & $vswhere -latest -property installationPath
        if ($installPath) {
            $vcvarsall = Join-Path $installPath "VC\Auxiliary\Build\vcvarsall.bat"
            if (Test-Path $vcvarsall) {
                return $vcvarsall
            }
        }
    }
    
    # Try common paths
    $commonPaths = @(
        "${env:ProgramFiles}\Microsoft Visual Studio\2022\*\VC\Auxiliary\Build\vcvarsall.bat",
        "${env:ProgramFiles(x86)}\Microsoft Visual Studio\2019\*\VC\Auxiliary\Build\vcvarsall.bat",
        "${env:ProgramFiles(x86)}\Microsoft Visual Studio\2017\*\VC\Auxiliary\Build\vcvarsall.bat"
    )
    
    foreach ($path in $commonPaths) {
        $found = Get-ChildItem $path -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($found) {
            return $found.FullName
        }
    }
    
    return $null
}

# Clean if requested
if ($Clean) {
    Write-Host "Cleaning previous builds..." -ForegroundColor Yellow
    
    # Clean Rust
    $rustTarget = Join-Path $CompetitorsDir "rust\target"
    if (Test-Path $rustTarget) {
        Remove-Item $rustTarget -Recurse -Force
        Write-Host "  [OK] Cleaned Rust build" -ForegroundColor Green
    }
    
    # Clean C++
    $cppExe = Join-Path $CompetitorsDir "cpp\arithmetic_bench.exe"
    $cppObj = Join-Path $CompetitorsDir "cpp\*.obj"
    if (Test-Path $cppExe) {
        Remove-Item $cppExe -Force
    }
    Remove-Item $cppObj -Force -ErrorAction SilentlyContinue
    Write-Host "  [OK] Cleaned C++ build" -ForegroundColor Green
    
    # Clean Zig
    $zigExe = Join-Path $CompetitorsDir "zig\arithmetic_bench.exe"
    $zigCache = Join-Path $CompetitorsDir "zig\zig-cache"
    if (Test-Path $zigExe) {
        Remove-Item $zigExe -Force
    }
    if (Test-Path $zigCache) {
        Remove-Item $zigCache -Recurse -Force
    }
    Write-Host "  [OK] Cleaned Zig build" -ForegroundColor Green
    
    Write-Host ""
}

$successCount = 0
$failCount = 0
$results = @()

# Build Rust benchmark
Write-Host "1. Rust Benchmark" -ForegroundColor Cyan
Write-Host "-----------------" -ForegroundColor Gray

if (Test-Command "cargo") {
    Write-Host "  Found Rust toolchain" -ForegroundColor Green
    
    Push-Location (Join-Path $CompetitorsDir "rust")
    
    try {
        Write-Host "  Building..." -NoNewline
        
        if ($Release) {
            if ($Verbose) {
                cargo build --release
            } else {
                cargo build --release --quiet 2>&1 | Out-Null
            }
        } else {
            if ($Verbose) {
                cargo build
            } else {
                cargo build --quiet 2>&1 | Out-Null
            }
        }
        
        if ($LASTEXITCODE -eq 0) {
            Write-Host " SUCCESS" -ForegroundColor Green
            $successCount++
            
            $exePath = if ($Release) {
                ".\target\release\arithmetic_bench.exe"
            } else {
                ".\target\debug\arithmetic_bench.exe"
            }
            
            if (Test-Path $exePath) {
                $size = (Get-Item $exePath).Length / 1KB
                Write-Host "  Binary size: $([Math]::Round($size, 2)) KB" -ForegroundColor Gray
                $results += "Rust: [OK] Built successfully ($([Math]::Round($size, 2)) KB)"
            }
        } else {
            Write-Host " FAILED" -ForegroundColor Red
            $failCount++
            $results += "Rust: [ERROR] Build failed"
        }
    } catch {
        Write-Host " ERROR: $_" -ForegroundColor Red
        $failCount++
        $results += "Rust: [ERROR] Build error"
    }
    
    Pop-Location
} else {
    Write-Host "  Rust not installed" -ForegroundColor Yellow
    Write-Host "  Install from: https://rustup.rs/" -ForegroundColor Gray
    $results += "Rust: [WARNING] Not installed"
}

Write-Host ""

# Build C++ benchmark
Write-Host "2. C++ Benchmark" -ForegroundColor Cyan
Write-Host "----------------" -ForegroundColor Gray

$cppCompiler = $null
$compilerName = ""

# Check for compilers in order of preference
if (Test-Command "cl") {
    $cppCompiler = "cl"
    $compilerName = "MSVC (cl)"
} else {
    # Try to find MSVC
    $vcvarsall = Find-MSVC
    if ($vcvarsall) {
        Write-Host "  Found Visual Studio, setting up environment..." -ForegroundColor Yellow
        & cmd /c "`"$vcvarsall`" x64 && cl 2>&1" | Out-Null
        if (Test-Command "cl") {
            $cppCompiler = "cl"
            $compilerName = "MSVC (via vcvarsall)"
        }
    }
}

if (!$cppCompiler -and (Test-Command "g++")) {
    $cppCompiler = "g++"
    $compilerName = "GCC (g++)"
}

if (!$cppCompiler -and (Test-Command "clang++")) {
    $cppCompiler = "clang++"
    $compilerName = "Clang (clang++)"
}

if ($cppCompiler) {
    Write-Host "  Found $compilerName" -ForegroundColor Green
    
    $cppSource = Join-Path $CompetitorsDir "cpp\arithmetic_bench.cpp"
    $cppOutput = Join-Path $CompetitorsDir "cpp\arithmetic_bench.exe"
    
    try {
        Write-Host "  Building..." -NoNewline
        
        $buildCommand = switch ($cppCompiler) {
            "cl" {
                if ($Release) {
                    "cl /O2 /EHsc /std:c++20 /Fe:`"$cppOutput`" `"$cppSource`" /nologo"
                } else {
                    "cl /Od /EHsc /std:c++20 /Zi /Fe:`"$cppOutput`" `"$cppSource`" /nologo"
                }
            }
            "g++" {
                if ($Release) {
                    "g++ -O3 -march=native -std=c++20 -o `"$cppOutput`" `"$cppSource`""
                } else {
                    "g++ -O0 -g -std=c++20 -o `"$cppOutput`" `"$cppSource`""
                }
            }
            "clang++" {
                if ($Release) {
                    "clang++ -O3 -march=native -std=c++20 -o `"$cppOutput`" `"$cppSource`""
                } else {
                    "clang++ -O0 -g -std=c++20 -o `"$cppOutput`" `"$cppSource`""
                }
            }
        }
        
        if ($Verbose) {
            Write-Host ""
            Write-Host "  Command: $buildCommand" -ForegroundColor Gray
            Invoke-Expression $buildCommand
        } else {
            Invoke-Expression "$buildCommand 2>&1" | Out-Null
        }
        
        if (Test-Path $cppOutput) {
            Write-Host " SUCCESS" -ForegroundColor Green
            $successCount++
            
            $size = (Get-Item $cppOutput).Length / 1KB
            Write-Host "  Binary size: $([Math]::Round($size, 2)) KB" -ForegroundColor Gray
            $results += "C++: [OK] Built successfully with $compilerName ($([Math]::Round($size, 2)) KB)"
        } else {
            Write-Host " FAILED" -ForegroundColor Red
            $failCount++
            $results += "C++: [ERROR] Build failed"
        }
    } catch {
        Write-Host " ERROR: $_" -ForegroundColor Red
        $failCount++
        $results += "C++: [ERROR] Build error"
    }
} else {
    Write-Host "  No C++ compiler found" -ForegroundColor Yellow
    Write-Host "  Install options:" -ForegroundColor Gray
    Write-Host "    - Visual Studio: https://visualstudio.microsoft.com/" -ForegroundColor Gray
    Write-Host "    - MinGW: https://www.mingw-w64.org/" -ForegroundColor Gray
    Write-Host "    - LLVM: https://releases.llvm.org/" -ForegroundColor Gray
    $results += "C++: [WARNING] No compiler found"
}

Write-Host ""

# Build Zig benchmark
Write-Host "3. Zig Benchmark" -ForegroundColor Cyan
Write-Host "----------------" -ForegroundColor Gray

if (Test-Command "zig") {
    $zigVersion = zig version
    Write-Host "  Found Zig $zigVersion" -ForegroundColor Green
    
    Push-Location (Join-Path $CompetitorsDir "zig")
    
    try {
        Write-Host "  Building..." -NoNewline
        
        $zigSource = "arithmetic_bench.zig"
        $zigOutput = "arithmetic_bench.exe"
        
        if ($Release) {
            if ($Verbose) {
                zig build-exe $zigSource -O ReleaseFast
            } else {
                zig build-exe $zigSource -O ReleaseFast 2>&1 | Out-Null
            }
        } else {
            if ($Verbose) {
                zig build-exe $zigSource
            } else {
                zig build-exe $zigSource 2>&1 | Out-Null
            }
        }
        
        if (Test-Path $zigOutput) {
            Write-Host " SUCCESS" -ForegroundColor Green
            $successCount++
            
            $size = (Get-Item $zigOutput).Length / 1KB
            Write-Host "  Binary size: $([Math]::Round($size, 2)) KB" -ForegroundColor Gray
            $results += "Zig: [OK] Built successfully ($([Math]::Round($size, 2)) KB)"
        } else {
            Write-Host " FAILED" -ForegroundColor Red
            $failCount++
            $results += "Zig: [ERROR] Build failed"
        }
    } catch {
        Write-Host " ERROR: $_" -ForegroundColor Red
        $failCount++
        $results += "Zig: [ERROR] Build error"
    }
    
    Pop-Location
} else {
    Write-Host "  Zig not installed" -ForegroundColor Yellow
    Write-Host "  Install from: https://ziglang.org/download/" -ForegroundColor Gray
    $results += "Zig: [WARNING] Not installed"
}

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "           BUILD SUMMARY" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

foreach ($result in $results) {
    $color = if ($result -match "[OK]") { "Green" } 
             elseif ($result -match "[ERROR]") { "Red" } 
             else { "Yellow" }
    Write-Host $result -ForegroundColor $color
}

Write-Host ""
Write-Host "Total: $successCount succeeded, $failCount failed" -ForegroundColor $(if ($failCount -eq 0) { "Green" } else { "Yellow" })

if ($successCount -gt 0) {
    Write-Host ""
    Write-Host "You can now run benchmarks with:" -ForegroundColor Cyan
    Write-Host "  .\run_benchmarks.ps1" -ForegroundColor White
    Write-Host "  .\quick_bench.ps1" -ForegroundColor White
}

Write-Host ""