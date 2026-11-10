#!/usr/bin/env python3
"""Transport same-source cross-built ARM inputs, never native qualification."""

from __future__ import annotations

import argparse
import gzip
import hashlib
import io
import os
import re
import secrets
import struct
import sys
import tarfile
import tomllib
import zlib
from pathlib import Path

import ci_bootstrap_candidate as candidate
from ci_candidate_io import (
    absolute, decode, directory, encode, entries, fail, metadata,
    publish_directory, read_at, read_file, rename_new, unchanged_directory, write_at,
)
from release_toolchain_artifact import ArtifactError

SCHEMA = "seen-linux-arm64-inputs-v1"
MANIFEST_MAX = 1024 * 1024
TOTAL_MAX = 64 * 1024 * 1024
BINARY_LIMITS = dict(candidate.BINARY_LIMITS)
MEMBERS = {**BINARY_LIMITS, "native-candidate.json": MANIFEST_MAX}


def elf(data: bytes, machine: int = 183) -> None:
    """Require a linked ELF64 LE image, not a renamed object or IR file."""
    if len(data) < 64 or data[:7] != b"\x7fELF\x02\x01\x01":
        fail("input is not ELF64 little-endian")
    kind, actual, version, entry, phoff, _, _, ehsize, phsize, phnum = struct.unpack_from(
        "<HHIQQQIHHH", data, 16)
    if (kind not in (2, 3) or actual != machine or version != 1 or ehsize != 64
            or phsize != 56 or not 1 <= phnum < 65535 or phoff < 64
            or phoff + phnum * phsize > len(data)):
        fail("input is not a linked executable of the required ELF machine")
    executable = False
    for index in range(phnum):
        segment, flags, offset, address, _, filesz, memsz, align = struct.unpack_from(
            "<IIQQQQQQ", data, phoff + index * phsize)
        if segment == 1:
            if (filesz > memsz or offset + filesz > len(data)
                    or (align > 1 and (align & (align - 1) or offset % align != address % align))):
                fail("invalid ELF load segment")
            executable |= bool(flags & 1 and filesz and address <= entry < address + filesz)
    if not executable:
        fail("ELF entry point is not in a file-backed executable load segment")


def source_version(root: Path) -> str:
    try:
        value = tomllib.loads(read_file(root / "Seen.toml", MANIFEST_MAX).decode("utf-8"))["project"]["version"]
    except (UnicodeDecodeError, tomllib.TOMLDecodeError, KeyError, TypeError) as error:
        fail(f"invalid source version: {error}")
    if not isinstance(value, str) or not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", value):
        fail("invalid source version")
    return value


def binding(root: Path, candidate_dir: Path) -> dict:
    """Canonical x64 verification is provenance only; never execute that ELF."""
    value, raw, binaries = candidate.verify(root, candidate_dir)
    for data in binaries.values():
        elf(data, 62)
    result = {
        "schema": SCHEMA, "status": "cross-built-unverified", "target": "linux-arm64",
        "source_version": source_version(root),
        "source_commit": value["source_commit"], "source_tree": value["source_tree"],
        "origin_candidate": {
            "manifest_sha256": hashlib.sha256(raw).hexdigest(),
            "compiler_sha256": hashlib.sha256(binaries["seen"]).hexdigest(),
            "client_sha256": hashlib.sha256(binaries["seen-pkg"]).hexdigest(),
        },
    }
    if candidate.source_identity(root)["source_commit"] != result["source_commit"]:
        fail("source changed during ARM provenance binding")
    return result


def verify_payload(root: Path, candidate_dir: Path, raw: bytes,
                   binaries: dict[str, bytes], expected: dict | None = None) -> dict:
    value = candidate.require_keys(decode(raw), {
        "schema", "status", "target", "source_version", "source_commit", "source_tree",
        "origin_candidate", "files",
    }, "ARM input manifest")
    current = binding(root, candidate_dir)
    if expected is not None and current != expected:
        fail("source or origin candidate changed during ARM input operation")
    if any(value[key] != current[key] for key in current):
        fail("ARM input source or origin candidate identity mismatch")
    files = candidate.require_keys(value["files"], set(BINARY_LIMITS), "ARM input files")
    for name, maximum in BINARY_LIMITS.items():
        elf(binaries[name])
        if metadata(binaries[name]) != candidate.valid_metadata(files[name], maximum):
            fail(f"ARM payload digest or size mismatch: {name}")
    if len(raw) + sum(map(len, binaries.values())) > TOTAL_MAX:
        fail("ARM inputs exceed total size limit")
    return value


def verify(root: Path, candidate_dir: Path, path: Path) -> tuple[dict, bytes, dict[str, bytes]]:
    candidate.inside(root, path)
    with directory(path) as fd:
        entries(fd, set(MEMBERS))
        raw = read_at(fd, "native-candidate.json", MANIFEST_MAX)
        binaries = {name: read_at(fd, name, maximum, executable=True)
                    for name, maximum in BINARY_LIMITS.items()}
        value = verify_payload(root, candidate_dir, raw, binaries)
        # Re-read bytes, not merely the manifest or a self-authorized cache.
        for name, data in {**binaries, "native-candidate.json": raw}.items():
            if read_at(fd, name, MEMBERS[name], executable=name in BINARY_LIMITS) != data:
                fail("ARM input changed during verification")
        entries(fd, set(MEMBERS))
        unchanged_directory(path, fd)
        return value, raw, binaries


