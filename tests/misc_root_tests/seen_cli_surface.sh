#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
COMPILER="${SEEN_BIN:-$ROOT_DIR/compiler_seen/target/seen}"
VERSION="${SEEN_EXPECTED_VERSION:-0.17.0}"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-cli-surface

fail() {
    echo "FAIL: $*" >&2
    exit 1
}

if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != "1" ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- \
        bash "$0" "$@"
fi
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
ARTIFACT_ROOT="$SEEN_ARTIFACT_ROOT"

TMP_DIR="$(mktemp -d "$ARTIFACT_ROOT/cli-surface.XXXXXX")"
cleanup() {
    case "$TMP_DIR" in
        "$ARTIFACT_ROOT"/cli-surface.*)
            [ -d "$TMP_DIR" ] && [ ! -L "$TMP_DIR" ] &&
                [ "$(dirname -- "$TMP_DIR")" = "$ARTIFACT_ROOT" ] || return 1
            rm -rf -- "$TMP_DIR"
            ;;
        *) echo "Refusing to remove unexpected test path: $TMP_DIR" >&2; return 1 ;;
    esac
}
trap cleanup EXIT

seen_command() {
    bash "$ATTESTED_SEEN" "$COMPILER" "$@"
}

deterministic_seen_command() {
    SEEN_DETERMINISTIC=1 SOURCE_DATE_EPOCH=1700000000 \
        SEEN_DETERMINISTIC_SEED=1101 SEEN_HASH_SEED=1101 \
        LANG=C.UTF-8 LC_ALL=C.UTF-8 TZ=UTC \
        bash "$ATTESTED_SEEN" "$COMPILER" "$@"
}

flag_only_deterministic_seen_command() {
    env -u SEEN_DETERMINISTIC -u SEEN_HASH_SEED \
        SOURCE_DATE_EPOCH=1700000000 SEEN_DETERMINISTIC_SEED=1101 \
        LANG=C.UTF-8 LC_ALL=C.UTF-8 TZ=UTC \
        bash "$ATTESTED_SEEN" "$COMPILER" "$@"
}

missing_epoch_seen_command() {
    env -u SOURCE_DATE_EPOCH -u SEEN_HASH_SEED \
        SEEN_DETERMINISTIC=1 SEEN_DETERMINISTIC_SEED=1101 \
        LANG=C.UTF-8 LC_ALL=C.UTF-8 TZ=UTC \
        bash "$ATTESTED_SEEN" "$COMPILER" "$@"
}

invalid_seed_seen_command() {
    SEEN_DETERMINISTIC=1 SOURCE_DATE_EPOCH=1700000000 \
        SEEN_DETERMINISTIC_SEED=not-a-seed \
        LANG=C.UTF-8 LC_ALL=C.UTF-8 TZ=UTC \
        bash "$ATTESTED_SEEN" "$COMPILER" "$@"
}

semantic_profile_seen_command() {
    env -u SEEN_DETERMINISTIC -u SOURCE_DATE_EPOCH \
        -u SEEN_DETERMINISTIC_SEED -u SEEN_HASH_SEED \
        LANG=C.UTF-8 LC_ALL=C.UTF-8 TZ=UTC \
        bash "$ATTESTED_SEEN" "$COMPILER" "$@"
}

invalid_worker_usage() {
    bash "$ATTESTED_SEEN" --usage-only-invalid-worker "$COMPILER" "$@"
}

expect_success_contains() {
    local label="$1"
    local expected="$2"
    shift 2

    local output
    if ! output="$("$@" 2>&1)"; then
        echo "$output" >&2
        fail "$label: expected success"
    fi
    if ! grep -Fq -- "$expected" <<<"$output"; then
        echo "$output" >&2
        fail "$label: expected output to contain '$expected'"
    fi
}

expect_success() {
    local label="$1"
    shift

    local output
    if ! output="$("$@" 2>&1)"; then
        echo "$output" >&2
        fail "$label: expected success"
    fi
}

expect_failure_contains() {
    local label="$1"
    local expected="$2"
    shift 2

    local output
    set +e
    output="$("$@" 2>&1)"
    local status=$?
    set -e

    if [[ "$status" -eq 0 ]]; then
        echo "$output" >&2
        fail "$label: expected failure"
    fi
    if ! grep -Fq -- "$expected" <<<"$output"; then
        echo "$output" >&2
        fail "$label: expected output to contain '$expected'"
    fi
}

