#!/bin/bash
# Safe rebuild: Only updates compiler if bootstrap verifies
#
# This script builds a new compiler from the frozen bootstrap and verifies
# that it achieves bootstrap (stage2 == stage3 or stage3 == stage4).
# Only if verification passes does it update the production compiler.

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
DIM='\033[2m'
NC='\033[0m' # No Color

STAGE2="/tmp/stage2_safe_rebuild"
STAGE3="/tmp/stage3_safe_rebuild"
COMPILER_SOURCE="compiler_seen/src/main_compiler.seen"

# --- Progress monitoring helpers ---

# Format seconds as HH:MM:SS
format_time() {
    local secs=$1
    printf "%02d:%02d:%02d" $((secs/3600)) $(((secs%3600)/60)) $((secs%60))
}

# Format bytes as human-readable (pure bash, no bc dependency)
format_bytes() {
    local bytes=$1
    if [ "$bytes" -ge 1073741824 ]; then
        local gb=$((bytes / 1073741824))
        local remainder=$(( (bytes % 1073741824) * 10 / 1073741824 ))
        printf "%d.%dGB" "$gb" "$remainder"
    elif [ "$bytes" -ge 1048576 ]; then
        printf "%dMB" "$((bytes / 1048576))"
    elif [ "$bytes" -ge 1024 ]; then
        printf "%dKB" "$((bytes / 1024))"
    else
        printf "%dB" "$bytes"
    fi
}

# Monitor a compilation step in background, printing live progress.
# Usage: monitor_compilation <PID> <stage_label>
# Watches /tmp/seen_module_*.ll files to track per-module progress.
monitor_compilation() {
    local compile_pid=$1
    local label=$2
    local start_time=$SECONDS
    local total_modules=0
    local last_status=""

    while kill -0 "$compile_pid" 2>/dev/null; do
        local elapsed=$((SECONDS - start_time))
        local elapsed_fmt=$(format_time $elapsed)

        # Count .ll files (modules with IR generated)
        local ll_count=$(ls /tmp/seen_module_*.ll 2>/dev/null | wc -l)

        # Count .o files (modules fully compiled)
        local obj_count=$(ls /tmp/seen_module_*.o 2>/dev/null | wc -l)

        # Check for module 5 (the big one) -- if its .ll exists
        local mod5_status=""
        if [ -f /tmp/seen_module_5.ll ]; then
            local mod5_size=$(stat -c%s /tmp/seen_module_5.ll 2>/dev/null || stat -f%z /tmp/seen_module_5.ll 2>/dev/null || echo 0)
            mod5_status="mod5.ll=$(format_bytes $mod5_size)"
        else
            # Check if a fork child is working on it (large RSS process)
            local fork_pids=$(pgrep -P "$compile_pid" 2>/dev/null || true)
            if [ -n "$fork_pids" ]; then
                for fpid in $fork_pids; do
                    local frss=$(ps -o rss= -p "$fpid" 2>/dev/null | tr -d ' ')
                    if [ -n "$frss" ] && [ "$frss" -gt 500000 ]; then
                        mod5_status="mod5: IR gen ($(format_bytes $((frss * 1024))) RSS)"
                        break
                    fi
                done
            fi
            if [ -z "$mod5_status" ]; then
                mod5_status="mod5: waiting"
            fi
        fi

        # Check if we're in opt/link phase (parallel opt script exists and running)
        local phase="IR gen"
        if pgrep -f "seen_parallel_opt.sh" > /dev/null 2>&1; then
            phase="opt"
        fi
        if pgrep -f "clang.*flto.*seen_module" > /dev/null 2>&1 || pgrep -f "ld.lld.*seen_module" > /dev/null 2>&1; then
            phase="link"
        fi

        # Build status line
        local status="${CYAN}[$label]${NC} ${elapsed_fmt}  ${BOLD}${ll_count} .ll${NC} | ${BOLD}${obj_count} .o${NC}  phase:${phase}  ${DIM}${mod5_status}${NC}"

        # Only reprint if status changed (avoid flicker)
        if [ "$status" != "$last_status" ]; then
            printf "\r\033[K${status}"
            last_status="$status"
        fi

        sleep 5
    done

    # Final status
    local elapsed=$((SECONDS - start_time))
    local elapsed_fmt=$(format_time $elapsed)
    local ll_count=$(ls /tmp/seen_module_*.ll 2>/dev/null | wc -l)
    local obj_count=$(ls /tmp/seen_module_*.o 2>/dev/null | wc -l)
    printf "\r\033[K${CYAN}[$label]${NC} ${GREEN}done${NC} in ${elapsed_fmt}  ${ll_count} .ll | ${obj_count} .o\n"
}

# Run a compilation step with live progress monitoring.
# Usage: run_with_progress <label> <command...>
# Returns the exit code of the compilation command.
run_with_progress() {
    local label=$1
    shift
    local logfile=$1
    shift

    # Start compilation in background, logging to file
    "$@" > "$logfile" 2>&1 &
    local compile_pid=$!

    # Start progress monitor
    monitor_compilation "$compile_pid" "$label" &
    local monitor_pid=$!

    # Wait for compilation to finish
    local exit_code=0
    wait "$compile_pid" || exit_code=$?

    # Stop monitor
    kill "$monitor_pid" 2>/dev/null || true
    wait "$monitor_pid" 2>/dev/null || true

    return $exit_code
}

# Snapshot watcher: periodically copies .ll files to a safe directory so they
# survive the frozen compiler's exit cleanup (which deletes ALL /tmp/seen_module_*).
# Usage: start_ll_snapshot_watcher <compiler_pid> <snapshot_dir>
start_ll_snapshot_watcher() {
    local watch_pid=$1
    local snapshot_dir=$2
    mkdir -p "$snapshot_dir"
    while kill -0 "$watch_pid" 2>/dev/null; do
        for f in /tmp/seen_module_*.ll; do
            [ -f "$f" ] || continue
            [[ "$f" == *.opt.ll ]] && continue
            local bn=$(basename "$f")
            if [ ! -f "$snapshot_dir/$bn" ] || [ "$f" -nt "$snapshot_dir/$bn" ]; then
                cp "$f" "$snapshot_dir/$bn" 2>/dev/null || true
            fi
        done
        sleep 3
    done
    # Final sweep after compiler exits
    for f in /tmp/seen_module_*.ll; do
        [ -f "$f" ] || continue
        [[ "$f" == *.opt.ll ]] && continue
        cp "$f" "$snapshot_dir/$(basename "$f")" 2>/dev/null || true
    done
}

# Kill orphaned fork children from the frozen compiler.
# Uses SIGKILL (not SIGTERM) because SIGTERM triggers cleanup handlers that delete files.
kill_frozen_orphans() {
    pkill -9 -f "$(basename "$FROZEN")" 2>/dev/null || true
    pkill -9 -f "seen_parallel_opt" 2>/dev/null || true
    pkill -9 -f "opt.*seen_module" 2>/dev/null || true
    pkill -9 -f "clang.*seen_module" 2>/dev/null || true
    pkill -9 -f "ld.lld.*seen_module" 2>/dev/null || true
    sleep 2
}

extract_expected_module_count() {
    local log_file=$1
    local count=""
    if [ -f "$log_file" ]; then
        count=$(grep -Eo 'Found [0-9]+ modules' "$log_file" 2>/dev/null | head -1 | awk '{print $2}')
    fi
    if [ -z "$count" ]; then
        echo 0
    else
        echo "$count"
    fi
}

