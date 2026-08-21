#!/usr/bin/env bash
# Run a strict four-phase PGO cycle inside an already verified build scope.

set -euo pipefail

ROOT_DIR="$(cd -P -- "${BASH_SOURCE[0]%/*}/.." && pwd -P)"
ARTIFACT_HELPER="$ROOT_DIR/scripts/artifact_root.sh"
HARD_SCOPE="$ROOT_DIR/scripts/run_in_hard_memory_scope.sh"

if [ "$#" -lt 2 ]; then
    echo "Usage: SEEN_PGO_COMPILER=/absolute/seen $0 <source.seen> <output> [training args...]" >&2
    exit 2
fi
SOURCE=$1
OUTPUT=$2
shift 2
TRAINING_ARGS=("$@")
case "$SOURCE" in /*) ;; *) SOURCE="$PWD/$SOURCE" ;; esac
case "$OUTPUT" in /*) ;; *) OUTPUT="$PWD/$OUTPUT" ;; esac

[ "${SEEN_MEMORY_GUARD_IN_SCOPE:-0}" = "1" ] || {
    echo "RESOURCE STOP: PGO builds require a verified aggregate memory scope" >&2
    exit 126
}
"$HARD_SCOPE" --label "PGO build read-back" --verify-only -- >/dev/null || {
    echo "RESOURCE STOP: PGO build scope read-back failed" >&2
    exit 126
}
# shellcheck source=scripts/artifact_root.sh
source "$ARTIFACT_HELPER"
seen_artifact_root_init "$ROOT_DIR" || exit 126
scope=$(seen_artifact_scope_init pgo-build) || exit 126
work=$(seen_artifact_mktemp_dir "$scope" run) || exit 126

COMPILER=${SEEN_PGO_COMPILER:-}
case "$COMPILER" in /*) ;; *)
    echo "RESOURCE STOP: SEEN_PGO_COMPILER must be an absolute path" >&2
    exit 126 ;;
esac
[ -x "$COMPILER" ] && [ ! -L "$COMPILER" ] || {
    echo "RESOURCE STOP: unsafe PGO compiler" >&2
    exit 126
}

instrumented="$work/instrumented"
profraw="$work/default.profraw"
profdata="$work/default.profdata"
profile_relative=${profdata#"$ROOT_DIR"/}
case "$profile_relative" in .seen/agent-tools/*) ;; *)
    echo "RESOURCE STOP: PGO profile escaped project artifacts" >&2
    exit 126 ;;
esac

"$COMPILER" compile "$SOURCE" "$instrumented" --release --lto thin \
    --pgo-generate --no-cache
LLVM_PROFILE_FILE="$profraw" "$instrumented" "${TRAINING_ARGS[@]}"
llvm-profdata merge -sparse "$profraw" -o "$profdata"
(
    cd "$ROOT_DIR"
    "$COMPILER" compile "$SOURCE" "$OUTPUT" --release --lto thin \
        --pgo-use "$profile_relative" --no-cache
)
echo "PGO build complete: $OUTPUT"
