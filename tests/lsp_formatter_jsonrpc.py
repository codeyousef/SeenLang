#!/usr/bin/env python3
"""End-to-end LSP formatter protocol regression suite."""

from __future__ import annotations

import json
import os
from pathlib import Path
import selectors
import subprocess
import sys
import time
from typing import Any


LANGUAGES = {
    "en": ("fun", "let", "return"),
    "ar": ("دالة", "اجعل", "رجع"),
    "es": ("función", "sea", "retornar"),
    "ru": ("функция", "пусть", "вернуть"),
    "zh": ("函数", "让", "返回"),
    "ja": ("関数", "定数", "戻る"),
}


def fail(message: str) -> None:
    raise AssertionError(message)


def utf16_length(text: str) -> int:
    return len(text.encode("utf-16-le")) // 2


def utf16_index(text: str, units: int) -> int:
    used = 0
    for index, character in enumerate(text):
        width = utf16_length(character)
        if used == units:
            return index
        if used + width > units:
            fail("LSP edit splits a UTF-16 surrogate pair")
        used += width
    if used == units:
        return len(text)
    fail(f"UTF-16 character {units} exceeds line length {used}")


def line_offsets(text: str) -> tuple[list[str], list[int]]:
    lines = text.splitlines(keepends=True)
    if not lines or text.endswith(("\n", "\r")):
        lines.append("")
    contents: list[str] = []
    offsets: list[int] = []
    offset = 0
    for line in lines:
        offsets.append(offset)
        contents.append(line.removesuffix("\n").removesuffix("\r"))
        offset += len(line)
    return contents, offsets


def apply_edits(text: str, edits: list[dict[str, Any]]) -> str:
    lines, offsets = line_offsets(text)
    replacements: list[tuple[int, int, str]] = []
    previous_end = -1
    for edit in sorted(
        edits,
        key=lambda item: (
            item["range"]["start"]["line"],
            item["range"]["start"]["character"],
        ),
    ):
        start = edit["range"]["start"]
        end = edit["range"]["end"]
        if start["line"] != end["line"]:
            fail("formatter edit spans original physical lines")
        line = start["line"]
        if line >= len(lines):
            fail("formatter edit line is outside the document")
        absolute_start = offsets[line] + utf16_index(lines[line], start["character"])
        absolute_end = offsets[line] + utf16_index(lines[line], end["character"])
        if absolute_start < previous_end or absolute_end < absolute_start:
            fail("formatter edits overlap or use an inverted range")
        previous_end = absolute_end
        replacements.append((absolute_start, absolute_end, edit["newText"]))

    result = text
    for start, end, replacement in reversed(replacements):
        result = result[:start] + replacement + result[end:]
    return result


