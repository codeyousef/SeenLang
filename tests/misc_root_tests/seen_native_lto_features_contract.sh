#!/usr/bin/env bash
# FEL-748: Seen-emitted functions carry no target attributes, so a native
# ThinLTO link must hand LLD the host feature set clang detected. Clang alone
# forwards only mcpu=<host cpu>, whose default features can include extensions
# a VM masks (AVX-512 on x86-64-v3 runners), producing SIGILL.
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-native-lto-features
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"

if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
FIXTURE="$ROOT_DIR/tests/fixtures/fel-748/main.seen"
WORK_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/native-lto-features.XXXXXX")"

cleanup() {
    local status=$?
    case "$WORK_DIR" in
        "$SEEN_ARTIFACT_ROOT"/native-lto-features.*)
            if [ "$status" -eq 0 ] && [ -z "${SEEN_KEEP_TMP:-}" ]; then
                [ -d "$WORK_DIR" ] && [ ! -L "$WORK_DIR" ] &&
                    rm -rf -- "$WORK_DIR"
            else
                echo "Preserved native ThinLTO feature artifacts: $WORK_DIR" >&2
            fi
            ;;
        *) status=1 ;;
    esac
    exit "$status"
}
trap cleanup EXIT

fail() {
    echo "FAIL: FEL-748 native ThinLTO features: $*" >&2
    exit 1
}

case "$(uname -s)/$(uname -m)" in
    Linux/x86_64) ;;
    *) echo "SKIP: FEL-748 native ThinLTO features need a Linux x86-64 host"; exit 0 ;;
esac

compile_release() {
    local output=$1 log=$2
    shift 2
    bash "$ATTESTED_SEEN" "$COMPILER" compile "$FIXTURE" "$output" \
        --release --lto=thin --no-cache --no-fork --verbose "$@" \
        >"$log" 2>&1 || { tail -n 80 "$log" >&2; fail "compile $*"; }
    timeout --foreground --kill-after=5s 60s "$output" >>"$log" 2>&1 ||
        { tail -n 20 "$log" >&2; fail "run $*"; }
    grep -Fq 'PASS: FEL-748 native ThinLTO features' "$log" ||
        fail "fixture result $*"
}

# Native: the link receives exactly the feature set clang detects here.
compile_release "$WORK_DIR/native" "$WORK_DIR/native.log"
selected=$(sed -n 's/^ *Native ThinLTO features: //p' "$WORK_DIR/native.log")
[ -n "$selected" ] || fail "native link did not select host features"
detected=$(clang -march=native -### -x c -c /dev/null -o /dev/null 2>&1 |
    tr ' ' '\n' | grep -A1 -Fx '"-target-feature"' |
    grep -v -Fx -e '"-target-feature"' -e '--' | tr -d '"' | paste -sd,)
[ "$selected" = "$detected" ] ||
    fail "selected features differ from clang -march=native"
case ",$selected," in
    *,+avx512f,*) ;;
    *) if objdump -d --no-show-raw-insn "$WORK_DIR/native" | grep -q zmm; then
           fail "AVX-512 instructions on a host without avx512f"
       fi ;;
esac

# Explicit CPU: no native probe, and x86-64-v3 code never uses AVX-512.
compile_release "$WORK_DIR/v3" "$WORK_DIR/v3.log" --target-cpu=x86-64-v3
if grep -Fq 'Native ThinLTO features:' "$WORK_DIR/v3.log"; then
    fail "explicit target CPU ran the native feature probe"
fi
if objdump -d --no-show-raw-insn "$WORK_DIR/v3" | grep -q zmm; then
    fail "x86-64-v3 build contains AVX-512 instructions"
fi

echo "PASS: FEL-748 native ThinLTO link uses detected host features"
