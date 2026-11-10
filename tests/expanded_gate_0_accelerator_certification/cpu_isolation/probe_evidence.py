#!/usr/bin/env python3
"""Test-only observer: inspect actual strace paths/execs, never infer from ELF alone."""
import argparse
import ast
import gzip
import hashlib
import json
import os
from pathlib import Path
import re
import sys
import tempfile

MAX_BYTES = 64 * 1024 * 1024
MAX_COMPRESSED_TRACE = 8 * 1024 * 1024
MAX_COMPRESSED_TRACES = 64 * 1024 * 1024
GPU_TOOLS = frozenset(("nvcc", "ptxas", "nvlink", "fatbinary", "cuobjdump", "nvidia-smi",
                       "glslc", "glslangValidator", "spirv-as", "spirv-val", "hipcc", "hipconfig",
                       "rocminfo", "rocm-smi", "metal", "metallib", "xcrun"))
QUOTED = re.compile(r'"(?:[^"\\]|\\.)*"')
GPU_LIBRARY = re.compile(r"^(?:lib)?(?:cuda|cudart|cublas|nvrtc|nvidia|vulkan|amdhip|hiprtc|OpenCL|Metal)[a-z0-9_-]*\.(?:so(?:\.[0-9.]+)?|a|dylib|dll)$", re.I)
GPU_PACKAGE = re.compile(r"(?:^|[^a-z0-9])(?:cuda|cudart|cublas|vulkan|hip|rocm|opencl|metal)(?:$|[^a-z0-9])", re.I)
GPU_HEADER = re.compile(r"^(?:cuda(?:_runtime(?:_api)?)?|cublas(?:Lt|_v2)?|nvrtc|vulkan(?:_core)?|hip_runtime(?:_api)?)\.h$", re.I)
SDK_ROOT = re.compile(r"^/(?:opt|usr/local)/(?:cuda(?:-[^/]+)?|vulkan[^/]*|rocm(?:-[^/]+)?)(?:/|$)", re.I)
# Only failed lookup of known host tools in an actual PATH entry is incidental.
# Opening files, successful execution and every GPU-named tool remain findings.
HOST_TOOL = re.compile(r"^(?:(?:x86_64|i[3-6]86|aarch64|arm|riscv64|powerpc64|s390x)(?:-[a-z0-9_]+)+-)?(?:g?make|g?nm|g?strip|gcc|g\+\+|cc|c\+\+|clang(?:\+\+)?|(?:gcc|llvm)-(?:ar|ranlib|nm)|llvm-(?:strip|objdump|objcopy)|ar|as|ranlib|ld(?:\.(?:lld|gold|bfd))?|dlltool|tapi|readelf|objdump|objcopy|pkg-config|pkgconf|ninja(?:-build)?)(?:-?[0-9]+(?:\.[0-9]+)*)?$")
FAILED_TOOL_LOOKUP = re.compile(r"\b(?:execve|execveat|access|faccessat|faccessat2|stat|lstat|newfstatat|statx)\(.*= -1 ENOENT\b")


def bounded_text(path):
    path = Path(path)
    if not path.is_file() or path.is_symlink() or path.stat().st_size > MAX_BYTES:
        raise ValueError(f"missing, unsafe or oversized evidence: {path}")
    return path.read_text(encoding="utf-8", errors="strict")


def strings(line):
    values = []
    for token in QUOTED.findall(line):
        value = ast.literal_eval(token)
        if isinstance(value, str):
            values.append(value)
    return values


