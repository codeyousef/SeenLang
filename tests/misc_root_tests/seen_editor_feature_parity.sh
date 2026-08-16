#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

case "${SEEN_MAIN_VMEM_KB:-}" in
    ''|*[!0-9]*|0)
        echo "SEEN_MAIN_VMEM_KB must be a positive explicit memory cap" >&2
        exit 2
        ;;
esac
case "${SEEN_OPT_VMEM_KB:-}" in
    ''|*[!0-9]*|0)
        echo "SEEN_OPT_VMEM_KB must be a positive explicit optimizer cap" >&2
        exit 2
        ;;
esac
ulimit -v "$SEEN_MAIN_VMEM_KB"

python3 - "$ROOT_DIR" <<'PY'
import json
import pathlib
import re
import sys

root = pathlib.Path(sys.argv[1])
expected_languages = ["en", "ar", "es", "ru", "zh", "ja"]

def read(path):
    return (root / path).read_text(encoding="utf-8")

def fail(message):
    raise SystemExit(message)

package = json.loads(read("vscode-seen/package.json"))
package_lock = json.loads(read("vscode-seen/package-lock.json"))
package_version = package["version"]
if package_lock["version"] != package_version:
    fail(f"VS Code package/lock version drifted: {package_version} != {package_lock['version']}")
if package_lock["packages"][""]["version"] != package_version:
    fail("VS Code root lock package version drifted from package.json")
if f"## [{package_version}]" not in read("vscode-seen/CHANGELOG.md"):
    fail(f"VS Code changelog has no entry for package version {package_version}")

language_enum = package["contributes"]["configuration"]["properties"]["seen.language.default"]["enum"]
if language_enum != expected_languages:
    fail(f"seen.language.default enum drifted: {language_enum}")

commands = {entry["command"] for entry in package["contributes"]["commands"]}
for command in ["seen.pkgFetch", "seen.pkgPack", "seen.pkgPrebuild", "seen.pkgPublish"]:
    if command not in commands:
        fail(f"missing VS Code command {command}")

task_enum = package["contributes"]["taskDefinitions"][0]["properties"]["task"]["enum"]
expected_tasks = [
    "compile", "run", "check", "compile-shared",
    "pkg-fetch", "pkg-pack", "pkg-prebuild", "pkg-publish",
]
if task_enum != expected_tasks:
    fail(f"VS Code task definitions advertise unsupported or missing operations: {task_enum}")

commands_source = read("vscode-seen/src/commands.ts")
for unsupported in ["seen/switchLanguage", "seen/translate", "seen/getStreamInfo"]:
    if unsupported in commands_source:
        fail(f"extension still calls unsupported custom LSP method {unsupported}")
if "French (fr)" in commands_source:
    fail("extension language picker still advertises French")
if "Japanese (ja)" not in commands_source:
    fail("extension language picker does not advertise Japanese")
if "editor.action.formatDocument" not in commands_source:
    fail("seen.format does not route through the configured document formatter")
for forbidden in [
    "['build'", "['test'", "['clean'", "['init'", "['benchmark'", "['repl'",
    "runBenchmarks(", "shellArgs: ['repl']",
]:
    if forbidden in commands_source:
        fail(f"extension still invokes an unsupported compiler subcommand: {forbidden}")
for warning in [
    "seen test is not supported", "seen clean is not supported", "seen init is not supported",
    "seen benchmark is not supported", "seen repl is not supported",
]:
    if warning not in commands_source:
        fail(f"extension does not fail closed with a clear warning: {warning}")
if "'Seen Compile', 'compile', ['compile'" not in commands_source:
    fail("seen.build compatibility command does not map to seen compile")
for task in expected_tasks:
    if task not in commands_source:
        fail(f"extension source has no implementation for advertised task {task}")

repl_source = read("vscode-seen/src/repl.ts")
for forbidden in ["shellArgs: ['repl']", "createTerminal("]:
    if forbidden in repl_source:
        fail(f"REPL integration still launches an unsupported subcommand: {forbidden}")
