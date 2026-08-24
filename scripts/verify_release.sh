#!/usr/bin/env bash
# Verify a canonical CORE-004B manifest, its four artifacts, and all signatures.
set -euo pipefail

SCRIPT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}" && pwd -P)"
KEY_PATH=""
CERT_IDENTITY=""
CERT_ISSUER="https://token.actions.githubusercontent.com"
MANIFEST=""
ARTIFACT_DIR=""

die() { echo "core.004b.unsigned: $*" >&2; exit 1; }
usage() {
    cat >&2 <<'EOF'
Usage: verify_release.sh [--key PATH]
       [--certificate-identity EXACT-REGEXP] [--certificate-oidc-issuer URL]
       --manifest PATH --artifact-dir DIR

Keyless verification derives the exact release.yml tag identity from the
manifest version. An override must equal that derived anchored expression.
EOF
    exit 1
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --key) [[ $# -ge 2 ]] || usage; KEY_PATH="$2"; shift 2 ;;
        --certificate-identity) [[ $# -ge 2 ]] || usage; CERT_IDENTITY="$2"; shift 2 ;;
        --certificate-oidc-issuer) [[ $# -ge 2 ]] || usage; CERT_ISSUER="$2"; shift 2 ;;
        --manifest) [[ $# -ge 2 ]] || usage; MANIFEST="$2"; shift 2 ;;
        --artifact-dir) [[ $# -ge 2 ]] || usage; ARTIFACT_DIR="$2"; shift 2 ;;
        -h|--help) usage ;;
        *) die "unknown option: $1" ;;
    esac
done
[[ -n "$MANIFEST" && -n "$ARTIFACT_DIR" ]] || usage
command -v cosign >/dev/null 2>&1 || die "cosign is required"
[[ -f "$MANIFEST" && ! -L "$MANIFEST" && -s "$MANIFEST" ]] || die "manifest is missing, empty, or a symlink"
[[ -f "$MANIFEST.sha256" && ! -L "$MANIFEST.sha256" ]] || die "manifest checksum is missing"
[[ -f "$MANIFEST.bundle" && ! -L "$MANIFEST.bundle" ]] || die "manifest signature bundle is missing"
EXPECTED="$(cat "$MANIFEST.sha256")"
ACTUAL="$(sha256sum "$MANIFEST" | awk '{print $1}')"
[[ "$EXPECTED" == "$ACTUAL" ]] || die "manifest checksum mismatch"
python3 "$SCRIPT_DIR/check_release_artifact_manifest.py" \
    --manifest "$MANIFEST" --artifact-dir "$ARTIFACT_DIR" >/dev/null

mapfile -t MANIFEST_POLICY < <(python3 - "$MANIFEST" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as source:
    manifest = json.load(source)
print(manifest["version"])
print(manifest["signer"]["mode"])
print(manifest["signer"]["identity"])
print(manifest["signer"]["issuer"])
PY
)
[[ "${#MANIFEST_POLICY[@]}" -eq 4 ]] || die "manifest signing policy is unreadable"
MANIFEST_VERSION="${MANIFEST_POLICY[0]}"
MANIFEST_SIGNER_MODE="${MANIFEST_POLICY[1]}"
MANIFEST_SIGNER_IDENTITY="${MANIFEST_POLICY[2]}"
MANIFEST_SIGNER_ISSUER="${MANIFEST_POLICY[3]}"

VERIFY_PREFIX=(verify-blob)
if [[ -n "$KEY_PATH" ]]; then
    VERIFY_PREFIX+=(--key "$KEY_PATH")
else
    [[ "$MANIFEST_VERSION" =~ ^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-[0-9A-Za-z]+([.-][0-9A-Za-z]+)*)?$ ]] ||
        die "manifest version cannot form an exact certificate identity"
    ESCAPED_VERSION="${MANIFEST_VERSION//./\\.}"
    EXPECTED_CERT_IDENTITY="^https://github\\.com/codeyousef/SeenLang/\\.github/workflows/release\\.yml@refs/tags/v${ESCAPED_VERSION}\$"
    [[ "$MANIFEST_SIGNER_MODE" == "keyless" ]] ||
        die "certificate verification requires a keyless manifest"
    [[ "$MANIFEST_SIGNER_IDENTITY" == "$EXPECTED_CERT_IDENTITY" ]] ||
        die "manifest signer identity is not the exact anchored release.yml tag identity"
    [[ "$MANIFEST_SIGNER_ISSUER" == "https://token.actions.githubusercontent.com" ]] ||
        die "manifest signer issuer is not the GitHub Actions OIDC issuer"
    if [[ -n "$CERT_IDENTITY" && "$CERT_IDENTITY" != "$EXPECTED_CERT_IDENTITY" ]]; then
        die "certificate identity override would weaken the exact release tag policy"
    fi
    [[ "$CERT_ISSUER" == "https://token.actions.githubusercontent.com" ]] ||
        die "certificate issuer override would weaken the GitHub Actions OIDC policy"
    CERT_IDENTITY="$EXPECTED_CERT_IDENTITY"
    VERIFY_PREFIX+=(--certificate-identity-regexp "$CERT_IDENTITY" --certificate-oidc-issuer "$CERT_ISSUER")
fi
cosign "${VERIFY_PREFIX[@]}" --bundle "$MANIFEST.bundle" "$MANIFEST" >/dev/null 2>&1 || die "manifest signature is invalid"

while IFS= read -r name; do
    artifact="$ARTIFACT_DIR/$name"
    cosign "${VERIFY_PREFIX[@]}" --bundle "$artifact.bundle" "$artifact" >/dev/null 2>&1 || die "artifact signature is invalid: $name"
done < <(python3 - "$MANIFEST" <<'PY'
import json, sys
with open(sys.argv[1], encoding="utf-8") as source:
    for artifact in json.load(source)["artifacts"]:
        print(artifact["name"])
PY
)
echo "Verified OK: canonical manifest and four signed artifacts"