def inspect_trace(path, poison, expected_exec, search_path=None):
    text = bounded_text(path)
    findings = []
    calls = 0
    executions = 0
    expected_seen = False
    host_path_lookups = 0
    path_entries = set((os.environ.get("PATH", "") if search_path is None else search_path).split(os.pathsep))
    poison = str(Path(poison))
    for number, line in enumerate(text.splitlines(), 1):
        # Includes failed access/stat/open probes and shell PATH lookup, not just exec.
        if not re.search(r"\b[a-z][a-z0-9_]*\(", line):
            continue
        calls += 1
        values = strings(line)
        is_exec = bool(re.search(r"\bexecve(?:at)?\(", line))
        if is_exec:
            executions += 1
            if values and values[0] == expected_exec and re.search(r"= 0$", line):
                expected_seen = True
        reasons = set()
        for value in values:
            leaf = value.rsplit("/", 1)[-1]
            if value == poison or value.startswith(poison + "/"):
                reasons.add("poisoned-sdk-path")
            if SDK_ROOT.search(value):
                incidental = (str(Path(value).parent) in path_entries
                              and Path(value).parent.name == "bin"
                              and HOST_TOOL.fullmatch(leaf)
                              and FAILED_TOOL_LOOKUP.search(line))
                if incidental:
                    host_path_lookups += 1
                else:
                    reasons.add("host-sdk-path")
            if (re.match(r"^/usr/(?:include|lib(?:32|64)?|share)/(?:vulkan|cuda|nvidia|rocm|hip)(?:/|$)", value, re.I)
                    or re.match(r"^/etc/vulkan(?:/|$)", value, re.I)):
                reasons.add("system-sdk-path")
            if re.match(r"^/(?:dev/(?:nvidia[^/]*|dri)(?:/|$)|proc/driver/nvidia(?:/|$)|sys/class/drm(?:/|$))", value):
                reasons.add("gpu-device-discovery-path")
            if leaf in GPU_TOOLS:
                reasons.add("gpu-tool-path")
            if GPU_HEADER.fullmatch(leaf) or GPU_LIBRARY.match(leaf):
                reasons.add("gpu-sdk-file")
            if leaf.endswith(".pc") and GPU_PACKAGE.search(leaf):
                reasons.add("gpu-package-metadata")
        if is_exec and values and Path(values[0]).name in ("pkg-config", "pkgconf"):
            if any(GPU_PACKAGE.search(argument) for argument in values[1:]):
                reasons.add("gpu-pkg-config")
        if reasons:
            if len(findings) < 32:
                findings.append({"line": number, "reasons": sorted(reasons), "raw": line[:4096]})
    complete = bool(text.strip()) and calls > 0 and executions > 0 and expected_seen
    if re.search(r"strace:|<detached|\+\+\+ superseded", text):
        complete = False
    return {"observed_syscalls": calls, "observed_exec_calls": executions,
            "expected_exec_observed": expected_seen, "trace_complete": complete,
            "sdk_probe_findings": findings, "finding_limit": 32,
            "incidental_host_path_lookups": host_path_lookups,
            "boundary": "Linux file/process syscalls, including failed probes; environment reads alone and non-filesystem in-process work are not observable"}


def observer_contract():
    poison = "/observer-contract/poison-sdk"
    baseline = '1 execve("/usr/bin/cc", ["cc"], 0x0) = 0\n'
    cases = []
    for tool in ("gmake", "gnm", "gstrip", "dlltool", "tapi", "gcc-ar-16.2",
                 "gcc-ranlib16", "x86_64-pc-linux-gnu-ld.lld", "pkg-config"):
        cases.append((f"host-path-{tool}", f'1 newfstatat(AT_FDCWD, "/opt/cuda/bin/{tool}", 0x0, 0) = -1 ENOENT (No such file or directory)\n', False))
    cases.extend((
        ("host-path-exec-miss", '1 execve("/opt/cuda/bin/gmake", ["gmake"], 0x0) = -1 ENOENT (No such file or directory)\n', False),
        ("host-pkg-config", '1 execve("/usr/bin/pkg-config", ["pkg-config", "--libs", "zlib"], 0x0) = 0\n', False),
        ("unknown-sdk-tool", '1 access("/opt/cuda/bin/sdk-discover", X_OK) = -1 ENOENT (No such file or directory)\n', True),
        ("sdk-tool-present", '1 newfstatat(AT_FDCWD, "/opt/cuda/bin/gmake", 0x0, 0) = 0\n', True),
        ("gpu-tool-path", '1 access("/opt/cuda/bin/nvcc", X_OK) = -1 ENOENT (No such file or directory)\n', True),
        ("sdk-header", '1 openat(AT_FDCWD, "/opt/cuda/include/cuda.h", O_RDONLY) = -1 ENOENT (No such file or directory)\n', True),
        ("sdk-library", '1 openat(AT_FDCWD, "/opt/cuda/lib64/libcudart.so", O_RDONLY) = -1 ENOENT (No such file or directory)\n', True),
        ("sdk-tool-open", '1 openat(AT_FDCWD, "/opt/cuda/bin/gmake", O_RDONLY) = -1 ENOENT (No such file or directory)\n', True),
        ("sdk-tool-executed", '1 execve("/opt/cuda/bin/gmake", ["gmake"], 0x0) = 0\n', True),
        ("sdk-non-path", '1 access("/opt/cuda-99/bin/gmake", X_OK) = -1 ENOENT (No such file or directory)\n', True),
        ("sdk-root", '1 access("/opt/cuda", F_OK) = -1 ENOENT (No such file or directory)\n', True),
        ("pkg-config-vulkan", '1 execve("/usr/bin/pkg-config", ["pkg-config", "--libs", "vulkan"], 0x0) = 0\n', True),
        ("system-library", '1 openat(AT_FDCWD, "/usr/lib/libvulkan.so", O_RDONLY) = 3\n', True),
        ("system-icd-found", '1 openat(AT_FDCWD, "/etc/vulkan/icd.d/nvidia_icd.json", O_RDONLY) = 3\n', True),
        ("system-icd-missing", '1 openat(AT_FDCWD, "/etc/vulkan/icd.d/nvidia_icd.json", O_RDONLY) = -1 ENOENT (No such file or directory)\n', True),
        ("poison-header", f'1 openat(AT_FDCWD, "{poison}/include/cuda.h", O_RDONLY) = -1 ENOENT (No such file or directory)\n', True),
    ))
    results = []
    with tempfile.TemporaryDirectory(prefix="observer-contract-", dir=os.environ.get("TMPDIR")) as directory:
        path = Path(directory) / "case.trace"
        for name, operation, expected in cases:
            path.write_text(baseline + operation, encoding="utf-8")
            observed = inspect_trace(path, poison, "/usr/bin/cc", "/usr/bin:/opt/cuda/bin")
            passed = observed["trace_complete"] and bool(observed["sdk_probe_findings"]) == expected
            results.append({"name": name, "expected_detection": expected, "passed": passed})
        path.write_text(baseline, encoding="utf-8")
        incomplete = inspect_trace(path, poison, "/not-executed", "/usr/bin:/opt/cuda/bin")
        results.append({"name": "missing-execution-fails-closed", "passed": not incomplete["trace_complete"]})
    return {"cases": results, "passed": all(case["passed"] for case in results),
            "boundary": "Synthetic syscall parser contracts only; actual observer calibration is recorded separately"}