count_module_objects() {
    local dir=$1
    local count=0
    for f in "$dir"/seen_module_*.o; do
        [ -f "$f" ] || continue
        count=$((count+1))
    done
    echo "$count"
}

count_plain_module_lls() {
    local dir=$1
    local count=0
    for f in "$dir"/seen_module_*.ll; do
        [ -f "$f" ] || continue
        [[ "$f" == *.opt.ll ]] && continue
        count=$((count+1))
    done
    echo "$count"
}

count_module_opt_lls() {
    local dir=$1
    local count=0
    for f in "$dir"/seen_module_*.opt.ll; do
        [ -f "$f" ] || continue
        count=$((count+1))
    done
    echo "$count"
}

list_modules_missing_objects() {
    local dir=$1
    local missing=""
    for llfile in "$dir"/seen_module_*.ll; do
        [ -f "$llfile" ] || continue
        [[ "$llfile" == *.opt.ll ]] && continue
        local modname=$(basename "$llfile" .ll)
        if [ ! -f "$dir/${modname}.o" ]; then
            missing="$missing ${modname}"
        fi
    done
    echo "$missing"
}

find_problem_empty_modules() {
    local dir=$1
    local empty=""
    for llfile in "$dir"/seen_module_*.ll; do
        [ -f "$llfile" ] || continue
        [[ "$llfile" == *.opt.ll ]] && continue
        local defines=$(grep -c '^define' "$llfile" 2>/dev/null | tail -1)
        defines=${defines:-0}
        if [ "$defines" -eq 0 ] 2>/dev/null; then
            local strings=$(grep -c '@\.str' "$llfile" 2>/dev/null | tail -1)
            strings=${strings:-0}
            if [ "$strings" -gt 0 ] 2>/dev/null; then
                empty="$empty $(basename "$llfile" .ll)"
            fi
        fi
    done
    echo "$empty"
}

echo "=== Safe Rebuild Script ==="
echo ""

# Detect host platform
HOST_OS=$(uname -s)
HOST_ARCH=$(uname -m)

if [ "$HOST_OS" = "Darwin" ]; then
    # macOS
    if [ "$HOST_ARCH" = "arm64" ]; then
        FROZEN="bootstrap/stage1_frozen_macos_arm64"
        HASH_FILE="bootstrap/stage1_frozen_macos_arm64.sha256"
        echo "Detected macOS ARM64 (Apple Silicon), using stage1_frozen_macos_arm64"
    else
        FROZEN="bootstrap/stage1_frozen_macos_x86_64"
        HASH_FILE="bootstrap/stage1_frozen_macos_x86_64.sha256"
        echo "Detected macOS x86_64, using stage1_frozen_macos_x86_64"
    fi
else
    # Linux: Auto-detect CPU ISA level and select appropriate bootstrap
    detect_isa_level() {
        if grep -q 'avx512f' /proc/cpuinfo 2>/dev/null; then
            echo "v4"
        else
            echo "v3"
        fi
    }

    ISA_LEVEL=$(detect_isa_level)
    if [ "$ISA_LEVEL" = "v4" ]; then
        FROZEN="bootstrap/stage1_frozen"
        HASH_FILE="bootstrap/stage1_frozen.sha256"
        echo "Detected x86-64-v4 (AVX-512) CPU, using stage1_frozen"
    else
        FROZEN="bootstrap/stage1_frozen_v3"
        HASH_FILE="bootstrap/stage1_frozen_v3.sha256"
        echo "Detected x86-64-v3 CPU, using stage1_frozen_v3"
    fi
fi

# Check frozen compiler exists
if [ ! -f "$FROZEN" ]; then
    echo -e "${RED}ERROR: Frozen compiler not found at $FROZEN${NC}"
    echo "Run this script from the repository root."
    if [ "$HOST_OS" = "Darwin" ]; then
        echo "On macOS, run scripts/bootstrap_macos.sh first to create the macOS bootstrap."
    fi
    exit 1
fi

# Verify frozen compiler hash (cross-platform)
echo "Verifying frozen compiler integrity..."
verify_hash() {
    if command -v sha256sum &>/dev/null; then
        sha256sum -c "$1" > /dev/null 2>&1
    elif command -v shasum &>/dev/null; then
        shasum -a 256 -c "$1" > /dev/null 2>&1
    else
        echo -e "${YELLOW}WARNING: No sha256sum or shasum found, skipping hash verification${NC}"
        return 0
    fi
}
if verify_hash "$HASH_FILE"; then
    echo -e "${GREEN}Frozen compiler verified.${NC}"
else
    echo -e "${RED}ERROR: Frozen compiler hash verification failed!${NC}"
    echo "The bootstrap compiler may be corrupted."
    exit 1
fi

# Kill any leftover compilation processes that might write to /tmp/seen_module_*
# and interfere with this build (race condition causes duplicate symbols)
pkill -9 -f "seen compile" 2>/dev/null || true
pkill -9 -f "seen build" 2>/dev/null || true
sleep 1

# Clean up any previous test files and cache
rm -f "$STAGE2" "$STAGE3"
rm -rf .seen_cache/ /tmp/seen_ir_cache/

# --- Opt wrapper setup (platform-specific) ---

if [ "$HOST_OS" = "Darwin" ]; then
    # macOS: use comprehensive ABI mismatch fixer (macos_opt_wrapper.py)
    OPT_WRAPPER_DIR=$(mktemp -d /tmp/seen_opt_wrapper.XXXXXX)
    if [ -d "/opt/homebrew/opt/llvm/bin" ]; then
        LLVM_BIN="/opt/homebrew/opt/llvm/bin"
    elif [ -d "/usr/local/opt/llvm/bin" ]; then
        LLVM_BIN="/usr/local/opt/llvm/bin"
    else
        LLVM_BIN=""
    fi
    PYTHON3_PATH=""
    for p in /opt/homebrew/bin/python3 /usr/local/bin/python3 /usr/bin/python3; do
        if [ -x "$p" ]; then PYTHON3_PATH="$p"; break; fi
    done
    [ -z "$PYTHON3_PATH" ] && PYTHON3_PATH=$(which python3 2>/dev/null || echo "python3")
    cp bootstrap/macos_opt_wrapper.py "$OPT_WRAPPER_DIR/macos_opt_wrapper_impl.py"
    cat > "$OPT_WRAPPER_DIR/opt" << WRAPPER_EOF
#!/bin/sh
export PATH="$LLVM_BIN:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:\$PATH"
exec "$PYTHON3_PATH" "$OPT_WRAPPER_DIR/macos_opt_wrapper_impl.py" "\$@"
WRAPPER_EOF
    chmod +x "$OPT_WRAPPER_DIR/opt"
    export PATH="$OPT_WRAPPER_DIR:$LLVM_BIN:$PATH"
    echo "macOS: opt wrapper enabled (python3=$PYTHON3_PATH)"
else
    # Linux: LLVM 21+ rejects duplicate 'declare' statements with different attributes.
    REAL_OPT=$(command -v opt)
    OPT_WRAPPER_DIR="/tmp/seen_opt_override"
    mkdir -p "$OPT_WRAPPER_DIR"
