#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
SEEN_COMPILE_CMD="${SEEN_COMPILE_CMD:-compile}"
TMP_DIR="$(mktemp -d /tmp/seen_pkg_scoped_identity.XXXXXX)"
REGISTRY_DIR="$TMP_DIR/registry"
REGISTRY_ALT_DIR="$TMP_DIR/registry-alt"
PACKAGE_DIR="$TMP_DIR/mathx"
CONSUMER_DIR="$TMP_DIR/consumer"
BAD_PACKAGE_DIR="$TMP_DIR/bad-package"
OUTPUT_BIN="$TMP_DIR/consumer_bin"

cleanup() {
    rm -rf "$TMP_DIR"
}

trap cleanup EXIT

mkdir -p "$REGISTRY_DIR" "$REGISTRY_ALT_DIR" "$PACKAGE_DIR/src" \
    "$CONSUMER_DIR/src" "$BAD_PACKAGE_DIR/src"

cat >"$PACKAGE_DIR/Seen.toml" <<'EOF'
manifest-version = 1

[project]
name = "math_core"
version = "0.1.0"

[package]
identity = "alice/mathx"

[dependencies]

[native.dependencies]
EOF

cat >"$PACKAGE_DIR/src/math_core.seen" <<'EOF'
pub fun answer() r: Int {
    return 42
}

fun hidden() r: Int {
    return 7
}
EOF

(
    cd "$PACKAGE_DIR"
    "$COMPILER" pkg publish "$REGISTRY_DIR" >/dev/null
)

test -f "$REGISTRY_DIR/index/alice/mathx.toml"
test -f \
    "$REGISTRY_DIR/archives/alice/mathx/mathx-0.1.0.seenpkg.tgz"

cat >"$PACKAGE_DIR/src/math_core.seen" <<'EOF'
pub fun answer() r: Int {
    return 43
}

fun hidden() r: Int {
    return 8
}
EOF

(
    cd "$PACKAGE_DIR"
    "$COMPILER" pkg publish "$REGISTRY_ALT_DIR" >/dev/null
)

cat >"$CONSUMER_DIR/Seen.toml" <<EOF
[project]
name = "consumer"
version = "0.1.0"

[registries]
default = "$REGISTRY_DIR"
alternate = "$REGISTRY_ALT_DIR"

[dependencies]
calc = { package = "alice/mathx", version = "0.1.0" }
calc_alt = { package = "alice/mathx", version = "0.1.0", registry = "alternate" }

[native.dependencies]
EOF

cat >"$CONSUMER_DIR/src/main.seen" <<'EOF'
import calc.{answer}

fun main() r: Int {
    if answer() != 42 {
        return 1
    }
    return 0
}
EOF

"$COMPILER" pkg fetch "$CONSUMER_DIR" >/dev/null

test "$(find "$CONSUMER_DIR/.seen/packages/alice/mathx/0.1.0" \
    -mindepth 2 -maxdepth 2 -name Seen.toml | wc -l)" -eq 2
grep -Fq 'version = 2' "$CONSUMER_DIR/Seen.lock"
grep -Fq 'alias = "calc"' "$CONSUMER_DIR/Seen.lock"
grep -Fq 'package = "alice/mathx"' "$CONSUMER_DIR/Seen.lock"
grep -Fq 'source = "legacy-local-static"' "$CONSUMER_DIR/Seen.lock"
grep -Fq "registry_origin = \"$REGISTRY_DIR\"" "$CONSUMER_DIR/Seen.lock"
grep -Fq "registry_origin = \"$REGISTRY_ALT_DIR\"" "$CONSUMER_DIR/Seen.lock"
test "$(grep -Ec '^archive_sha256 = "[0-9a-f]{64}"$' \
    "$CONSUMER_DIR/Seen.lock")" -eq 2
if grep -Eq '^(name|registry|path) = ' "$CONSUMER_DIR/Seen.lock"; then
    echo "FAIL: dependency lock contains an ambiguous legacy field"
    exit 1
fi

"$COMPILER" "$SEEN_COMPILE_CMD" "$CONSUMER_DIR/src/main.seen" \
    "$OUTPUT_BIN" --fast >/dev/null
