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
SCOPE=seen-pkg-scoped-identity
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != "1" ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- \
        bash "$0" "$@"
fi
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
TMP_DIR="${SEEN_REGISTRY_FIXTURE_ROOT:-$(mktemp -d "$SEEN_ARTIFACT_ROOT/seen-pkg-scoped-identity.XXXXXX")}"
FIXTURE="$ROOT_DIR/tests/package_registry_fixture.py"
if [ -z "${SEEN_REGISTRY_FIXTURE_ROOT:-}" ]; then
    exec python3 "$FIXTURE" run "$TMP_DIR" bash "$0" "$@"
fi
REGISTRY_DIR="https://registry.fixture.test/packages"
REGISTRY_ALT_DIR="https://alternate.fixture.test/packages"
TRUST_ARGS=(--trusted-root "default=$TMP_DIR/root.json"
    --trusted-root-sha256 "default=$SEEN_REGISTRY_ROOT_SHA256"
    --environment default=development --repository-id default=seen-dev-test-fixture-v1
    --trusted-root "alternate=$TMP_DIR/root.json"
    --trusted-root-sha256 "alternate=$SEEN_REGISTRY_ROOT_SHA256"
    --environment alternate=development --repository-id alternate=seen-dev-test-fixture-v1)
PUBLISH_ARGS=(--token-file "$TMP_DIR/token" --source-forge github
    --source-repository-id 987654321 --source-installation-id fixture-publisher
    --source-ref refs/heads/fixture --source-commit 0123456789abcdef0123456789abcdef01234567
    --license-spdx MIT)
PACKAGE_DIR="$TMP_DIR/mathx"
CONSUMER_DIR="$TMP_DIR/consumer"
BAD_PACKAGE_DIR="$TMP_DIR/bad-package"

run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
FIXTURE_ARTIFACT_PREFIX=seen-pkg-scoped-identity
source "$ROOT_DIR/scripts/compiled_fixture_support.sh"

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

mkdir -p "$PACKAGE_DIR/src" "$CONSUMER_DIR/src" "$BAD_PACKAGE_DIR/src"

cat >"$PACKAGE_DIR/Seen.toml" <<'EOF'
manifest-version = 1

[project]
name = 'math_core' # literal strings are valid package TOML
version = '0.1.0' # the helper owns manifest grammar

[package]
identity = 'alice/mathx'
visibility = "public"
include = ["src/**/*.seen"]
assets = []
capabilities = []

[dependencies]

[native.dependencies]
EOF

cat >"$PACKAGE_DIR/src/value.seen" <<'EOF'
pub fun answer() r: Int {
    let helper = hidden
    if helper() != hidden() { return 0 }
    return hidden() + 35
}

@async
pub fun typedFloat() r: Float { return 42.5 }

@async
pub fun typedBool() r: Bool { return true }

@async
pub fun typedString() r: String { return "first-origin" }

@intrinsic("llvm.bswap.i64")
pub fun swapBytes(value: Int) r: Int { return value }

@init
fun initializeOrigin() r: Void {
    println(71)

}
fun hidden() r: Int {
    return 7
}
EOF

for marker in native seen_std project_root; do
    mkdir -p "$PACKAGE_DIR/src/$marker"
    cat >"$PACKAGE_DIR/src/$marker/value.seen" <<EOF
pub fun marker_$marker() r: Int { return 42 }
EOF
done

bash "$ATTESTED_SEEN" "$COMPILER" pkg publish "$PACKAGE_DIR" \
    --registry "$REGISTRY_DIR" "${PUBLISH_ARGS[@]}" >"$TMP_DIR/publish.log"
grep -Fq 'lifecycle=quarantined' "$TMP_DIR/publish.log"
test -s "$TMP_DIR/registry.fixture.test/blob.tgz"
python3 "$FIXTURE" activate "$TMP_DIR"

cat >"$PACKAGE_DIR/src/value.seen" <<'EOF'
pub fun answer() r: Int {
    let helper = hidden
    if helper() != hidden() { return 0 }
    return hidden() + 35
}

@async
pub fun typedFloat() r: Float { return 43.5 }

@async
pub fun typedBool() r: Bool { return false }

@async
pub fun typedString() r: String { return "second-origin" }

@intrinsic("llvm.bswap.i64")
pub fun swapBytes(value: Int) r: Int { return value }