expect_usage_failure_contains() {
    local label="$1"
    local expected="$2"
    shift 2

    local output
    set +e
    output="$("$@" 2>&1)"
    local status=$?
    set -e

    if [[ "$status" -ne 1 ]]; then
        echo "$output" >&2
        fail "$label: expected compiler usage exit code 1, got $status"
    fi
    if ! grep -Fq -- "$expected" <<<"$output"; then
        echo "$output" >&2
        fail "$label: expected output to contain '$expected'"
    fi
}

expect_exit_one_contains() {
    local label="$1"
    local expected="$2"
    shift 2

    local output
    set +e
    output="$("$@" 2>&1)"
    local status=$?
    set -e

    if [[ "$status" -ne 1 ]]; then
        echo "$output" >&2
        fail "$label: expected exit code 1, got $status"
    fi
    if ! grep -Fq -- "$expected" <<<"$output"; then
        echo "$output" >&2
        fail "$label: expected output to contain '$expected'"
    fi
}

expect_stable_usage_failure_contains() {
    local label="$1"
    local expected="$2"
    shift 2

    local first second first_status second_status
    set +e
    first="$("$@" 2>&1)"
    first_status=$?
    second="$("$@" 2>&1)"
    second_status=$?
    set -e

    if [[ "$first_status" -ne 1 || "$second_status" -ne 1 ]]; then
        echo "$first" >&2
        echo "$second" >&2
        fail "$label: expected repeatable compiler usage exit code 1"
    fi
    if [[ "$first" != "$second" ]]; then
        echo "$first" >&2
        echo "$second" >&2
        fail "$label: diagnostic output changed between identical invocations"
    fi
    if ! grep -Fq -- "$expected" <<<"$first"; then
        echo "$first" >&2
        fail "$label: expected output to contain '$expected'"
    fi
}

# Keep deliberately unsupported CLI probes inside the disposable tree in case
# an older binary recognizes one of those commands.
cd -- "$TMP_DIR"

expect_success_contains "--version" "Seen $VERSION" seen_command --version
expect_success_contains "-v" "Seen $VERSION" seen_command -v
expect_success_contains "--help" "seen compile" seen_command --help
expect_success_contains "-h" "seen compile" seen_command -h

help_output="$(seen_command --help 2>&1)"
if grep -Fq "seen build <" <<<"$help_output"; then
    echo "$help_output" >&2
    fail "--help advertised legacy seen build usage"
fi

expect_failure_contains "build unsupported" "not supported by the shipped compiler" seen_command build
expect_failure_contains "init unsupported" "not supported by the shipped compiler" seen_command init demo
expect_failure_contains "fmt unsupported" "not supported by the shipped compiler" seen_command fmt file.seen
expect_failure_contains "format unsupported" "not supported by the shipped compiler" seen_command format file.seen
expect_failure_contains "clean unsupported" "not supported by the shipped compiler" seen_command clean
expect_failure_contains "test missing project" "test command is missing a required operand" seen_command test
expect_failure_contains "c backend unsupported" "Supported backend: llvm" seen_command compile file.seen out --backend=c

for command in compile check run test bundle sign notarize lipo ipa translate import-c lsp pkg; do
    expect_success_contains "$command --help" "Usage: seen $command" seen_command "$command" --help
    expect_success_contains "$command -h" "Usage: seen $command" seen_command "$command" -h
done

expect_success_contains "CORE-004C_installed_help compile" "--deterministic" \
    seen_command compile --help
expect_success_contains "CORE-004C_installed_help check" "--deterministic" \
    seen_command check --help
expect_success_contains "CORE-004C_installed_help run" "--deterministic" \
    seen_command run --help

expect_usage_failure_contains "unknown global option" "unknown global option '--wat'" seen_command --wat
expect_usage_failure_contains "unknown compile option" "unknown option '--wat'" seen_command compile file.seen --wat
expect_usage_failure_contains "missing option value" "option '--profile' requires a value" seen_command compile file.seen --profile
expect_usage_failure_contains "CORE-004C_missing_value" "core.004c.invalid" \
    seen_command check file.seen --profile
expect_stable_usage_failure_contains "CORE-004C_unknown_profile" "core.004c.invalid" \
    seen_command check file.seen --profile unknown
expect_stable_usage_failure_contains "CORE-004C_profile_conflict" "core.004c.conflict" \
    seen_command compile file.seen --deterministic --profile default
