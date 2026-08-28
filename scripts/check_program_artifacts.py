#!/usr/bin/env python3
"""Validate and compare canonical CORE-004F user-program artifacts."""

import argparse
import hashlib
import json
import os
import random
import stat
import sys
import tempfile
import time
from pathlib import Path

SCHEMA = "seen-program-artifacts-v1"
CASES = {
    "two-root-default", "cold-warm-no-cache", "release-full-lto",
    "release-thin-lto", "object-manifest", "package-graph",
    "installed-compiler", "path-remap",
}
RESULT_IDS = (
    "root-a-cold", "root-a-warm", "root-a-disabled",
    "root-b-cold", "root-b-warm", "root-b-disabled",
)
CACHE_MODES = ("cold", "warm", "disabled", "cold", "warm", "disabled")
CACHE_STATES = ("miss", "hit", "disabled", "miss", "hit", "disabled")
COMPILER_ORIGINS = ("checkout", "checkout", "checkout", "installed", "installed", "installed")
ENVIRONMENT_NAMES = {
    "LANG", "LC_ALL", "SEEN_DETERMINISTIC_SEED", "SEEN_HASH_SEED",
    "SOURCE_DATE_EPOCH", "TZ",
}
ROLE_ORDER = {"executable": 0, "object": 1, "object-manifest": 2, "package-archive": 3, "package-manifest": 4}
MAX_EVIDENCE_BYTES = 8_388_608
MAX_ARTIFACT_BYTES = 2_147_483_648
MAX_MANIFEST_BYTES = 16_777_216

FIELDS = {"build_input", "cancelled", "case", "results", "schema"}
INPUT_FIELDS = {
    "compiler_sha256", "coverage", "cpu", "debug", "environment", "features",
    "input_digest", "lock_sha256", "lto", "object_cache_abi",
    "package_client_sha256", "path_map", "pic", "profile", "runtime_sha256",
    "sanitizer", "simd", "source_date_epoch", "source_tree_sha256",
    "stdlib_sha256", "target", "toolchain", "triple",
}
ENVIRONMENT_FIELDS = {"name", "value_sha256"}
RESULT_FIELDS = {
    "artifacts", "build_input_digest", "cache", "cache_identity", "cache_mode",
    "compiler_origin", "exit_code", "id", "path_map", "published_atomically",
    "root_identity", "stderr_sha256", "stdout_sha256",
}
ARTIFACT_FIELDS = {"bytes", "executable", "name", "role", "sha256"}
CACHE_FIELDS = {"artifact_set_sha256", "input_digest", "key", "state"}


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


def digest(value):
    return (
        isinstance(value, str)
        and len(value) == 64
        and all(character in "0123456789abcdef" for character in value)
    )


def text(value, maximum):
    return (
        isinstance(value, str)
        and 0 < len(value) <= maximum
        and all(32 <= ord(character) <= 126 for character in value)
    )


def safe_name(value):
    if not text(value, 255) or value in (".", "..") or value.startswith("-"):
        return False
    return all(character.isalnum() or character in "._+-" for character in value)


def canonical_json(value):
    return (json.dumps(value, separators=(",", ":"), sort_keys=True) + "\n").encode("utf-8")


def sha256(payload):
    return hashlib.sha256(payload).hexdigest()


def build_input_digest(value):
    unsigned = dict(value)
    unsigned.pop("input_digest", None)
    return sha256(canonical_json(unsigned))


def artifact_set_digest(artifacts):
    return sha256(canonical_json(artifacts))


def cache_key(case, input_digest):
    frame = b"seen-program-cache-v1\0" + case.encode("ascii") + b"\0" + input_digest.encode("ascii")
    return sha256(frame)


