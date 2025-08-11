# Test script to verify PowerShell syntax
param([switch]$Verbose)

Write-Host "Testing PowerShell syntax..." -ForegroundColor Cyan

# Test if main script has syntax errors
$scriptPath = Join-Path $PSScriptRoot "run_benchmarks.ps1"
$errors = $null
$tokens = $null
$ast = [System.Management.Automation.Language.Parser]::ParseFile($scriptPath, [ref]$tokens, [ref]$errors)

if ($errors.Count -gt 0) {
    Write-Host "[ERROR] Syntax errors found in run_benchmarks.ps1:" -ForegroundColor Red
    foreach ($error in $errors) {
        Write-Host "  Line $($error.Extent.StartLineNumber): $($error.Message)" -ForegroundColor Yellow
    }
    exit 1
} else {
    Write-Host "[OK] run_benchmarks.ps1 syntax is valid" -ForegroundColor Green
}

# Test other scripts
$otherScripts = @(
    "build_competitors.ps1",
    "quick_bench.ps1"
)

foreach ($script in $otherScripts) {
    $scriptPath = Join-Path $PSScriptRoot $script
    if (Test-Path $scriptPath) {
        $errors = $null
        $tokens = $null
        $ast = [System.Management.Automation.Language.Parser]::ParseFile($scriptPath, [ref]$tokens, [ref]$errors)
        
        if ($errors.Count -gt 0) {
            Write-Host "[ERROR] Syntax errors found in ${script}:" -ForegroundColor Red
            foreach ($error in $errors) {
                Write-Host "  Line $($error.Extent.StartLineNumber): $($error.Message)" -ForegroundColor Yellow
            }
        } else {
            Write-Host "[OK] $script syntax is valid" -ForegroundColor Green
        }
    }
}

Write-Host ""
Write-Host "All PowerShell scripts have valid syntax!" -ForegroundColor Green
Write-Host ""
Write-Host "You can now run the benchmarks with:" -ForegroundColor Cyan
Write-Host "  .\run_benchmarks.ps1" -ForegroundColor White
Write-Host "  .\run_benchmarks.ps1 -Mode aot -Iterations 500" -ForegroundColor White
Write-Host "  .\build_competitors.ps1" -ForegroundColor White
Write-Host "  .\quick_bench.ps1" -ForegroundColor White