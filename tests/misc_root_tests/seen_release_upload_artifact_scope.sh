#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TMP_DIR="$(mktemp -d /tmp/seen_release_upload_scope.XXXXXX)"
trap 'rm -rf "$TMP_DIR"' EXIT

FIXTURE_ROOT="$TMP_DIR/repo"
FIXTURE_BIN="$FIXTURE_ROOT/bin"
MIN_PATH="$TMP_DIR/path"
OPTIONAL_PATH="$TMP_DIR/optional-path"
DIST_DIR="$FIXTURE_ROOT/dist"
STALE_ARTIFACT="$DIST_DIR/seen-0.8.0-linux-x64.tar.gz"
CURRENT_ARTIFACT="$DIST_DIR/seen-0.10.1-linux-x64.tar.gz"
COMPILER_COMPONENT="$DIST_DIR/seen-compiler-0.10.1-linux-x64"
RUNTIME_COMPONENT="$DIST_DIR/seen-runtime-0.10.1-linux-x64.tar.gz"
STDLIB_COMPONENT="$DIST_DIR/seen-stdlib-0.10.1-linux-x64.tar.gz"
PACKAGE_CLIENT_COMPONENT="$DIST_DIR/seen-pkg-0.10.1-linux-x64"
STALE_OPTIONAL_ARTIFACT="$DIST_DIR/seen-lang_0.10.1_amd64.deb"
IMPLICIT_MACOS_ARTIFACT="$DIST_DIR/seen-0.10.1-macos-arm64.tar.gz"

mkdir -p "$FIXTURE_ROOT/scripts" "$FIXTURE_BIN" "$MIN_PATH" "$OPTIONAL_PATH" "$DIST_DIR"
cp "$ROOT_DIR/scripts/build_and_upload_release.sh" "$FIXTURE_ROOT/scripts/"
cp "$ROOT_DIR/scripts/release_tag_policy.sh" "$FIXTURE_ROOT/scripts/"
cat > "$FIXTURE_ROOT/scripts/run_in_hard_memory_scope.sh" <<'SCOPE_EOF'
#!/usr/bin/env bash
set -euo pipefail
[[ "${1:-}" == "--verify-only" ]] || exit 126
SCOPE_EOF

cat > "$FIXTURE_BIN/seen" <<'SEEN_EOF'
#!/usr/bin/env bash
set -euo pipefail
if [[ "${1:-}" == "compile" ]]; then
    compile_args=" $* "
    if [[ "$compile_args" != *" --jobs 1 "* ||
        "$compile_args" != *" --opt-jobs 1 "* ||
        "$compile_args" != *" --no-fork "* ]]; then

        echo "release smoke omitted serial compiler flags" >&2
        exit 1
    fi
    printf '#!/usr/bin/env bash\nexit 0\n' >"${3:?}"
    chmod +x "${3:?}"
    exit 0
fi
exit 2
SEEN_EOF

cat > "$FIXTURE_BIN/seen-pkg" <<'PKG_EOF'
#!/usr/bin/env bash
exit 0
PKG_EOF

cat > "$FIXTURE_ROOT/scripts/build_release.sh" <<'BUILD_EOF'
#!/usr/bin/env bash
set -euo pipefail
version=""
output_dir=""
artifact_suffix=""
while [[ "$#" -gt 0 ]]; do
    case "$1" in
        --version)
            version="$2"
            shift 2
            ;;
        --output-dir)
            output_dir="$2"
            shift 2
            ;;
        --artifact-suffix)
            artifact_suffix="$2"
            shift 2
            ;;
        *)
            shift
            ;;
    esac
done
mkdir -p "$output_dir"
printf 'current release artifact\n' > "$output_dir/seen-$version-$artifact_suffix.tar.gz"
if [[ "$artifact_suffix" == "linux-x64" ]]; then
    printf 'compiler\n' > "$output_dir/seen-compiler-$version-linux-x64"
    printf 'runtime\n' > "$output_dir/seen-runtime-$version-linux-x64.tar.gz"
    printf 'stdlib\n' > "$output_dir/seen-stdlib-$version-linux-x64.tar.gz"
    printf 'package-client\n' > "$output_dir/seen-pkg-$version-linux-x64"
