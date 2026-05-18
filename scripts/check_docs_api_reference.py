#!/usr/bin/env python3
"""Lightweight public docs/API-reference drift checks."""

from __future__ import annotations

import pathlib
import re
import sys
import urllib.parse


ROOT = pathlib.Path(__file__).resolve().parents[1]
DOCS = ROOT / "docs"
API = DOCS / "api-reference"
README = ROOT / "README.md"
LANGUAGES = ROOT / "languages"
STDLIB = ROOT / "seen_std" / "src"


def public_markdown_files() -> list[pathlib.Path]:
    files = [README]
    files.extend(
        p
        for p in DOCS.rglob("*.md")
        if "docs/private" not in p.as_posix()
    )
    return sorted(files)


def rel(path: pathlib.Path) -> str:
    return path.relative_to(ROOT).as_posix()


def check_stale_cli_examples(errors: list[str]) -> None:
    stale = [
        "seen fmt",
        "seen format",
        "seen init",
        "seen test",
        "seen clean",
        "--backend c",
        "--trace-llvm",
        "--dump-struct-layouts",
        "--runtime-debug",
    ]
    for path in public_markdown_files():
        text = path.read_text(encoding="utf-8", errors="ignore")
        for line_no, line in enumerate(text.splitlines(), 1):
            for needle in stale:
                if needle not in line:
                    continue
                if "not shipped compiler commands" in line:
                    continue
                errors.append(f"{rel(path)}:{line_no} contains stale CLI reference `{needle}`")
        for line_no, line in enumerate(text.splitlines(), 1):
            if "seen build" in line and "older `seen build` examples are stale" not in line and "not a shipped" not in line:
                errors.append(f"{rel(path)}:{line_no} uses stale `seen build`")


def check_language_docs(errors: list[str]) -> None:
    langs = sorted(p.name for p in LANGUAGES.iterdir() if p.is_dir())
    expected = ["ar", "en", "es", "ja", "ru", "zh"]
    if langs != expected:
        errors.append(f"language dirs are {langs}, expected {expected}")

    per_lang_counts = {lang: len(list((LANGUAGES / lang).glob("*.toml"))) for lang in langs}
    if len(set(per_lang_counts.values())) != 1:
        errors.append(f"language TOML counts differ: {per_lang_counts}")
        return

    per_lang = next(iter(per_lang_counts.values()))
    total = per_lang * len(langs)
    multilingual = (DOCS / "multilingual.md").read_text(encoding="utf-8")
    if f"{per_lang} files per language" not in multilingual:
        errors.append("docs/multilingual.md has stale per-language TOML count")
    if f"{total} files across {len(langs)} languages" not in multilingual:
        errors.append("docs/multilingual.md has stale total TOML count")

    public_text = "\n".join(
        p.read_text(encoding="utf-8", errors="ignore")
        for p in public_markdown_files()
    )
    if re.search(r"\bFrench\b|Français|languages/fr\b", public_text):
        errors.append("public docs still mention unsupported French language entries")


def check_api_module_coverage(errors: list[str]) -> None:
    api_text = "\n".join(
        p.read_text(encoding="utf-8", errors="ignore")
        for p in API.glob("*.md")
    )
    modules = sorted(p.relative_to(STDLIB).with_suffix("").as_posix() for p in STDLIB.rglob("*.seen"))
    missing = [m for m in modules if f"`{m}`" not in api_text and m not in api_text]
    if missing:
        preview = ", ".join(missing[:20])
        suffix = "" if len(missing) <= 20 else f" ... (+{len(missing) - 20} more)"
        errors.append(f"API reference missing stdlib module paths: {preview}{suffix}")

    required_pages = [
        "async.md",
        "audio.md",
        "env.md",
        "ffi.md",
        "framework.md",
        "graphics.md",
        "input.md",
        "memory.md",
        "net.md",
        "platform.md",
        "scripting.md",
        "security.md",
        "simd.md",
        "thread.md",
        "time.md",
        "uww.md",
        "stdlib-modules.md",
    ]
    for page in required_pages:
        if not (API / page).exists():
            errors.append(f"missing API reference page docs/api-reference/{page}")


def check_public_doc_links(errors: list[str]) -> None:
    link_re = re.compile(r"!?\[[^\]]*\]\(([^)]+)\)")
    for path in sorted(p for p in DOCS.rglob("*.md") if "docs/private" not in p.as_posix()):
        text = path.read_text(encoding="utf-8", errors="ignore")
        for match in link_re.finditer(text):
            href = match.group(1).strip()
            target_href = href.split("#", 1)[0]
            if not target_href:
                continue
            if re.match(r"^[a-zA-Z][a-zA-Z0-9+.-]*:", target_href):
                continue
            target = (path.parent / urllib.parse.unquote(target_href)).resolve()
            if not target.exists():
                errors.append(f"{rel(path)} links to missing {href}")


def main() -> int:
    errors: list[str] = []
    check_stale_cli_examples(errors)
    check_language_docs(errors)
    check_api_module_coverage(errors)
    check_public_doc_links(errors)

    if errors:
        print("Docs/API reference checks failed:")
        for error in errors:
            print(f"- {error}")
        return 1

    print("Docs/API reference checks passed.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
