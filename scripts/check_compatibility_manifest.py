#!/usr/bin/env python3
"""Validate and canonically render a bounded Seen compatibility manifest."""

from __future__ import annotations

import argparse
import json
import os
import random
import re
import signal
import sys
import time
from pathlib import Path

MAX_BYTES_HARD = 1024 * 1024
MAX_TARGETS = 32
SEMVER = re.compile(
    r"^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)"
    r"(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$"
)
IDENTITY = re.compile(r"^[A-Za-z0-9._+-]+$")
TARGET_NAMES = {
    "android-arm64",
    "ios-arm64",
    "ios-sim-arm64",
    "linux-arm64",
    "linux-riscv64",
    "linux-x86_64",
    "macos-arm64",
    "macos-x86_64",
    "windows-x86_64",
}
SUPPORT = {"required", "declared-toolchain-dependent"}
TOP_FIELDS = {
    "components",
    "determinism",
    "platforms",
    "release_version",
    "schema",
    "schema_version",
    "targets",
}
DETERMINISM_FIELDS = {
    "certification",
    "context_schema",
    "object_cache_record_schema",
    "program_artifacts_schema",
    "program_build_input_schema",
    "program_reproducibility_schema",
    "release_lto_cache_record_schema",
    "required_provenance",
}
DETERMINISM_SCHEMAS = {
    "context_schema": "seen-deterministic-context-v1",
    "object_cache_record_schema": "seen-object-cache-record-v1",
    "program_artifacts_schema": "seen-program-artifacts-v1",
    "program_build_input_schema": "seen-program-build-input-v1",
    "program_reproducibility_schema": "seen-program-reproducibility-v1",
    "release_lto_cache_record_schema": "seen-release-lto-cache-record-v1",
}
REQUIRED_DETERMINISTIC_PROVENANCE = [
    "artifact-hash",
    "artifact-size",
    "build-input-digest",
    "builder-identity",
    "cancellation-state",
    "command",
    "compiler-identity",
    "containment-readback",
    "epoch",
    "lock-digest",
    "mode",
    "root-identity",
    "source-digest",
    "target-identity",
    "toolchain-identity",
]
CERTIFICATION_FIELDS = {
    "artifact_comparison",
    "builder_count",
    "complete_corpus_required",
    "distinct_builder_identities",
    "distinct_root_identities",
    "installed_archive_required",
    "signed_evidence_required",
}


class ContractError(ValueError):
    pass


def fail(code: str, message: str) -> None:
    raise ContractError(f"core.002a.{code}: {message}")


def exact_object(value: object, fields: set[str], name: str) -> dict[str, object]:
    if not isinstance(value, dict) or set(value) != fields:
        fail("invalid", f"{name} has missing or unknown fields")
    return value


def bounded_string(
    value: object, name: str, maximum: int, pattern: re.Pattern[str]
) -> str:
    if (
        not isinstance(value, str)
        or not value
        or len(value.encode("utf-8")) > maximum
        or not pattern.fullmatch(value)
    ):
        fail("invalid", f"{name} is invalid")
    return value


