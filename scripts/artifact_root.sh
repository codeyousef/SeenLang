#!/usr/bin/env bash
# Resolve a private, project-local root for disposable build and test artifacts.
#
# Source this file, then call:
#   seen_artifact_root_init "$REPO_ROOT"
#   seen_artifact_scope_init safe-rebuild
#
# SEEN_ARTIFACT_ROOT may override the default `.seen/agent-tools` location. A
# relative override is resolved from the repository root. Overrides must stay
# inside the repository, be ignored by Git, and contain no symbolic-link path
# components. These checks keep cleanup operations away from tracked or external
# user data.

seen_artifact_error() {
    printf 'ERROR: %s\n' "$*" >&2
}

seen_artifact_canonical_dir() {
    (
        cd -P -- "$1" 2>/dev/null || exit 1
        pwd -P
    )
}

seen_artifact_assert_safe_relative_path() {
    local relative_path=$1
    local component
    local remaining=$relative_path

    case "$relative_path" in
        ''|/*)
            seen_artifact_error "artifact path must be a non-empty project-relative path"
            return 1
            ;;
        *$'\n'*|*$'\r'*)
            seen_artifact_error "artifact path must not contain newlines"
            return 1
            ;;
    esac

    while :; do
        case "$remaining" in
            */*)
                component=${remaining%%/*}
                remaining=${remaining#*/}
                ;;
            *)
                component=$remaining
                remaining=""
                ;;
        esac
        case "$component" in
            ''|.) ;;
            ..)
                seen_artifact_error "artifact path must not contain '..'"
                return 1
                ;;
        esac
        [ -n "$remaining" ] || break
    done
}

seen_artifact_assert_no_symlink_components() {
    local repo_root=$1
    local relative_path=$2
    local current=$repo_root
    local component
    local remaining=$relative_path

    while :; do
        case "$remaining" in
            */*)
                component=${remaining%%/*}
                remaining=${remaining#*/}
                ;;
            *)
                component=$remaining
                remaining=""
                ;;
        esac
        case "$component" in
            ''|.)
                [ -n "$remaining" ] || break
                continue
                ;;
        esac
        current="$current/$component"
        if [ -L "$current" ]; then
            seen_artifact_error "artifact path traverses symbolic link: $current"
            return 1
        fi
        if [ -e "$current" ] && [ ! -d "$current" ]; then
            seen_artifact_error "artifact path component is not a directory: $current"
            return 1
        fi
        [ -n "$remaining" ] || break
    done
}

seen_artifact_root_init() {
    local requested_repo_root=${1:-}
    local repo_root
    local candidate
    local relative_path
    local canonical_root

    if [ -z "$requested_repo_root" ] || [ ! -d "$requested_repo_root" ]; then
        seen_artifact_error "repository root is missing or is not a directory"
        return 1
    fi
    repo_root=$(seen_artifact_canonical_dir "$requested_repo_root") || {
        seen_artifact_error "could not resolve repository root: $requested_repo_root"
        return 1
    }

    candidate=${SEEN_ARTIFACT_ROOT:-$repo_root/.seen/agent-tools}
    case "$candidate" in
        /*) ;;
        *) candidate="$repo_root/$candidate" ;;
    esac

    case "$candidate" in
        "$repo_root"/*) relative_path=${candidate#"$repo_root"/} ;;
        *)
            seen_artifact_error "SEEN_ARTIFACT_ROOT must stay inside the repository: $candidate"
            return 1
            ;;
    esac

    seen_artifact_assert_safe_relative_path "$relative_path" || return 1
    seen_artifact_assert_no_symlink_components "$repo_root" "$relative_path" || return 1

    if command -v git >/dev/null 2>&1 &&
        git -C "$repo_root" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
        if ! git -C "$repo_root" check-ignore -q -- "$relative_path"; then
            seen_artifact_error "artifact root is not ignored by Git: $candidate"
            return 1
        fi
    else
        case "$relative_path" in
            .seen|.seen/*) ;;
            *)
                seen_artifact_error "without Git, artifact root must be under .seen/: $candidate"
                return 1
                ;;
        esac
    fi

    mkdir -p -- "$candidate" || {
        seen_artifact_error "could not create artifact root: $candidate"
        return 1
    }
    seen_artifact_assert_no_symlink_components "$repo_root" "$relative_path" || return 1
    canonical_root=$(seen_artifact_canonical_dir "$candidate") || {
        seen_artifact_error "could not resolve artifact root: $candidate"
        return 1
    }

    case "$canonical_root" in
        "$repo_root"/*) ;;
        *)
            seen_artifact_error "resolved artifact root escaped the repository: $canonical_root"
            return 1
            ;;
    esac
    if [ "$canonical_root" = "$repo_root" ]; then
        seen_artifact_error "repository root itself cannot be used for disposable artifacts"
        return 1
    fi

    SEEN_PROJECT_ROOT=$repo_root
    SEEN_ARTIFACT_ROOT=$canonical_root
    export SEEN_PROJECT_ROOT SEEN_ARTIFACT_ROOT
}

seen_artifact_scope_init() {
    local scope=${1:-}
    local scope_root

    if [ -z "${SEEN_ARTIFACT_ROOT:-}" ] || [ ! -d "$SEEN_ARTIFACT_ROOT" ]; then
        seen_artifact_error "call seen_artifact_root_init before creating a scope"
        return 1
    fi
    case "$scope" in
        ''|*[!A-Za-z0-9._-]*)
            seen_artifact_error "artifact scope must use only letters, digits, '.', '_', and '-'"
            return 1
            ;;
    esac

    scope_root="$SEEN_ARTIFACT_ROOT/$scope"
    if [ -L "$scope_root" ]; then
        seen_artifact_error "artifact scope is a symbolic link: $scope_root"
        return 1
    fi
    if [ -e "$scope_root" ] && [ ! -d "$scope_root" ]; then
        seen_artifact_error "artifact scope is not a directory: $scope_root"
        return 1
    fi
    mkdir -p -- "$scope_root" || {
        seen_artifact_error "could not create artifact scope: $scope_root"
        return 1
    }
    if [ -L "$scope_root" ]; then
        seen_artifact_error "artifact scope became a symbolic link: $scope_root"
        return 1
    fi
    printf '%s\n' "$scope_root"
}

seen_artifact_mktemp_dir() {
    local scope_root=${1:-}
    local prefix=${2:-work}

    if [ -z "$scope_root" ] || [ ! -d "$scope_root" ] || [ -L "$scope_root" ]; then
        seen_artifact_error "temporary artifact scope is invalid: $scope_root"
        return 1
    fi
    case "$scope_root" in
        "$SEEN_ARTIFACT_ROOT"/*) ;;
        *)
            seen_artifact_error "temporary artifact scope is outside SEEN_ARTIFACT_ROOT"
            return 1
            ;;
    esac
    case "$prefix" in
        ''|*[!A-Za-z0-9._-]*)
            seen_artifact_error "temporary artifact prefix contains unsafe characters"
            return 1
            ;;
    esac

    mktemp -d "$scope_root/$prefix.XXXXXX"
}
