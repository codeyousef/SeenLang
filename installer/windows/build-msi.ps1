# Build script for Seen Language MSI Installer
# Requires WiX Toolset v3.11+ (or WiX v4 via dotnet tool)
#
# Usage:
#   .\build-msi.ps1 -Version 1.0.0 -Platform x64
#   .\build-msi.ps1 -Version 1.0.0 -Platform x64 -SourceDir C:\path\to\staged

param(
    [Parameter(Mandatory=$true)]
    [string]$Version,

    [Parameter(Mandatory=$false)]
    [string]$Platform = "x64",

    [string]$SourceDir = "",
    [string]$OutputDir = "output",
    [switch]$Verbose = $false
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = Resolve-Path (Join-Path $ScriptDir "..\..") -ErrorAction SilentlyContinue

# Default source directory: look for staged package
if (-not $SourceDir) {
    # Try target-windows staging directory
    $SourceDir = Join-Path $ProjectRoot "target-windows\seen-${Version}-windows-x64"
    if (-not (Test-Path $SourceDir)) {
        # Fallback: try target-windows directly
        $SourceDir = Join-Path $ProjectRoot "target-windows"
    }
}

if ($Platform -notin @("x64", "arm64")) {
    Write-Error "Platform must be x64 or arm64"
}

Write-Host "Building Seen Language $Version MSI for $Platform..." -ForegroundColor Green
Write-Host "  Source: $SourceDir" -ForegroundColor Gray

# Validate source directory
if (-not (Test-Path $SourceDir)) {
    Write-Error "Source directory not found: $SourceDir`nRun scripts/package_windows.sh first."
}

$SeenExe = Join-Path $SourceDir "bin\seen.exe"
if (-not (Test-Path $SeenExe)) {
    Write-Error "seen.exe not found at: $SeenExe`nRun scripts/build_windows.sh and scripts/package_windows.sh first."
}

# Create output directory
$OutputPath = Join-Path $ScriptDir $OutputDir
if (-not (Test-Path $OutputPath)) {
    New-Item -ItemType Directory -Path $OutputPath -Force | Out-Null
}

# Locate WiX tools
$WixBin = ""
if ($env:WIX) {
    $WixBin = Join-Path $env:WIX "bin"
}
if (-not $WixBin -or -not (Test-Path (Join-Path $WixBin "candle.exe"))) {
    # Try common WiX install locations
    $WixPaths = @(
        "${env:ProgramFiles(x86)}\WiX Toolset v3.11\bin",
        "${env:ProgramFiles(x86)}\WiX Toolset v3.14\bin",
        "${env:ProgramFiles}\WiX Toolset v3.14\bin"
    )
    foreach ($p in $WixPaths) {
        if (Test-Path (Join-Path $p "candle.exe")) {
            $WixBin = $p
            break
        }
    }
}

if (-not $WixBin -or -not (Test-Path (Join-Path $WixBin "candle.exe"))) {
    Write-Error "WiX Toolset not found. Install from https://wixtoolset.org/releases/ and set WIX environment variable."
}

$candle = Join-Path $WixBin "candle.exe"
$light = Join-Path $WixBin "light.exe"
$heat = Join-Path $WixBin "heat.exe"

Write-Host "  WiX: $WixBin" -ForegroundColor Gray

# Check for optional files
$HasIcon = "no"
$IconPath = Join-Path $ScriptDir "..\assets\icons\seen-icon.ico"
if (Test-Path $IconPath) {
    $HasIcon = "yes"
}

$HasLicense = "no"
$LicenseRtf = Join-Path $ScriptDir "license.rtf"
if (Test-Path $LicenseRtf) {
    $HasLicense = "yes"
}

# Create temp working directory
$TempDir = Join-Path $env:TEMP "seen-msi-build-$(Get-Random)"
New-Item -ItemType Directory -Path $TempDir -Force | Out-Null

try {
    $FragmentFiles = @()

    # Harvest stdlib directory with heat.exe if it exists
    $StdlibSrcDir = Join-Path $SourceDir "lib\seen\std"
    if ((Test-Path $heat) -and (Test-Path $StdlibSrcDir)) {
        Write-Host "Harvesting stdlib files..." -ForegroundColor Blue
        $StdlibFragment = Join-Path $TempDir "stdlib-fragment.wxs"
        & $heat dir $StdlibSrcDir -cg StdlibFiles -dr StdlibDir -srd -ag -sfrag -var "var.StdlibSrcDir" -out $StdlibFragment
        if ($LASTEXITCODE -eq 0) {
            $FragmentFiles += $StdlibFragment
        } else {
            Write-Warning "heat.exe failed for stdlib, using empty component group"
        }
    }

    # Harvest languages directory
    $LangSrcDir = Join-Path $SourceDir "share\seen\languages"
    if ((Test-Path $heat) -and (Test-Path $LangSrcDir)) {
        Write-Host "Harvesting language files..." -ForegroundColor Blue
        $LangFragment = Join-Path $TempDir "languages-fragment.wxs"
        & $heat dir $LangSrcDir -cg LanguageFiles -dr LanguagesDir -srd -ag -sfrag -var "var.LangSrcDir" -out $LangFragment
        if ($LASTEXITCODE -eq 0) {
            $FragmentFiles += $LangFragment
        } else {
            Write-Warning "heat.exe failed for languages, using empty component group"
        }
    }

    # Harvest docs directory
    $DocsSrcDir = Join-Path $SourceDir "share\seen\docs"
    if ((Test-Path $heat) -and (Test-Path $DocsSrcDir)) {
        Write-Host "Harvesting doc files..." -ForegroundColor Blue
        $DocsFragment = Join-Path $TempDir "docs-fragment.wxs"
        & $heat dir $DocsSrcDir -cg DocFiles -dr DocsDir -srd -ag -sfrag -var "var.DocsSrcDir" -out $DocsFragment
        if ($LASTEXITCODE -eq 0) {
            $FragmentFiles += $DocsFragment
        } else {
            Write-Warning "heat.exe failed for docs, using empty component group"
        }
    }

    # Build WiX variables
    $WixVars = @(
        "-dVersion=$Version",
        "-dPlatform=$Platform",
        "-dSourceDir=$SourceDir",
        "-dHasIcon=$HasIcon",
        "-dHasLicense=$HasLicense"
    )

    if ($HasIcon -eq "yes") {
        $WixVars += "-dIconPath=$IconPath"
    }
    if ($HasLicense -eq "yes") {
        $WixVars += "-dLicenseRtf=$LicenseRtf"
    }
    if (Test-Path $StdlibSrcDir) {
        $WixVars += "-dStdlibSrcDir=$StdlibSrcDir"
    }
    if (Test-Path $LangSrcDir) {
        $WixVars += "-dLangSrcDir=$LangSrcDir"
    }
    if (Test-Path $DocsSrcDir) {
        $WixVars += "-dDocsSrcDir=$DocsSrcDir"
    }

    # If we generated heat fragments, define HeatGenerated
    if ($FragmentFiles.Count -gt 0) {
        $WixVars += "-dHeatGenerated=yes"
    }

    # Compile main .wxs
    Write-Host "Compiling WiX source..." -ForegroundColor Blue
    $WxsFile = Join-Path $ScriptDir "seen.wxs"
    $MainObj = Join-Path $TempDir "seen.wixobj"

    $CandleArgs = @("-nologo", "-out", $MainObj, $WxsFile) + $WixVars
    if ($Verbose) { $CandleArgs += "-v" }

    & $candle @CandleArgs
    if ($LASTEXITCODE -ne 0) {
        throw "candle.exe failed for seen.wxs (exit code $LASTEXITCODE)"
    }

    # Compile fragment files
    $AllObjs = @($MainObj)
    foreach ($frag in $FragmentFiles) {
        $fragBase = [System.IO.Path]::GetFileNameWithoutExtension($frag)
        $fragObj = Join-Path $TempDir "$fragBase.wixobj"
        $FragCandleArgs = @("-nologo", "-out", $fragObj, $frag) + $WixVars
        if ($Verbose) { $FragCandleArgs += "-v" }

        & $candle @FragCandleArgs
        if ($LASTEXITCODE -ne 0) {
            Write-Warning "candle.exe failed for $frag, skipping"
            continue
        }
        $AllObjs += $fragObj
    }

    # Link
    Write-Host "Linking MSI package..." -ForegroundColor Blue
    $MsiFile = Join-Path $OutputPath "Seen-$Version-$Platform.msi"

    $LightArgs = @(
        "-nologo",
        "-ext", "WixUIExtension",
        "-ext", "WixUtilExtension",
        "-cultures:en-us",
        "-out", $MsiFile
    ) + $AllObjs

    if ($Verbose) { $LightArgs += "-v" }

    & $light @LightArgs
    if ($LASTEXITCODE -ne 0) {
        throw "light.exe failed (exit code $LASTEXITCODE)"
    }

    # Success
    if (Test-Path $MsiFile) {
        $MsiSize = [math]::Round((Get-ItemProperty $MsiFile).Length / 1MB, 2)
        $Hash = Get-FileHash $MsiFile -Algorithm SHA256
        "$($Hash.Hash)  $(Split-Path $MsiFile -Leaf)" | Out-File "$MsiFile.sha256" -Encoding ASCII

        Write-Host "" -ForegroundColor Green
        Write-Host "MSI created: $MsiFile" -ForegroundColor Green
        Write-Host "  Size: $MsiSize MB" -ForegroundColor Gray
        Write-Host "  SHA256: $($Hash.Hash)" -ForegroundColor Gray
    } else {
        throw "MSI file was not created"
    }

} catch {
    Write-Error "MSI build failed: $($_.Exception.Message)"
    throw
} finally {
    if (Test-Path $TempDir) {
        Remove-Item $TempDir -Recurse -Force -ErrorAction SilentlyContinue
    }
}
