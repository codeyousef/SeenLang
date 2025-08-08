# Universal Seen Language Installer for Windows
# Usage: Invoke-WebRequest -Uri https://install.seen-lang.org/install.ps1 | Invoke-Expression
#    or: powershell -ExecutionPolicy Bypass -File install.ps1

param(
    [string]$Version = "latest",
    [string]$InstallDir = "$env:LOCALAPPDATA\Seen",
    [string]$Arch = $null,
    [switch]$NoPath = $false,
    [switch]$NoStdlib = $false,
    [switch]$Help = $false,
    [switch]$System = $false
)

# Ensure we can run scripts
$ExecutionPolicy = Get-ExecutionPolicy
if ($ExecutionPolicy -eq "Restricted") {
    Write-Host "PowerShell execution policy is Restricted. This installer needs to run scripts." -ForegroundColor Red
    Write-Host "Please run: Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser" -ForegroundColor Yellow
    exit 1
}

# Configuration
$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"  # Disable progress bar for faster downloads

# Colors for output
function Write-Header {
    Write-Host ""
    Write-Host "===============================================" -ForegroundColor Cyan
    Write-Host "     Seen Language Installer for Windows      " -ForegroundColor Cyan
    Write-Host "===============================================" -ForegroundColor Cyan
    Write-Host ""
}

function Write-Error-Custom {
    param([string]$Message)
    Write-Host "Error: $Message" -ForegroundColor Red
    exit 1
}

function Write-Warning-Custom {
    param([string]$Message)
    Write-Host "Warning: $Message" -ForegroundColor Yellow
}

function Write-Info {
    param([string]$Message)
    Write-Host $Message -ForegroundColor Blue
}

function Write-Success {
    param([string]$Message)
    Write-Host $Message -ForegroundColor Green
}

function Show-Help {
    Write-Host "Seen Language Installer for Windows"
    Write-Host ""
    Write-Host "Usage: powershell -ExecutionPolicy Bypass -File install.ps1 [OPTIONS]"
    Write-Host ""
    Write-Host "Options:"
    Write-Host "  -Version VERSION     Install specific version (default: latest)"
    Write-Host "  -InstallDir DIR      Installation directory (default: `$env:LOCALAPPDATA\Seen)"
    Write-Host "  -Arch ARCH           Target architecture (default: auto-detect)"
    Write-Host "  -NoPath              Don't modify PATH"
    Write-Host "  -NoStdlib            Don't install standard library"
    Write-Host "  -System              Install system-wide (requires admin privileges)"
    Write-Host "  -Help                Show this help message"
    Write-Host ""
    Write-Host "Architecture options: x64, arm64"
    Write-Host ""
    Write-Host "Examples:"
    Write-Host "  .\install.ps1                              # Install latest version"
    Write-Host "  .\install.ps1 -Version 1.0.0               # Install specific version"
    Write-Host "  .\install.ps1 -System                      # System-wide installation"
    Write-Host "  .\install.ps1 -InstallDir C:\Tools\Seen    # Custom directory"
    Write-Host ""
    exit 0
}

# Show help if requested
if ($Help) {
    Show-Help
}