cat > "$OPT_WRAPPER_DIR/opt" << WRAPPER_EOF
#!/bin/bash
# Wrapper: deduplicate declare statements in .ll files before invoking real opt.
# The frozen compiler emits extern __-prefixed functions twice (once from ir_declarations
# with nounwind, once from extern handler with possibly different types). We do a two-pass
# approach: first collect all declared function names, then on second pass keep only the
# LAST declaration for each function (which matches call site types).
ARGS=("\$@")
for arg in "\${ARGS[@]}"; do
    if [[ "\$arg" == *.ll && "\$arg" != *.opt.ll && -f "\$arg" ]]; then
        awk '
        # Pass 1: count declarations per function name
        NR == FNR {
            if (/^declare /) {
                if (match(\$0, /@([A-Za-z0-9_.]+)/, m)) {
                    count[m[1]]++
                    seen_count[m[1]] = 0
                }
            }
            next
        }
        # Pass 2: for functions with duplicates, skip all but the last
        /^declare / {
            if (match(\$0, /@([A-Za-z0-9_.]+)/, m)) {
                fname = m[1]
                seen_count[fname]++
                if (count[fname] > 1 && seen_count[fname] < count[fname]) next
            }
        }
        { print }
        ' "\$arg" "\$arg" > "\${arg}.dedup" && mv "\${arg}.dedup" "\$arg"

        # Fix byteAt codegen bug: pre-refactoring compiler emits string concat
        # (seen_int_to_string + seen_char_to_str + seen_str_concat_ss) instead of
        # integer add for byteAt() + int expressions. Replace with add i64.
        python3 -c "
import re, sys
with open(sys.argv[1]) as f:
    content = f.read()
pattern = re.compile(
    r'  (%\d+) = call %SeenString @seen_int_to_string\(i64 (%\d+)\)\n'
    r'  (%\d+) = call %SeenString @seen_char_to_str\(i64 (%\d+)\)\n'
    r'  (%\d+) = call %SeenString @seen_str_concat_ss\(%SeenString \1, %SeenString \3\)'
)
def fix(m):
    return f'  {m.group(1)} = add i64 0, 0\n  {m.group(3)} = add i64 0, 0\n  {m.group(5)} = add i64 {m.group(2)}, {m.group(4)}'
new_content, count = pattern.subn(fix, content)
if count > 0:
    with open(sys.argv[1], 'w') as f:
        f.write(new_content)
    print(f'  byteAt fix: patched {count} site(s) in {sys.argv[1]}', file=sys.stderr)
" "\$arg" 2>&1 || true

        # Apply comprehensive IR fixups (declare dedup, type mismatches, SSA, etc.)
        python3 "$SCRIPT_DIR/fix_ir.py" "\$arg" 2>&1 || true

        # Fix bare 0 as type in declare params (e.g. (i64, 0) → (i64, i64))
        # This is a belt-and-suspenders fix in case fix_ir.py doesn't catch it
        sed -i 's/^\(declare.*(\)\(.*\), 0)/\1\2, i64)/g' "\$arg" 2>/dev/null || true

        # Fix corrupt declares from string constants leaking into declare generator
        # Stage1 parses @funcName(...) from string constants, producing broken declares
        # with \00 or other garbage. Remove these — the correct declare is already present.
        sed -i '/^declare.*\\\\00/d' "\$arg" 2>/dev/null || true

        # NOTE: Phantom declare removal disabled — it was too aggressive and removed
        # declares for cross-module functions (emitIncludeStrImpl, etc.) that ARE called
        # from the module via ThinLTO. The awk dedup + fix_ir.py handle the critical cases.

        # IR Validation: run llvm-as structural check on the fixed .ll file
        if ! llvm-as "\$arg" -o /dev/null 2>/tmp/seen_verify_err.txt; then
            echo "IR VERIFY WARNING: \$arg (continuing)" >&2
            head -2 /tmp/seen_verify_err.txt >&2
            rm -f /tmp/seen_verify_err.txt
        fi
        rm -f /tmp/seen_verify_err.txt

        # NOTE: seen_ir_lint disabled — it naively counts commas to determine
        # argument count, causing false positives on LLVM inline struct literals
        # like %SeenString { i64 7, ptr @.str }. llvm-as above catches real errors.
    fi
done
exec "$REAL_OPT" "\$@"
WRAPPER_EOF
    chmod +x "$OPT_WRAPPER_DIR/opt"
fi  # end platform-specific opt wrapper

# Step 1: Build stage2 with frozen compiler (--fast)
# NOTE: PATH override ensures our dedup opt wrapper runs instead of system opt.
echo ""
echo "Step 1: Building stage2 with frozen compiler (--fast)..."
echo -e "${DIM}The frozen compiler generates IR for all 50+ modules.${NC}"
echo -e "${DIM}Module 5 (llvm_ir_gen.seen, 14K lines) typically takes 1-2 minutes.${NC}"
echo ""

# Clean stale .ll/.o from previous runs so counts are accurate
rm -f /tmp/seen_module_*.ll /tmp/seen_module_*.o /tmp/seen_module_*.opt.ll

if [ "$HOST_OS" = "Darwin" ]; then
    # macOS: PATH already set via export above; use --no-cache
    # The frozen compiler may fail at its internal link step (e.g., internal globals
    # eliminated by opt) but still produce a full .opt.ll set we can relink in step 1b.
    if run_with_progress "S1→S2" /tmp/safe_rebuild_stage2.log $FROZEN compile "$COMPILER_SOURCE" "$STAGE2" --fast --no-cache; then
        echo -e "${GREEN}Stage2 build succeeded.${NC}"
    else
        EXPECTED_STAGE2_MODULES=$(extract_expected_module_count /tmp/safe_rebuild_stage2.log)
        OPT_LL_COUNT=$(count_module_opt_lls /tmp)
        if [ "$EXPECTED_STAGE2_MODULES" -gt 0 ] && [ "$OPT_LL_COUNT" -eq "$EXPECTED_STAGE2_MODULES" ]; then
            echo -e "${YELLOW}Stage2 internal link failed, but the full $OPT_LL_COUNT/$EXPECTED_STAGE2_MODULES .opt.ll set is available for relink.${NC}"
        else
            echo -e "${RED}ERROR: Stage2 build failed!${NC}"
            if [ "$EXPECTED_STAGE2_MODULES" -gt 0 ]; then
                echo "Expected $EXPECTED_STAGE2_MODULES optimized modules, found $OPT_LL_COUNT."
            fi
            echo "Check /tmp/safe_rebuild_stage2.log for details."
            tail -30 /tmp/safe_rebuild_stage2.log
            exit 1
        fi
    fi
