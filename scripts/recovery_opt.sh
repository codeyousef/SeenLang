#!/bin/bash
# Recovery script: fix up .ll files and produce .o files.
# Runs WITHOUT set -e so per-module failures can be reported explicitly.
#
# Works in a private temp directory to avoid interference from concurrent
# compilations that share /tmp/seen_module_* paths.
#
# Recovery only succeeds when EVERY copied .ll file produces a matching .o file.
# On success, prints "RECOVERY_DIR=<path>" as the LAST line of output.
# The caller should link .o files from that directory instead of /tmp.
#
# Usage: recovery_opt.sh <opt_wrapper_dir> <script_dir> [source_ll_dir] [--skip-fixups]
#   opt_wrapper_dir: directory containing the opt wrapper script
#   script_dir:      directory containing fix_ir.py and other helpers
#   source_ll_dir:   directory containing seen_module_*.ll (default: $SEEN_RECOVERY_LL_DIR or /tmp)

OPT_WRAPPER_DIR="$1"
SCRIPT_DIR="$2"
SKIP_FIXUPS=0
SOURCE_LL_DIR="${SEEN_RECOVERY_LL_DIR:-/tmp}"
if [ "${3:-}" = "--skip-fixups" ]; then
    SKIP_FIXUPS=1
elif [ -n "${3:-}" ]; then
    SOURCE_LL_DIR="$3"
fi
if [ "${4:-}" = "--skip-fixups" ]; then
    SKIP_FIXUPS=1
fi

if [ -z "$OPT_WRAPPER_DIR" ] || [ -z "$SCRIPT_DIR" ]; then
    echo "  ERROR: recovery_opt.sh requires <opt_wrapper_dir> <script_dir>"
    exit 1
fi
if [ ! -d "$SOURCE_LL_DIR" ]; then
    echo "  ERROR: recovery source directory not found: $SOURCE_LL_DIR"
    exit 1
fi

if [ "$SKIP_FIXUPS" = "0" ] && [ ! -x "$OPT_WRAPPER_DIR/opt" ]; then
    echo "  ERROR: opt wrapper not found at $OPT_WRAPPER_DIR/opt"
    exit 1
fi

REAL_OPT=$(command -v opt)
PROCESSED=0
FAILED=0
FAILED_MODULES=""

run_with_opt_limit() {
    if [ "${SEEN_LOW_MEMORY:-0}" = "1" ] && [ -n "${SEEN_OPT_VMEM_KB:-}" ]; then
        (
            ulimit -v "$SEEN_OPT_VMEM_KB" 2>/dev/null || true
            "$@"
        )
    else
        "$@"
    fi
}

# Copy .ll files to a private directory so concurrent compilations
# (which also write to /tmp/seen_module_*) can't interfere.
WORK_DIR=$(mktemp -d /tmp/seen_recovery.XXXXXX)
LL_COUNT=0
for f in "$SOURCE_LL_DIR"/seen_module_*.ll; do
    [ -f "$f" ] || continue
    [[ "$f" == *.opt.ll ]] && continue
    cp "$f" "$WORK_DIR/" 2>/dev/null && LL_COUNT=$((LL_COUNT+1))
done
echo "  Recovery: $LL_COUNT .ll files copied from $SOURCE_LL_DIR to $WORK_DIR"

if [ "$LL_COUNT" -eq 0 ]; then
    echo "  ERROR: no .ll files to process"
    rm -rf "$WORK_DIR"
    exit 1
fi

for llfile in "$WORK_DIR"/seen_module_*.ll; do
    [ -f "$llfile" ] || { FAILED=$((FAILED+1)); continue; }
    modname=$(basename "$llfile" .ll)
    optfile="$WORK_DIR/${modname}.opt.ll"
    objfile="$WORK_DIR/${modname}.o"

    # Run opt: use wrapper (fixups + opt) or real opt directly (--skip-fixups)
    if [ "$SKIP_FIXUPS" = "1" ]; then
        OPT_CMD="$REAL_OPT"
    else
        OPT_CMD="$OPT_WRAPPER_DIR/opt"
    fi
    if ! run_with_opt_limit "$OPT_CMD" \
        -passes='function(sroa,instcombine<no-verify-fixpoint>,simplifycfg),default<O1>' \
        -inline-threshold=250 -S "$llfile" -o "$optfile" 2>/dev/null; then
        echo "  ERROR: opt failed for $modname"
        FAILED=$((FAILED+1))
        FAILED_MODULES="$FAILED_MODULES $modname"
        continue
    fi

    # Generate ThinLTO bitcode
    if [ ! -f "$optfile" ]; then
        echo "  ERROR: opt did not emit $optfile"
        FAILED=$((FAILED+1))
        FAILED_MODULES="$FAILED_MODULES $modname"
        continue
    fi
    if ! run_with_opt_limit "$REAL_OPT" --thinlto-bc "$optfile" -o "$objfile" 2>/dev/null; then
        echo "  ERROR: thinlto-bc failed for $modname"
        FAILED=$((FAILED+1))
        FAILED_MODULES="$FAILED_MODULES $modname"
        continue
    fi

    if [ -f "$objfile" ]; then
        PROCESSED=$((PROCESSED+1))
    else
        echo "  ERROR: object file missing for $modname"
        FAILED=$((FAILED+1))
        FAILED_MODULES="$FAILED_MODULES $modname"
    fi
done

echo "  Recovery: processed=$PROCESSED failed=$FAILED"
if [ -n "$FAILED_MODULES" ]; then
    echo "  Failed modules:$FAILED_MODULES"
fi

# Count .o files in work directory
OBJ_COUNT=0
for f in "$WORK_DIR"/seen_module_*.o; do
    [ -f "$f" ] && OBJ_COUNT=$((OBJ_COUNT+1))
done
echo "  Total .o files: $OBJ_COUNT"

# Output work directory path for the caller to use for linking.
# The caller is responsible for cleaning up this directory.
if [ "$FAILED" -eq 0 ] && [ "$OBJ_COUNT" -eq "$LL_COUNT" ]; then
    echo "RECOVERY_DIR=$WORK_DIR"
    exit 0
else
    echo "  ERROR: recovery requires a complete object set ($OBJ_COUNT/$LL_COUNT produced)"
    rm -rf "$WORK_DIR"
    exit 1
fi