fi
BUILD_EOF

cat > "$FIXTURE_ROOT/scripts/run_with_project_artifacts.sh" <<'WRAPPER_EOF'
#!/usr/bin/env bash
set -euo pipefail
shift
[[ "${1:-}" == "--" ]] || exit 2
shift
exec "$@"
WRAPPER_EOF

cat > "$FIXTURE_ROOT/scripts/sign_release.sh" <<'SIGN_EOF'
#!/usr/bin/env bash
set -euo pipefail
manifest=""; artifacts=()
while [[ $# -gt 0 ]]; do
    case "$1" in
        --manifest) manifest="$2"; shift 2 ;;
        --artifact) artifacts+=("${2#*=}"); shift 2 ;;
        --signer-identity)
            [[ "$2" == '^https://github\.com/codeyousef/SeenLang/\.github/workflows/release\.yml@refs/tags/v0\.10\.1$' ]] || exit 1
            shift 2
            ;;
        --signer-issuer)
            [[ "$2" == 'https://token.actions.githubusercontent.com' ]] || exit 1
            shift 2
            ;;
        --key|--version|--source-commit|--source-digest) shift 2 ;;
        *) shift ;;
    esac
done
[[ -n "$manifest" && "${#artifacts[@]}" -eq 4 ]] || exit 1
for artifact in "${artifacts[@]}"; do
    sha256sum "$artifact" | awk '{print $1}' >"$artifact.sha256"
    printf 'bundle\n' >"$artifact.bundle"
done
printf '{}\n' >"$manifest"
sha256sum "$manifest" | awk '{print $1}' >"$manifest.sha256"
printf 'bundle\n' >"$manifest.bundle"
SIGN_EOF

cat > "$FIXTURE_BIN/git" <<'GIT_EOF'
#!/usr/bin/env bash
set -euo pipefail
if [[ "${1:-}" == "-C" ]]; then
    shift 2
fi
case "${1:-}" in
    archive)
        printf 'fixture archive\n'
        ;;
    rev-parse)
        case "${*: -1}" in
            HEAD|refs/tags/v0.10.1^{commit})
                printf '1111111111111111111111111111111111111111\n'
                ;;
            refs/tags/v0.10.1)
                case "${GIT_LOCAL_TAG_MODE:-annotated}" in
                    annotated) printf '2222222222222222222222222222222222222222\n' ;;
                    lightweight) printf '1111111111111111111111111111111111111111\n' ;;
                    *) exit 2 ;;
                esac
                ;;
            *)
                echo "unexpected git rev-parse invocation: $*" >&2
                exit 2
                ;;
        esac
        ;;
    cat-file)
        case "${GIT_LOCAL_TAG_MODE:-annotated}" in
            annotated) printf 'tag\n' ;;
            lightweight) printf 'commit\n' ;;
            *) exit 2 ;;
        esac
        ;;
    remote)
        [[ "${2:-}" == "get-url" && "${3:-}" == "origin" ]] || exit 2
        case "${GIT_REMOTE_URL_MODE:-canonical}" in
            canonical) printf 'https://github.com/codeyousef/SeenLang.git\n' ;;
            mismatch) printf 'https://github.com/fork/SeenLang.git\n' ;;
            *) exit 2 ;;
        esac
        ;;
    ls-remote)
        case "${GIT_REMOTE_TAG_MODE:-exact}" in
            exact)
                printf '1111111111111111111111111111111111111111\trefs/heads/main\n'
                printf '2222222222222222222222222222222222222222\trefs/tags/v0.10.1\n'
                printf '1111111111111111111111111111111111111111\trefs/tags/v0.10.1^{}\n'
                ;;
            absent)
                printf '1111111111111111111111111111111111111111\trefs/heads/main\n'
                ;;
            object-mismatch)
                printf '1111111111111111111111111111111111111111\trefs/heads/main\n'
                printf '3333333333333333333333333333333333333333\trefs/tags/v0.10.1\n'
                printf '1111111111111111111111111111111111111111\trefs/tags/v0.10.1^{}\n'
                ;;
            commit-mismatch)
                printf '1111111111111111111111111111111111111111\trefs/heads/main\n'
                printf '2222222222222222222222222222222222222222\trefs/tags/v0.10.1\n'
                printf '3333333333333333333333333333333333333333\trefs/tags/v0.10.1^{}\n'
                ;;
            main-mismatch)
                printf '3333333333333333333333333333333333333333\trefs/heads/main\n'
                printf '2222222222222222222222222222222222222222\trefs/tags/v0.10.1\n'
                printf '1111111111111111111111111111111111111111\trefs/tags/v0.10.1^{}\n'
                ;;
            lightweight)
                printf '1111111111111111111111111111111111111111\trefs/heads/main\n'
                printf '1111111111111111111111111111111111111111\trefs/tags/v0.10.1\n'
                ;;
            *) exit 2 ;;
        esac
        ;;
    *)
        echo "unexpected git invocation: $*" >&2
        exit 2
        ;;