def validate_input(value, case):
    if not isinstance(value, dict) or set(value) != INPUT_FIELDS:
        fail("invalid", "build-input fields are invalid")
    for field in (
        "compiler_sha256", "lock_sha256", "package_client_sha256",
        "runtime_sha256", "source_tree_sha256", "stdlib_sha256",
    ):
        if not digest(value[field]):
            fail("invalid", f"build-input digest is invalid: {field}")
    if not digest(value["input_digest"]) or value["input_digest"] != build_input_digest(value):
        fail("input_change", "canonical build-input digest does not match its fields")
    if (
        value["profile"] != "deterministic"
        or value["target"] != "linux-x86_64"
        or value["triple"] != "x86_64-unknown-linux-gnu"
        or value["path_map"] != "/seen/src"
        or value["simd"] not in ("none", "sse4.2", "avx2", "avx512")
        or value["lto"] not in ("none", "full", "thin")
        or not text(value["cpu"], 64)
        or not text(value["toolchain"], 256)
        or not text(value["object_cache_abi"], 64)
        or not value["object_cache_abi"].startswith("seen-object-cache-abi-v")
    ):
        fail("invalid", "build-input target or tool identity is invalid")
    if not integer(value["source_date_epoch"]) or value["source_date_epoch"] < 0:
        fail("limit", "source-date epoch is invalid")
    for field in ("coverage", "debug", "pic"):
        if not isinstance(value[field], bool):
            fail("invalid", f"build-input boolean is invalid: {field}")
    if (
        value["cpu"] == "native"
        or value["debug"]
        or value["coverage"]
        or value["sanitizer"] != "none"
    ):
        fail("unsupported_combination", "build mode cannot guarantee raw reproducibility")
    expected_lto = "none"
    if case == "release-full-lto":
        expected_lto = "full"
    elif case == "release-thin-lto":
        expected_lto = "thin"
    if value["lto"] != expected_lto:
        fail("invalid", "case and LTO mode disagree")
    if case == "object-manifest" and not value["pic"]:
        fail("invalid", "object-manifest certification requires PIC objects")
    features = value["features"]
    if not isinstance(features, list) or len(features) > 64:
        fail("limit", "feature set exceeds policy")
    if any(not isinstance(item, str) or not text(item, 128) for item in features):
        fail("invalid", "feature set contains an invalid name")
    if features != sorted(set(features)):
        fail("invalid", "feature set is not canonical")
    environment = value["environment"]
    if not isinstance(environment, list):
        fail("invalid", "environment pin set is invalid")
    if len(environment) != len(ENVIRONMENT_NAMES):
        fail("invalid", "complete deterministic environment pins are required")
    names = []
    for entry in environment:
        if not isinstance(entry, dict) or set(entry) != ENVIRONMENT_FIELDS:
            fail("invalid", "environment pin fields are invalid")
        if entry["name"] not in ENVIRONMENT_NAMES or not digest(entry["value_sha256"]):
            fail("invalid", "environment pin is invalid")
        names.append(entry["name"])
    if names != sorted(ENVIRONMENT_NAMES):
        fail("invalid", "environment pins are incomplete, duplicated, or unordered")
    return value


def validate_artifacts(value):
    if not isinstance(value, list) or not 1 <= len(value) <= 64:
        fail("limit", "artifact set is empty or oversized")
    names = set()
    prior_identity = None
    executable_count = 0
    for artifact in value:
        if not isinstance(artifact, dict) or set(artifact) != ARTIFACT_FIELDS:
            fail("invalid", "artifact pin fields are invalid")
        role = artifact["role"]
        if not isinstance(role, str) or role not in ROLE_ORDER:
            fail("invalid", "artifact role is invalid")
        identity = (ROLE_ORDER[role], artifact["name"])
        if prior_identity is not None and identity <= prior_identity:
            fail("invalid", "artifact roles or names are not canonical")
        prior_identity = identity
        if not safe_name(artifact["name"]) or artifact["name"] in names:
            fail("invalid", "artifact name is unsafe or duplicated")
        names.add(artifact["name"])
        if not digest(artifact["sha256"]):
            fail("invalid", "artifact digest is invalid")
        if not integer(artifact["bytes"]) or not 1 <= artifact["bytes"] <= MAX_ARTIFACT_BYTES:
            fail("limit", "artifact size is outside policy")
        if not isinstance(artifact["executable"], bool):
            fail("invalid", "artifact executable state is invalid")
        if artifact["role"] == "executable":
            executable_count += 1
            if not artifact["executable"]:
                fail("invalid", "program executable pin is not executable")
        elif artifact["executable"]:
            fail("invalid", "non-executable artifact has executable authority")
    return executable_count