else
    # Linux S1→S2: Inline process management so we can run a snapshot watcher.
    # The frozen compiler deletes ALL /tmp/seen_module_* on exit (even on failure),
    # so we snapshot .ll files while it's running.
    SNAPSHOT_DIR="/tmp/seen_ll_snapshot_$$"
    rm -rf "$SNAPSHOT_DIR"

    # Start compiler in background
    env PATH="$OPT_WRAPPER_DIR:$PATH" $FROZEN compile "$COMPILER_SOURCE" "$STAGE2" --fast --no-cache > /tmp/safe_rebuild_stage2.log 2>&1 &
    COMPILE_PID=$!

    # Start progress monitor and snapshot watcher
    monitor_compilation "$COMPILE_PID" "S1→S2" &
    MONITOR_PID=$!
    start_ll_snapshot_watcher "$COMPILE_PID" "$SNAPSHOT_DIR" &
    WATCHER_PID=$!

    # Wait for compiler
    COMPILE_EXIT=0
    wait "$COMPILE_PID" || COMPILE_EXIT=$?

    # Stop monitor and watcher
    kill "$MONITOR_PID" 2>/dev/null || true
    wait "$MONITOR_PID" 2>/dev/null || true
    kill "$WATCHER_PID" 2>/dev/null || true
    wait "$WATCHER_PID" 2>/dev/null || true

    EXPECTED_STAGE2_MODULES=$(extract_expected_module_count /tmp/safe_rebuild_stage2.log)
    if [ "$EXPECTED_STAGE2_MODULES" -le 0 ]; then
        echo -e "${RED}ERROR: Could not determine expected module count for Stage2.${NC}"
        echo "Check /tmp/safe_rebuild_stage2.log for details."
        tail -30 /tmp/safe_rebuild_stage2.log
        rm -rf "$SNAPSHOT_DIR"
        exit 1
    fi

    if [ "$COMPILE_EXIT" -eq 0 ]; then
        STAGE2_OBJ_COUNT=$(count_module_objects /tmp)
        if [ "$STAGE2_OBJ_COUNT" -ne "$EXPECTED_STAGE2_MODULES" ]; then
            echo -e "${RED}ERROR: Stage2 reported success but produced only $STAGE2_OBJ_COUNT/$EXPECTED_STAGE2_MODULES module objects.${NC}"
            echo "Check /tmp/safe_rebuild_stage2.log for details."
            tail -30 /tmp/safe_rebuild_stage2.log
            rm -rf "$SNAPSHOT_DIR"
            exit 1
        fi
        echo -e "${GREEN}Stage2 build succeeded.${NC}"
        rm -rf "$SNAPSHOT_DIR"
    else
        # Kill orphaned fork children before recovery (SIGKILL to avoid cleanup handlers)
        echo -e "${YELLOW}Stage2 compilation failed (exit=$COMPILE_EXIT), killing orphans...${NC}"
        kill_frozen_orphans

        # Check how many .ll files we have: first from snapshot, fallback to live /tmp
        SNAP_COUNT=$(count_plain_module_lls "$SNAPSHOT_DIR")
        LIVE_COUNT=$(count_plain_module_lls /tmp)

        if [ "$SNAP_COUNT" -gt "$LIVE_COUNT" ]; then
            LL_COUNT=$SNAP_COUNT
            LL_SOURCE="snapshot"
            echo -e "${YELLOW}Snapshot has $SNAP_COUNT .ll files (live: $LIVE_COUNT). Restoring from snapshot...${NC}"
            # Clean /tmp of any partial files, then restore from snapshot
            rm -f /tmp/seen_module_*.ll /tmp/seen_module_*.o /tmp/seen_module_*.opt.ll
            cp "$SNAPSHOT_DIR"/seen_module_*.ll /tmp/ 2>/dev/null || true
        else
            LL_COUNT=$LIVE_COUNT
            LL_SOURCE="live"
            echo -e "${YELLOW}Using $LIVE_COUNT live .ll files from /tmp.${NC}"
        fi

        if [ "$LL_COUNT" -eq "$EXPECTED_STAGE2_MODULES" ]; then
            echo -e "${YELLOW}Recovering with the full $LL_COUNT/$EXPECTED_STAGE2_MODULES .ll set ($LL_SOURCE)...${NC}"

            # Clean stale .o and .opt.ll from the compiler's failed internal opt/link —
            # we must regenerate them from the raw .ll files via our own opt wrapper.
            rm -f /tmp/seen_module_*.o /tmp/seen_module_*.opt.ll

            # Run recovery in subprocess (immune to set -e).
            # Recovery works in a private temp dir to avoid interference from
            # concurrent compilations. It outputs RECOVERY_DIR=<path> on success.
            RECOVERY_EXIT=0
            RECOVERY_OUTPUT=$(bash "$SCRIPT_DIR/recovery_opt.sh" "$OPT_WRAPPER_DIR" "$SCRIPT_DIR" 2>&1) || RECOVERY_EXIT=$?
            echo "$RECOVERY_OUTPUT" | grep -v '^RECOVERY_DIR=' || true

            if [ "$RECOVERY_EXIT" -ne 0 ]; then
                echo -e "${RED}ERROR: Recovery failed.${NC}"
                rm -rf "$SNAPSHOT_DIR"
                exit 1
            fi

            RECOVERY_DIR=$(echo "$RECOVERY_OUTPUT" | grep '^RECOVERY_DIR=' | tail -1 | cut -d= -f2)
            if [ -z "$RECOVERY_DIR" ] || [ ! -d "$RECOVERY_DIR" ]; then
                echo -e "${RED}ERROR: Recovery failed — no output directory.${NC}"
                rm -rf "$SNAPSHOT_DIR"
                exit 1
            fi

            OBJ_COUNT=$(count_module_objects "$RECOVERY_DIR")
            if [ "$OBJ_COUNT" -ne "$EXPECTED_STAGE2_MODULES" ]; then
                MISSING_MODULES=$(list_modules_missing_objects "$RECOVERY_DIR")
                echo -e "${RED}ERROR: Recovery failed — only $OBJ_COUNT/$EXPECTED_STAGE2_MODULES .o files produced.${NC}"
                if [ -n "$MISSING_MODULES" ]; then
                    echo "Missing objects:$MISSING_MODULES"
                fi
                rm -rf "$SNAPSHOT_DIR" "$RECOVERY_DIR"
                exit 1
            fi
            echo "  Recovery: $OBJ_COUNT/$EXPECTED_STAGE2_MODULES .o files ready."

            # Check for empty modules that might cause link failures.
            # Skip modules that are legitimately empty (only declares/types, no string
            # constants — these are re-export shims with no real code).
            EMPTY_MODULES=$(find_problem_empty_modules "$RECOVERY_DIR")
            EMPTY_COUNT=$(echo "$EMPTY_MODULES" | wc -w)
            if [ "$EMPTY_COUNT" -gt 0 ]; then
                echo -e "${YELLOW}Empty modules ($EMPTY_COUNT with 0 function definitions):${EMPTY_MODULES}${NC}"

                # --- Retry loop: re-run frozen compiler to fill empty modules ---
                # OOM kills are non-deterministic, so retrying will likely produce
                # a different set of empty modules (or none at all).
                MAX_RETRIES=2
                RETRY=0
                while [ "$EMPTY_COUNT" -gt 0 ] && [ "$RETRY" -lt "$MAX_RETRIES" ]; do
                    RETRY=$((RETRY+1))
                    echo -e "${YELLOW}Retry $RETRY/$MAX_RETRIES: re-running frozen compiler to fill $EMPTY_COUNT empty module(s)...${NC}"

                    # Clean /tmp for retry run
                    rm -f /tmp/seen_module_*.ll /tmp/seen_module_*.o /tmp/seen_module_*.opt.ll

                    RETRY_SNAPSHOT="/tmp/seen_retry_snapshot_${$}_${RETRY}"
                    rm -rf "$RETRY_SNAPSHOT"

                    env PATH="$OPT_WRAPPER_DIR:$PATH" $FROZEN compile "$COMPILER_SOURCE" /dev/null --fast --no-cache > /tmp/retry_${RETRY}.log 2>&1 &
                    RETRY_PID=$!

                    start_ll_snapshot_watcher "$RETRY_PID" "$RETRY_SNAPSHOT" &
                    RETRY_WATCHER=$!
                    monitor_compilation "$RETRY_PID" "Retry$RETRY" &
                    RETRY_MONITOR=$!

                    # Wait with 10-min timeout
                    timeout 600 tail --pid=$RETRY_PID -f /dev/null 2>/dev/null || true
                    kill -9 $RETRY_PID 2>/dev/null || true; wait $RETRY_PID 2>/dev/null || true
                    kill $RETRY_WATCHER 2>/dev/null || true; wait $RETRY_WATCHER 2>/dev/null || true
                    kill $RETRY_MONITOR 2>/dev/null || true; wait $RETRY_MONITOR 2>/dev/null || true
                    kill_frozen_orphans

                    RETRY_COUNT=$(ls "$RETRY_SNAPSHOT"/seen_module_*.ll 2>/dev/null | wc -l)
                    echo ""
                    echo "  Retry $RETRY: captured $RETRY_COUNT .ll files"

                    # Replace modules where retry has MORE defines (catches both
                    # empty modules and partially-generated/truncated modules)
                    REPLACED=0
                    for retry_ll in "$RETRY_SNAPSHOT"/seen_module_*.ll; do
                        [ -f "$retry_ll" ] || continue
                        [[ "$retry_ll" == *.opt.ll ]] && continue
                        bn=$(basename "$retry_ll")
                        mod=$(basename "$retry_ll" .ll)
                        orig_ll="$RECOVERY_DIR/$bn"
                        [ -f "$orig_ll" ] || continue

                        orig_defines=$(grep -c '^define' "$orig_ll" 2>/dev/null | tail -1)
                        orig_defines=${orig_defines:-0}
                        retry_defines=$(grep -c '^define' "$retry_ll" 2>/dev/null | tail -1)
                        retry_defines=${retry_defines:-0}

                        if [ "$retry_defines" -gt "$orig_defines" ] 2>/dev/null; then
                            cp "$retry_ll" "$orig_ll"
                            rm -f "$RECOVERY_DIR/${mod}.opt.ll" "$RECOVERY_DIR/${mod}.o"
                            echo "    Replaced $bn: $orig_defines -> $retry_defines defines"
                            REPLACED=$((REPLACED+1))
                        fi
                    done
                    rm -rf "$RETRY_SNAPSHOT"
                    echo "  Retry $RETRY: replaced $REPLACED module(s)"

                    # Re-run opt wrapper + thinlto-bc on replaced modules (those missing .o)
                    if [ "$REPLACED" -gt 0 ]; then
                        REAL_OPT_BIN=$(command -v opt)
                        for llfile in "$RECOVERY_DIR"/seen_module_*.ll; do
                            [ -f "$llfile" ] || continue
                            [[ "$llfile" == *.opt.ll ]] && continue
                            mod=$(basename "$llfile" .ll)
                            objfile="$RECOVERY_DIR/${mod}.o"
                            [ -f "$objfile" ] && continue

                            optfile="$RECOVERY_DIR/${mod}.opt.ll"
                            echo "    Re-optimizing ${mod}..."
                            # Use opt wrapper to apply dedup, byteAt fix, fix_ir.py
                            if ! "$OPT_WRAPPER_DIR/opt" \
                                -passes='function(sroa,instcombine<no-verify-fixpoint>,simplifycfg),default<O1>' \
                                -inline-threshold=250 -S "$llfile" -o "$optfile" 2>/dev/null; then
                                cp "$llfile" "$optfile" 2>/dev/null || true
                            fi
                            "$REAL_OPT_BIN" --thinlto-bc "$optfile" -o "$objfile" 2>/dev/null || true
                        done
                    fi

                    # Re-count empty modules
                    EMPTY_MODULES=$(find_problem_empty_modules "$RECOVERY_DIR")
                    EMPTY_COUNT=$(echo "$EMPTY_MODULES" | wc -w)
                    if [ "$EMPTY_COUNT" -eq 0 ]; then
                        echo -e "${GREEN}  All modules now have function definitions!${NC}"
                    else
                        echo -e "${YELLOW}  Still $EMPTY_COUNT empty module(s):${EMPTY_MODULES}${NC}"
                    fi
                done

                # Recount .o files after retries
                if [ "$RETRY" -gt 0 ]; then
                    OBJ_COUNT=$(count_module_objects "$RECOVERY_DIR")
                    echo "  Post-retry: $OBJ_COUNT/$EXPECTED_STAGE2_MODULES .o files ready."
                fi

                # --- Pass 2: Two-pass .ll merge for satellite modules ---
                if [ "$EMPTY_COUNT" -gt 0 ]; then
                # The frozen compiler generates module 5 (llvm_ir_gen.seen) correctly
                # but outputs 0 defines for satellite codegen modules. The production
                # compiler generates satellite modules correctly but hangs on module 5.
                # Merge: pick the .ll with more defines from each compiler.
                PROD_COMPILER="compiler_seen/target/seen"
                if [ -x "$PROD_COMPILER" ]; then
                    echo -e "${YELLOW}Running Pass 2 (production compiler) to fill empty modules...${NC}"

                    # Clean /tmp for Pass 2
                    rm -f /tmp/seen_module_*.ll /tmp/seen_module_*.o /tmp/seen_module_*.opt.ll

                    # Run production compiler with timeout + snapshot watcher
                    PASS2_SNAPSHOT="/tmp/seen_pass2_snapshot_$$"
                    rm -rf "$PASS2_SNAPSHOT"

                    $PROD_COMPILER compile "$COMPILER_SOURCE" /dev/null --fast --no-cache > /tmp/pass2.log 2>&1 &
                    PASS2_PID=$!

                    start_ll_snapshot_watcher "$PASS2_PID" "$PASS2_SNAPSHOT" &
                    PASS2_WATCHER=$!
                    monitor_compilation "$PASS2_PID" "Pass2" &
                    PASS2_MONITOR=$!

                    # Wait with 10-min timeout (satellite modules complete within 5 min)
                    timeout 600 tail --pid=$PASS2_PID -f /dev/null 2>/dev/null || true
                    kill -9 $PASS2_PID 2>/dev/null; wait $PASS2_PID 2>/dev/null || true
                    kill $PASS2_WATCHER 2>/dev/null; wait $PASS2_WATCHER 2>/dev/null || true
                    kill $PASS2_MONITOR 2>/dev/null; wait $PASS2_MONITOR 2>/dev/null || true
                    # Kill any children of the Pass 2 compiler (fork children, opt, clang)
                    for child in $(pgrep -P $PASS2_PID 2>/dev/null); do
                        kill -9 "$child" 2>/dev/null || true
                    done
                    sleep 2

                    PASS2_COUNT=$(ls "$PASS2_SNAPSHOT"/seen_module_*.ll 2>/dev/null | wc -l)
                    echo ""
                    echo "  Pass 2: captured $PASS2_COUNT .ll files"

                    # Merge: for each module, pick the .ll with more defines
                    MERGED=0
                    for pass1_ll in "$RECOVERY_DIR"/seen_module_*.ll; do
                        [ -f "$pass1_ll" ] || continue
                        [[ "$pass1_ll" == *.opt.ll ]] && continue
                        bn=$(basename "$pass1_ll")
                        pass2_ll="$PASS2_SNAPSHOT/$bn"
                        [ -f "$pass2_ll" ] || continue

                        p1_defines=$(grep -c '^define' "$pass1_ll" 2>/dev/null | tail -1)
                        p1_defines=${p1_defines:-0}
                        p2_defines=$(grep -c '^define' "$pass2_ll" 2>/dev/null | tail -1)
                        p2_defines=${p2_defines:-0}

                        if [ "$p2_defines" -gt "$p1_defines" ] 2>/dev/null; then
                            cp "$pass2_ll" "$pass1_ll"
                            modname=$(basename "$pass1_ll" .ll)
                            rm -f "$RECOVERY_DIR/${modname}.opt.ll" "$RECOVERY_DIR/${modname}.o"
                            echo "    Merged $bn: $p1_defines -> $p2_defines defines (from production)"
                            MERGED=$((MERGED+1))
                        fi
                    done
                    # Also add Pass 2 .ll files not present in RECOVERY_DIR
                    for pass2_ll in "$PASS2_SNAPSHOT"/seen_module_*.ll; do
                        [ -f "$pass2_ll" ] || continue
                        [[ "$pass2_ll" == *.opt.ll ]] && continue
                        bn=$(basename "$pass2_ll")
                        if [ ! -f "$RECOVERY_DIR/$bn" ]; then
                            cp "$pass2_ll" "$RECOVERY_DIR/$bn"
                            echo "    Added $bn from Pass 2 (not in Pass 1)"
                            MERGED=$((MERGED+1))
                        fi
                    done
                    rm -rf "$PASS2_SNAPSHOT"
                    echo "  Merged $MERGED modules from Pass 2"

                    if [ "$MERGED" -gt 0 ]; then
                        # Re-run opt + thinlto-bc on merged modules (those missing .o files)
                        REAL_OPT_BIN=$(command -v opt)
                        for llfile in "$RECOVERY_DIR"/seen_module_*.ll; do
                            [ -f "$llfile" ] || continue
                            [[ "$llfile" == *.opt.ll ]] && continue
                            modname=$(basename "$llfile" .ll)
                            objfile="$RECOVERY_DIR/${modname}.o"
                            [ -f "$objfile" ] && continue

                            optfile="$RECOVERY_DIR/${modname}.opt.ll"
                            echo "    Re-optimizing $modname..."
                            if ! "$REAL_OPT_BIN" \
                                -passes='function(sroa,instcombine<no-verify-fixpoint>,simplifycfg),default<O1>' \
                                -inline-threshold=250 -S "$llfile" -o "$optfile" 2>/dev/null; then
                                cp "$llfile" "$optfile" 2>/dev/null || true
                            fi
                            "$REAL_OPT_BIN" --thinlto-bc "$optfile" -o "$objfile" 2>/dev/null || true
                        done

                        # Recount .o files after merge
                        OBJ_COUNT=$(count_module_objects "$RECOVERY_DIR")
                        echo "  Post-merge: $OBJ_COUNT/$EXPECTED_STAGE2_MODULES .o files ready."
                    fi
                else
                    echo -e "${YELLOW}No production compiler available for Pass 2 merge.${NC}"
                    echo -e "${YELLOW}Fix: revert module list changes, rebuild, update stage1_frozen, re-apply.${NC}"
                fi
                fi
            fi

            EMPTY_MODULES=$(find_problem_empty_modules "$RECOVERY_DIR")
            EMPTY_COUNT=$(echo "$EMPTY_MODULES" | wc -w)
            if [ "$EMPTY_COUNT" -gt 0 ]; then
                echo -e "${RED}ERROR: Recovery left $EMPTY_COUNT module(s) with missing function bodies:${EMPTY_MODULES}${NC}"
                rm -rf "$SNAPSHOT_DIR" "$RECOVERY_DIR"
                exit 1
            fi

            OBJ_COUNT=$(count_module_objects "$RECOVERY_DIR")
            if [ "$OBJ_COUNT" -ne "$EXPECTED_STAGE2_MODULES" ]; then
                MISSING_MODULES=$(list_modules_missing_objects "$RECOVERY_DIR")
                echo -e "${RED}ERROR: Recovery object set is incomplete ($OBJ_COUNT/$EXPECTED_STAGE2_MODULES).${NC}"
                if [ -n "$MISSING_MODULES" ]; then
                    echo "Missing objects:$MISSING_MODULES"
                fi
                rm -rf "$SNAPSHOT_DIR" "$RECOVERY_DIR"
                exit 1
            fi

            # Pre-compile runtime
            RT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)/seen_runtime"
            if [ ! -f "$RT_DIR/seen_runtime.o" ] || [ "$RT_DIR/seen_runtime.c" -nt "$RT_DIR/seen_runtime.o" ]; then
                echo "  Pre-compiling runtime..."
                clang -O3 -flto=thin -march=native -ffunction-sections -fdata-sections -pthread \
                    -c -I "$RT_DIR" "$RT_DIR/seen_runtime.c" -o "$RT_DIR/seen_runtime.o" 2>/dev/null || true
            fi
            if [ -f "$RT_DIR/seen_region.c" ]; then
                if [ ! -f "$RT_DIR/seen_region.o" ] || [ "$RT_DIR/seen_region.c" -nt "$RT_DIR/seen_region.o" ]; then
                    clang -O3 -flto=thin -march=native -ffunction-sections -fdata-sections \
                        -c -I "$RT_DIR" "$RT_DIR/seen_region.c" -o "$RT_DIR/seen_region.o" 2>/dev/null || true
                fi
            fi
            if [ -f "$RT_DIR/seen_gpu.c" ]; then
                if [ ! -f "$RT_DIR/seen_gpu.o" ] || [ "$RT_DIR/seen_gpu.c" -nt "$RT_DIR/seen_gpu.o" ]; then
                    clang -O3 -flto=thin -march=native -ffunction-sections -fdata-sections \
                        -c -I "$RT_DIR" "$RT_DIR/seen_gpu.c" -o "$RT_DIR/seen_gpu.o" 2>/dev/null || true
                fi
            fi

            # Link from recovery directory (not /tmp, which may be contaminated
            # by concurrent compilations)
            echo "  Linking $OBJ_COUNT modules..."
            LINK_OBJS=""
            for obj in "$RECOVERY_DIR"/seen_module_*.o; do
                LINK_OBJS="$LINK_OBJS $obj"
            done
            RT_OBJS="$RT_DIR/seen_runtime.o"
            [ -f "$RT_DIR/seen_region.o" ] && RT_OBJS="$RT_OBJS $RT_DIR/seen_region.o"
            [ -f "$RT_DIR/seen_gpu.o" ] && RT_OBJS="$RT_OBJS $RT_DIR/seen_gpu.o"

            LINK_LIBS="-lm -lpthread"
            [ -f "$RT_DIR/seen_gpu.o" ] && pkg-config --exists vulkan 2>/dev/null && LINK_LIBS="$LINK_LIBS -lvulkan"

            if clang -O1 -flto=thin -fuse-ld=lld \
                -Wl,--thinlto-cache-dir=/tmp/seen_thinlto_cache \
                -Wl,--allow-multiple-definition \
                -march=native -Wl,--gc-sections -Wno-unused-command-line-argument \
                $LINK_OBJS $RT_OBJS -o "$STAGE2" $LINK_LIBS 2>/tmp/safe_rebuild_link.log; then
                echo -e "${GREEN}Stage2 recovery link succeeded ($(wc -c < "$STAGE2" | tr -d ' ') bytes).${NC}"
            else
                echo -e "${RED}ERROR: Stage2 recovery link failed.${NC}"
                grep -E 'undefined|error' /tmp/safe_rebuild_link.log | head -10
                rm -rf "$SNAPSHOT_DIR" "$RECOVERY_DIR"
                exit 1
            fi
            rm -rf "$RECOVERY_DIR"
        else
            echo -e "${RED}ERROR: Stage2 build failed after generating only $LL_COUNT/$EXPECTED_STAGE2_MODULES .ll files from $LL_SOURCE.${NC}"
            echo "Check /tmp/safe_rebuild_stage2.log for details."
            tail -30 /tmp/safe_rebuild_stage2.log
            rm -rf "$SNAPSHOT_DIR"
            exit 1
        fi
        rm -rf "$SNAPSHOT_DIR"
    fi
