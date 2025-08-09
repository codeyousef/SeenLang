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
        Write-Host "âœ… $tool : $($result | Select-Object -First 1)" -ForegroundColor Green
    } catch {
        Write-Host "âŒ $tool : Not found" -ForegroundColor Red
        $missing += $tool
    }
}

if ($missing.Count -eq 0) {
    Write-Host "`nAll tools verified successfully!" -ForegroundColor Green
} else {
    Write-Host "`nMissing tools: $($missing -join ', ')" -ForegroundColor Red
}
