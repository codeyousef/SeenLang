#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-import-c-typedefs
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != "1" ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- bash "$0" "$@"
fi
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/seen-import-c-typedefs.XXXXXX")"
HEADER="$TMP_DIR/typedefs.h"
OUT="$TMP_DIR/typedefs_bindings.seen"

cleanup() {
    local status=$?
    trap - EXIT
    if [ "$status" -ne 0 ] || [ -n "${SEEN_KEEP_TMP:-}" ]; then
        echo "Preserved typedef regression artifacts: $TMP_DIR (status $status)" >&2
        if [ "$status" -ne 0 ]; then
            for log in import-c.stdout.log import-c.stderr.log check.log compile.log; do
                if [ -f "$TMP_DIR/$log" ]; then
                    printf 'Typedef regression diagnostic: %s\n' "$TMP_DIR/$log" >&2
                    tail -c 16384 -- "$TMP_DIR/$log" >&2 || true
                fi
            done
        fi
        exit "$status"
    fi
    case "$TMP_DIR" in
        "$SEEN_ARTIFACT_ROOT"/seen-import-c-typedefs.*)
            [ -d "$TMP_DIR" ] && [ ! -L "$TMP_DIR" ] &&
                [ "$(dirname -- "$TMP_DIR")" = "$SEEN_ARTIFACT_ROOT" ] || exit 1
            rm -rf -- "$TMP_DIR" || exit 1 ;;
        *) exit 1 ;;
    esac
    exit 0
}

trap cleanup EXIT

cat >"$HEADER" <<'EOF'
typedef unsigned int MyFlags;
typedef struct MyHandle_T* MyHandle;
typedef float MyRgba[4];
typedef struct MyInfo {
    MyFlags flags;
    MyHandle handle;
    const char* name;
    MyRgba rgba;
} MyInfo;
typedef void (*MyCallback)(MyHandle h, const MyInfo* info);

MyFlags do_thing(MyHandle handle, MyFlags flags, MyCallback callback, const MyInfo* info, MyHandle* out_handle);
typedef int (*ScalarCallback)(int value);
ScalarCallback get_callback(void);
int call_callback(ScalarCallback callback, int value);
int widen_i8(signed char value);
unsigned int widen_u8(unsigned char value);
int widen_i16(short value);
unsigned int widen_u16(unsigned short value);
unsigned int identity_u32(unsigned int value);
float add_half_f32(float value);
double add_half_f64(double value);
const char* get_text(void);
int check_text(const char* value);
signed char echo_i8(signed char value);
unsigned char echo_u8(unsigned char value);
short echo_i16(short value);
unsigned short echo_u16(unsigned short value);
_Bool echo_bool(_Bool value);
typedef signed char SignedByteAlias;
SignedByteAlias echo_byte_alias(SignedByteAlias value);
int widen_byte_alias(SignedByteAlias value);
EOF

bash "$ATTESTED_SEEN" "$COMPILER" import-c "$HEADER" \
    >"$TMP_DIR/import-c.stdout.log" 2>"$TMP_DIR/import-c.stderr.log"
sed -n '/^\/\/ Auto-generated/,$p' "$TMP_DIR/import-c.stdout.log" >"$OUT"

if [ "$(grep -c '^type MyFlags = UInt32$' "$OUT")" -ne 1 ]; then
    echo "FAIL: import-c should emit MyFlags typedef alias"
    exit 1
fi

if [ "$(grep -c '^type MyHandle = Ptr$' "$OUT")" -ne 1 ]; then
    echo "FAIL: import-c should emit MyHandle opaque pointer alias"
    exit 1
fi

if [ "$(grep -c '^type MyRgba = Float32\[4\]$' "$OUT")" -ne 1 ]; then
    echo "FAIL: import-c should emit fixed-array typedef aliases"
    exit 1
fi

if [ "$(grep -Fxc 'class MyInfo {' "$OUT")" -ne 1 ]; then
    echo "FAIL: import-c should emit a repr(C) class for MyInfo"
    exit 1
fi

if [ "$(grep -Fxc 'type MyCallback = fn(MyHandle, *MyInfo) -> Void' "$OUT")" -ne 1 ]; then
    echo "FAIL: import-c should emit a typed MyCallback function-pointer alias"
    exit 1
fi

if [ "$(grep -c '^    var flags: MyFlags$' "$OUT")" -ne 1 ]; then
    echo "FAIL: import-c should preserve typedef aliases in generated struct fields"
    exit 1