fi

# macOS relink: The frozen bootstrap produces ThinLTO bitcode .o files which
# don't link correctly without -flto=thin. Relink from .opt.ll using llc + clang.
if [ "$HOST_OS" = "Darwin" ]; then
    echo ""
    echo "Step 1b: macOS relink from .opt.ll files (llc + clang -O2)..."

    # Post-opt fixup: LLVM opt may eliminate internal globals that are cross-module referenced.
    # Scan all .opt.ll files: for each 'external global @X' reference, ensure @X is defined
    # (non-external) in at least one module. If eliminated, re-inject from the original .ll.
    echo "    Fixing cross-module globals post-opt..."
    "$PYTHON3_PATH" - <<'POSTOPT_FIX'
import re, glob, os

opt_files = sorted(glob.glob('/tmp/seen_module_*.opt.ll'))
ll_files = sorted(glob.glob('/tmp/seen_module_*.ll'))
ll_files = [f for f in ll_files if not f.endswith('.opt.ll')]

# Collect: which opt files define globals, which declare them external
defined = {}   # gname -> (file, line)
external = {}  # gname -> set of files needing it

for f in opt_files:
    with open(f) as fh:
        for line in fh:
            gm = re.match(r'(@\w+)\s*=\s*(external\s+)?(?:local_unnamed_addr\s+)?(?:unnamed_addr\s+)?(?:internal\s+)?global\s+(\S+)', line)
            if gm:
                gname = gm.group(1)
                is_ext = gm.group(2) is not None
                if is_ext:
                    external.setdefault(gname, set()).add(f)
                else:
                    defined[gname] = (f, gm.group(3))

