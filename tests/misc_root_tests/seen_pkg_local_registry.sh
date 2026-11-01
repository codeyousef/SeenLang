#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
SEEN_COMPILE_CMD="${SEEN_COMPILE_CMD:-compile}"
[ "$SEEN_COMPILE_CMD" = "compile" ] || {
    echo "FAIL: only the supported compile command is accepted" >&2
    exit 2
}
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-pkg-local-registry
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != "1" ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- \
        bash "$0" "$@"
fi
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
TMP_DIR="${SEEN_REGISTRY_FIXTURE_ROOT:-$(mktemp -d "$SEEN_ARTIFACT_ROOT/seen-pkg-local-registry.XXXXXX")}"
FIXTURE="$ROOT_DIR/tests/package_registry_fixture.py"
if [ -z "${SEEN_REGISTRY_FIXTURE_ROOT:-}" ]; then
    exec python3 "$FIXTURE" run "$TMP_DIR" bash "$0" "$@"
fi
REGISTRY_ORIGIN="https://registry.fixture.test/packages"
REGISTRY_ALT_ORIGIN="https://alternate.fixture.test/packages"
TRUST_ARGS=(--trusted-root "default=$TMP_DIR/root.json"
    --trusted-root-sha256 "default=$SEEN_REGISTRY_ROOT_SHA256"
    --environment default=development --repository-id default=seen-dev-test-fixture-v1)
PUBLISH_ARGS=(--token-file "$TMP_DIR/token" --source-forge github
    --source-repository-id 987654321 --source-installation-id fixture-publisher
    --source-ref refs/heads/fixture --source-commit 0123456789abcdef0123456789abcdef01234567
    --license-spdx MIT)
PACKAGE_DIR="$TMP_DIR/mathx"
CONSUMER_DIR="$TMP_DIR/consumer"
OUTPUT_BIN="$TMP_DIR/consumer_bin"

cleanup() {
    local status=$?
    if [ "$status" -ne 0 ] || [ -n "${SEEN_KEEP_TMP:-}" ]; then
        printf 'Preserved registry fixture artifacts: %s (status %s)\n' "$TMP_DIR" "$status" >&2
        return "$status"
    fi
    case "$TMP_DIR" in
        "$SEEN_ARTIFACT_ROOT"/seen-pkg-local-registry.*)
            [ -d "$TMP_DIR" ] && [ ! -L "$TMP_DIR" ] &&
                [ "$(dirname -- "$TMP_DIR")" = "$SEEN_ARTIFACT_ROOT" ] || return 1
            chmod -R u+w -- "$TMP_DIR"
            rm -rf -- "$TMP_DIR"
            ;;
        *) return 1 ;;
    esac
}

trap cleanup EXIT

run_recorded() {
    local label="$1"
    shift
    local status=0
    "$@" >"$TMP_DIR/$label.log" 2>&1 || status=$?
    printf '%s\n' "$status" >"$TMP_DIR/$label.exit"
    if [ "$status" -ne 0 ]; then
        cat "$TMP_DIR/$label.log" >&2
    fi
    return "$status"
}

mkdir -p "$PACKAGE_DIR/src" "$CONSUMER_DIR/src"

cat >"$PACKAGE_DIR/Seen.toml" <<'EOF'
manifest-version = 1
[project]
name = "mathx"
version = "0.1.0"
[package]
identity = "alice/mathx"
visibility = "public"
include = ["src/**/*.seen"]
assets = []
capabilities = []
[dependencies]
EOF

cat >"$PACKAGE_DIR/src/value.seen" <<'EOF'
pub fun answer() r: Int {
    return 42
}
EOF

bash "$ATTESTED_SEEN" "$COMPILER" pkg publish "$PACKAGE_DIR" \
    --registry "$REGISTRY_ORIGIN" "${PUBLISH_ARGS[@]}" >"$TMP_DIR/publish.log"
grep -Fq 'Submitted alice/mathx@0.1.0' "$TMP_DIR/publish.log"
grep -Fq 'lifecycle=quarantined' "$TMP_DIR/publish.log"
grep -Fq 'availability=unavailable' "$TMP_DIR/publish.log"
test -s "$TMP_DIR/registry.fixture.test/blob.tgz"

# Go computes archive digests internally. Reject an unbound upload receipt,
# rather than overriding a legacy external sha256sum command.
touch "$TMP_DIR/alternate.fixture.test/bad-confirmation"
if bash "$ATTESTED_SEEN" "$COMPILER" pkg publish "$PACKAGE_DIR" \
    --registry "$REGISTRY_ALT_ORIGIN" "${PUBLISH_ARGS[@]}" \
    >"$TMP_DIR/checksum-failure.log" 2>&1; then
    echo "FAIL: pkg publish accepted mismatched archive confirmation"
    exit 1
fi
grep -Fq 'mismatched archive confirmation' "$TMP_DIR/checksum-failure.log"
if grep -Fq 'alternate.fixture.test POST uploads/upl_0123456789abcdef/complete' "$TMP_DIR/requests.log"; then
    echo "FAIL: checksum failure completed the publication"
    exit 1
fi
test ! -e "$TMP_DIR/alternate.fixture.test/active"

cat >"$CONSUMER_DIR/Seen.toml" <<EOF
manifest-version = 1
[project]
name = "consumer"
version = "0.1.0"
[registries]
default = "$REGISTRY_ORIGIN"
[dependencies]
mathx = { package = "alice/mathx", version = "0.1.0" }
EOF

cat >"$CONSUMER_DIR/src/main.seen" <<'EOF'
import mathx.value.{answer}