esac
GIT_EOF

cat > "$FIXTURE_BIN/gh" <<'GH_EOF'
#!/usr/bin/env bash
set -euo pipefail
case "${1:-} ${2:-}" in
    "auth status")
        exit 0
        ;;
    "api --include")
        case "${GH_RELEASE_MODE:-absent}" in
            absent)
                printf 'HTTP/2.0 404 Not Found\n'
                exit 1
                ;;
            existing)
                printf 'HTTP/2.0 200 OK\n\n{"tag_name":"v0.10.1","assets":[]}\n'
                ;;
            error)
                printf 'network unavailable\n' >&2
                exit 2
                ;;
            *) exit 2 ;;
        esac
        ;;
    "release create")
        printf '%s\0' "$@" > "${GH_CAPTURE:?}"
        ;;
    *)
        echo "unexpected gh invocation: $*" >&2
        exit 2
        ;;
esac
GH_EOF

chmod 755 \
    "$FIXTURE_BIN/seen" \
    "$FIXTURE_BIN/seen-pkg" \
    "$FIXTURE_BIN/git" \
    "$FIXTURE_BIN/gh" \
    "$FIXTURE_ROOT/scripts/build_release.sh" \
    "$FIXTURE_ROOT/scripts/run_in_hard_memory_scope.sh" \
    "$FIXTURE_ROOT/scripts/run_with_project_artifacts.sh" \
    "$FIXTURE_ROOT/scripts/sign_release.sh"

export SEEN_RELEASE_CONTAINMENT_IN_SCOPE=1
export SEEN_RELEASE_PROJECT_WRAPPER="$FIXTURE_ROOT/scripts/run_with_project_artifacts.sh"
export SEEN_JOBS=1 SEEN_OPT_JOBS=1 SEEN_PACKAGE_JOBS=1 SEEN_NO_FORK=1

for tool in awk bash basename cat chmod cp dirname grep ls mkdir mktemp rm sha256sum sort; do
    ln -s "$(command -v "$tool")" "$MIN_PATH/$tool"
done

cat > "$OPTIONAL_PATH/dpkg-deb" <<'DPKG_EOF'
#!/usr/bin/env bash
exit 0
DPKG_EOF
chmod 755 "$OPTIONAL_PATH/dpkg-deb"

assert_checksum_scope() {
    local expected_macos="${1:-}"
    if ! grep -Fq 'seen-0.10.1-linux-x64.tar.gz' "$DIST_DIR/SHA256SUMS"; then
        echo "SHA256SUMS omitted the current release artifact" >&2
        exit 1
    fi
    if grep -Fq 'seen-0.8.0-linux-x64.tar.gz' "$DIST_DIR/SHA256SUMS"; then
        echo "SHA256SUMS included a stale release artifact" >&2
        exit 1
    fi
    if [[ -z "$expected_macos" ]] && grep -Fq "$(basename "$IMPLICIT_MACOS_ARTIFACT")" "$DIST_DIR/SHA256SUMS"; then
        echo "SHA256SUMS included an implicit stale macOS artifact" >&2
        exit 1
    fi
    if [[ -n "$expected_macos" ]] && ! grep -Fq "$(basename "$expected_macos")" "$DIST_DIR/SHA256SUMS"; then
        echo "SHA256SUMS omitted the explicit macOS input" >&2
        exit 1
    fi
}

