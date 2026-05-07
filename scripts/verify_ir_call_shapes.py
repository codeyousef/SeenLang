#!/usr/bin/env python3
"""Verify direct LLVM IR call shapes against declare/define signatures.

This is intentionally diagnostic-only. It does not repair IR; it catches
aggregate/scalar/pointer call-shape mismatches before opt has to be the first
thing to complain.
"""

from __future__ import annotations

import argparse
import re
import sys
from dataclasses import dataclass
from pathlib import Path


FUNC_NAME = r'(?:"[^"]+"|[A-Za-z_$.\-][A-Za-z0-9_$.\-]*)'
DECL_RE = re.compile(rf"^\s*(declare|define)\s+(?P<prefix>.*?)@(?P<name>{FUNC_NAME})\((?P<params>.*)\)")
CALL_RE = re.compile(rf"\bcall\b(?P<prefix>.*?)@(?P<name>{FUNC_NAME})\((?P<args>.*)\)")
TYPE_STARTS = (
    "void",
    "ptr",
    "i1",
    "i8",
    "i16",
    "i32",
    "i64",
    "i128",
    "float",
    "double",
    "half",
    "bfloat",
    "x86_fp80",
)
IGNORED_PREFIX_WORDS = {
    "declare",
    "define",
    "call",
    "tail",
    "musttail",
    "notail",
    "fastcc",
    "coldcc",
    "ccc",
    "dso_local",
    "local_unnamed_addr",
    "unnamed_addr",
    "private",
    "internal",
    "external",
    "available_externally",
    "linkonce",
    "linkonce_odr",
    "weak",
    "weak_odr",
    "appending",
    "dllimport",
    "dllexport",
    "noundef",
    "zeroext",
    "signext",
    "nonnull",
    "noalias",
    "dereferenceable",
    "nocapture",
    "readonly",
    "writeonly",
    "readnone",
}
ARG_ATTR_PREFIXES = (
    "noundef",
    "zeroext",
    "signext",
    "nonnull",
    "noalias",
    "nocapture",
    "readonly",
    "writeonly",
    "readnone",
    "byval",
    "sret",
    "align",
    "dereferenceable",
)


@dataclass(frozen=True)
class Signature:
    path: Path
    line: int
    ret: str
    params: tuple[str, ...]
    vararg: bool


@dataclass(frozen=True)
class CallSite:
    path: Path
    line: int
    function: str
    callee: str
    ret: str
    args: tuple[str, ...]


def strip_comment(line: str) -> str:
    in_quote = False
    idx = 0
    while idx < len(line):
        ch = line[idx]
        if ch == '"' and (idx == 0 or line[idx - 1] != "\\"):
            in_quote = not in_quote
        if ch == ";" and not in_quote:
            return line[:idx]
        idx += 1
    return line


def split_top_level(text: str, sep: str = ",") -> list[str]:
    parts: list[str] = []
    start = 0
    paren = brace = bracket = angle = 0
    in_quote = False
    idx = 0
    while idx < len(text):
        ch = text[idx]
        if ch == '"' and (idx == 0 or text[idx - 1] != "\\"):
            in_quote = not in_quote
        elif not in_quote:
            if ch == "(":
                paren += 1
            elif ch == ")":
                if paren == 0:
                    parts.append(text[start:idx].strip())
                    return parts
                paren -= 1
            elif ch == "{":
                brace += 1
            elif ch == "}":
                brace = max(0, brace - 1)
            elif ch == "[":
                bracket += 1
            elif ch == "]":
                bracket = max(0, bracket - 1)
            elif ch == "<":
                angle += 1
            elif ch == ">":
                angle = max(0, angle - 1)
            elif ch == sep and paren == 0 and brace == 0 and bracket == 0 and angle == 0:
                parts.append(text[start:idx].strip())
                start = idx + 1
        idx += 1
    tail = text[start:].strip()
    if tail:
        parts.append(tail)
    return parts