# Find globals referenced but not defined in any opt file
missing = set()
for gname in external:
    if gname not in defined:
        missing.add(gname)

if missing:
    # Try to find definitions in original .ll files
    orig_defs = {}
    for f in ll_files:
        with open(f) as fh:
            for line in fh:
                gm = re.match(r'(@\w+)\s*=\s*(?:internal\s+)?global\s+(\S+)\s+(.*)', line)
                if gm and gm.group(1) in missing:
                    orig_defs[gm.group(1)] = (gm.group(2), gm.group(3).strip(), f)

    # Inject missing globals into the first module that references them externally
    for gname in missing:
        if gname in orig_defs:
            gtype, gval, src = orig_defs[gname]
            # Add to first referencing opt file
            target = sorted(external[gname])[0]
            with open(target) as fh:
                content = fh.read()
            # Replace external declaration with actual definition
            content = re.sub(
                rf'^{re.escape(gname)}\s*=\s*external\s+(?:local_unnamed_addr\s+)?(?:unnamed_addr\s+)?global\s+\S+\s*$',
                f'{gname} = global {gtype} {gval}',
                content, count=1, flags=re.MULTILINE
            )
            with open(target, 'w') as fh:
                fh.write(content)
            print(f'    Injected {gname} into {os.path.basename(target)}')