assert_release_args() {
    local capture="$1"
    local expected_macos="${2:-}"
    local arg index saw_current=0 saw_checksums=0 saw_macos=0 saw_compiler=0 saw_runtime=0 saw_stdlib=0 saw_package_client=0 saw_repo=0 saw_verify_tag=0
    local -a args=()

    mapfile -d '' -t args < "$capture"
    if [[ "${args[0]:-}" != "release" || "${args[1]:-}" != "create" ||
        "${args[2]:-}" != "v0.10.1" ]]; then
        echo "unexpected gh release create prefix" >&2
        exit 1
    fi

    for index in "${!args[@]}"; do
        arg="${args[$index]}"
        if [[ "$arg" == "$STALE_ARTIFACT" || "$arg" == *seen-0.8.0-linux-x64.tar.gz* ]]; then
            echo "gh release create included a stale release artifact" >&2
            exit 1
        fi
        if [[ "$arg" == "$STALE_OPTIONAL_ARTIFACT" ]]; then
            echo "gh release create included a stale same-version optional artifact" >&2
            exit 1
        fi
        [[ "$arg" == "$CURRENT_ARTIFACT" ]] && saw_current=1
        [[ "$arg" == "$COMPILER_COMPONENT" ]] && saw_compiler=1
        [[ "$arg" == "$RUNTIME_COMPONENT" ]] && saw_runtime=1
        [[ "$arg" == "$STDLIB_COMPONENT" ]] && saw_stdlib=1
        [[ "$arg" == "$PACKAGE_CLIENT_COMPONENT" ]] && saw_package_client=1
        [[ "$arg" == "$DIST_DIR/SHA256SUMS" ]] && saw_checksums=1
        [[ -n "$expected_macos" && "$arg" == "$expected_macos" ]] && saw_macos=1
        if [[ "$arg" == "$DIST_DIR/"*.tar.gz && "$arg" != "$CURRENT_ARTIFACT" && "$arg" != "$RUNTIME_COMPONENT" && "$arg" != "$STDLIB_COMPONENT" && "$arg" != "$expected_macos" ]]; then
            echo "gh release create included an unexpected tarball: $arg" >&2
            exit 1
        fi
        if [[ "$arg" == "--clobber" ]]; then
            echo "gh release create requested asset replacement" >&2
            exit 1
        fi
        if [[ "$arg" == "--repo" && "${args[$((index + 1))]:-}" == "codeyousef/SeenLang" ]]; then
            saw_repo=1
        fi
        [[ "$arg" == "--verify-tag" ]] && saw_verify_tag=1
    done

    if [[ "$saw_current" -ne 1 || "$saw_checksums" -ne 1 || "$saw_compiler" -ne 1 ||
        "$saw_runtime" -ne 1 || "$saw_stdlib" -ne 1 || "$saw_package_client" -ne 1 ||
        "$saw_repo" -ne 1 || "$saw_verify_tag" -ne 1 ]]; then
        echo "gh release create omitted the scoped artifact set" >&2
        exit 1
    fi
    if [[ -n "$expected_macos" && "$saw_macos" -ne 1 ]]; then
        echo "gh release create omitted the explicit macOS input" >&2
        exit 1
    fi
}

run_release_case() {
    local remote_tag_mode="${1:-exact}"
    local capture="$TMP_DIR/gh-create-$remote_tag_mode.args"

    printf 'stale release artifact\n' > "$STALE_ARTIFACT"
    printf 'implicit stale macOS artifact\n' > "$IMPLICIT_MACOS_ARTIFACT"
    PATH="$FIXTURE_BIN:$MIN_PATH" \
        GH_CAPTURE="$capture" \
        GH_RELEASE_MODE=absent \
        GIT_REMOTE_TAG_MODE="$remote_tag_mode" \
        SEEN_LINUX_X64_COMPILER="$FIXTURE_BIN/seen" \
        SEEN_LINUX_X64_V3_COMPILER="$FIXTURE_BIN/seen-v3-absent" \
        SEEN_PACKAGE_CLIENT_BIN="$FIXTURE_BIN/seen-pkg" \
        SEEN_RELEASE_SIGN_MODE=key SEEN_COSIGN_KEY="$TMP_DIR/test.key" \
        "$FIXTURE_ROOT/scripts/build_and_upload_release.sh" 0.10.1 >/dev/null

    assert_checksum_scope
    assert_release_args "$capture"
}

