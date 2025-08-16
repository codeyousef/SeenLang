# Simple Seen Language Installer for Windows
param(
    [switch]$System = $false
)

function Write-Success {
    param([string]$Message)
    Write-Host $Message -ForegroundColor Green
}

function Write-Info {
    param([string]$Message)
    Write-Host $Message -ForegroundColor Blue
}

function Write-Error-Custom {
    param([string]$Message)
    Write-Host "Error: $Message" -ForegroundColor Red
    exit 1
}

function Test-Administrator {
    $currentUser = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = New-Object Security.Principal.WindowsPrincipal($currentUser)
    return $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

Write-Host ""
Write-Host "===============================================" -ForegroundColor Cyan
Write-Host "     Seen Language Installer                  " -ForegroundColor Cyan
Write-Host "===============================================" -ForegroundColor Cyan
Write-Host ""

# Check for system installation
if ($System) {
    if (-not (Test-Administrator)) {
        Write-Error-Custom "System-wide installation requires administrator privileges."
    }
    $InstallDir = "$env:ProgramFiles\Seen"
} else {
    $InstallDir = "$env:LOCALAPPDATA\Seen"
}

Write-Info "Installing Seen to: $InstallDir"

# Find the compiler executable
$compilerPath = "D:\Projects\Rust\seenlang\compiler_seen\target\seen.exe"

if (-not (Test-Path $compilerPath)) {
    Write-Error-Custom "Seen compiler not found at: $compilerPath"
}

Write-Info "Found Seen compiler at: $compilerPath"

# Create installation directory structure
$binDir = Join-Path $InstallDir "bin"
$libDir = Join-Path $InstallDir "lib"
$shareDir = Join-Path $InstallDir "share\seen"

Write-Info "Creating installation directories..."
New-Item -ItemType Directory -Path $binDir -Force | Out-Null
New-Item -ItemType Directory -Path $libDir -Force | Out-Null
New-Item -ItemType Directory -Path $shareDir -Force | Out-Null

# Copy compiler
Write-Info "Installing Seen compiler..."
Copy-Item $compilerPath (Join-Path $binDir "seen.exe") -Force

# Copy language files
$languagesPath = "D:\Projects\Rust\seenlang\languages"
if (Test-Path $languagesPath) {
    Write-Info "Installing language configurations..."
    Copy-Item $languagesPath (Join-Path $shareDir "languages") -Recurse -Force
}

# Copy examples
$examplesPath = "D:\Projects\Rust\seenlang\examples"
if (Test-Path $examplesPath) {
    Write-Info "Installing examples..."
    Copy-Item $examplesPath (Join-Path $shareDir "examples") -Recurse -Force
}

# Update PATH
Write-Info "Setting up PATH..."

if ($System) {
    $scope = [System.EnvironmentVariableTarget]::Machine
    $scopeName = "system"
} else {
    $scope = [System.EnvironmentVariableTarget]::User
    $scopeName = "user"
}

try {
    $currentPath = [Environment]::GetEnvironmentVariable("PATH", $scope)
    
    if ($currentPath -notlike "*$binDir*") {
        $newPath = if ($currentPath) { "$binDir;$currentPath" } else { $binDir }
        [Environment]::SetEnvironmentVariable("PATH", $newPath, $scope)
        Write-Info "✓ Added $binDir to $scopeName PATH"
    } else {
        Write-Info "✓ $binDir already in $scopeName PATH"
    }
    
    # Update current session PATH
    $env:PATH = "$binDir;$env:PATH"
    
} catch {
    Write-Host "Warning: Failed to update PATH. Please add $binDir to your PATH manually." -ForegroundColor Yellow
}

# Set environment variables for Seen
Write-Info "Setting up Seen environment variables..."

try {
    [Environment]::SetEnvironmentVariable("SEEN_HOME", $InstallDir, $scope)
    [Environment]::SetEnvironmentVariable("SEEN_LIB", $libDir, $scope)
    [Environment]::SetEnvironmentVariable("SEEN_SHARE", $shareDir, $scope)
    
    # Update current session
    $env:SEEN_HOME = $InstallDir
    $env:SEEN_LIB = $libDir
    $env:SEEN_SHARE = $shareDir
    
    Write-Success "✓ Environment variables set"
} catch {
    Write-Host "Warning: Failed to set environment variables." -ForegroundColor Yellow
}

# Test installation
Write-Info "Verifying installation..."

try {
    $output = & seen --version 2>&1
    if ($LASTEXITCODE -eq 0) {
        Write-Success "✓ Seen installed successfully!"
        Write-Success "Version: $output"
    } else {
        Write-Error-Custom "Installation verification failed."
    }
} catch {
    Write-Error-Custom "Installation verification failed. 'seen' command not found."
}

# Show completion message
Write-Host ""
Write-Success "==============================================="
Write-Success "     Installation completed successfully!      "
Write-Success "==============================================="
Write-Host ""
Write-Host "To get started with Seen:"
Write-Host ""
Write-Host "  # Create a new project" -ForegroundColor Blue
Write-Host "  seen init my-project" -ForegroundColor Blue
Write-Host "  cd my-project" -ForegroundColor Blue
Write-Host ""
Write-Host "  # Build your project" -ForegroundColor Blue
Write-Host "  seen build main.seen" -ForegroundColor Blue
Write-Host ""
Write-Host "  # Run your project" -ForegroundColor Blue
Write-Host "  seen run main.seen" -ForegroundColor Blue
Write-Host ""
Write-Host "For VS Code support:"
Write-Host "  • Restart VS Code" -ForegroundColor Blue  
Write-Host "  • Open a .seen file" -ForegroundColor Blue
Write-Host "  • The Seen extension should now work with the system-wide compiler" -ForegroundColor Blue
Write-Host ""
Write-Host "Installation directory: $InstallDir" -ForegroundColor Gray
Write-Host ""
Write-Host "You may need to restart your terminal or VS Code." -ForegroundColor Yellow
Write-Host ""