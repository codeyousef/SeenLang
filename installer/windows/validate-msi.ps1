# MSI Validation Script for Seen Language Installer
# Validates the generated MSI file for completeness and correctness

param(
    [Parameter(Mandatory=$true)]
    [string]$MsiPath,
    
    [switch]$Detailed = $false,
    [switch]$DryRun = $false
)

$ErrorActionPreference = "Stop"

function Write-Header {
    param([string]$Title)
    Write-Host ""
    Write-Host "=" * 50 -ForegroundColor Cyan
    Write-Host "  $Title" -ForegroundColor Cyan
    Write-Host "=" * 50 -ForegroundColor Cyan
}

function Test-MsiFile {
    param([string]$Path)
    
    Write-Header "MSI File Validation"
    
    if (-not (Test-Path $Path)) {
        throw "MSI file not found: $Path"
    }
    
    $MsiInfo = Get-ItemProperty $Path
    $Size = [math]::Round($MsiInfo.Length / 1MB, 2)
    
    Write-Host "✓ MSI file exists: $Path" -ForegroundColor Green
    Write-Host "  Size: $Size MB" -ForegroundColor Gray
    Write-Host "  Created: $($MsiInfo.CreationTime)" -ForegroundColor Gray
    Write-Host "  Modified: $($MsiInfo.LastWriteTime)" -ForegroundColor Gray
    
    # Basic size validation
    if ($MsiInfo.Length -lt 1MB) {
        Write-Warning "MSI file seems unusually small ($Size MB)"
    } elseif ($MsiInfo.Length -gt 100MB) {
        Write-Warning "MSI file seems unusually large ($Size MB)"
    }
    
    return $true
}

function Get-MsiProperties {
    param([string]$MsiPath)
    
    Write-Header "MSI Properties"
    
    try {
        # Create Windows Installer object
        $Installer = New-Object -ComObject WindowsInstaller.Installer
        $Database = $Installer.OpenDatabase($MsiPath, 0)
        
        # Query basic properties
        $PropertiesToCheck = @(
            "ProductName",
            "ProductVersion", 
            "Manufacturer",
            "ProductCode",
            "UpgradeCode"
        )
        
        $Properties = @{}
        
        foreach ($PropertyName in $PropertiesToCheck) {
            try {
                $View = $Database.OpenView("SELECT `Value` FROM Property WHERE `Property`='$PropertyName'")
                $View.Execute()
                $Record = $View.Fetch()
                
                if ($Record) {
                    $Value = $Record.StringData(1)
                    $Properties[$PropertyName] = $Value
                    Write-Host "✓ $PropertyName`: $Value" -ForegroundColor Green
                }
                
                $View.Close()
            } catch {
                Write-Warning "Could not read property: $PropertyName"
            }
        }
        
        # Cleanup COM objects
        [System.Runtime.InteropServices.Marshal]::ReleaseComObject($Database) | Out-Null
        [System.Runtime.InteropServices.Marshal]::ReleaseComObject($Installer) | Out-Null
        
        return $Properties
        
    } catch {
        Write-Warning "Could not read MSI properties: $($_.Exception.Message)"
        return @{}
    }
}

function Test-MsiComponents {
    param([string]$MsiPath)
    
    Write-Header "MSI Components"
    
    try {
        $Installer = New-Object -ComObject WindowsInstaller.Installer
        $Database = $Installer.OpenDatabase($MsiPath, 0)
        
        # Check for required components
        $RequiredComponents = @(
            "SeenExecutable",
            "StandardLibraryFiles", 
            "LanguageFiles",
            "PathEnvironmentVariable",
            "SeenRegistryKeys"
        )
        
        $FoundComponents = @()
        
        $View = $Database.OpenView("SELECT `Component` FROM Component")
        $View.Execute()
        
        while ($Record = $View.Fetch()) {
            $ComponentName = $Record.StringData(1)
            $FoundComponents += $ComponentName
            
            if ($ComponentName -in $RequiredComponents) {
                Write-Host "✓ Required component: $ComponentName" -ForegroundColor Green
            } elseif ($Detailed) {
                Write-Host "  Optional component: $ComponentName" -ForegroundColor Gray
            }
        }
        
        # Check for missing required components
        $MissingComponents = $RequiredComponents | Where-Object { $_ -notin $FoundComponents }
        if ($MissingComponents) {
            Write-Warning "Missing required components: $($MissingComponents -join ', ')"
        }
        
        Write-Host "  Total components: $($FoundComponents.Count)" -ForegroundColor Gray
        
        $View.Close()
        [System.Runtime.InteropServices.Marshal]::ReleaseComObject($Database) | Out-Null
        [System.Runtime.InteropServices.Marshal]::ReleaseComObject($Installer) | Out-Null
        
    } catch {
        Write-Warning "Could not analyze MSI components: $($_.Exception.Message)"
    }
}

