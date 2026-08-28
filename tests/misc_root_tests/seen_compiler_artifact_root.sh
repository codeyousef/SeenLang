#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -P)"
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen-dev}"
TEST_PARENT="$ROOT_DIR/.seen/agent-tools/tests"
TEST_ROOT="$TEST_PARENT/compiler-artifact-root"
PROJECT_ROOT="$TEST_ROOT/project"
ARTIFACT_ROOT="$PROJECT_ROOT/ignored artifacts/compiler test"

fail() {
    printf 'FAIL: %s\n' "$*" >&2
    exit 1
}

safe_cleanup() {
    case "$TEST_ROOT" in
        "$ROOT_DIR"/.seen/agent-tools/tests/compiler-artifact-root) ;;
        *) return 1 ;;
    esac
    [ ! -L "$TEST_PARENT" ] || return 1
    [ ! -L "$TEST_ROOT" ] || return 1
    rm -rf -- "$TEST_ROOT"
}

if rg -n '"/tmp(?:/|\\b)' "$ROOT_DIR/compiler_seen/src/main_compiler.seen";
then
    fail "compiler source still contains a hardcoded absolute temporary path"
fi
rg -q 'fun configureCompilerArtifactRoot' \
    "$ROOT_DIR/compiler_seen/src/main_compiler.seen" ||
    fail "compiler artifact-root resolver is missing"
rg -q 'artifactPathIsGitIgnored' \
    "$ROOT_DIR/compiler_seen/src/main_compiler.seen" ||
    fail "compiler does not validate non-.seen overrides against Git ignore rules"
rg -q 'artifactPathTraversesExistingSymlink' \
    "$ROOT_DIR/compiler_seen/src/main_compiler.seen" ||
    fail "compiler does not reject symlink components before creating its root"
rg -q 'removeCompilerArtifactTree' \
    "$ROOT_DIR/compiler_seen/src/main_compiler.seen" ||
    fail "compiler recursive artifact cleanup is not root-guarded"
rg -q 'compilerArtifactPath\("seen_ir_cache/' \
    "$ROOT_DIR/compiler_seen/src/main_compiler.seen" ||
    fail "compiler IR cache is not routed through the artifact root"
rg -q 'compilerArtifactPath\("seen_thinlto_cache"\)' \
    "$ROOT_DIR/compiler_seen/src/main_compiler.seen" ||
    fail "compiler ThinLTO cache is not routed through the artifact root"
for quoted_name in irCacheFile irCacheObj \
    sanitizedRuntimeObj sanitizedTeeRuntimeObj rtTargetOut teeTargetOut \
    targetRegionOut targetGpuOut targetHotReloadOut platformShimOut jitObj \
    tmpX86 tmpArm tmpOut buildDir appDir; do
    rg -q "mainCompilerShellQuote\\($quoted_name" \
        "$ROOT_DIR/compiler_seen/src/main_compiler.seen" ||
        fail "compiler artifact operand is not shell-quoted: $quoted_name"
done
rg -Fq 'storeCachedObjectAtomically(objFile, irCacheDest' \
    "$ROOT_DIR/compiler_seen/src/main_compiler.seen" ||
    fail "compiler IR cache destination bypasses the quoting atomic publisher"
! rg -q 'compilerArtifactPath\("seen_pgo\.profdata"\)' \
    "$ROOT_DIR/compiler_seen/src/main_compiler.seen" ||
    fail "compiler still performs implicit raw-profile merging"
rg -q 'canonical relative \.profdata path' \
    "$ROOT_DIR/compiler_seen/src/release/release_optimization.seen" ||
    fail "release policy does not require caller-supplied canonical profile data"

if [ "${SEEN_ARTIFACT_STATIC_ONLY:-0}" = "1" ]; then
    printf 'compiler artifact-root static checks passed\n'
    exit 0
fi

