# Run only bounded version/protocol smoke on the real Windows release runner.
param([Parameter(Mandatory = $true)][string]$Version)
$ErrorActionPreference = 'Stop'
if ($Version -notmatch '^[0-9]+\.[0-9]+\.[0-9]+$') { throw 'Invalid release version' }
if ($env:GITHUB_REF_TYPE -ne 'tag' -or $env:GITHUB_REF_NAME -ne "v$Version") {
    throw 'Windows smoke must run on the exact release tag'
}
$tag = "v$Version"
$repository = 'codeyousef/SeenLang'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$inputDir = Join-Path $env:RUNNER_TEMP "seen-platform-inputs-$Version"
if (Test-Path $inputDir) { throw 'Stale platform input directory exists' }
New-Item -ItemType Directory -Path $inputDir | Out-Null
$release = gh release view $tag --repo $repository --json isDraft,assets | ConvertFrom-Json
if ($LASTEXITCODE -ne 0 -or -not $release.isDraft) { throw 'Release is not an unpublished draft' }
$names = @(
    "seen-$Version-macos-arm64.tar.gz",
    "seen-$Version-windows-x64.zip",
    "Seen-$Version-windows-x64-setup.exe",
    "seen-$Version-platform-inputs.json"
)
$actual = @($release.assets | ForEach-Object { $_.name } | Sort-Object)
$expected = @($names | Sort-Object)
if (@(Compare-Object $actual $expected).Count -ne 0) {
    throw 'Draft asset set is incomplete or unexpected'
}
foreach ($name in $names) {
    gh release download $tag --repo $repository --pattern $name --dir $inputDir
    if ($LASTEXITCODE -ne 0) { throw "Could not download $name" }
}
python "$root/scripts/release_platform_inputs.py" verify `
    --root $root --version $Version --input-dir $inputDir
if ($LASTEXITCODE -ne 0) { throw 'Cross-platform source/hash validation failed' }
$extract = Join-Path $inputDir 'extract'
Expand-Archive -LiteralPath (Join-Path $inputDir "seen-$Version-windows-x64.zip") `
    -DestinationPath $extract
$bin = Join-Path $extract "seen-$Version-windows-x64/bin"

function Invoke-Bounded([string]$Executable, [string[]]$Arguments) {
    $start = [System.Diagnostics.ProcessStartInfo]::new()
    $start.FileName = $Executable
    $start.UseShellExecute = $false
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    foreach ($argument in $Arguments) { [void]$start.ArgumentList.Add($argument) }
    $process = [System.Diagnostics.Process]::new()
    $process.StartInfo = $start
    try {
        if (-not $process.Start()) { throw "Could not start $Executable" }
        if (-not $process.WaitForExit(15000)) {
            $process.Kill($true)
            throw "Timed out: $Executable"
        }
        $output = $process.StandardOutput.ReadToEnd().Trim()
        $errors = $process.StandardError.ReadToEnd().Trim()
        if ($process.ExitCode -ne 0) {
            throw "$Executable exited $($process.ExitCode): $errors"
        }
        return $output
    } finally {
        $process.Dispose()
    }
}
$compilerOutput = Invoke-Bounded (Join-Path $bin 'seen.exe') @('--version')
if (($compilerOutput -split "`n")[0].Trim() -ne "Seen $Version") {
    throw "Windows compiler version mismatch: $compilerOutput"
}
$clientOutput = Invoke-Bounded (Join-Path $bin 'seen-pkg.exe') `
    @('--expect-version', $Version, 'version', '--machine')
$clientLines = @($clientOutput -split '\r?\n')
if ('protocol=SEENPKG1' -notin $clientLines -or "version=$Version" -notin $clientLines) {
    throw "Windows package-client protocol mismatch: $clientOutput"
}
Write-Output "PASS: Windows x64 native compiler and package client $Version"
