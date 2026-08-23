#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/.." && pwd -P)"
ARTIFACT_HELPER="$ROOT_DIR/scripts/artifact_root.sh"
CHECKER="$ROOT_DIR/scripts/check_release_ci_run.py"
TOOLCHAIN_CHECKER="$ROOT_DIR/scripts/release_toolchain_artifact.py"

fail() {
    echo "release-ci: core.004b.invalid: $*" >&2
    exit 1
}

[[ "${GITHUB_REPOSITORY:-}" =~ ^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$ ]] ||
    fail "GITHUB_REPOSITORY is invalid"
[[ "${GITHUB_SHA:-}" =~ ^[0-9a-f]{40}$ ]] || fail "GITHUB_SHA is invalid"
[[ "${GITHUB_REF_TYPE:-}" == "tag" && "${GITHUB_REF_NAME:-}" == v* ]] ||
    fail "release verification requires a version tag"
[ -f "$ARTIFACT_HELPER" ] && [ ! -L "$ARTIFACT_HELPER" ] ||
    fail "artifact-root helper is missing or unsafe"
[ -f "$CHECKER" ] && [ ! -L "$CHECKER" ] || fail "CI evidence checker is missing or unsafe"
[ -f "$TOOLCHAIN_CHECKER" ] && [ ! -L "$TOOLCHAIN_CHECKER" ] ||
    fail "release toolchain checker is missing or unsafe"
command -v gh >/dev/null 2>&1 || fail "gh CLI is unavailable"
if [ -z "${GH_TOKEN:-}" ]; then
    gh auth status >/dev/null 2>&1 || fail "GitHub authentication is unavailable"
fi

tag_commit=$(git -C "$ROOT_DIR" rev-parse "${GITHUB_REF_NAME}^{commit}") ||
    fail "could not resolve release tag"
[ "$tag_commit" = "$GITHUB_SHA" ] ||
    fail "release tag resolves to $tag_commit instead of $GITHUB_SHA"

# shellcheck source=scripts/artifact_root.sh
source "$ARTIFACT_HELPER" || fail "could not load artifact-root helper"
seen_artifact_root_init "$ROOT_DIR" || fail "could not initialize artifact root"
scope_root=$(seen_artifact_scope_init release-ci-attestation) ||
    fail "could not create release CI evidence scope"
run_root=$(seen_artifact_mktemp_dir "$scope_root" run) ||
    fail "could not create release CI evidence directory"
evidence="$run_root/workflow-runs.json"

cleanup() {
    local status=$?
    case "$run_root" in
        "$scope_root"/run.*)
            [ -d "$run_root" ] && [ ! -L "$run_root" ] && rm -rf -- "$run_root"
            ;;
        *) return 1 ;;
    esac
    return "$status"
}
trap cleanup EXIT

gh api --method GET \
    "repos/$GITHUB_REPOSITORY/actions/workflows/ci.yml/runs" \
    -f head_sha="$GITHUB_SHA" -f branch=main -f event=push \
    -f status=completed -f per_page=20 > "$evidence" ||
    fail "could not fetch authoritative CI evidence"

report="$run_root/attestation.json"
python3 "$CHECKER" --evidence "$evidence" \
    --commit "$GITHUB_SHA" --repository "$GITHUB_REPOSITORY" > "$report"
cat "$report"
run_id=$(python3 -c \
    'import json,sys; value=json.load(open(sys.argv[1], encoding="utf-8")); run=value["run_id"]; assert isinstance(run, int) and not isinstance(run, bool) and run > 0; print(run)' \
    "$report") || fail "could not read the attested CI run identifier"

download_root="$run_root/toolchain"
mkdir -p "$download_root" || fail "could not create release toolchain download directory"
gh run download "$run_id" --repo "$GITHUB_REPOSITORY" \
    --name "seen-release-toolchain-$GITHUB_SHA" --dir "$download_root" ||
    fail "could not download the exact certified release toolchain"
archive="$download_root/release-toolchain.tar.gz"
[ -f "$archive" ] && [ ! -L "$archive" ] ||
    fail "certified release toolchain archive is missing or unsafe"
source_tree=$(git -C "$ROOT_DIR" rev-parse 'HEAD^{tree}') ||
    fail "could not resolve the release source tree"
python3 "$TOOLCHAIN_CHECKER" install --archive "$archive" --root "$ROOT_DIR" \
    --commit "$GITHUB_SHA" --tree "$source_tree" --cpu-baseline x86-64 ||
    fail "certified release toolchain validation or installation failed"