"$OUTPUT_BIN" >/dev/null

cat >"$CONSUMER_DIR/src/private_probe.seen" <<'EOF'
import calc.{hidden}

fun main() r: Int {
    return hidden()
}
EOF

if "$COMPILER" "$SEEN_COMPILE_CMD" \
    "$CONSUMER_DIR/src/private_probe.seen" "$TMP_DIR/private_probe" \
    --fast >/dev/null 2>&1; then
    echo "FAIL: package-private symbol was importable by a consumer"
    exit 1
fi

assert_inline_dependency_rejected() {
    local case_name="$1"
    local dependency_line="$2"

    cat >"$CONSUMER_DIR/Seen.toml" <<EOF
[project]
name = "consumer"
version = "0.1.0"

[registries]
default = "$REGISTRY_DIR"

[dependencies]
$dependency_line
EOF

    if "$COMPILER" pkg fetch "$CONSUMER_DIR" >/dev/null 2>&1; then
        echo "FAIL: malformed inline dependency was accepted: $case_name"
        exit 1
    fi
}

assert_inline_dependency_rejected \
    "missing closing brace" \
    'calc = { package = "alice/mathx", version = "0.1.0"'
assert_inline_dependency_rejected \
    "missing comma" \
    'calc = { package = "alice/mathx" version = "0.1.0" }'
assert_inline_dependency_rejected \
    "double comma" \
    'calc = { package = "alice/mathx",, version = "0.1.0" }'
assert_inline_dependency_rejected \
    "trailing comma" \
    'calc = { package = "alice/mathx", version = "0.1.0", }'
assert_inline_dependency_rejected \
    "malformed key-value separator" \
    'calc = { package : "alice/mathx", version = "0.1.0" }'
assert_inline_dependency_rejected \
    "trailing tokens" \
    'calc = { package = "alice/mathx", version = "0.1.0" } garbage'
assert_inline_dependency_rejected \
    "unterminated quote" \
    'calc = { path = "native/lib }'
assert_inline_dependency_rejected \
    "quote garbage" \
    'calc = { path = "native/lib"garbage }'
assert_inline_dependency_rejected \
    "quoted system boolean" \
    'calc = { system = "true", path = "native/lib" }'
assert_inline_dependency_rejected \
    "non-boolean system value" \
    'calc = { system = trueish, path = "native/lib" }'

mkdir -p "$CONSUMER_DIR/native/system=true/lib"
cat >"$CONSUMER_DIR/native/system=true/lib/Seen.toml" <<'EOF'
[project]
name = "quoted-system-path"
version = "0.1.0"
EOF

cat >"$CONSUMER_DIR/Seen.toml" <<'EOF'
[project]
name = "consumer"
version = "0.1.0"

[dependencies]
quoted_system_text = { path = "native/system=true/lib" }
explicit_false = { system = false, path = "native/system=true/lib" }
EOF

cat >"$CONSUMER_DIR/src/main.seen" <<'EOF'
fun main() r: Int {
    return 0
}
EOF

if ! "$COMPILER" pkg fetch "$CONSUMER_DIR" >/dev/null 2>&1; then
    echo "FAIL: system=true text inside a quoted path changed dependency classification"
    exit 1
fi
if ! "$COMPILER" "$SEEN_COMPILE_CMD" "$CONSUMER_DIR/src/main.seen" \
    "$TMP_DIR/quoted_system_path_bin" --fast >/dev/null 2>&1; then
    echo "FAIL: quoted system=true path was linked as a legacy system dependency"
    exit 1
fi

cat >"$CONSUMER_DIR/Seen.toml" <<EOF
[project]
name = "consumer"
version = "0.1.0"

[registries]
default = "$REGISTRY_DIR"

[dependencies]
calc = { package = "alice/../mathx", version = "0.1.0" }
EOF

if "$COMPILER" pkg fetch "$CONSUMER_DIR" >/dev/null 2>&1; then
    echo "FAIL: traversal-shaped package identity was accepted"
    exit 1
fi

cat >"$CONSUMER_DIR/Seen.toml" <<EOF
[project]
name = "consumer"
version = "0.1.0"

