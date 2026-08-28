#!/usr/bin/env python3
"""Certify CORE-004G evidence against two independent builder roots."""

import argparse
import hashlib
import importlib.util
import os
import shutil
import stat
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "program_reproducibility_checker",
    ROOT / "scripts/check_program_reproducibility.py",
)
checker = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(checker)

MAX_PUBLIC_KEY_BYTES = 65_536
MAX_SIGNATURE_BYTES = 4_096
ED25519_SIGNATURE_BYTES = 64
ED25519_SPKI_PREFIX = bytes.fromhex("302a300506032b6570032100")
ED25519_SPKI_BYTES = len(ED25519_SPKI_PREFIX) + 32


def safe_directory(path):
    try:
        metadata = path.lstat()
    except OSError:
        checker.fail("io", "certification directory metadata is unavailable")
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
        checker.fail("invalid", "certification directory is unsafe")
    try:
        return path.resolve(strict=True)
    except OSError:
        checker.fail("io", "certification directory cannot be resolved")


def path_identity(path):
    return hashlib.sha256(os.fsencode(str(path))).hexdigest()


def inspect_regular(path, minimum, maximum, executable=False):
    flags = os.O_RDONLY | os.O_CLOEXEC
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError:
        checker.fail("io", "certification input cannot be opened safely")
    try:
        before = os.fstat(descriptor)
        if not stat.S_ISREG(before.st_mode) or not minimum <= before.st_size <= maximum:
            checker.fail("limit", "certification input size is outside policy")
        if executable and before.st_mode & 0o111 == 0:
            checker.fail("invalid", "certified executable is not executable")
        digest_value = hashlib.sha256()
        total = 0
        while True:
            chunk = os.read(descriptor, 1_048_576)
            if not chunk:
                break
            digest_value.update(chunk)
            total += len(chunk)
        after = os.fstat(descriptor)
        identity_before = (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns)
        identity_after = (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns)
        if identity_before != identity_after or total != before.st_size:
            checker.fail("input_drift", "certification input changed while it was read")
        return digest_value.hexdigest(), total
    finally:
        os.close(descriptor)


def read_regular(path, minimum, maximum):
    flags = os.O_RDONLY | os.O_CLOEXEC
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError:
        checker.fail("io", "certification input cannot be opened safely")
    try:
        before = os.fstat(descriptor)
        if not stat.S_ISREG(before.st_mode) or not minimum <= before.st_size <= maximum:
            checker.fail("limit", "certification input size is outside policy")
        chunks = []
        total = 0
        while True:
            chunk = os.read(descriptor, min(1_048_576, maximum - total + 1))
            if not chunk:
                break
            total += len(chunk)
            if total > maximum:
                checker.fail("limit", "certification input exceeded its byte limit")
            chunks.append(chunk)
        after = os.fstat(descriptor)
        identity_before = (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns)
        identity_after = (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns)
        if identity_before != identity_after or total != before.st_size:
            checker.fail("input_drift", "certification input changed while it was read")
        return b"".join(chunks)
    finally:
        os.close(descriptor)


def direct_child(root, name):
    if not checker.safe_name(name):
        checker.fail("invalid", "certification input name is unsafe")
    return root / name


def require_pin(root, name, expected_digest, expected_bytes=None, executable=False):
    minimum = 1 if expected_bytes is None or expected_bytes > 0 else 0
    maximum = checker.MAX_ARTIFACT_BYTES
    if expected_bytes is not None and expected_bytes <= checker.MAX_CAPTURE_BYTES:
        maximum = checker.MAX_CAPTURE_BYTES
    actual_digest, actual_bytes = inspect_regular(
        direct_child(root, name), minimum, maximum, executable,
    )
    if actual_digest != expected_digest or (
        expected_bytes is not None and actual_bytes != expected_bytes
    ):
        checker.fail("raw_mismatch", "certification input does not match its raw-byte pin")