def validate(document: object) -> dict[str, object]:
    manifest = exact_object(document, TOP_FIELDS, "manifest")
    if manifest["schema"] != "seen-compatibility-manifest-v1":
        fail("invalid", "unsupported schema identity")
    if manifest["schema_version"] != 1:
        fail("invalid", "unsupported schema version")
    release_version = bounded_string(
        manifest["release_version"], "release_version", 64, SEMVER
    )

    components = exact_object(
        manifest["components"],
        {"compiler", "llvm", "package_client", "runtime", "standard_library"},
        "components",
    )
    compiler = exact_object(
        components["compiler"],
        {
            "layout_abi",
            "object_cache_abi",
            "package_interface_schema",
            "package_object_manifest_schema",
            "prebuilt_package_schema",
            "version",
        },
        "components.compiler",
    )
    compiler_version = bounded_string(
        compiler["version"], "components.compiler.version", 64, SEMVER
    )
    if compiler_version != release_version:
        fail("invalid", "compiler version must match release_version")
    for field in (
        "layout_abi",
        "object_cache_abi",
        "package_interface_schema",
        "package_object_manifest_schema",
        "prebuilt_package_schema",
    ):
        bounded_string(compiler[field], f"components.compiler.{field}", 128, IDENTITY)

    llvm = exact_object(components["llvm"], {"minimum_major"}, "components.llvm")
    llvm_major = llvm["minimum_major"]
    if isinstance(llvm_major, bool) or not isinstance(llvm_major, int) or not 18 <= llvm_major <= 255:
        fail("invalid", "components.llvm.minimum_major must be between 18 and 255")

    package_client = exact_object(
        components["package_client"], {"protocol", "version"}, "components.package_client"
    )
    package_version = bounded_string(
        package_client["version"], "components.package_client.version", 64, SEMVER
    )
    if package_version != release_version:
        fail("invalid", "package-client version must match release_version")
    bounded_string(
        package_client["protocol"], "components.package_client.protocol", 128, IDENTITY
    )

    runtime = exact_object(components["runtime"], {"abi"}, "components.runtime")
    bounded_string(runtime["abi"], "components.runtime.abi", 128, IDENTITY)

    stdlib = exact_object(
        components["standard_library"],
        {"module_manifest_version", "version"},
        "components.standard_library",
    )
    bounded_string(stdlib["version"], "components.standard_library.version", 64, SEMVER)
    module_version = stdlib["module_manifest_version"]
    if isinstance(module_version, bool) or not isinstance(module_version, int) or not 1 <= module_version <= 255:
        fail("invalid", "standard-library module manifest version is invalid")

    determinism = exact_object(
        manifest["determinism"], DETERMINISM_FIELDS, "determinism"
    )
    for field, expected in DETERMINISM_SCHEMAS.items():
        actual = bounded_string(determinism[field], f"determinism.{field}", 128, IDENTITY)
        if actual != expected:
            fail("invalid", f"determinism.{field} must be {expected}")
    if determinism["required_provenance"] != REQUIRED_DETERMINISTIC_PROVENANCE:
        fail("invalid", "determinism.required_provenance is incomplete or unordered")

    certification = exact_object(
        determinism["certification"], CERTIFICATION_FIELDS,
        "determinism.certification",
    )
    if certification["artifact_comparison"] != "raw-bytes":
        fail("invalid", "deterministic artifacts must be compared as raw bytes")
    builder_count = certification["builder_count"]
    if isinstance(builder_count, bool) or builder_count != 2:
        fail("invalid", "deterministic certification requires exactly two builders")
    for field in (
        "complete_corpus_required",
        "distinct_builder_identities",
        "distinct_root_identities",
        "installed_archive_required",
        "signed_evidence_required",
    ):
        if certification[field] is not True:
            fail("invalid", f"determinism.certification.{field} must be true")

    platforms = exact_object(
        manifest["platforms"],
        {"linux-arm64", "linux-x86_64", "macos", "windows"},
        "platforms",
    )
    expected_platforms = {
        "linux-arm64": "declared-toolchain-dependent",
        "linux-x86_64": "required",
        "macos": "declared-toolchain-dependent",
        "windows": "declared-toolchain-dependent",
    }
    if platforms != expected_platforms:
        fail("invalid", "platform applicability differs from schema version 1")

    targets = manifest["targets"]
    if not isinstance(targets, list) or not 1 <= len(targets) <= MAX_TARGETS:
        fail("limit", f"targets must contain between 1 and {MAX_TARGETS} entries")
    names: list[str] = []
    required_count = 0
    for index, raw_target in enumerate(targets):
        target = exact_object(raw_target, {"name", "support", "triple"}, f"targets[{index}]")
        name = target["name"]
        if not isinstance(name, str) or name not in TARGET_NAMES:
            fail("platform", f"targets[{index}].name is unsupported")
        if name in names:
            fail("invalid", f"duplicate target: {name}")
        names.append(name)
        support = target["support"]
        if support not in SUPPORT:
            fail("invalid", f"targets[{index}].support is invalid")
        if support == "required":
            required_count += 1
            if name != "linux-x86_64":
                fail("platform", "only linux-x86_64 is required in schema version 1")
        bounded_string(target["triple"], f"targets[{index}].triple", 128, IDENTITY)
    if names != sorted(names):
        fail("invalid", "targets must be ordered by name")
    if required_count != 1 or "linux-x86_64" not in names:
        fail("platform", "linux-x86_64 must be the single required target")
    return manifest


