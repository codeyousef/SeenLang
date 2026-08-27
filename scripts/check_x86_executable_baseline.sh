#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 2 ]; then
    echo "Usage: check_x86_executable_baseline.sh <x86-64|x86-64-v3> <executable>" >&2
    exit 2
fi

baseline=$1
executable=$2
case "$baseline" in
    x86-64|x86-64-v3) ;;
    *)
        echo "FAIL: unsupported executable CPU baseline: $baseline" >&2
        exit 2
        ;;
esac
[ -f "$executable" ] && [ -x "$executable" ] || {
    echo "FAIL: baseline executable is unavailable: $executable" >&2
    exit 1
}
command -v file >/dev/null 2>&1 && command -v objdump >/dev/null 2>&1 || {
    echo "FAIL: executable baseline audit requires file and objdump" >&2
    exit 1
}
file "$executable" | grep -Eq 'ELF 64-bit.*x86-64' || {
    echo "FAIL: executable baseline audit expected an x86-64 ELF binary" >&2
    exit 1
}

# x86 instructions are at most 15 bytes. A width of 16 keeps raw-byte
# continuations from being mistaken for instruction starts. In 64-bit mode
# c4/c5 are VEX prefixes and 62 is the EVEX prefix.
if ! objdump -d --insn-width=16 "$executable" | awk -v baseline="$baseline" '
    /^[[:space:]]*[[:xdigit:]]+:/ {
        first = tolower($2)
        lower = tolower($0)
        if (baseline == "x86-64" &&
            (first == "c4" || first == "c5" || first == "62" ||
             lower ~ /%ymm[0-9]+|%zmm[0-9]+|%k[0-7]([^[:alnum:]_]|$)/)) {
            print "FAIL: x86-64 executable contains VEX/EVEX instruction: " $0 > "/dev/stderr"
            failed = 1
        }
        if (baseline == "x86-64-v3" &&
            (first == "62" ||
             lower ~ /%zmm[0-9]+|%k[0-7]([^[:alnum:]_]|$)/)) {
            print "FAIL: x86-64-v3 executable contains AVX-512 instruction: " $0 > "/dev/stderr"
            failed = 1
        }
    }
    END { exit failed ? 1 : 0 }
'; then
    exit 1
fi

echo "PASS: executable satisfies $baseline instruction baseline"
