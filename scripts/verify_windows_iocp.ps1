#Requires -Version 7
# Execute cross-built IOCP fixtures on a native Windows kernel inside a job object.
param(
    [Parameter(Mandatory = $true)][string]$Archive,
    [Parameter(Mandatory = $true)][string]$Report
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$JobMemoryBytes = 2GB
$ProcessMemoryBytes = 1GB
$ActiveProcesses = 24
$FixtureTimeoutSeconds = 300
$OutputLimit = 65536
$Fixtures = @('iocp-native-fast.exe', 'iocp-native-release.exe',
              'iocp-contract-fast.exe', 'iocp-contract-release.exe')
$VerifiedLine = '{"schema":"seen-io-iocp-v1","backend":"iocp","classification":"verified",' +
    '"scope":"windows-file-contract","submission_overlap":2,' +
    '"native_overlap_verified":false,"fallback_count":0}'

if (-not $IsWindows -or [Environment]::Is64BitOperatingSystem -eq $false) {
    throw 'native Windows IOCP qualification requires a 64-bit Windows kernel'
}
if ([System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture -ne 'X64') {
    throw 'native Windows IOCP qualification requires an x64 kernel'
}
if (Test-Path -LiteralPath $Report) { throw "report already exists: $Report" }

Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class SeenJob {
    [StructLayout(LayoutKind.Sequential)] struct Basic {
        public long PerProcessUserTimeLimit, PerJobUserTimeLimit; public uint LimitFlags;
        public UIntPtr MinimumWorkingSetSize, MaximumWorkingSetSize; public uint ActiveProcessLimit;
        public UIntPtr Affinity; public uint PriorityClass, SchedulingClass; }
    [StructLayout(LayoutKind.Sequential)] struct Io {
        public ulong a, b, c, d, e, f; }
    [StructLayout(LayoutKind.Sequential)] struct Extended {
        public Basic BasicLimitInformation; public Io IoInfo;
        public UIntPtr ProcessMemoryLimit, JobMemoryLimit, PeakProcessMemoryUsed, PeakJobMemoryUsed; }
    [DllImport("kernel32.dll", SetLastError = true)] static extern IntPtr CreateJobObjectW(IntPtr a, IntPtr n);
    [DllImport("kernel32.dll", SetLastError = true)] static extern bool SetInformationJobObject(IntPtr j, int c, ref Extended i, int l);
    [DllImport("kernel32.dll", SetLastError = true)] static extern bool QueryInformationJobObject(IntPtr j, int c, out Extended i, int l, IntPtr r);
    [DllImport("kernel32.dll", SetLastError = true)] static extern bool AssignProcessToJobObject(IntPtr j, IntPtr p);
    [DllImport("kernel32.dll")] static extern IntPtr GetCurrentProcess();
    const uint ProcessMemory = 0x100, JobMemory = 0x200, ActiveProcess = 0x8, KillOnClose = 0x2000;
    static IntPtr handle;
    public static long[] Enter(long job, long process, uint processes) {
        handle = CreateJobObjectW(IntPtr.Zero, IntPtr.Zero);
        if (handle == IntPtr.Zero) throw new System.ComponentModel.Win32Exception();
        var info = new Extended();
        info.BasicLimitInformation.LimitFlags = ProcessMemory | JobMemory | ActiveProcess | KillOnClose;
        info.BasicLimitInformation.ActiveProcessLimit = processes;
        info.ProcessMemoryLimit = (UIntPtr)(ulong)process; info.JobMemoryLimit = (UIntPtr)(ulong)job;
        int size = Marshal.SizeOf(typeof(Extended));
        if (!SetInformationJobObject(handle, 9, ref info, size)) throw new System.ComponentModel.Win32Exception();
        if (!AssignProcessToJobObject(handle, GetCurrentProcess())) throw new System.ComponentModel.Win32Exception();
        Extended actual;
        if (!QueryInformationJobObject(handle, 9, out actual, size, IntPtr.Zero)) throw new System.ComponentModel.Win32Exception();
        if ((actual.BasicLimitInformation.LimitFlags & info.BasicLimitInformation.LimitFlags) != info.BasicLimitInformation.LimitFlags)
            throw new InvalidOperationException("job object limit read-back failed");
        return new long[] { (long)(ulong)actual.JobMemoryLimit, (long)(ulong)actual.ProcessMemoryLimit,
                            actual.BasicLimitInformation.ActiveProcessLimit };
    }
}
'@

$work = Join-Path $env:RUNNER_TEMP ("seen-windows-iocp-" + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $work | Out-Null
& tar.exe -xzf $Archive -C $work
if ($LASTEXITCODE -ne 0) { throw "could not extract $Archive" }
$expected = @($Fixtures + 'windows-iocp-inputs.json' | Sort-Object)
$actual = @(Get-ChildItem -LiteralPath $work -Force | ForEach-Object Name | Sort-Object)
if (Compare-Object $expected $actual) { throw 'unexpected Windows input member set' }
$manifestPath = Join-Path $work 'windows-iocp-inputs.json'
$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json -AsHashtable
if ($manifest.schema -ne 'seen-windows-iocp-inputs-v1' -or $manifest.target -ne 'windows-x86_64') {
    throw 'unexpected Windows input manifest'
}
foreach ($name in $Fixtures) {
    $digest = (Get-FileHash -LiteralPath (Join-Path $work $name) -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($digest -ne $manifest.files[$name].sha256) { throw "fixture digest mismatch: $name" }
}

$limits = [SeenJob]::Enter($JobMemoryBytes, $ProcessMemoryBytes, $ActiveProcesses)
if ($limits[0] -ne $JobMemoryBytes -or $limits[1] -ne $ProcessMemoryBytes -or $limits[2] -ne $ActiveProcesses) {
    throw 'job object limits differ from the requested caps'
}
Write-Output "windows-iocp: job memory=$($limits[0]) process memory=$($limits[1]) processes=$($limits[2])"

function Limit([string]$text) {
    if ($text.Length -gt $OutputLimit) { return $text.Substring(0, $OutputLimit) }
    return $text
}

$results = @()
foreach ($name in $Fixtures) {
    $info = [System.Diagnostics.ProcessStartInfo]::new((Join-Path $work $name))
    if ($name.StartsWith('iocp-native-')) {
        $info.ArgumentList.Add((Join-Path $work ($name + '.data')))
    }
    $info.WorkingDirectory = $work
    $info.UseShellExecute = $false
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $process = [System.Diagnostics.Process]::Start($info)
    $stdout = $process.StandardOutput.ReadToEndAsync()
    $stderr = $process.StandardError.ReadToEndAsync()
    if (-not $process.WaitForExit($FixtureTimeoutSeconds * 1000)) {
        $process.Kill($true)
        throw "fixture exceeded $FixtureTimeoutSeconds seconds: $name"
    }
    $process.WaitForExit()
    $out = Limit $stdout.Result
    $err = Limit $stderr.Result
    Write-Output "--- $name exit=$($process.ExitCode)"
    Write-Output $out
    if ($err) { Write-Output "stderr: $err" }
    if ($process.ExitCode -ne 0) { throw "fixture failed with exit $($process.ExitCode): $name" }
    if ($name.StartsWith('iocp-native-') -and ($out -split "`r?`n") -notcontains $VerifiedLine) {
        throw "fixture did not report verified native IOCP execution: $name"
    }
    $results += [ordered]@{
        name = $name; exit_code = $process.ExitCode; stdout = $out
        sha256 = $manifest.files[$name].sha256
    }
}

$value = [ordered]@{
    schema = 'seen-windows-iocp-report-v1'; status = 'verified'
    inputs_manifest_sha256 = (Get-FileHash -LiteralPath $manifestPath -Algorithm SHA256).Hash.ToLowerInvariant()
    os = [ordered]@{
        platform = [Environment]::OSVersion.Platform.ToString()
        version = [Environment]::OSVersion.Version.ToString()
        architecture = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
    }
    job = [ordered]@{
        job_memory_limit_bytes = $limits[0]; process_memory_limit_bytes = $limits[1]
        active_process_limit = [int]$limits[2]; fixture_timeout_seconds = $FixtureTimeoutSeconds
    }
    results = $results
}
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $Report) | Out-Null
$value | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $Report -Encoding utf8NoBOM
Write-Output 'PASS: native Windows IOCP fixtures verified inside a capped job object'
