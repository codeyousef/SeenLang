#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

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
if package["version"] != "1.5.0":
    fail("VS Code extension version must be 1.5.0")

language_enum = package["contributes"]["configuration"]["properties"]["seen.language.default"]["enum"]
if language_enum != expected_languages:
    fail(f"seen.language.default enum drifted: {language_enum}")

commands = {entry["command"] for entry in package["contributes"]["commands"]}
for command in ["seen.pkgFetch", "seen.pkgPack", "seen.pkgPrebuild", "seen.pkgPublish"]:
    if command not in commands:
        fail(f"missing VS Code command {command}")

task_enum = package["contributes"]["taskDefinitions"][0]["properties"]["task"]["enum"]
for task in ["pkg-fetch", "pkg-pack", "pkg-prebuild", "pkg-publish"]:
    if task not in task_enum:
        fail(f"missing VS Code task {task}")

commands_source = read("vscode-seen/src/commands.ts")
for unsupported in ["seen/switchLanguage", "seen/translate", "seen/getStreamInfo"]:
    if unsupported in commands_source:
        fail(f"extension still calls unsupported custom LSP method {unsupported}")
if "French (fr)" in commands_source:
    fail("extension language picker still advertises French")
if "Japanese (ja)" not in commands_source:
    fail("extension language picker does not advertise Japanese")

diagnostics_source = read("vscode-seen/src/errorDiagnostics.ts")
if "--json-errors" in diagnostics_source:
    fail("standalone diagnostics still use unsupported --json-errors")
for expected in ["stdout.on", "stderr.on", "mkdtemp"]:
    if expected not in diagnostics_source:
        fail(f"standalone diagnostics missing {expected}")

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

main_compiler = read("compiler_seen/src/main_compiler.seen")
if 'lang == "fr"' in main_compiler or "en, ar, es, ru, zh, ja, fr" in main_compiler:
    fail("compiler translate language list still includes French")

for doc_path in ["docs/getting-started.md", "docs/index.md", "vscode-seen/README.md", "vscode-seen/CHANGELOG.md"]:
    text = read(doc_path)
    if re.search(r"\bFrench\b|\bfr\b", text):
        fail(f"{doc_path} still advertises French as a supported language")

print("editor feature parity guard passed")
PY