def validate_result(value, index, build_input, case):
    if not isinstance(value, dict) or set(value) != RESULT_FIELDS:
        fail("invalid", "build result fields are invalid")
    if (
        value["id"] != RESULT_IDS[index]
        or value["cache_mode"] != CACHE_MODES[index]
        or value["compiler_origin"] != COMPILER_ORIGINS[index]
    ):
        fail("invalid", "build-result order or identity is not canonical")
    if (
        not digest(value["root_identity"])
        or not digest(value["cache_identity"])
        or not digest(value["stdout_sha256"])
        or not digest(value["stderr_sha256"])
        or value["path_map"] != build_input["path_map"]
        or value["published_atomically"] is not True
    ):
        fail("invalid", "build-result identity or publication state is invalid")
    if not integer(value["exit_code"]) or not 0 <= value["exit_code"] <= 255:
        fail("limit", "build-result exit code is invalid")
    if value["exit_code"] != 0:
        fail("mismatch", "successful artifact evidence requires exit code zero")
    if value["build_input_digest"] != build_input["input_digest"]:
        fail("input_change", "build result used a different input identity")
    executable_count = validate_artifacts(value["artifacts"])
    if case == "object-manifest":
        roles = [artifact["role"] for artifact in value["artifacts"]]
        if "object" not in roles or "object-manifest" not in roles or executable_count:
            fail("invalid", "object-manifest case has the wrong artifact set")
    elif executable_count != 1:
        fail("invalid", "exactly one program executable is required")
    cache = value["cache"]
    if not isinstance(cache, dict) or set(cache) != CACHE_FIELDS:
        fail("invalid", "cache record fields are invalid")
    if (
        cache["state"] != CACHE_STATES[index]
        or cache["input_digest"] != build_input["input_digest"]
        or cache["key"] != cache_key(case, build_input["input_digest"])
        or cache["artifact_set_sha256"] != artifact_set_digest(value["artifacts"])
    ):
        fail("stale_cache", "cache record does not bind the complete artifact input")
    return value


def validate_value(value, cancelled=False):
    if not isinstance(value, dict) or set(value) != FIELDS:
        fail("invalid", "program-artifact evidence fields are invalid")
    if cancelled or value["cancelled"] is True:
        fail("cancelled", "program artifact certification was cancelled")
    if not isinstance(value["cancelled"], bool):
        fail("invalid", "cancellation state must be boolean")
    if (
        value["schema"] != SCHEMA
        or not isinstance(value["case"], str)
        or value["case"] not in CASES
    ):
        fail("invalid", "program-artifact schema or case is invalid")
    build_input = validate_input(value["build_input"], value["case"])
    results = value["results"]
    if not isinstance(results, list) or len(results) != len(RESULT_IDS):
        fail("limit", "six cold/warm/disabled two-root results are required")
    for index, result in enumerate(results):
        validate_result(result, index, build_input, value["case"])
    root_a = {result["root_identity"] for result in results[:3]}
    root_b = {result["root_identity"] for result in results[3:]}
    cache_a = {result["cache_identity"] for result in results[:3]}
    cache_b = {result["cache_identity"] for result in results[3:]}
    if any(len(group) != 1 for group in (root_a, root_b, cache_a, cache_b)):
        fail("root_identity", "each builder must retain one build and cache root identity")
    identities = root_a | root_b | cache_a | cache_b
    if len(identities) != 4:
        fail("root_identity", "build and cache roots are not independent")
    baseline = results[0]
    for result in results[1:]:
        if (
            result["artifacts"] != baseline["artifacts"]
            or result["stdout_sha256"] != baseline["stdout_sha256"]
            or result["stderr_sha256"] != baseline["stderr_sha256"]
            or result["exit_code"] != baseline["exit_code"]
        ):
            fail("mismatch", "cold, warm, disabled, checkout, or installed artifacts differ")
    return value


def validate(raw, max_bytes=MAX_EVIDENCE_BYTES, cancelled=False):
    if not integer(max_bytes) or not 1 <= max_bytes <= MAX_EVIDENCE_BYTES:
        fail("limit", "invalid evidence byte limit")
    if len(raw) > max_bytes:
        fail("limit", "program-artifact evidence byte limit exceeded")
    try:
        value = json.loads(raw.decode("utf-8"), object_pairs_hook=pairs)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        fail("invalid", f"invalid JSON: {type(error).__name__}")
    try:
        return validate_value(value, cancelled)
    except ContractError:
        raise
    except (KeyError, IndexError, TypeError, ValueError, OverflowError) as error:
        fail("invalid", f"invalid evidence value: {type(error).__name__}")