def publish_archive(output: Path, data: bytes) -> None:
    with directory(output.parent, create=True) as parent:
        temporary = ".arm-inputs." + secrets.token_hex(12)
        try:
            write_at(parent, temporary, data, 0o644)
            unchanged_directory(output.parent, parent)
            rename_new(parent, temporary, output.name)
            os.fsync(parent)
        finally:
            try:
                os.unlink(temporary, dir_fd=parent)
            except FileNotFoundError:
                pass


def create(root: Path, candidate_dir: Path, compiler: Path, client: Path, output: Path) -> None:
    candidate.ignored_output(root, output)
    expected = binding(root, candidate_dir)
    binaries = {"seen": read_file(compiler, BINARY_LIMITS["seen"], executable=True),
                "seen-pkg": read_file(client, BINARY_LIMITS["seen-pkg"], executable=True)}
    raw = encode({**expected, "files": {name: metadata(data) for name, data in binaries.items()}})
    verify_payload(root, candidate_dir, raw, binaries, expected)
    buffer = io.BytesIO()
    with tarfile.open(fileobj=buffer, mode="w:", format=tarfile.USTAR_FORMAT) as archive:
        for name, data in {**binaries, "native-candidate.json": raw}.items():
            info = tarfile.TarInfo(name)
            info.size = len(data)
            info.mode = 0o755 if name in BINARY_LIMITS else 0o644
            archive.addfile(info, io.BytesIO(data))
    packed = gzip.compress(buffer.getvalue(), mtime=0)
    if len(buffer.getvalue()) > TOTAL_MAX or len(packed) > TOTAL_MAX:
        fail("ARM archive exceeds total size limit")
    if binding(root, candidate_dir) != expected:
        fail("source or origin candidate changed before ARM archive publication")
    publish_archive(output, packed)


def unpack(root: Path, candidate_dir: Path, archive_path: Path, output: Path) -> None:
    candidate.ignored_output(root, output)
    expected = binding(root, candidate_dir)
    packed = read_file(archive_path, TOTAL_MAX)
    with gzip.GzipFile(fileobj=io.BytesIO(packed)) as compressed:
        data = compressed.read(TOTAL_MAX + 1)
    if len(data) > TOTAL_MAX:
        fail("ARM archive expands beyond size limit")
    payload = {}
    offset = 0
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:") as archive:
        for member in archive:
            mode = 0o755 if member.name in BINARY_LIMITS else 0o644
            if (member.name not in MEMBERS or member.name in payload
                    or member.type not in (tarfile.REGTYPE, tarfile.AREGTYPE)
                    or member.issparse() or member.pax_headers or archive.pax_headers
                    or member.linkname or member.mode != mode
                    or member.offset != offset or member.offset_data != offset + 512
                    or not 1 <= member.size <= MEMBERS[member.name]):
                fail("unsafe ARM archive member set, type, mode, header, or size")
            source = archive.extractfile(member)
            if source is None:
                fail("unreadable ARM archive member")
            with source:
                content = source.read(MEMBERS[member.name] + 1)
            if len(content) != member.size:
                fail("ARM archive member size mismatch")
            payload[member.name] = content
            offset = member.offset_data + ((member.size + 511) // 512) * 512
            if any(data[member.offset_data + member.size:offset]):
                fail("nonzero ARM archive member padding")
    if (set(payload) != set(MEMBERS) or len(data) % 512
            or len(data) - offset < 1024 or any(data[offset:])):
        fail("invalid ARM archive member set or end markers")
    verify_payload(root, candidate_dir, payload["native-candidate.json"],
                   {name: payload[name] for name in BINARY_LIMITS}, expected)
    publish_directory(output, {name: (content, 0o755 if name in BINARY_LIMITS else 0o644)
                               for name, content in payload.items()})


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="operation", required=True)
    for operation in ("create", "unpack", "verify"):
        command = subparsers.add_parser(operation)
        command.add_argument("--root", type=Path, required=True)
        command.add_argument("--candidate", type=Path, required=True)
        if operation == "create":
            command.add_argument("--compiler", type=Path, required=True)
            command.add_argument("--client", type=Path, required=True)
        if operation == "unpack":
            command.add_argument("--archive", type=Path, required=True)
        command.add_argument("--directory" if operation == "verify" else "--output",
                             type=Path, required=True)
    args = parser.parse_args()
    try:
        absolute(args.root)
        if args.operation == "create":
            create(args.root, args.candidate, args.compiler, args.client, args.output)
        elif args.operation == "unpack":
            unpack(args.root, args.candidate, args.archive, args.output)
        else:
            value, _, _ = verify(args.root, args.candidate, args.directory)
            print(encode(value).decode(), end="")
        return 0
    except (ArtifactError, OSError, ValueError, tarfile.TarError, EOFError, zlib.error) as error:
        print(f"linux-arm64-inputs: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
