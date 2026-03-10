#!/bin/bash
# Cross-compile Seen programs for Windows x86-64 from Linux/WSL
# Prerequisites: llvm-20, clang-20, gcc-mingw-w64-x86-64, python3
#
# Strategy:
#   1. Use frozen bootstrap to compile .seen source → .ll (LLVM IR)
#   2. Transform .ll for Windows x64 ABI (struct passing via byval)
#   3. Compile .ll → assembly via llc targeting mingw
#   4. Link with cross-compiled runtime via mingw-gcc → .exe
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
RUNTIME_DIR="${PROJECT_DIR}/seen_runtime"
OUTPUT_DIR="${PROJECT_DIR}/target-windows"
ABI_SCRIPT="${SCRIPT_DIR}/ll_win64_abi.py"

# Resolve versioned tool names
OPT=$(command -v opt-20 2>/dev/null || command -v opt 2>/dev/null || true)
LLC=$(command -v llc-20 2>/dev/null || command -v llc 2>/dev/null || true)
MINGW_GCC=$(command -v x86_64-w64-mingw32-gcc 2>/dev/null || true)

# Detect frozen compiler
if [ -f "${PROJECT_DIR}/compiler_seen/target/seen" ]; then
    SEEN="${PROJECT_DIR}/compiler_seen/target/seen"
elif [ -f "${PROJECT_DIR}/bootstrap/stage1_frozen_v3" ]; then
    SEEN="${PROJECT_DIR}/bootstrap/stage1_frozen_v3"
else
    echo "ERROR: No Seen compiler found"
    exit 1
fi

# Check prerequisites
MISSING=""
[ -z "$OPT" ] && MISSING="$MISSING opt(opt-20)"
[ -z "$LLC" ] && MISSING="$MISSING llc(llc-20)"
[ -z "$MINGW_GCC" ] && MISSING="$MISSING x86_64-w64-mingw32-gcc"
[ ! -f "$ABI_SCRIPT" ] && MISSING="$MISSING ll_win64_abi.py"
if [ -n "$MISSING" ]; then
    echo "ERROR: Missing:$MISSING"
    echo "Install: sudo apt-get install llvm-20 clang-20 lld-20 gcc-mingw-w64-x86-64"
    exit 1
fi

# Set up opt/clang wrapper for frozen compiler bugs
setup_tool_wrappers() {
    REAL_OPT="$OPT"
    mkdir -p /tmp/seen_opt_override
    cat > /tmp/seen_opt_override/opt << 'WRAPPER_EOF'
#!/bin/bash
ARGS=("$@")
for arg in "${ARGS[@]}"; do
    if [[ "$arg" == *.ll && "$arg" != *.opt.ll && -f "$arg" ]]; then
        awk '
        NR == FNR { if (/^declare /) { if (match($0, /@([A-Za-z0-9_.]+)/, m)) { count[m[1]]++; seen_count[m[1]] = 0 } } next }
        /^declare / { if (match($0, /@([A-Za-z0-9_.]+)/, m)) { fname = m[1]; seen_count[fname]++; if (count[fname] > 1 && seen_count[fname] < count[fname]) next } }
        { print }
        ' "$arg" "$arg" > "${arg}.dedup" && mv "${arg}.dedup" "$arg"

        python3 -c "
import re, sys
with open(sys.argv[1]) as f: content = f.read()
pattern = re.compile(r'  (%\d+) = call %SeenString @seen_int_to_string\(i64 (%\d+)\)\n  (%\d+) = call %SeenString @seen_char_to_str\(i64 (%\d+)\)\n  (%\d+) = call %SeenString @seen_str_concat_ss\(%SeenString \1, %SeenString \3\)')
def fix(m): return f'  {m.group(1)} = add i64 0, 0\n  {m.group(3)} = add i64 0, 0\n  {m.group(5)} = add i64 {m.group(2)}, {m.group(4)}'
new_content, count = pattern.subn(fix, content)
if count > 0:
    with open(sys.argv[1], 'w') as f: f.write(new_content)
    print(f'  byteAt fix: patched {count} site(s)', file=sys.stderr)
" "$arg" 2>&1 || true
    fi
done
WRAPPER_EOF
    sed -i "1a REAL_OPT=\"$REAL_OPT\"" /tmp/seen_opt_override/opt
    echo "exec \"\$REAL_OPT\" \"\$@\"" >> /tmp/seen_opt_override/opt
    chmod +x /tmp/seen_opt_override/opt

    # Symlink clang and lld if they're versioned
    for tool in clang clang++ ld.lld; do
        if ! command -v "$tool" &>/dev/null; then
            versioned=$(command -v "${tool}-20" 2>/dev/null || command -v "${tool%.*}-20" 2>/dev/null || true)
            [ -n "$versioned" ] && ln -sf "$versioned" "/tmp/seen_opt_override/$tool"
        fi
    done
}

