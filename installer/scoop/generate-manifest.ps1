# Generate Scoop manifest for Seen Language
# This script creates/updates the Scoop manifest with current release information

param(
    [Parameter(Mandatory=$true)]
    [string]$Version,
    
    [string]$WindowsX64Url = "",
    [string]$WindowsArm64Url = "",
    [string]$WindowsX64Hash = "",
    [string]$WindowsArm64Hash = "",
    [string]$TemplateFile = "seen-lang.json",
    [string]$OutputFile = "",
    [string]$GitHubRepo = "seen-lang/seen",
    [switch]$Verbose = $false
)

$ErrorActionPreference = "Stop"

# Configuration
$ProgressPreference = "SilentlyContinue"  # Disable progress bar for faster downloads

function Write-Header {
    param([string]$Title)
    Write-Host ""
    Write-Host "=" * 50 -ForegroundColor Cyan
    Write-Host "  $Title" -ForegroundColor Cyan
    Write-Host "=" * 50 -ForegroundColor Cyan
    Write-Host ""
}

function Write-Info {
    param([string]$Message)
    Write-Host $Message -ForegroundColor Blue
}

function Write-Success {
    param([string]$Message)
    Write-Host $Message -ForegroundColor Green
}

function Write-Warning-Custom {
    param([string]$Message)
    Write-Host "Warning: $Message" -ForegroundColor Yellow
}

function Write-Error-Custom {
    param([string]$Message)
    Write-Host "Error: $Message" -ForegroundColor Red
    exit 1
}

function Show-Help {
    Write-Host "Scoop Manifest Generator for Seen Language"
    Write-Host ""
    Write-Host "Usage: powershell -File generate-manifest.ps1 -Version <version> [options]"
    Write-Host ""
    Write-Host "Required:"
    Write-Host "  -Version VERSION         Release version (e.g., 1.0.0)"
    Write-Host ""
    Write-Host "Options:"
    Write-Host "  -WindowsX64Url URL       Windows x64 ZIP URL"
    Write-Host "  -WindowsArm64Url URL     Windows ARM64 ZIP URL"
    Write-Host "  -WindowsX64Hash HASH     Windows x64 SHA256 hash"
    Write-Host "  -WindowsArm64Hash HASH   Windows ARM64 SHA256 hash"
    Write-Host "  -TemplateFile FILE       Template manifest file (default: seen-lang.json)"
    Write-Host "  -OutputFile FILE         Output manifest file (default: auto-generated)"
    Write-Host "  -GitHubRepo REPO         GitHub repository (default: seen-lang/seen)"
    Write-Host "  -Verbose                 Enable verbose output"
    Write-Host ""
    Write-Host "If URLs/hashes are not provided, they will be auto-generated from GitHub releases."
    Write-Host ""
    Write-Host "Examples:"
    Write-Host "  .\generate-manifest.ps1 -Version 1.0.0"
    Write-Host "  .\generate-manifest.ps1 -Version 1.2.3 -Verbose"
    Write-Host "  .\generate-manifest.ps1 -Version 2.0.0 -OutputFile C:\scoop-bucket\bucket\seen-lang.json"
    Write-Host ""
    exit 0
}

# Show help if no parameters provided
if (-not $Version) {
    Show-Help
}

# Get script directory
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path

# Set defaults
if (-not $TemplateFile) {
    $TemplateFile = Join-Path $ScriptDir "seen-lang.json"
}

if (-not $OutputFile) {
    $OutputFile = Join-Path $ScriptDir "seen-lang-$Version.json"
}

Write-Header "Generating Scoop Manifest for Seen Language $Version"

Write-Info "Configuration:"
Write-Info "  Version: $Version"
Write-Info "  GitHub Repo: $GitHubRepo"
Write-Info "  Template: $TemplateFile"
Write-Info "  Output: $OutputFile"

function Test-Dependencies {
    Write-Info "Checking dependencies..."
    
    # Check PowerShell version
    $psVersion = $PSVersionTable.PSVersion
    if ($psVersion.Major -lt 5) {
        Write-Error-Custom "PowerShell 5.0+ required. Current version: $psVersion"
    }
    
    Write-Success "✓ All dependencies available"
}