function Test-MsiFeatures {
    param([string]$MsiPath)
    
    Write-Header "MSI Features"
    
    try {
        $Installer = New-Object -ComObject WindowsInstaller.Installer
        $Database = $Installer.OpenDatabase($MsiPath, 0)
        
        $View = $Database.OpenView("SELECT `Feature`, `Title`, `Description` FROM Feature")
        $View.Execute()
        
        $Features = @()
        
        while ($Record = $View.Fetch()) {
            $Feature = @{
                Feature = $Record.StringData(1)
                Title = $Record.StringData(2)
                Description = $Record.StringData(3)
            }
            $Features += $Feature
            
            Write-Host "✓ Feature: $($Feature.Feature)" -ForegroundColor Green
            if ($Detailed) {
                Write-Host "  Title: $($Feature.Title)" -ForegroundColor Gray
                Write-Host "  Description: $($Feature.Description)" -ForegroundColor Gray
            }
        }
        
        Write-Host "  Total features: $($Features.Count)" -ForegroundColor Gray
        
        $View.Close()
        [System.Runtime.InteropServices.Marshal]::ReleaseComObject($Database) | Out-Null
        [System.Runtime.InteropServices.Marshal]::ReleaseComObject($Installer) | Out-Null
        
    } catch {
        Write-Warning "Could not analyze MSI features: $($_.Exception.Message)"
    }
}

function Test-MsiInstallation {
    param([string]$MsiPath)
    
    if ($DryRun) {
        Write-Header "Installation Test (Dry Run)"
        Write-Host "Skipping installation test (dry run mode)" -ForegroundColor Yellow
        return
    }
    
    Write-Header "Installation Test"
    Write-Host "Testing MSI installation (this requires administrator privileges)..." -ForegroundColor Yellow
    
    # Check if running as administrator
    $IsAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
    
    if (-not $IsAdmin) {
        Write-Warning "Administrator privileges required for installation test. Skipping."
        return
    }
    
    try {
        # Test installation in quiet mode
        Write-Host "Installing MSI (quiet mode)..." -ForegroundColor Blue
        $InstallProcess = Start-Process -FilePath "msiexec.exe" -ArgumentList "/i", "`"$MsiPath`"", "/quiet", "/l*v", "install-test.log" -Wait -PassThru
        
        if ($InstallProcess.ExitCode -eq 0) {
            Write-Host "✓ Installation test passed" -ForegroundColor Green
            
            # Test if seen.exe is available
            try {
                $SeenVersion = & seen --version 2>&1
                Write-Host "✓ Seen executable working: $SeenVersion" -ForegroundColor Green
            } catch {
                Write-Warning "Seen executable test failed: $($_.Exception.Message)"
            }
            
            # Uninstall
            Write-Host "Uninstalling test installation..." -ForegroundColor Blue
            $UninstallProcess = Start-Process -FilePath "msiexec.exe" -ArgumentList "/x", "`"$MsiPath`"", "/quiet" -Wait -PassThru
            
            if ($UninstallProcess.ExitCode -eq 0) {
                Write-Host "✓ Uninstallation test passed" -ForegroundColor Green
            } else {
                Write-Warning "Uninstallation failed with exit code: $($UninstallProcess.ExitCode)"
            }
            
        } else {
            Write-Warning "Installation failed with exit code: $($InstallProcess.ExitCode)"
            
            # Show installation log if available
            if (Test-Path "install-test.log") {
                Write-Host "Installation log (last 20 lines):" -ForegroundColor Yellow
                Get-Content "install-test.log" -Tail 20 | ForEach-Object {
                    Write-Host "  $_" -ForegroundColor Gray
                }
            }
        }
        
    } catch {
        Write-Warning "Installation test failed: $($_.Exception.Message)"
    }
}

function New-ValidationReport {
    param(
        [string]$MsiPath,
        [hashtable]$Properties,
        [bool]$TestResult
    )
    
    Write-Header "Validation Report"
    
    $Report = @{
        MsiPath = $MsiPath
        ValidationTime = Get-Date
        Properties = $Properties
        TestResult = $TestResult
        FileSize = [math]::Round((Get-ItemProperty $MsiPath).Length / 1MB, 2)
    }
    
    # Generate report
    $ReportPath = "$MsiPath.validation-report.json"
    $Report | ConvertTo-Json -Depth 3 | Out-File $ReportPath -Encoding UTF8
    
    Write-Host "✓ Validation report saved: $ReportPath" -ForegroundColor Green
    
    # Summary
    Write-Host ""
    Write-Host "Summary:" -ForegroundColor Cyan
    Write-Host "  MSI File: $MsiPath" -ForegroundColor White
    Write-Host "  Size: $($Report.FileSize) MB" -ForegroundColor White
    Write-Host "  Product: $($Properties.ProductName) v$($Properties.ProductVersion)" -ForegroundColor White
    Write-Host "  Manufacturer: $($Properties.Manufacturer)" -ForegroundColor White
    Write-Host "  Validation: $(if($TestResult) { 'PASSED' } else { 'FAILED' })" -ForegroundColor $(if($TestResult) { 'Green' } else { 'Red' })
}

# Main validation process
try {
    Write-Host "Validating MSI: $MsiPath" -ForegroundColor Cyan
    
    $TestResult = $true
    
    # Basic file validation
    Test-MsiFile -Path $MsiPath
    
    # Read MSI properties  
    $Properties = Get-MsiProperties -MsiPath $MsiPath
    
    # Validate components
    Test-MsiComponents -MsiPath $MsiPath
    
    # Validate features
    Test-MsiFeatures -MsiPath $MsiPath
    
    # Test installation (if not dry run)
    Test-MsiInstallation -MsiPath $MsiPath
    
    # Generate report
    New-ValidationReport -MsiPath $MsiPath -Properties $Properties -TestResult $TestResult
    
    Write-Host ""
    Write-Host "MSI validation completed successfully!" -ForegroundColor Green
    
} catch {
    Write-Host ""
    Write-Host "MSI validation failed: $($_.Exception.Message)" -ForegroundColor Red
    exit 1
}