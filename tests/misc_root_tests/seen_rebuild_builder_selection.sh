#!/usr/bin/env bash
# Mock-only regression for fail-closed quick/verify builder selection.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd -P)"
SELECTOR="$REPO_ROOT/scripts/rebuild_builder_selection.sh"
SAFE_REBUILD="$REPO_ROOT/scripts/safe_rebuild.sh"
ARTIFACT_ROOT_SCRIPT="$REPO_ROOT/scripts/artifact_root.sh"

fail() {
    echo "builder-selection regression: $*" >&2
    exit 1
}

[ -f "$SELECTOR" ] || fail "missing selector: $SELECTOR"
[ -f "$ARTIFACT_ROOT_SCRIPT" ] || fail "missing artifact-root helper"
# shellcheck source=scripts/artifact_root.sh
source "$ARTIFACT_ROOT_SCRIPT"
seen_artifact_root_init "$REPO_ROOT"
TEST_SCOPE=$(seen_artifact_scope_init rebuild-builder-selection-test)
TEST_ROOT=$(seen_artifact_mktemp_dir "$TEST_SCOPE" run)

cleanup() {
    local status=$?
    case "$TEST_ROOT" in
        "$TEST_SCOPE"/run.*)
            if [ -d "$TEST_ROOT" ] && [ ! -L "$TEST_ROOT" ] &&
                [ "$(dirname -- "$TEST_ROOT")" = "$TEST_SCOPE" ]; then

                rm -rf -- "$TEST_ROOT"
            fi
            ;;
        *)
            echo "builder-selection regression: refusing unsafe cleanup: $TEST_ROOT" >&2
            status=1
            ;;
    esac
    return "$status"
}
trap cleanup EXIT

make_compiler() {
    local path=$1
    local label=$2
    local version=$3
    local log=$4
    mkdir -p -- "$(dirname -- "$path")"
    {
        printf '%s\n' '#!/usr/bin/env bash'
        printf '%s\n' 'set -euo pipefail'
        printf 'printf "%%s\\n" %q >> %q\n' "$label" "$log"
        printf '%s\n' '[ "$#" -eq 1 ] && [ "$1" = "--version" ] || {'
        printf '%s\n' '    echo "mock compiler received a non-version command" >&2'
        printf '%s\n' '    exit 99'
        printf '%s\n' '}'
        printf 'printf "Seen %%s\\nLanguage: Seen\\n" %q\n' "$version"
    } > "$path"
    chmod 755 "$path"
}

make_sidecar() {
    local path=$1
    local version=$2
    local log=$3
    mkdir -p -- "$(dirname -- "$path")"
    {
        printf '%s\n' '#!/usr/bin/env bash'
        printf '%s\n' 'set -euo pipefail'
        printf 'printf "sidecar:%%s\\n" "$*" >> %q\n' "$log"
        printf 'expected=%q\n' "$version"
        printf '%s\n' '[ "$#" -eq 4 ] && [ "$1" = "--expect-version" ] &&'
        printf '%s\n' '    [ "$2" = "$expected" ] && [ "$3" = "version" ] &&'
        printf '%s\n' '    [ "$4" = "--machine" ] || exit 78'
        printf '%s\n' 'printf "protocol=SEENPKG1\nversion=%s\n" "$expected"'
    } > "$path"
    chmod 755 "$path"
}

new_scenario() {
    local name=$1
    local root="$TEST_ROOT/$name"
    mkdir -p -- "$root"
    printf '%s\n' "$root"
}

select_builder() {
    local root=$1
    shift
    bash "$SELECTOR" --repo-root "$root" --checkout-version 0.19.2 \
        --source-sidecar "$root/source-sidecar" "$@"
}

# The first present preferred compiler wins, and lower-priority candidates are
# not even probed.
scenario=$(new_scenario preferred)
log="$scenario/invocations.log"
make_compiler "$scenario/compiler_seen/target/seen" current 0.19.2 "$log"
make_compiler "$scenario/target/release/seen" release 0.19.2 "$log"
make_compiler "$scenario/stage3_recovery_head" recovery 0.19.2 "$log"
make_sidecar "$scenario/source-sidecar" 0.19.2 "$log"
selected=$(select_builder "$scenario")
[ "$selected" = "$scenario/compiler_seen/target/seen" ] ||
    fail "checkout compiler was not preferred"
grep -Fxq current "$log" || fail "checkout compiler was not version-probed"
if grep -Eq '^(release|recovery)$' "$log"; then
    fail "selector probed a lower-priority compiler"
fi