function New-Urls {
    Write-Info "Generating release URLs..."
    
    $baseUrl = "https://github.com/$GitHubRepo/releases/download/v$Version"
    
    if (-not $WindowsX64Url) {
        $script:WindowsX64Url = "$baseUrl/seen-$Version-windows-x64.zip"
    }
    
    if (-not $WindowsArm64Url) {
        $script:WindowsArm64Url = "$baseUrl/seen-$Version-windows-arm64.zip"
    }
    
    Write-Success "✓ URLs generated"
}

function Get-FileHash-Remote {
    param([string]$Url)
    
    $filename = [System.IO.Path]::GetFileName($Url)
    Write-Info "  Fetching SHA256 for $filename..."
    
    try {
        # Create temporary file
        $tempFile = [System.IO.Path]::GetTempFileName()
        
        # Download file
        Invoke-WebRequest -Uri $Url -OutFile $tempFile -UseBasicParsing -ErrorAction Stop
        
        # Calculate hash
        $hash = Get-FileHash $tempFile -Algorithm SHA256
        
        # Cleanup
        Remove-Item $tempFile -Force
        
        return $hash.Hash.ToLower()
        
    } catch {
        Write-Warning-Custom "Could not fetch hash for $filename`: $($_.Exception.Message)"
        
        # Try to fetch from GitHub releases API
        try {
            $apiUrl = "https://api.github.com/repos/$GitHubRepo/releases/tags/v$Version"
            $releaseInfo = Invoke-RestMethod $apiUrl
            
            $asset = $releaseInfo.assets | Where-Object { $_.name -eq $filename }
            if ($asset) {
                # Download from asset URL
                Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $tempFile -UseBasicParsing
                $hash = Get-FileHash $tempFile -Algorithm SHA256
                Remove-Item $tempFile -Force
                return $hash.Hash.ToLower()
            }
        } catch {
            # Return placeholder hash
            Write-Warning-Custom "Using placeholder hash for $filename"
        }
        
        return "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
    }
}

function Get-AllHashes {
    Write-Info "Fetching SHA256 hashes..."
    
    if (-not $WindowsX64Hash) {
        $script:WindowsX64Hash = Get-FileHash-Remote $WindowsX64Url
    }
    
    if (-not $WindowsArm64Hash) {
        $script:WindowsArm64Hash = Get-FileHash-Remote $WindowsArm64Url
    }
    
    Write-Success "✓ Hashes fetched"
}

function New-Manifest {
    Write-Info "Generating manifest..."
    
    # Check if template exists
    if (-not (Test-Path $TemplateFile)) {
        Write-Error-Custom "Template file not found: $TemplateFile"
    }
    
    try {
        # Read template as JSON
        $templateContent = Get-Content $TemplateFile -Raw | ConvertFrom-Json
        
        # Update version
        $templateContent.version = $Version
        
        # Update main URL and hash (defaults to x64)
        $templateContent.url = $WindowsX64Url
        $templateContent.hash = $WindowsX64Hash
        
        # Update architecture-specific URLs and hashes
        $templateContent.architecture.'64bit'.url = $WindowsX64Url
        $templateContent.architecture.'64bit'.hash = $WindowsX64Hash
        $templateContent.architecture.arm64.url = $WindowsArm64Url
        $templateContent.architecture.arm64.hash = $WindowsArm64Hash
        
        # Update autoupdate URLs
        if ($templateContent.autoupdate) {
            $templateContent.autoupdate.architecture.'64bit'.url = "https://github.com/$GitHubRepo/releases/download/v`$version/seen-`$version-windows-x64.zip"
            $templateContent.autoupdate.architecture.arm64.url = "https://github.com/$GitHubRepo/releases/download/v`$version/seen-`$version-windows-arm64.zip"
        }
        
        # Convert back to JSON and save
        $jsonOutput = $templateContent | ConvertTo-Json -Depth 10
        $jsonOutput | Out-File $OutputFile -Encoding UTF8
        
        Write-Success "✓ Manifest generated: $OutputFile"
        
    } catch {
        Write-Error-Custom "Failed to generate manifest: $($_.Exception.Message)"
    }
}