run_rejected_tag_case() {
    local remote_tag_mode="$1"
    local local_tag_mode="${2:-annotated}"
    local expected="$3"
    local capture="$TMP_DIR/gh-rejected-$remote_tag_mode-$local_tag_mode.args"
    local output status

    set +e
    output="$(PATH="$FIXTURE_BIN:$MIN_PATH" \
        GH_CAPTURE="$capture" \
        GH_RELEASE_MODE=absent \
        GIT_REMOTE_TAG_MODE="$remote_tag_mode" \
        GIT_LOCAL_TAG_MODE="$local_tag_mode" \
        SEEN_LINUX_X64_COMPILER="$FIXTURE_BIN/seen" \
        SEEN_LINUX_X64_V3_COMPILER="$FIXTURE_BIN/seen-v3-absent" \
        SEEN_PACKAGE_CLIENT_BIN="$FIXTURE_BIN/seen-pkg" \
        SEEN_RELEASE_SIGN_MODE=key SEEN_COSIGN_KEY="$TMP_DIR/test.key" \
        "$FIXTURE_ROOT/scripts/build_and_upload_release.sh" 0.10.1 2>&1)"
    status=$?
    set -e

    [[ "$status" -ne 0 ]] || {
        echo "release accepted rejected tag state: $remote_tag_mode/$local_tag_mode" >&2
        exit 1
    }
    grep -Fq "$expected" \
        <<<"$output" || {
        echo "$output" >&2
        echo "release did not report rejected tag state: $remote_tag_mode/$local_tag_mode" >&2
        exit 1
    }
    [[ ! -e "$capture" ]] || {
        echo "release mutated GitHub state after a rejected tag" >&2
        exit 1
    }
}

run_release_preflight_rejection() {
    local mode="$1"
    local expected="$2"
    local capture="$TMP_DIR/gh-release-$mode.args"
    local output status

    set +e
    output="$(PATH="$FIXTURE_BIN:$MIN_PATH" \
        GH_CAPTURE="$capture" \
        GH_RELEASE_MODE="$mode" \
        GIT_REMOTE_TAG_MODE=exact \
        SEEN_LINUX_X64_COMPILER="$FIXTURE_BIN/seen" \
        SEEN_LINUX_X64_V3_COMPILER="$FIXTURE_BIN/seen-v3-absent" \
        SEEN_PACKAGE_CLIENT_BIN="$FIXTURE_BIN/seen-pkg" \
        SEEN_RELEASE_SIGN_MODE=key SEEN_COSIGN_KEY="$TMP_DIR/test.key" \
        "$FIXTURE_ROOT/scripts/build_and_upload_release.sh" 0.10.1 2>&1)"
    status=$?
    set -e

    [[ "$status" -ne 0 ]] || {
        echo "release preflight accepted $mode release state" >&2
        exit 1
    }
    grep -Fq "$expected" <<<"$output" || {
        echo "$output" >&2
        echo "release preflight omitted $mode diagnostic" >&2
        exit 1
    }
    [[ ! -e "$capture" ]] || {
        echo "release preflight mutated an existing or unknown release" >&2
        exit 1
    }
}

run_repository_rejection() {
    local repository="$1"
    local remote_url_mode="$2"
    local expected="$3"
    local capture="$TMP_DIR/gh-repository-${repository//\//-}-$remote_url_mode.args"
    local output status

    set +e
    output="$(PATH="$FIXTURE_BIN:$MIN_PATH" \
        GH_CAPTURE="$capture" \
        GH_RELEASE_MODE=absent \
        GITHUB_REPOSITORY="$repository" \
        GIT_REMOTE_URL_MODE="$remote_url_mode" \
        SEEN_LINUX_X64_COMPILER="$FIXTURE_BIN/seen" \
        SEEN_LINUX_X64_V3_COMPILER="$FIXTURE_BIN/seen-v3-absent" \
        SEEN_PACKAGE_CLIENT_BIN="$FIXTURE_BIN/seen-pkg" \
        SEEN_RELEASE_SIGN_MODE=key SEEN_COSIGN_KEY="$TMP_DIR/test.key" \
        "$FIXTURE_ROOT/scripts/build_and_upload_release.sh" 0.10.1 2>&1)"
    status=$?
    set -e

    [[ "$status" -ne 0 ]] || {
        echo "release accepted mismatched repository state" >&2
        exit 1
    }
    grep -Fq "$expected" <<<"$output" || {
        echo "$output" >&2
        echo "release omitted repository mismatch diagnostic" >&2
        exit 1
    }
    [[ ! -e "$capture" ]] || {
        echo "release mutated a repository whose tag was not verified" >&2
        exit 1
    }
}

