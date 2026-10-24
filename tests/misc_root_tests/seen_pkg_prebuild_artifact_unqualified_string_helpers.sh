#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
CAPPED_ENTRY="$ROOT_DIR/scripts/run_capped_regression.sh"
SCOPE=seen-pkg-prebuild-unqualified-strings
if [ "${SEEN_CAPPED_REGRESSION_ACTIVE:-0}" != "1" ]; then
    exec bash "$CAPPED_ENTRY" "$SCOPE" --compiler "$COMPILER" -- \
        bash "$0" "$@"
fi
COMPILER="${SEEN_CAPPED_REGRESSION_COMPILER:-$COMPILER}"
bash "$CAPPED_ENTRY" --verify-active "$SCOPE" --compiler "$COMPILER"
ATTESTED_SEEN="${SEEN_ATTESTED_COMPILER_RUNNER:?}"
TMP_DIR="$(mktemp -d "$SEEN_ARTIFACT_ROOT/seen-pkg-prebuild-unqualified-strings.XXXXXX")"

cleanup() {
    if [ -z "${SEEN_KEEP_TMP:-}" ]; then
        case "$TMP_DIR" in
            "$SEEN_ARTIFACT_ROOT"/seen-pkg-prebuild-unqualified-strings.*)
                [ -d "$TMP_DIR" ] && [ ! -L "$TMP_DIR" ] &&
                    [ "$(dirname -- "$TMP_DIR")" = "$SEEN_ARTIFACT_ROOT" ] || return 1
                rm -rf -- "$TMP_DIR"
                ;;
            *) return 1 ;;
        esac
    else
        echo "KEEP: $TMP_DIR"
    fi
}

trap cleanup EXIT

mkdir -p "$TMP_DIR/facade/src" "$TMP_DIR/consumer/src"

cat >"$TMP_DIR/facade/Seen.toml" <<'EOF'
[project]
name = "facade_repro"
version = "0.1.0"
language = "en"
modules = ["src/string_helpers.seen"]
EOF

cat >"$TMP_DIR/facade/src/string_helpers.seen" <<'EOF'
pub fun reproAppendText(base: String, value: String) r: String {
    if value == "" {
        return base
    }
    if base == "" {
        return value
    }
    return base + "," + value
}

pub fun reproListText(values: Array<String>) r: String {
    var text = ""
    for i in 0..values.length() {
        text = reproAppendText(text, values.get(i))
    }
    return text
}
EOF

bash "$ATTESTED_SEEN" "$COMPILER" pkg prebuild \
    "$TMP_DIR/facade" "$TMP_DIR/build/facade" >/dev/null

cat >"$TMP_DIR/consumer/Seen.toml" <<EOF
[project]
name = "consumer_repro"
version = "0.1.0"
language = "en"
modules = ["src/use_dependency.seen"]

[dependencies]
facade_repro = { artifact = "$TMP_DIR/build/facade" }
EOF

cat >"$TMP_DIR/consumer/src/use_dependency.seen" <<'EOF'
pub fun reproConsumerJoin(values: Array<String>) r: String {
    var text = ""
    for i in 0..values.length() {
        text = reproAppendText(text, values.get(i))
    }
    return text
}
EOF

mkdir -p "$TMP_DIR/unrelated-cwd/src"
printf 'not the dependency source\n' > "$TMP_DIR/unrelated-cwd/src/string_helpers.seen"
(
    cd -- "$TMP_DIR/unrelated-cwd"
    bash "$ATTESTED_SEEN" "$COMPILER" pkg prebuild \
        "$TMP_DIR/consumer" "$TMP_DIR/build/consumer" >/dev/null
)
python3 - "$TMP_DIR/build/consumer" <<'PY'
import hashlib
from pathlib import Path, PurePosixPath
import sys
import tomllib
root = Path(sys.argv[1])
metadata = tomllib.loads((root / 'Seen.pkg.toml').read_text())
raw = (root / 'objects.tsv').read_bytes()
assert hashlib.sha256(raw).hexdigest() == metadata['object_manifest_sha256']
sources = []
for row in raw.decode().splitlines()[4:]:
    object_name, source_name, digest = row.split('\t')
    source = PurePosixPath(source_name)
    assert str(source) == source_name and not source.is_absolute()
    assert '..' not in source.parts and '\\' not in source_name
    assert hashlib.sha256((root / object_name).read_bytes()).hexdigest() == digest
    sources.append(source_name)
assert sorted(sources) == ['src/string_helpers.seen', 'src/use_dependency.seen'], sources
interface = root / 'src/__prebuilt/facade_repro/src/string_helpers.seen'
assert interface.read_bytes() == (root.parent / 'facade/src/string_helpers.seen').read_bytes()
index = (root / 'interface.index.tsv').read_text()
assert 'module\tsrc/__prebuilt/facade_repro/src/string_helpers.seen\t' in index
assert hashlib.sha256((root / 'interface.index.tsv').read_bytes()).hexdigest() == metadata['declaration_digest']
print('PASS: transitive artifact source provenance and object hashes survive unrelated caller cwd')
PY

# Lowercase unmodified declarations are module-private. This fixture tests an
# exported package API, so declare and verify its visibility explicitly.
grep -F $'function\tsrc/use_dependency.seen\tpublic\treproConsumerJoin\t' \
    "$TMP_DIR/build/consumer/interface.index.tsv" >/dev/null

mkdir -p "$TMP_DIR/client/src"
cat >"$TMP_DIR/client/Seen.toml" <<EOF
manifest-version = 1
[project]
name = "string_helper_client"
version = "0.1.0"
language = "en"