[ -x "$COMPILER" ] || fail "fresh compiler is not executable: $COMPILER"
mkdir -p -- "$TEST_PARENT"
safe_cleanup
mkdir -p -- "$PROJECT_ROOT/src"
trap safe_cleanup EXIT

git init -q "$PROJECT_ROOT"
printf '%s\n' '.seen/' '/ignored artifacts/' '/ignored-link/' > \
    "$PROJECT_ROOT/.gitignore"

printf '%s\n' \
    'manifest-version = 1' \
    '' \
    '[project]' \
    'name = "artifact_root_fixture"' \
    'version = "0.1.0"' \
    'language = "en"' \
    '' \
    '[dependencies]' > "$PROJECT_ROOT/Seen.toml"
printf '%s\n' \
    'fun main() {' \
    '    println("artifact root")' \
    '}' > "$PROJECT_ROOT/src/main.seen"

SEEN_PROJECT_ROOT="$PROJECT_ROOT" \
SEEN_ARTIFACT_ROOT="$ARTIFACT_ROOT" \
SEEN_DATA_PATH="$ROOT_DIR" \
    "$COMPILER" compile "$PROJECT_ROOT/src/main.seen" \
    "$TEST_ROOT/artifact-root-bin" --no-cache --no-fork >/dev/null

[ -x "$TEST_ROOT/artifact-root-bin" ] ||
    fail "compiler did not produce the requested output"
[ -d "$ARTIFACT_ROOT" ] ||
    fail "compiler did not create the requested project-local artifact root"
find "$ARTIFACT_ROOT" -maxdepth 1 -type d -name 'seen_compile_*' \
    -print -quit | grep -q . ||
    fail "compiler did not place its compile directory under the artifact root"

nonignored_root="$PROJECT_ROOT/tracked-artifacts"
if SEEN_PROJECT_ROOT="$PROJECT_ROOT" \
    SEEN_ARTIFACT_ROOT="$nonignored_root" \
    SEEN_DATA_PATH="$ROOT_DIR" \
        "$COMPILER" compile "$PROJECT_ROOT/src/main.seen" \
        "$TEST_ROOT/nonignored-bin" --no-cache --no-fork \
        >"$TEST_ROOT/nonignored.out" 2>"$TEST_ROOT/nonignored.err"; then
    fail "compiler accepted a non-ignored artifact root"
fi
[ ! -e "$nonignored_root" ] ||
    fail "compiler created a rejected non-ignored artifact root"

mkdir -p -- "$TEST_ROOT/symlink-target"
ln -s -- "$TEST_ROOT/symlink-target" "$PROJECT_ROOT/ignored-link"
if SEEN_PROJECT_ROOT="$PROJECT_ROOT" \
    SEEN_ARTIFACT_ROOT="$PROJECT_ROOT/ignored-link/child" \
    SEEN_DATA_PATH="$ROOT_DIR" \
        "$COMPILER" compile "$PROJECT_ROOT/src/main.seen" \
        "$TEST_ROOT/symlink-bin" --no-cache --no-fork \
        >"$TEST_ROOT/symlink.out" 2>"$TEST_ROOT/symlink.err"; then
    fail "compiler accepted an artifact root through a symbolic link"
fi
[ ! -e "$TEST_ROOT/symlink-target/child" ] ||
    fail "compiler created a child through a rejected symbolic link"

if SEEN_PROJECT_ROOT="$PROJECT_ROOT" \
    SEEN_ARTIFACT_ROOT="$TEST_ROOT/outside-project" \
    SEEN_DATA_PATH="$ROOT_DIR" \
        "$COMPILER" compile "$PROJECT_ROOT/src/main.seen" \
        "$TEST_ROOT/rejected-bin" --no-cache --no-fork \
        >"$TEST_ROOT/rejected.out" 2>"$TEST_ROOT/rejected.err"; then
    fail "compiler accepted an artifact root outside the project"
fi
[ ! -e "$TEST_ROOT/outside-project" ] ||
    fail "compiler created a rejected artifact root"

printf 'compiler artifact-root tests passed\n'