if "seen repl is not supported" not in repl_source:
    fail("REPL compatibility commands do not explain that the compiler has no REPL")

extension_source = read("vscode-seen/src/extension.ts")
if "sendRequest<any[]>('textDocument/formatting'" not in extension_source:
    fail("VS Code formatter middleware does not forward Seen-specific LSP options")
if "next(document, protocolOptions" in extension_source:
    fail("VS Code formatter relies on a converter that strips Seen-specific LSP options")

format_properties = package["contributes"]["configuration"]["properties"]
for setting in ["seen.formatting.enable", "seen.formatting.sortImports", "seen.formatting.useManifest"]:
    if setting not in format_properties:
        fail(f"missing VS Code formatter setting {setting}")
default_formatter = package["contributes"]["configurationDefaults"]["[seen]"]["editor.defaultFormatter"]
extension_id = package["publisher"] + "." + package["name"]
if default_formatter != extension_id:
    fail(f"Seen default formatter drifted: {default_formatter} != {extension_id}")
extension_tests = read("vscode-seen/src/test/suite/extension.test.ts")
if "seen-lang.seen" in extension_tests:
    fail("VS Code tests still use the obsolete extension identifier")
if extension_id not in extension_tests:
    fail("VS Code tests do not use the manifest-derived extension identifier")

diagnostics_source = read("vscode-seen/src/errorDiagnostics.ts")
if "--json-errors" in diagnostics_source:
    fail("standalone diagnostics still use unsupported --json-errors")
for expected in ["stdout.on", "stderr.on", "mkdtemp", ".seen", "agent-tools", "vscode-check"]:
    if expected not in diagnostics_source:
        fail(f"standalone diagnostics missing {expected}")
for forbidden in ["os.tmpdir()", "import * as os from 'os'"]:
    if forbidden in diagnostics_source:
        fail(f"standalone diagnostics still uses host temporary storage: {forbidden}")

grammar = read("vscode-seen/syntaxes/seen.tmLanguage.json")
for token in ["using", "operator", "effect", "sealed", "package"]:
    if token not in grammar:
        fail(f"grammar missing token {token}")
if '"begin": "^\\\\s*///\\\\s*$"' not in grammar:
    fail("grammar must keep standalone /// block comments")

snippets = read("vscode-seen/snippets/seen.code-snippets")
for token in ["effect(", "@using", ".callInt(", ".callIntPtr(", "package_name", "sealed class", " in "]:
    if token not in snippets:
        fail(f"snippets missing {token}")
if "callHotReloadInt" in snippets:
    fail("snippets still reference stale callHotReloadInt helper")

server = read("compiler_seen/src/lsp/server.seen")
if 'run_frontend(content, uri, "en")' in server:
    fail("LSP still hardcodes English frontend parsing")
if "missing `main` function" in server:
    fail("LSP still injects missing-main diagnostics")
for token in ["resolveDocumentLanguage", "isTripleSlashDelimiterLine", "effect", "package", "sealed", "@using"]:
    if token not in server:
        fail(f"LSP source missing {token}")

formatter = read("compiler_seen/src/lsp/formatter.seen")
for token in ["formatSeenDocumentEdits", "formatterUtf16Length", "formatterSortImportBlocks", "formatterIndentFromToml"]:
    if token not in formatter:
        fail(f"LSP formatter source missing {token}")

main_compiler = read("compiler_seen/src/main_compiler.seen")
if 'lang == "fr"' in main_compiler or "en, ar, es, ru, zh, ja, fr" in main_compiler:
    fail("compiler translate language list still includes French")

for doc_path in ["docs/getting-started.md", "docs/index.md", "vscode-seen/README.md", "vscode-seen/CHANGELOG.md"]:
    text = read(doc_path)
    if re.search(r"\bFrench\b|\bfr\b", text):
        fail(f"{doc_path} still advertises French as a supported language")

print("editor feature parity guard passed")
PY