def canonical(value):
    return canonical_json(validate_value(value))


def inspect_regular(path, maximum, executable=False, capture=False):
    flags = os.O_RDONLY | os.O_CLOEXEC
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError:
        fail("io", "artifact cannot be opened safely")
    try:
        before = os.fstat(descriptor)
        if not stat.S_ISREG(before.st_mode) or not 1 <= before.st_size <= maximum:
            fail("limit", "artifact size is outside policy")
        if executable and before.st_mode & 0o111 == 0:
            fail("mismatch", "certified executable is not executable")
        digest_value = hashlib.sha256()
        chunks = []
        total = 0
        while True:
            chunk = os.read(descriptor, 1_048_576)
            if not chunk:
                break
            total += len(chunk)
            if total > maximum:
                fail("limit", "artifact exceeded its byte limit")
            digest_value.update(chunk)
            if capture:
                chunks.append(chunk)
        after = os.fstat(descriptor)
        before_identity = (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns)
        after_identity = (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns)
        if before_identity != after_identity or total != before.st_size:
            fail("input_change", "artifact changed while it was read")
        return digest_value.hexdigest(), total, b"".join(chunks)
    finally:
        os.close(descriptor)


def read_regular_bounded(path, maximum):
    """Read one stable regular file without following a swapped symbolic link."""
    flags = os.O_RDONLY | os.O_CLOEXEC
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError:
        fail("io", "evidence cannot be opened safely")
    try:
        before = os.fstat(descriptor)
        if not stat.S_ISREG(before.st_mode):
            fail("invalid", "evidence file is unsafe")
        if not 1 <= before.st_size <= maximum:
            fail("limit", "evidence file is empty or oversized")
        chunks = []
        total = 0
        while True:
            chunk = os.read(descriptor, min(1_048_576, maximum - total + 1))
            if not chunk:
                break
            total += len(chunk)
            if total > maximum:
                fail("limit", "evidence file exceeded its byte limit")
            chunks.append(chunk)
        after = os.fstat(descriptor)
        before_identity = (
            before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns,
        )
        after_identity = (
            after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns,
        )
        if before_identity != after_identity or total != before.st_size:
            fail("input_change", "evidence changed while it was read")
        return b"".join(chunks)
    finally:
        os.close(descriptor)


def verify_object_manifest(payload):
    try:
        text_value = payload.decode("ascii")
    except UnicodeDecodeError:
        fail("path_remap", "object manifest must be ASCII")
    lines = [line for line in text_value.splitlines() if line]
    if not lines or lines != sorted(set(lines)):
        fail("path_remap", "object manifest rows are empty, duplicated, or unordered")
    for line in lines:
        fields = line.split("\t")
        if len(fields) != 2 or not safe_name(fields[0]):
            fail("path_remap", "object manifest row is invalid")
        source = fields[1]
        if (
            not text(source, 4096)
            or source.startswith("/")
            or "\\" in source
            or any(part in ("", ".", "..") for part in source.split("/"))
        ):
            fail("path_remap", "object manifest leaked a physical source path")


def verify_files(value, result_roots):
    if set(result_roots) != set(RESULT_IDS):
        fail("limit", "every build result requires one artifact root")
    for result in value["results"]:
        root = result_roots[result["id"]]
        try:
            metadata = root.lstat()
        except OSError:
            fail("io", "artifact root metadata is unavailable")
        if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
            fail("invalid", "artifact root is unsafe")
        expected_names = {artifact["name"] for artifact in result["artifacts"]}
        actual_names = set()
        try:
            children = list(root.iterdir())
        except OSError:
            fail("io", "artifact root cannot be enumerated")
        for child in children:
            try:
                child_metadata = child.lstat()
            except OSError:
                fail("io", "artifact member metadata is unavailable")
            if stat.S_ISLNK(child_metadata.st_mode) or not stat.S_ISREG(
                child_metadata.st_mode
            ):
                fail("invalid", "artifact root contains an unsafe member")
            actual_names.add(child.name)
        if actual_names != expected_names:
            fail("mismatch", "artifact root and evidence member sets differ")
        for artifact in result["artifacts"]:
            digest_value, size, payload = inspect_regular(
                root / artifact["name"],
                MAX_MANIFEST_BYTES if artifact["role"] == "object-manifest" else MAX_ARTIFACT_BYTES,
                artifact["executable"],
                artifact["role"] == "object-manifest",
            )
            if digest_value != artifact["sha256"] or size != artifact["bytes"]:
                fail("mismatch", "artifact bytes do not match their pin")
            if artifact["role"] == "object-manifest":
                verify_object_manifest(payload)
    return value


