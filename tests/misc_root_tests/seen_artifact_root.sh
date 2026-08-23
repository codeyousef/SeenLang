#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -P)"
LIBRARY="$ROOT_DIR/scripts/artifact_root.sh"
WRAPPER="$ROOT_DIR/scripts/run_with_project_artifacts.sh"
PREBUILD_GATES="$ROOT_DIR/scripts/seen_prebuild_gates.sh"
STAGE1_ACCEPTANCE="$ROOT_DIR/scripts/seen_stage1_acceptance.sh"
SAFE_REBUILD="$ROOT_DIR/scripts/safe_rebuild.sh"
TEST_PARENT="$ROOT_DIR/.seen/agent-tools/tests"
FIXTURE_ROOT="$TEST_PARENT/artifact-root"

fail() {
    printf 'FAIL: %s\n' "$*" >&2
    exit 1
}

safe_fixture_cleanup() {
    local resolved_parent
    local resolved_fixture_parent

    [ -n "$ROOT_DIR" ] && [ -n "$TEST_PARENT" ] && [ -n "$FIXTURE_ROOT" ] || return 1
    case "$TEST_PARENT" in
        "$ROOT_DIR"/.seen/agent-tools/tests) ;;
        *) return 1 ;;
    esac
    [ ! -L "$TEST_PARENT" ] || return 1
    if [ -d "$TEST_PARENT" ]; then
        resolved_parent=$(cd -P -- "$TEST_PARENT" && pwd -P) || return 1
        resolved_fixture_parent=$(dirname -- "$FIXTURE_ROOT")
        [ "$resolved_parent" = "$resolved_fixture_parent" ] || return 1
    fi
    rm -rf -- "$FIXTURE_ROOT"
}

mkdir -p -- "$TEST_PARENT"
safe_fixture_cleanup
mkdir -p -- "$FIXTURE_ROOT"
trap safe_fixture_cleanup EXIT

default_root=$(
    unset SEEN_ARTIFACT_ROOT
    # shellcheck source=scripts/artifact_root.sh
    source "$LIBRARY"
    seen_artifact_root_init "$ROOT_DIR"
    printf '%s\n' "$SEEN_ARTIFACT_ROOT"
)
[ "$default_root" = "$ROOT_DIR/.seen/agent-tools" ] ||
    fail "default artifact root was '$default_root'"

relative_root=$(
    export SEEN_ARTIFACT_ROOT=".seen/agent-tools/tests/artifact-root/relative"
    # shellcheck source=scripts/artifact_root.sh
    source "$LIBRARY"
    seen_artifact_root_init "$ROOT_DIR"
    printf '%s\n' "$SEEN_ARTIFACT_ROOT"
)
[ "$relative_root" = "$FIXTURE_ROOT/relative" ] ||
    fail "relative override was not anchored to the repository"

space_root=$(
    export SEEN_ARTIFACT_ROOT=".seen/agent-tools/tests/artifact-root/path with spaces"
    # shellcheck source=scripts/artifact_root.sh
    source "$LIBRARY"
    seen_artifact_root_init "$ROOT_DIR"
    printf '%s\n' "$SEEN_ARTIFACT_ROOT"
)
[ "$space_root" = "$FIXTURE_ROOT/path with spaces" ] ||
    fail "artifact resolver did not preserve spaces in an ignored path"

if (
    export SEEN_ARTIFACT_ROOT="../seen-artifacts-outside-repository"
    # shellcheck source=scripts/artifact_root.sh
    source "$LIBRARY"
    seen_artifact_root_init "$ROOT_DIR"
) 2>/dev/null; then
    fail "outside-repository override was accepted"
fi

if (
    export SEEN_ARTIFACT_ROOT="tracked-artifacts"
    # shellcheck source=scripts/artifact_root.sh
    source "$LIBRARY"
    seen_artifact_root_init "$ROOT_DIR"
) 2>/dev/null; then
    fail "non-ignored override was accepted"
fi
[ ! -e "$ROOT_DIR/tracked-artifacts" ] ||
    fail "non-ignored override was created before validation"

mkdir -p -- "$FIXTURE_ROOT/real"
ln -s -- "$FIXTURE_ROOT/real" "$FIXTURE_ROOT/link"
if (
    export SEEN_ARTIFACT_ROOT=".seen/agent-tools/tests/artifact-root/link/child"
    # shellcheck source=scripts/artifact_root.sh
    source "$LIBRARY"
    seen_artifact_root_init "$ROOT_DIR"
) 2>/dev/null; then
    fail "symbolic-link component was accepted"
fi