def trim_after_signature(text: str) -> str:
    paren = 0
    in_quote = False
    idx = 0
    while idx < len(text):
        ch = text[idx]
        if ch == '"' and (idx == 0 or text[idx - 1] != "\\"):
            in_quote = not in_quote
        elif not in_quote:
            if ch == "(":
                paren += 1
            elif ch == ")":
                if paren == 0:
                    return text[:idx]
                paren -= 1
        idx += 1
    return text


def normalize_function_name(name: str) -> str:
    if name.startswith('"') and name.endswith('"'):
        return name[1:-1]
    return name


def starts_type_token(token: str) -> bool:
    if not token:
        return False
    if token.startswith("%") or token.startswith("{") or token.startswith("[") or token.startswith("<"):
        return True
    return any(token == start or token.startswith(start + " ") for start in TYPE_STARTS)


def normalize_type(type_text: str) -> str:
    text = type_text.strip()
    text = re.sub(r"\s+", " ", text)
    text = re.sub(r"\s*\*\s*$", "*", text)
    return text


def leading_type(text: str) -> str:
    text = text.strip()
    while True:
        stripped = False
        for attr in ARG_ATTR_PREFIXES:
            if text == attr:
                return ""
            if text.startswith(attr + " "):
                text = text[len(attr) :].strip()
                stripped = True
                break
            if text.startswith(attr + "("):
                depth = 0
                for idx, ch in enumerate(text):
                    if ch == "(":
                        depth += 1
                    elif ch == ")":
                        depth -= 1
                        if depth == 0:
                            text = text[idx + 1 :].strip()
                            stripped = True
                            break
                break
        if not stripped:
            break

    if not text:
        return ""
    if text.startswith("{"):
        return normalize_type(read_balanced_prefix(text, "{", "}"))
    if text.startswith("["):
        return normalize_type(read_balanced_prefix(text, "[", "]"))
    if text.startswith("<"):
        return normalize_type(read_balanced_prefix(text, "<", ">"))
    first = text.split(None, 1)[0]
    if starts_type_token(first):
        return normalize_type(first)
    return ""


def read_balanced_prefix(text: str, open_ch: str, close_ch: str) -> str:
    depth = 0
    in_quote = False
    for idx, ch in enumerate(text):
        if ch == '"' and (idx == 0 or text[idx - 1] != "\\"):
            in_quote = not in_quote
        elif not in_quote:
            if ch == open_ch:
                depth += 1
            elif ch == close_ch:
                depth -= 1
                if depth == 0:
                    return text[: idx + 1]
    return text


def trailing_return_type(prefix: str) -> str:
    tokens = split_top_level(prefix.strip(), " ")
    candidates: list[str] = []
    idx = 0
    while idx < len(tokens):
        token = tokens[idx].strip()
        if not token or token in IGNORED_PREFIX_WORDS:
            idx += 1
            continue
        if token in {"align", "dereferenceable"}:
            idx += 2
            continue
        if starts_type_token(token):
            candidates.append(token)
        idx += 1
    if candidates:
        return normalize_type(candidates[-1])
    return normalize_type(prefix.strip().split()[-1]) if prefix.strip() else ""


def parse_params(params_text: str) -> tuple[tuple[str, ...], bool]:
    params = trim_after_signature(params_text)
    if not params.strip():
        return (), False
    vararg = False
    parsed: list[str] = []
    for part in split_top_level(params):
        part = part.strip()
        if not part:
            continue
        if part == "...":
            vararg = True
            continue
        ty = leading_type(part)
        if ty:
            parsed.append(ty)
    return tuple(parsed), vararg


def parse_file(path: Path) -> tuple[dict[str, Signature], list[CallSite]]:
    signatures: dict[str, Signature] = {}
    calls: list[CallSite] = []
    current_function = "<top-level>"
    with path.open("r", encoding="utf-8", errors="replace") as handle:
        for line_no, raw_line in enumerate(handle, 1):
            line = strip_comment(raw_line).strip()
            if not line:
                continue
            decl = DECL_RE.search(line)
            if decl:
                name = normalize_function_name(decl.group("name"))
                ret = trailing_return_type(decl.group("prefix"))
                params, vararg = parse_params(decl.group("params"))
                if decl.group(1) == "define":
                    current_function = name
                signatures[name] = Signature(path, line_no, ret, params, vararg)
                continue
            if line == "}":
                current_function = "<top-level>"
                continue
            call = CALL_RE.search(line)
            if call:
                callee = normalize_function_name(call.group("name"))
                if callee.startswith("llvm."):
                    continue
                ret = trailing_return_type(call.group("prefix"))
                args, _ = parse_params(call.group("args"))
                calls.append(CallSite(path, line_no, current_function, callee, ret, args))
    return signatures, calls