@init
fun initializeOrigin() r: Void {
    println(81)

}
pub fun alternateAnswer() r: Int {
    return answer()
}

fun hidden() r: Int {
    return 8
}
EOF

for marker in native seen_std project_root; do
    cat >"$PACKAGE_DIR/src/$marker/value.seen" <<EOF
pub fun alternate_marker_$marker() r: Int { return 43 }
EOF
done

bash "$ATTESTED_SEEN" "$COMPILER" pkg publish "$PACKAGE_DIR" \
    --registry "$REGISTRY_ALT_DIR" "${PUBLISH_ARGS[@]}" >"$TMP_DIR/publish-alt.log"
grep -Fq 'lifecycle=quarantined' "$TMP_DIR/publish-alt.log"
python3 "$FIXTURE" activate "$TMP_DIR" alternate.fixture.test

cat >"$CONSUMER_DIR/Seen.toml" <<EOF
manifest-version = 1
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

# Bind the same exported spelling in a distinct consumer lexical module.
cat >"$CONSUMER_DIR/src/alternate_scope.seen" <<'EOF'
import calc_alt.value.{answer}
import calc_alt.value.{typedFloat, typedBool, typedString, swapBytes}

pub fun alternateScopedAnswer() r: Int {
    let target = answer
    if target() != 43 { return 0 }
    if (await typedFloat()) != 43.5 { return 0 }
    if await typedBool() { return 0 }
    if (await typedString()) != "second-origin" { return 0 }
    let swapped = swapBytes
    if swapped(1) != 72057594037927936 { return 0 }
    if swapBytes(256) != 281474976710656 { return 0 }
    return answer()
}
EOF

cat >"$CONSUMER_DIR/src/root_alias_provider.seen" <<'EOF'
pub fun sin(value: Int) r: Int { return value + 100 }
EOF

cat >"$CONSUMER_DIR/src/module_alias_scope.seen" <<'EOF'
import calc.value as math
import root_alias_provider as local_math
import env.env as process_env

extern "C" fun seen_cstr_free(value: *Void) r: Void
extern "C" fun seen_memory_release_bytes(bytes: Int64) r: Void

fun answer() r: Int { return 900 }

class ModuleLike {
    fun answer() r: Int { return 711 }
}

fun parameterAliasShadow(math: ModuleLike) r: Int { return math.answer() }

pub fun qualifiedScopedAnswer() r: Int {
    let target = math.answer
    if target() != 42 or math.answer() != 42 { return 0 }
    let localSin = local_math.sin
    if localSin(2) != 102 or local_math.sin(3) != 103 { return 0 }
    if not process_env.has("SEEN_PKG_IDENTITY_ALIAS_PROBE") { return 0 }
    let probe = process_env.getOrDefault("SEEN_PKG_IDENTITY_ALIAS_PROBE", "")
    let probeMatches = probe == "origin-owned"
    // Present environment values own a copied C string, as args() values do.
    let probeBytes = probe.length() + 1
    let probeRepresentation = (&probe) as *UInt64
    seen_cstr_free(probeRepresentation[1] as *Void)
    seen_memory_release_bytes(probeBytes as Int64)
    if not probeMatches { return 0 }
    let math = ModuleLike{}
    if math.answer() != 711 or parameterAliasShadow(math) != 711 { return 0 }
    return 42
}
EOF

cat >"$CONSUMER_DIR/src/main.seen" <<'EOF'
import calc.value.{answer}
import calc_alt.value.{alternateAnswer}
import calc.value.{typedFloat, typedBool, typedString, swapBytes}
import module_alias_scope.{qualifiedScopedAnswer}
import alternate_scope.{alternateScopedAnswer}
import calc.native.value.{marker_native}
import calc_alt.native.value.{alternate_marker_native}
import calc.seen_std.value.{marker_seen_std}
import calc_alt.seen_std.value.{alternate_marker_seen_std}
import calc.project_root.value.{marker_project_root}
import calc_alt.project_root.value.{alternate_marker_project_root}

fun callShadowedAnswer(answer: Int) r: Int {
    return answer()
}