def openssl_command(arguments, *, error):
    try:
        result = subprocess.run(
            ["openssl", *arguments], stdout=subprocess.PIPE,
            stderr=subprocess.PIPE, timeout=30, check=False,
        )
    except (FileNotFoundError, subprocess.TimeoutExpired):
        checker.fail("unsigned", error)
    if result.returncode != 0:
        checker.fail("unsigned", error)
    return result.stdout


def verify_ed25519(inventory, public_key, signature, scratch_root):
    scratch = Path(tempfile.mkdtemp(prefix=".program-ed25519-", dir=scratch_root))
    try:
        inventory_path = scratch / "inventory.json"
        public_key_path = scratch / "public.pem"
        signature_path = scratch / "signature.bin"
        inventory_path.write_bytes(inventory)
        public_key_path.write_bytes(public_key)
        signature_path.write_bytes(signature)
        public_key_path.chmod(0o600)
        signature_path.chmod(0o600)
        der = openssl_command(
            ["pkey", "-pubin", "-in", str(public_key_path), "-outform", "DER"],
            error="builder public key is not a valid Ed25519 key",
        )
        if len(der) != ED25519_SPKI_BYTES or not der.startswith(ED25519_SPKI_PREFIX):
            checker.fail("unsigned", "builder public key is not Ed25519")
        openssl_command(
            ["pkeyutl", "-verify", "-pubin", "-inkey", str(public_key_path),
             "-rawin", "-in", str(inventory_path), "-sigfile", str(signature_path)],
            error="builder inventory signature verification failed",
        )
        return "ed25519-sha256:" + hashlib.sha256(der).hexdigest()
    finally:
        shutil.rmtree(scratch, ignore_errors=True)


def verify_builder(builder, root, cache_root, signature, public_key,
                   expected_identity, expected_issuer, scratch_root):
    if builder["build_root_digest"] != path_identity(root):
        checker.fail("root_identity", "builder root identity does not match evidence")
    if builder["cache_root_digest"] != path_identity(cache_root):
        checker.fail("root_identity", "builder cache identity does not match evidence")
    attestation = builder["attestation"]
    if attestation["mode"] == "keyless":
        checker.fail("unsigned", "keyless evidence requires an identity-token verifier")
    if attestation["mode"] not in ("key", "kms"):
        checker.fail("unsigned", "builder attestation mode is unsupported")
    if (not checker.text(expected_identity, 512) or
            not checker.text(expected_issuer, 512)):
        checker.fail("unsigned", "external identity or issuer pin is invalid")
    if (attestation["identity"] != expected_identity or
            attestation["issuer"] != expected_issuer):
        checker.fail("unsigned", "builder identity or issuer does not match its external pin")
    inventory = checker.builder_inventory_payload(builder)
    if attestation["inventory_sha256"] != hashlib.sha256(inventory).hexdigest():
        checker.fail("unsigned", "builder inventory attestation is stale")
    signature_payload = read_regular(signature, 1, MAX_SIGNATURE_BYTES)
    if len(signature_payload) != ED25519_SIGNATURE_BYTES:
        checker.fail("unsigned", "builder signature is not an Ed25519 signature")
    if hashlib.sha256(signature_payload).hexdigest() != attestation["signature_sha256"]:
        checker.fail("unsigned", "builder signature sidecar does not match evidence")
    public_key_payload = read_regular(public_key, 1, MAX_PUBLIC_KEY_BYTES)
    verified_identity = verify_ed25519(
        inventory, public_key_payload, signature_payload, scratch_root,
    )
    if verified_identity != expected_identity:
        checker.fail("unsigned", "builder public key does not match its external identity pin")
    require_pin(root, builder["compiler"], builder["compiler_sha256"], executable=True)
    require_pin(
        root,
        builder["installed_archive"],
        builder["installed_archive_sha256"],
    )
    for result in builder["programs"]:
        for run in result["runs"]:
            require_pin(
                root, run["artifact"], run["artifact_sha256"],
                run["artifact_bytes"], executable=True,
            )
            require_pin(
                root, run["manifest"], run["manifest_sha256"],
                run["manifest_bytes"],
            )
            require_pin(
                root, run["stdout"], run["stdout_sha256"], run["stdout_bytes"],
            )
            require_pin(
                root, run["stderr"], run["stderr_sha256"], run["stderr_bytes"],
            )
    return verified_identity


