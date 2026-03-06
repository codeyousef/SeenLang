@echo off
REM Build script wrapper for Seen Language MSI Installer
REM Usage: build.bat <version> <platform> [options]

setlocal enabledelayedexpansion

REM Default values
set VERSION=
set PLATFORM=
set VERBOSE=
set SOURCE_DIR=..\..\target-wsl\release

REM Parse command line arguments
:parse_args
if "%~1"=="" goto :check_args
if "%~1"=="--verbose" (
    set VERBOSE=-Verbose
    shift
    goto :parse_args
)
if "%~1"=="--source-dir" (
    set SOURCE_DIR=%~2
    shift
    shift
    goto :parse_args
)
if "%~1"=="--help" goto :show_help
if "%VERSION%"=="" (
    set VERSION=%~1
    shift
    goto :parse_args
)
if "%PLATFORM%"=="" (
    set PLATFORM=%~1
    shift
    goto :parse_args
)
echo Unknown argument: %~1
goto :show_help

:check_args
if "%VERSION%"=="" (
    echo Error: Version is required
    goto :show_help
)
if "%PLATFORM%"=="" (
    echo Error: Platform is required (x64 or arm64)
    goto :show_help
)

REM Validate platform
if not "%PLATFORM%"=="x64" if not "%PLATFORM%"=="arm64" (
    echo Error: Platform must be x64 or arm64
    exit /b 1
)

echo Building Seen Language %VERSION% MSI for %PLATFORM%...

REM Check for PowerShell
where powershell >nul 2>&1
if %errorlevel% neq 0 (
    echo Error: PowerShell is required to build the MSI installer
    exit /b 1
)

REM Check for WiX Toolset
if "%WIX%"=="" (
    echo Error: WiX Toolset not found. Please install WiX Toolset and ensure WIX environment variable is set.
    echo Download from: https://wixtoolset.org/releases/
    exit /b 1
)

REM Run PowerShell build script
powershell -ExecutionPolicy Bypass -File build-msi.ps1 -Version "%VERSION%" -Platform "%PLATFORM%" -SourceDir "%SOURCE_DIR%" %VERBOSE%
if %errorlevel% neq 0 (
    echo MSI build failed!
    exit /b 1
)

echo.
echo MSI build completed successfully!
goto :eof

:show_help
echo Seen Language MSI Installer Build Script
echo.
echo Usage: %~nx0 ^<version^> ^<platform^> [options]
echo.
echo Arguments:
echo   version              Version number (e.g., 1.0.0)
echo   platform             Target platform (x64 or arm64)
echo.
echo Options:
echo   --source-dir DIR     Source directory with binaries (default: ..\..\target-wsl\release)
echo   --verbose            Enable verbose output
echo   --help               Show this help message
echo.
echo Examples:
echo   %~nx0 1.0.0 x64
echo   %~nx0 1.2.3 arm64 --verbose
echo   %~nx0 2.0.0 x64 --source-dir C:\Build\Release
echo.
echo Requirements:
echo   - WiX Toolset v3.11+ (https://wixtoolset.org/releases/)
echo   - PowerShell 5.0+
echo   - Seen binaries built and available in source directory
echo.
exit /b 1