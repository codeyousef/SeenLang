#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
VERIFY="$ROOT_DIR/scripts/verify_stdlib_component_payload.sh"
scope="${SEEN_ARTIFACT_ROOT:-$ROOT_DIR/.seen}/stdlib-component-payload"
rm -rf -- "$scope"
mkdir -p "$scope/good/seen_std/src/json" \
    "$scope/bad/seen_std/src/json/build" \
    "$scope/bad/seen_std/src/json/target" \
    "$scope/bad/seen_std/src/json/.seen"
printf 'good\n' > "$scope/good/seen_std/src/json/value.seen"
printf 'temp\n' > "$scope/bad/seen_std/src/json/value.seen.tmp.1"
printf 'build\n' > "$scope/bad/seen_std/src/json/build/object"
printf 'target\n' > "$scope/bad/seen_std/src/json/target/object"
printf 'cache\n' > "$scope/bad/seen_std/src/json/.seen/object"

tar -C "$scope/good" -czf "$scope/good.tar.gz" seen_std
"$VERIFY" "$scope/good.tar.gz" >/dev/null

for prohibited in tmp build target cache; do
    case "$prohibited" in
        tmp) path=seen_std/src/json/value.seen.tmp.1 ;;
        build) path=seen_std/src/json/build ;;
        target) path=seen_std/src/json/target ;;
        cache) path=seen_std/src/json/.seen ;;
    esac
    tar -C "$scope/bad" -czf "$scope/$prohibited.tar.gz" "$path"
    if "$VERIFY" "$scope/$prohibited.tar.gz" >"$scope/$prohibited.out" 2>&1; then
        echo "FAIL: prohibited $prohibited payload was accepted" >&2
        exit 1
    fi
    grep -Eq 'prohibited generated member|unexpected member root' \
        "$scope/$prohibited.out"
done

grep -Fq 'tracked_stdlib_files' "$ROOT_DIR/scripts/build_release.sh"
grep -Fq 'verify_stdlib_component_payload.sh' \
    "$ROOT_DIR/scripts/build_and_upload_release.sh"
echo "PASS: standalone stdlib payload rejects generated debris"
