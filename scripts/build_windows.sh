#!/bin/bash
# Cross-compile Seen programs for Windows x86-64 from Linux/WSL
# Prerequisites: llvm-20, clang-20, gcc-mingw-w64-x86-64, python3
#
# Strategy:
#   1. Use frozen bootstrap to compile .seen source → .ll (LLVM IR)
#   2. Transform .ll for Windows x64 ABI (struct passing via byval)
#   3. Compile transformed .ll → COFF object via clang targeting mingw
#   4. Link with cross-compiled runtime via mingw-gcc → .exe
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
RUNTIME_DIR="${PROJECT_DIR}/seen_runtime"
OUTPUT_DIR="${PROJECT_DIR}/target-windows"
ABI_SCRIPT="${SCRIPT_DIR}/ll_win64_abi.py"
BUILD_TRACE_COMMON="${SCRIPT_DIR}/build_trace_common.sh"
if [ -f "$BUILD_TRACE_COMMON" ]; then
    # shellcheck source=scripts/build_trace_common.sh
    source "$BUILD_TRACE_COMMON"
    seen_build_trace_init "build_windows"
fi

# Resolve versioned tool names
OPT=$(command -v opt-20 2>/dev/null || command -v opt 2>/dev/null || true)
LLC=$(command -v llc-20 2>/dev/null || command -v llc 2>/dev/null || true)
CLANG=$(command -v clang-20 2>/dev/null || command -v clang 2>/dev/null || true)
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

read -r -a SEEN_WINDOWS_COMPILE_FLAGS <<< "${SEEN_WINDOWS_COMPILE_FLAGS:---fast --no-cache --no-fork --target-cpu=x86-64 --simd=none}"
SEEN_WINDOWS_OBJECT_BACKEND="${SEEN_WINDOWS_OBJECT_BACKEND:-clang}"
SEEN_WINDOWS_CLANG_OPT="${SEEN_WINDOWS_CLANG_OPT:--O2}"
read -r -a SEEN_WINDOWS_CLANG_WARN_FLAGS <<< "${SEEN_WINDOWS_CLANG_WARN_FLAGS:--Wno-pass-failed}"
SEEN_WINDOWS_LLC_OPT="${SEEN_WINDOWS_LLC_OPT:--O2}"
read -r -a SEEN_WINDOWS_LINK_FLAGS <<< "${SEEN_WINDOWS_LINK_FLAGS:--Wl,--allow-multiple-definition}"

# Check prerequisites
MISSING=""
[ -z "$OPT" ] && MISSING="$MISSING opt(opt-20)"
[ -z "$MINGW_GCC" ] && MISSING="$MISSING x86_64-w64-mingw32-gcc"
[ ! -f "$ABI_SCRIPT" ] && MISSING="$MISSING ll_win64_abi.py"
case "$SEEN_WINDOWS_OBJECT_BACKEND" in
    clang)
        [ -z "$CLANG" ] && MISSING="$MISSING clang(clang-20)"
        ;;
    llc)
        [ -z "$LLC" ] && MISSING="$MISSING llc(llc-20)"
        ;;
    *)
        echo "ERROR: Unsupported SEEN_WINDOWS_OBJECT_BACKEND=$SEEN_WINDOWS_OBJECT_BACKEND"
        echo "Supported backends: clang, llc"
        exit 1
        ;;
esac
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

        # Comprehensive IR fixups (struct-zero, ptr-null, declare conflicts, allocsize, etc.)
        FIX_IR="$(dirname "$(readlink -f "$0")")/../scripts/fix_ir.py"
        [ ! -f "$FIX_IR" ] && FIX_IR="$(cd "$(dirname "$0")/.." && pwd)/scripts/fix_ir.py"
        [ -f "$FIX_IR" ] && python3 "$FIX_IR" "$arg" 2>&1 || true
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
    local runtime_key runtime_cache_dir runtime_cache_obj
    runtime_key=$(windows_hash_key \
        "runtime-v2" \
        "$RUNTIME_DIR/seen_runtime.c" \
        "$RUNTIME_DIR/seen_runtime.h" \
        "$RUNTIME_DIR/seen_region.c" \
        "$RUNTIME_DIR/seen_region.h" \
        "$RUNTIME_DIR/seen_tee_common.c" \
        "$RUNTIME_DIR/seen_tee.h" \
        "$RUNTIME_DIR/seen_compat_win32.h")
    runtime_cache_dir="$OUTPUT_DIR/runtime-cache"
    runtime_cache_obj="$runtime_cache_dir/$runtime_key.o"
    mkdir -p "$runtime_cache_dir"
    if [ -f "$runtime_cache_obj" ]; then
        cp "$runtime_cache_obj" "$OUTPUT_DIR/seen_runtime_win.o"
        if declare -F seen_build_trace_event >/dev/null 2>&1; then
            seen_build_trace_event "windows runtime cache" "hit" "$runtime_key"
        fi
    else
        echo "  Compiling runtime for Windows..."
        $MINGW_GCC -c -O2 -DNDEBUG -D_WIN32 \
            "$RUNTIME_DIR/seen_runtime.c" \
            -o "$OUTPUT_DIR/seen_runtime_win.o" \
            -I"$RUNTIME_DIR"
        cp "$OUTPUT_DIR/seen_runtime_win.o" "$runtime_cache_obj"
        if declare -F seen_build_trace_event >/dev/null 2>&1; then
            seen_build_trace_event "windows runtime cache" "miss" "$runtime_key"
        fi
    fi
}