fun main() r: Int {
    if answer() != 42 {
        return 1
    }
    if alternateAnswer() != 43 {
        return 2
    }
    if marker_native() != 42 or alternate_marker_native() != 43 {
        return 3
    }
    if marker_seen_std() != 42 or alternate_marker_seen_std() != 43 {
        return 4
    }
    if marker_project_root() != 42 or alternate_marker_project_root() != 43 {
        return 5
    }
    if alternateScopedAnswer() != 43 {
        return 6
    }
    if callShadowedAnswer(alternateAnswer) != 43 {
        return 7
    }
    if qualifiedScopedAnswer() != 42 { return 8 }
    if (await typedFloat()) != 42.5 { return 9 }
    if not (await typedBool()) { return 10 }
    if (await typedString()) != "first-origin" { return 11 }
    let swapped = swapBytes
    if swapped(1) != 72057594037927936 { return 12 }
    if swapBytes(256) != 281474976710656 { return 13 }
    return 0
}
EOF

bash "$ATTESTED_SEEN" "$COMPILER" pkg fetch "$CONSUMER_DIR" \
    "${TRUST_ARGS[@]}" >/dev/null

test "$(find "$CONSUMER_DIR/.seen/views/alice/mathx/0.1.0" \
    -mindepth 4 -maxdepth 4 -name Seen.toml | wc -l)" -eq 2
grep -Fq 'version = 2' "$CONSUMER_DIR/Seen.lock"
grep -Fq 'alias = "calc"' "$CONSUMER_DIR/Seen.lock"
grep -Fq 'alias = "calc_alt"' "$CONSUMER_DIR/Seen.lock"
grep -Fq 'package = "alice/mathx"' "$CONSUMER_DIR/Seen.lock"
test "$(grep -Fc 'source = "hosted-registry"' "$CONSUMER_DIR/Seen.lock")" -eq 2
grep -Fq "registry_origin = \"$REGISTRY_DIR\"" "$CONSUMER_DIR/Seen.lock"
grep -Fq "registry_origin = \"$REGISTRY_ALT_DIR\"" "$CONSUMER_DIR/Seen.lock"
test "$(grep -E '^archive_sha256 = "[0-9a-f]{64}"$' \
    "$CONSUMER_DIR/Seen.lock" | sort -u | wc -l)" -eq 2
if grep -Eq '^(registry|path) = ' "$CONSUMER_DIR/Seen.lock"; then
    echo "FAIL: dependency lock contains an ambiguous legacy field"
    exit 1
fi
python3 "$FIXTURE" restyle-lock "$CONSUMER_DIR"
cp "$CONSUMER_DIR/Seen.lock" "$TMP_DIR/valid.lock"
cp "$TMP_DIR/requests.log" "$TMP_DIR/before-frozen.requests"
bash "$ATTESTED_SEEN" "$COMPILER" pkg fetch "$CONSUMER_DIR" --frozen >/dev/null
cmp "$TMP_DIR/valid.lock" "$CONSUMER_DIR/Seen.lock"
cmp "$TMP_DIR/before-frozen.requests" "$TMP_DIR/requests.log"

export SEEN_PKG_IDENTITY_ALIAS_PROBE=origin-owned
for profile in fast release undefined address; do
    compile_case "$profile" consumer "$CONSUMER_DIR/src/main.seen"
    test "$(grep -Fxc '71' "$TMP_DIR/consumer-$profile.log")" -eq 1
    test "$(grep -Fxc '81' "$TMP_DIR/consumer-$profile.log")" -eq 1
    echo "PASS: scoped registry ownership $profile native execution"
done

cat >"$CONSUMER_DIR/src/private_probe.seen" <<'EOF'
import calc.value.{hidden}

fun main() r: Int {
    return hidden()
}
EOF

if run_recorded private-probe bash "$ATTESTED_SEEN" "$COMPILER" "$SEEN_COMPILE_CMD" \
    "$CONSUMER_DIR/src/private_probe.seen" "$TMP_DIR/private_probe" --fast --frozen; then
    echo "FAIL: package-private symbol was importable by a consumer"
    exit 1
fi
cmp "$TMP_DIR/valid.lock" "$CONSUMER_DIR/Seen.lock"

