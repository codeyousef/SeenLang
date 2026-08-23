#!/usr/bin/env python3
"""Create and install the exact portable toolchain certified by main CI."""

from __future__ import annotations

import argparse
import gzip
import hashlib
import io
import json
import os
import re
import shutil
import stat
import sys
import tarfile
import tempfile
from pathlib import Path


SCHEMA = "seen-release-toolchain-v1"
SHA = re.compile(r"[0-9a-f]{40}")
SHA256 = re.compile(r"[0-9a-f]{64}")
BASELINES = {"x86-64", "x86-64-v3"}
MAX_ARCHIVE_BYTES = 384 * 1024 * 1024
FILES = {
    "compiler": ("compiler_seen/target/seen", 256 * 1024 * 1024, 0o755),
    "package-client": ("compiler_seen/target/seen-pkg", 64 * 1024 * 1024, 0o755),
    "full-release.stamp": ("target/seen-build/full-release.stamp", 64 * 1024, 0o644),
}
ARCHIVE_PREFIX = "release-toolchain"
MANIFEST_NAME = f"{ARCHIVE_PREFIX}/manifest.json"


class ArtifactError(ValueError):
    pass


def fail(message: str) -> None:
    raise ArtifactError(f"core.004b.invalid: {message}")


def hash_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def parse_stamp(path: Path) -> dict[str, str]:
    if path.is_symlink() or not path.is_file() or path.stat().st_size > 64 * 1024:
        fail("full release stamp is missing or unsafe")
    values: dict[str, str] = {}
    try:
        for line in path.read_text(encoding="utf-8").splitlines():
            key, separator, value = line.partition("=")
            if not separator or not key or key in values:
                fail("full release stamp is malformed")
            values[key] = value
    except UnicodeDecodeError as error:
        fail(f"full release stamp is not UTF-8: {error}")
    return values


def validate_identity(commit: str, tree: str, baseline: str) -> None:
    if not SHA.fullmatch(commit) or not SHA.fullmatch(tree):
        fail("commit and tree must be lowercase 40-character hashes")
    if baseline not in BASELINES:
        fail("unsupported release CPU baseline")


def source_files(root: Path, commit: str, tree: str, baseline: str) -> dict[str, Path]:
    validate_identity(commit, tree, baseline)
    result: dict[str, Path] = {}
    for name, (relative, maximum, mode) in FILES.items():
        path = root / relative
        if path.is_symlink() or not path.is_file():
            fail(f"release toolchain input is missing or unsafe: {relative}")
        size = path.stat().st_size
        if size < 1 or size > maximum:
            fail(f"release toolchain input size is invalid: {relative}")
        if mode == 0o755 and not os.access(path, os.X_OK):
            fail(f"release toolchain input is not executable: {relative}")
        result[name] = path

    stamp = parse_stamp(result["full-release.stamp"])
    required = {
        "stamp_version", "commit", "tree", "compiler_sha256",
        "package_client_sha256", "release_cpu_baseline",
    }
    if not required.issubset(stamp) or stamp["stamp_version"] != "3":
        fail("full release stamp has an unsupported contract")
    if stamp["commit"] != commit or stamp["tree"] != tree:
        fail("full release stamp does not match the certified Git identity")
    if stamp["release_cpu_baseline"] != baseline:
        fail("full release stamp does not match the portable CPU baseline")
    if stamp["compiler_sha256"] != hash_file(result["compiler"]):
        fail("compiler hash does not match the full release stamp")
    if stamp["package_client_sha256"] != hash_file(result["package-client"]):
        fail("package-client hash does not match the full release stamp")
    return result


def manifest_for(paths: dict[str, Path], commit: str, tree: str, baseline: str) -> bytes:
    files = {
        name: {
            "bytes": path.stat().st_size,
            "mode": FILES[name][2],
            "sha256": hash_file(path),
        }
        for name, path in sorted(paths.items())
    }
    value = {
        "cpu_baseline": baseline,
        "files": files,
        "source_commit": commit,
        "source_tree": tree,
        "schema": SCHEMA,
    }
    return (json.dumps(value, separators=(",", ":"), sort_keys=True) + "\n").encode()