def atomic_write(path, payload, cancelled=False):
    try:
        parent = path.parent.resolve(strict=True)
        metadata = parent.lstat()
    except OSError:
        fail("io", "output parent is unavailable")
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
        fail("invalid", "output parent is unsafe")
    try:
        existing = path.lstat()
        if stat.S_ISLNK(existing.st_mode) or not stat.S_ISREG(existing.st_mode):
            fail("invalid", "output path is unsafe")
    except FileNotFoundError:
        pass
    descriptor, temporary = tempfile.mkstemp(prefix=".program-artifacts-", dir=parent)
    try:
        with os.fdopen(descriptor, "wb") as stream:
            stream.write(payload)
            stream.flush()
            os.fsync(stream.fileno())
        if cancelled:
            fail("cancelled", "program artifact certification was cancelled")
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


def reject_output_alias(output, evidence, result_roots):
    try:
        output_path = output.resolve(strict=False)
        evidence_path = evidence.resolve(strict=True)
    except OSError:
        fail("io", "input or output identity is unavailable")
    if output_path == evidence_path:
        fail("invalid", "output must not replace its evidence input")
    for root in result_roots.values():
        try:
            root_path = root.resolve(strict=True)
        except OSError:
            fail("io", "artifact root identity is unavailable")
        if output_path == root_path or root_path in output_path.parents:
            fail("invalid", "output must remain outside certified artifact roots")


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
            validate(bytes(changed))
        except ContractError:
            rejected += 1
    return cases, rejected


def result_root(value):
    if "=" not in value:
        raise argparse.ArgumentTypeError("result root must be ID=PATH")
    identity, raw_path = value.split("=", 1)
    if identity not in RESULT_IDS or not raw_path:
        raise argparse.ArgumentTypeError("result root identity is invalid")
    return identity, Path(raw_path)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--evidence", type=Path, required=True)
    parser.add_argument("--result-root", action="append", default=[], type=result_root)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--max-bytes", type=int, default=MAX_EVIDENCE_BYTES)
    parser.add_argument("--fuzz-seconds", type=float, default=0)
    parser.add_argument("--seed", type=int, default=1101)
    parser.add_argument("--test-cancel-after-read", action="store_true")
    parser.add_argument("--test-cancel-before-commit", action="store_true")
    args = parser.parse_args(argv)
    try:
        if not 0 <= args.fuzz_seconds <= 300:
            fail("limit", "invalid fuzz duration")
        if not integer(args.max_bytes) or not 1 <= args.max_bytes <= MAX_EVIDENCE_BYTES:
            fail("limit", "invalid evidence byte limit")
        raw = read_regular_bounded(args.evidence, args.max_bytes)
        value = validate(raw, args.max_bytes, args.test_cancel_after_read)
        roots = dict(args.result_root)
        if len(roots) != len(args.result_root):
            fail("invalid", "result root identity is duplicated")
        if roots:
            verify_files(value, roots)
        if args.fuzz_seconds:
            cases, rejected = fuzz(raw, args.fuzz_seconds, args.seed)
            print(f"seed={args.seed} cases={cases} rejected={rejected}", file=sys.stderr)
        payload = canonical(value)
        if args.output:
            reject_output_alias(args.output, args.evidence, roots)
            atomic_write(args.output, payload, args.test_cancel_before_commit)
        else:
            sys.stdout.buffer.write(payload)
    except ContractError as error:
        print(f"core.004f.{error.code}: {error}", file=sys.stderr)
        return 130 if error.code == "cancelled" else 1
    except OSError as error:
        print(f"core.004f.io: {type(error).__name__}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
