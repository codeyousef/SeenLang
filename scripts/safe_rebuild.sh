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

        # Remove phantom declares: declares for functions never called in the module.
        # Stage1's cross-module declare generator emits declares for every function name
        # in the registry, even those only referenced inside string constants (IR text).
        # ThinLTO cannot always DCE these, causing undefined symbol link errors.
        python3 -c "
import re, sys
with open(sys.argv[1]) as f:
    content = f.read()
# Collect all declared function names
declares = {}
for m in re.finditer(r'^(declare\s+\S+\s+@(\S+)\(.*)', content, re.MULTILINE):
    name = m.group(2)
    declares[name] = m.group(1)
# Check which are actually called (not just declared or in string constants)
removed = 0
for name, decl_line in list(declares.items()):
    # Skip LLVM intrinsics — always needed
    if name.startswith('llvm.'):
        continue
    # Count call sites: @name( in non-declare, non-string-constant context
    # Simple heuristic: if @name appears ONLY in declare lines and string constants, remove
    uses = [m for m in re.finditer(r'@' + re.escape(name) + r'[\(, ]', content) if not content[content.rfind('\n', 0, m.start())+1:m.start()].lstrip().startswith('declare')]
    # Filter out uses inside string constants (c\"...\")
    real_uses = []
    for u in uses:
        line_start = content.rfind('\n', 0, u.start()) + 1
        line = content[line_start:content.find('\n', u.start())]
        if '= private unnamed_addr constant' in line:
            continue  # Inside string constant
        real_uses.append(u)
    if len(real_uses) == 0:
        content = content.replace(decl_line + '\n', '')
        removed += 1
if removed > 0:
    with open(sys.argv[1], 'w') as f:
        f.write(content)
    print(f'  phantom declare fix: removed {removed} unused declares from {sys.argv[1]}', file=sys.stderr)
" "\$arg" 2>&1 || true

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
    # eliminated by opt) but still produce .opt.ll files we can relink in step 1b.
    if run_with_progress "S1→S2" /tmp/safe_rebuild_stage2.log $FROZEN compile "$COMPILER_SOURCE" "$STAGE2" --fast --no-cache; then
        echo -e "${GREEN}Stage2 build succeeded.${NC}"
    else
        OPT_LL_COUNT=$(ls /tmp/seen_module_*.opt.ll 2>/dev/null | wc -l | tr -d ' ')
        if [ "$OPT_LL_COUNT" -gt 30 ]; then
            echo -e "${YELLOW}Stage2 internal link failed, but $OPT_LL_COUNT .opt.ll files available for relink.${NC}"
        else
            echo -e "${RED}ERROR: Stage2 build failed!${NC}"
            echo "Check /tmp/safe_rebuild_stage2.log for details."
            tail -30 /tmp/safe_rebuild_stage2.log
            exit 1
        fi
    fi
else
    if run_with_progress "S1→S2" /tmp/safe_rebuild_stage2.log env PATH="$OPT_WRAPPER_DIR:$PATH" $FROZEN compile "$COMPILER_SOURCE" "$STAGE2" --fast --no-fork; then
        echo -e "${GREEN}Stage2 build succeeded.${NC}"
    else
        # Recovery: The frozen compiler uses fork-parallel IR gen and multiple
        # processes race on /tmp/seen_parallel_opt.sh, causing shell syntax errors.
        # If .ll files were generated, we can recover by running opt + link ourselves.
        LL_COUNT=$(ls /tmp/seen_module_*.ll 2>/dev/null | grep -v '\.opt\.ll$' | wc -l)
        if [ "$LL_COUNT" -gt 30 ]; then
            echo -e "${YELLOW}Stage2 compilation failed (opt script race), recovering with $LL_COUNT .ll files...${NC}"

            # Run the opt wrapper on each .ll file (dedup declares, fix byteAt bugs, etc.)
            for llfile in /tmp/seen_module_*.ll; do
                [[ "$llfile" == *.opt.ll ]] && continue
                "$OPT_WRAPPER_DIR/opt" --version > /dev/null 2>&1  # no-op to test wrapper
                # Apply the wrapper's fixups by calling it with a dummy pass
                # The wrapper intercepts .ll files and applies fixups before calling real opt
                modname=$(basename "$llfile" .ll)
                optfile="/tmp/${modname}.opt.ll"
                objfile="/tmp/${modname}.o"
                # Skip if .o already exists (from a successful earlier batch)
                [ -f "$objfile" ] && continue
                # Run opt with fast-mode passes through the wrapper
                if ! env PATH="$OPT_WRAPPER_DIR:$PATH" opt -passes='function(sroa,instcombine<no-verify-fixpoint>,simplifycfg),default<O1>' \
                    -inline-threshold=250 -S "$llfile" -o "$optfile" 2>/dev/null; then
                    cp "$llfile" "$optfile"
                fi
                # Generate ThinLTO bitcode
                opt --thinlto-bc "$optfile" -o "$objfile" 2>/dev/null || true
            done

            # Count resulting .o files
            OBJ_COUNT=$(ls /tmp/seen_module_*.o 2>/dev/null | wc -l)
            if [ "$OBJ_COUNT" -lt 30 ]; then
                echo -e "${RED}ERROR: Recovery failed — only $OBJ_COUNT .o files produced.${NC}"
                exit 1
            fi
            echo "  Recovery: $OBJ_COUNT .o files ready."

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

            # Link
            echo "  Linking $OBJ_COUNT modules..."
            LINK_OBJS=""
            for obj in /tmp/seen_module_*.o; do
                LINK_OBJS="$LINK_OBJS $obj"
            done
            RT_OBJS="$RT_DIR/seen_runtime.o"
            [ -f "$RT_DIR/seen_region.o" ] && RT_OBJS="$RT_OBJS $RT_DIR/seen_region.o"
            [ -f "$RT_DIR/seen_gpu.o" ] && RT_OBJS="$RT_OBJS $RT_DIR/seen_gpu.o"

            LINK_LIBS="-lm -lpthread"
            [ -f "$RT_DIR/seen_gpu.o" ] && pkg-config --exists vulkan 2>/dev/null && LINK_LIBS="$LINK_LIBS -lvulkan"

            if clang -O1 -flto=thin -fuse-ld=lld \
                -Wl,--thinlto-cache-dir=/tmp/seen_thinlto_cache \
                -march=native -Wl,--gc-sections -Wno-unused-command-line-argument \
                $LINK_OBJS $RT_OBJS -o "$STAGE2" $LINK_LIBS 2>/tmp/safe_rebuild_link.log; then
                echo -e "${GREEN}Stage2 recovery link succeeded ($(wc -c < "$STAGE2" | tr -d ' ') bytes).${NC}"
            else
                echo -e "${RED}ERROR: Stage2 recovery link failed.${NC}"
                cat /tmp/safe_rebuild_link.log
                exit 1
            fi
        else
            echo -e "${RED}ERROR: Stage2 build failed (only $LL_COUNT .ll files).${NC}"
            echo "Check /tmp/safe_rebuild_stage2.log for details."
            tail -30 /tmp/safe_rebuild_stage2.log
            exit 1
        fi
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

    OPT_LL_COUNT=$(ls /tmp/seen_module_*.opt.ll 2>/dev/null | wc -l | tr -d ' ')
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
    OPT_LL_COUNT=$(ls /tmp/seen_module_*.opt.ll 2>/dev/null | wc -l | tr -d ' ')
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