class LspClient:
    def __init__(self, executable: Path, environment: dict[str, str]):
        self.process = subprocess.Popen(
            [str(executable)],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=environment,
        )
        self.notifications: list[dict[str, Any]] = []

    def send(self, message: dict[str, Any]) -> None:
        assert self.process.stdin is not None
        body = json.dumps(message, ensure_ascii=False, separators=(",", ":")).encode()
        header = f"Content-Length: {len(body)}\r\n\r\n".encode()
        self.process.stdin.write(header + body)
        self.process.stdin.flush()

    def _read_exact(self, count: int, deadline: float) -> bytes:
        assert self.process.stdout is not None
        result = bytearray()
        selector = selectors.DefaultSelector()
        selector.register(self.process.stdout, selectors.EVENT_READ)
        try:
            while len(result) < count:
                remaining = deadline - time.monotonic()
                if remaining <= 0 or not selector.select(remaining):
                    fail("timed out reading an LSP payload")
                chunk = os.read(self.process.stdout.fileno(), count - len(result))
                if not chunk:
                    stderr = self.process.stderr.read().decode(errors="replace")
                    fail(f"LSP server closed its output early: {stderr}")
                result.extend(chunk)
        finally:
            selector.close()
        return bytes(result)

    def read(self, timeout: float = 10.0) -> dict[str, Any]:
        deadline = time.monotonic() + timeout
        header = bytearray()
        while not header.endswith(b"\r\n\r\n"):
            header.extend(self._read_exact(1, deadline))
            if len(header) > 8192:
                fail("LSP response header exceeded 8 KiB")
        content_length = None
        for line in header.decode("ascii").split("\r\n"):
            if line.lower().startswith("content-length:"):
                content_length = int(line.split(":", 1)[1].strip())
        if content_length is None:
            fail("LSP response omitted Content-Length")
        return json.loads(self._read_exact(content_length, deadline).decode("utf-8"))

    def response(self, request_id: int) -> dict[str, Any]:
        while True:
            message = self.read()
            if message.get("id") == request_id:
                return message
            self.notifications.append(message)

    def initialize(self) -> None:
        self.send({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}})
        response = self.response(1)
        if "result" not in response:
            fail(f"LSP initialize failed: {response}")
        self.send({"jsonrpc": "2.0", "method": "initialized", "params": {}})

    def open(self, uri: str, text: str, version: int = 1) -> None:
        self.send(
            {
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": uri,
                        "languageId": "seen",
                        "version": version,
                        "text": text,
                    }
                },
            }
        )

    def change(self, uri: str, text: str, version: int) -> None:
        self.send(
            {
                "jsonrpc": "2.0",
                "method": "textDocument/didChange",
                "params": {
                    "textDocument": {"uri": uri, "version": version},
                    "contentChanges": [{"text": text}],
                },
            }
        )

    def format(
        self, uri: str, request_id: int, *, use_manifest: bool = True
    ) -> dict[str, Any]:
        self.send(
            {
                "jsonrpc": "2.0",
                "id": request_id,
                "method": "textDocument/formatting",
                "params": {
                    "textDocument": {"uri": uri},
                    "options": {
                        "tabSize": 2,
                        "insertSpaces": True,
                        "useManifest": use_manifest,
                        "sortImports": True,
                    },
                },
            }
        )
        return self.response(request_id)

    def close(self) -> None:
        if self.process.poll() is None:
            self.send({"jsonrpc": "2.0", "id": 999, "method": "shutdown", "params": {}})
            self.response(999)
            self.send({"jsonrpc": "2.0", "method": "exit", "params": {}})
        try:
            self.process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            self.process.terminate()
            self.process.wait(timeout=5)
            fail("LSP server did not exit after shutdown")
        if self.process.returncode != 0:
            assert self.process.stderr is not None
            fail(self.process.stderr.read().decode(errors="replace"))


def manifest(language: str, *, line_width: int = 100) -> str:
    return (
        "manifest-version = 1\n\n"
        "[project]\n"
        'name = "formatter_protocol"\n'
        'version = "0.1.0"\n'
        f'language = "{language}"\n\n'
        "[format]\n"
        f"line-width = {line_width}\n"
        "indent = 7\n"
        "trailing-comma = true\n"
        "sort-imports = true\n"
    )