scope_root=$(
    export SEEN_ARTIFACT_ROOT="$FIXTURE_ROOT/scopes"
    # shellcheck source=scripts/artifact_root.sh
    source "$LIBRARY"
    seen_artifact_root_init "$ROOT_DIR"
    seen_artifact_scope_init safe-rebuild
)
[ "$scope_root" = "$FIXTURE_ROOT/scopes/safe-rebuild" ] ||
    fail "scope root was not created below the artifact root"

temporary_dir=$(
    export SEEN_ARTIFACT_ROOT="$FIXTURE_ROOT/temporary"
    # shellcheck source=scripts/artifact_root.sh
    source "$LIBRARY"
    seen_artifact_root_init "$ROOT_DIR"
    scope=$(seen_artifact_scope_init tests)
    seen_artifact_mktemp_dir "$scope" fixture
)
case "$temporary_dir" in
    "$FIXTURE_ROOT"/temporary/tests/fixture.*) ;;
    *) fail "temporary directory escaped its artifact scope: $temporary_dir" ;;
esac
[ -d "$temporary_dir" ] || fail "temporary directory was not created"

for cache_guard in seen_artifact_assert_safe_relative_path \
    seen_artifact_assert_no_symlink_components seen_artifact_canonical_dir; do

    grep -Fq "$cache_guard" "$STAGE1_ACCEPTANCE" ||
        fail "Stage-1 cache validation omitted $cache_guard"
done

if [ "$(uname -s)" = "Linux" ]; then
    forged_project_root="$FIXTURE_ROOT/forged-rebuild"
    forged_rebuild_root="$forged_project_root/safe-rebuild/run.forged"
    mkdir -p -- "$forged_rebuild_root"
    if SEEN_ARTIFACT_ROOT="$forged_rebuild_root" \
        PROJECT_ARTIFACT_ROOT="$forged_project_root" \
        SEEN_ARTIFACT_NAMESPACE_ACTIVE=1 \
        SEEN_REBUILD_WORK_ROOT="$forged_rebuild_root" \
        "$PREBUILD_GATES" --artifact-preflight \
        >"$FIXTURE_ROOT/forged-rebuild.out" \
        2>"$FIXTURE_ROOT/forged-rebuild.err"; then

        fail "prebuild gates trusted a forged rebuild namespace marker"
    fi
    grep -Fq 'prebuild artifact namespace validation failed' \
        "$FIXTURE_ROOT/forged-rebuild.err" ||
        fail "forged rebuild marker did not reach namespace identity validation"

    forged_preflight_root="$FIXTURE_ROOT/forged-preflight"
    forged_preflight_run="$forged_preflight_root/prebuild-gates/run.forged"
    mkdir -p -- "$forged_preflight_run"
    if SEEN_ARTIFACT_ROOT="$forged_preflight_root" \
        SEEN_PREFLIGHT_NAMESPACE_ACTIVE=1 \
        SEEN_PREFLIGHT_WORK_ROOT="$forged_preflight_run" \
        "$PREBUILD_GATES" --artifact-preflight \
        >"$FIXTURE_ROOT/forged-preflight.out" \
        2>"$FIXTURE_ROOT/forged-preflight.err"; then

        fail "prebuild gates trusted a forged standalone namespace marker"
    fi
    grep -Fq 'prebuild artifact namespace validation failed' \
        "$FIXTURE_ROOT/forged-preflight.err" ||
        fail "forged standalone marker did not reach namespace identity validation"
fi