if not missing:
    print('    Post-opt fixup: 0 missing globals')
else:
    print(f'    Post-opt fixup: {len(missing)} missing globals, {len(missing & set(orig_defs.keys()))} fixed')
POSTOPT_FIX

    EXPECTED_STAGE2_MODULES=$(extract_expected_module_count /tmp/safe_rebuild_stage2.log)
    OPT_LL_COUNT=$(count_module_opt_lls /tmp)
    if [ "$EXPECTED_STAGE2_MODULES" -le 0 ]; then
        echo -e "${RED}ERROR: Could not determine expected module count for macOS relink.${NC}"
        exit 1
    fi
    if [ "$OPT_LL_COUNT" -ne "$EXPECTED_STAGE2_MODULES" ]; then
        echo -e "${RED}ERROR: Refusing macOS relink with only $OPT_LL_COUNT/$EXPECTED_STAGE2_MODULES optimized modules.${NC}"
        exit 1
    fi
    if [ "$OPT_LL_COUNT" -gt 0 ]; then
        echo "    Found $OPT_LL_COUNT .opt.ll modules, relinking with llc..."
        RELINK_FAILED=0
        RELINK_OBJS=""
        for optll in /tmp/seen_module_*.opt.ll; do
            modname=$(basename "$optll" .opt.ll)
            objfile="/tmp/${modname}.relink.o"
            if ! llc -mtriple=arm64-apple-macosx -filetype=obj -O2 "$optll" -o "$objfile" 2>/tmp/relink_llc.log; then
                echo -e "${RED}    llc failed for $modname${NC}"
                cat /tmp/relink_llc.log
                RELINK_FAILED=1
                break
            fi
            RELINK_OBJS="$RELINK_OBJS $objfile"
        done
        if [ "$RELINK_FAILED" = "0" ]; then
            NATIVE_RT="/tmp/seen_runtime_native.o"
            clang -O2 -c -I seen_runtime seen_runtime/seen_runtime.c -o "$NATIVE_RT" 2>/dev/null || true
            NATIVE_REGION="/tmp/seen_region_native.o"
            [ -f seen_runtime/seen_region.c ] && clang -O2 -c -I seen_runtime seen_runtime/seen_region.c -o "$NATIVE_REGION" 2>/dev/null || true
            RT_OBJS="$NATIVE_RT"
            [ -f "$NATIVE_REGION" ] && RT_OBJS="$RT_OBJS $NATIVE_REGION"
            if clang -O2 -arch arm64 $RELINK_OBJS $RT_OBJS -o "$STAGE2" -lm -lpthread 2>/tmp/relink_link.log; then
                echo -e "${GREEN}    macOS relink succeeded ($(wc -c < "$STAGE2" | tr -d ' ') bytes).${NC}"
            else
                echo -e "${RED}    macOS relink failed${NC}"
                cat /tmp/relink_link.log
                exit 1
            fi
            rm -f /tmp/seen_module_*.relink.o "$NATIVE_RT" "$NATIVE_REGION"
        else
            echo -e "${RED}ERROR: macOS relink failed${NC}"
            exit 1
        fi
    fi
fi

