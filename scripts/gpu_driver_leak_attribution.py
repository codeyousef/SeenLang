#!/usr/bin/env python3
"""Scoped LeakSanitizer attribution for installed GPU driver leaks (FEL-1111).

The owner approved scoped suppressions for leaks that SDK-only C controls
reproduce inside NVIDIA libcuda and the Vulkan loader/ICD without any Seen code.
Native LSan suppressions cannot express this: the NVIDIA ICD is unloaded by
vkDestroyInstance, so its frames are reported as <unknown module>.

A fixture report is accepted only when every leak's immediate allocator caller
is an approved driver/loader module for the backend and the fixture leaks no
more bytes or allocations than the SDK-only control run in the same scope.
Anything else, including any non-leak sanitizer error, remains a failure.
"""
import argparse
from pathlib import Path
import re
import sys

MAX_REPORT = 16 * 1024**2
DRIVER_MODULES = {
    "cuda": re.compile(r"libcuda\.so(\.\d+)*"),
    "vulkan": re.compile(r"<unknown module>|libvulkan\.so(\.\d+)*|libdbus-1\.so(\.\d+)*|"
                         r"libGLX_nvidia\.so(\.\d+)*"),
}
HEADER = re.compile(r"^(Direct|Indirect) leak of (\d+) byte\(s\) in (\d+) object\(s\) allocated from:$")
FRAME = re.compile(r"^\s+#(\d+) 0x[0-9a-f]+(?: in (.+?))?\s+\((<unknown module>|[^()+]+)(?:\+0x[0-9a-f]+)?\)")
SUMMARY = re.compile(r"^SUMMARY: AddressSanitizer: (\d+) byte\(s\) leaked in (\d+) allocation\(s\)\.$")


def require(condition, message):
    if not condition:
        raise ValueError(f"gpu-driver-leak: {message}")


def parse(text):
    """Return leak blocks and the reported totals; reject non-leak errors."""
    lines = text.splitlines()
    for line in lines:
        require(not re.search(r"ERROR: (AddressSanitizer|UndefinedBehaviorSanitizer)", line),
                f"non-leak sanitizer error is never attributable: {line.strip()}")
    blocks, summary, current = [], None, None
    for line in lines:
        header = HEADER.match(line)
        if header:
            current = {"bytes": int(header[2]), "objects": int(header[3]), "frames": []}
            blocks.append(current)
            continue
        frame = FRAME.match(line)
        if frame and current is not None:
            require(int(frame[1]) == len(current["frames"]), "leak stack frames are not contiguous")
            current["frames"].append((frame[2] or "", Path(frame[3]).name if frame[3] != "<unknown module>"
                                      else frame[3]))
            continue
        total = SUMMARY.match(line)
        if total:
            require(summary is None, "multiple leak summaries")
            summary = (int(total[1]), int(total[2]))
        current = None if not line.strip() else current
    if not blocks:
        require(summary is None and "LeakSanitizer" not in text, "leak report has no parseable stacks")
        return [], (0, 0)
    require(summary is not None, "leak report has no summary")
    require(summary == (sum(b["bytes"] for b in blocks), sum(b["objects"] for b in blocks)),
            "leak summary does not equal its parsed stacks")
    return blocks, summary


def allocator_caller(block):
    # LSan records frame #0 as the allocation interceptor; #1 made the allocation.
    frames = block["frames"]
    require(len(frames) >= 2, "leak stack has no allocator caller")
    return frames[1][1]


def attribute(backend, fixture_text, control_text):
    pattern = DRIVER_MODULES[backend]
    control_blocks, control_total = parse(control_text)
    for block in control_blocks:
        require(pattern.fullmatch(allocator_caller(block)),
                f"SDK-only {backend} control leaked outside approved driver modules")
    blocks, total = parse(fixture_text)
    for block in blocks:
        module = allocator_caller(block)
        require(pattern.fullmatch(module),
                f"{block['bytes']}-byte leak allocated from {module}, not an approved {backend} driver module")
    require(total[0] <= control_total[0] and total[1] <= control_total[1],
            f"fixture driver leaks {total[0]} B/{total[1]} allocations exceed the SDK-only control "
            f"{control_total[0]} B/{control_total[1]} allocations")
    return {"backend": backend, "fixture": total, "control": control_total}


def read_reports(paths):
    data = b""
    for path in paths:
        data += Path(path).read_bytes()
        require(len(data) <= MAX_REPORT, "sanitizer reports exceed their bound")
    return data.decode("utf-8", "replace")


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--backend", choices=sorted(DRIVER_MODULES), required=True)
    parser.add_argument("--control", nargs="*", default=[], help="SDK-only control sanitizer reports")
    parser.add_argument("--fixture", nargs="*", default=[], help="fixture sanitizer reports")
    args = parser.parse_args()
    try:
        result = attribute(args.backend, read_reports(args.fixture), read_reports(args.control))
    except (OSError, ValueError) as error:
        print(f"FAIL: {error}", file=sys.stderr)
        return 23
    print(f"gpu-driver-leak-attribution backend={result['backend']} "
          f"fixture={result['fixture'][0]}B/{result['fixture'][1]} "
          f"control={result['control'][0]}B/{result['control'][1]}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
