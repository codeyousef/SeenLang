#!/usr/bin/env bash
# Sign, verify, and pin exactly the compiler/runtime/stdlib/package-client set.
set -euo pipefail

SCRIPT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}" && pwd -P)"
MODE=""
KEY_PATH=""
KMS_URI=""
VERSION=""
SOURCE_COMMIT=""
SOURCE_DIGEST=""
TARGET="linux-x86_64"
MANIFEST=""
IDENTITY=""
ISSUER=""
ARTIFACTS=()

die() { echo "core.004b.invalid: $*" >&2; exit 1; }
usage() {
    cat >&2 <<'EOF'
Usage: sign_release.sh (--keyless | --key PATH | --kms URI)
       --version VERSION --source-commit SHA1 --source-digest SHA256
       --manifest PATH --signer-identity TEXT --signer-issuer TEXT
       --artifact compiler=PATH --artifact runtime=PATH
       --artifact stdlib=PATH --artifact package-client=PATH

Every artifact is checksummed, signed, and verified. The canonical manifest is
then generated, checksummed, signed, and verified. No verification bypass exists.
Keyless identity must be the anchored, escaped release.yml identity for VERSION.
EOF
    exit 1
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --keyless) MODE="keyless"; shift ;;
        --key) [[ $# -ge 2 ]] || usage; MODE="key"; KEY_PATH="$2"; shift 2 ;;
        --kms) [[ $# -ge 2 ]] || usage; MODE="kms"; KMS_URI="$2"; shift 2 ;;
        --version) [[ $# -ge 2 ]] || usage; VERSION="$2"; shift 2 ;;
        --source-commit) [[ $# -ge 2 ]] || usage; SOURCE_COMMIT="$2"; shift 2 ;;
        --source-digest) [[ $# -ge 2 ]] || usage; SOURCE_DIGEST="$2"; shift 2 ;;
        --target) [[ $# -ge 2 ]] || usage; TARGET="$2"; shift 2 ;;
        --manifest) [[ $# -ge 2 ]] || usage; MANIFEST="$2"; shift 2 ;;
        --signer-identity) [[ $# -ge 2 ]] || usage; IDENTITY="$2"; shift 2 ;;
        --signer-issuer) [[ $# -ge 2 ]] || usage; ISSUER="$2"; shift 2 ;;
        --artifact) [[ $# -ge 2 ]] || usage; ARTIFACTS+=("$2"); shift 2 ;;
        -h|--help) usage ;;
        *) die "unknown option: $1" ;;
    esac
done

[[ -n "$MODE" && -n "$VERSION" && -n "$SOURCE_COMMIT" && -n "$SOURCE_DIGEST" && -n "$MANIFEST" && -n "$IDENTITY" && -n "$ISSUER" ]] || usage
[[ "${#ARTIFACTS[@]}" -eq 4 ]] || die "exactly four --artifact arguments are required"
if ! [[ "$VERSION" =~ ^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-[0-9A-Za-z]+([.-][0-9A-Za-z]+)*)?$ ]]; then
    die "release version is not a supported semantic version"
fi
if [[ "$MODE" == "keyless" ]]; then
    ESCAPED_VERSION="${VERSION//./\\.}"
    EXPECTED_IDENTITY="^https://github\\.com/codeyousef/SeenLang/\\.github/workflows/release\\.yml@refs/tags/v${ESCAPED_VERSION}\$"
    [[ "$IDENTITY" == "$EXPECTED_IDENTITY" ]] ||
        die "keyless signer identity must be the exact anchored release.yml tag identity"
    [[ "$ISSUER" == "https://token.actions.githubusercontent.com" ]] ||
        die "keyless signer issuer must be the GitHub Actions OIDC issuer"
fi
EXPECTED=(compiler runtime stdlib package-client)
PATHS=()
for index in 0 1 2 3; do
    role="${ARTIFACTS[$index]%%=*}"
    path="${ARTIFACTS[$index]#*=}"
    [[ "$role" == "${EXPECTED[$index]}" && "$path" != "${ARTIFACTS[$index]}" ]] || die "artifact roles must be ordered compiler, runtime, stdlib, package-client"
    [[ -f "$path" && ! -L "$path" && -s "$path" ]] || die "artifact is missing, empty, or a symlink: $role"
    PATHS+=("$path")
done
command -v cosign >/dev/null 2>&1 || die "cosign is required"

make_sign_args() {
    SIGN_ARGS=(sign-blob --yes --bundle "$1.bundle")
    case "$MODE" in
        keyless) ;;
        key) SIGN_ARGS+=(--key "$KEY_PATH") ;;
        kms) SIGN_ARGS+=(--key "$KMS_URI") ;;
        *) die "unsupported signer mode" ;;
    esac
    SIGN_ARGS+=("$1")
}

make_verify_args() {
    VERIFY_ARGS=(verify-blob --bundle "$1.bundle")
    case "$MODE" in
        keyless) VERIFY_ARGS+=(--certificate-identity-regexp "$IDENTITY" --certificate-oidc-issuer "$ISSUER") ;;
        key) VERIFY_ARGS+=(--key "${SEEN_COSIGN_PUBLIC_KEY:-${KEY_PATH%.key}.pub}") ;;
        kms) VERIFY_ARGS+=(--key "$KMS_URI") ;;
    esac
    VERIFY_ARGS+=("$1")
}

for artifact in "${PATHS[@]}"; do
    sha256sum "$artifact" | awk '{print $1}' > "$artifact.sha256"
    make_sign_args "$artifact"
    cosign "${SIGN_ARGS[@]}" >/dev/null || die "signing failed: $(basename "$artifact")"
    make_verify_args "$artifact"
    cosign "${VERIFY_ARGS[@]}" >/dev/null 2>&1 || die "post-sign verification failed: $(basename "$artifact")"
done

GENERATOR_ARGS=(--output "$MANIFEST" --version "$VERSION" --target "$TARGET"
    --source-commit "$SOURCE_COMMIT" --source-digest "$SOURCE_DIGEST"
    --signer-mode "$MODE" --signer-identity "$IDENTITY" --signer-issuer "$ISSUER")
for artifact in "${ARTIFACTS[@]}"; do GENERATOR_ARGS+=(--artifact "$artifact"); done
"$SCRIPT_DIR/generate_release_manifest.sh" "${GENERATOR_ARGS[@]}" >/dev/null
sha256sum "$MANIFEST" | awk '{print $1}' > "$MANIFEST.sha256"
make_sign_args "$MANIFEST"
cosign "${SIGN_ARGS[@]}" >/dev/null || die "manifest signing failed"
make_verify_args "$MANIFEST"
cosign "${VERIFY_ARGS[@]}" >/dev/null 2>&1 || die "manifest post-sign verification failed"

VERIFY_MODE=()
case "$MODE" in
    keyless) VERIFY_MODE+=(--certificate-identity "$IDENTITY" --certificate-oidc-issuer "$ISSUER") ;;
    key) VERIFY_MODE+=(--key "${SEEN_COSIGN_PUBLIC_KEY:-${KEY_PATH%.key}.pub}") ;;
    kms) VERIFY_MODE+=(--key "$KMS_URI") ;;
esac
"$SCRIPT_DIR/verify_release.sh" "${VERIFY_MODE[@]}" --manifest "$MANIFEST" --artifact-dir "$(dirname "${PATHS[0]}")"
echo "PASS: signed and pinned compiler, runtime, stdlib, and package-client artifacts"