fi

if [ "$(grep -c '^    var handle: MyHandle$' "$OUT")" -ne 1 ]; then
    echo "FAIL: import-c should preserve opaque handle aliases in generated struct fields"
    exit 1
fi

if [ "$(grep -c '^    var name: \*Char$' "$OUT")" -ne 1 ]; then
    echo "FAIL: import-c should emit raw pointer fields for C string members"
    exit 1
fi

if [ "$(grep -c '^    var rgba: MyRgba$' "$OUT")" -ne 1 ]; then
    echo "FAIL: import-c should preserve fixed-array typedef aliases in generated struct fields"
    exit 1
fi

if [ "$(grep -c '^extern fun do_thing(arg0: MyHandle, arg1: MyFlags, arg2: MyCallback, arg3: \*MyInfo, arg4: \*MyHandle) r: MyFlags$' "$OUT")" -ne 1 ]; then
    echo "FAIL: import-c should reuse typedefs and record pointers in generated function signatures"
    exit 1
fi

# The cross-module fixture deliberately exports its generated C declarations;
# the same-module importer assertions above still inspect unchanged output.
sed 's/^extern fun /pub extern fun /' "$OUT" > "$TMP_DIR/imported_typedefs.seen"
cat >"$TMP_DIR/consumer.seen" <<'EOF'
import imported_typedefs.{get_callback, call_callback, widen_i8, widen_u8, widen_i16, widen_u16, identity_u32, add_half_f32, add_half_f64, get_text, check_text, echo_i8, echo_u8, echo_i16, echo_u16, echo_bool, echo_byte_alias, widen_byte_alias}

fun local_i8(value: Int8) r: Int8 { return value }

fun main() r: Int {
    let callback = get_callback()
    if call_callback(callback, 41 as Int32) != (42 as Int32) { return 1 }
    if call_callback(callback, -41 as Int32) != (-40 as Int32) { return 2 }
    if widen_i8(-101 as Int8) != (-101 as Int32) { return 3 }
    if widen_u8(250 as UInt8) != (250 as UInt32) { return 4 }
    if widen_i16(-30000 as Int16) != (-30000 as Int32) { return 5 }
    if widen_u16(60000 as UInt16) != (60000 as UInt32) { return 6 }
    if identity_u32(4294967000 as UInt32) != (4294967000 as UInt32) { return 7 }
    if add_half_f32(1.25 as Float32) != (1.75 as Float32) { return 8 }
    if add_half_f64(1.25 as Float64) != (1.75 as Float64) { return 9 }
    let text = get_text()
    if check_text(text) != (1 as Int32) { return 10 }
    if echo_i8(-101 as Int8) != (-101 as Int8) { return 11 }
    if echo_u8(250 as UInt8) != (250 as UInt8) { return 12 }
    if echo_i16(-30000 as Int16) != (-30000 as Int16) { return 13 }
    if echo_u16(60000 as UInt16) != (60000 as UInt16) { return 14 }
    if not echo_bool(true) or echo_bool(false) { return 15 }
    if local_i8(-101 as Int8) != (-101 as Int8) { return 16 }
    if echo_byte_alias(-101 as Int8) != (-101 as Int8) { return 17 }
    if widen_byte_alias(-101 as Int8) != (-101 as Int32) { return 18 }
    return 0
}
EOF
sed '1d' "$TMP_DIR/consumer.seen" >> "$OUT"

mkdir -p "$TMP_DIR/native/lib"
cat > "$TMP_DIR/native/callback.c" <<'EOF'
#include "../typedefs.h"
static int plus_one(int value) { return value + 1; }
ScalarCallback get_callback(void) { return plus_one; }
int call_callback(ScalarCallback callback, int value) { return callback(value); }
int widen_i8(signed char value) { return value; }
unsigned int widen_u8(unsigned char value) { return value; }
int widen_i16(short value) { return value; }
unsigned int widen_u16(unsigned short value) { return value; }
unsigned int identity_u32(unsigned int value) { return value; }
float add_half_f32(float value) { return value + 0.5f; }
double add_half_f64(double value) { return value + 0.5; }
const char* get_text(void) { return "seen"; }
int check_text(const char* value) { return value && value[0] == 's' && value[3] == 'n'; }
signed char echo_i8(signed char value) { return value; }
unsigned char echo_u8(unsigned char value) { return value; }
short echo_i16(short value) { return value; }
unsigned short echo_u16(unsigned short value) { return value; }
_Bool echo_bool(_Bool value) { return value; }
SignedByteAlias echo_byte_alias(SignedByteAlias value) { return value; }
int widen_byte_alias(SignedByteAlias value) { return value; }
EOF
cat > "$TMP_DIR/Seen.toml" <<'EOF'
manifest-version = 1
[project]
name = "typedef_callback_probe"
version = "0.1.0"
modules = ["typedefs_bindings.seen"]
[native.dependencies]
typedef_callback = { path = "native/lib" }
EOF
timeout --foreground --kill-after=5s 120s clang -std=c11 -O2 -Wall -Wextra -Werror \
    -fPIC -shared "$TMP_DIR/native/callback.c" \
    -o "$TMP_DIR/native/lib/libtypedef_callback.so" > "$TMP_DIR/c-shim.log" 2>&1