expect_usage_failure_contains "CORE-004C_check_profile_conflict" "core.004c.conflict" \
    seen_command check file.seen --deterministic --profile default
expect_stable_usage_failure_contains "CORE-004C_simd_conflict" "core.004c.conflict" \
    seen_command compile file.seen --deterministic --simd avx2
expect_usage_failure_contains "CORE-004C_deterministic_value_rejected" "core.004c.invalid" \
    seen_command run file.seen --deterministic=true
expect_usage_failure_contains "conflicting repeated option" "conflicting values for option '--backend'" seen_command compile file.seen --backend llvm --backend c
expect_usage_failure_contains "conflicting PGO modes" "conflicting options '--pgo-generate' and '--pgo-use'" seen_command compile file.seen --pgo-generate --pgo-use profile.profdata
expect_usage_failure_contains "invalid target CPU" "invalid value 'not-a-cpu' for --target-cpu" seen_command compile file.seen --target-cpu not-a-cpu
expect_usage_failure_contains "invalid SIMD policy" "invalid value 'not-a-policy' for --simd" seen_command compile file.seen --simd not-a-policy
expect_usage_failure_contains "invalid sanitizer" "invalid value 'not-a-sanitizer' for --sanitize" seen_command compile file.seen --sanitize not-a-sanitizer
expect_usage_failure_contains "invalid target" "invalid value 'not-a-target' for --target" seen_command compile file.seen --target not-a-target
expect_usage_failure_contains "invalid jobs suffix" "--jobs requires a positive integer" invalid_worker_usage compile file.seen --jobs 1junk
expect_usage_failure_contains "invalid optimizer jobs suffix" "--opt-jobs requires a positive integer" invalid_worker_usage compile file.seen --opt-jobs 2workers
expect_usage_failure_contains "overflowing project prefix" "--projectprefix requires a positive integer" seen_command compile file.seen --projectprefix 9223372036854775808
expect_usage_failure_contains "extra check operand" "unexpected extra operand 'other.seen'" seen_command check file.seen other.seen
expect_usage_failure_contains "extra version operand" "unexpected extra operand 'extra'" seen_command --version extra
expect_usage_failure_contains "help with extra operand" "unknown option '--help'" seen_command compile --help junk
expect_success_contains "pkg prebuild --help" "Usage: seen pkg prebuild" seen_command pkg prebuild --help
expect_success_contains "pkg prebuild -h" "Usage: seen pkg prebuild" seen_command pkg prebuild -h
expect_usage_failure_contains "pkg prebuild unknown option" "unknown option '--wat'" seen_command pkg prebuild --wat
expect_usage_failure_contains "pkg prebuild help with extra operand" "unknown option '--help'" seen_command pkg prebuild --help extra
expect_usage_failure_contains "pkg prebuild extra operand" "unexpected extra operand 'third'" seen_command pkg prebuild first second third
expect_usage_failure_contains "CORE-004C_stale_path_rejected" "core.004c.invalid" \
    seen_command determinism file.seen

CORE_004C_FIXTURES="$ROOT_DIR/tests/fixtures/core-004c"
CORE_004C_OK="$CORE_004C_FIXTURES/deterministic_ok.seen"
CORE_004C_REJECT="$CORE_004C_FIXTURES/nondeterministic_hashmap.seen"

expect_success_contains "CORE-004C_profile_happy" "[OK] Check passed" \
    seen_command check "$CORE_004C_OK" --profile deterministic
expect_success_contains "CORE-004C_alias_happy" "[OK] Check passed" \
    deterministic_seen_command check "$CORE_004C_OK" --deterministic
expect_success_contains "CORE-004E_flag_only_reexec" "[OK] Check passed" \
    flag_only_deterministic_seen_command check "$CORE_004C_OK" --deterministic
expect_success_contains "CORE-004C_alias_explicit_profile_happy" "[OK] Check passed" \
    deterministic_seen_command check "$CORE_004C_OK" --deterministic \
    --profile deterministic

expect_exit_one_contains "CORE-004E_missing_epoch" "core.004e.missing_epoch" \
    missing_epoch_seen_command compile "$CORE_004C_OK" \
    "$TMP_DIR/core-004e-missing-epoch" --deterministic
expect_exit_one_contains "CORE-004E_invalid_seed" "core.004e.invalid_seed" \
    invalid_seed_seen_command compile "$CORE_004C_OK" \
    "$TMP_DIR/core-004e-invalid-seed" --deterministic