def collect_paths(items: list[str]) -> list[Path]:
    paths: list[Path] = []
    for item in items:
        path = Path(item)
        if path.is_dir():
            paths.extend(sorted(p for p in path.glob("*.ll") if not p.name.endswith(".opt.ll")))
            paths.extend(sorted(p for p in path.glob("seen_module_*.ll") if not p.name.endswith(".opt.ll")))
        else:
            paths.append(path)
    seen: set[Path] = set()
    unique: list[Path] = []
    for path in paths:
        resolved = path.resolve()
        if resolved not in seen:
            seen.add(resolved)
            unique.append(path)
    return unique


def shapes_match(expected: str, actual: str) -> bool:
    if expected == actual:
        return True
    # Treat old typed pointer spelling as pointer-compatible when it appears in
    # saved bootstrap IR. Opaque ptr is the canonical shape for current output.
    if expected == "ptr" and actual.endswith("*"):
        return True
    if actual == "ptr" and expected.endswith("*"):
        return True
    return False


def verify(paths: list[Path]) -> int:
    all_signatures: dict[str, Signature] = {}
    all_calls: list[CallSite] = []
    for path in paths:
        if not path.exists():
            print(f"ERROR: IR file not found: {path}", file=sys.stderr)
            return 1
        signatures, calls = parse_file(path)
        for name, sig in signatures.items():
            previous = all_signatures.get(name)
            if previous and (
                previous.ret != sig.ret
                or previous.params != sig.params
                or previous.vararg != sig.vararg
            ):
                print(
                    f"ERROR: {path}:{sig.line}: @{name} signature "
                    f"{sig.ret}({', '.join(sig.params)}) conflicts with "
                    f"{previous.path}:{previous.line}: {previous.ret}"
                    f"({', '.join(previous.params)})",
                    file=sys.stderr,
                )
                return 1
            all_signatures[name] = sig
        all_calls.extend(calls)

    errors = 0
    for call in all_calls:
        sig = all_signatures.get(call.callee)
        if sig is None:
            continue
        if call.ret and not shapes_match(sig.ret, call.ret):
            print(
                f"ERROR: {call.path}:{call.line}: in @{call.function}, call "
                f"to @{call.callee} returns {call.ret}, declaration returns {sig.ret}",
                file=sys.stderr,
            )
            errors += 1
        expected_count = len(sig.params)
        actual_count = len(call.args)
        if sig.vararg:
            if actual_count < expected_count:
                print(
                    f"ERROR: {call.path}:{call.line}: in @{call.function}, "
                    f"call to @{call.callee} has {actual_count} args, "
                    f"declaration needs at least {expected_count}",
                    file=sys.stderr,
                )
                errors += 1
                continue
        elif actual_count != expected_count:
            print(
                f"ERROR: {call.path}:{call.line}: in @{call.function}, "
                f"call to @{call.callee} has {actual_count} args, "
                f"declaration has {expected_count}",
                file=sys.stderr,
            )
            errors += 1
            continue
        for idx, expected in enumerate(sig.params):
            actual = call.args[idx] if idx < len(call.args) else ""
            if not shapes_match(expected, actual):
                print(
                    f"ERROR: {call.path}:{call.line}: in @{call.function}, "
                    f"call to @{call.callee} arg {idx + 1} is {actual}, "
                    f"declaration expects {expected}",
                    file=sys.stderr,
                )
                errors += 1
    return 1 if errors else 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("paths", nargs="+", help="LLVM .ll files or directories")
    args = parser.parse_args()
    paths = collect_paths(args.paths)
    if not paths:
        print("ERROR: no LLVM .ll files provided", file=sys.stderr)
        return 1
    return verify(paths)


if __name__ == "__main__":
    sys.exit(main())
