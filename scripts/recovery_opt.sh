#!/bin/bash
# Recovery script: fix up .ll files and produce .o files.
# Runs WITHOUT set -e so non-fatal fixup failures don't abort the caller.
#
# Works in a private temp directory to avoid interference from concurrent
# compilations that share /tmp/seen_module_* paths.
#
# On success, prints "RECOVERY_DIR=<path>" as the LAST line of output.
# The caller should link .o files from that directory instead of /tmp.
#
# Usage: recovery_opt.sh <opt_wrapper_dir> <script_dir>
#   opt_wrapper_dir: directory containing the opt wrapper script
#   script_dir:      directory containing fix_ir.py and other helpers

OPT_WRAPPER_DIR="$1"
SCRIPT_DIR="$2"
SKIP_FIXUPS=0
if [ "$3" = "--skip-fixups" ]; then
    SKIP_FIXUPS=1
fi

if [ -z "$OPT_WRAPPER_DIR" ] || [ -z "$SCRIPT_DIR" ]; then
    echo "  ERROR: recovery_opt.sh requires <opt_wrapper_dir> <script_dir>"
    exit 1
fi

if [ "$SKIP_FIXUPS" = "0" ] && [ ! -x "$OPT_WRAPPER_DIR/opt" ]; then
    echo "  ERROR: opt wrapper not found at $OPT_WRAPPER_DIR/opt"
    exit 1
fi

REAL_OPT=$(command -v opt)
PROCESSED=0
FAILED=0

# Copy .ll files to a private directory so concurrent compilations
# (which also write to /tmp/seen_module_*) can't interfere.
WORK_DIR=$(mktemp -d /tmp/seen_recovery.XXXXXX)
LL_COUNT=0
for f in /tmp/seen_module_*.ll; do
    [ -f "$f" ] || continue
    [[ "$f" == *.opt.ll ]] && continue
    cp "$f" "$WORK_DIR/" 2>/dev/null && LL_COUNT=$((LL_COUNT+1))
done
echo "  Recovery: $LL_COUNT .ll files copied to $WORK_DIR"

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
    if ! "$OPT_CMD" \
        -passes='function(sroa,instcombine<no-verify-fixpoint>,simplifycfg),default<O1>' \
        -inline-threshold=250 -S "$llfile" -o "$optfile" 2>/dev/null; then
        # Opt wrapper failed — use raw fixed .ll as fallback
        # (the wrapper already applied fixups to $llfile in place)
        if [ -f "$llfile" ]; then
            cp "$llfile" "$optfile" 2>/dev/null || { FAILED=$((FAILED+1)); continue; }
        else
            FAILED=$((FAILED+1))
            continue
        fi
    fi

    # Generate ThinLTO bitcode
    [ -f "$optfile" ] || { FAILED=$((FAILED+1)); continue; }
    "$REAL_OPT" --thinlto-bc "$optfile" -o "$objfile" 2>/dev/null || true

    if [ -f "$objfile" ]; then
        PROCESSED=$((PROCESSED+1))
    else
        FAILED=$((FAILED+1))
    fi
done

echo "  Recovery: processed=$PROCESSED failed=$FAILED"

# Count .o files in work directory
OBJ_COUNT=0
for f in "$WORK_DIR"/seen_module_*.o; do
    [ -f "$f" ] && OBJ_COUNT=$((OBJ_COUNT+1))
done
echo "  Total .o files: $OBJ_COUNT"

# Output work directory path for the caller to use for linking.
# The caller is responsible for cleaning up this directory.
if [ "$OBJ_COUNT" -gt 30 ]; then
    echo "RECOVERY_DIR=$WORK_DIR"
    exit 0
else
    rm -rf "$WORK_DIR"
    exit 1
fi