expect_exit_one_contains "CORE-004C_semantic_profile_only" "Determinism Error:" \
    semantic_profile_seen_command compile "$CORE_004C_REJECT" \
    "$TMP_DIR/core-004c-semantic-profile" --profile deterministic

expect_exit_one_contains "CORE-004C_compile_check_run_parity compile" \
    "Determinism Error:" deterministic_seen_command compile "$CORE_004C_REJECT" \
    "$TMP_DIR/core-004c-rejected" --deterministic
expect_exit_one_contains "CORE-004C_compile_check_run_parity check" \
    "Determinism Error:" deterministic_seen_command check "$CORE_004C_REJECT" \
    --deterministic
expect_exit_one_contains "CORE-004C_compile_check_run_parity run" \
    "Determinism Error:" deterministic_seen_command run "$CORE_004C_REJECT" \
    --deterministic

expect_success_contains "CORE-004C_alias_happy compile" \
    "Architecture: target-cpu=x86-64" deterministic_seen_command compile "$CORE_004C_OK" \
    "$TMP_DIR/core-004c-compiled" --deterministic --profile deterministic \
    --simd none --no-cache --no-fork --jobs 1 --opt-jobs 1
expect_success "CORE-004C_alias_happy run" \
    deterministic_seen_command run "$CORE_004C_OK" --deterministic --no-cache

mkdir -p "$TMP_DIR/work"
printf 'manifest-version = 1\n\n[project]\nname = "cli_fixture"\nversion = "0.1.0"\nlanguage = "en"\n\n[dependencies]\n' > \
    "$TMP_DIR/work/Seen.toml"
printf 'fun main() r: Int { println("hello")\n let assert = 1\n return assert\n}\n' > "$TMP_DIR/work/input.seen"
printf 'fun main() r: Int { return 0 }\n' > "$TMP_DIR/work/--input.seen"

expect_success_contains "check dash path" "[OK] Check passed" \
    bash -c 'cd "$1" && bash "$2" "$3" check -- --input.seen' _ \
    "$TMP_DIR/work" "$ATTESTED_SEEN" "$COMPILER"

# Omission discovers the enclosing project, while every explicit operand must
# name that project directory or its Seen.toml exactly. These fixtures stop
# before compilation because the enclosing manifest deliberately has no entry.
expect_failure_contains "pkg prebuild omitted project" \
    "package project needs [build].entry" \
    bash -c 'cd "$1" && bash "$2" "$3" pkg prebuild' _ \
    "$TMP_DIR/work" "$ATTESTED_SEEN" "$COMPILER"
expect_failure_contains "pkg prebuild explicit project directory" \
    "package project needs [build].entry" \
    seen_command pkg prebuild "$TMP_DIR/work"
expect_failure_contains "pkg prebuild explicit project manifest" \
    "package project needs [build].entry" \
    seen_command pkg prebuild "$TMP_DIR/work/Seen.toml"
expect_failure_contains "pkg prebuild explicit empty project" \
    "could not find Seen.toml for pkg prebuild" \
    bash -c 'cd "$1" && bash "$2" "$3" pkg prebuild ""' _ \
    "$TMP_DIR/work" "$ATTESTED_SEEN" "$COMPILER"

# The missing dash-prefixed operand is deliberately below a valid enclosing
# manifest. An explicit project operand must not fall back to that ancestor.
expect_failure_contains "pkg prebuild dash path" "could not find Seen.toml for pkg prebuild" \
    bash -c 'cd "$1" && bash "$2" "$3" pkg prebuild -- --missing-project' _ \
    "$TMP_DIR/work" "$ATTESTED_SEEN" "$COMPILER"

mkdir "$TMP_DIR/work/not-a-project"
expect_failure_contains "pkg prebuild explicit directory without manifest" \
    "could not find Seen.toml for pkg prebuild" \
    seen_command pkg prebuild "$TMP_DIR/work/not-a-project"
expect_failure_contains "pkg prebuild explicit non-manifest file" \
    "could not find Seen.toml for pkg prebuild" \
    seen_command pkg prebuild "$TMP_DIR/work/input.seen"

arbitrary_output="$(cd "$TMP_DIR/work" && env -u SEEN_DATA_PATH \
    bash "$ATTESTED_SEEN" "$COMPILER" translate input.seen --from en --to es 2>&1)" || {
    echo "$arbitrary_output" >&2
    fail "translation from arbitrary CWD failed"
}
if ! grep -Fq "función" <<<"$arbitrary_output"; then
    echo "$arbitrary_output" >&2
    fail "translation from arbitrary CWD did not load executable-relative packs"
