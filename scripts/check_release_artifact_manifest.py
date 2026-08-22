#!/usr/bin/env python3
"""Validate and generate canonical CORE-004B signed-artifact manifests."""

import argparse
import hashlib
import json
import os
import random
import stat
import sys
import time
from pathlib import Path

SCHEMA = "seen-release-artifact-manifest-v1"
ROLES = ("compiler", "runtime", "stdlib", "package-client")
MAX_MANIFEST_BYTES = 1_048_576
MAX_ARTIFACT_BYTES = 2_147_483_648
FIELDS = {"artifacts", "schema", "signer", "source_commit", "source_digest", "target", "version"}
SIGNER_FIELDS = {"identity", "issuer", "mode"}
ARTIFACT_FIELDS = {"bundle", "bundle_sha256", "bytes", "checksum", "name", "role", "sha256"}


class ContractError(ValueError):
    def __init__(self, code, message):
        super().__init__(message)
        self.code = code


def fail(code, message):
    raise ContractError(code, message)


def pairs(items):
    value = {}
    for key, item in items:
        if key in value:
            fail("invalid", f"duplicate field: {key}")
        value[key] = item
    return value


def integer(value):
    return isinstance(value, int) and not isinstance(value, bool)


def digest(value, size=64):
    return isinstance(value, str) and len(value) == size and all(character in "0123456789abcdef" for character in value)


def printable(value, maximum):
    return isinstance(value, str) and 0 < len(value) <= maximum and all(32 <= ord(character) <= 126 for character in value)


def safe_name(value):
    if not printable(value, 255) or value in (".", "..") or value.startswith("-"):
        return False
    return "/" not in value and "\\" not in value and all(character.isalnum() or character in "._+-" for character in value)


def semver(value):
    if not printable(value, 64) or value.startswith("v") or "+" in value:
        return False
    core = value.split("-", 1)[0].split(".")
    if len(core) != 3:
        return False
    return all(part.isdigit() and (part == "0" or not part.startswith("0")) for part in core)


def parse(raw, max_bytes=MAX_MANIFEST_BYTES, cancelled=False):
    if not integer(max_bytes) or not 1 <= max_bytes <= MAX_MANIFEST_BYTES:
        fail("limit", "invalid manifest byte limit")
    if len(raw) > max_bytes:
        fail("limit", "release artifact manifest byte limit exceeded")
    try:
        value = json.loads(raw.decode("utf-8"), object_pairs_hook=pairs)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        fail("invalid", f"invalid JSON: {type(error).__name__}")
    if cancelled:
        fail("cancelled", "release artifact certification was cancelled")
    return validate(value)


def validate(value):
    if not isinstance(value, dict) or set(value) != FIELDS:
        fail("invalid", "release artifact manifest fields are invalid")
    if value["schema"] != SCHEMA or not semver(value["version"]):
        fail("invalid", "release artifact manifest identity is invalid")
    if value["target"] != "linux-x86_64":
        fail("platform", "release artifact target is unsupported")
    if not digest(value["source_commit"], 40) or not digest(value["source_digest"]):
        fail("invalid", "release source pin is invalid")
    signer = value["signer"]
    if not isinstance(signer, dict) or set(signer) != SIGNER_FIELDS:
        fail("invalid", "signer fields are invalid")
    if signer["mode"] not in ("keyless", "key", "kms") or not printable(signer["identity"], 512) or not printable(signer["issuer"], 512):
        fail("unsigned", "signer identity is missing or invalid")
    artifacts = value["artifacts"]
    if not isinstance(artifacts, list) or len(artifacts) != len(ROLES):
        fail("limit", "exactly four release artifacts are required")
    names = set()
    for index, artifact in enumerate(artifacts):
        if not isinstance(artifact, dict) or set(artifact) != ARTIFACT_FIELDS:
            fail("invalid", "release artifact fields are invalid")
        if artifact["role"] != ROLES[index]:
            fail("invalid", "release artifact role order is not canonical")
        if not safe_name(artifact["name"]) or artifact["name"] in names:
            fail("invalid", "release artifact name is unsafe or duplicated")
        names.add(artifact["name"])
        if artifact["checksum"] != artifact["name"] + ".sha256" or artifact["bundle"] != artifact["name"] + ".bundle":
            fail("unsigned", "release artifact sidecar names are not exact")
        if not digest(artifact["sha256"]) or not digest(artifact["bundle_sha256"]):
            fail("unsigned", "release artifact digest or signature bundle pin is invalid")
        if not integer(artifact["bytes"]) or not 1 <= artifact["bytes"] <= MAX_ARTIFACT_BYTES:
            fail("limit", "release artifact size is invalid")
    return value


def canonical(value):
    return (json.dumps(validate(value), separators=(",", ":"), sort_keys=True) + "\n").encode("utf-8")


def safe_file(path, maximum=MAX_ARTIFACT_BYTES):
    try:
        metadata = path.lstat()
    except OSError as error:
        fail("io", f"artifact metadata unavailable: {type(error).__name__}")
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISREG(metadata.st_mode):
        fail("invalid", f"artifact is not a regular non-symlink file: {path.name}")
    if not 1 <= metadata.st_size <= maximum:
        fail("limit", f"artifact size is outside policy: {path.name}")
    return metadata


def sha256(path):
    digest_value = hashlib.sha256()
    try:
        with path.open("rb") as source:
            while True:
                chunk = source.read(1024 * 1024)
                if not chunk:
                    break
                digest_value.update(chunk)
    except OSError as error:
        fail("io", f"artifact read failed: {type(error).__name__}")
    return digest_value.hexdigest()


