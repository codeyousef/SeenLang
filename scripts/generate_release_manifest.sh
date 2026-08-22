#!/usr/bin/env bash
# Generate the canonical CORE-004B four-component release manifest.
set -euo pipefail

SCRIPT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}" && pwd -P)"

usage() {
    cat >&2 <<'EOF'
Usage: generate_release_manifest.sh --output PATH --version VERSION
       --source-commit SHA1 --source-digest SHA256
       --signer-mode keyless|key|kms --signer-identity TEXT --signer-issuer TEXT
       --artifact compiler=PATH --artifact runtime=PATH
       --artifact stdlib=PATH --artifact package-client=PATH
EOF
    exit 1
}

[[ $# -gt 0 ]] || usage
exec python3 "$SCRIPT_DIR/check_release_artifact_manifest.py" "$@"