fi
if ! grep -Fq "assert" <<<"$arbitrary_output" || grep -Fq "afirmar" <<<"$arbitrary_output"; then
    echo "$arbitrary_output" >&2
    fail "translation rewrote the inactive KeywordAssert spelling"
fi

dash_output="$(cd "$TMP_DIR/work" && seen_command translate --from en --to es -- --input.seen 2>&1)" || {
    echo "$dash_output" >&2
    fail "-- source path translation failed"
}
if ! grep -Fq "función" <<<"$dash_output"; then
    echo "$dash_output" >&2
    fail "-- source path was not treated as an operand"
fi

if ! (cd "$TMP_DIR/work" && seen_command translate input.seen --from en \
    --to es --output=--translated.seen >/dev/null 2>&1); then

    fail "dash-prefixed translation output path failed"
fi
if [[ ! -f "$TMP_DIR/work/--translated.seen" ]]; then
    fail "dash-prefixed translation output was not created"
fi

: > "$TMP_DIR/work/empty.seen"
if ! seen_command translate "$TMP_DIR/work/empty.seen" --from en --to es -o "$TMP_DIR/work/empty.es.seen" >/dev/null 2>&1; then
    fail "empty readable source was rejected"
fi
if [[ ! -f "$TMP_DIR/work/empty.es.seen" || -s "$TMP_DIR/work/empty.es.seen" ]]; then
    fail "empty source translation did not create an empty output"
fi

expect_failure_contains "translation read error" "Could not read file" seen_command translate "$TMP_DIR/work/missing.seen" --from en --to es
expect_failure_contains "translation write error" "Could not write output file" seen_command translate "$TMP_DIR/work/input.seen" --from en --to es -o "$TMP_DIR/missing/output.seen"

mkdir "$TMP_DIR/work/existing-directory"
printf 'sentinel\n' > "$TMP_DIR/work/existing-directory/sentinel"
expect_failure_contains "atomic replace error" "Could not write output file" seen_command translate "$TMP_DIR/work/input.seen" --from en --to es -o "$TMP_DIR/work/existing-directory"
if [[ ! -f "$TMP_DIR/work/existing-directory/sentinel" ]]; then
    fail "failed atomic replacement changed the destination directory"
fi
if compgen -G "$TMP_DIR/work/existing-directory.tmp.*" >/dev/null; then
    fail "failed atomic replacement left a temporary file"
fi
if find "$TMP_DIR/work" -maxdepth 1 -name '.seen-tmp.*' -print -quit | grep -q .; then
    fail "failed atomic replacement left a native temporary file"
fi

mkdir -p "$TMP_DIR/partial/languages/en"
cp "$ROOT_DIR/languages/en/en-keywords-control.toml" "$TMP_DIR/partial/languages/en/"
expect_failure_contains "incomplete configured language pack" "missing or incomplete language pack 'en'" env SEEN_DATA_PATH="$TMP_DIR/partial" bash "$ATTESTED_SEEN" "$COMPILER" translate "$TMP_DIR/work/input.seen" --from en --to es

mkdir -p "$TMP_DIR/empty-pack"
cp -R "$ROOT_DIR/languages" "$TMP_DIR/empty-pack/languages"
: > "$TMP_DIR/empty-pack/languages/en/en-keywords-control.toml"
expect_failure_contains "empty configured language pack file" "missing or incomplete language pack 'en'" env SEEN_DATA_PATH="$TMP_DIR/empty-pack" bash "$ATTESTED_SEEN" "$COMPILER" translate "$TMP_DIR/work/input.seen" --from en --to es

mkdir -p "$TMP_DIR/malformed-pack"
cp -R "$ROOT_DIR/languages" "$TMP_DIR/malformed-pack/languages"
printf '[unrelated]\n"fun" = "KeywordFun"\n' > \
    "$TMP_DIR/malformed-pack/languages/en/en-keywords-control.toml"
expect_failure_contains "malformed configured language pack table" \
    "missing or incomplete language pack 'en'" env \
    SEEN_DATA_PATH="$TMP_DIR/malformed-pack" bash "$ATTESTED_SEEN" "$COMPILER" translate \
    "$TMP_DIR/work/input.seen" --from en --to es

echo "CLI surface checks passed"