def verify_files(value, directory):
    directory = directory.resolve()
    for artifact in value["artifacts"]:
        artifact_path = directory / artifact["name"]
        checksum_path = directory / artifact["checksum"]
        bundle_path = directory / artifact["bundle"]
        metadata = safe_file(artifact_path)
        safe_file(checksum_path, 1024)
        safe_file(bundle_path, MAX_MANIFEST_BYTES)
        if metadata.st_size != artifact["bytes"] or sha256(artifact_path) != artifact["sha256"]:
            fail("mismatch", f"artifact bytes do not match pin: {artifact['role']}")
        try:
            checksum = checksum_path.read_text(encoding="ascii").strip()
        except (OSError, UnicodeError) as error:
            fail("io", f"checksum read failed: {type(error).__name__}")
        if checksum != artifact["sha256"]:
            fail("mismatch", f"checksum sidecar does not match pin: {artifact['role']}")
        if sha256(bundle_path) != artifact["bundle_sha256"]:
            fail("mismatch", f"signature bundle does not match pin: {artifact['role']}")
    return value


def build(version, target, source_commit, source_digest, signer_mode, signer_identity, signer_issuer, artifacts):
    if tuple(role for role, _ in artifacts) != ROLES:
        fail("invalid", "artifact arguments must specify compiler, runtime, stdlib, package-client in order")
    entries = []
    for role, path in artifacts:
        metadata = safe_file(path)
        checksum_path = Path(str(path) + ".sha256")
        bundle_path = Path(str(path) + ".bundle")
        safe_file(checksum_path, 1024)
        safe_file(bundle_path, MAX_MANIFEST_BYTES)
        artifact_digest = sha256(path)
        try:
            checksum = checksum_path.read_text(encoding="ascii").strip()
        except (OSError, UnicodeError) as error:
            fail("io", f"checksum read failed: {type(error).__name__}")
        if checksum != artifact_digest:
            fail("mismatch", f"checksum does not pin artifact: {role}")
        entries.append({"bundle": path.name + ".bundle", "bundle_sha256": sha256(bundle_path),
            "bytes": metadata.st_size, "checksum": path.name + ".sha256", "name": path.name,
            "role": role, "sha256": artifact_digest})
    return validate({"artifacts": entries, "schema": SCHEMA,
        "signer": {"identity": signer_identity, "issuer": signer_issuer, "mode": signer_mode},
        "source_commit": source_commit, "source_digest": source_digest,
        "target": target, "version": version})


def fuzz(raw, seconds, seed):
    rng = random.Random(seed)
    deadline = time.monotonic() + seconds
    cases = rejected = 0
    while time.monotonic() < deadline:
        changed = bytearray(raw)
        if changed:
            changed[rng.randrange(len(changed))] = rng.randrange(256)
        cases += 1
        try:
            parse(bytes(changed))
        except ContractError:
            rejected += 1
    return cases, rejected


def artifact_argument(value):
    if "=" not in value:
        raise argparse.ArgumentTypeError("artifact must be ROLE=PATH")
    role, path = value.split("=", 1)
    return role, Path(path)


def atomic_write(path, payload, cancelled=False):
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.parent / ("." + path.name + f".tmp.{os.getpid()}")
    try:
        with temporary.open("xb") as output:
            output.write(payload)
            output.flush()
            os.fsync(output.fileno())
        if cancelled:
            fail("cancelled", "release artifact certification was cancelled")
        os.replace(temporary, path)
    finally:
        try:
            temporary.unlink()
        except FileNotFoundError:
            pass


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path)
    parser.add_argument("--artifact-dir", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--version")
    parser.add_argument("--target", default="linux-x86_64")
    parser.add_argument("--source-commit")
    parser.add_argument("--source-digest")
    parser.add_argument("--signer-mode", choices=("keyless", "key", "kms"))
    parser.add_argument("--signer-identity")
    parser.add_argument("--signer-issuer")
    parser.add_argument("--artifact", action="append", type=artifact_argument, default=[])
    parser.add_argument("--max-bytes", type=int, default=MAX_MANIFEST_BYTES)
    parser.add_argument("--fuzz-seconds", type=float, default=0)
    parser.add_argument("--seed", type=int, default=1101)
    parser.add_argument("--test-cancel-after-read", action="store_true")
    parser.add_argument("--test-cancel-before-commit", action="store_true")
    args = parser.parse_args(argv)
    try:
        if not 0 <= args.fuzz_seconds <= 300:
            fail("limit", "invalid fuzz duration")
        if args.manifest:
            safe_file(args.manifest, args.max_bytes)
            raw = args.manifest.read_bytes()
            value = parse(raw, args.max_bytes, args.test_cancel_after_read)
            if args.artifact_dir:
                verify_files(value, args.artifact_dir)
        else:
            required = (args.output, args.version, args.source_commit, args.source_digest,
                args.signer_mode, args.signer_identity, args.signer_issuer)
            if any(item is None for item in required):
                fail("invalid", "generation requires output, version, source pins, and signer identity")
            value = build(args.version, args.target, args.source_commit, args.source_digest,
                args.signer_mode, args.signer_identity, args.signer_issuer, args.artifact)
            raw = canonical(value)
            atomic_write(args.output, raw, args.test_cancel_before_commit)
        if args.fuzz_seconds:
            cases, rejected = fuzz(canonical(value), args.fuzz_seconds, args.seed)
            print(f"seed={args.seed} cases={cases} rejected={rejected}", file=sys.stderr)
        sys.stdout.buffer.write(canonical(value))
    except ContractError as error:
        print(f"core.004b.{error.code}: {error}", file=sys.stderr)
        return 130 if error.code == "cancelled" else 1
    except OSError as error:
        print(f"core.004b.io: {type(error).__name__}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
