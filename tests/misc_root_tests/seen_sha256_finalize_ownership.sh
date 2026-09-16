#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
SCOPE=seen-sha256-finalize-ownership
CAPPED="$ROOT_DIR/scripts/run_capped_regression.sh"

if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$CAPPED" "$SCOPE" --compiler "$COMPILER" -- bash "$0"
fi

COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
bash "$CAPPED" --verify-active "$SCOPE" --compiler "$COMPILER"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
SOURCE="$ROOT_DIR/tests/fixtures/sha256_finalize_ownership.seen"
WORK="$(mktemp -d "$SEEN_ARTIFACT_ROOT/sha256-ownership.XXXXXX")"

for profile in fast release; do
    args=(compile "$SOURCE" "$WORK/$profile" --no-cache --jobs 1 --opt-jobs 1)
    if [ "$profile" = fast ]; then
        args+=(--fast)
    else
        args+=(--release --lto=thin --target-cpu=x86-64)
    fi
    if ! timeout --foreground --kill-after=10s 900s \
        bash "$ATTESTED_SEEN" "$COMPILER" "${args[@]}" \
        >"$WORK/$profile.compile.log" 2>&1; then
        tail -c 32768 "$WORK/$profile.compile.log" >&2
        echo "FAIL: $profile SHA-256 ownership compilation" >&2
        exit 1
    fi
    if ! timeout --foreground --kill-after=5s 60s \
        "$WORK/$profile" >"$WORK/$profile.run.log" 2>&1; then
        tail -c 32768 "$WORK/$profile.run.log" >&2
        echo "FAIL: $profile SHA-256 allocator baseline" >&2
        exit 1
    fi
    grep -Fxq 'PASS: SHA-256 allocator baseline' "$WORK/$profile.run.log" || {
        echo "FAIL: $profile SHA-256 ownership result is missing" >&2
        exit 1
    }
done

echo 'PASS: fast and release/ThinLTO SHA-256 ownership'