hash_file_for_windows() {
    local file="$1"
    if [ -f "$file" ]; then
        sha256sum "$file" | awk '{print $1}'
    elif [ -d "$file" ]; then
        find "$file" -type f -print0 | sort -z | xargs -0 sha256sum 2>/dev/null | sha256sum | awk '{print $1}'
    else
        printf 'missing'
    fi
}

windows_hash_key() {
    local label="$1"
    shift
    {
        printf '%s\n' "$label"
        printf 'seen=%s\n' "$(hash_file_for_windows "$SEEN")"
        printf 'abi=%s\n' "$(hash_file_for_windows "$ABI_SCRIPT")"
        printf 'backend=%s\n' "$SEEN_WINDOWS_OBJECT_BACKEND"
        printf 'clang_opt=%s\n' "$SEEN_WINDOWS_CLANG_OPT"
        printf 'clang_warn=%s\n' "${SEEN_WINDOWS_CLANG_WARN_FLAGS[*]}"
        printf 'llc_opt=%s\n' "$SEEN_WINDOWS_LLC_OPT"
        printf 'clang=%s\n' "$($CLANG --version 2>/dev/null | head -1 || true)"
        printf 'llc=%s\n' "$($LLC --version 2>/dev/null | head -1 || true)"
        printf 'mingw=%s\n' "$($MINGW_GCC --version 2>/dev/null | head -1 || true)"
        printf 'flags=%s\n' "${SEEN_WINDOWS_COMPILE_FLAGS[*]}"
        local path
        for path in "$@"; do
            printf '%s=%s\n' "$path" "$(hash_file_for_windows "$path")"
        done
    } | sha256sum | awk '{print $1}'
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
    rm -f "$OUTPUT_DIR"/seen_module_*_win.ll "$OUTPUT_DIR"/seen_module_*_win.s "$OUTPUT_DIR"/seen_module_*_win.o 2>/dev/null || true

    echo "=== Cross-compiling for Windows x86-64 ==="
    echo "  Input:  $INPUT"
    echo "  Output: $OUTPUT_EXE"
    echo "  Object backend: $SEEN_WINDOWS_OBJECT_BACKEND"
    echo ""

    # Step 1: Set up tool wrappers
    setup_tool_wrappers

    # Step 2: Compile .seen → .ll (Linux LLVM IR)
    echo "[1/5] Compiling Seen source to LLVM IR..."
    local LINUX_BIN="/tmp/seen_win_build_$$"
    local IR_KEY IR_DIR IR_TMP_DIR
    IR_KEY=$(windows_hash_key "ir-v1" "$INPUT" "$PROJECT_DIR/seen_std/src" "$PROJECT_DIR/seen_runtime/seen_runtime.c")
    IR_DIR="$OUTPUT_DIR/ir-cache/$IR_KEY"
    chmod +x "$SEEN"
    if [ -f "$IR_DIR/.ready" ]; then
        echo "  Reusing Windows IR cache: $IR_KEY"
        if declare -F seen_build_trace_event >/dev/null 2>&1; then
            seen_build_trace_event "windows ir cache" "hit" "$IR_KEY"
        fi
    else
        IR_TMP_DIR="$OUTPUT_DIR/ir-cache/$IR_KEY.tmp.$$"
        rm -rf "$IR_TMP_DIR"
        mkdir -p "$IR_TMP_DIR"
        SEEN_COMPILER_SOURCE_ROOT="${SEEN_COMPILER_SOURCE_ROOT:-$PROJECT_DIR}" \
            PATH="/tmp/seen_opt_override:$PATH" "$SEEN" compile "$INPUT" "$LINUX_BIN" \
            "${SEEN_WINDOWS_COMPILE_FLAGS[@]}" \
            --emit-module-ir-dir "$IR_TMP_DIR" --stop-after-ir 2>&1
        touch "$IR_TMP_DIR/.ready"
        rm -rf "$IR_DIR"
        mv "$IR_TMP_DIR" "$IR_DIR"
        if declare -F seen_build_trace_event >/dev/null 2>&1; then
            seen_build_trace_event "windows ir cache" "miss" "$IR_KEY"
        fi
    fi
    echo ""

    # Step 3: Collect .ll files (exclude .opt.ll which are optimized duplicates)
    local LL_FILES=$(find "$IR_DIR" -maxdepth 1 -type f -name 'seen_module_*.ll' ! -name '*.opt.ll' ! -name '*.polly.ll' | sort || true)
    local LL_COUNT=$(echo "$LL_FILES" | grep -c "seen_module" 2>/dev/null || echo 0)
    echo "[2/5] Found $LL_COUNT module .ll files"

    if [ "$LL_COUNT" -eq 0 ]; then
        echo "ERROR: No .ll files generated"
        exit 1
    fi

    # Step 4: Transform .ll for Windows ABI
    echo "[3/5] Transforming IR for Windows x64 ABI..."
    local WIN_LINK_INPUTS=""
    local OBJECT_CACHE_DIR="$OUTPUT_DIR/object-cache"
    mkdir -p "$OBJECT_CACHE_DIR"
    for ll in $LL_FILES; do
        local base=$(basename "$ll" .ll)
        local win_ll="$OUTPUT_DIR/${base}_win.ll"
        local win_asm="$OUTPUT_DIR/${base}_win.s"
        local win_obj="$OUTPUT_DIR/${base}_win.o"
        local object_key object_cache_obj object_cache_tmp

        object_key=$(windows_hash_key "object-v1" "$ll")
        object_cache_obj="$OBJECT_CACHE_DIR/$object_key.o"
        if [ -f "$object_cache_obj" ]; then
            WIN_LINK_INPUTS="$WIN_LINK_INPUTS $object_cache_obj"
            echo "  $base -> cache hit"
            if declare -F seen_build_trace_event >/dev/null 2>&1; then
                seen_build_trace_event "windows object cache" "hit" "$object_key"
            fi
            continue
        fi

        python3 "$ABI_SCRIPT" "$ll" "$win_ll"

        if [ "$SEEN_WINDOWS_OBJECT_BACKEND" = "clang" ]; then
            "$CLANG" -target x86_64-w64-windows-gnu -c "$win_ll" -o "$win_obj" "$SEEN_WINDOWS_CLANG_OPT" "${SEEN_WINDOWS_CLANG_WARN_FLAGS[@]}" 2>&1
            object_cache_tmp="${object_cache_obj}.tmp.$$"
            cp "$win_obj" "$object_cache_tmp"
            mv -f "$object_cache_tmp" "$object_cache_obj"
            WIN_LINK_INPUTS="$WIN_LINK_INPUTS $object_cache_obj"
        else
            $LLC "$win_ll" -o "$win_asm" -mtriple=x86_64-w64-mingw32 "$SEEN_WINDOWS_LLC_OPT" --filetype=asm 2>&1
            # Remove LLVM directives that GNU assembler doesn't understand
            sed -i \
                -e '/.addrsig/d' \
                -e '/\.section\s.*,discard,/d' \
                -e 's/\.section[[:space:]]\+\.ctors,"dw",unique,[0-9]\+/.section .ctors,"dw"/' \
                "$win_asm"
            WIN_LINK_INPUTS="$WIN_LINK_INPUTS $win_asm"
        fi
        echo "  $base -> OK"
        if declare -F seen_build_trace_event >/dev/null 2>&1; then
            seen_build_trace_event "windows object cache" "miss" "$object_key"
        fi
    done

    # Step 5: Compile runtime
    echo ""
    echo "[4/5] Cross-compiling runtime..."
    compile_runtime

    # Step 6: Link
    echo "[5/5] Linking $OUTPUT_EXE..."
    $MINGW_GCC -static \
        $WIN_LINK_INPUTS \
        "$OUTPUT_DIR/seen_runtime_win.o" \
        -o "$OUTPUT_EXE" \
        "${SEEN_WINDOWS_LINK_FLAGS[@]}" \
        -lkernel32 -ladvapi32 -lshell32 -lws2_32

    # Clean up temp files
    rm -f "$LINUX_BIN" $OUTPUT_DIR/seen_module_*_win.ll $OUTPUT_DIR/seen_module_*_win.s $OUTPUT_DIR/seen_module_*_win.o

    local SIZE=$(stat -c%s "$OUTPUT_EXE" 2>/dev/null || stat -f%z "$OUTPUT_EXE")
    echo ""
    echo "=== SUCCESS ==="
    echo "Built: $OUTPUT_EXE ($(( SIZE / 1024 ))KB)"
}

cross_compile "$@"