function Test-Administrator {
    $currentUser = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = New-Object Security.Principal.WindowsPrincipal($currentUser)
    return $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

function Get-Architecture {
    if ($Arch) {
        switch ($Arch.ToLower()) {
            "x64" { return "x64" }
            "amd64" { return "x64" }
            "arm64" { return "arm64" }
            "aarch64" { return "arm64" }
            default { Write-Error-Custom "Unsupported architecture: $Arch. Supported: x64, arm64" }
        }
    }
    
    $osArch = $env:PROCESSOR_ARCHITECTURE
    $wow64Arch = $env:PROCESSOR_ARCHITEW6432
    
    if ($wow64Arch) {
        $osArch = $wow64Arch
    }
    
    switch ($osArch) {
        "AMD64" { return "x64" }
        "ARM64" { return "arm64" }
        default { Write-Error-Custom "Unsupported architecture: $osArch" }
    }
}

function Test-Dependencies {
    Write-Info "Checking dependencies..."
    
    # Check for required .NET Framework version
    $netVersion = Get-ItemProperty "HKLM:\SOFTWARE\Microsoft\NET Framework Setup\NDP\v4\Full\" -Name Release -ErrorAction SilentlyContinue
    if (-not $netVersion -or $netVersion.Release -lt 461808) {
        Write-Warning-Custom ".NET Framework 4.7.2 or later is recommended for optimal performance"
    }
    
    Write-Success "✓ Dependencies checked"
}

function New-TemporaryDirectory {
    $tempPath = [System.IO.Path]::GetTempPath()
    $tempDir = Join-Path $tempPath ([System.IO.Path]::GetRandomFileName())
    New-Item -ItemType Directory -Path $tempDir | Out-Null
    return $tempDir
}

function Get-SeenRelease {
    param(
        [string]$Version,
        [string]$Architecture,
        [string]$TempDir
    )
    
    $baseUrl = "https://github.com/seen-lang/seen/releases"
    $filename = "seen-$Version-windows-$Architecture.zip"
    
    if ($Version -eq "latest") {
        $url = "$baseUrl/latest/download/$filename"
    } else {
        $url = "$baseUrl/download/v$Version/$filename"
    }
    
    $downloadPath = Join-Path $TempDir "seen.zip"
    
    Write-Info "Downloading Seen $Version for Windows ($Architecture)..."
    Write-Info "URL: $url"
    
    try {
        Invoke-WebRequest -Uri $url -OutFile $downloadPath -UseBasicParsing
        Write-Success "✓ Download completed"
        return $downloadPath
    } catch {
        if ($_.Exception.Response.StatusCode -eq 404) {
            Write-Error-Custom "Release not found. Please check the version number and try again."
        } else {
            Write-Error-Custom "Failed to download Seen release: $($_.Exception.Message)"
        }
    }
}

function Test-Download {
    param([string]$FilePath)
    
    Write-Info "Verifying download..."
    
    if (-not (Test-Path $FilePath)) {
        Write-Error-Custom "Download file not found"
    }
    
    # Check if it's a valid ZIP file
    try {
        Add-Type -AssemblyName System.IO.Compression.FileSystem
        $zip = [System.IO.Compression.ZipFile]::OpenRead($FilePath)
        $zip.Dispose()
        Write-Success "✓ Download verification passed"
    } catch {
        Write-Error-Custom "Downloaded file is not a valid ZIP archive"
    }
}

function Install-Seen {
    param(
        [string]$ZipPath,
        [string]$InstallDir,
        [bool]$InstallStdlib = $true
    )
    
    Write-Info "Extracting files..."
    
    # Create installation directory
    if (-not (Test-Path $InstallDir)) {
        try {
            New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
        } catch {
            Write-Error-Custom "Failed to create installation directory: $InstallDir"
        }
    }
    
    # Extract ZIP file
    try {
        Add-Type -AssemblyName System.IO.Compression.FileSystem
        [System.IO.Compression.ZipFile]::ExtractToDirectory($ZipPath, $InstallDir)
    } catch {
        Write-Error-Custom "Failed to extract files: $($_.Exception.Message)"
    }
    
    Write-Info "Installing Seen to $InstallDir..."
    
    # Set up directory structure
    $binDir = Join-Path $InstallDir "bin"
    $libDir = Join-Path $InstallDir "lib"
    $shareDir = Join-Path $InstallDir "share"
    
    New-Item -ItemType Directory -Path $binDir -Force | Out-Null
    New-Item -ItemType Directory -Path $libDir -Force | Out-Null
    New-Item -ItemType Directory -Path $shareDir -Force | Out-Null
    
    # Move binaries to bin directory if not already there
    $seenExe = Get-ChildItem -Path $InstallDir -Name "seen.exe" -Recurse | Select-Object -First 1
    if ($seenExe) {
        $seenPath = Join-Path $InstallDir $seenExe
        if ($seenPath -ne (Join-Path $binDir "seen.exe")) {
            Move-Item $seenPath (Join-Path $binDir "seen.exe") -Force
        }
    }
    
    # Move LSP server if present
    $lspExe = Get-ChildItem -Path $InstallDir -Name "seen-lsp.exe" -Recurse | Select-Object -First 1
    if ($lspExe) {
        $lspPath = Join-Path $InstallDir $lspExe
        if ($lspPath -ne (Join-Path $binDir "seen-lsp.exe")) {
            Move-Item $lspPath (Join-Path $binDir "seen-lsp.exe") -Force
        }
    }
    
    # Move RISC-V tools if present
    $riscvExe = Get-ChildItem -Path $InstallDir -Name "seen-riscv.exe" -Recurse | Select-Object -First 1
    if ($riscvExe) {
        $riscvPath = Join-Path $InstallDir $riscvExe
        if ($riscvPath -ne (Join-Path $binDir "seen-riscv.exe")) {
            Move-Item $riscvPath (Join-Path $binDir "seen-riscv.exe") -Force
        }
    }
    
    # Install standard library
    if ($InstallStdlib) {
        $stdlibDir = Get-ChildItem -Path $InstallDir -Name "stdlib" -Directory -Recurse | Select-Object -First 1
        if ($stdlibDir) {
            $stdlibPath = Join-Path $InstallDir $stdlibDir
            $targetStdlibPath = Join-Path $libDir "seen"
            if ($stdlibPath -ne $targetStdlibPath) {
                if (Test-Path $targetStdlibPath) {
                    Remove-Item $targetStdlibPath -Recurse -Force
                }
                Move-Item $stdlibPath $targetStdlibPath -Force
            }
        }
    }
    
    # Install language configurations
    $languagesDir = Get-ChildItem -Path $InstallDir -Name "languages" -Directory -Recurse | Select-Object -First 1
    if ($languagesDir) {
        $languagesPath = Join-Path $InstallDir $languagesDir
        $targetLanguagesPath = Join-Path $shareDir "seen"
        New-Item -ItemType Directory -Path $targetLanguagesPath -Force | Out-Null
        if ($languagesPath -ne (Join-Path $targetLanguagesPath "languages")) {
            if (Test-Path (Join-Path $targetLanguagesPath "languages")) {
                Remove-Item (Join-Path $targetLanguagesPath "languages") -Recurse -Force
            }
            Move-Item $languagesPath (Join-Path $targetLanguagesPath "languages") -Force
        }
    }
    
    # Install documentation
    $docsDir = Get-ChildItem -Path $InstallDir -Name "docs" -Directory -Recurse | Select-Object -First 1
    if ($docsDir) {
        $docsPath = Join-Path $InstallDir $docsDir
        $targetDocsPath = Join-Path $shareDir "seen"
        New-Item -ItemType Directory -Path $targetDocsPath -Force | Out-Null
        if ($docsPath -ne (Join-Path $targetDocsPath "docs")) {
            if (Test-Path (Join-Path $targetDocsPath "docs")) {
                Remove-Item (Join-Path $targetDocsPath "docs") -Recurse -Force
            }
            Move-Item $docsPath (Join-Path $targetDocsPath "docs") -Force
        }
    }
    
    Write-Success "✓ Seen installed successfully"
}

function Add-ToPath {
    param(
        [string]$InstallDir,
        [bool]$SystemWide = $false
    )
    
    if ($NoPath) {
        return
    }
    
    Write-Info "Setting up PATH..."
    
    $binPath = Join-Path $InstallDir "bin"
    
    if ($SystemWide) {
        $scope = [System.EnvironmentVariableTarget]::Machine
        $scopeName = "system"
    } else {
        $scope = [System.EnvironmentVariableTarget]::User
        $scopeName = "user"
    }
    
    try {
        $currentPath = [Environment]::GetEnvironmentVariable("PATH", $scope)
        
        if ($currentPath -notlike "*$binPath*") {
            $newPath = if ($currentPath) { "$binPath;$currentPath" } else { $binPath }
            [Environment]::SetEnvironmentVariable("PATH", $newPath, $scope)
            Write-Info "✓ Added $binPath to $scopeName PATH"
        } else {
            Write-Info "✓ $binPath already in $scopeName PATH"
        }
        
        # Update current session PATH
        $env:PATH = "$binPath;$env:PATH"
        
        Write-Success "✓ PATH configuration completed"
        
    } catch {
        Write-Warning-Custom "Failed to update PATH. Please add $binPath to your PATH manually."
    }
}

function New-StartMenuShortcuts {
    param([string]$InstallDir)
    
    if ($System) {
        $startMenuPath = "$env:ProgramData\Microsoft\Windows\Start Menu\Programs"
    } else {
        $startMenuPath = "$env:APPDATA\Microsoft\Windows\Start Menu\Programs"
    }
    
    $seenFolder = Join-Path $startMenuPath "Seen Language"
    
    try {
        if (-not (Test-Path $seenFolder)) {
            New-Item -ItemType Directory -Path $seenFolder -Force | Out-Null
        }
        
        $seenExe = Join-Path $InstallDir "bin\seen.exe"
        if (Test-Path $seenExe) {
            $WshShell = New-Object -comObject WScript.Shell
            
            # Create shortcut for Seen command prompt
            $shortcut = $WshShell.CreateShortcut("$seenFolder\Seen Command Prompt.lnk")
            $shortcut.TargetPath = "cmd.exe"
            $shortcut.Arguments = "/k `"cd /d %USERPROFILE% && echo Seen Language Environment && seen --version`""
            $shortcut.WorkingDirectory = "%USERPROFILE%"
            $shortcut.Description = "Open command prompt with Seen Language"
            $shortcut.Save()
            
            Write-Info "✓ Created Start Menu shortcuts"
        }
    } catch {
        Write-Warning-Custom "Failed to create Start Menu shortcuts: $($_.Exception.Message)"
    }
}

function Test-Installation {
    Write-Info "Verifying installation..."
    
    try {
        $output = & seen --version 2>&1
        if ($LASTEXITCODE -eq 0) {
            Write-Success "✓ Seen installed: $output"
            return $true
        } else {
            Write-Error-Custom "Installation verification failed. 'seen --version' returned exit code $LASTEXITCODE"
        }
    } catch {
        Write-Error-Custom "Installation verification failed. 'seen' command not found or not working."
    }
    
    return $false
}

function Remove-TemporaryFiles {
    param([string]$TempDir)
    
    if ($TempDir -and (Test-Path $TempDir)) {
        try {
            Remove-Item $TempDir -Recurse -Force
        } catch {
            Write-Warning-Custom "Failed to clean up temporary files in $TempDir"
        }
    }
}

function Show-GettingStarted {
    param([string]$InstallDir)
    
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
    Write-Host "  seen build" -ForegroundColor Blue
    Write-Host ""
    Write-Host "  # Run your project" -ForegroundColor Blue
    Write-Host "  seen run" -ForegroundColor Blue
    Write-Host ""
    Write-Host "For VS Code support, install the extension:"
    Write-Host "  code --install-extension seen-lang.seen-vscode" -ForegroundColor Blue
    Write-Host ""
    Write-Host "Documentation: https://docs.seen-lang.org"
    Write-Host "Community: https://discord.gg/seen-lang"
    Write-Host ""
    Write-Host "Installation directory: $InstallDir" -ForegroundColor Gray
    Write-Host ""
    Write-Host "You may need to restart your terminal or run:" -ForegroundColor Yellow
    Write-Host "  refreshenv" -ForegroundColor Yellow
    Write-Host ""
}

# Main installation function
function Install-SeenLanguage {
    Write-Header
    
    # Validate system requirements
    if ($System) {
        if (-not (Test-Administrator)) {
            Write-Error-Custom "System-wide installation requires administrator privileges. Run PowerShell as Administrator or remove -System flag."
        }
        $InstallDir = "$env:ProgramFiles\Seen"
    }
    
    # Detect system information
    $architecture = Get-Architecture
    Write-Info "Detected: Windows ($architecture)"
    Write-Info "Installing Seen $Version to $InstallDir"
    
    if ($System) {
        Write-Info "System-wide installation (requires administrator)"
    } else {
        Write-Info "User installation"
    }
    
    # Pre-flight checks
    Test-Dependencies
    
    # Create temporary directory
    $tempDir = New-TemporaryDirectory
    
    try {
        # Download and install
        $downloadPath = Get-SeenRelease -Version $Version -Architecture $architecture -TempDir $tempDir
        Test-Download -FilePath $downloadPath
        Install-Seen -ZipPath $downloadPath -InstallDir $InstallDir -InstallStdlib (-not $NoStdlib)
        Add-ToPath -InstallDir $InstallDir -SystemWide $System
        New-StartMenuShortcuts -InstallDir $InstallDir
        
        # Verify and complete
        if (Test-Installation) {
            Show-GettingStarted -InstallDir $InstallDir
        }
    } finally {
        # Cleanup
        Remove-TemporaryFiles -TempDir $tempDir
    }
}

# Run the installation
try {
    Install-SeenLanguage
} catch {
    Write-Error-Custom "Installation failed: $($_.Exception.Message)"
}