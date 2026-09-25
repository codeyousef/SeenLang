#!/usr/bin/env python3
"""Bind locally built macOS/Windows inputs to one exact Seen release commit."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
import tarfile
import zipfile
from pathlib import Path


def fail(message: str) -> None:
    raise ValueError(f"release-platform-inputs: {message}")


def git(root: Path, spec: str) -> str:
    return subprocess.check_output(
        ["git", "-C", str(root), "rev-parse", spec], text=True
    ).strip()


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def names(version: str) -> tuple[str, str, str]:
    if not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", version):
        fail("invalid version")
    return (
        f"seen-{version}-macos-arm64.tar.gz",
        f"seen-{version}-windows-x64.zip",
        f"Seen-{version}-windows-x64-setup.exe",
    )


def validate_provenance(
    text: str, version: str, commit: str, platform: str,
    compiler: bytes, package_client: bytes,
) -> None:
    lines = text.splitlines()
    if any("=" not in line for line in lines):
        fail(f"{platform} provenance has an invalid line")
    values = dict(line.split("=", 1) for line in lines)
    if len(values) != len(lines):
        fail(f"{platform} provenance has duplicate fields")
    expected = {
        "release_version": version,
        "source_commit": commit,
        "platform": platform,
        "compiler_sha256": hashlib.sha256(compiler).hexdigest(),
        "package_client_sha256": hashlib.sha256(package_client).hexdigest(),
    }
    if values != expected:
        fail(f"{platform} archive provenance does not match its binaries or release")


def verify_payloads(directory: Path, version: str, commit: str) -> None:
    mac_name, windows_name, installer_name = names(version)
    mac_path = directory / mac_name
    windows_path = directory / windows_name
    installer_path = directory / installer_name
    for path in (mac_path, windows_path, installer_path):
        if not path.is_file() or path.is_symlink() or path.stat().st_size == 0:
            fail(f"missing, empty, or unsafe input: {path.name}")
    mac_root = f"seen-{version}-macos-arm64/"
    with tarfile.open(mac_path, "r:gz") as archive:
        member_list = archive.getmembers()
        members = {member.name: member for member in member_list}
        if len(members) != len(member_list):
            fail("macOS archive has duplicate entries")
        if any(
            (member.name != mac_root.rstrip("/") and not member.name.startswith(mac_root))
            or ".." in Path(member.name).parts
            or member.issym()
            or member.islnk()
            for member in members.values()
        ):
            fail("macOS archive has an unsafe entry")
        required = (
            mac_root + "bin/seen",
            mac_root + "bin/seen-pkg",
            mac_root + "bin/compatibility-manifest.json",
            mac_root + "share/seen/release-provenance.env",
        )
        if any(name not in members or not members[name].isfile() for name in required):
            fail("macOS archive lacks a required regular-file payload")
        provenance_file = archive.extractfile(members[required[-1]])
        compiler_file = archive.extractfile(members[required[0]])
        package_file = archive.extractfile(members[required[1]])
        manifest_file = archive.extractfile(members[required[2]])
        if any(value is None for value in (provenance_file, compiler_file, package_file, manifest_file)):
            fail("macOS archive members cannot be read")
        provenance = provenance_file.read(4096).decode("utf-8")
        compiler = compiler_file.read()
        package_client = package_file.read()
        if compiler[:4] not in (b"\xcf\xfa\xed\xfe", b"\xca\xfe\xba\xbe"):
            fail("macOS compiler is not a Mach-O executable")
        if json.load(manifest_file)["release_version"] != version:
            fail("macOS archive compatibility version differs")
        validate_provenance(provenance, version, commit, "macos-arm64", compiler, package_client)
    windows_root = f"seen-{version}-windows-x64/"
    with zipfile.ZipFile(windows_path) as archive:
        if len(set(archive.namelist())) != len(archive.namelist()):
            fail("Windows archive has duplicate entries")
        if any(
            not name.startswith(windows_root) or ".." in Path(name).parts
            for name in archive.namelist()
        ):
            fail("Windows archive has an unsafe entry")
        required = (
            windows_root + "bin/seen.exe",
            windows_root + "bin/seen-pkg.exe",
            windows_root + "bin/compatibility-manifest.json",
            windows_root + "share/seen/release-provenance.env",
        )
        if any(name not in archive.namelist() for name in required):
            fail("Windows archive lacks a required payload")
        compiler = archive.read(required[0])
        package_client = archive.read(required[1])
        if compiler[:2] != b"MZ":
            fail("Windows compiler is not a PE executable")
        if json.loads(archive.read(required[2]))["release_version"] != version:
            fail("Windows archive compatibility version differs")
        provenance = archive.read(required[-1]).decode("utf-8")
        validate_provenance(provenance, version, commit, "windows-x64", compiler, package_client)
    if installer_path.open("rb").read(2) != b"MZ":
        fail("Windows installer is not a PE executable")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=("create", "verify"))
    parser.add_argument("--root", required=True, type=Path)
    parser.add_argument("--version", required=True)
    parser.add_argument("--input-dir", required=True, type=Path)
    args = parser.parse_args()
    root = args.root.resolve(strict=True)
    directory = args.input_dir.resolve(strict=True)
    commit = git(root, "HEAD")
    tree = git(root, "HEAD^{tree}")
    verify_payloads(directory, args.version, commit)
    filenames = names(args.version)
    manifest_path = directory / f"seen-{args.version}-platform-inputs.json"
    expected = {
        "schema": "seen-platform-inputs-v1",
        "version": args.version,
        "source_commit": commit,
        "source_tree": tree,
        "assets": [
            {"name": name, "size": (directory / name).stat().st_size,
             "sha256": sha256(directory / name)}
            for name in filenames
        ],
    }
    if args.mode == "create":
        if manifest_path.exists():
            fail("platform input manifest already exists")
        manifest_path.write_text(
            json.dumps(expected, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
    else:
        if not manifest_path.is_file() or manifest_path.is_symlink():
            fail("platform input manifest is missing or unsafe")
        actual = json.loads(manifest_path.read_text(encoding="utf-8"))
        if actual != expected:
            fail("platform input manifest does not match files or source commit")
    print(f"PASS: three-platform input identity {args.version} {commit}")


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, KeyError, tarfile.TarError, zipfile.BadZipFile) as error:
        print(error, file=sys.stderr)
        sys.exit(1)
