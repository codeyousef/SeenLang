#!/usr/bin/env python3
"""Validate canonical CORE-004A two-builder bootstrap evidence."""
import argparse, json, random, sys, time
from pathlib import Path

SCHEMA = "seen-bootstrap-reproducibility-v1"
NORMALIZATIONS = {"none", "elf64-x86_64-v1"}
MAX_BYTES = 1_048_576
MAX_ARTIFACT_BYTES = 1_073_741_824
FIELDS = {"builders", "command", "limits", "normalization", "schema", "source_commit", "source_date_epoch", "source_digest", "target"}
LIMIT_FIELDS = {"jobs", "memory_max_bytes", "memory_swap_max_bytes", "opt_jobs", "pids_max"}
BUILDER_FIELDS = {"artifact_bytes", "build_root_digest", "builder_sha256", "id", "normalized_artifact_sha256", "raw_artifact_sha256", "toolchain"}


class ContractError(ValueError):
    def __init__(self, code, message): super().__init__(message); self.code = code


def fail(code, message): raise ContractError(code, message)


def pairs(items):
    output = {}
    for key, value in items:
        if key in output: fail("invalid", f"duplicate field: {key}")
        output[key] = value
    return output


def integer(value): return isinstance(value, int) and not isinstance(value, bool)


def digest(value, size):
    return isinstance(value, str) and len(value) == size and all(character in "0123456789abcdef" for character in value)


def text(value, maximum):
    return isinstance(value, str) and 0 < len(value) <= maximum and all(32 <= ord(character) <= 126 for character in value)


def validate_builder(builder, expected_id):
    if not isinstance(builder, dict) or set(builder) != BUILDER_FIELDS:
        fail("invalid", "builder evidence fields are invalid")
    if builder["id"] != expected_id:
        fail("invalid", "builder evidence order is not canonical")
    for key in ("builder_sha256", "build_root_digest", "raw_artifact_sha256", "normalized_artifact_sha256"):
        if not digest(builder[key], 64): fail("invalid", f"invalid builder digest: {key}")
    if not integer(builder["artifact_bytes"]) or not 1 <= builder["artifact_bytes"] <= MAX_ARTIFACT_BYTES:
        fail("limit", "builder artifact size is invalid")
    if not text(builder["toolchain"], 256): fail("invalid", "builder toolchain is invalid")
    return builder


def validate(raw, max_bytes=MAX_BYTES, cancelled=False):
    if not integer(max_bytes) or not 1 <= max_bytes <= MAX_BYTES: fail("limit", "invalid evidence byte limit")
    if len(raw) > max_bytes: fail("limit", "bootstrap evidence byte limit exceeded")
    try: value = json.loads(raw.decode(), object_pairs_hook=pairs)
    except (UnicodeDecodeError, json.JSONDecodeError) as error: fail("invalid", f"invalid JSON: {type(error).__name__}")
    if cancelled: fail("cancelled", "bootstrap certification was cancelled")
    if not isinstance(value, dict) or set(value) != FIELDS: fail("invalid", "bootstrap evidence fields are invalid")
    if value["schema"] != SCHEMA: fail("invalid", "bootstrap evidence schema is invalid")
    if value["target"] != "linux-x86_64": fail("platform", "bootstrap target is unsupported")
    if not digest(value["source_commit"], 40) or not digest(value["source_digest"], 64):
        fail("invalid", "bootstrap source identity is invalid")
    if not integer(value["source_date_epoch"]) or value["source_date_epoch"] < 0:
        fail("invalid", "source date epoch is invalid")
    if value["normalization"] not in NORMALIZATIONS or not text(value["command"], 4096):
        fail("invalid", "normalization or command is invalid")
    limits = value["limits"]
    if not isinstance(limits, dict) or set(limits) != LIMIT_FIELDS: fail("invalid", "containment fields are invalid")
    if any(not integer(limits[key]) for key in LIMIT_FIELDS): fail("invalid", "containment values must be integers")
    if limits["memory_max_bytes"] < 1 or limits["memory_swap_max_bytes"] != 0:
        fail("limit", "memory containment is invalid")
    if not 1 <= limits["pids_max"] <= 24 or limits["jobs"] != 1 or limits["opt_jobs"] != 1:
        fail("limit", "task or worker containment is invalid")
    builders = value["builders"]
    if not isinstance(builders, list) or len(builders) != 2: fail("limit", "exactly two builders are required")
    first = validate_builder(builders[0], "builder-a"); second = validate_builder(builders[1], "builder-b")
    if first["builder_sha256"] != second["builder_sha256"] or first["build_root_digest"] == second["build_root_digest"]:
        fail("invalid", "builders must use the same pinned seed in independent roots")
    if first["toolchain"] != second["toolchain"]: fail("invalid", "builder toolchains differ")
    if value["normalization"] == "none" and any(
            builder["raw_artifact_sha256"] != builder["normalized_artifact_sha256"]
            for builder in builders):
        fail("invalid", "normalization none cannot change the artifact digest")
    if first["normalized_artifact_sha256"] != second["normalized_artifact_sha256"] or first["artifact_bytes"] != second["artifact_bytes"]:
        fail("mismatch", "two builders produced different compiler artifacts")
    return value


def canonical(value):
    return (json.dumps(validate(json.dumps(value).encode()), separators=(",", ":"), sort_keys=True) + "\n").encode()


def fuzz(raw, seconds, seed):
    rng = random.Random(seed); deadline = time.monotonic() + seconds; cases = rejected = 0
    while time.monotonic() < deadline:
        changed = bytearray(raw)
        if changed: changed[rng.randrange(len(changed))] = rng.randrange(256)
        cases += 1
        try: validate(bytes(changed))
        except ContractError: rejected += 1
    return cases, rejected


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__); parser.add_argument("--evidence", type=Path, required=True)
    parser.add_argument("--max-bytes", type=int, default=MAX_BYTES); parser.add_argument("--fuzz-seconds", type=float, default=0)
    parser.add_argument("--seed", type=int, default=1101); parser.add_argument("--test-cancel-after-read", action="store_true")
    args = parser.parse_args(argv)
    try:
        if not 0 <= args.fuzz_seconds <= 300: fail("limit", "invalid fuzz duration")
        metadata = args.evidence.lstat()
        if args.evidence.is_symlink() or not args.evidence.is_file(): fail("limit", "evidence file is unsafe")
        if metadata.st_size > args.max_bytes: fail("limit", "evidence file is oversized")
        raw = args.evidence.read_bytes(); value = validate(raw, args.max_bytes, args.test_cancel_after_read)
        if args.fuzz_seconds:
            cases, rejected = fuzz(raw, args.fuzz_seconds, args.seed); print(f"seed={args.seed} cases={cases} rejected={rejected}", file=sys.stderr)
        sys.stdout.buffer.write(canonical(value))
    except ContractError as error:
        print(f"core.004a.{error.code}: {error}", file=sys.stderr); return 130 if error.code == "cancelled" else 1
    except OSError as error: print(f"core.004a.io: {type(error).__name__}", file=sys.stderr); return 1
    return 0


if __name__ == "__main__": sys.exit(main())
