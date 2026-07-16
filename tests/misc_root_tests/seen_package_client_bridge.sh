#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TMP_DIR="$(mktemp -d /tmp/seen_package_bridge.XXXXXX)"

cleanup() {
    rm -rf "$TMP_DIR"
}
trap cleanup EXIT

cat > "$TMP_DIR/request.seenpkg" <<'REQUEST_EOF'
SEENPKG1
1
7
whoami
REQUEST_EOF

cat > "$TMP_DIR/helper" <<'HELPER_EOF'
#!/usr/bin/env sh
set -eu
test "$#" -eq 2
test "$1" = "--request"
test -f "$2"
case "$2" in
    "${SEEN_BRIDGE_UNSAFE_TMPDIR:-}"/*)
        exit 24
        ;;
    /proc/self/fd/*|/dev/fd/*)
        ;;
    *)
        exit 25
        ;;
esac
grep -qx 'whoami' "$2"
exit 23
HELPER_EOF
chmod 755 "$TMP_DIR/helper"

cat > "$TMP_DIR/bridge_test.c" <<'C_EOF'
#include "seen_runtime.h"

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

extern int64_t __ExecuteProgram(SeenString path);
extern SeenString __CurrentExecutablePath(void);
extern SeenString __CurrentWorkingDirectory(void);
extern CommandResult* __ExecuteCommand(SeenString command);

static SeenString seen_string(char *value) {
    SeenString result;
    result.len = (int64_t)strlen(value);
    result.data = value;
    return result;
}

int main(int argc, char **argv) {
    if (argc != 5) return 2;
    SeenString executable = __CurrentExecutablePath();
    if (executable.len <= 0 || executable.data == NULL) {
        fputs("current executable path was empty\n", stderr);
        return 3;
    }
    SeenString working_directory = __CurrentWorkingDirectory();
    if (working_directory.len <= 0 || working_directory.data == NULL) {
        fputs("current working directory was empty\n", stderr);
        return 5;
    }
    CommandResult *framed_directory = __ExecuteCommand(
        seen_string("SEEN_CURRENT_DIRECTORY_V1"));
    if (!framed_directory || !framed_directory->success ||
        framed_directory->output.len != working_directory.len ||
        memcmp(framed_directory->output.data, working_directory.data,
            (size_t)working_directory.len) != 0) {
        fputs("native current-directory frame mismatch\n", stderr);
        return 11;
    }

    static const char canonical_prefix[] = "SEEN_CANONICAL_PATH_V1_";
    static const char hex[] = "0123456789abcdef";
    size_t input_path_length = strlen(argv[2]);
    size_t canonical_frame_length = strlen(canonical_prefix) +
        input_path_length * 2;
    char *canonical_frame = malloc(canonical_frame_length + 1);
    if (!canonical_frame) return 12;
    memcpy(canonical_frame, canonical_prefix, strlen(canonical_prefix));
    size_t canonical_cursor = strlen(canonical_prefix);
    for (size_t index = 0; index < input_path_length; index++) {
        unsigned char value = (unsigned char)argv[2][index];
        canonical_frame[canonical_cursor++] = hex[value >> 4];
        canonical_frame[canonical_cursor++] = hex[value & 15];
    }
    canonical_frame[canonical_cursor] = '\0';
    CommandResult *canonical_result = __ExecuteCommand(
        seen_string(canonical_frame));
    free(canonical_frame);
    if (!canonical_result || !canonical_result->success ||
        canonical_result->output.len != (int64_t)strlen(argv[3]) ||
        memcmp(canonical_result->output.data, argv[3], strlen(argv[3])) != 0) {
        fputs("native canonical-path frame did not resolve symlink\n", stderr);
        return 13;
    }

    size_t malicious_length = strlen(canonical_prefix) + strlen(argv[4]) + 16;
    char *malicious_frame = malloc(malicious_length);
    if (!malicious_frame) return 14;
    snprintf(malicious_frame, malicious_length, "%szz;touch %s",
        canonical_prefix, argv[4]);
    CommandResult *malicious_result = __ExecuteCommand(
        seen_string(malicious_frame));
    free(malicious_frame);
    if (!malicious_result || malicious_result->success) {
        fputs("malformed canonical-path frame unexpectedly succeeded\n", stderr);
        return 15;
    }
    FILE *request = fopen(argv[2], "rb");
    if (!request) return 6;
    if (fseek(request, 0, SEEK_END) != 0) return 7;
    long request_length = ftell(request);
    if (request_length <= 0 || fseek(request, 0, SEEK_SET) != 0) return 8;
    const char *prefix = "SEEN_EXACT_REQUEST_V1\n";
    size_t helper_length = strlen(argv[1]);
    char helper_length_text[32];
    int helper_length_digits = snprintf(helper_length_text,
        sizeof(helper_length_text), "%zu\n", helper_length);
    size_t frame_length = strlen(prefix) + (size_t)helper_length_digits +
        helper_length + (size_t)request_length;
    char *frame = malloc(frame_length);
    if (!frame) return 9;
    size_t cursor = 0;
    memcpy(frame + cursor, prefix, strlen(prefix)); cursor += strlen(prefix);
    memcpy(frame + cursor, helper_length_text, (size_t)helper_length_digits);
    cursor += (size_t)helper_length_digits;
    memcpy(frame + cursor, argv[1], helper_length); cursor += helper_length;
    if (fread(frame + cursor, 1, (size_t)request_length, request) !=
        (size_t)request_length) return 10;
    fclose(request);
    SeenString framed_request = {(int64_t)frame_length, frame};
    int64_t status = __ExecuteProgram(framed_request);
    free(frame);
    if (status != 23) {
        fprintf(stderr, "helper exit status: %lld\n", (long long)status);
        return 4;
    }
    return 0;
}
C_EOF

: "${SEEN_TEST_VMEM_KB:=2097152}"
(
    ulimit -v "$SEEN_TEST_VMEM_KB"
    clang -std=c11 -O1 -I "$ROOT_DIR/seen_runtime" \
        "$TMP_DIR/bridge_test.c" "$ROOT_DIR/seen_runtime/seen_runtime.c" \
        -lm -lpthread -ldl -o "$TMP_DIR/bridge_test"
)

mkdir "$TMP_DIR/unsafe-tmp" "$TMP_DIR/real-project"
chmod 0777 "$TMP_DIR/unsafe-tmp"
mv "$TMP_DIR/request.seenpkg" "$TMP_DIR/real-project/request.seenpkg"
ln -s "$TMP_DIR/real-project" "$TMP_DIR/link-project"
TMPDIR="$TMP_DIR/unsafe-tmp" \
    SEEN_BRIDGE_UNSAFE_TMPDIR="$TMP_DIR/unsafe-tmp" \
    "$TMP_DIR/bridge_test" "$TMP_DIR/helper" \
    "$TMP_DIR/link-project/request.seenpkg" \
    "$TMP_DIR/real-project/request.seenpkg" "$TMP_DIR/injected-marker"
if [[ -e "$TMP_DIR/injected-marker" ]]; then
    echo "malformed canonical-path frame reached a shell" >&2
    exit 1
fi

python3 - "$ROOT_DIR/compiler_seen/src/main_compiler.seen" \
    "$ROOT_DIR/seen_runtime/seen_runtime.c" <<'PY'
import pathlib
import sys

compiler = pathlib.Path(sys.argv[1]).read_text(encoding="utf-8")
runtime = pathlib.Path(sys.argv[2]).read_text(encoding="utf-8")


def function_body(source: str, name: str, next_name: str) -> str:
    start_marker = f"fun {name}("
    end_marker = f"fun {next_name}("
    start = source.index(start_marker)
    end = source.index(end_marker, start + len(start_marker))
    return source[start:end]


jit = function_body(compiler, "jitRunCommand", "aotRunCommand")
prepare = jit.index("ensureProjectPackageDependenciesForInput(jitInput, true)")
collect = jit.index("collectModulePaths(jitInput")
assert prepare < collect, "default JIT run must prepare packages before module collection"

run_dispatch = function_body(compiler, "runScriptCommand", "seenCompilerVersion")
assert "return jitRunCommand(" in run_dispatch, "default run must dispatch through audited JIT path"

mode_forwarding = function_body(
    compiler, "appendPackageResolutionModeArgs", "isHttpRegistryLocation"
)
assert 'requestArgs.push("--frozen")' in mode_forwarding
assert mode_forwarding.index('if g_packageFrozenCli == 1') < mode_forwarding.index(
    'requestArgs.push("--frozen")'
)

assert "GetTempFileNameA" not in runtime, "Windows request path must not use close-and-reopen temp files"
for required in (
    "CreateDirectoryW",
    "CreateFileW",
    "CREATE_NEW",
    "FILE_FLAG_OPEN_REPARSE_POINT",
    "SystemFunction036",
    "SetSecurityDescriptorDacl",
    "SE_DACL_PROTECTED",
    "_wspawnv",
):
    assert required in runtime, f"missing Windows request hardening primitive: {required}"

assert 'libs = " -ladvapi32"' in compiler, "Windows runtime ACL APIs must be linked"

resolver = function_body(
    compiler, "resolvePackageDependencyProjectRoot", "findNearestSeenPkgTomlFromDir"
)
assert "ensureRegistryPackageInstalled" not in resolver
assert "resolveExplicitPackageDependencyProjectRoot" in resolver
for required in (
    "SEEN_CURRENT_DIRECTORY_V1",
    "SEEN_CANONICAL_PATH_V1_",
    "pathUsesWindowsComparison",
    "pathByteIsSeparator",
):
    assert required in compiler, f"missing package-map path hardening: {required}"
PY

echo "PASS: package bridge and native path frames avoid shell interpretation"