def parse_and_validate(raw: bytes, maximum: int) -> dict[str, object]:
    if len(raw) > maximum:
        fail("limit", f"manifest byte limit exceeded ({maximum})")
    if raw.startswith(b"\xef\xbb\xbf"):
        fail("invalid", "manifest must not contain a UTF-8 BOM")
    return validate(json.loads(raw.decode("utf-8")))


def fuzz(corpus: bytes, seconds: float, seed: int, maximum: int) -> None:
    rng = random.Random(seed)
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        candidate = bytearray(corpus)
        mutations = rng.randint(1, 8)
        for _ in range(mutations):
            if candidate and rng.randrange(4) == 0:
                del candidate[rng.randrange(len(candidate))]
            elif len(candidate) < maximum:
                offset = rng.randrange(len(candidate) + 1)
                candidate[offset:offset] = bytes([rng.randrange(256)])
            elif candidate:
                candidate[rng.randrange(len(candidate))] = rng.randrange(256)
        try:
            parse_and_validate(bytes(candidate), maximum)
        except (UnicodeDecodeError, json.JSONDecodeError, ContractError):
            pass


def positive_bounded(value: str, maximum: int, name: str) -> int:
    try:
        parsed = int(value, 10)
    except ValueError as error:
        raise argparse.ArgumentTypeError(f"{name} must be an integer") from error
    if not 1 <= parsed <= maximum:
        raise argparse.ArgumentTypeError(f"{name} must be between 1 and {maximum}")
    return parsed


def cancel(_signum: int, _frame: object) -> None:
    raise KeyboardInterrupt


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("manifest", type=Path)
    parser.add_argument("--max-bytes", default=str(64 * 1024))
    parser.add_argument("--fuzz-seconds", type=float, default=0.0)
    parser.add_argument("--seed", type=int, default=1101)
    parser.add_argument("--test-cancel-after-read", action="store_true", help=argparse.SUPPRESS)
    args = parser.parse_args()
    try:
        maximum = positive_bounded(args.max_bytes, MAX_BYTES_HARD, "max-bytes")
        if args.fuzz_seconds < 0.0 or args.fuzz_seconds > 300.0:
            fail("limit", "fuzz-seconds must be between 0 and 300")
        if args.manifest.is_symlink():
            fail("invalid", "manifest must be a regular non-symlink file")
        path = args.manifest.resolve(strict=True)
        if not path.is_file():
            fail("invalid", "manifest must be a regular non-symlink file")
        raw = path.read_bytes()
        if args.test_cancel_after_read:
            if os.environ.get("SEEN_CORE_002A_TEST_HOOKS") != "1":
                fail("invalid", "cancellation test hook is disabled")
            raise KeyboardInterrupt
        manifest = parse_and_validate(raw, maximum)
        if args.fuzz_seconds > 0.0:
            fuzz(raw, args.fuzz_seconds, args.seed, maximum)
            print(
                f"compatibility-manifest: fuzz seed={args.seed} "
                f"seconds={args.fuzz_seconds:g} status=pass",
                file=sys.stderr,
            )
        sys.stdout.buffer.write(
            (json.dumps(manifest, indent=2, sort_keys=True) + "\n").encode("utf-8")
        )
    except KeyboardInterrupt:
        print("compatibility-manifest: core.002a.cancelled: validation cancelled", file=sys.stderr)
        return 130
    except (
        OSError,
        UnicodeDecodeError,
        json.JSONDecodeError,
        ContractError,
        argparse.ArgumentTypeError,
    ) as error:
        print(f"compatibility-manifest: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    signal.signal(signal.SIGTERM, cancel)
    sys.exit(main())