[dependencies]
consumer_repro = { artifact = "$TMP_DIR/build/consumer" }
EOF
cat >"$TMP_DIR/client/src/main.seen" <<'EOF'
import consumer_repro.use_dependency.{reproConsumerJoin}
extern "C" fun seen_string_release_owned(value: String)
fun main() r: Int {
    let values = ["first", "second", "third"]
    let joined = reproConsumerJoin(values)
    if joined != "first,second,third" { return 1 }
    seen_string_release_owned(joined)
    values.free()
    let empty = Array<String>()
    if reproConsumerJoin(empty) != "" { return 2 }
    empty.free()
    return 0
}
main()
EOF
bash "$ATTESTED_SEEN" "$COMPILER" pkg fetch "$TMP_DIR/client" \
    > "$TMP_DIR/client-fetch.log" 2>&1
test -s "$TMP_DIR/client/Seen.lock"
for profile in fast release; do
    flags=(--fast)
    if [ "$profile" = release ]; then flags=(--release --lto=thin); fi
    output="$TMP_DIR/client-$profile"
    bash "$ATTESTED_SEEN" "$COMPILER" compile "$TMP_DIR/client/src/main.seen" "$output" \
        --no-cache --frozen --target-cpu=x86-64 "${flags[@]}" > "$TMP_DIR/client-$profile.compile.log" 2>&1
    bash "$ROOT_DIR/scripts/check_x86_executable_baseline.sh" x86-64 "$output"
    timeout --foreground --kill-after=5s 120s "$output" > "$TMP_DIR/client-$profile.run.log" 2>&1
done

python3 - "$TMP_DIR" <<'PY'
import hashlib
from pathlib import Path
import re
import shutil
import sys
root = Path(sys.argv[1])
cases = {
    'dot': '.', 'dot-segment': './src/string_helpers.seen',
    'parent': 'src/../string_helpers.seen',
    'empty-segment': 'src//string_helpers.seen',
    'trailing-slash': 'src/string_helpers.seen/',
    'backslash': 'src\\string_helpers.seen',
    'absolute': '/outside/string_helpers.seen',
    'oversize': 'x' * 4097,
    'tab': 'src/string\thelper.seen',
}
for name, source in cases.items():
    artifact = root / ('bad-' + name)
    shutil.copytree(root / 'build/facade', artifact)
    manifest = artifact / 'objects.tsv'
    rows = manifest.read_text().splitlines()
    fields = rows[4].split('\t')
    fields[1] = source
    rows[4] = '\t'.join(fields)
    raw = ('\n'.join(rows) + '\n').encode()
    manifest.write_bytes(raw)
    metadata = artifact / 'Seen.pkg.toml'
    text, changed = re.subn(r'^object_manifest_sha256 = "[0-9a-f]{64}"$',
        'object_manifest_sha256 = "' + hashlib.sha256(raw).hexdigest() + '"',
        metadata.read_text(), flags=re.M)
    assert changed == 1
    metadata.write_text(text)
    project = root / ('rejected-' + name)
    (project / 'src').mkdir(parents=True)
    shutil.copy2(root / 'consumer/src/use_dependency.seen', project / 'src/use_dependency.seen')
    (project / 'Seen.toml').write_text(
        '[project]\nname = "rejected_provenance"\nversion = "0.1.0"\n'
        'modules = ["src/use_dependency.seen"]\n[dependencies]\n'
        'facade_repro = { artifact = "' + str(artifact) + '" }\n')
PY
for name in dot dot-segment parent empty-segment trailing-slash backslash absolute oversize tab; do
    status=0
    bash "$ATTESTED_SEEN" "$COMPILER" pkg prebuild "$TMP_DIR/rejected-$name" \
        "$TMP_DIR/rejected-output-$name" > "$TMP_DIR/rejected-$name.log" 2>&1 || status=$?
    test "$status" -eq 1
    grep -Fq 'has an unpinned or stale object manifest' "$TMP_DIR/rejected-$name.log"
    test ! -e "$TMP_DIR/rejected-output-$name/Seen.pkg.toml"
done

# An entry package cannot replace the dependency's pinned declaration module,
# nor redirect the copy through a symlink retained in its source tree.
for kind in collision symlink; do
    project="$TMP_DIR/interface-$kind"
    mkdir -p "$project/src" "$TMP_DIR/outside-$kind/facade_repro/src"
    cp -- "$TMP_DIR/consumer/Seen.toml" "$project/Seen.toml"
    cp -- "$TMP_DIR/consumer/src/use_dependency.seen" "$project/src/use_dependency.seen"
    if [ "$kind" = collision ]; then
        mkdir -p "$project/src/__prebuilt/facade_repro/src"
        printf '// entry package canary\n' > "$project/src/__prebuilt/facade_repro/src/string_helpers.seen"
    else
        ln -s -- "$TMP_DIR/outside-$kind" "$project/src/__prebuilt"
    fi
    printf 'outside canary\n' > "$TMP_DIR/outside-$kind/facade_repro/src/string_helpers.seen"
    status=0
    bash "$ATTESTED_SEEN" "$COMPILER" pkg prebuild "$project" \
        "$TMP_DIR/interface-output-$kind" > "$TMP_DIR/interface-$kind.log" 2>&1 || status=$?
    test "$status" -eq 1
    grep -Fq 'could not preserve dependency interface module' "$TMP_DIR/interface-$kind.log"
    test ! -e "$TMP_DIR/interface-output-$kind/Seen.pkg.toml"
    test "$(cat "$TMP_DIR/outside-$kind/facade_repro/src/string_helpers.seen")" = 'outside canary'
done

echo "PASS: prebuilt package artifacts preserve unqualified dependency String helper ABI"
