#!/usr/bin/env python3
"""Stage ordinary toolchain layouts and real native include consumers."""
from __future__ import annotations

import argparse
import shutil
import sys
from pathlib import Path


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--compiler", type=Path, required=True)
    parser.add_argument("--destination", type=Path, required=True)
    args = parser.parse_args()
    repo = args.repo.resolve(strict=True)
    destination = args.destination.resolve(strict=True)
    if repo / ".seen" not in destination.parents:
        parser.error("destination must be an existing project-local scratch directory")
    sys.path.insert(0, str(repo / "scripts"))
    from produce_program_reproducibility_evidence import (  # existing payload policy
        ignore_generated,
        selected_package_client,
    )
    compiler = args.compiler.resolve(strict=True)
    package_client = selected_package_client(repo, compiler)
    for layout in ("checkout", "installed"):
        root = destination / layout
        binary = root / ("compiler_seen/target" if layout == "checkout" else "bin")
        std = root / ("seen_std/src" if layout == "checkout" else "lib/seen/std")
        runtime = root / ("seen_runtime" if layout == "checkout" else "lib/seen/runtime")
        binary.mkdir(parents=True)
        shutil.copy2(compiler, binary / "seen")
        shutil.copy2(package_client, binary / "seen-pkg")
        shutil.copytree(repo / "seen_std/src", std, ignore=ignore_generated)
        shutil.copytree(repo / "seen_runtime", runtime, ignore=ignore_generated)
        compatibility = root / ("releases" if layout == "checkout" else "bin")
        compatibility.mkdir(exist_ok=True)
        shutil.copy2(repo / "releases/compatibility-manifest.json", compatibility)
        if layout == "installed":
            for path in sorted(root.rglob("*"), reverse=True):
                path.chmod(path.stat().st_mode & ~0o222)
            root.chmod(root.stat().st_mode & ~0o222)

    fixture = repo / "tests/fixtures/native-runtime-flags"
    caller = destination / "consumer"
    caller.mkdir()
    shutil.copy2(fixture / "main.seen", caller / "main.seen")
    (caller / "Seen.toml").write_text(
        'manifest-version = 1\n[project]\nname = "native_flags_consumer"\n'
        'version = "0.1.0"\nlanguage = "en"\nmodules = ["main.seen"]\n', encoding="utf-8")
    forced = caller / "forced headers"
    forced.mkdir()
    shutil.copy2(fixture / "forced.h", forced / "entry.h")
    for folder, filename, macro, value in (
        ("separate", "separate", "SEPARATE", 11),
        ("combined", "combined", "COMBINED", 23),
        ("include with spaces", "spaces", "SPACES", 37),
        ("system headers", "system", "SYSTEM", 41),
    ):
        directory = caller / folder
        directory.mkdir()
        (directory / f"fel748_{filename}.h").write_text(
            f"#define FEL748_{macro} {value}\n", encoding="utf-8")
    for name, value in (("environment first", 619), ("environment second", -1)):
        directory = caller / name
        directory.mkdir()
        (directory / "fel748_environment.h").write_text(
            f"#define FEL748_ENVIRONMENT {value}\n", encoding="utf-8")
        (directory / "fel748_separate.h").write_text(
            "#error explicit -I must precede inherited search paths\n", encoding="utf-8")
    shutil.copy2(caller / "environment first/fel748_environment.h", caller)
    poison = caller / "cxx only"
    poison.mkdir()
    for name in ("stddef.h", "stdint.h", "string.h"):
        (poison / name).write_text("#error CPLUS_INCLUDE_PATH must not affect C runtime sources\n", encoding="utf-8")
    macros = caller / "macro headers"
    macros.mkdir()
    (macros / "macros.h").write_text("#define FEL748_IMACROS 53\n", encoding="utf-8")
    common_flags = (
        "-I separate -Icombined -I 'include with spaces' -isystem 'system headers' "
        "'-DFEL748_QUOTED_TEXT=\"quoted macro with spaces:748\"' "
    )
    for name, include, imacros in (
        ("controlled", "-include 'forced headers/entry.h'", "-imacros 'macro headers/macros.h'"),
        ("joined-include", "'-includeforced headers/entry.h'", "-imacros 'macro headers/macros.h'"),
        ("joined-imacros", "-include 'forced headers/entry.h'", "'-imacrosmacro headers/macros.h'"),
    ):
        (caller / f"{name}.cflags").write_text(
            f"{common_flags}{include} {imacros}\n", encoding="utf-8")
    # Forced includes use Clang's include search, not caller/file concatenation.
    # Neither bare name exists in the caller directory.
    for name, include, imacros, search in (
        ("forced-search-include", "-include entry.h", "-imacros 'macro headers/macros.h'", "-I 'forced headers'"),
        ("forced-search-include-joined", "-includeentry.h", "-imacros 'macro headers/macros.h'", "-I 'forced headers'"),
        ("forced-search-imacros", "-include 'forced headers/entry.h'", "-imacros macros.h", "-I 'macro headers'"),
        ("forced-search-imacros-joined", "-include 'forced headers/entry.h'", "-imacrosmacros.h", "-I 'macro headers'"),
        ("forced-search-include-env", "-include entry.h", "-imacros 'macro headers/macros.h'", ""),
        ("forced-search-imacros-env", "-include 'forced headers/entry.h'", "-imacros macros.h", ""),
    ):
        (caller / f"{name}.cflags").write_text(
            f"{common_flags}{search} {include} {imacros}\n", encoding="utf-8")
    after = caller / "after headers"
    after.mkdir()
    (after / "fel748_after.h").write_text("#define FEL748_AFTER 67\n", encoding="utf-8")
    (after / "fel748_system.h").write_text(
        "#error -isystem-after must follow explicit -isystem directories\n", encoding="utf-8")
    regular = "-include 'forced headers/entry.h' -imacros 'macro headers/macros.h'"
    # LLVM 20 accepts but ignores -isystem-after for this native C driver.
    # Preserve its operand consumption; use supported -idirafter for lookup.
    for name, option in (
        ("isystem-after", "-isystem-after 'ignored after path'"),
        ("isystem-after-joined", "'-isystem-afterignored after path'"),
    ):
        (caller / f"{name}.cflags").write_text(
            f"{common_flags}{option} -idirafter 'after headers' -DFEL748_EXPECT_AFTER=1 {regular}\n", encoding="utf-8")
    sdk = caller / "test sdk"
    sdk.mkdir()
    # Preserve host libc headers while giving sysroot-relative frameworks a
    # distinct root. No development toolchain or large tree is copied.
    (sdk / "usr").symlink_to("/usr", target_is_directory=True)
    for root in (caller, sdk):
        headers = root / "framework headers/Fel748.framework/Headers"
        headers.mkdir(parents=True)
        (headers / "value.h").write_text("#define FEL748_FRAMEWORK 71\n", encoding="utf-8")
    for name, option in (
        ("framework-sysroot", "-iframeworkwithsysroot 'framework headers'"),
        ("framework-sysroot-joined", "'-iframeworkwithsysrootframework headers'"),
        ("framework-sysroot-absolute", "-iframeworkwithsysroot '/framework headers'"),
        ("framework-sysroot-absolute-joined", "'-iframeworkwithsysroot/framework headers'"),
    ):
        (caller / f"{name}.cflags").write_text(
            f"{common_flags}-isysroot 'test sdk' {option} "
            f"-DFEL748_EXPECT_FRAMEWORK=1 {regular}\n", encoding="utf-8")
    adapter = caller / "adapter"
    adapter.mkdir()
    shutil.copy2(fixture / "pkg-config", adapter / "pkg-config")
    (adapter / "pkg-config").chmod(0o755)
    shutil.copytree(repo / "tests/fixtures/core-rel-003/happy", caller / "pgo")
    (caller / "pgo/Seen.toml").write_text(
        'manifest-version = 1\n[project]\nname = "native_flags_pgo"\n'
        'version = "0.1.0"\nlanguage = "en"\nmodules = ["main.seen", "helper.seen"]\n',
        encoding="utf-8")


if __name__ == "__main__":
    main()
