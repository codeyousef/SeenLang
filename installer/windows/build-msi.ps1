# Build script for Seen Language MSI Installer
# Requires WiX Toolset v3.11+ to be installed

param(
    [Parameter(Mandatory=$true)]
    [string]$Version,
    
    [Parameter(Mandatory=$true)]
    [string]$Platform, # x64 or arm64
    
    [string]$SourceDir = "..\..\target-wsl\release",
    [string]$OutputDir = "output",
    [string]$WixPath = "${env:WIX}bin",
    [switch]$Verbose = $false
)

$ErrorActionPreference = "Stop"

# Configuration
$ProductName = "Seen Language"
$Manufacturer = "Seen Language Team"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = Resolve-Path (Join-Path $ScriptDir "..\..") 

# Validate parameters
if (-not $Version) {
    Write-Error "Version parameter is required"
}

if ($Platform -notin @("x64", "arm64")) {
    Write-Error "Platform must be x64 or arm64"
}

# Validate WiX installation
if (-not $env:WIX) {
    Write-Error "WiX Toolset not found. Please install WiX Toolset and ensure WIX environment variable is set."
}

$candle = Join-Path $WixPath "candle.exe"
$light = Join-Path $WixPath "light.exe"

if (-not (Test-Path $candle)) {
    Write-Error "candle.exe not found at: $candle"
}

if (-not (Test-Path $light)) {
    Write-Error "light.exe not found at: $light"
}

Write-Host "Building Seen Language $Version MSI for $Platform..." -ForegroundColor Green

# Create output directory
$OutputPath = Join-Path $ScriptDir $OutputDir
if (-not (Test-Path $OutputPath)) {
    New-Item -ItemType Directory -Path $OutputPath -Force | Out-Null
}

# Determine paths based on platform
$ProgramFilesVar = if ($Platform -eq "x64") { "ProgramFiles64Folder" } else { "ProgramFilesFolder" }

# Validate source files
$SeenExePath = Join-Path $ProjectRoot "target-wsl\release\seen.exe"
$SeenLspExePath = Join-Path $ProjectRoot "target-wsl\release\seen-lsp.exe"  
$SeenRiscvExePath = Join-Path $ProjectRoot "target-wsl\release\seen-riscv.exe"
$StdlibPath = Join-Path $ProjectRoot "seen_std"
$LanguagesPath = Join-Path $ProjectRoot "languages"
$DocsPath = Join-Path $ProjectRoot "docs"
$IconPath = Join-Path $ScriptDir "assets\seen-icon.ico"

$MissingFiles = @()
@($SeenExePath, $StdlibPath, $LanguagesPath) | ForEach-Object {
    if (-not (Test-Path $_)) {
        $MissingFiles += $_
    }
}

if ($MissingFiles.Count -gt 0) {
    Write-Error "Missing required files: $($MissingFiles -join ', ')"
}

# Optional files (warn if missing)
@($SeenLspExePath, $SeenRiscvExePath, $DocsPath, $IconPath) | ForEach-Object {
    if (-not (Test-Path $_)) {
        Write-Warning "Optional file missing: $_"
    }
}

# Create temporary work directory
$TempDir = Join-Path $env:TEMP "seen-msi-build-$(Get-Random)"
New-Item -ItemType Directory -Path $TempDir -Force | Out-Null