[registries]
default = "$REGISTRY_DIR"

[dependencies]
calc = { pakage = "alice/mathx", version = "0.1.0" }
EOF

if "$COMPILER" pkg fetch "$CONSUMER_DIR" >/dev/null 2>&1; then
    echo "FAIL: misspelled package field fell back to the alias"
    exit 1
fi

cat >"$CONSUMER_DIR/Seen.toml" <<EOF
[project]
name = "consumer"
version = "0.1.0"

[registries]
default = "$REGISTRY_DIR"

[dependencies]
calc = { subpackage = "alice/evil", version = "0.1.0" }
EOF

if "$COMPILER" pkg fetch "$CONSUMER_DIR" >/dev/null 2>&1; then
    echo "FAIL: subpackage field was parsed as package"
    exit 1
fi

cat >"$CONSUMER_DIR/Seen.toml" <<EOF
[project]
name = "consumer"
version = "0.1.0"

[registries]
default = "$REGISTRY_DIR"

[dependencies]
calc = { package = "alice/mathx", version = "0.1.0", registrry = "private" }
EOF

if "$COMPILER" pkg fetch "$CONSUMER_DIR" >/dev/null 2>&1; then
    echo "FAIL: unknown registry field was silently ignored"
    exit 1
fi

cat >"$CONSUMER_DIR/Seen.toml" <<EOF
[project]
name = "consumer"
version = "0.1.0"

[registries]
default = "$REGISTRY_DIR"

[dependencies]
calc = { package = "alice/mathx", path = "../mathx", version = "0.1.0" }
EOF

if "$COMPILER" pkg fetch "$CONSUMER_DIR" >/dev/null 2>&1; then
    echo "FAIL: mixed registry and path dependency source was accepted"
    exit 1
fi

cat >"$CONSUMER_DIR/Seen.toml" <<EOF
[project]
name = "consumer"
version = "0.1.0"

[registries]
default = "$REGISTRY_DIR"

[dependencies]
calc = { package = "alice/mathx", package = "alice/other", version = "0.1.0" }
EOF

if "$COMPILER" pkg fetch "$CONSUMER_DIR" >/dev/null 2>&1; then
    echo "FAIL: duplicate inline package field was accepted"
    exit 1
fi

cat >"$CONSUMER_DIR/Seen.toml" <<EOF
[project]
name = "consumer"
version = "0.1.0"

[registries]
default = "$REGISTRY_DIR"

[dependencies]
calc = { package = "alice/mathx", version = "0.1.0" }
calc = { package = "alice/mathx", version = "0.1.0" }
EOF

if "$COMPILER" pkg fetch "$CONSUMER_DIR" >/dev/null 2>&1; then
    echo "FAIL: duplicate dependency alias was accepted"
    exit 1
fi

cat >"$CONSUMER_DIR/Seen.toml" <<'EOF'
[project]
name = "consumer"
version = "0.1.0"

[dependencies]
legacy_native = { system = true, path = "native/lib" }
legacy_native = { system = true, path = "native/lib" }
EOF

if "$COMPILER" pkg fetch "$CONSUMER_DIR" >/dev/null 2>&1; then
    echo "FAIL: duplicate legacy system dependency alias was accepted"
    exit 1
fi

cat >"$CONSUMER_DIR/Seen.toml" <<'EOF'
[project]
name = "consumer"
version = "0.1.0"

[dependencies]
legacy_native = { system = true, path = "native/lib" }
EOF

if ! "$COMPILER" pkg fetch "$CONSUMER_DIR" >/dev/null 2>&1; then
    echo "FAIL: legacy system dependency with native path regressed"
    exit 1
fi

cat >"$BAD_PACKAGE_DIR/Seen.toml" <<'EOF'
[project]
name = "../escape"
version = "0.1.0"

[package]
identity = "alice/escape"
EOF

if "$COMPILER" pkg publish "$REGISTRY_DIR" \
    "$BAD_PACKAGE_DIR" >/dev/null 2>&1; then
    echo "FAIL: traversal-shaped project name was accepted for publishing"
    exit 1
fi

echo "PASS: package aliases and scoped registry identities stay distinct"
