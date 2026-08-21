#!/usr/bin/env python3
"""Validate and canonically render a bounded Seen machine diagnostic."""

from __future__ import annotations

import argparse
import json
import os
import random
import re
import sys
import time
from pathlib import Path

MAX_INPUT_BYTES = 1_048_576
MAX_MESSAGE_BYTES = 4096
MAX_CAUSES = 8
MAX_DEPTH = 8
MAX_IDENTITY_BYTES = 128
MAX_PATH_BYTES = 4096
TOP_FIELDS = {"context", "error", "platform", "schema"}
CONTEXT_FIELDS = {
    "backend", "device_capability", "entry_point", "fallback_reason",
    "maturity", "source", "target",
}
SOURCE_FIELDS = {"column", "file", "line"}
ERROR_FIELDS = {
    "causes", "code", "message", "native_code", "operation",
    "redaction", "retry", "subsystem",
}
MATURITY = {
    "unsupported", "compile-only", "experimental-hardware", "verified",
    "production-certified",
}
PLATFORMS = {
    "android-arm64", "ios-arm64", "ios-sim-arm64", "linux-arm64",
    "linux-riscv64", "linux-x86_64", "macos", "macos-arm64",
    "macos-x86_64", "windows", "windows-x86_64",
}
IDENTITY_RE = re.compile(r"^[A-Za-z0-9._+-]+$")
PATH_RE = re.compile(r"^[A-Za-z0-9._/-]+$")


class DiagnosticError(ValueError):
    def __init__(self, code: str, message: str) -> None:
        super().__init__(message)
        self.code = code


def fail(code: str, message: str) -> None:
    raise DiagnosticError(code, message)


def object_without_duplicates(
    pairs: list[tuple[str, object]],
) -> dict[str, object]:
    value: dict[str, object] = {}
    for key, item in pairs:
        if key in value:
            fail("invalid", f"duplicate JSON field: {key}")
        value[key] = item
    return value


def exact_object(value: object, fields: set[str], name: str) -> dict[str, object]:
    if not isinstance(value, dict) or set(value) != fields:
        fail("invalid", f"{name} has missing or unknown fields")
    return value


def bounded_string(value: object, maximum: int, name: str) -> str:
    if not isinstance(value, str):
        fail("invalid", f"{name} is not a string")
    if len(value.encode("utf-8")) > maximum:
        fail("limit", f"{name} byte limit exceeded")
    return value


def identity(value: object, name: str, optional: bool = False) -> str:
    item = bounded_string(value, MAX_IDENTITY_BYTES, name)
    if optional and item == "":
        return item
    if not item or not IDENTITY_RE.fullmatch(item):
        fail("invalid", f"{name} is not a stable ASCII identity")
    return item


def source_path(value: object) -> str:
    path = bounded_string(value, MAX_PATH_BYTES, "source file")
    if path == "":
        return path
    if path.startswith("/") or path.endswith("/") or ".." in path or \
            "//" in path or not PATH_RE.fullmatch(path):
        fail("invalid", "source file is not a canonical relative path")
    return path