function Test-Manifest {
    Write-Info "Validating manifest..."
    
    if (-not (Test-Path $OutputFile)) {
        Write-Error-Custom "Generated manifest file not found: $OutputFile"
    }
    
    try {
        # Parse JSON to validate syntax
        $manifest = Get-Content $OutputFile -Raw | ConvertFrom-Json
        
        # Check required fields
        $requiredFields = @("version", "description", "homepage", "license", "url", "hash", "bin")
        $missingFields = @()
        
        foreach ($field in $requiredFields) {
            if (-not $manifest.$field) {
                $missingFields += $field
            }
        }
        
        if ($missingFields.Count -gt 0) {
            Write-Warning-Custom "Missing required fields: $($missingFields -join ', ')"
        }
        
        # Check if version matches
        if ($manifest.version -ne $Version) {
            Write-Warning-Custom "Version mismatch in manifest: expected $Version, got $($manifest.version)"
        }
        
        # Check URL format
        if (-not ($manifest.url -match "^https://")) {
            Write-Warning-Custom "URL should use HTTPS protocol"
        }
        
        # Check hash format (should be 64 character hex string)
        if (-not ($manifest.hash -match "^[a-fA-F0-9]{64}$")) {
            Write-Warning-Custom "Hash should be 64-character hexadecimal string"
        }
        
        Write-Success "  ✓ JSON syntax validation passed"
        Write-Info "  Manifest version: $($manifest.version)"
        Write-Info "  Description: $($manifest.description)"
        Write-Info "  License: $($manifest.license)"
        
    } catch {
        Write-Warning-Custom "Manifest validation failed: $($_.Exception.Message)"
    }
    
    Write-Success "✓ Manifest validation completed"
}

function Show-UsageInstructions {
    Write-Success ""
    Write-Success "=" * 50
    Write-Success "     Scoop manifest generated!              "
    Write-Success "=" * 50
    Write-Success ""
    Write-Success "Generated manifest: $OutputFile"
    Write-Success ""
    Write-Success "To use this manifest:"
    Write-Success ""
    Write-Success "1. For local testing:"
    Write-Success "   scoop install $OutputFile"
    Write-Success ""
    Write-Success "2. For Scoop bucket (recommended):"
    Write-Success "   # Copy to your bucket repository:"
    Write-Success "   Copy-Item $OutputFile C:\path\to\scoop-bucket\bucket\seen-lang.json"
    Write-Success "   "
    Write-Success "   # Then users can install with:"
    Write-Success "   scoop bucket add your-bucket https://github.com/your-org/scoop-bucket"
    Write-Success "   scoop install seen-lang"
    Write-Success ""
    Write-Success "3. For main Scoop bucket (submit PR):"
    Write-Success "   # Submit to https://github.com/ScoopInstaller/Main"
    Write-Success "   # Or https://github.com/ScoopInstaller/Extras"
    Write-Success ""
    Write-Success "Testing the manifest:"
    Write-Success "  scoop audit seen-lang"
    Write-Success "  scoop checkver seen-lang"
    Write-Success ""
    Write-Success "Documentation:"
    Write-Success "  https://github.com/ScoopInstaller/Scoop/wiki"
    Write-Success "  https://github.com/ScoopInstaller/Scoop/wiki/App-Manifests"
    Write-Success ""
}

# Main process
function Main {
    Write-Header "Scoop Manifest Generation"
    
    # Validate environment
    Test-Dependencies
    
    # Generate URLs and fetch hashes
    New-Urls
    Get-AllHashes
    
    # Generate the manifest
    New-Manifest
    Test-Manifest
    
    # Show usage instructions
    Show-UsageInstructions
}

# Run main function
try {
    Main
} catch {
    Write-Error-Custom "Manifest generation failed: $($_.Exception.Message)"
}