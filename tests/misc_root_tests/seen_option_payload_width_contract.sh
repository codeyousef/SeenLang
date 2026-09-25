#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-option-payload-width
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"

if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
export SEEN_COMPILER_SOURCE_ROOT="$ROOT_DIR"
FIXTURE="$ROOT_DIR/tests/fixtures/fel-1586/typed_option_payload.seen"
VULKAN_FIXTURE="$ROOT_DIR/tests/p3/test_vulkan_readback_wrapper.seen"
WORK_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/option-payload-width.XXXXXX")"

cleanup() {
    local status=$?
    case "$WORK_DIR" in
        "$SEEN_ARTIFACT_ROOT"/option-payload-width.*)
            if [ "$status" -eq 0 ] && [ -z "${SEEN_KEEP_TMP:-}" ]; then
                [ -d "$WORK_DIR" ] && [ ! -L "$WORK_DIR" ] &&
                    rm -rf -- "$WORK_DIR"
            else
                echo "Preserved Option payload artifacts: $WORK_DIR" >&2
            fi
            ;;
        *) status=1 ;;
    esac
    exit "$status"
}
trap cleanup EXIT

run_compiler() {
    timeout --foreground --kill-after=10s 900s \
        bash "$ATTESTED_SEEN" "$COMPILER" "$@"
}

for profile in fast release; do
    binary="$WORK_DIR/typed-option-$profile"
    ir_dir="$WORK_DIR/typed-option-$profile-ir"
    log="$WORK_DIR/typed-option-$profile.log"
    flags=(--no-cache --frozen --target-cpu=x86-64 --jobs 1
        --opt-jobs 1 --emit-module-ir-dir "$ir_dir")
    if [ "$profile" = release ]; then
        flags+=(--release --lto=thin)
    else
        flags+=(--fast)
    fi
    run_compiler check "$FIXTURE" >"$log" 2>&1
    run_compiler compile "$FIXTURE" "$binary" "${flags[@]}" \
        >>"$log" 2>&1
    timeout --foreground --kill-after=5s 120s "$binary" >>"$log" 2>&1
    grep -Fxq 'PASS: typed Option payload conversions' "$log"
    rg --no-ignore -q 'sext i32 .* to i64' "$ir_dir" || {
        echo "FAIL: $profile omitted signed Option payload widening" >&2
        exit 1
    }
    rg --no-ignore -q 'zext i32 .* to i64' "$ir_dir" || {
        echo "FAIL: $profile omitted unsigned Option payload widening" >&2
        exit 1
    }
done

vulkan_ir="$WORK_DIR/vulkan-ir"
run_compiler compile "$VULKAN_FIXTURE" "$WORK_DIR/vulkan-unlinked" \
    --fast --no-cache --frozen --target-cpu=x86-64 --jobs 1 --opt-jobs 1 \
    --emit-module-ir-dir "$vulkan_ir" --stop-after-ir \
    >"$WORK_DIR/vulkan.log" 2>&1
vulkan_module=$(rg --no-ignore -l 'define .*@vk_read_image_rgba8_into\(' \
    "$vulkan_ir" | sed -n '1p')
[ -n "$vulkan_module" ] || {
    echo 'FAIL: Vulkan readback module IR was not emitted' >&2
    exit 1
}
timeout --foreground --kill-after=5s 120s opt -passes=verify \
    -disable-output "$vulkan_module" >>"$WORK_DIR/vulkan.log" 2>&1
rg -q 'sext i32 .* to i64' "$vulkan_module" || {
    echo 'FAIL: Vulkan readback omitted signed result widening' >&2
    exit 1
}

echo 'PASS: FEL-1586 typed Option payloads and Vulkan readback IR'