def create_archive(root: Path, output: Path, commit: str, tree: str, baseline: str) -> None:
    root = root.resolve(strict=True)
    paths = source_files(root, commit, tree, baseline)
    manifest = manifest_for(paths, commit, tree, baseline)
    output.parent.mkdir(parents=True, exist_ok=True)
    temporary = output.with_name(f".{output.name}.{os.getpid()}.tmp")
    try:
        with temporary.open("wb") as raw:
            with gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0) as compressed:
                with tarfile.open(fileobj=compressed, mode="w", format=tarfile.PAX_FORMAT) as archive:
                    manifest_info = tarfile.TarInfo(MANIFEST_NAME)
                    manifest_info.size = len(manifest)
                    manifest_info.mode = 0o644
                    manifest_info.mtime = manifest_info.uid = manifest_info.gid = 0
                    archive.addfile(manifest_info, io.BytesIO(manifest))
                    for name, path in sorted(paths.items()):
                        info = tarfile.TarInfo(f"{ARCHIVE_PREFIX}/{name}")
                        info.size = path.stat().st_size
                        info.mode = FILES[name][2]
                        info.mtime = info.uid = info.gid = 0
                        with path.open("rb") as source:
                            archive.addfile(info, source)
        if temporary.stat().st_size < 1 or temporary.stat().st_size > MAX_ARCHIVE_BYTES:
            fail("release toolchain archive size is invalid")
        os.replace(temporary, output)
    finally:
        temporary.unlink(missing_ok=True)


def no_duplicates(pairs: list[tuple[str, object]]) -> dict[str, object]:
    value: dict[str, object] = {}
    for key, item in pairs:
        if key in value:
            fail(f"duplicate release toolchain manifest field: {key}")
        value[key] = item
    return value


def load_manifest(archive: tarfile.TarFile, member: tarfile.TarInfo) -> dict[str, object]:
    if member.size < 1 or member.size > 64 * 1024:
        fail("release toolchain manifest size is invalid")
    source = archive.extractfile(member)
    if source is None:
        fail("release toolchain manifest is unreadable")
    try:
        value = json.loads(source.read().decode("utf-8"), object_pairs_hook=no_duplicates)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        fail(f"release toolchain manifest is invalid: {error}")
    if not isinstance(value, dict) or set(value) != {
        "cpu_baseline", "files", "source_commit", "source_tree", "schema"
    }:
        fail("release toolchain manifest has an unexpected shape")
    return value


def verify_manifest(value: dict[str, object], commit: str, tree: str, baseline: str) -> dict[str, dict[str, object]]:
    validate_identity(commit, tree, baseline)
    if (
        value["schema"] != SCHEMA
        or value["source_commit"] != commit
        or value["source_tree"] != tree
        or value["cpu_baseline"] != baseline
    ):
        fail("release toolchain manifest identity does not match the requested release")
    files = value["files"]
    if not isinstance(files, dict) or set(files) != set(FILES):
        fail("release toolchain manifest file set is invalid")
    for name, metadata in files.items():
        if not isinstance(metadata, dict) or set(metadata) != {"bytes", "mode", "sha256"}:
            fail(f"release toolchain metadata is malformed: {name}")
        maximum = FILES[name][1]
        if (
            isinstance(metadata["bytes"], bool)
            or not isinstance(metadata["bytes"], int)
            or not 1 <= metadata["bytes"] <= maximum
            or metadata["mode"] != FILES[name][2]
            or not isinstance(metadata["sha256"], str)
            or not SHA256.fullmatch(metadata["sha256"])
        ):
            fail(f"release toolchain metadata is invalid: {name}")
    return files  # type: ignore[return-value]


