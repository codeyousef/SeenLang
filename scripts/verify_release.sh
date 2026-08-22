#!/usr/bin/env bash
# Verify a canonical CORE-004B manifest, its four artifacts, and all signatures.
set -euo pipefail

SCRIPT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}" && pwd -P)"
KEY_PATH=""
CERT_IDENTITY="github.com/.*SeenLang"
CERT_ISSUER="https://token.actions.githubusercontent.com"
MANIFEST=""
ARTIFACT_DIR=""

die() { echo "core.004b.unsigned: $*" >&2; exit 1; }
usage() {
    cat >&2 <<'EOF'
Usage: verify_release.sh [--key PATH]
       [--certificate-identity REGEXP] [--certificate-oidc-issuer URL]
       --manifest PATH --artifact-dir DIR
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

VERIFY_PREFIX=(verify-blob)
if [[ -n "$KEY_PATH" ]]; then
    VERIFY_PREFIX+=(--key "$KEY_PATH")
else
    VERIFY_PREFIX+=(--certificate-identity-regexp "$CERT_IDENTITY" --certificate-oidc-issuer "$CERT_ISSUER")
fi
cosign "${VERIFY_PREFIX[@]}" --bundle "$MANIFEST.bundle" "$MANIFEST" >/dev/null 2>&1 || die "manifest signature is invalid"
python3 "$SCRIPT_DIR/check_release_artifact_manifest.py" --manifest "$MANIFEST" --artifact-dir "$ARTIFACT_DIR" >/dev/null

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
