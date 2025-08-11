@echo off
REM Batch script for running Seen benchmark suite on Windows
REM Simple wrapper around PowerShell script

setlocal enabledelayedexpansion

echo ===========================================
echo   Seen Language Performance Benchmarks
echo ===========================================
echo.

REM Check if PowerShell is available
where powershell >nul 2>&1
if %errorlevel% neq 0 (
    echo ERROR: PowerShell not found!
    echo Please install PowerShell to run benchmarks.
    exit /b 1
)

REM Parse command line arguments
set MODE=jit
set ITERATIONS=100
set CATEGORY=
set VALIDATE=
set COMPARE=

:parse_args
if "%~1"=="" goto :run_benchmarks
if /i "%~1"=="--mode" (
    set MODE=%~2
    shift
    shift
    goto :parse_args
)
if /i "%~1"=="--iterations" (
    set ITERATIONS=%~2
    shift
    shift
    goto :parse_args
)
if /i "%~1"=="--category" (
    set CATEGORY=%~2
    shift
    shift
    goto :parse_args
)
if /i "%~1"=="--validate" (
    set VALIDATE=-Validate
    shift
    goto :parse_args
)
if /i "%~1"=="--compare" (
    set COMPARE=-Compare "%~2"
    shift
    shift
    goto :parse_args
)
if /i "%~1"=="--help" (
    goto :show_help
)
if /i "%~1"=="-h" (
    goto :show_help
)
if /i "%~1"=="/?" (
    goto :show_help
)
shift
goto :parse_args

:show_help
echo Usage: run_benchmarks.bat [OPTIONS]
echo.
echo Options:
echo   --mode [jit^|aot]      Execution mode (default: jit)
echo   --iterations N        Number of iterations (default: 100)
echo   --category NAME       Run specific category only
echo   --validate           Validate performance claims
echo   --compare BASELINE   Compare against baseline file
echo   --help               Show this help message
echo.
echo Examples:
echo   run_benchmarks.bat
echo   run_benchmarks.bat --mode aot --iterations 500
echo   run_benchmarks.bat --category microbenchmarks --validate
echo   run_benchmarks.bat --compare baseline.json
exit /b 0

:run_benchmarks
REM Build PowerShell command
set PS_CMD=powershell -ExecutionPolicy Bypass -File "%~dp0run_benchmarks.ps1"
set PS_CMD=%PS_CMD% -Mode %MODE% -Iterations %ITERATIONS%

if defined CATEGORY (
    set PS_CMD=%PS_CMD% -Category %CATEGORY%
)

if defined VALIDATE (
    set PS_CMD=%PS_CMD% %VALIDATE%
)

if defined COMPARE (
    set PS_CMD=%PS_CMD% %COMPARE%
)

REM Execute PowerShell script
echo Running benchmarks with:
echo   Mode: %MODE%
echo   Iterations: %ITERATIONS%
if defined CATEGORY echo   Category: %CATEGORY%
echo.
echo Launching PowerShell script...
echo ----------------------------------------
%PS_CMD%
set EXIT_CODE=%errorlevel%

if %EXIT_CODE% neq 0 (
    echo.
    echo ----------------------------------------
    echo ERROR: Benchmark suite failed with exit code %EXIT_CODE%
    exit /b %EXIT_CODE%
)

echo.
echo ----------------------------------------
echo Benchmark suite completed successfully!
exit /b 0