def validate_archive(
    archive_path: Path, commit: str, tree: str, baseline: str, install_root: Path | None
) -> None:
    if (
        archive_path.is_symlink()
        or not archive_path.is_file()
        or not 1 <= archive_path.stat().st_size <= MAX_ARCHIVE_BYTES
    ):
        fail("release toolchain archive is missing or unsafe")
    with tarfile.open(archive_path, mode="r:gz") as archive:
        members = archive.getmembers()
        expected = {MANIFEST_NAME, *(f"{ARCHIVE_PREFIX}/{name}" for name in FILES)}
        names = [member.name for member in members]
        if len(names) != len(set(names)) or set(names) != expected:
            fail("release toolchain archive entry set is invalid")
        for member in members:
            if not member.isfile() or member.issym() or member.islnk():
                fail(f"release toolchain archive entry is unsafe: {member.name}")
        manifest_member = next(member for member in members if member.name == MANIFEST_NAME)
        metadata = verify_manifest(load_manifest(archive, manifest_member), commit, tree, baseline)

        staging: Path | None = None
        try:
            if install_root is not None:
                install_root = install_root.resolve(strict=True)
                staging_parent = install_root / "target"
                if staging_parent.is_symlink():
                    fail("release toolchain staging directory is unsafe")
                staging_parent.mkdir(parents=True, exist_ok=True)
                staging = Path(tempfile.mkdtemp(prefix="release-toolchain.", dir=staging_parent))
            for name, (_relative, maximum, expected_mode) in FILES.items():
                member = archive.getmember(f"{ARCHIVE_PREFIX}/{name}")
                expected_metadata = metadata[name]
                if member.size != expected_metadata["bytes"] or member.size > maximum:
                    fail(f"release toolchain archive size mismatch: {name}")
                if stat.S_IMODE(member.mode) != expected_mode:
                    fail(f"release toolchain archive mode mismatch: {name}")
                source = archive.extractfile(member)
                if source is None:
                    fail(f"release toolchain archive entry is unreadable: {name}")
                digest = hashlib.sha256()
                destination = staging / name if staging is not None else None
                sink = destination.open("wb") if destination is not None else None
                try:
                    for chunk in iter(lambda: source.read(1024 * 1024), b""):
                        digest.update(chunk)
                        if sink is not None:
                            sink.write(chunk)
                finally:
                    if sink is not None:
                        sink.close()
                if digest.hexdigest() != expected_metadata["sha256"]:
                    fail(f"release toolchain archive hash mismatch: {name}")
                if destination is not None:
                    destination.chmod(expected_mode)
            if install_root is not None and staging is not None:
                for name, (relative, _maximum, _mode) in FILES.items():
                    destination = install_root / relative
                    if destination.parent.is_symlink():
                        fail(f"release toolchain destination is unsafe: {relative}")
                    destination.parent.mkdir(parents=True, exist_ok=True)
                    os.replace(staging / name, destination)
        finally:
            if staging is not None:
                shutil.rmtree(staging, ignore_errors=True)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    create = subparsers.add_parser("create")
    create.add_argument("--root", required=True, type=Path)
    create.add_argument("--output", required=True, type=Path)
    verify = subparsers.add_parser("verify")
    verify.add_argument("--archive", required=True, type=Path)
    install = subparsers.add_parser("install")
    install.add_argument("--archive", required=True, type=Path)
    install.add_argument("--root", required=True, type=Path)
    for child in (create, verify, install):
        child.add_argument("--commit", required=True)
        child.add_argument("--tree", required=True)
        child.add_argument("--cpu-baseline", required=True)
    args = parser.parse_args()
    try:
        if args.command == "create":
            create_archive(args.root, args.output, args.commit, args.tree, args.cpu_baseline)
        else:
            validate_archive(
                args.archive, args.commit, args.tree, args.cpu_baseline,
                args.root if args.command == "install" else None,
            )
        return 0
    except (ArtifactError, OSError, tarfile.TarError) as error:
        print(f"release-toolchain: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