try {
    # Copy WiX source files to temp directory
    Copy-Item (Join-Path $ScriptDir "seen.wxs") $TempDir
    
    # Copy assets if they exist
    $AssetsDir = Join-Path $ScriptDir "assets"
    if (Test-Path $AssetsDir) {
        Copy-Item $AssetsDir $TempDir -Recurse
    } else {
        Write-Warning "Assets directory not found. Creating placeholder files..."
        $TempAssetsDir = Join-Path $TempDir "assets"
        New-Item -ItemType Directory -Path $TempAssetsDir -Force | Out-Null
        
        # Create placeholder icon if missing
        if (-not (Test-Path $IconPath)) {
            $PlaceholderIcon = Join-Path $TempAssetsDir "seen-icon.ico"
            # Create a minimal ICO file (this is just a placeholder)
            [System.IO.File]::WriteAllBytes($PlaceholderIcon, @(0, 0, 1, 0, 1, 0, 16, 16, 16, 0, 1, 0, 4, 0, 40, 1, 0, 0, 22, 0, 0, 0))
            $IconPath = $PlaceholderIcon
        }
    }
    
    # Build WiX variables
    $WixVars = @(
        "-dVersion=$Version"
        "-dPlatform=$Platform"  
        "-dProgramFilesFolder=$ProgramFilesVar"
        "-dSeenExePath=$SeenExePath"
        "-dSeenLspExePath=$SeenLspExePath"
        "-dSeenRiscvExePath=$SeenRiscvExePath"
        "-dStdlibPath=$StdlibPath"
        "-dLanguagesPath=$LanguagesPath"
        "-dDocsPath=$DocsPath"
        "-dIconPath=$IconPath"
        "-dVSCodeExtensionPath=" # Placeholder for future VS Code extension
    )
    
    # Compile with candle
    Write-Host "Compiling WiX source..." -ForegroundColor Blue
    $WxsFile = Join-Path $TempDir "seen.wxs"
    $WixObjFile = Join-Path $TempDir "seen.wixobj"
    
    $CandleArgs = @(
        "-nologo"
        "-out", $WixObjFile
        $WxsFile
    ) + $WixVars
    
    if ($Verbose) {
        $CandleArgs += "-v"
    }
    
    & $candle @CandleArgs
    if ($LASTEXITCODE -ne 0) {
        throw "Candle compilation failed with exit code $LASTEXITCODE"
    }
    
    # Link with light
    Write-Host "Linking MSI package..." -ForegroundColor Blue
    $MsiFile = Join-Path $OutputPath "Seen-$Version-$Platform.msi"
    
    $LightArgs = @(
        "-nologo"
        "-ext", "WixUIExtension"
        "-ext", "WixUtilExtension" 
        "-cultures:en-us"
        "-out", $MsiFile
        $WixObjFile
    )
    
    if ($Verbose) {
        $LightArgs += "-v"
    }
    
    & $light @LightArgs
    if ($LASTEXITCODE -ne 0) {
        throw "Light linking failed with exit code $LASTEXITCODE"
    }
    
    # Validate the MSI
    Write-Host "Validating MSI package..." -ForegroundColor Blue
    if (Test-Path $MsiFile) {
        $MsiInfo = Get-ItemProperty $MsiFile
        $MsiSize = [math]::Round($MsiInfo.Length / 1MB, 2)
        
        Write-Host "✓ MSI created successfully: $MsiFile" -ForegroundColor Green
        Write-Host "  Size: $MsiSize MB" -ForegroundColor Gray
        Write-Host "  Version: $Version" -ForegroundColor Gray
        Write-Host "  Platform: $Platform" -ForegroundColor Gray
        
        # Generate hash for verification
        $Hash = Get-FileHash $MsiFile -Algorithm SHA256
        $HashFile = "$MsiFile.sha256"
        "$($Hash.Hash)  $(Split-Path $MsiFile -Leaf)" | Out-File $HashFile -Encoding ASCII
        Write-Host "  SHA256: $HashFile" -ForegroundColor Gray
        
        return $MsiFile
    } else {
        throw "MSI file was not created"
    }
    
} catch {
    Write-Error "MSI build failed: $($_.Exception.Message)"
    throw
} finally {
    # Cleanup
    if (Test-Path $TempDir) {
        Remove-Item $TempDir -Recurse -Force
    }
}

Write-Host ""
Write-Host "MSI build completed successfully!" -ForegroundColor Green
Write-Host "Output: $MsiFile" -ForegroundColor Cyan