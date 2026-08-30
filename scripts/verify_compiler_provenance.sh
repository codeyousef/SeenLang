#!/usr/bin/env bash
# Verify that a Seen compiler is the exact immutable component named by a
# release package's bounded provenance manifest.

set -euo pipefail

fail() {
    echo "compiler-provenance: invalid: $*" >&2
    exit 1
}

if [ "$#" -lt 2 ] || [ "$#" -gt 3 ]; then
    echo "Usage: $0 <compiler-provenance.env> <compiler> [expected-version]" >&2
    exit 2
fi

manifest=$1
compiler=$2
expected_version=${3:-}

[ -f "$manifest" ] && [ ! -L "$manifest" ] || fail "manifest is missing or unsafe"
[ -f "$compiler" ] && [ -x "$compiler" ] && [ ! -L "$compiler" ] ||
    fail "compiler is missing, non-executable, or unsafe"
[ "$(wc -c < "$manifest" | tr -d ' ')" -le 4096 ] || fail "manifest exceeds 4096 bytes"

seen_compiler_provenance_version=
release_version=
source_commit=
source_asset=
compiler_sha256=
compiler_size=
compiler_build_id=
cpu_baseline=
seen_keys=

while IFS='=' read -r key value; do
    [ -n "$key" ] || fail "manifest contains an empty key"
    case "$key" in
        seen_compiler_provenance_version|release_version|source_commit|source_asset|compiler_sha256|compiler_size|compiler_build_id|cpu_baseline)
            case " $seen_keys " in *" $key "*) fail "duplicate key: $key" ;; esac
            seen_keys="$seen_keys $key"
            printf -v "$key" '%s' "$value"
            ;;
        *) fail "unknown key: $key" ;;
    esac
done < "$manifest"

[ "$seen_compiler_provenance_version" = 1 ] || fail "unsupported manifest version"
[[ "$release_version" =~ ^[0-9]+\.[0-9]+\.[0-9]+([.-][0-9A-Za-z.-]+)?$ ]] ||
    fail "invalid release version"
[[ "$source_commit" =~ ^[0-9a-f]{40}$ ]] || fail "invalid source commit"
case "$source_asset" in
    "seen-compiler-$release_version-linux-x64"|"seen-compiler-$release_version-linux-x64-v3") ;;
    *) fail "source asset is not a signed Linux compiler component" ;;
esac
[[ "$compiler_sha256" =~ ^[0-9a-f]{64}$ ]] || fail "invalid compiler SHA-256"
[[ "$compiler_size" =~ ^[1-9][0-9]*$ ]] || fail "invalid compiler size"
[[ "$compiler_build_id" =~ ^([0-9a-f]+|none)$ ]] || fail "invalid compiler build ID"
case "$cpu_baseline:$source_asset" in
    "x86-64:seen-compiler-$release_version-linux-x64"|"x86-64-v3:seen-compiler-$release_version-linux-x64-v3") ;;
    *) fail "CPU baseline and source asset disagree" ;;
esac
if [ -n "$expected_version" ] && [ "$release_version" != "$expected_version" ]; then
    fail "release version $release_version does not match expected $expected_version"
fi

actual_sha256=$(sha256sum "$compiler" | awk '{print $1}')
[ "$actual_sha256" = "$compiler_sha256" ] ||
    fail "compiler SHA-256 mismatch: expected $compiler_sha256, got $actual_sha256"
actual_size=$(wc -c < "$compiler" | tr -d ' ')
[ "$actual_size" = "$compiler_size" ] ||
    fail "compiler size mismatch: expected $compiler_size, got $actual_size"

version_output=$("$compiler" --version 2>/dev/null) || fail "compiler version probe failed"
IFS=$'\n' read -r version_line _ <<< "$version_output"
[ "$version_line" = "Seen $release_version" ] ||
    fail "compiler version mismatch: expected Seen $release_version, got ${version_line:-empty}"

if [ "$compiler_build_id" != none ] && command -v readelf >/dev/null 2>&1; then
    actual_build_id=$(readelf -n "$compiler" 2>/dev/null |
        awk '/Build ID:/ {print tolower($3); exit}')
    [ "$actual_build_id" = "$compiler_build_id" ] ||
        fail "compiler build ID mismatch: expected $compiler_build_id, got ${actual_build_id:-none}"
fi

echo "compiler-provenance: verified source_asset=$source_asset sha256=$compiler_sha256"