if [ "$(uname -s)" = "Linux" ] && command -v bwrap >/dev/null 2>&1; then
    wrapper_root="$FIXTURE_ROOT/wrapper root"
    marker="seen-wrapper-host-probe-$$"
    [ ! -e "/tmp/$marker" ] || fail "wrapper probe path already exists"
    wrapper_output=$(
        SEEN_ARTIFACT_ROOT="$wrapper_root" \
            "$WRAPPER" wrapper-success -- \
            bash -c '
                [ "$1" = "value with spaces" ] || exit 10
                [ "$2" = "*" ] || exit 11
                [ -z "$3" ] || exit 12
                [ "$SEEN_PROJECT_ROOT" = "$4" ] || exit 13
                [ "$TMPDIR" = "$SEEN_ARTIFACT_ROOT/tool-tmp" ] || exit 14
                tmp_identity=$(stat -c "%d:%i" /tmp) || exit 15
                root_identity=$(stat -c "%d:%i" "$SEEN_ARTIFACT_ROOT") || exit 16
                [ "$tmp_identity" = "$root_identity" ] || exit 17
                touch "/tmp/$5" || exit 18
                printf "wrapper-ok\n"
            ' bash "value with spaces" '*' '' "$ROOT_DIR" "$marker"
    )
    [ "$wrapper_output" = "wrapper-ok" ] ||
        fail "project-artifact wrapper did not preserve command arguments"
    [ ! -e "/tmp/$marker" ] ||
        fail "project-artifact wrapper wrote its probe into host /tmp"
    if find "$wrapper_root/wrapper-success" -mindepth 1 -maxdepth 1 \
        -type d -name 'run.*' -print -quit | grep -q .; then
        fail "successful wrapper command left a run directory behind"
    fi

    stale_auxiliary_root="$wrapper_root/outer-hard-scope"
    stale_auxiliary_config="$stale_auxiliary_root/auxiliary-limits/ripgrep.conf"
    mkdir -p -- "${stale_auxiliary_config%/*}"
    printf '%s\n' '--threads=1' > "$stale_auxiliary_config"
    rebound_output=$(
        SEEN_ARTIFACT_ROOT="$wrapper_root" \
            SEEN_HARD_MEMORY_SCOPE_ACTIVE=1 \
            RIPGREP_CONFIG_PATH="$stale_auxiliary_config" \
            "$WRAPPER" wrapper-hard-scope -- \
            bash -c '
                expected="$SEEN_ARTIFACT_ROOT/auxiliary-limits/ripgrep.conf"
                [ "$RIPGREP_CONFIG_PATH" = "$expected" ] || exit 20
                [ -f "$expected" ] && [ ! -L "$expected" ] || exit 21
                [ "$(cat "$expected")" = "--threads=1" ] || exit 22
                [ "$GOMAXPROCS" = "1" ] || exit 23
                printf "hard-scope-rebound\n"
            '
    )
    [ "$rebound_output" = "hard-scope-rebound" ] ||
        fail "nested project-artifact scope did not rebind serial auxiliary limits"
    if find "$wrapper_root/wrapper-hard-scope" -mindepth 1 -maxdepth 1 \
        -type d -name 'run.*' -print -quit | grep -q .; then
        fail "successful hard-scope wrapper command left a run directory behind"
    fi

    if SEEN_ARTIFACT_ROOT="$wrapper_root" \
        "$WRAPPER" wrapper-clean -- bash -c 'exit 17'; then
        fail "project-artifact wrapper lost the command failure status"
    fi
    if find "$wrapper_root/wrapper-clean" -mindepth 1 -maxdepth 1 \
        -type d -name 'run.*' -print -quit | grep -q .; then
        fail "failed wrapper command was retained without --keep-on-failure"
    fi

    if SEEN_ARTIFACT_ROOT="$wrapper_root" \
        "$WRAPPER" wrapper-keep --keep-on-failure -- \
        bash -c 'touch "$SEEN_ARTIFACT_ROOT/preserved"; exit 9' \
        2>"$FIXTURE_ROOT/wrapper-keep.err"; then
        fail "kept wrapper command lost the command failure status"
    fi
    kept_run=$(find "$wrapper_root/wrapper-keep" -mindepth 1 -maxdepth 1 \
        -type d -name 'run.*' -print -quit)
    [ -n "$kept_run" ] && [ -f "$kept_run/preserved" ] ||
        fail "--keep-on-failure did not retain the exact failed run"

    if SEEN_ARTIFACT_ROOT="$wrapper_root" \
        "$WRAPPER" 'bad/scope' -- true 2>/dev/null; then
        fail "project-artifact wrapper accepted an unsafe scope"
    fi

    rebuild_root="$FIXTURE_ROOT/rebuild"
    preflight_output=$(
        SEEN_ARTIFACT_ROOT="$rebuild_root" \
            "$ROOT_DIR/scripts/safe_rebuild.sh" --artifact-preflight
    )
    case "$preflight_output" in
        *"Project artifact root: $rebuild_root"*) ;;
        *) fail "safe rebuild did not honor the artifact-root override" ;;
    esac
    case "$preflight_output" in
        *"Frozen-bootstrap temporary mapping: project-local"*) ;;
        *) fail "safe rebuild did not confirm its project-local temporary mapping" ;;
    esac

    # Model the second safe_rebuild invocation made by the aggregate hard
    # scope. At that point SEEN_ARTIFACT_ROOT is intentionally the per-run
    # directory, while PROJECT_ARTIFACT_ROOT must still identify the stable
    # base used to resolve the safe-rebuild scope.
    hard_reexec_base="$FIXTURE_ROOT/hard reexec base"
    hard_reexec_work="$hard_reexec_base/safe-rebuild/run.hard-reexec"
    hard_reexec_tmp="$hard_reexec_work/tool-tmp"
    hard_reexec_rg_config="$hard_reexec_work/auxiliary-limits/ripgrep.conf"
    mkdir -p -- "$hard_reexec_tmp" "${hard_reexec_rg_config%/*}"
    printf '%s\n' '--threads=1' > "$hard_reexec_rg_config"
    hard_reexec_output=$(
        bwrap --die-with-parent --bind / / --dev-bind /dev /dev \
            --proc /proc --ro-bind /sys /sys \
            --bind "$hard_reexec_work" /tmp -- \
            env \
                SEEN_ARTIFACT_ROOT="$hard_reexec_work" \
                PROJECT_ARTIFACT_ROOT="$hard_reexec_base" \
                SEEN_ARTIFACT_NAMESPACE_ACTIVE=1 \
                SEEN_REBUILD_WORK_ROOT="$hard_reexec_work" \
                SEEN_REBUILD_AGGREGATE_SCOPE_ACTIVE=1 \
                SEEN_MEMORY_GUARD_IN_SCOPE=1 \
                TMPDIR="$hard_reexec_tmp" \
                RIPGREP_CONFIG_PATH="$hard_reexec_rg_config" \
                RAYON_NUM_THREADS=1 OMP_NUM_THREADS=1 \
                OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 \
                NUMEXPR_NUM_THREADS=1 VECLIB_MAXIMUM_THREADS=1 \
                BLIS_NUM_THREADS=1 GOMAXPROCS=1 RUST_TEST_THREADS=1 \
                CARGO_BUILD_JOBS=1 RPM_BUILD_NCPUS=1 \
                "$SAFE_REBUILD" --artifact-preflight
    )
    case "$hard_reexec_output" in
        *"Project artifact root: $hard_reexec_base"*) ;;
        *) fail "hard-scope re-exec lost the stable project artifact base" ;;
    esac
    case "$hard_reexec_output" in
        *"Rebuild artifact directory: $hard_reexec_work"*) ;;
        *) fail "hard-scope re-exec nested or replaced its per-run directory" ;;
    esac
    [ ! -e "$hard_reexec_work/safe-rebuild" ] ||
        fail "hard-scope re-exec created a nested safe-rebuild scope"

    if bwrap --die-with-parent --bind / / --dev-bind /dev /dev \
        --proc /proc --ro-bind /sys /sys \
        --bind "$hard_reexec_work" /tmp -- \
        env -u PROJECT_ARTIFACT_ROOT \
            SEEN_ARTIFACT_ROOT="$hard_reexec_work" \
            SEEN_ARTIFACT_NAMESPACE_ACTIVE=1 \
            SEEN_REBUILD_WORK_ROOT="$hard_reexec_work" \
            TMPDIR="$hard_reexec_tmp" \
            "$SAFE_REBUILD" --artifact-preflight \
            >"$FIXTURE_ROOT/missing-project-root.out" \
            2>"$FIXTURE_ROOT/missing-project-root.err"; then

        fail "namespace re-exec accepted a missing stable project artifact base"
    fi
    grep -Fq 'inherited project artifact root is missing or escaped' \
        "$FIXTURE_ROOT/missing-project-root.err" ||
        fail "missing stable artifact base did not fail at re-exec validation"

    external_cwd="$FIXTURE_ROOT/external-cwd"
    mkdir -p -- "$external_cwd/.seen_cache"
    printf 'outside-cache-sentinel\n' > "$external_cwd/.seen_cache/preserved"
    printf 'outside-stage2-sentinel\n' > "$external_cwd/stage2_head"
    printf 'outside-stage3-sentinel\n' > "$external_cwd/stage3_head"
    external_preflight_output=$(
        cd -- "$external_cwd"
        SEEN_ARTIFACT_ROOT="$rebuild_root" \
            "$ROOT_DIR/scripts/safe_rebuild.sh" --artifact-preflight
    )
    case "$external_preflight_output" in
        *"Checkout working directory: $ROOT_DIR"*) ;;
        *) fail "absolute safe-rebuild invocation did not anchor itself to the checkout" ;;
    esac
    [ "$(cat "$external_cwd/.seen_cache/preserved")" = \
        "outside-cache-sentinel" ] ||
        fail "external working-directory cache sentinel was changed"
    [ "$(cat "$external_cwd/stage2_head")" = "outside-stage2-sentinel" ] ||
        fail "external working-directory stage2 sentinel was changed"
    [ "$(cat "$external_cwd/stage3_head")" = "outside-stage3-sentinel" ] ||
        fail "external working-directory stage3 sentinel was changed"

    prebuild_root="$FIXTURE_ROOT/prebuild"
    prebuild_output=$(
        SEEN_ARTIFACT_ROOT="$prebuild_root" \
            "$PREBUILD_GATES" --artifact-preflight
    )
    case "$prebuild_output" in
        *"Project artifact root: $prebuild_root"*) ;;
        *) fail "prebuild gates did not honor the artifact-root override" ;;
    esac
    case "$prebuild_output" in
        *"Legacy-fixture temporary mapping: project-local"*) ;;
        *) fail "prebuild gates did not confirm their project-local temporary mapping" ;;
    esac
fi

printf 'artifact-root tests passed\n'