if [ "$HOST_OS" = "Darwin" ]; then
    # macOS: full S2→S3 bootstrap verification works
    rm -rf .seen_cache/ /tmp/seen_ir_cache/
    rm -f /tmp/seen_module_*.ll /tmp/seen_module_*.o /tmp/seen_module_*.opt.ll

    echo ""
    echo "Step 2: Building stage3 with stage2 (--fast)..."
    if run_with_progress "S2→S3" /tmp/safe_rebuild_stage3.log $STAGE2 compile "$COMPILER_SOURCE" "$STAGE3" --fast --no-cache; then
        echo -e "${GREEN}Stage3 build succeeded.${NC}"
    else
        echo -e "${RED}ERROR: Stage3 build failed!${NC}"
        echo "Check /tmp/safe_rebuild_stage3.log for details."
        tail -30 /tmp/safe_rebuild_stage3.log
        rm -f "$STAGE2"
        exit 1
    fi

    # Relink stage3
    echo ""
    echo "Step 2b: macOS relink stage3 from .opt.ll files..."
    EXPECTED_STAGE3_MODULES=$(extract_expected_module_count /tmp/safe_rebuild_stage3.log)
    OPT_LL_COUNT=$(count_module_opt_lls /tmp)
    if [ "$EXPECTED_STAGE3_MODULES" -le 0 ]; then
        echo -e "${RED}ERROR: Could not determine expected module count for macOS stage3 relink.${NC}"
        exit 1
    fi
    if [ "$OPT_LL_COUNT" -ne "$EXPECTED_STAGE3_MODULES" ]; then
        echo -e "${RED}ERROR: Refusing macOS stage3 relink with only $OPT_LL_COUNT/$EXPECTED_STAGE3_MODULES optimized modules.${NC}"
        exit 1
    fi
    if [ "$OPT_LL_COUNT" -gt 0 ]; then
        echo "    Found $OPT_LL_COUNT .opt.ll modules, relinking with llc..."
        RELINK_FAILED=0
        RELINK_OBJS=""
        for optll in /tmp/seen_module_*.opt.ll; do
            modname=$(basename "$optll" .opt.ll)
            objfile="/tmp/${modname}.relink.o"
            if ! llc -mtriple=arm64-apple-macosx -filetype=obj -O2 "$optll" -o "$objfile" 2>/tmp/relink_llc.log; then
                echo -e "${RED}    llc failed for $modname${NC}"
                cat /tmp/relink_llc.log
                RELINK_FAILED=1
                break
            fi
            RELINK_OBJS="$RELINK_OBJS $objfile"
        done
        if [ "$RELINK_FAILED" = "0" ]; then
            NATIVE_RT="/tmp/seen_runtime_native.o"
            clang -O2 -c -I seen_runtime seen_runtime/seen_runtime.c -o "$NATIVE_RT" 2>/dev/null || true
            NATIVE_REGION="/tmp/seen_region_native.o"
            [ -f seen_runtime/seen_region.c ] && clang -O2 -c -I seen_runtime seen_runtime/seen_region.c -o "$NATIVE_REGION" 2>/dev/null || true
            RT_OBJS="$NATIVE_RT"
            [ -f "$NATIVE_REGION" ] && RT_OBJS="$RT_OBJS $NATIVE_REGION"
            if clang -O2 -arch arm64 $RELINK_OBJS $RT_OBJS -o "$STAGE3" -lm -lpthread 2>/tmp/relink_link.log; then
                echo -e "${GREEN}    macOS stage3 relink succeeded ($(wc -c < "$STAGE3" | tr -d ' ') bytes).${NC}"
            else
                echo -e "${RED}    macOS stage3 relink failed${NC}"
                cat /tmp/relink_link.log
                exit 1
            fi
            rm -f /tmp/seen_module_*.relink.o "$NATIVE_RT" "$NATIVE_REGION"
        else
            echo -e "${RED}ERROR: macOS stage3 relink failed${NC}"
            exit 1
        fi
    fi

    echo ""
    echo "Step 3: Verifying bootstrap..."
    if diff "$STAGE2" "$STAGE3" > /dev/null 2>&1; then
        echo -e "${GREEN}Bootstrap verified: Stage2 == Stage3 (identical binaries)!${NC}"
    else
        echo -e "${YELLOW}Note: Stage2 != Stage3 (expected if stage1_frozen is older than source).${NC}"
        echo -e "${GREEN}Stage3 build succeeded — using Stage3 as production compiler.${NC}"
    fi
    VERIFIED="$STAGE3"
else
    # Linux: Attempt S2→S3 bootstrap verification with a timeout.
    # If Bug A (SeenString field corruption) is fixed, S2 should be able to
    # cold-compile. Fall back to S2 if S2→S3 times out or fails.
    rm -rf .seen_cache/ /tmp/seen_ir_cache/
    rm -f /tmp/seen_module_*.ll /tmp/seen_module_*.o /tmp/seen_module_*.opt.ll

    echo ""
    echo "Step 2: Attempting S2→S3 bootstrap verification (Linux)..."
    echo -e "${DIM}Timeout: 30 minutes. Falls back to S2 if this fails.${NC}"

    if timeout 1800 bash -c "$(printf '%q' "$STAGE2") compile $(printf '%q' "$COMPILER_SOURCE") $(printf '%q' "$STAGE3") --fast --no-cache --no-fork" > /tmp/safe_rebuild_stage3.log 2>&1; then
        echo -e "${GREEN}Stage3 build succeeded.${NC}"

        echo ""
        echo "Step 3: Verifying bootstrap..."
        if diff "$STAGE2" "$STAGE3" > /dev/null 2>&1; then
            echo -e "${GREEN}Bootstrap verified: Stage2 == Stage3 (identical binaries)!${NC}"
        else
            echo -e "${YELLOW}Note: Stage2 != Stage3 (expected if stage1_frozen is older than source).${NC}"
            echo -e "${GREEN}Stage3 build succeeded — using Stage3 as production compiler.${NC}"
        fi
        VERIFIED="$STAGE3"
    else
        S3_EXIT=$?
        echo -e "${YELLOW}S2→S3 build failed or timed out (exit=$S3_EXIT).${NC}"
        if [ "$S3_EXIT" = "124" ]; then
            echo -e "${YELLOW}Timeout reached — cold-compile hang likely still present.${NC}"
        else
            echo "Check /tmp/safe_rebuild_stage3.log for details."
            tail -10 /tmp/safe_rebuild_stage3.log 2>/dev/null
        fi
        echo -e "${GREEN}Using Stage2 as production compiler (verified via frozen bootstrap).${NC}"
        VERIFIED="$STAGE2"
    fi

    rm -rf "$OPT_WRAPPER_DIR"
fi

# Install production compiler
echo ""
echo "Installing production compiler..."
mkdir -p compiler_seen/target
# Remove before copy to avoid "Text file busy" if the binary is in use
rm -f compiler_seen/target/seen 2>/dev/null || true
cp "$VERIFIED" compiler_seen/target/seen
chmod +x compiler_seen/target/seen
cp "$STAGE2" stage2_head 2>/dev/null || true
[ -f "$STAGE3" ] && cp "$STAGE3" stage3_head 2>/dev/null || true

# Also install to target/release/seen (README install path)
mkdir -p target/release
cp compiler_seen/target/seen target/release/seen
chmod +x target/release/seen

# Clean up
rm -f "$STAGE2" "$STAGE3"
rm -rf .seen_cache/
[ -n "$OPT_WRAPPER_DIR" ] && rm -rf "$OPT_WRAPPER_DIR"

echo ""
echo -e "${GREEN}=== Safe Rebuild Complete ===${NC}"
echo ""
echo "Production compiler updated: compiler_seen/target/seen"
echo "Also installed to: target/release/seen"
echo "Safe to commit your changes."
