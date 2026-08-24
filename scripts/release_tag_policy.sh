#!/usr/bin/env bash
# Read-only release-tag policy shared by CI attestation and the uploader.

seen_release_tag_policy_error() {
    printf 'release-tag: core.004b.invalid: %s\n' "$*" >&2
    return 1
}

seen_release_remote_ref() {
    local refs="$1"
    local wanted_ref="$2"

    awk -v wanted="$wanted_ref" '
        $2 == wanted {
            count += 1
            value = $1
        }
        END {
            if (count != 1) {
                exit 1
            }
            print value
        }
    ' <<<"$refs"
}

seen_release_verify_published_tag() {
    local root="$1"
    local tag="$2"
    local expected_commit="$3"
    local expected_repository="$4"
    local tag_ref="refs/tags/$tag"
    local local_tag_object local_tag_type local_tag_commit origin_url remote_refs
    local remote_main remote_tag_object remote_tag_commit object_id

    [[ "$tag" =~ ^v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-[0-9A-Za-z]+([.-][0-9A-Za-z]+)*)?$ ]] ||
        seen_release_tag_policy_error "release tag is not a supported version tag: $tag" || return 1
    [[ "$expected_commit" =~ ^[0-9a-f]{40}$ ]] ||
        seen_release_tag_policy_error "expected release commit is invalid" || return 1
    [[ "$expected_repository" =~ ^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$ ]] ||
        seen_release_tag_policy_error "expected release repository is invalid" || return 1

    origin_url=$(git -C "$root" remote get-url origin) ||
        seen_release_tag_policy_error "origin URL cannot be resolved" || return 1
    case "$origin_url" in
        "https://github.com/$expected_repository"|\
        "https://github.com/$expected_repository.git"|\
        "git@github.com:$expected_repository"|\
        "git@github.com:$expected_repository.git"|\
        "ssh://git@github.com/$expected_repository"|\
        "ssh://git@github.com/$expected_repository.git") ;;
        *)
            seen_release_tag_policy_error \
                "origin does not identify expected repository $expected_repository"
            return 1
            ;;
    esac

    local_tag_object=$(git -C "$root" rev-parse --verify "$tag_ref") ||
        seen_release_tag_policy_error "local tag $tag is missing" || return 1
    local_tag_type=$(git -C "$root" cat-file -t "$local_tag_object") ||
        seen_release_tag_policy_error "local tag object cannot be inspected" || return 1
    [ "$local_tag_type" = "tag" ] ||
        seen_release_tag_policy_error "local tag $tag is not annotated" || return 1
    local_tag_commit=$(git -C "$root" rev-parse --verify "$tag_ref^{commit}") ||
        seen_release_tag_policy_error "local tag $tag cannot be peeled to a commit" || return 1
    [ "$local_tag_commit" = "$expected_commit" ] ||
        seen_release_tag_policy_error \
            "local tag $tag peels to $local_tag_commit instead of $expected_commit" || return 1

    remote_refs=$(git -C "$root" ls-remote origin \
        refs/heads/main "$tag_ref" "$tag_ref^{}") ||
        seen_release_tag_policy_error "remote main/tag identity could not be read" || return 1
    remote_main=$(seen_release_remote_ref "$remote_refs" refs/heads/main) ||
        seen_release_tag_policy_error "remote main is missing or ambiguous" || return 1
    remote_tag_object=$(seen_release_remote_ref "$remote_refs" "$tag_ref") ||
        seen_release_tag_policy_error "remote tag $tag is missing or ambiguous" || return 1
    remote_tag_commit=$(seen_release_remote_ref "$remote_refs" "$tag_ref^{}") ||
        seen_release_tag_policy_error \
            "remote tag $tag is not an unambiguous annotated tag" || return 1

    for object_id in "$remote_main" "$remote_tag_object" "$remote_tag_commit"; do
        [[ "$object_id" =~ ^[0-9a-f]{40}$ ]] ||
            seen_release_tag_policy_error "remote returned an invalid object identifier" || return 1
    done
    [ "$remote_tag_object" = "$local_tag_object" ] ||
        seen_release_tag_policy_error \
            "remote tag object $remote_tag_object differs from local tag object $local_tag_object" || return 1
    [ "$remote_tag_commit" = "$expected_commit" ] ||
        seen_release_tag_policy_error \
            "remote tag $tag peels to $remote_tag_commit instead of $expected_commit" || return 1
    [ "$remote_main" = "$expected_commit" ] ||
        seen_release_tag_policy_error \
            "remote main is $remote_main instead of release commit $expected_commit" || return 1
}