# Absence—not failure or mismatch—permits the next preferred path.
scenario=$(new_scenario release-fallback)
log="$scenario/invocations.log"
make_compiler "$scenario/target/release/seen" release 0.19.2 "$log"
make_compiler "$scenario/stage3_recovery_head" recovery 0.19.2 "$log"
make_sidecar "$scenario/source-sidecar" 0.19.2 "$log"
selected=$(select_builder "$scenario")
[ "$selected" = "$scenario/target/release/seen" ] ||
    fail "release compiler was not selected when checkout compiler was absent"
if grep -Fxq recovery "$log"; then
    fail "selector probed recovery after selecting release"
fi

scenario=$(new_scenario recovery-fallback)
log="$scenario/invocations.log"
make_compiler "$scenario/stage3_recovery_head" recovery 0.19.2 "$log"
make_sidecar "$scenario/source-sidecar" 0.19.2 "$log"
selected=$(select_builder "$scenario")
[ "$selected" = "$scenario/stage3_recovery_head" ] ||
    fail "recovery compiler was not selected when preferred files were absent"

# An explicit builder overrides every implicit path.
scenario=$(new_scenario explicit)
log="$scenario/invocations.log"
make_compiler "$scenario/custom/seen" explicit 0.19.2 "$log"
make_compiler "$scenario/compiler_seen/target/seen" current 0.19.2 "$log"
make_sidecar "$scenario/source-sidecar" 0.19.2 "$log"
selected=$(select_builder "$scenario" --explicit-builder custom/seen)
[ "$selected" = "$scenario/custom/seen" ] || fail "explicit builder did not win"
if grep -Fxq current "$log"; then
    fail "selector probed an implicit candidate after an explicit selection"
fi

# A present preferred candidate with the wrong version is terminal. It must not
# fall through to a matching release/recovery binary.
scenario=$(new_scenario version-mismatch)
log="$scenario/invocations.log"
make_compiler "$scenario/compiler_seen/target/seen" current 0.10.0 "$log"
make_compiler "$scenario/target/release/seen" release 0.19.2 "$log"
make_compiler "$scenario/stage3_recovery_head" recovery 0.19.2 "$log"
make_sidecar "$scenario/source-sidecar" 0.19.2 "$log"
if select_builder "$scenario" >"$scenario/stdout" 2>"$scenario/stderr"; then
    fail "version-mismatched preferred compiler was accepted"
fi
grep -Fq 'selected builder version mismatch' "$scenario/stderr" ||
    fail "version mismatch diagnostic was missing"
if grep -Eq '^(release|recovery)$' "$log"; then
    fail "version mismatch fell through to another candidate"
fi

# A source-sidecar mismatch is terminal before any compile-like command exists.
scenario=$(new_scenario sidecar-mismatch)
log="$scenario/invocations.log"
make_compiler "$scenario/compiler_seen/target/seen" current 0.19.2 "$log"
make_sidecar "$scenario/source-sidecar" 0.10.0 "$log"
if select_builder "$scenario" >"$scenario/stdout" 2>"$scenario/stderr"; then
    fail "mismatched source sidecar was accepted"
fi
grep -Fq 'source package sidecar handshake failed' "$scenario/stderr" ||
    fail "sidecar mismatch diagnostic was missing"

# Static integration assertions: quick/verify consumes one selector result,
# injects only the source helper, and returns instead of iterating after failure.
grep -Fq 'candidate=$(select_tier_builder)' "$SAFE_REBUILD" ||
    fail "safe_rebuild does not consume one selected builder"
grep -Fq 'SEEN_PACKAGE_CLIENT="$SOURCE_PACKAGE_CLIENT"' "$SAFE_REBUILD" ||
    fail "source helper is not explicitly scoped"
grep -Fq 'legacy fallback is forbidden' "$SAFE_REBUILD" ||
    fail "terminal compile/smoke failure diagnostic is missing"
if grep -Fq 'tier_builder_candidates' "$SAFE_REBUILD"; then
    fail "legacy multi-candidate iterator remains reachable"
fi
tier_body="$TEST_ROOT/run-tiered-rebuild.body"
awk '
    /^run_tiered_rebuild\(\)/ { inside = 1 }
    inside { print }
    inside && /^}$/ { exit }
' "$SAFE_REBUILD" > "$tier_body"
grep -Fq 'return "$compile_status"' "$tier_body" ||
    fail "selected-builder compile failure is not terminal"
if grep -Eq '^[[:space:]]*(while|continue)([[:space:]]|$)' "$tier_body"; then
    fail "run_tiered_rebuild still iterates after selecting a builder"
fi
for forbidden in compiler_seen/target/seen-dev stage2_head stage3_head \
    bootstrap/stage1_frozen_v3 bootstrap/stage1_frozen; do
    if grep -Fq "$forbidden" "$SELECTOR"; then
        fail "quick/verify selector still contains legacy candidate $forbidden"
    fi
done

echo "builder-selection regression: PASS"