bash "$ATTESTED_SEEN" "$COMPILER" check "$OUT" > "$TMP_DIR/check.log" 2>&1
for source in "$OUT" "$TMP_DIR/consumer.seen"; do
mode=$(basename "$source" .seen)
for profile in fast release; do
    profile="$mode-$profile"
    flags=(--fast)
    if [[ "$profile" = *-release ]]; then flags=(--release --lto=thin); fi
    (cd "$TMP_DIR"; bash "$ATTESTED_SEEN" "$COMPILER" compile \
        "$source" "$TMP_DIR/typedefs-$profile" --no-cache --target-cpu=x86-64 \
        --emit-module-ir-dir "$TMP_DIR/ir-$profile" "${flags[@]}") \
        > "$TMP_DIR/compile.log" 2>&1
    cp -- "$TMP_DIR/compile.log" "$TMP_DIR/$profile.compile.log"
    timeout --foreground --kill-after=5s 120s "$TMP_DIR/typedefs-$profile" \
        > "$TMP_DIR/$profile.run.log" 2>&1
    printf 'PASS: real C callback/scalar/pointer execution: %s\n' "$profile"
    grep -Eq '^declare ptr @get_callback\(\)' "$TMP_DIR/ir-$profile"/*.ll
    grep -Eq '^declare i32 @call_callback\(ptr[^,]*, i32[^)]*\)' "$TMP_DIR/ir-$profile"/*.ll
    grep -Eq '^declare i32 @widen_i8\(i8[^)]*\)' "$TMP_DIR/ir-$profile"/*.ll
    grep -Eq '^declare i32 @widen_i16\(i16[^)]*\)' "$TMP_DIR/ir-$profile"/*.ll
    grep -Eq '^declare float @add_half_f32\(float[^)]*\)' "$TMP_DIR/ir-$profile"/*.ll
    grep -Eq '^declare double @add_half_f64\(double[^)]*\)' "$TMP_DIR/ir-$profile"/*.ll
    # Raw *T parameters/returns retain the existing Seen i64 address ABI
    # (mapDeclaredTypeImpl), distinct from opaque Ptr/function values (ptr).
    # A C string must never be substituted with a two-word SeenString value.
    grep -Eq '^declare i64 @get_text\(\)' "$TMP_DIR/ir-$profile"/*.ll
    grep -Eq '^declare i32 @check_text\(i64[^)]*\)' "$TMP_DIR/ir-$profile"/*.ll
    if grep -Eq '(get_text|check_text)\([^)]*%SeenString|%SeenString @get_text' "$TMP_DIR/ir-$profile"/*.ll; then
        echo 'FAIL: a C string pointer was lowered as SeenString' >&2; exit 1
    fi
    for signature in \
        'i32 @widen_i8(i8 signext)' 'i32 @widen_u8(i8 zeroext)' \
        'i32 @widen_i16(i16 signext)' 'i32 @widen_u16(i16 zeroext)' \
        'signext i8 @echo_i8(i8 signext)' 'zeroext i8 @echo_u8(i8 zeroext)' \
        'signext i16 @echo_i16(i16 signext)' 'zeroext i16 @echo_u16(i16 zeroext)' \
        'zeroext i1 @echo_bool(i1 zeroext)' \
        'signext i8 @echo_byte_alias(i8 signext)'; do
        grep -Fq "declare $signature" "$TMP_DIR/ir-$profile"/*.ll
    done
    grep -Eq 'call i32 @widen_i8\(i8 signext ' "$TMP_DIR/ir-$profile"/*.ll
    grep -Eq 'call i32 @widen_u8\(i8 zeroext ' "$TMP_DIR/ir-$profile"/*.ll
    grep -Eq 'call signext i8 @echo_i8\(i8 signext ' "$TMP_DIR/ir-$profile"/*.ll
    if grep -Eq '(call|define|declare) (signext|zeroext) i8 @local_i8|@local_i8\(i8 (signext|zeroext)' "$TMP_DIR/ir-$profile"/*.ll; then
        echo 'FAIL: foreign scalar extension leaked into Seen internal ABI' >&2; exit 1
    fi
done
done

for target in linux-arm64 macos-arm64 windows-x86_64 linux-riscv64; do
    (cd "$TMP_DIR"; bash "$ATTESTED_SEEN" "$COMPILER" compile \
        "$OUT" "$TMP_DIR/$target-output" --fast --no-cache --target="$target" \
        --emit-module-ir-dir "$TMP_DIR/ir-$target" --stop-after-ir) \
        > "$TMP_DIR/compile.log" 2>&1
    cp -- "$TMP_DIR/compile.log" "$TMP_DIR/$target.compile.log"
    [ ! -e "$TMP_DIR/$target-output" ]
    for ir in "$TMP_DIR/ir-$target"/*.ll; do
        timeout --foreground --kill-after=5s 60s opt -passes=verify -disable-output "$ir"
    done
    case "$target" in
        macos-arm64)
            grep -Fq 'declare i32 @widen_i8(i8 signext)' "$TMP_DIR/ir-$target"/*.ll
            grep -Fq 'declare zeroext i8 @echo_u8(i8 zeroext)' "$TMP_DIR/ir-$target"/*.ll ;;
        linux-riscv64)
            grep -Fq 'declare signext i32 @widen_i8(i8 signext)' "$TMP_DIR/ir-$target"/*.ll
            grep -Eq 'declare signext i32 @identity_u32\(i32 signext\)' "$TMP_DIR/ir-$target"/*.ll
            grep -Eq 'call signext i32 @identity_u32\(i32 signext ' "$TMP_DIR/ir-$target"/*.ll ;;
        *)
            grep -Fq 'declare i32 @widen_i8(i8)' "$TMP_DIR/ir-$target"/*.ll
            grep -Fq 'declare i8 @echo_u8(i8)' "$TMP_DIR/ir-$target"/*.ll
            if grep -Eq '@(widen_i8|echo_u8)\(i8 (signext|zeroext)' "$TMP_DIR/ir-$target"/*.ll; then
                echo "FAIL: host scalar extension attributes leaked into $target" >&2; exit 1
            fi ;;
    esac
    printf 'PASS: target C scalar ABI raw IR (compile-only): %s\n' "$target"
done

for bad in unknown-parameter missing-arrow trailing-comma generic-pointer; do
    case "$bad" in
        unknown-parameter) type='fn(UndefinedCallbackArgument) -> Void'; diagnostic=E_TYPE_UNKNOWN ;;
        missing-arrow) type='fn(Int32) Int32'; diagnostic=E_TYPE_SYNTAX ;;
        trailing-comma) type='fn(Int32,) -> Void'; diagnostic=E_TYPE_SYNTAX ;;
        generic-pointer) type='Ptr<Int32>'; diagnostic=E_TYPE_ARITY ;;
    esac
    printf 'type RejectedCallback = %s\nfun main() r: Int { return 0 }\n' "$type" \
        > "$TMP_DIR/$bad.seen"
    status=0
    bash "$ATTESTED_SEEN" "$COMPILER" check "$TMP_DIR/$bad.seen" \
        > "$TMP_DIR/$bad.log" 2>&1 || status=$?
    [ "$status" -eq 1 ] || { echo "FAIL: $bad returned $status" >&2; exit 1; }
    grep -Fq "$diagnostic" "$TMP_DIR/$bad.log"
    grep -Eq ':[1-9][0-9]*:[1-9][0-9]*' "$TMP_DIR/$bad.log"
    if grep -Eq 'Optimization pipeline|Build succeeded|\[Pass 2\]' "$TMP_DIR/$bad.log"; then
        echo "FAIL: invalid callback entered code generation: $bad" >&2; exit 1
    fi
done

echo "PASS: import-c preserves typed aliases and callback ABI in fast/ThinLTO execution"