# Cross-compile the C runtime for Windows (cached)
compile_runtime() {
    if [ ! -f "$OUTPUT_DIR/seen_runtime_win.o" ] || \
       [ "$RUNTIME_DIR/seen_runtime.c" -nt "$OUTPUT_DIR/seen_runtime_win.o" ]; then
        echo "  Compiling runtime for Windows..."
        $MINGW_GCC -c -O2 -DNDEBUG -D_WIN32 \
            "$RUNTIME_DIR/seen_runtime.c" \
            -o "$OUTPUT_DIR/seen_runtime_win.o" \
            -I"$RUNTIME_DIR"
    fi
}

# Main: cross-compile a .seen file to Windows .exe
cross_compile() {
    local INPUT="$1"
    local OUTPUT_EXE="$2"

    if [ -z "$INPUT" ] || [ -z "$OUTPUT_EXE" ]; then
        echo "Usage: $0 <input.seen> <output.exe>"
        echo ""
        echo "Cross-compiles a Seen source file to a Windows x86-64 executable."
        echo "Run from WSL with LLVM and mingw-w64 installed."
        exit 1
    fi

    mkdir -p "$OUTPUT_DIR"

    echo "=== Cross-compiling for Windows x86-64 ==="
    echo "  Input:  $INPUT"
    echo "  Output: $OUTPUT_EXE"
    echo ""

    # Step 1: Set up tool wrappers
    setup_tool_wrappers

    # Step 2: Compile .seen → .ll (Linux LLVM IR)
    echo "[1/5] Compiling Seen source to LLVM IR..."
    local LINUX_BIN="/tmp/seen_win_build_$$"
    rm -rf .seen_cache /tmp/seen_ir_cache /tmp/seen_module_*.ll /tmp/seen_module_*.o
    chmod +x "$SEEN"
    PATH="/tmp/seen_opt_override:$PATH" "$SEEN" compile "$INPUT" "$LINUX_BIN" --fast 2>&1
    echo ""

    # Step 3: Collect .ll files (exclude .opt.ll which are optimized duplicates)
    local LL_FILES=$(ls /tmp/seen_module_*.ll 2>/dev/null | grep -v '\.opt\.ll' || true)
    local LL_COUNT=$(echo "$LL_FILES" | grep -c "seen_module" 2>/dev/null || echo 0)
    echo "[2/5] Found $LL_COUNT module .ll files"

    if [ "$LL_COUNT" -eq 0 ]; then
        echo "ERROR: No .ll files generated"
        exit 1
    fi

    # Step 4: Transform .ll for Windows ABI
    echo "[3/5] Transforming IR for Windows x64 ABI..."
    local WIN_ASM_FILES=""
    for ll in $LL_FILES; do
        local base=$(basename "$ll" .ll)
        local win_ll="$OUTPUT_DIR/${base}_win.ll"
        local win_asm="$OUTPUT_DIR/${base}_win.s"

        python3 "$ABI_SCRIPT" "$ll" "$win_ll"
        $LLC "$win_ll" -o "$win_asm" -mtriple=x86_64-w64-mingw32 -O2 --filetype=asm 2>&1
        # Remove LLVM directives that GNU assembler doesn't understand
        sed -i '/.addrsig/d; /\.section\s.*,discard,/d' "$win_asm"
        WIN_ASM_FILES="$WIN_ASM_FILES $win_asm"
        echo "  $base -> OK"
    done

    # Step 5: Compile runtime
    echo ""
    echo "[4/5] Cross-compiling runtime..."
    compile_runtime

    # Step 6: Link
    echo "[5/5] Linking $OUTPUT_EXE..."
    $MINGW_GCC -static \
        $WIN_ASM_FILES \
        "$OUTPUT_DIR/seen_runtime_win.o" \
        -o "$OUTPUT_EXE" \
        -lkernel32 -ladvapi32 -lshell32 -lws2_32

    # Clean up temp files
    rm -f "$LINUX_BIN" $OUTPUT_DIR/seen_module_*_win.ll $OUTPUT_DIR/seen_module_*_win.s

    local SIZE=$(stat -c%s "$OUTPUT_EXE" 2>/dev/null || stat -f%z "$OUTPUT_EXE")
    echo ""
    echo "=== SUCCESS ==="
    echo "Built: $OUTPUT_EXE ($(( SIZE / 1024 ))KB)"
}

cross_compile "$@"
