#!/usr/bin/env bash
# Select one exact-version quick/verify bootstrap builder.  This helper only
# performs version/sidecar handshakes; it never compiles or retries a builder.

set -euo pipefail

REPO_ROOT=""
CHECKOUT_VERSION=""
SOURCE_SIDECAR=""
EXPLICIT_BUILDER=""

usage() {
    cat <<'EOF'
Usage: rebuild_builder_selection.sh --repo-root PATH --checkout-version VERSION
       --source-sidecar PATH [--explicit-builder PATH]

Selects exactly one quick/verify builder. An explicit builder wins; otherwise
the first present path is chosen in this order:
  compiler_seen/target/seen
  target/release/seen
  stage3_recovery_head

The selected compiler must report exactly "Seen VERSION" on the first line of
--version, and SOURCE-SIDECAR must pass the exact SEENPKG1 VERSION handshake.
The selected path is printed to stdout. Selection or handshake failure is
terminal; this helper never falls through after choosing a present candidate.
EOF
}

fail() {
    echo "builder selection: $*" >&2
    exit 1
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --repo-root)
            [ "$#" -ge 2 ] || fail "--repo-root requires a path"
            REPO_ROOT=$2
            shift 2
            ;;
        --checkout-version)
            [ "$#" -ge 2 ] || fail "--checkout-version requires a value"
            CHECKOUT_VERSION=$2
            shift 2
            ;;
        --source-sidecar)
            [ "$#" -ge 2 ] || fail "--source-sidecar requires a path"
            SOURCE_SIDECAR=$2
            shift 2
            ;;
        --explicit-builder)
            [ "$#" -ge 2 ] || fail "--explicit-builder requires a path"
            EXPLICIT_BUILDER=$2
            shift 2
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            fail "unknown option: $1"
            ;;
    esac
done

[ -n "$REPO_ROOT" ] || fail "--repo-root is required"
[ -n "$CHECKOUT_VERSION" ] || fail "--checkout-version is required"
[ -n "$SOURCE_SIDECAR" ] || fail "--source-sidecar is required"
case "$CHECKOUT_VERSION" in
    *[!0-9A-Za-z.+-]*|'') fail "invalid checkout version: $CHECKOUT_VERSION" ;;
esac

[ -d "$REPO_ROOT" ] && [ ! -L "$REPO_ROOT" ] ||
    fail "repository root is not a real directory: $REPO_ROOT"
REPO_ROOT=$(cd "$REPO_ROOT" && pwd -P) ||
    fail "could not resolve repository root"

case "$SOURCE_SIDECAR" in
    /*) ;;
    *) SOURCE_SIDECAR="$REPO_ROOT/$SOURCE_SIDECAR" ;;
esac
[ -f "$SOURCE_SIDECAR" ] && [ -x "$SOURCE_SIDECAR" ] &&
    [ ! -L "$SOURCE_SIDECAR" ] ||
    fail "source package sidecar is not a regular executable: $SOURCE_SIDECAR"
SOURCE_SIDECAR=$(cd "$(dirname -- "$SOURCE_SIDECAR")" && pwd -P)/$(basename -- "$SOURCE_SIDECAR")

candidate=""
if [ -n "$EXPLICIT_BUILDER" ]; then
    case "$EXPLICIT_BUILDER" in
        /*) candidate=$EXPLICIT_BUILDER ;;
        *) candidate="$REPO_ROOT/$EXPLICIT_BUILDER" ;;
    esac
else
    for relative in \
        compiler_seen/target/seen \
        target/release/seen \
        stage3_recovery_head; do

        path="$REPO_ROOT/$relative"
        if [ -e "$path" ] || [ -L "$path" ]; then
            candidate=$path
            break
        fi
    done
fi

[ -n "$candidate" ] || fail "no preferred checkout-version builder is present"
[ -f "$candidate" ] && [ -x "$candidate" ] && [ ! -L "$candidate" ] ||
    fail "selected builder is not a regular executable: $candidate"
candidate=$(cd "$(dirname -- "$candidate")" && pwd -P)/$(basename -- "$candidate")

command -v timeout >/dev/null 2>&1 ||
    fail "timeout is required for the bounded version probe"
set +e
version_output=$(timeout 10 "$candidate" --version 2>&1)
version_status=$?
set -e
[ "$version_status" -eq 0 ] ||
    fail "selected builder --version failed (status $version_status): $candidate"
first_line=${version_output%%$'\n'*}
[ "$first_line" = "Seen $CHECKOUT_VERSION" ] ||
    fail "selected builder version mismatch: expected 'Seen $CHECKOUT_VERSION', got '$first_line'"

set +e
sidecar_output=$(timeout 10 "$SOURCE_SIDECAR" \
    --expect-version "$CHECKOUT_VERSION" version --machine 2>&1)
sidecar_status=$?
set -e
expected_sidecar_output=$(printf 'protocol=SEENPKG1\nversion=%s' \
    "$CHECKOUT_VERSION")
[ "$sidecar_status" -eq 0 ] &&
    [ "$sidecar_output" = "$expected_sidecar_output" ] ||
    fail "source package sidecar handshake failed for Seen $CHECKOUT_VERSION"

printf '%s\n' "$candidate"