assert_inline_dependency_rejected() {
    local case_name="$1"
    local dependency_line="$2"
    local diagnostic="${3:-parse Seen.toml:}"

    cat >"$CONSUMER_DIR/Seen.toml" <<EOF
manifest-version = 1
[project]
name = "consumer"
version = "0.1.0"

[registries]
default = "$REGISTRY_DIR"

[dependencies]
$dependency_line
EOF

    if bash "$ATTESTED_SEEN" "$COMPILER" pkg fetch \
        "$CONSUMER_DIR" >"$TMP_DIR/inline-rejected.log" 2>&1; then
        echo "FAIL: malformed inline dependency was accepted: $case_name"
        exit 1
    fi
    grep -Fq "$diagnostic" "$TMP_DIR/inline-rejected.log"
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
    'calc = { system = "true", path = "native/lib" }' \
    'system must be a boolean'
assert_inline_dependency_rejected \
    "non-boolean system value" \
    'calc = { system = trueish, path = "native/lib" }'
assert_inline_dependency_rejected \
    "explicit false system dependency" \
    'calc = { system = false, path = "native/lib" }' \
    'system dependency must be exactly'

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
EOF

cat >"$CONSUMER_DIR/src/main.seen" <<'EOF'
fun main() r: Int {
    return 0
}
EOF

if ! bash "$ATTESTED_SEEN" "$COMPILER" pkg fetch \
    "$CONSUMER_DIR" >/dev/null 2>&1; then
    echo "FAIL: system=true text inside a quoted path changed dependency classification"
    exit 1
fi
if ! run_recorded quoted-system-compile bash "$ATTESTED_SEEN" "$COMPILER" "$SEEN_COMPILE_CMD" \
    "$CONSUMER_DIR/src/main.seen" "$TMP_DIR/quoted_system_path_bin" --fast; then
    echo "FAIL: quoted system=true path was linked as a legacy system dependency"
    exit 1
fi

cat >"$CONSUMER_DIR/Seen.toml" <<EOF
manifest-version = 1
[project]
name = "consumer"
version = "0.1.0"

[registries]
default = "$REGISTRY_DIR"

[dependencies]
calc = { package = "alice/../mathx", version = "0.1.0" }
EOF

if bash "$ATTESTED_SEEN" "$COMPILER" pkg fetch \
    "$CONSUMER_DIR" >"$TMP_DIR/rejected-identity.log" 2>&1; then
    echo "FAIL: traversal-shaped package identity was accepted"
    exit 1
fi
grep -Fq 'must be canonical owner/name' "$TMP_DIR/rejected-identity.log"

cat >"$CONSUMER_DIR/Seen.toml" <<EOF
manifest-version = 1
[project]
name = "consumer"
version = "0.1.0"

[registries]
default = "$REGISTRY_DIR"

[dependencies]
calc = { pakage = "alice/mathx", version = "0.1.0" }
EOF

if bash "$ATTESTED_SEEN" "$COMPILER" pkg fetch \
    "$CONSUMER_DIR" >"$TMP_DIR/rejected-field.log" 2>&1; then
    echo "FAIL: misspelled package field fell back to the alias"
    exit 1
fi
grep -Fq 'unknown field "pakage"' "$TMP_DIR/rejected-field.log"

cat >"$CONSUMER_DIR/Seen.toml" <<EOF
manifest-version = 1
[project]
name = "consumer"
version = "0.1.0"

[registries]
default = "$REGISTRY_DIR"

[dependencies]
calc = { subpackage = "alice/evil", version = "0.1.0" }
EOF

if bash "$ATTESTED_SEEN" "$COMPILER" pkg fetch \
    "$CONSUMER_DIR" >"$TMP_DIR/rejected-field.log" 2>&1; then
    echo "FAIL: subpackage field was parsed as package"
    exit 1
fi
grep -Fq 'unknown field "subpackage"' "$TMP_DIR/rejected-field.log"

cat >"$CONSUMER_DIR/Seen.toml" <<EOF
manifest-version = 1
[project]
name = "consumer"
version = "0.1.0"

[registries]
default = "$REGISTRY_DIR"

[dependencies]
calc = { package = "alice/mathx", version = "0.1.0", registrry = "private" }
EOF

if bash "$ATTESTED_SEEN" "$COMPILER" pkg fetch \
    "$CONSUMER_DIR" >"$TMP_DIR/rejected-field.log" 2>&1; then
    echo "FAIL: unknown registry field was silently ignored"
    exit 1
fi
grep -Fq 'unknown field "registrry"' "$TMP_DIR/rejected-field.log"

cat >"$CONSUMER_DIR/Seen.toml" <<EOF
manifest-version = 1
[project]
name = "consumer"
version = "0.1.0"

[registries]
default = "$REGISTRY_DIR"

[dependencies]
calc = { package = "alice/mathx", path = "../mathx", version = "0.1.0" }
EOF

if bash "$ATTESTED_SEEN" "$COMPILER" pkg fetch \
    "$CONSUMER_DIR" >"$TMP_DIR/rejected-source.log" 2>&1; then
    echo "FAIL: mixed registry and path dependency source was accepted"
    exit 1
fi
grep -Fq 'must select exactly one of package, path, or artifact' "$TMP_DIR/rejected-source.log"

cat >"$CONSUMER_DIR/Seen.toml" <<EOF
manifest-version = 1
[project]
name = "consumer"
version = "0.1.0"

[registries]
default = "$REGISTRY_DIR"

[dependencies]
calc = { package = "alice/mathx", package = "alice/other", version = "0.1.0" }
EOF

if bash "$ATTESTED_SEEN" "$COMPILER" pkg fetch \
    "$CONSUMER_DIR" >"$TMP_DIR/rejected-duplicate.log" 2>&1; then
    echo "FAIL: duplicate inline package field was accepted"
    exit 1
fi
grep -Fq 'parse Seen.toml:' "$TMP_DIR/rejected-duplicate.log"

cat >"$CONSUMER_DIR/Seen.toml" <<EOF
manifest-version = 1
[project]
name = "consumer"
version = "0.1.0"

[registries]
default = "$REGISTRY_DIR"

[dependencies]
calc = { package = "alice/mathx", version = "0.1.0" }
calc = { package = "alice/mathx", version = "0.1.0" }
EOF

if bash "$ATTESTED_SEEN" "$COMPILER" pkg fetch \
    "$CONSUMER_DIR" >"$TMP_DIR/rejected-duplicate.log" 2>&1; then
    echo "FAIL: duplicate dependency alias was accepted"
    exit 1
fi
grep -Fq 'parse Seen.toml:' "$TMP_DIR/rejected-duplicate.log"

cat >"$CONSUMER_DIR/Seen.toml" <<'EOF'
[project]
name = "consumer"
version = "0.1.0"

[dependencies]
legacy_native = { system = true, path = "native/lib" }
legacy_native = { system = true, path = "native/lib" }
EOF

if bash "$ATTESTED_SEEN" "$COMPILER" pkg fetch \
    "$CONSUMER_DIR" >"$TMP_DIR/rejected-duplicate.log" 2>&1; then
    echo "FAIL: duplicate legacy system dependency alias was accepted"
    exit 1
fi
grep -Fq 'parse Seen.toml:' "$TMP_DIR/rejected-duplicate.log"

cat >"$CONSUMER_DIR/Seen.toml" <<'EOF'
[project]
name = "consumer"
version = "0.1.0"

[dependencies]
legacy_native = { system = true, path = "native/lib" }
EOF

if ! bash "$ATTESTED_SEEN" "$COMPILER" pkg fetch \
    "$CONSUMER_DIR" >/dev/null 2>&1; then
    echo "FAIL: legacy system dependency with native path regressed"
    exit 1
fi

cat >"$BAD_PACKAGE_DIR/Seen.toml" <<'EOF'
manifest-version = 1
[project]
name = "../escape"
version = "0.1.0"

[package]
identity = "alice/escape"
visibility = "public"
include = ["src/**/*.seen"]
assets = []
capabilities = []
[dependencies]
EOF
cat >"$BAD_PACKAGE_DIR/src/value.seen" <<'EOF'
pub fun answer() r: Int { return 0 }
EOF

if bash "$ATTESTED_SEEN" "$COMPILER" pkg publish "$BAD_PACKAGE_DIR" \
    --registry "$REGISTRY_DIR" "${PUBLISH_ARGS[@]}" >"$TMP_DIR/bad-project.log" 2>&1; then
    echo "FAIL: traversal-shaped project name was accepted for publishing"
    exit 1
fi
grep -Fq 'project name "../escape" must match' "$TMP_DIR/bad-project.log"

echo "PASS: package aliases and scoped registry identities stay distinct"
