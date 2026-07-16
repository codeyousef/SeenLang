#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TMP_DIR="$(mktemp -d /tmp/seen_build_installers_pkg.XXXXXX)"
trap 'rm -rf "$TMP_DIR"' EXIT

FIXTURE_ROOT="$TMP_DIR/repo"
CALL_LOG="$TMP_DIR/installer-calls.log"
mkdir -p "$FIXTURE_ROOT/scripts" "$FIXTURE_ROOT/installer/linux"
cp "$ROOT_DIR/scripts/build_installers.sh" "$FIXTURE_ROOT/scripts/build_installers.sh"

cat > "$FIXTURE_ROOT/stage3_seen" <<'STAGE3_EOF'
#!/usr/bin/env bash
exit 0
STAGE3_EOF
chmod +x "$FIXTURE_ROOT/stage3_seen"

cat > "$FIXTURE_ROOT/scripts/build_package_client.sh" <<'BUILD_EOF'
#!/usr/bin/env bash
set -euo pipefail

version=""
goos=""
goarch=""
output=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --version) version="$2"; shift 2 ;;
        --goos) goos="$2"; shift 2 ;;
        --goarch) goarch="$2"; shift 2 ;;
        --output) output="$2"; shift 2 ;;
        *) exit 2 ;;
    esac
done
[[ "$version" == "0.10.0" && "$goos" == "linux" && "$goarch" == "amd64" ]]
cat > "$output" <<'HELPER_EOF'
#!/usr/bin/env bash
if [[ "${1:-}" == "--expect-version" && "${2:-}" == "0.10.0" &&
      "${3:-}" == "version" && "${4:-}" == "--machine" ]]; then
    printf 'protocol=SEENPKG1\nversion=0.10.0\n'
    exit 0
fi
exit 1
HELPER_EOF
chmod +x "$output"
BUILD_EOF
chmod +x "$FIXTURE_ROOT/scripts/build_package_client.sh"

cat > "$FIXTURE_ROOT/installer/linux/fake-builder.sh" <<'BUILDER_EOF'
#!/usr/bin/env bash
set -euo pipefail

source_dir=""
while [[ $# -gt 0 ]]; do
    if [[ "$1" == "--source-dir" ]]; then
        source_dir="$2"
        shift 2
    else
        shift
    fi
done
[[ -n "$source_dir" ]]
[[ -x "$FIXTURE_ROOT/$source_dir/seen" ]]
[[ -x "$FIXTURE_ROOT/$source_dir/seen-pkg" ]]
"$FIXTURE_ROOT/$source_dir/seen-pkg" \
    --expect-version 0.10.0 version --machine >/dev/null
basename "$0" >> "$CALL_LOG"
BUILDER_EOF
chmod +x "$FIXTURE_ROOT/installer/linux/fake-builder.sh"
for builder in build-deb.sh build-rpm.sh build-appimage.sh; do
    cp "$FIXTURE_ROOT/installer/linux/fake-builder.sh" \
        "$FIXTURE_ROOT/installer/linux/$builder"
done

export FIXTURE_ROOT CALL_LOG
"$FIXTURE_ROOT/scripts/build_installers.sh" \
    --version 0.10.0 \
    --stage3 "$FIXTURE_ROOT/stage3_seen" \
    --output-dir "$TMP_DIR/output" >/dev/null

[[ -x "$FIXTURE_ROOT/installer/tmp/linux/seen-pkg" ]]
[[ "$(wc -l < "$CALL_LOG" | tr -d ' ')" == "3" ]]

echo "build_installers package-client staging test passed"
