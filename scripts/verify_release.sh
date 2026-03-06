#!/usr/bin/env bash
# Verifies release artifact signatures using cosign (Sigstore).
#
# Usage:
#   verify_release.sh [--key <path>] [--certificate-identity <id>] <artifact>...
#
# Modes:
#   (default)     Keyless verification using Sigstore transparency log
#   --key <path>  Verify with a local cosign public key
#
# Examples:
#   # Keyless verification (CI-signed artifacts)
#   verify_release.sh seen-linux-x64
#
#   # Key-based verification
#   verify_release.sh --key cosign.pub seen-linux-x64

set -euo pipefail

KEY_PATH=""
CERT_IDENTITY="github.com/.*seenlang"
CERT_ISSUER="https://token.actions.githubusercontent.com"
ARTIFACTS=()

usage() {
    echo "Usage: $0 [--key <path>] [--certificate-identity <regex>] [--certificate-oidc-issuer <url>] <artifact>..."
    echo ""
    echo "Options:"
    echo "  --key <path>                     Verify with a local cosign public key"
    echo "  --certificate-identity <regex>   Certificate identity pattern (default: github.com/.*seenlang)"
    echo "  --certificate-oidc-issuer <url>  OIDC issuer URL (default: GitHub Actions)"
    echo "  -h, --help                       Show this help"
    exit 1
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --key)
            KEY_PATH="$2"
            shift 2
            ;;
        --certificate-identity)
            CERT_IDENTITY="$2"
            shift 2
            ;;
        --certificate-oidc-issuer)
            CERT_ISSUER="$2"
            shift 2
            ;;
        -h|--help)
            usage
            ;;
        -*)
            echo "Error: Unknown option $1"
            usage
            ;;
        *)
            ARTIFACTS+=("$1")
            shift
            ;;
    esac
done

if [[ ${#ARTIFACTS[@]} -eq 0 ]]; then
    echo "Error: No artifacts specified"
    usage
fi

if ! command -v cosign &>/dev/null; then
    echo "Error: cosign is not installed."
    echo "Install: go install github.com/sigstore/cosign/v2/cmd/cosign@latest"
    exit 1
fi

PASSED=0
FAILED=0

for ARTIFACT in "${ARTIFACTS[@]}"; do
    if [[ ! -f "$ARTIFACT" ]]; then
        echo "FAIL: Artifact not found: $ARTIFACT"
        FAILED=$((FAILED + 1))
        continue
    fi

    echo "=== Verifying: $ARTIFACT ==="

    # Verify SHA-256 checksum if available
    if [[ -f "${ARTIFACT}.sha256" ]]; then
        EXPECTED=$(cat "${ARTIFACT}.sha256")
        ACTUAL=$(sha256sum "$ARTIFACT" | awk '{print $1}')
        if [[ "$EXPECTED" == "$ACTUAL" ]]; then
            echo "  Checksum: OK"
        else
            echo "  Checksum: MISMATCH"
            echo "    Expected: $EXPECTED"
            echo "    Actual:   $ACTUAL"
            FAILED=$((FAILED + 1))
            continue
        fi
    else
        echo "  Checksum: skipped (no .sha256 file)"
    fi

    # Verify Sigstore signature
    if [[ ! -f "${ARTIFACT}.bundle" ]]; then
        echo "  Signature: MISSING (no .bundle file)"
        FAILED=$((FAILED + 1))
        continue
    fi

    VERIFY_ARGS=(verify-blob --bundle "${ARTIFACT}.bundle")

    if [[ -n "$KEY_PATH" ]]; then
        VERIFY_ARGS+=(--key "$KEY_PATH")
    else
        VERIFY_ARGS+=(
            --certificate-identity-regexp "$CERT_IDENTITY"
            --certificate-oidc-issuer "$CERT_ISSUER"
        )
    fi

    VERIFY_ARGS+=("$ARTIFACT")

    if cosign "${VERIFY_ARGS[@]}" 2>/dev/null; then
        echo "  Signature: OK"
        PASSED=$((PASSED + 1))
    else
        echo "  Signature: INVALID"
        FAILED=$((FAILED + 1))
    fi

    echo ""
done

echo "Results: $PASSED passed, $FAILED failed"

if [[ $FAILED -ne 0 ]]; then
    exit 1
fi
