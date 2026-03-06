#!/usr/bin/env bash
# Signs release artifacts using cosign (Sigstore) with optional KMS/HSM backend.
#
# Usage:
#   sign_release.sh [--key <cosign-key>] [--kms <kms-uri>] [--keyless] <artifact>...
#
# Modes:
#   --keyless     OIDC-based keyless signing (for CI with GitHub Actions OIDC)
#   --key <path>  Sign with a local cosign key file
#   --kms <uri>   Sign with KMS/HSM backend (e.g., gcpkms://, awskms://, hashivault://, pkcs11://)
#
# Each artifact produces:
#   <artifact>.sha256   - SHA-256 checksum
#   <artifact>.bundle   - Sigstore bundle (signature + certificate)
#
# Examples:
#   # Keyless (CI)
#   sign_release.sh --keyless seen-linux-x64
#
#   # Local key
#   sign_release.sh --key cosign.key seen-linux-x64
#
#   # AWS KMS
#   sign_release.sh --kms awskms:///arn:aws:kms:us-east-1:123456789:key/abcd-1234 seen-linux-x64
#
#   # PKCS#11 HSM
#   sign_release.sh --kms pkcs11:token=mytoken;slot-id=0;pin-value=1234 seen-linux-x64

set -euo pipefail

MODE=""
KEY_PATH=""
KMS_URI=""
ARTIFACTS=()
VERIFY_AFTER_SIGN=true

usage() {
    echo "Usage: $0 [--key <path>] [--kms <uri>] [--keyless] [--no-verify] <artifact>..."
    echo ""
    echo "Options:"
    echo "  --keyless       OIDC-based keyless signing (requires ambient credentials)"
    echo "  --key <path>    Sign with a local cosign key file"
    echo "  --kms <uri>     Sign with KMS/HSM backend"
    echo "  --no-verify     Skip post-sign verification"
    echo "  -h, --help      Show this help"
    exit 1
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --keyless)
            MODE="keyless"
            shift
            ;;
        --key)
            MODE="key"
            KEY_PATH="$2"
            shift 2
            ;;
        --kms)
            MODE="kms"
            KMS_URI="$2"
            shift 2
            ;;
        --no-verify)
            VERIFY_AFTER_SIGN=false
            shift
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

if [[ -z "$MODE" ]]; then
    echo "Error: Must specify --keyless, --key, or --kms"
    usage
fi

if [[ ${#ARTIFACTS[@]} -eq 0 ]]; then
    echo "Error: No artifacts specified"
    usage
fi

# Check cosign is installed
if ! command -v cosign &>/dev/null; then
    echo "Error: cosign is not installed."
    echo "Install: go install github.com/sigstore/cosign/v2/cmd/cosign@latest"
    echo "  or: https://docs.sigstore.dev/system_config/installation/"
    exit 1
fi

FAILED=0

for ARTIFACT in "${ARTIFACTS[@]}"; do
    if [[ ! -f "$ARTIFACT" ]]; then
        echo "Error: Artifact not found: $ARTIFACT"
        FAILED=1
        continue
    fi

    echo "=== Signing: $ARTIFACT ==="

    # Generate SHA-256 checksum
    sha256sum "$ARTIFACT" | awk '{print $1}' > "${ARTIFACT}.sha256"
    echo "  Checksum: $(cat "${ARTIFACT}.sha256")"

    # Sign with cosign
    SIGN_ARGS=(sign-blob --yes --bundle "${ARTIFACT}.bundle")

    case "$MODE" in
        keyless)
            # Keyless uses ambient OIDC credentials (GitHub Actions, etc.)
            ;;
        key)
            SIGN_ARGS+=(--key "$KEY_PATH")
            ;;
        kms)
            SIGN_ARGS+=(--key "$KMS_URI")
            ;;
    esac

    SIGN_ARGS+=("$ARTIFACT")

    if cosign "${SIGN_ARGS[@]}"; then
        echo "  Signed: ${ARTIFACT}.bundle"
    else
        echo "  Error: Failed to sign $ARTIFACT"
        FAILED=1
        continue
    fi

    # Verify signature if requested
    if [[ "$VERIFY_AFTER_SIGN" == "true" ]]; then
        VERIFY_ARGS=(verify-blob --bundle "${ARTIFACT}.bundle")

        case "$MODE" in
            keyless)
                VERIFY_ARGS+=(
                    --certificate-identity-regexp "github.com/.*seenlang"
                    --certificate-oidc-issuer "https://token.actions.githubusercontent.com"
                )
                ;;
            key)
                VERIFY_ARGS+=(--key "${KEY_PATH%.key}.pub")
                ;;
            kms)
                VERIFY_ARGS+=(--key "$KMS_URI")
                ;;
        esac

        VERIFY_ARGS+=("$ARTIFACT")

        if cosign "${VERIFY_ARGS[@]}" 2>/dev/null; then
            echo "  Verified: OK"
        else
            echo "  Warning: Post-sign verification failed"
        fi
    fi

    echo ""
done

if [[ $FAILED -ne 0 ]]; then
    echo "Some artifacts failed to sign."
    exit 1
fi

echo "All artifacts signed successfully."