def valid_protocol_suite(client: LspClient, work: Path) -> None:
    next_id = 10
    for language, (fun_kw, let_kw, return_kw) in LANGUAGES.items():
        project = work / f"six languages {language}"
        project.mkdir(parents=True)
        (project / "Seen.toml").write_text(manifest(language), encoding="utf-8")
        source_path = project / "main file.seen"
        source = (
            f"{fun_kw} main()r:Int{{\n"
            f'  {let_kw} emoji = "😀";{let_kw} value=1+2\n'
            f"{return_kw} value\n"
            "}\n"
        )
        expected = (
            f"{fun_kw} main() r: Int {{\n"
            f'  {let_kw} emoji = "😀"; {let_kw} value = 1 + 2\n'
            f"  {return_kw} value\n"
            "}\n"
        )
        uri = source_path.as_uri()
        if "%20" not in uri:
            fail("protocol fixture did not exercise percent-encoded file paths")
        client.open(uri, source)
        response = client.format(uri, next_id)
        next_id += 1
        if "error" in response:
            fail(f"{language} formatting returned an error: {response['error']}")
        edits = response.get("result")
        if not isinstance(edits, list) or not edits:
            fail(f"{language} formatting returned no edits")
        formatted = apply_edits(source, edits)
        if formatted != expected:
            fail(f"{language} protocol formatting mismatch\n{formatted!r}\n{expected!r}")

        emoji_line = source.splitlines()[1]
        emoji_edits = [edit for edit in edits if edit["range"]["start"]["line"] == 1]
        if len(emoji_edits) != 1:
            fail(f"{language} expected one minimal edit on the emoji line")
        expected_prefix = emoji_line.index(";", emoji_line.index("😀")) + 1
        if emoji_edits[0]["range"]["start"]["character"] != utf16_length(
            emoji_line[:expected_prefix]
        ):
            fail(f"{language} edit start was not a minimal UTF-16 position")

        client.change(uri, formatted, 2)
        second = client.format(uri, next_id)
        next_id += 1
        if second.get("result") != []:
            fail(f"{language} formatter was not idempotent: {second}")

    invalid_project = work / "invalid region"
    invalid_project.mkdir()
    (invalid_project / "Seen.toml").write_text(manifest("en"), encoding="utf-8")
    invalid_uri = (invalid_project / "invalid gap.seen").as_uri()
    invalid_source = (
        "fun before()r:Int{ return 1 }\n"
        "let     = 1\n"
        "fun after()r:Int{ return 2 }\n"
    )
    client.open(invalid_uri, invalid_source)
    invalid_response = client.format(invalid_uri, next_id)
    next_id += 1
    invalid_formatted = apply_edits(invalid_source, invalid_response.get("result", []))
    if invalid_formatted.splitlines()[1] != "let     = 1":
        fail("parser-invalid balanced gap was modified by protocol formatting")
    if "fun before() r: Int" not in invalid_formatted or "fun after() r: Int" not in invalid_formatted:
        fail("valid regions around an invalid gap were not formatted")

    import_project = work / "attached imports"
    import_project.mkdir()
    (import_project / "Seen.toml").write_text(manifest("en"), encoding="utf-8")
    import_uri = (import_project / "imports.seen").as_uri()
    import_source = (
        "// zeta\nimport zeta.mod\n"
        "// alpha\nimport alpha.mod\n"
        "fun main() r: Int { return 0 }\n"
    )
    import_expected = (
        "// alpha\nimport alpha.mod\n"
        "// zeta\nimport zeta.mod\n"
        "fun main() r: Int { return 0 }\n"
    )
    client.open(import_uri, import_source)
    import_response = client.format(import_uri, next_id)
    next_id += 1
    if apply_edits(import_source, import_response.get("result", [])) != import_expected:
        fail("attached import comments did not move with their imports")

    crlf_project = work / "dominant newline"
    crlf_project.mkdir()
    (crlf_project / "Seen.toml").write_text(
        manifest("en", line_width=44), encoding="utf-8"
    )
    crlf_uri = (crlf_project / "wide call.seen").as_uri()
    crlf_source = (
        "fun combine(first: Int, second: Int, third: Int) r: Int {\r\n"
        "  return first+second+third\r\n}\r\n"
        "fun main() r: Int {\r\n"
        "  let value=combine(111111111,222222222,333333333)\r\n"
        "  return value\r\n}\r\n"
    )
    client.open(crlf_uri, crlf_source)
    crlf_response = client.format(crlf_uri, next_id)
    crlf_edits = crlf_response.get("result", [])
    crlf_formatted = apply_edits(crlf_source, crlf_edits)
    if "\n" in crlf_formatted.replace("\r\n", ""):
        fail("inserted formatter lines did not use the dominant CRLF style")
    if "333333333,\r\n" not in crlf_formatted:
        fail("line-width reflow did not add the configured trailing comma")