fun main() r: Int {
    if answer() != 42 {
        return 1
    }
    return 0
}
EOF

# Submission is not activation: quarantined bytes are absent from signed
# resolver metadata. Only the explicit test-fixture activation makes them visible.
if bash "$ATTESTED_SEEN" "$COMPILER" pkg fetch "$CONSUMER_DIR" \
    "${TRUST_ARGS[@]}" >"$TMP_DIR/quarantined-fetch.log" 2>&1; then
    echo "FAIL: quarantined publication was resolver-visible"
    exit 1
fi
test ! -e "$CONSUMER_DIR/Seen.lock"
python3 "$FIXTURE" activate "$TMP_DIR"
bash "$ATTESTED_SEEN" "$COMPILER" pkg fetch "$CONSUMER_DIR" \
    "${TRUST_ARGS[@]}" >/dev/null
test -f "$CONSUMER_DIR/Seen.lock"
test "$(find "$CONSUMER_DIR/.seen/views/alice/mathx/0.1.0" \
    -mindepth 4 -maxdepth 4 -name Seen.toml | wc -l)" -eq 1
grep -Fq 'source = "hosted-registry"' "$CONSUMER_DIR/Seen.lock"
cp "$CONSUMER_DIR/Seen.lock" "$TMP_DIR/valid.lock"
cp "$TMP_DIR/requests.log" "$TMP_DIR/before-frozen.requests"
bash "$ATTESTED_SEEN" "$COMPILER" pkg fetch "$CONSUMER_DIR" --frozen >/dev/null
cmp "$TMP_DIR/valid.lock" "$CONSUMER_DIR/Seen.lock"
cmp "$TMP_DIR/before-frozen.requests" "$TMP_DIR/requests.log"

run_recorded consumer-compile bash "$ATTESTED_SEEN" "$COMPILER" "$SEEN_COMPILE_CMD" \
    "$CONSUMER_DIR/src/main.seen" "$OUTPUT_BIN" --fast --frozen
run_recorded consumer-execution timeout --foreground --kill-after=5s 120s "$OUTPUT_BIN"

cp "$CONSUMER_DIR/Seen.toml" "$TMP_DIR/valid.toml"
printf '\n# changed manifest\n' >>"$CONSUMER_DIR/Seen.toml"
if bash "$ATTESTED_SEEN" "$COMPILER" pkg fetch "$CONSUMER_DIR" --frozen \
    >"$TMP_DIR/stale-lock.log" 2>&1; then
    echo "FAIL: frozen fetch accepted a changed manifest"
    exit 1
fi
grep -Fq 'locked_manifest_mismatch' "$TMP_DIR/stale-lock.log"
cmp "$TMP_DIR/valid.lock" "$CONSUMER_DIR/Seen.lock"
cp "$TMP_DIR/valid.toml" "$CONSUMER_DIR/Seen.toml"

# A fresh consumer/cache must verify downloaded bytes, not reuse the good blob.
mkdir -p "$TMP_DIR/tampered-consumer"
cp "$TMP_DIR/valid.toml" "$TMP_DIR/tampered-consumer/Seen.toml"
python3 "$FIXTURE" tamper-blob "$TMP_DIR"
if SEEN_PACKAGE_CACHE="$TMP_DIR/tampered-cache" \
    bash "$ATTESTED_SEEN" "$COMPILER" pkg fetch "$TMP_DIR/tampered-consumer" \
    "${TRUST_ARGS[@]}" >"$TMP_DIR/tampered-archive.log" 2>&1; then
    echo "FAIL: modified registry archive passed signed checksum verification"
    exit 1
fi
grep -Fq 'signing_target_hash_mismatch' "$TMP_DIR/tampered-archive.log"
test ! -e "$TMP_DIR/tampered-consumer/Seen.lock"
test ! -e "$TMP_DIR/tampered-consumer/.seen/package-map.tsv"
python3 "$FIXTURE" tamper-blob "$TMP_DIR"

# Re-sign malformed target metadata so the digest/path check itself is reached.
python3 "$FIXTURE" malformed-digest "$TMP_DIR"
mkdir -p "$TMP_DIR/malformed-consumer/.seen/views/victim"
cp "$TMP_DIR/valid.toml" "$TMP_DIR/malformed-consumer/Seen.toml"
touch "$TMP_DIR/malformed-consumer/.seen/views/victim/sentinel"
if SEEN_PACKAGE_CACHE="$TMP_DIR/malformed-cache" \
    bash "$ATTESTED_SEEN" "$COMPILER" pkg fetch "$TMP_DIR/malformed-consumer" \
    "${TRUST_ARGS[@]}" >"$TMP_DIR/malformed-digest.log" 2>&1; then
    echo "FAIL: traversal-shaped signed digest was accepted"
    exit 1
fi
grep -Fq 'signing_target_path_digest_mismatch' "$TMP_DIR/malformed-digest.log"
test -f "$TMP_DIR/malformed-consumer/.seen/views/victim/sentinel"
test ! -e "$TMP_DIR/malformed-consumer/Seen.lock"

cat >"$CONSUMER_DIR/Seen.toml" <<'EOF'
[project]
name = "consumer"
version = "0.1.0"

[dependencies]
EOF
bash "$ATTESTED_SEEN" "$COMPILER" pkg fetch "$CONSUMER_DIR" >/dev/null
if [ -f "$CONSUMER_DIR/Seen.lock" ]; then
    echo "FAIL: removing registry dependencies left a stale Seen.lock"
    exit 1
fi

echo "PASS: authenticated local HTTPS publication, signed fetch, integrity, frozen lock, and native consumption"