run_dry_run_case() {
    local capture="$TMP_DIR/gh-dry-run.args"

    PATH="$FIXTURE_BIN:$MIN_PATH" \
        GH_CAPTURE="$capture" \
        SEEN_LINUX_X64_COMPILER="$FIXTURE_BIN/seen" \
        SEEN_LINUX_X64_V3_COMPILER="$FIXTURE_BIN/seen-v3-absent" \
        SEEN_PACKAGE_CLIENT_BIN="$FIXTURE_BIN/seen-pkg" \
        SEEN_RELEASE_DRY_RUN=1 \
        "$FIXTURE_ROOT/scripts/build_and_upload_release.sh" 0.10.1 >/dev/null

    assert_checksum_scope
    if [[ -e "$capture" ]]; then
        echo "local dry run contacted GitHub" >&2
        exit 1
    fi
}

run_broad_identity_rejection() {
    local output status

    set +e
    output="$(PATH="$FIXTURE_BIN:$MIN_PATH" \
        SEEN_LINUX_X64_COMPILER="$FIXTURE_BIN/seen" \
        SEEN_LINUX_X64_V3_COMPILER="$FIXTURE_BIN/seen-v3-absent" \
        SEEN_PACKAGE_CLIENT_BIN="$FIXTURE_BIN/seen-pkg" \
        SEEN_RELEASE_DRY_RUN=1 \
        SEEN_RELEASE_SIGN_IDENTITY='github.com/.*SeenLang' \
        "$FIXTURE_ROOT/scripts/build_and_upload_release.sh" 0.10.1 2>&1)"
    status=$?
    set -e

    [[ "$status" -ne 0 ]] || {
        echo "release uploader accepted a broad signer identity" >&2
        exit 1
    }
    grep -Fq 'must be the exact anchored release.yml tag identity' <<<"$output" || {
        echo "$output" >&2
        echo "release uploader omitted its broad-identity diagnostic" >&2
        exit 1
    }
}

run_failed_optional_case() {
    local capture="$TMP_DIR/gh-failed-optional.args"
    local output status

    printf 'stale release artifact\n' > "$STALE_ARTIFACT"
    printf 'stale same-version optional artifact\n' > "$STALE_OPTIONAL_ARTIFACT"
    printf 'stale checksums\n' > "$DIST_DIR/SHA256SUMS"
    printf 'stale formula\n' > "$DIST_DIR/seen-lang.rb"
    printf 'stale sidecar\n' > "$DIST_DIR/seen-0.10.1-windows-x64.zip.sha256"
    set +e
    output="$(PATH="$FIXTURE_BIN:$OPTIONAL_PATH:$MIN_PATH" \
        GH_CAPTURE="$capture" \
        GH_RELEASE_MODE=absent \
        SEEN_LINUX_X64_COMPILER="$FIXTURE_BIN/seen" \
        SEEN_LINUX_X64_V3_COMPILER="$FIXTURE_BIN/seen-v3-absent" \
        SEEN_PACKAGE_CLIENT_BIN="$FIXTURE_BIN/seen-pkg" \
        SEEN_RELEASE_SIGN_MODE=key SEEN_COSIGN_KEY="$TMP_DIR/test.key" \
        "$FIXTURE_ROOT/scripts/build_and_upload_release.sh" 0.10.1 2>&1)"
    status=$?
    set -e

    if [[ "$status" -eq 0 ]]; then
        echo "stale same-version optional artifact satisfied the release gate" >&2
        exit 1
    fi
    if [[ -e "$STALE_OPTIONAL_ARTIFACT" ]]; then
        echo "release preparation retained a stale same-version optional artifact" >&2
        exit 1
    fi
    if [[ ! -s "$STALE_ARTIFACT" ]]; then
        echo "version-scoped cleanup removed an unrelated release artifact" >&2
        exit 1
    fi
    for generated in \
        "$DIST_DIR/SHA256SUMS" \
        "$DIST_DIR/seen-lang.rb" \
        "$DIST_DIR/seen-0.10.1-windows-x64.zip.sha256"; do
        if [[ -e "$generated" ]]; then
            echo "release preparation retained stale generated output: $generated" >&2
            exit 1
        fi
    done
    if ! grep -Fq "required release artifact missing or empty: $STALE_OPTIONAL_ARTIFACT" <<<"$output"; then
        echo "$output" >&2
        echo "release did not fail closed on the skipped optional artifact" >&2
        exit 1
    fi
    if [[ -e "$capture" ]]; then
        echo "release reached gh with a failed optional artifact build" >&2
        exit 1
    fi
}