def integer(value: object, name: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        fail("invalid", f"{name} is not an integer")
    return value


def validate_error(raw: object, depth: int = 0) -> dict[str, object]:
    if depth > MAX_DEPTH:
        fail("limit", "diagnostic cause depth exceeded")
    value = exact_object(raw, ERROR_FIELDS, "diagnostic error")
    causes = value["causes"]
    if not isinstance(causes, list):
        fail("invalid", "diagnostic causes are not an array")
    if len(causes) > MAX_CAUSES or (depth == MAX_DEPTH and causes):
        fail("limit", "diagnostic cause limit exceeded")
    redaction = value["redaction"]
    if redaction not in {"public", "sensitive"}:
        fail("invalid", "diagnostic redaction class is invalid")
    retry = value["retry"]
    if retry not in {"never", "transient"}:
        fail("invalid", "diagnostic retry class is invalid")
    message = bounded_string(value["message"], MAX_MESSAGE_BYTES, "message")
    native_code = value["native_code"]
    if native_code is not None:
        native_code = integer(native_code, "native code")
        if not -(2**63) <= native_code < 2**63:
            fail("limit", "native code is outside i64")
    rendered_causes = [validate_error(cause, depth + 1) for cause in causes]
    if redaction == "sensitive":
        message = "<redacted>"
    return {
        "causes": rendered_causes,
        "code": identity(value["code"], "diagnostic code"),
        "message": message,
        "native_code": native_code,
        "operation": identity(value["operation"], "diagnostic operation"),
        "redaction": redaction,
        "retry": retry,
        "subsystem": identity(value["subsystem"], "diagnostic subsystem"),
    }


def parse_and_validate(
    raw: bytes,
    max_bytes: int = MAX_INPUT_BYTES,
    max_path_bytes: int = MAX_PATH_BYTES,
    cancelled: bool = False,
) -> dict[str, object]:
    if not 1 <= max_bytes <= MAX_INPUT_BYTES or len(raw) > max_bytes:
        fail("limit", "machine diagnostic input byte limit exceeded")
    if not 1 <= max_path_bytes <= MAX_PATH_BYTES:
        fail("limit", "machine diagnostic path limit is invalid")
    if cancelled:
        fail("cancelled", "machine diagnostic validation cancelled")
    if raw.startswith(b"\xef\xbb\xbf"):
        fail("invalid", "machine diagnostic input must not contain a UTF-8 BOM")
    try:
        document = json.loads(
            raw.decode("utf-8"), object_pairs_hook=object_without_duplicates)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        fail("invalid", f"machine diagnostic JSON is invalid: {error}")
    value = exact_object(document, TOP_FIELDS, "machine diagnostic input")
    if value["schema"] != "seen-machine-diagnostic-input-v1":
        fail("invalid", "machine diagnostic input schema is unsupported")
    if value["platform"] not in PLATFORMS:
        fail("platform", "machine diagnostic platform is unsupported")
    context = exact_object(value["context"], CONTEXT_FIELDS, "diagnostic context")
    source = exact_object(context["source"], SOURCE_FIELDS, "diagnostic source")
    file = source_path(source["file"])
    if len(file.encode("utf-8")) > max_path_bytes:
        fail("limit", "source file byte limit exceeded")
    line = integer(source["line"], "source line")
    column = integer(source["column"], "source column")
    if line < 0 or column < 0 or (not file and (line or column)) or \
            (file and (line < 1 or column < 1)):
        fail("invalid", "source coordinates are invalid")
    maturity = context["maturity"]
    if maturity not in MATURITY:
        fail("invalid", "backend maturity is invalid")
    error = validate_error(value["error"])
    fallback = bounded_string(
        context["fallback_reason"], MAX_MESSAGE_BYTES, "fallback reason")
    if error["redaction"] == "sensitive":
        if file:
            file = "<redacted>"
        if fallback:
            fallback = "<redacted>"
    return {
        "context": {
            "backend": identity(context["backend"], "backend", optional=True),
            "device_capability": identity(
                context["device_capability"], "device capability", optional=True),
            "entry_point": identity(
                context["entry_point"], "entry point", optional=True),
            "fallback_reason": fallback,
            "maturity": maturity,
            "source": {"column": column, "file": file, "line": line},
            "target": identity(context["target"], "target", optional=True),
        },
        "error": error,
        "schema": "seen-machine-diagnostic-v1",
    }


def fuzz(corpus: bytes, seconds: float, seed: int) -> None:
    rng = random.Random(seed)
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        mutated = bytearray(corpus)
        for _ in range(1 + rng.randrange(4)):
            action = rng.randrange(3)
            if action == 0 and mutated:
                del mutated[rng.randrange(len(mutated))]
            elif action == 1 and len(mutated) < MAX_INPUT_BYTES:
                mutated.insert(rng.randrange(len(mutated) + 1), rng.randrange(256))
            elif mutated:
                mutated[rng.randrange(len(mutated))] = rng.randrange(256)
        try:
            json.dumps(parse_and_validate(bytes(mutated)), sort_keys=True)
        except DiagnosticError:
            pass


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("input", type=Path)
    parser.add_argument("--max-bytes", type=int, default=MAX_INPUT_BYTES)
    parser.add_argument("--max-path-bytes", type=int, default=MAX_PATH_BYTES)
    parser.add_argument("--fuzz-seconds", type=float, default=0.0)
    parser.add_argument("--seed", type=int, default=1101)
    parser.add_argument("--test-cancel-after-read", action="store_true")
    args = parser.parse_args(argv)
    try:
        if not 0 <= args.fuzz_seconds <= 300:
            fail("limit", "fuzz-seconds must be between 0 and 300")
        raw = args.input.read_bytes()
        cancelled = args.test_cancel_after_read
        if cancelled and os.environ.get("SEEN_MACHINE_DIAGNOSTIC_TEST_HOOKS") != "1":
            fail("invalid", "cancellation hook is test-only")
        rendered = parse_and_validate(
            raw, args.max_bytes, args.max_path_bytes, cancelled)
        if args.fuzz_seconds:
            fuzz(raw, args.fuzz_seconds, args.seed)
            print(
                f"machine-diagnostic: fuzz seed={args.seed} "
                f"seconds={args.fuzz_seconds:g} status=pass", file=sys.stderr)
        json.dump(rendered, sys.stdout, indent=2, sort_keys=True)
        sys.stdout.write("\n")
    except DiagnosticError as error:
        print(f"core.rel.001.{error.code}: {error}", file=sys.stderr)
        return 130 if error.code == "cancelled" else 1
    except OSError as error:
        print(f"core.rel.001.io: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