def retain_traces(checks_path):
    checks_path = Path(checks_path)
    root = checks_path.parent
    checks = json.loads(bounded_text(checks_path))
    catalog = []
    errors = []
    replacements = {}
    failed_traces = set()
    compressed_total = 0
    # Each raw trace is compressed once, independent of its check references.
    for source in sorted(root.glob("*.trace")):
        if source.is_symlink() or not source.is_file():
            raise ValueError(f"unsafe raw trace: {source}")
        target = source.with_name(source.name + ".gz")
        raw_hash = hashlib.sha256()
        raw_bytes = 0
        try:
            with source.open("rb") as incoming, target.open("xb") as outgoing:
                with gzip.GzipFile(filename="", mode="wb", fileobj=outgoing, mtime=0) as compressed:
                    while chunk := incoming.read(1024 * 1024):
                        raw_hash.update(chunk)
                        raw_bytes += len(chunk)
                        compressed.write(chunk)
            compressed_bytes = target.stat().st_size
            compressed_hash = hashlib.sha256()
            with target.open("rb") as stream:
                while chunk := stream.read(1024 * 1024):
                    compressed_hash.update(chunk)
            bound_reached = raw_bytes >= MAX_BYTES
            retained = (compressed_bytes <= MAX_COMPRESSED_TRACE
                        and compressed_total + compressed_bytes <= MAX_COMPRESSED_TRACES)
            catalog.append({"raw_path": source.name, "raw_bytes": raw_bytes,
                            "raw_sha256": raw_hash.hexdigest(), "path": target.name,
                            "bytes": compressed_bytes, "sha256": compressed_hash.hexdigest(),
                            "encoding": "gzip", "raw_bound_reached": bound_reached,
                            "retained": retained})
            if retained:
                compressed_total += compressed_bytes
                replacements[source.name] = target.name
            else:
                target.unlink()
                replacements[source.name] = "trace-retention.json"
                failed_traces.add(source.name)
                errors.append(f"trace omitted: exceeds 8 MiB/file or 64 MiB/run compressed retention limit: {source.name}")
            source.unlink()
            if bound_reached:
                failed_traces.add(source.name)
                errors.append(f"raw trace reached file limit; completeness not established: {source.name}")
        except (OSError, ValueError) as error:
            failed_traces.add(source.name)
            errors.append(str(error))
    for row in checks:
        retention_failed = any(name in failed_traces for name in row["logs"])
        row["logs"] = list(dict.fromkeys(replacements.get(name, name) for name in row["logs"]))
        if retention_failed:
            if row["exit_status"] == 0:
                row["exit_status"] = 1
            row["details"]["trace_retention_error"] = "Raw trace incomplete or retention failed; see trace-retention.json"
    checks_path.write_text(json.dumps(checks, indent=2) + "\n", encoding="utf-8")
    return {"traces": catalog, "unique_compressed_bytes": compressed_total,
            "per_trace_limit_bytes": MAX_COMPRESSED_TRACE, "total_trace_limit_bytes": MAX_COMPRESSED_TRACES,
            "errors": errors, "passed": not errors,
            "boundary": "Raw syscall bytes retained once as .trace.gz members; both raw and compressed SHA-256 recorded before gate manifest"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    record = sub.add_parser("record")
    record.add_argument("--checks", required=True)
    record.add_argument("--name", required=True)
    record.add_argument("--status", required=True, type=int)
    record.add_argument("--execution", default="none")
    record.add_argument("--classification", default="verified")
    record.add_argument("--detail", default="")
    record.add_argument("logs", nargs="*")
    trace = sub.add_parser("trace")
    trace.add_argument("--trace", required=True)
    trace.add_argument("--poison", required=True)
    trace.add_argument("--expect-exec", required=True)
    trace.add_argument("--expect-detection", action="store_true")
    trace.add_argument("--trap-log")
    elf = sub.add_parser("elf")
    elf.add_argument("--dynamic", required=True)
    trap_log = sub.add_parser("tool-log")
    trap_log.add_argument("--log", required=True)
    diagnostic = sub.add_parser("diagnostic")
    diagnostic.add_argument("--log", required=True)
    diagnostic.add_argument("--status", type=int, required=True)
    diagnostic.add_argument("--code", required=True)
    sub.add_parser("contract")
    retain = sub.add_parser("retain-traces")
    retain.add_argument("--checks", required=True)
    args = parser.parse_args()
    if args.command == "record":
        checks = json.loads(bounded_text(args.checks))
        if not isinstance(checks, list) or len(checks) >= 128 or not 0 <= args.status <= 255:
            raise ValueError("invalid bounded check record")
        if not re.fullmatch(r"[a-z0-9][a-z0-9-]*", args.name):
            raise ValueError("unstable check identity")
        if args.status == 0 and (args.execution == "none" or args.classification == "unsupported"):
            raise ValueError("unexecuted or unsupported required check cannot pass")
        root = Path(args.checks).parent.resolve()
        for log in args.logs:
            candidate = Path(log)
            if candidate.is_absolute() or ".." in candidate.parts:
                raise ValueError("log must be relative and contained")
            bounded_text(root / candidate)
        checks.append({"name": args.name, "exit_status": args.status,
                       "classification": args.classification, "execution": args.execution,
                       "required": True, "logs": args.logs, "details": {"boundary": args.detail}})
        Path(args.checks).write_text(json.dumps(checks, indent=2) + "\n", encoding="utf-8")
        return 0
    if args.command == "trace":
        result = inspect_trace(args.trace, args.poison, args.expect_exec)
        if args.trap_log:
            trap_lines = bounded_text(args.trap_log).splitlines()
            result["observed_tool_trap_lines"] = len(trap_lines)
            result["tool_trap_log_empty"] = not trap_lines
        detected = bool(result["sdk_probe_findings"])
        passed = result["trace_complete"] and (detected if args.expect_detection else not detected)
        if args.trap_log and not args.expect_detection:
            passed = passed and result["tool_trap_log_empty"]
        result["expected_detection"] = args.expect_detection
    elif args.command == "elf":
        text = bounded_text(args.dynamic)
        libraries = re.findall(r"\(NEEDED\).*?\[([^\]]+)\]", text)
        result = {"needed_libraries": libraries, "accelerator_libraries": [lib for lib in libraries if GPU_LIBRARY.match(lib)],
                  "boundary": "ELF NEEDED only; independent syscall traces establish observed SDK discovery isolation"}
        passed = bool(text.strip()) and not result["accelerator_libraries"]
    elif args.command == "tool-log":
        lines = bounded_text(args.log).splitlines()
        passed = not lines
        result = {"observed_tool_trap_lines": len(lines), "tool_trap_log_empty": passed,
                  "boundary": "Rejecting PATH tool tripwires only; no claim about unobserved ASan filesystem/dlopen probes"}
    elif args.command == "contract":
        result = observer_contract()
        passed = result["passed"]
    elif args.command == "retain-traces":
        result = retain_traces(args.checks)
        passed = result["passed"]
    else:
        text = bounded_text(args.log)
        # A timeout/signal/containment rejection must not masquerade as an ownership rejection.
        passed = args.status == 1 and args.code in text
        result = {"compiler_exit_status": args.status, "expected_diagnostic": args.code,
                  "diagnostic_observed": args.code in text}
    result["passed"] = bool(passed)
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0 if passed else 1


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, ValueError, SyntaxError) as error:
        print(json.dumps({"passed": False, "observer_error": str(error)}))
        sys.exit(1)