run_explicit_macos_case() {
    local input_dir="$TMP_DIR/macos-input"
    local input_artifact="$input_dir/seen-0.10.1-macos-arm64.tar.gz"
    local copied_artifact="$DIST_DIR/$(basename "$input_artifact")"
    local capture="$TMP_DIR/gh-explicit-macos.args"

    mkdir -p "$input_dir"
    printf 'explicit macOS release artifact\n' > "$input_artifact"
    PATH="$FIXTURE_BIN:$MIN_PATH" \
        GH_CAPTURE="$capture" \
        GH_RELEASE_MODE=absent \
        SEEN_LINUX_X64_COMPILER="$FIXTURE_BIN/seen" \
        SEEN_LINUX_X64_V3_COMPILER="$FIXTURE_BIN/seen-v3-absent" \
        SEEN_PACKAGE_CLIENT_BIN="$FIXTURE_BIN/seen-pkg" \
        SEEN_RELEASE_SIGN_MODE=key SEEN_COSIGN_KEY="$TMP_DIR/test.key" \
        SEEN_RELEASE_MACOS_INPUT_DIR="$input_dir" \
        "$FIXTURE_ROOT/scripts/build_and_upload_release.sh" 0.10.1 >/dev/null

    if [[ ! -s "$input_artifact" || ! -s "$copied_artifact" ]]; then
        echo "explicit macOS input was not preserved and staged" >&2
        exit 1
    fi
    assert_checksum_scope "$copied_artifact"
    assert_release_args "$capture" "$copied_artifact"
}

if grep -Fq -- '--clobber' "$FIXTURE_ROOT/scripts/build_and_upload_release.sh"; then
    echo "release uploader retains an asset overwrite path" >&2
    exit 1
fi
if grep -Fq 'git -C "$ROOT_DIR" push' "$FIXTURE_ROOT/scripts/build_and_upload_release.sh" ||
    grep -Fq 'git -C "$ROOT_DIR" tag' "$FIXTURE_ROOT/scripts/build_and_upload_release.sh"; then

    echo "release uploader retains a tag publication path" >&2
    exit 1
fi

run_failed_optional_case
run_broad_identity_rejection
run_dry_run_case
run_release_case exact
run_release_preflight_rejection existing 'ordinary publishing never modifies existing releases or assets'
run_release_preflight_rejection error 'Could not prove that release v0.10.1 is absent'
run_repository_rejection fork/SeenLang canonical 'GITHUB_REPOSITORY must identify codeyousef/SeenLang'
run_repository_rejection codeyousef/SeenLang mismatch 'origin does not identify expected repository codeyousef/SeenLang'
run_rejected_tag_case absent annotated 'remote tag v0.10.1 is missing or ambiguous'
run_rejected_tag_case object-mismatch annotated 'differs from local tag object'
run_rejected_tag_case commit-mismatch annotated 'peels to 3333333333333333333333333333333333333333'
run_rejected_tag_case main-mismatch annotated 'remote main is 3333333333333333333333333333333333333333'
run_rejected_tag_case lightweight lightweight 'local tag v0.10.1 is not annotated'
run_explicit_macos_case

echo "release upload artifact scope test passed"