def atomic_write(path, payload, cancelled=False):
    parent = safe_directory(path.parent)
    try:
        existing = path.lstat()
        if stat.S_ISLNK(existing.st_mode) or not stat.S_ISREG(existing.st_mode):
            checker.fail("invalid", "certification output path is unsafe")
    except FileNotFoundError:
        pass
    except OSError:
        checker.fail("io", "certification output metadata is unavailable")
    descriptor, temporary = tempfile.mkstemp(prefix=".program-certification-", dir=parent)
    try:
        with os.fdopen(descriptor, "wb") as stream:
            stream.write(payload)
            stream.flush()
            os.fsync(stream.fileno())
        if cancelled:
            checker.fail("cancelled", "program certification was cancelled")
        os.replace(temporary, path)
        directory = os.open(parent, os.O_RDONLY | os.O_DIRECTORY)
        try:
            os.fsync(directory)
        finally:
            os.close(directory)
    finally:
        try:
            os.unlink(temporary)
        except FileNotFoundError:
            pass


def certify(args):
    raw = read_regular(args.candidate, 1, checker.MAX_EVIDENCE_BYTES)
    value = checker.validate(raw)
    root_a = safe_directory(args.builder_a_root)
    root_b = safe_directory(args.builder_b_root)
    cache_a = safe_directory(args.cache_a_root)
    cache_b = safe_directory(args.cache_b_root)
    scratch_root = safe_directory(args.output.parent)
    if len({root_a, root_b, cache_a, cache_b}) != 4:
        checker.fail("root_identity", "two independent build and cache roots are required")
    identity_a = verify_builder(
        value["builders"][0], root_a, cache_a, args.signature_a,
        args.public_key_a, args.expected_identity_a, args.expected_issuer_a,
        scratch_root,
    )
    identity_b = verify_builder(
        value["builders"][1], root_b, cache_b, args.signature_b,
        args.public_key_b, args.expected_identity_b, args.expected_issuer_b,
        scratch_root,
    )
    if identity_a == identity_b:
        checker.fail("unsigned", "two builders require distinct pinned signing keys")
    atomic_write(args.output, checker.canonical(value), args.test_cancel_before_commit)
    return value


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--builder-a-root", type=Path, required=True)
    parser.add_argument("--builder-b-root", type=Path, required=True)
    parser.add_argument("--cache-a-root", type=Path, required=True)
    parser.add_argument("--cache-b-root", type=Path, required=True)
    parser.add_argument("--signature-a", type=Path, required=True)
    parser.add_argument("--signature-b", type=Path, required=True)
    parser.add_argument("--public-key-a", type=Path, required=True)
    parser.add_argument("--public-key-b", type=Path, required=True)
    parser.add_argument("--expected-identity-a", required=True)
    parser.add_argument("--expected-identity-b", required=True)
    parser.add_argument("--expected-issuer-a", required=True)
    parser.add_argument("--expected-issuer-b", required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--test-cancel-before-commit", action="store_true")
    args = parser.parse_args(argv)
    try:
        certify(args)
        print(f"PASS: CORE-004G two-builder program evidence: {args.output}")
    except checker.ContractError as error:
        print(f"core.004g.{error.code}: {error}", file=sys.stderr)
        return 130 if error.code == "cancelled" else 1
    except OSError as error:
        print(f"core.004g.io: {type(error).__name__}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