def rejected_language_suite(
    executable: Path, environment: dict[str, str], work: Path, language: str
) -> None:
    project = work / f"rejected {language}"
    project.mkdir(parents=True)
    (project / "Seen.toml").write_text(manifest(language), encoding="utf-8")
    uri = (project / "main.seen").as_uri()
    client = LspClient(executable, environment)
    try:
        client.initialize()
        client.open(uri, "fun main() r: Int { return 0 }\n")
        response = client.format(uri, 80)
        error = response.get("error", {})
        if "unsupported language" not in error.get("message", "") and \
                "language pack" not in error.get("message", ""):
            fail(f"language failure did not return an explicit LSP error: {response}")
        while not any(
            notification.get("method") == "textDocument/publishDiagnostics"
            and notification.get("params", {}).get("diagnostics")
            for notification in client.notifications
        ):
            client.notifications.append(client.read())
    finally:
        client.close()


def portable_file_uri_suite(client: LspClient) -> None:
    source = "fun main()r:Int{ return 0 }\n"
    expected = "fun main() r: Int { return 0 }\n"
    uris = (
        "file:///C:/Seen%20Project/src/main.seen",
        "file://build-server/share/Seen%20Project/src/main.seen",
        "file:////build-server/share/Seen%20Project/src/main.seen",
    )
    for request_id, uri in enumerate(uris, start=90):
        client.open(uri, source)
        response = client.format(uri, request_id, use_manifest=False)
        if "error" in response:
            fail(f"portable file URI formatting failed for {uri}: {response}")
        if apply_edits(source, response.get("result", [])) != expected:
            fail(f"portable file URI was resolved incorrectly: {uri}")


def main() -> int:
    if len(sys.argv) != 4:
        fail(
            "usage: lsp_formatter_jsonrpc.py "
            "<server-executable> <repo-root> <allowed-artifact-root>"
        )
    executable = Path(sys.argv[1]).resolve()
    repo = Path(sys.argv[2]).resolve()
    allowed_input = Path(sys.argv[3])
    if not allowed_input.is_absolute() or not allowed_input.is_dir() or \
            allowed_input.is_symlink():
        fail("allowed artifact root must be an existing absolute directory")
    allowed_root = allowed_input.resolve()
    if allowed_input != allowed_root:
        fail("allowed artifact root must be canonical and symlink-free")
    try:
        allowed_relative = allowed_root.relative_to(repo)
    except ValueError:
        fail("allowed artifact root must stay inside the checkout")
    ignored = subprocess.run(
        ["git", "-C", str(repo), "check-ignore", "-q", "--", str(allowed_relative)],
        check=False,
    )
    if ignored.returncode != 0:
        fail("allowed artifact root must be ignored by Git")
    artifact_text = os.environ.get("SEEN_ARTIFACT_ROOT", "")
    if not artifact_text:
        fail("SEEN_ARTIFACT_ROOT is required")
    artifact_input = Path(artifact_text)
    if not artifact_input.is_absolute() or not artifact_input.is_dir() or \
            artifact_input.is_symlink():
        fail("SEEN_ARTIFACT_ROOT must be an existing absolute directory")
    artifact_root = artifact_input.resolve()
    if artifact_input != artifact_root:
        fail("SEEN_ARTIFACT_ROOT must be canonical and symlink-free")
    if artifact_root != allowed_root and allowed_root not in artifact_root.parents:
        fail("JSON-RPC fixtures must stay under the acceptance artifact root")
    work = artifact_root / "formatter-jsonrpc-fixtures"
    work.mkdir()

    environment = os.environ.copy()
    environment["SEEN_DATA_PATH"] = str(repo / "languages")
    client = LspClient(executable, environment)
    try:
        client.initialize()
        valid_protocol_suite(client, work)
        portable_file_uri_suite(client)
    finally:
        client.close()

    rejected_language_suite(executable, environment, work, "xx")

    incomplete = work / "incomplete data path"
    incomplete.mkdir()
    missing_environment = environment.copy()
    missing_environment["SEEN_DATA_PATH"] = str(incomplete)
    rejected_language_suite(executable, missing_environment, work, "en")

    print("LSP formatter JSON-RPC protocol tests passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
