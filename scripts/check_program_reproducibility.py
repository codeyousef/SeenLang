#!/usr/bin/env python3
"""Validate canonical CORE-004G two-builder program evidence."""

import argparse
import hashlib
import json
import random
import stat
import sys
import time
from pathlib import Path

SCHEMA = "seen-program-reproducibility-v1"
TARGET = "linux-x86_64"
PROGRAMS = (
    ("single-file", "default"),
    ("multi-module", "default"),
    ("locked-package", "default"),
    ("deterministic-runtime-context", "default"),
    ("default", "default"),
    ("release-full-lto", "release-full-lto"),
    ("release-thin-lto", "release-thin-lto"),
)
CACHE_MODES = ("cold", "warm", "disabled")
SIGNER_MODES = {"keyless", "key", "kms"}
MAX_EVIDENCE_BYTES = 8_388_608
MAX_ARTIFACT_BYTES = 2_147_483_648
MAX_CAPTURE_BYTES = 16_777_216
MAX_MEMORY_BYTES = 68_719_476_736

FIELDS = {
    "build_input_digest", "builders", "cancelled", "lock_digest", "programs",
    "schema", "source_commit", "source_date_epoch", "source_digest", "target",
}
PROGRAM_FIELDS = {
    "build_input_digest", "command", "expected_exit_code",
    "expected_stderr_sha256", "expected_stdout_sha256", "id", "lock_digest",
    "mode", "source_digest",
}
BUILDER_FIELDS = {
    "attestation", "build_root_digest", "builder_identity", "cache_root_digest",
    "compiler", "compiler_identity", "compiler_sha256", "containment", "id",
    "installed_archive", "installed_archive_sha256", "programs", "toolchain",
}
ATTESTATION_FIELDS = {
    "identity", "inventory_sha256", "issuer", "mode", "signature_sha256",
}
CONTAINMENT_FIELDS = {
    "jobs", "memory_available_bytes", "memory_max_bytes",
    "memory_max_readback_bytes", "memory_swap_max_bytes",
    "memory_swap_max_readback_bytes", "opt_jobs", "pids_max",
    "pids_max_readback", "tasks_current",
}
RESULT_FIELDS = {"build_input_digest", "id", "runs"}
RUN_FIELDS = {
    "artifact", "artifact_bytes", "artifact_sha256", "cache_mode", "exit_code",
    "manifest", "manifest_bytes", "manifest_sha256", "stderr", "stderr_bytes",
    "stderr_sha256", "stdout", "stdout_bytes", "stdout_sha256",
}


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
    return (
        isinstance(value, str)
        and len(value) == size
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


def builder_inventory_payload(builder):
    inventory = dict(builder)
    inventory.pop("attestation", None)
    return canonical_json(inventory)


def builder_inventory_digest(builder):
    return hashlib.sha256(builder_inventory_payload(builder)).hexdigest()


def validate_containment(value):
    if not isinstance(value, dict) or set(value) != CONTAINMENT_FIELDS:
        fail("invalid", "containment readback fields are invalid")
    if any(not integer(value[field]) for field in CONTAINMENT_FIELDS):
        fail("invalid", "containment readback values must be integers")
    if (
        not 1 <= value["memory_max_bytes"] <= MAX_MEMORY_BYTES
        or value["memory_max_bytes"] > value["memory_available_bytes"]
        or value["memory_max_readback_bytes"] != value["memory_max_bytes"]
        or value["memory_swap_max_bytes"] != 0
        or value["memory_swap_max_readback_bytes"] != 0
        or not 1 <= value["pids_max"] <= 24
        or value["pids_max_readback"] != value["pids_max"]
        or not 1 <= value["tasks_current"] <= value["pids_max_readback"]
        or value["jobs"] != 1
        or value["opt_jobs"] != 1
    ):
        fail("limit", "containment readback is outside policy")


def validate_attestation(value):
    if not isinstance(value, dict) or set(value) != ATTESTATION_FIELDS:
        fail("unsigned", "builder attestation fields are invalid")
    if (
        value["mode"] not in SIGNER_MODES
        or not text(value["identity"], 512)
        or not text(value["issuer"], 512)
        or not digest(value["inventory_sha256"])
        or not digest(value["signature_sha256"])
    ):
        fail("unsigned", "builder attestation is missing or invalid")
    if value["mode"] in ("key", "kms"):
        prefix = "ed25519-sha256:"
        if not value["identity"].startswith(prefix) or not digest(
            value["identity"][len(prefix):]
        ):
            fail("unsigned", "builder key identity is not a pinned Ed25519 SPKI digest")


def validate_run(value, cache_mode, program, names):
    if not isinstance(value, dict) or set(value) != RUN_FIELDS:
        fail("invalid", "program run fields are invalid")
    if value["cache_mode"] != cache_mode:
        fail("invalid", "cache-mode order is not canonical")
    for field in ("artifact_sha256", "manifest_sha256", "stdout_sha256", "stderr_sha256"):
        if not digest(value[field]):
            fail("invalid", f"program run digest is invalid: {field}")
    for field in ("artifact", "manifest", "stdout", "stderr"):
        if not safe_name(value[field]) or value[field] in names:
            fail("invalid", "program run path is unsafe or reused")
        names.add(value[field])
    bounded = (
        ("artifact_bytes", 1, MAX_ARTIFACT_BYTES),
        ("manifest_bytes", 1, MAX_CAPTURE_BYTES),
        ("stdout_bytes", 0, MAX_CAPTURE_BYTES),
        ("stderr_bytes", 0, MAX_CAPTURE_BYTES),
        ("exit_code", 0, 255),
    )
    for field, minimum, maximum in bounded:
        if not integer(value[field]) or not minimum <= value[field] <= maximum:
            fail("limit", f"program run bound is invalid: {field}")
    if (
        value["exit_code"] != program["expected_exit_code"]
        or value["stdout_sha256"] != program["expected_stdout_sha256"]
        or value["stderr_sha256"] != program["expected_stderr_sha256"]
    ):
        fail("raw_mismatch", "program behavior differs from the declared result")
    return value


def validate_program(value, expected_id, expected_mode):
    if not isinstance(value, dict) or set(value) != PROGRAM_FIELDS:
        fail("invalid", "program descriptor fields are invalid")
    if value["id"] != expected_id or value["mode"] != expected_mode:
        fail("invalid", "program corpus order or mode is not canonical")
    for field in (
        "build_input_digest", "expected_stderr_sha256", "expected_stdout_sha256",
        "lock_digest", "source_digest",
    ):
        if not digest(value[field]):
            fail("invalid", f"program descriptor digest is invalid: {field}")
    if not text(value["command"], 4096):
        fail("invalid", "program command is invalid")
    if not integer(value["expected_exit_code"]) or not 0 <= value["expected_exit_code"] <= 255:
        fail("limit", "program exit-code bound is invalid")
    return value


def validate_builder(value, expected_id, programs):
    if not isinstance(value, dict) or set(value) != BUILDER_FIELDS:
        fail("invalid", "builder evidence fields are invalid")
    if value["id"] != expected_id:
        fail("builder_identity", "builder evidence order is not canonical")
    for field in (
        "build_root_digest", "builder_identity", "cache_root_digest", "compiler_sha256",
        "installed_archive_sha256",
    ):
        if not digest(value[field]):
            fail("invalid", f"builder digest is invalid: {field}")
    if (
        not safe_name(value["compiler"])
        or not safe_name(value["installed_archive"])
        or not text(value["compiler_identity"], 256)
        or not text(value["toolchain"], 256)
    ):
        fail("invalid", "builder tool identity is invalid")
    validate_containment(value["containment"])
    validate_attestation(value["attestation"])
    if value["attestation"]["inventory_sha256"] != builder_inventory_digest(value):
        fail("unsigned", "builder inventory digest does not match its attestation")
    results = value["programs"]
    if not isinstance(results, list) or len(results) != len(PROGRAMS):
        fail("limit", "builder evidence must cover the complete program corpus")
    names = set()
    for index, program in enumerate(programs):
        result = results[index]
        if not isinstance(result, dict) or set(result) != RESULT_FIELDS:
            fail("invalid", "builder program result fields are invalid")
        if result["id"] != program["id"]:
            fail("invalid", "builder program result order is not canonical")
        if result["build_input_digest"] != program["build_input_digest"]:
            fail("input_drift", "builder program input digest drifted")
        runs = result["runs"]
        if not isinstance(runs, list) or len(runs) != len(CACHE_MODES):
            fail("limit", "every program requires cold, warm, and disabled cache runs")
        for run_index, cache_mode in enumerate(CACHE_MODES):
            validate_run(runs[run_index], cache_mode, program, names)
    return value


def compare_builders(first, second):
    if first["builder_identity"] == second["builder_identity"]:
        fail("builder_identity", "two distinct builder identities are required")
    root_identities = {
        first["build_root_digest"], first["cache_root_digest"],
        second["build_root_digest"], second["cache_root_digest"],
    }
    if len(root_identities) != 4:
        fail("root_identity", "build and cache roots must be independent")
    if (
        first["compiler"] != second["compiler"]
        or first["compiler_identity"] != second["compiler_identity"]
        or first["compiler_sha256"] != second["compiler_sha256"]
        or first["toolchain"] != second["toolchain"]
    ):
        fail("toolchain_drift", "builders used different compiler or toolchain identities")
    if (
        first["installed_archive"] != second["installed_archive"]
        or first["installed_archive_sha256"] != second["installed_archive_sha256"]
    ):
        fail("installed_archive", "builders used different installed release archives")
    for program_index in range(len(PROGRAMS)):
        baseline = first["programs"][program_index]["runs"][0]
        for builder in (first, second):
            for run in builder["programs"][program_index]["runs"]:
                for field in (
                    "artifact_bytes", "artifact_sha256", "exit_code", "manifest_bytes",
                    "manifest_sha256", "stderr_bytes", "stderr_sha256", "stdout_bytes",
                    "stdout_sha256",
                ):
                    if run[field] != baseline[field]:
                        fail("raw_mismatch", "two builders produced different raw program evidence")


def validate_value(value, cancelled=False):
    if not isinstance(value, dict) or set(value) != FIELDS:
        fail("invalid", "program reproducibility evidence fields are invalid")
    if cancelled or value["cancelled"] is True:
        fail("cancelled", "program certification was cancelled")
    if not isinstance(value["cancelled"], bool):
        fail("invalid", "cancellation state must be boolean")
    if value["target"] != TARGET:
        fail("platform", "program certification target is unsupported")
    if (
        value["schema"] != SCHEMA
        or not digest(value["source_commit"], 40)
        or not digest(value["source_digest"])
        or not digest(value["lock_digest"])
        or not digest(value["build_input_digest"])
        or not integer(value["source_date_epoch"])
        or value["source_date_epoch"] < 0
    ):
        fail("invalid", "program certification identity is invalid")
    programs = value["programs"]
    builders = value["builders"]
    if not isinstance(programs, list) or len(programs) != len(PROGRAMS):
        fail("limit", "program certification corpus is incomplete")
    if not isinstance(builders, list) or len(builders) != 2:
        fail("limit", "exactly two builders are required")
    for index, (program_id, mode) in enumerate(PROGRAMS):
        validate_program(programs[index], program_id, mode)
    first = validate_builder(builders[0], "builder-a", programs)
    second = validate_builder(builders[1], "builder-b", programs)
    compare_builders(first, second)
    return value


def validate(raw, max_bytes=MAX_EVIDENCE_BYTES, cancelled=False):
    if not integer(max_bytes) or not 1 <= max_bytes <= MAX_EVIDENCE_BYTES:
        fail("limit", "invalid evidence byte limit")
    if len(raw) > max_bytes:
        fail("limit", "program evidence byte limit exceeded")
    try:
        value = json.loads(raw.decode("utf-8"), object_pairs_hook=pairs)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        fail("invalid", f"invalid JSON: {type(error).__name__}")
    return validate_value(value, cancelled)


def canonical(value):
    return canonical_json(validate_value(value))


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


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--evidence", type=Path, required=True)
    parser.add_argument("--max-bytes", type=int, default=MAX_EVIDENCE_BYTES)
    parser.add_argument("--fuzz-seconds", type=float, default=0)
    parser.add_argument("--seed", type=int, default=1101)
    parser.add_argument("--test-cancel-after-read", action="store_true")
    args = parser.parse_args(argv)
    try:
        if not 0 <= args.fuzz_seconds <= 300:
            fail("limit", "invalid fuzz duration")
        metadata = args.evidence.lstat()
        if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISREG(metadata.st_mode):
            fail("invalid", "evidence file is unsafe")
        if metadata.st_size > args.max_bytes:
            fail("limit", "evidence file is oversized")
        raw = args.evidence.read_bytes()
        value = validate(raw, args.max_bytes, args.test_cancel_after_read)
        if args.fuzz_seconds:
            cases, rejected = fuzz(raw, args.fuzz_seconds, args.seed)
            print(f"seed={args.seed} cases={cases} rejected={rejected}", file=sys.stderr)
        sys.stdout.buffer.write(canonical(value))
    except ContractError as error:
        print(f"core.004g.{error.code}: {error}", file=sys.stderr)
        return 130 if error.code == "cancelled" else 1
    except OSError as error:
        print(f"core.004g.io: {type(error).__name__}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
