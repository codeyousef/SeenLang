#!/usr/bin/env python3
"""Dependency-free consistency checks for the draft Seen registry contract."""

from __future__ import annotations

import hashlib
import json
import re
import tomllib
import unicodedata
from copy import deepcopy
from datetime import datetime
from pathlib import Path
from typing import Any
from urllib.parse import urlsplit


ROOT = Path(__file__).resolve().parents[1]
CONTRACT = ROOT / "contracts" / "package-registry" / "v1"


def load_json(relative: str) -> Any:
    with (CONTRACT / relative).open(encoding="utf-8") as handle:
        return json.load(handle)


def load_toml(relative: str) -> dict[str, Any]:
    with (CONTRACT / relative).open("rb") as handle:
        return tomllib.load(handle)


def canonical_json_bytes(value: Any) -> bytes:
    """Encode the frozen seen-tuf-canonical-json-v1 representation."""
    return json.dumps(
        value,
        ensure_ascii=False,
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")


def resolve_schema_ref(
    root: dict[str, Any], reference: str
) -> tuple[dict[str, Any], dict[str, Any]]:
    document = root
    fragment = reference
    if not reference.startswith("#"):
        filename, separator, fragment_text = reference.partition("#")
        assert filename, reference
        if filename.startswith("urn:seen:"):
            matches = [
                load_json(f"schemas/{path.name}")
                for path in sorted((CONTRACT / "schemas").glob("*.json"))
                if load_json(f"schemas/{path.name}").get("$id") == filename
            ]
            assert len(matches) == 1, reference
            document = matches[0]
        else:
            assert "/" not in filename and "\\" not in filename, reference
            document = load_json(f"schemas/{filename}")
        fragment = f"#{fragment_text}" if separator else "#"
    assert fragment == "#" or fragment.startswith("#/"), reference
    current: Any = document
    if fragment == "#":
        return current, document
    for piece in fragment[2:].split("/"):
        current = current[piece.replace("~1", "/").replace("~0", "~")]
    assert isinstance(current, dict)
    return current, document


def schema_type_matches(value: Any, expected: str) -> bool:
    if expected == "object":
        return isinstance(value, dict)
    if expected == "array":
        return isinstance(value, list)
    if expected == "string":
        return isinstance(value, str)
    if expected == "integer":
        return isinstance(value, int) and not isinstance(value, bool)
    if expected == "number":
        return isinstance(value, (int, float)) and not isinstance(value, bool)
    if expected == "boolean":
        return isinstance(value, bool)
    if expected == "null":
        return value is None
    raise AssertionError(f"unsupported schema type: {expected}")


def is_date_time(value: str) -> bool:
    try:
        datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError:
        return False
    return "T" in value


def schema_errors(
    value: Any,
    schema: dict[str, Any] | bool,
    root: dict[str, Any],
    path: str = "$",
) -> list[str]:
    if schema is True:
        return []
    if schema is False:
        return [f"{path}: value is forbidden by schema"]
    errors: list[str] = []
    if "$ref" in schema:
        referenced, referenced_root = resolve_schema_ref(root, schema["$ref"])
        errors.extend(schema_errors(value, referenced, referenced_root, path))
    if "allOf" in schema:
        for child in schema["allOf"]:
            errors.extend(schema_errors(value, child, root, path))
    if "anyOf" in schema:
        if not any(not schema_errors(value, child, root, path) for child in schema["anyOf"]):
            errors.append(f"{path}: expected at least one matching schema")
    if "oneOf" in schema:
        matches = sum(not schema_errors(value, child, root, path) for child in schema["oneOf"])
        if matches != 1:
            errors.append(f"{path}: expected exactly one matching schema, got {matches}")
    if "not" in schema and not schema_errors(value, schema["not"], root, path):
        errors.append(f"{path}: matched forbidden schema")
    if "if" in schema:
        if not schema_errors(value, schema["if"], root, path):
            errors.extend(schema_errors(value, schema.get("then", {}), root, path))
        else:
            errors.extend(schema_errors(value, schema.get("else", {}), root, path))

    expected_type = schema.get("type")
    if expected_type:
        expected_types = expected_type if isinstance(expected_type, list) else [expected_type]
        if not any(schema_type_matches(value, item) for item in expected_types):
            return errors + [f"{path}: expected {' or '.join(expected_types)}"]
    if "const" in schema and value != schema["const"]:
        errors.append(f"{path}: expected constant {schema['const']!r}")
    if "enum" in schema and value not in schema["enum"]:
        errors.append(f"{path}: value is outside enum")

    if isinstance(value, str):
        if len(value) < schema.get("minLength", 0):
            errors.append(f"{path}: string is too short")
        if len(value) > schema.get("maxLength", len(value)):
            errors.append(f"{path}: string is too long")
        if "pattern" in schema and not re.search(schema["pattern"], value):
            errors.append(f"{path}: pattern mismatch")
        if schema.get("format") == "semver-exact" and not is_exact_semver(value):
            errors.append(f"{path}: invalid exact semantic version")
        if (
            schema.get("format") == "seen-semver-requirement-v1"
            and parse_semver_requirement(value) is None
        ):
            errors.append(f"{path}: invalid Seen semantic-version requirement")
        if schema.get("format") == "date-time" and not is_date_time(value):
            errors.append(f"{path}: invalid date-time")
        if schema.get("format") == "uri":
            parsed = urlsplit(value)
            if not parsed.scheme or not parsed.netloc:
                errors.append(f"{path}: invalid URI")

    if isinstance(value, (int, float)) and not isinstance(value, bool):
        if "minimum" in schema and value < schema["minimum"]:
            errors.append(f"{path}: number is below minimum")
        if "maximum" in schema and value > schema["maximum"]:
            errors.append(f"{path}: number is above maximum")

    if isinstance(value, list):
        if len(value) < schema.get("minItems", 0):
            errors.append(f"{path}: array has too few items")
        if len(value) > schema.get("maxItems", len(value)):
            errors.append(f"{path}: array has too many items")
        if schema.get("uniqueItems"):
            encoded = [json.dumps(item, sort_keys=True) for item in value]
            if len(encoded) != len(set(encoded)):
                errors.append(f"{path}: duplicate array items")
        prefix_items = schema.get("prefixItems", [])
        for index, child_schema in enumerate(prefix_items):
            if index < len(value):
                errors.extend(
                    schema_errors(value[index], child_schema, root, f"{path}[{index}]")
                )
        if "items" in schema:
            start = len(prefix_items) if prefix_items else 0
            for index in range(start, len(value)):
                errors.extend(
                    schema_errors(value[index], schema["items"], root, f"{path}[{index}]")
                )
        if "contains" in schema:
            matches = sum(
                not schema_errors(item, schema["contains"], root, f"{path}[{index}]")
                for index, item in enumerate(value)
            )
            minimum = schema.get("minContains", 1)
            maximum = schema.get("maxContains")
            if matches < minimum or (maximum is not None and matches > maximum):
                errors.append(f"{path}: contains matched {matches} items")

    if isinstance(value, dict):
        if len(value) < schema.get("minProperties", 0):
            errors.append(f"{path}: too few properties")
        if len(value) > schema.get("maxProperties", len(value)):
            errors.append(f"{path}: too many properties")
        for required in schema.get("required", []):
            if required not in value:
                errors.append(f"{path}: missing required property {required}")
        for key, dependents in schema.get("dependentRequired", {}).items():
            if key in value:
                for dependent in dependents:
                    if dependent not in value:
                        errors.append(
                            f"{path}: {key} requires property {dependent}"
                        )
        properties = schema.get("properties", {})
        additional = schema.get("additionalProperties", True)
        for key, child_value in value.items():
            if "propertyNames" in schema:
                errors.extend(schema_errors(key, schema["propertyNames"], root, f"{path}.<key>"))
            if key in properties:
                errors.extend(schema_errors(child_value, properties[key], root, f"{path}.{key}"))
            elif additional is False:
                errors.append(f"{path}: unexpected property {key}")
            elif isinstance(additional, dict):
                errors.extend(schema_errors(child_value, additional, root, f"{path}.{key}"))
    return errors


def identity_failure_reason(identity: str) -> str:
    if not identity.isascii():
        return "non_ascii"
    if identity.lower() != identity:
        return "not_lowercase"
    pieces = identity.split("/")
    if len(pieces) != 2:
        return "invalid_shape"
    if not all(pieces):
        return "empty_segment"
    if any(len(piece) > 63 for piece in pieces):
        return "segment_too_long"
    segment = re.compile(r"^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$")
    if not all(segment.fullmatch(piece) for piece in pieces):
        return "invalid_character"
    return ""


def is_exact_semver(value: str) -> bool:
    if not value or len(value) > 128:
        return False
    without_build, plus, build = value.partition("+")
    if plus:
        if not build or "+" in build:
            return False
        if any(
            not part or not re.fullmatch(r"[0-9A-Za-z-]+", part)
            for part in build.split(".")
        ):
            return False
    core, hyphen, prerelease = without_build.partition("-")
    if hyphen:
        if not prerelease:
            return False
        for part in prerelease.split("."):
            if not part or not re.fullmatch(r"[0-9A-Za-z-]+", part):
                return False
            if part.isdigit() and len(part) > 1 and part.startswith("0"):
                return False
    pieces = core.split(".")
    if len(pieces) != 3:
        return False
    return all(
        piece.isdigit()
        and (piece == "0" or not piece.startswith("0"))
        for piece in pieces
    )


def split_semver(value: str) -> tuple[tuple[int, int, int], tuple[str, ...], str] | None:
    if not is_exact_semver(value):
        return None
    without_build, _, build = value.partition("+")
    core, _, prerelease = without_build.partition("-")
    major, minor, patch = (int(piece) for piece in core.split("."))
    prerelease_parts = tuple(prerelease.split(".")) if prerelease else ()
    return (major, minor, patch), prerelease_parts, build


def compare_semver_precedence(left: str, right: str) -> int:
    left_parts = split_semver(left)
    right_parts = split_semver(right)
    assert left_parts is not None and right_parts is not None
    left_core, left_pre, _ = left_parts
    right_core, right_pre, _ = right_parts
    if left_core != right_core:
        return (left_core > right_core) - (left_core < right_core)
    if not left_pre or not right_pre:
        return (not left_pre) - (not right_pre)
    for left_item, right_item in zip(left_pre, right_pre):
        if left_item == right_item:
            continue
        left_number = left_item.isdigit()
        right_number = right_item.isdigit()
        if left_number and right_number:
            return (int(left_item) > int(right_item)) - (
                int(left_item) < int(right_item)
            )
        if left_number != right_number:
            return -1 if left_number else 1
        return (left_item > right_item) - (left_item < right_item)
    return (len(left_pre) > len(right_pre)) - (len(left_pre) < len(right_pre))


def parse_semver_requirement(value: str) -> dict[str, Any] | None:
    if not value or value != value.strip() or "\t" in value or "\n" in value:
        return None
    if value.startswith("^"):
        version = value[1:]
        parsed = split_semver(version)
        if parsed is None or parsed[2]:
            return None
        major, minor, patch = parsed[0]
        if major:
            upper = f"{major + 1}.0.0"
        elif minor:
            upper = f"0.{minor + 1}.0"
        else:
            upper = f"0.0.{patch + 1}"
        return {
            "kind": "caret",
            "lower": (">=", version),
            "upper": ("<", upper),
        }
    if value.startswith("~"):
        version = value[1:]
        if re.fullmatch(r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)", version):
            major, minor = (int(piece) for piece in version.split("."))
            lower = f"{major}.{minor}.0"
        else:
            parsed = split_semver(version)
            if parsed is None or parsed[2]:
                return None
            major, minor, _ = parsed[0]
            lower = version
        return {
            "kind": "tilde",
            "lower": (">=", lower),
            "upper": ("<", f"{major}.{minor + 1}.0"),
        }
    comparator = re.fullmatch(r"(>=|>)(\S+) (<=|<)(\S+)", value)
    if comparator:
        lower_operator, lower, upper_operator, upper = comparator.groups()
        lower_parts = split_semver(lower)
        upper_parts = split_semver(upper)
        if (
            lower_parts is None
            or upper_parts is None
            or lower_parts[2]
            or upper_parts[2]
            or compare_semver_precedence(lower, upper) >= 0
        ):
            return None
        return {
            "kind": "comparator-conjunction",
            "lower": (lower_operator, lower),
            "upper": (upper_operator, upper),
        }
    if is_exact_semver(value):
        return {"kind": "exact", "exact": value}
    return None


def requirement_allows(requirement: str, candidate: str) -> bool:
    parsed = parse_semver_requirement(requirement)
    candidate_parts = split_semver(candidate)
    if parsed is None or candidate_parts is None:
        return False
    if parsed["kind"] == "exact":
        return candidate == parsed["exact"]
    lower_operator, lower = parsed["lower"]
    upper_operator, upper = parsed["upper"]
    lower_comparison = compare_semver_precedence(candidate, lower)
    upper_comparison = compare_semver_precedence(candidate, upper)
    if lower_comparison < 0 or (lower_comparison == 0 and lower_operator == ">"):
        return False
    if upper_comparison > 0 or (upper_comparison == 0 and upper_operator == "<"):
        return False
    if candidate_parts[1]:
        opted_in_cores = {
            split_semver(bound)[0]
            for _, bound in (parsed["lower"], parsed["upper"])
            if split_semver(bound) is not None and split_semver(bound)[1]
        }
        if candidate_parts[0] not in opted_in_cores:
            return False
    return True


def check_identities() -> None:
    schema = load_json("schemas/package-identity.schema.json")
    cases = load_json("fixtures/package-identity-cases.json")
    assert schema["$id"].startswith("urn:seen:"), schema["$id"]
    pattern = re.compile(schema["pattern"])
    reserved = set(cases["reserved_owners"])

    for case in cases["syntactically_valid"]:
        identity = case["identity"]
        assert pattern.fullmatch(identity), identity
        assert identity_failure_reason(identity) == "", identity
        owner, name = identity.split("/")
        assert (owner, name) == (case["owner"], case["name"])
        assert (owner in reserved) is case["reserved"]

    for case in cases["invalid"]:
        identity = case["identity"]
        assert not pattern.fullmatch(identity), identity
        assert identity_failure_reason(identity) == case["reason"], identity

    policy = cases["registration_policy"]
    assert policy["status"] == "draft_not_enforced"
    assert all(
        policy[field] == "must_be_frozen_before_public_registration"
        for field in (
            "unicode_data_revision",
            "skeleton_algorithm",
            "similarity_thresholds",
        )
    )
    for case in cases["policy_examples"]:
        assert pattern.fullmatch(case["candidate"]), case["candidate"]
        assert pattern.fullmatch(case["conflicts_with"]), case["conflicts_with"]
        assert case["expected"] in {"allow", "manual_review", "reject_exact_skeleton"}


def check_manifest() -> None:
    schema = load_json("schemas/seen-manifest-v1.schema.json")
    manifest = load_toml("fixtures/scoped-dependencies.toml")
    assert schema["$id"] == "urn:seen:package-registry:v1:manifest"
    assert schema["additionalProperties"] is False
    assert "package" in schema["required"]
    assert schema["properties"]["project"]["additionalProperties"] is False
    assert schema["properties"]["package"]["additionalProperties"] is False
    assert not schema_errors(manifest, schema, schema), schema_errors(
        manifest, schema, schema
    )
    assert manifest["manifest-version"] == 1

    project = manifest["project"]
    package = manifest["package"]
    identity = package["identity"]
    assert identity_failure_reason(identity) == ""
    assert project["name"] != identity.split("/")[-1], (
        "fixture must prove project.name and package.identity are independent"
    )

    alias_pattern = re.compile(schema["$defs"]["alias"]["pattern"])
    reserved_aliases = set(schema["$defs"]["alias"]["not"]["enum"])
    version_pattern = re.compile(schema["$defs"]["exactVersion"]["pattern"])
    capability_values = set(schema["$defs"]["capability"]["enum"])
    assert set(package["capabilities"]) <= capability_values
    assert schema["$defs"]["exactVersion"]["format"] == "semver-exact"
    assert is_exact_semver(project["version"])

    seen_aliases: set[str] = set()
    for alias, dependency in manifest["dependencies"].items():
        assert alias_pattern.fullmatch(alias), alias
        assert alias not in reserved_aliases, alias
        assert alias not in seen_aliases, alias
        seen_aliases.add(alias)
        assert set(dependency) <= {"package", "version", "registry", "allow"}
        assert {"package", "version"} <= set(dependency)
        assert identity_failure_reason(dependency["package"]) == ""
        assert version_pattern.fullmatch(dependency["version"]), dependency["version"]
        assert is_exact_semver(dependency["version"]), dependency["version"]
        assert set(dependency.get("allow", [])) <= capability_values


def check_resolver_lock() -> None:
    schema = load_json("schemas/resolver-lock-v2.schema.json")
    lock = load_toml("fixtures/resolver-lock-v2.toml")
    assert schema["$id"] == "urn:seen:package-registry:v1:resolver-lock-v2"
    assert lock["version"] == 2
    assert not schema_errors(lock, schema, schema), schema_errors(lock, schema, schema)
    alias_pattern = re.compile(schema["$defs"]["alias"]["pattern"])
    reserved_aliases = set(schema["$defs"]["alias"]["not"]["enum"])
    version_pattern = re.compile(schema["$defs"]["exactVersion"]["pattern"])
    digest_pattern = re.compile(schema["$defs"]["sha256"]["pattern"])

    aliases: set[str] = set()
    for package in lock["packages"]:
        assert alias_pattern.fullmatch(package["alias"])
        assert package["alias"] not in reserved_aliases
        assert package["alias"] not in aliases
        aliases.add(package["alias"])
        assert version_pattern.fullmatch(package["version"])
        assert is_exact_semver(package["version"])
        assert package["source"] in {"hosted-registry", "legacy-local-static"}
        assert package["registry_origin"]
        assert not ({"name", "registry", "path", "sha256"} & set(package))
        if package["source"] == "hosted-registry":
            assert identity_failure_reason(package["package"]) == ""
            assert package["registry_origin"].startswith("https://")
            assert digest_pattern.fullmatch(package["archive_sha256"])


def check_semantic_versions() -> None:
    cases = load_json("fixtures/semantic-version-cases.json")
    assert cases["format"] == "semver-exact"
    assert all(is_exact_semver(value) for value in cases["valid"])
    assert not any(is_exact_semver(value) for value in cases["invalid"])

    surfaces: list[dict[str, Any]] = []
    for reference in cases["required_schema_surfaces"]:
        filename, separator, fragment = reference.partition("#")
        assert separator and fragment.startswith("/"), reference
        if filename == "openapi.yaml":
            openapi = (CONTRACT / filename).read_text(encoding="utf-8")
            match = re.search(
                r'^    ExactVersion:\n'
                r'      type: (\S+)\n'
                r'      format: (\S+)\n'
                r'      maxLength: ([0-9]+)\n'
                r'      pattern: ("(?:[^"\\]|\\.)*")$',
                openapi,
                re.MULTILINE,
            )
            assert match, reference
            surface = {
                "type": match.group(1),
                "format": match.group(2),
                "maxLength": int(match.group(3)),
                "pattern": json.loads(match.group(4)),
            }
        else:
            document = load_json(f"schemas/{filename}")
            surface: Any = document
            for piece in fragment[1:].split("/"):
                surface = surface[piece.replace("~1", "/").replace("~0", "~")]
            assert isinstance(surface, dict), reference
        assert surface["type"] == "string", reference
        assert surface["format"] == "semver-exact", reference
        assert surface["maxLength"] == 128, reference
        assert all(
            not schema_errors(value, surface, surface) for value in cases["valid"]
        ), reference
        assert all(
            schema_errors(value, surface, surface) for value in cases["invalid"]
        ), reference
        surfaces.append(surface)
    assert surfaces and all(surface == surfaces[0] for surface in surfaces[1:])


def resolution_result(case: dict[str, Any]) -> dict[str, Any]:
    requirements = [item["requirement"] for item in case["requirements"]]
    if any(parse_semver_requirement(item) is None for item in requirements):
        return {"outcome": "error", "error_code": "invalid_version_requirement"}

    mode = case["mode"]
    lock = case["lock"]
    origin = case["registry_origin"]
    if mode == "frozen" and lock is None:
        return {"outcome": "error", "error_code": "lock_required"}
    if lock is not None and lock["registry_origin"] != origin:
        return {"outcome": "error", "error_code": "locked_origin_mismatch"}

    same_origin = [
        candidate
        for candidate in case["candidates"]
        if candidate["registry_origin"] == origin
    ]
    if not same_origin and case["candidates"]:
        return {"outcome": "error", "error_code": "registry_origin_mismatch"}

    digests_by_version: dict[str, set[str]] = {}
    for candidate in same_origin:
        digests_by_version.setdefault(candidate["version"], set()).add(
            candidate["archive_sha256"]
        )
    if any(len(digests) > 1 for digests in digests_by_version.values()):
        return {"outcome": "error", "error_code": "metadata_equivocation"}

    locked_candidate: dict[str, Any] | None = None
    if lock is not None:
        version_matches = [
            candidate
            for candidate in same_origin
            if candidate["version"] == lock["version"]
        ]
        if version_matches and all(
            candidate["archive_sha256"] != lock["archive_sha256"]
            for candidate in version_matches
        ):
            return {"outcome": "error", "error_code": "lock_digest_mismatch"}
        locked_candidate = next(
            (
                candidate
                for candidate in version_matches
                if candidate["archive_sha256"] == lock["archive_sha256"]
            ),
            None,
        )
        if (
            locked_candidate is not None
            and locked_candidate["availability"] == "security-quarantined"
        ):
            return {
                "outcome": "error",
                "error_code": "locked_release_quarantined",
            }
        lock_matches = all(
            requirement_allows(requirement, lock["version"])
            for requirement in requirements
        )
        if mode == "frozen" and (locked_candidate is None or not lock_matches):
            return {
                "outcome": "error",
                "error_code": "locked_requirement_mismatch",
            }
        lock_is_eligible = (
            locked_candidate is not None
            and lock_matches
            and (
                locked_candidate["availability"] == "available"
                or (
                    mode in {"normal", "frozen"}
                    and locked_candidate["availability"] == "yanked"
                )
            )
        )
        if mode in {"normal", "frozen"} and lock_is_eligible:
            return {
                "outcome": "select",
                "version": lock["version"],
                "registry_origin": origin,
                "archive_sha256": lock["archive_sha256"],
                "from_lock": True,
                "write_lock": False,
            }

    eligible = [
        candidate
        for candidate in same_origin
        if candidate["availability"] == "available"
        and all(
            requirement_allows(requirement, candidate["version"])
            for requirement in requirements
        )
    ]
    if not eligible:
        individually_satisfiable = all(
            any(
                candidate["availability"] == "available"
                and requirement_allows(requirement, candidate["version"])
                for candidate in same_origin
            )
            for requirement in requirements
        )
        error_code = (
            "dependency_constraint_conflict"
            if len(requirements) > 1 and individually_satisfiable
            else "no_matching_version"
        )
        result: dict[str, Any] = {"outcome": "error", "error_code": error_code}
        if error_code == "dependency_constraint_conflict":
            result["conflict_requesters"] = sorted(
                item["requester"] for item in case["requirements"]
            )
        return result

    best = eligible[0]
    for candidate in eligible[1:]:
        if compare_semver_precedence(candidate["version"], best["version"]) > 0:
            best = candidate
    top = [
        candidate
        for candidate in eligible
        if compare_semver_precedence(candidate["version"], best["version"]) == 0
    ]
    if len({candidate["version"] for candidate in top}) > 1:
        return {"outcome": "error", "error_code": "ambiguous_build_metadata"}
    return {
        "outcome": "select",
        "version": best["version"],
        "registry_origin": origin,
        "archive_sha256": best["archive_sha256"],
        "from_lock": False,
        "write_lock": True,
    }


def check_semver_requirements() -> None:
    schema = load_json("schemas/semver-requirement-v1.schema.json")
    fixture = load_json("fixtures/semantic-requirement-cases-v1.json")
    assert schema["$id"] == "urn:seen:package-registry:v1:semver-requirement"
    assert fixture["canonicalization"] == "reject-noncanonical-input-without-repair"
    for case in fixture["valid"]:
        requirement = case["input"]
        assert not schema_errors(requirement, schema, schema), requirement
        parsed = parse_semver_requirement(requirement)
        assert parsed is not None and parsed["kind"] == case["kind"], requirement
        if case["kind"] == "exact":
            assert parsed["exact"] == case["exact_version"]
        else:
            assert parsed["lower"] == (
                case["lower"]["operator"],
                case["lower"]["version"],
            )
            assert parsed["upper"] == (
                case["upper"]["operator"],
                case["upper"]["version"],
            )
    for case in fixture["invalid"]:
        requirement = case["input"]
        assert schema_errors(requirement, schema, schema), requirement
        assert parse_semver_requirement(requirement) is None, requirement
        assert case["error_code"] == "invalid_version_requirement"

    policy = load_json("fixtures/resolution-policy-v1.json")
    case_schema = load_json("schemas/deterministic-resolution-case-v1.schema.json")
    assert policy["resolution_key"] == ["registry_origin", "package"]
    assert policy["canonical_input"]["cross_registry_fallback"] == "forbidden"
    assert policy["build_metadata_policy"]["lexical-build-tiebreak"] == "forbidden"
    assert policy["implementation_status"]["compiler_subset"] == "exact-requirements-only"
    assert policy["implementation_status"]["hosted_resolution"].startswith("disabled-")
    case_fixture = load_json(f"fixtures/{policy['case_fixture']}")
    assert case_fixture["policy"] == "resolution-policy-v1.json"
    cases = case_fixture["cases"]
    names = [case["name"] for case in cases]
    assert len(names) == len(set(names))
    coverage = " ".join(names + [case.get("description", "") for case in cases]).lower()
    for required in (
        "highest",
        "prerelease",
        "yanked",
        "quarantined",
        "build",
        "origin",
        "lock",
        "update",
        "conflict",
    ):
        assert required in coverage, f"missing deterministic-resolution case: {required}"
    for case in cases:
        errors = schema_errors(case, case_schema, case_schema)
        assert not errors, f"{case['name']}: {errors}"
        assert resolution_result(case) == case["expected"], case["name"]


def check_registry_origins() -> None:
    manifest_schema = load_json("schemas/seen-manifest-v1.schema.json")
    lock_schema = load_json("schemas/resolver-lock-v2.schema.json")
    cases = load_json("fixtures/registry-origin-cases.json")
    manifest_origin = manifest_schema["$defs"]["registryOrigin"]
    lock_origin = lock_schema["$defs"]["registryOrigin"]
    assert manifest_origin == lock_origin
    pattern = re.compile(manifest_origin["pattern"])
    assert all(pattern.fullmatch(value) for value in cases["valid"])
    assert not any(pattern.fullmatch(value) for value in cases["invalid"])


def recent_auth_result(
    server_now: int,
    auth_time: int | None,
    maximum_age: int,
    future_tolerance: int,
) -> tuple[str, str | None]:
    if auth_time is None:
        return "reject", "unauthenticated"
    age = server_now - auth_time
    if age < 0 - future_tolerance:
        return "reject", "unauthenticated"
    if age > maximum_age:
        return "reject", "forbidden"
    return "accept", None


def api_conformance_result(case: dict[str, Any]) -> dict[str, Any]:
    operation = case["operation_id"]
    preconditions = case["preconditions"]
    if operation == "getSignedMetadata":
        if preconditions.get("server_environment") == "development" and (
            preconditions.get("requested_repository") == "seen-prod-registry-v1"
        ):
            return {
                "authorization": "public",
                "http_status": 404,
                "error_code": "not_found",
            }
        return {"authorization": "public", "http_status": 200}
    if operation == "getArchiveBlob":
        if preconditions.get("private_release_only"):
            return {
                "authorization": "deny",
                "http_status": 404,
                "error_code": "not_found",
            }
        return {"authorization": "public", "http_status": 200}

    required_scopes = {
        "createSecurityReport": "registry:reports:create",
        "emergencySecurityQuarantineRelease": "registry:security:enforce",
        "createEnforcementAppeal": "registry:appeals:create",
        "reviewEnforcementAppeal": "registry:appeals:review",
        "reviewedReinstateRelease": "registry:security:reinstate",
        "createNamespaceTransfer": "registry:namespaces:transfer",
        "acceptNamespaceTransfer": "registry:namespaces:transfer:accept",
        "executeNamespaceTransfer": "registry:namespaces:transfer",
        "createOwnershipRecovery": "registry:namespaces:recovery:create",
        "reviewOwnershipRecovery": "registry:namespaces:recovery:review",
        "executeOwnershipRecovery": "registry:namespaces:recovery:execute",
    }
    publisher_denied = {
        "emergencySecurityQuarantineRelease",
        "reviewEnforcementAppeal",
        "reviewedReinstateRelease",
        "reviewOwnershipRecovery",
        "executeOwnershipRecovery",
    }
    if (
        case["actor"] == "publisher" and operation in publisher_denied
    ) or required_scopes[operation] not in case["scopes"]:
        return {
            "authorization": "deny",
            "http_status": 403,
            "error_code": "forbidden",
        }
    auth_age = case["recent_auth_age_seconds"]
    if auth_age is not None and auth_age > 900:
        return {
            "authorization": "deny",
            "http_status": 403,
            "error_code": "forbidden",
        }
    if operation == "reviewEnforcementAppeal" and preconditions.get(
        "reviewer_is_original_enforcer"
    ) and not preconditions.get("emergency_waiver"):
        return {
            "authorization": "deny",
            "http_status": 403,
            "error_code": "forbidden",
        }
    if operation == "executeNamespaceTransfer" and (
        preconditions.get("cooling_elapsed_seconds", 0) < 604800
    ):
        return {
            "authorization": "allow",
            "http_status": 409,
            "error_code": "state_transition_forbidden",
        }
    if operation == "executeOwnershipRecovery" and (
        preconditions.get("notice_elapsed_seconds", 0) < 2592000
    ):
        return {
            "authorization": "allow",
            "http_status": 409,
            "error_code": "state_transition_forbidden",
        }
    created = {
        "createSecurityReport",
        "createEnforcementAppeal",
        "reviewEnforcementAppeal",
        "createOwnershipRecovery",
    }
    return {
        "authorization": "allow",
        "http_status": 201 if operation in created else 200,
    }


def parse_openapi_operations(openapi: str) -> dict[str, tuple[str, str, str]]:
    operations: dict[str, tuple[str, str, str]] = {}
    current_path = ""
    current_method = ""
    block_lines: list[str] = []

    def finish_block() -> None:
        nonlocal block_lines
        if not current_path or not current_method or not block_lines:
            block_lines = []
            return
        block = "\n".join(block_lines)
        match = re.search(r"^      operationId: ([A-Za-z][A-Za-z0-9]+)$", block, re.MULTILINE)
        assert match, f"missing operationId for {current_method.upper()} {current_path}"
        operation_id = match.group(1)
        assert operation_id not in operations, operation_id
        assert re.search(r"^      responses:$", block, re.MULTILINE), operation_id
        if current_method in {"post", "delete"}:
            assert "#/components/parameters/IdempotencyKey" in block, operation_id
        operations[operation_id] = (current_method, current_path, block)
        block_lines = []

    for line in openapi.splitlines():
        path_match = re.fullmatch(r"  (/[^:]+):", line)
        if path_match:
            finish_block()
            current_path = path_match.group(1)
            current_method = ""
            continue
        method_match = re.fullmatch(r"    (get|post|put|patch|delete):", line)
        if method_match and current_path:
            finish_block()
            current_method = method_match.group(1)
            block_lines = [line]
            continue
        if current_method:
            block_lines.append(line)
    finish_block()
    return operations


def check_api_contracts() -> None:
    for schema_name, fixture_name in (
        ("package-record.schema.json", "package-record-v1.json"),
        ("release-record.schema.json", "release-record-v1.json"),
        ("source-proof.schema.json", "source-proof-v1.json"),
    ):
        schema = load_json(f"schemas/{schema_name}")
        fixture = load_json(f"fixtures/{fixture_name}")
        errors = schema_errors(fixture, schema, schema)
        assert not errors, f"{fixture_name}: {errors}"

    package = load_json("fixtures/package-record-v1.json")
    release = load_json("fixtures/release-record-v1.json")
    proof = load_json("fixtures/source-proof-v1.json")
    transitions = load_json("fixtures/release-transitions.json")
    assert identity_failure_reason(package["identity"]) == ""
    assert identity_failure_reason(release["package"]) == ""
    assert identity_failure_reason(proof["package"]) == ""
    assert set(release["state"]) == set(transitions["dimensions"])
    assert all(
        release["state"][dimension] in states
        for dimension, states in transitions["dimensions"].items()
    )
    assert proof["status"] == "verified"
    assert proof["verified_at"] is not None
    assert proof["license"]["compatible"] is True
    check_names = {check["name"] for check in proof["checks"]}
    assert check_names == {
        "repository-identity",
        "installation-identity",
        "commit-resolution",
        "archive-digest",
        "license",
    }
    assert all(check["status"] == "passed" for check in proof["checks"])

    proof_schema = load_json("schemas/source-proof.schema.json")
    proof_failures = load_json("fixtures/source-proof-failure-cases.json")
    assert proof_failures["base_source_proof"] == "source-proof-v1.json"
    for case in proof_failures["invalid_cases"]:
        invalid = apply_json_mutations(proof, case["mutations"])
        assert schema_errors(invalid, proof_schema, proof_schema), case["name"]
        assert case["expected"] == "schema-reject"

    error_schema = load_json("schemas/error-envelope.schema.json")
    errors_fixture = load_json("fixtures/api-error-cases-v1.json")
    catalog = errors_fixture["catalog"]
    assert set(error_schema["$defs"]["errorCode"]["enum"]) == set(catalog)
    names: set[str] = set()
    equivalent: dict[str, list[tuple[int, str, dict[str, Any]]]] = {}
    for case in errors_fixture["cases"]:
        assert case["name"] not in names
        names.add(case["name"])
        schema_failures = schema_errors(case["body"], error_schema, error_schema)
        assert not schema_failures, f"{case['name']}: {schema_failures}"
        error = case["body"]["error"]
        expected = catalog[error["code"]]
        assert case["http_status"] == expected["http_status"]
        assert error["retryable"] is expected["retryable"]
        assert case["headers"]["X-Request-Id"] == error["request_id"]
        if error["retryable"]:
            assert case["headers"]["Retry-After"] == error["retry_after_seconds"]
        else:
            assert error["retry_after_seconds"] is None
            assert "Retry-After" not in case["headers"]
        if "equivalence_group" in case:
            equivalent.setdefault(case["equivalence_group"], []).append(
                (case["http_status"], error["code"], error["details"])
            )
    assert len(equivalent["not-found"]) >= 2
    assert len(set(json.dumps(item, sort_keys=True) for item in equivalent["not-found"])) == 1

    behavior = load_json("fixtures/api-behavior-cases-v1.json")
    assert behavior["base_path"] == "/packages/api/v1"
    recent_auth = behavior["recent_auth"]
    assert recent_auth["claim"] == "auth_time"
    assert recent_auth["claim_encoding"] == "jwt-numeric-date-seconds"
    assert set(recent_auth["substitutes_forbidden"]) == {
        "exp",
        "iat",
        "token-issuance-time",
    }
    for case in recent_auth["cases"]:
        decision, error_code = recent_auth_result(
            case["server_now"],
            case["auth_time"],
            recent_auth["maximum_age_seconds"],
            recent_auth["future_tolerance_seconds"],
        )
        assert decision == case["expected"], case["name"]
        assert error_code == case.get("error_code"), case["name"]
    timestamp_pattern = re.compile(
        r"^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}(?:\.[0-9]{1,9})?Z$"
    )
    assert all(timestamp_pattern.fullmatch(item) for item in behavior["timestamps"]["valid"])
    assert not any(timestamp_pattern.fullmatch(item) for item in behavior["timestamps"]["invalid"])
    digest_pattern = re.compile(r"^[0-9a-f]{64}$")
    assert all(digest_pattern.fullmatch(item) for item in behavior["digests"]["valid"])
    assert not any(digest_pattern.fullmatch(item) for item in behavior["digests"]["invalid"])
    idempotency_pattern = re.compile(behavior["idempotency"]["key_pattern"])
    assert idempotency_pattern.fullmatch("seen-publish-0001")
    assert (
        behavior["privacy"]["absent_response"]
        == behavior["privacy"]["concealed_private_response"]
    )
    assert behavior["separation"]["identity_account_flows_in_registry_api"] is False

    openapi = (CONTRACT / "openapi.yaml").read_text(encoding="utf-8")
    assert openapi.startswith("openapi: 3.1.0\n")
    paths = re.findall(r"^  (/[^:]+):$", openapi, re.MULTILINE)
    assert paths and all(path.startswith("/packages/api/v1/") for path in paths)
    operations = parse_openapi_operations(openapi)
    operation_ids = list(operations)
    assert operation_ids
    path_text = " ".join(paths).lower()
    for required in (
        "metadata",
        "report",
        "appeal",
        "transfer",
        "recover",
        "quarantine",
        "reinstate",
    ):
        assert required in path_text, f"OpenAPI path missing {required} operation"
    assert not any(
        forbidden in path.lower()
        for path in paths
        for forbidden in ("/passkey", "/session", "/auth")
    )
    assert "type: http" in openapi and "scheme: bearer" in openapi
    assert "application/gzip" in openapi
    assert "X-Seen-Archive-Sha256" in openapi
    for reference in re.findall(r"\$ref: ['\"]?\./schemas/([^'\"#\n]+)", openapi):
        assert (CONTRACT / "schemas" / reference).is_file(), reference

    service_fixtures = sorted((CONTRACT / "fixtures").glob("*api*conformance*.json"))
    assert service_fixtures, "missing shared service-conformance fixture"
    for fixture_path in service_fixtures:
        fixture = json.loads(fixture_path.read_text(encoding="utf-8"))
        conformance_schema = load_json("schemas/registry-api-conformance-v1.schema.json")
        conformance_errors = schema_errors(
            fixture, conformance_schema, conformance_schema
        )
        assert not conformance_errors, f"{fixture_path.name}: {conformance_errors}"
        assert fixture["contract_version"] == 1
        assert fixture.get("cases"), fixture_path.name
        boundary = fixture["authorization_boundary"]
        assert boundary["required_claims"] == [
            "iss",
            "aud",
            "sub",
            "exp",
            "auth_time",
            "environment",
            "scopes",
        ]
        assert boundary["recent_auth_claim"] == "auth_time"
        assert boundary["recent_auth_max_age_seconds"] == 900
        assert boundary["future_auth_time_tolerance_seconds"] == 60
        assert len({case["name"] for case in fixture["cases"]}) == len(fixture["cases"])
        assert set(fixture["required_operations"]) <= set(operation_ids)
        assert {case["operation_id"] for case in fixture["cases"]} <= set(operation_ids)
        for case in fixture["cases"]:
            expected = {
                key: case["expected"][key]
                for key in ("authorization", "http_status", "error_code")
                if key in case["expected"]
            }
            assert api_conformance_result(case) == expected, case["name"]


def manifest_archive_binding_error(binding: dict[str, Any]) -> str | None:
    if binding["archive_manifest_sha256"] != binding["reserved_manifest_sha256"]:
        return "archive_manifest_digest_mismatch"
    archive_manifest = binding["archive_manifest"]
    reserved_manifest = binding["reserved_manifest"]
    if (
        archive_manifest["package_identity"] != binding["path_identity"]
        or archive_manifest["package_identity"]
        != reserved_manifest["package_identity"]
    ):
        return "archive_manifest_identity_mismatch"
    if (
        archive_manifest["project_version"] != binding["reserved_version"]
        or archive_manifest["project_version"]
        != reserved_manifest["project_version"]
    ):
        return "archive_manifest_version_mismatch"
    for field in ("capabilities", "include", "assets"):
        if archive_manifest[field] != reserved_manifest[field]:
            return f"archive_manifest_{field}_mismatch"
    if binding["all_effective_paths_match_include"] is not True:
        return "archive_include_policy_violation"
    return None


def archive_policy_error(
    policy: dict[str, Any], archive: dict[str, Any]
) -> str | None:
    parser_result = archive["parser_result"]
    if parser_result == "timeout":
        return "archive_validation_timeout"
    if parser_result != "complete":
        return "archive_parse_failed"
    stats = archive["stats"]
    limit_checks = (
        ("compressed_bytes", "compressed_bytes", "archive_compressed_size_limit"),
        ("expanded_bytes", "expanded_bytes", "archive_expanded_size_limit"),
        ("entry_count", "entry_count", "archive_entry_count_limit"),
        ("max_regular_file_bytes", "regular_file_bytes", "archive_file_size_limit"),
        ("max_path_bytes", "path_bytes", "archive_path_length_limit"),
        ("max_path_depth", "path_depth", "archive_path_depth_limit"),
        ("compression_ratio", "compression_ratio", "archive_compression_ratio_limit"),
    )
    for stat, limit, error_code in limit_checks:
        if stats[stat] > policy["limits"][limit]:
            return error_code

    entries = archive["entries"]
    paths = [entry["effective_path"] for entry in entries]
    for path in paths:
        if path.startswith("/") or re.match(r"^[A-Za-z]:[\\/]", path):
            return "archive_path_absolute"
        if "\\" in path or any(ord(character) < 32 for character in path):
            return "archive_path_invalid"
        pieces = path.split("/")
        if any(piece in {".", ".."} for piece in pieces):
            return "archive_path_traversal"
        if any(not piece for piece in pieces):
            return "archive_path_invalid"
    if len(paths) != len(set(paths)):
        return "archive_duplicate_path"
    folded = [path.casefold() for path in paths]
    if len(folded) != len(set(folded)):
        return "archive_portable_case_collision"
    normalized = [unicodedata.normalize("NFC", path) for path in paths]
    if len(normalized) != len(set(normalized)) or any(
        path != normalized_path for path, normalized_path in zip(paths, normalized)
    ):
        return "archive_unicode_normalization_collision"
    allowed_types = set(policy["entry_rules"]["allowed_types"])
    if any(entry["type"] not in allowed_types for entry in entries):
        return "archive_entry_type_forbidden"
    if any(
        entry["type"] == "regular-file" and int(entry["mode"], 8) & 0o111
        for entry in entries
    ):
        return "archive_executable_forbidden"
    content_rules = policy["content_rules"]
    classifiers = content_rules["path_name_classifiers"]
    magic_prefixes = [item["prefix_hex"] for item in content_rules["binary_magic_prefixes"]]
    script_prefixes = [item["prefix_hex"] for item in content_rules["script_content_prefixes"]]
    for entry in entries:
        if entry["type"] != "regular-file":
            continue
        path = entry["effective_path"].lower()
        basename = path.rsplit("/", 1)[-1]
        stem = basename.rsplit(".", 1)[0]
        suffix = f".{basename.rsplit('.', 1)[1]}" if "." in basename else ""
        leading_bytes = entry.get("leading_bytes_hex", "")
        if (
            suffix in classifiers["prebuilt_suffixes"]
            or basename in classifiers["prebuilt_basenames"]
            or any(leading_bytes.startswith(prefix) for prefix in magic_prefixes)
        ):
            return content_rules["prebuilt_error_code"]
        has_script_content = any(
            leading_bytes.startswith(prefix) for prefix in script_prefixes
        )
        path_segments = set(path.split("/")[:-1])
        if stem in classifiers["forbidden_lifecycle_stems"] or (
            has_script_content
            and path_segments.intersection(
                classifiers["forbidden_script_path_segments"]
            )
        ):
            return content_rules["script_error_code"]
    manifest_entries = [
        entry
        for entry in entries
        if entry["effective_path"] == "Seen.toml"
        and entry["type"] == "regular-file"
    ]
    if len(manifest_entries) != 1:
        return "archive_manifest_missing"
    if any(not entry["included_by_manifest"] for entry in entries):
        return "archive_include_policy_violation"
    return None


def check_archive_contracts() -> None:
    schema = load_json("schemas/archive-policy-v1.schema.json")
    policy = load_json("fixtures/archive-policy-v1.json")
    errors = schema_errors(policy, schema, schema)
    assert not errors, errors
    assert policy["archive_format"] == "tar+gzip"
    assert policy["content_model"] == "source-only"
    assert policy["failure_mode"].startswith("reject-closed")
    entry_rules = policy["entry_rules"]
    assert entry_rules["manifest_hash_input"] == "exact-raw-Seen.toml-bytes"
    assert entry_rules["manifest_reservation_binding"] == (
        "exact-hash-and-complete-parsed-value-before-ready"
    )
    assert entry_rules["manifest_bound_fields"] == [
        "package.identity",
        "project.version",
        "package.capabilities",
        "package.include",
        "package.assets",
    ]

    case_schema = load_json("schemas/archive-validation-case-v1.schema.json")
    fixture = load_json("fixtures/archive-policy-cases.json")
    names = [case["name"] for case in fixture["cases"]]
    assert len(names) == len(set(names))
    assert any(case["expected"] == "accept" for case in fixture["cases"])
    assert any(case["expected"] == "reject" for case in fixture["cases"])
    for case in fixture["cases"]:
        failures = schema_errors(case, case_schema, case_schema)
        assert not failures, f"{case['name']}: {failures}"
        observed = archive_policy_error(policy, case["archive"])
        expected = case.get("error_code")
        assert observed == expected, case["name"]
    coverage = " ".join(
        names + [case.get("description", "") for case in fixture["cases"]]
    ).lower()
    for required in (
        "traversal",
        "absolute",
        "symlink",
        "device",
        "collision",
        "bomb",
        "executable",
        "manifest",
        "truncated",
        "timeout",
        "prebuilt",
        "lifecycle",
        "binary",
    ):
        assert required in coverage, f"missing hostile archive case: {required}"

    binding_fixture = load_json("fixtures/manifest-archive-binding-cases-v1.json")
    assert binding_fixture["comparison"] == (
        "exact-hash-and-complete-parsed-value-before-ready"
    )
    binding_base = binding_fixture["base"]
    assert identity_failure_reason(binding_base["path_identity"]) == ""
    assert is_exact_semver(binding_base["reserved_version"])
    binding_names = [case["name"] for case in binding_fixture["cases"]]
    assert len(binding_names) == len(set(binding_names))
    for case in binding_fixture["cases"]:
        binding = apply_json_mutations(binding_base, case["mutations"])
        observed = manifest_archive_binding_error(binding)
        assert observed == case.get("error_code"), case["name"]
        assert case["expected"] == ("accept" if observed is None else "reject")


def target_binding_error(
    role_name: str,
    target_path: str,
    target_sha256: str,
    custom: dict[str, Any],
) -> str | None:
    pieces = target_path.split("/")
    if len(pieces) != 6 or pieces[0] != "packages":
        return "signing_path_not_delegated"
    _, owner, name, path_version, path_digest, path_leaf = pieces
    if f"{owner}/{name}" != custom["package"]:
        return "signing_target_path_identity_mismatch"
    if path_version != custom["version"]:
        return "signing_target_path_version_mismatch"
    if path_digest != custom["archive_sha256"]:
        return "signing_target_path_digest_mismatch"
    if path_leaf != custom["archive_filename"]:
        return "signing_target_path_leaf_mismatch"
    if target_sha256 != custom["archive_sha256"]:
        return "signing_target_hash_mismatch"

    if role_name == "releases":
        if custom["visibility"] != "public":
            return "signing_release_not_public"
        if custom["lifecycle"] != "active":
            return "signing_release_not_active"
        if custom["retention"] != "retained":
            return "signing_release_not_retained"
        if custom["availability"] == "security-quarantined":
            return "signing_wrong_role"
        if custom["availability"] not in {"available", "yanked"}:
            return "signing_release_availability_invalid"
    elif role_name == "security":
        if custom["visibility"] != "public":
            return "signing_release_not_public"
        if custom["lifecycle"] != "active":
            return "signing_release_not_active"
        if custom["retention"] != "retained":
            return "signing_release_not_retained"
        if custom["availability"] == "security-quarantined":
            if custom.get("security_action") != "quarantine":
                return "signing_security_action_invalid"
            if not custom.get("incident_id"):
                return "signing_security_incident_required"
        elif custom["availability"] == "available":
            if custom.get("security_action") != "reinstate-reviewed":
                return "signing_security_action_invalid"
            if not custom.get("incident_id"):
                return "signing_security_incident_required"
        else:
            return "signing_security_action_invalid"
    else:
        return "signing_wrong_role"
    return None


def signing_case_error(
    case: dict[str, Any], policy: dict[str, Any], validation_time: str
) -> str | None:
    environments = {item["name"]: item for item in policy["environments"]}
    if "development_keyid" in case and case["development_keyid"] == case.get(
        "production_keyid"
    ):
        return "signing_cross_environment_key_reuse"
    if case.get("development_kms_resource") == case.get(
        "production_kms_resource"
    ) and "development_kms_resource" in case:
        return "signing_cross_environment_kms_reuse"

    environment_name = case.get("environment")
    if environment_name is None:
        return None
    environment = environments[environment_name]
    if case.get("metadata_repository_id", environment["repository_id"]) != environment[
        "repository_id"
    ]:
        return "signing_repository_mismatch"
    if case.get("metadata_registry_origin", environment["registry_origin"]) != environment[
        "registry_origin"
    ]:
        return "signing_origin_mismatch"
    validation = datetime.fromisoformat(validation_time.replace("Z", "+00:00"))
    if case.get("network") == "unavailable":
        cached_expiry = datetime.fromisoformat(
            case["cached_metadata_expires"].replace("Z", "+00:00")
        )
        if cached_expiry <= validation:
            return "signing_no_fresh_trusted_metadata"

    role_name = case.get("role")
    if case.get("action") == "security-quarantine-target":
        role_name = "security"
    role: dict[str, Any] | None = None
    if role_name is not None:
        for group in ("tuf_roles", "delegated_roles", "attestation_roles"):
            if role_name in environment[group]:
                role = environment[group][role_name]
                break
        assert role is not None, role_name

    if case.get("key_status") == "revoked":
        return "signing_revoked_key"
    keyids = case.get("keyids")
    if keyids is not None and role is not None:
        environment_prefix = "dev-" if environment_name == "development" else "prod-"
        if any(
            keyid.startswith("dev-") or keyid.startswith("prod-")
            for keyid in keyids
            if not keyid.startswith(environment_prefix)
        ):
            return "signing_environment_mismatch"
        known_keys = {item["keyid"] for item in environment["keys"]}
        if any(keyid not in known_keys for keyid in keyids):
            return "signing_unknown_key"
        if any(keyid not in role["keyids"] for keyid in keyids):
            return "signing_wrong_role"
        if len(set(keyids)) < role["threshold"]:
            return "signing_threshold_not_met"

    target_path = case.get("target_path")
    if target_path is not None and role_name in {"releases", "security"}:
        if len(target_path.split("/")) != 6 or not target_path.startswith("packages/"):
            return "signing_path_not_delegated"
        if "metadata_package" in case:
            pieces = target_path.split("/")
            if "/".join(pieces[1:3]) != case["metadata_package"]:
                return "signing_target_path_identity_mismatch"
    if role_name == "releases" and "visibility" in case:
        if case["visibility"] != "public":
            return "signing_release_not_public"
        if case.get("lifecycle") != "active":
            return "signing_release_not_active"
        if case.get("retention") != "retained":
            return "signing_release_not_retained"
        if case.get("availability") == "security-quarantined":
            return "signing_wrong_role"
    if "expires" in case:
        expiry = datetime.fromisoformat(case["expires"].replace("Z", "+00:00"))
        if expiry <= validation:
            return "signing_metadata_expired"
    if "metadata_version" in case and "previous_trusted_version" in case:
        current = case["metadata_version"]
        previous = case["previous_trusted_version"]
        if current < previous:
            return (
                "signing_root_rollback"
                if role_name == "root"
                else "signing_metadata_rollback"
            )
        if current == previous and role_name == "timestamp":
            return "signing_freeze_detected"
    if "old_root_version" in case:
        if case["new_root_version"] != case["old_root_version"] + 1:
            return "signing_nonsequential_root_version"
        threshold = environment["tuf_roles"]["root"]["threshold"]
        if len(set(case.get("old_root_signatures", []))) < threshold:
            return "signing_old_root_threshold_not_met"
        if len(set(case.get("new_root_signatures", []))) < threshold:
            return "signing_new_root_threshold_not_met"
    if role_name == "root" and case.get("key_online") is True:
        return "signing_root_must_be_offline"
    if "expected_length" in case and case["expected_length"] != case["actual_length"]:
        return "signing_target_length_mismatch"
    if "expected_sha256" in case and case["expected_sha256"] != case["actual_sha256"]:
        return (
            "signing_target_hash_mismatch"
            if role_name in {"releases", "security"}
            else "signing_metadata_hash_mismatch"
        )
    return None


def check_signing_contracts() -> None:
    schema = load_json("schemas/signing-policy-v1.schema.json")
    policy = load_json("fixtures/signing-policy-v1.json")
    failures = schema_errors(policy, schema, schema)
    assert not failures, failures
    assert policy["canonical_serialization"] == "seen-tuf-canonical-json-v1"
    serialization = policy["canonical_serialization_rules"]
    assert serialization == {
        "scope": "selected-json-value",
        "signature_input": "envelope.signed",
        "metadata_file_input": "complete-envelope-including-signatures",
        "encoding": "utf-8",
        "object_key_order": "unicode-code-point-ascending",
        "separators": "comma-and-colon-without-whitespace",
        "string_escaping": (
            "rfc8259-minimal-short-control-escapes-otherwise-lowercase-u00xx"
        ),
        "unicode": "emit-unescaped-unicode-scalars",
        "numbers": "base-10-integers-no-leading-zeroes",
        "trailing_newline": False,
    }
    environments = {item["name"]: item for item in policy["environments"]}
    assert set(environments) == {"development", "production"}
    assert (
        environments["development"]["registry_origin"]
        == "https://seen.dev.yousef.codes/packages"
    )
    assert environments["production"]["registry_origin"] == "https://seen.yousef.codes/packages"

    key_sets: dict[str, set[str]] = {}
    public_keys: dict[str, set[str]] = {}
    kms_resources: dict[str, set[str]] = {}
    for environment_name, environment in environments.items():
        keys = {item["keyid"]: item for item in environment["keys"]}
        assert len(keys) == len(environment["keys"])
        key_sets[environment_name] = set(keys)
        public_keys[environment_name] = {item["public_key"] for item in keys.values()}
        kms_resources[environment_name] = {
            item["kms_resource"] for item in keys.values() if "kms_resource" in item
        }
        assert environment["repository_id"].startswith(
            "seen-dev-" if environment_name == "development" else "seen-prod-"
        )
        assert environment["metadata_prefix"].endswith(environment_name)
        for role_group in ("tuf_roles", "delegated_roles", "attestation_roles"):
            for role in environment[role_group].values():
                assert set(role["keyids"]) <= set(keys)
                assert 1 <= role["threshold"] <= len(role["keyids"])
                assert role["environment_bound"] is True
                assert role["repository_id_bound"] is True
        assert environment["tuf_roles"]["root"]["online"] is False
        assert environment["root_rotation"]["requires_old_root_threshold"] is True
        assert environment["root_rotation"]["requires_new_root_threshold"] is True
    assert key_sets["development"].isdisjoint(key_sets["production"])
    assert public_keys["development"].isdisjoint(public_keys["production"])
    assert kms_resources["development"].isdisjoint(kms_resources["production"])
    production = environments["production"]
    for role_group, role_name in (
        ("tuf_roles", "root"),
        ("tuf_roles", "targets"),
        ("delegated_roles", "releases"),
        ("delegated_roles", "security"),
    ):
        assert production[role_group][role_name]["threshold"] >= 2

    tuf_schema = load_json("schemas/tuf-metadata-envelope-v1.schema.json")
    tuf_fixture = load_json("fixtures/tuf-metadata-examples.json")
    assert tuf_fixture["canonical_serialization"] == policy["canonical_serialization"]
    metadata = tuf_fixture["metadata"]
    assert {
        "root",
        "targets",
        "release_targets",
        "security_targets",
        "snapshot",
        "timestamp",
    } <= set(metadata)
    role_for_document = {
        "root": ("tuf_roles", "root"),
        "targets": ("tuf_roles", "targets"),
        "release_targets": ("delegated_roles", "releases"),
        "security_targets": ("delegated_roles", "security"),
        "snapshot": ("tuf_roles", "snapshot"),
        "timestamp": ("tuf_roles", "timestamp"),
    }
    for document_name, document in metadata.items():
        errors = schema_errors(document, tuf_schema, tuf_schema)
        assert not errors, f"{document_name}: {errors}"
        signed = document["signed"]
        assert signed["environment"] == "production"
        assert signed["repository_id"] == production["repository_id"]
        group, role_name = role_for_document[document_name]
        role = production[group][role_name]
        signers = {signature["keyid"] for signature in document["signatures"]}
        assert signers <= set(role["keyids"])
        assert len(signers) >= role["threshold"]
        for target_path, target in signed.get("targets", {}).items():
            assert target["hashes"]["sha256"] == target["custom"]["archive_sha256"]
            assert target["custom"]["environment"] == "production"
            assert target["custom"]["registry_origin"] == production["registry_origin"]
            if document_name == "release_targets":
                assert target_binding_error(
                    "releases",
                    target_path,
                    target["hashes"]["sha256"],
                    target["custom"],
                ) is None
            if document_name == "security_targets":
                assert target_binding_error(
                    "security",
                    target_path,
                    target["hashes"]["sha256"],
                    target["custom"],
                ) is None

    chain = tuf_fixture["metadata_chain"]
    for meta_name, document_name in chain["snapshot_meta_documents"].items():
        document = metadata[document_name]
        canonical = canonical_json_bytes(document)
        descriptor = metadata["snapshot"]["signed"]["meta"][meta_name]
        assert descriptor == {
            "version": document["signed"]["version"],
            "length": len(canonical),
            "hashes": {"sha256": hashlib.sha256(canonical).hexdigest()},
        }, meta_name
    for meta_name, document_name in chain["timestamp_meta_documents"].items():
        document = metadata[document_name]
        canonical = canonical_json_bytes(document)
        descriptor = metadata["timestamp"]["signed"]["meta"][meta_name]
        assert descriptor == {
            "version": document["signed"]["version"],
            "length": len(canonical),
            "hashes": {"sha256": hashlib.sha256(canonical).hexdigest()},
        }, meta_name

    binding_names = [case["name"] for case in tuf_fixture["target_binding_cases"]]
    assert len(binding_names) == len(set(binding_names))
    for case in tuf_fixture["target_binding_cases"]:
        observed = target_binding_error(
            case["role"],
            case["target_path"],
            case["target_sha256"],
            case["custom"],
        )
        assert observed == case.get("error_code"), case["name"]
        assert case["expected"] == ("accept" if observed is None else "reject")

    assert production["target_overlay_policy"]["evaluation_order"] == [
        "security",
        "releases",
    ]
    for case in tuf_fixture["overlay_resolution_cases"]:
        candidates = {item["role"]: item for item in case["candidates"]}
        security = candidates["security"]
        release = candidates["releases"]
        if (
            security["present"]
            and security.get("availability") == "security-quarantined"
            and not security.get("incident_id")
        ):
            observed = {
                "decision": "reject",
                "error_code": "signing_security_incident_required",
            }
        elif security["present"] and security.get("archive_sha256") != release.get(
            "archive_sha256"
        ):
            observed = {
                "decision": "reject",
                "error_code": "signing_overlay_digest_mismatch",
            }
        elif security["present"]:
            observed = {
                "decision": "select",
                "role": "security",
                "availability": security["availability"],
            }
        else:
            observed = {
                "decision": "select",
                "role": "releases",
                "availability": release["availability"],
            }
        assert observed == case["expected"], case["name"]

    failure_fixture = load_json("fixtures/signing-failure-cases.json")
    for case in failure_fixture["valid"]:
        assert case["expected"] == "accept"
        assert signing_case_error(
            case, policy, failure_fixture["validation_time"]
        ) is None, case["name"]
    for case in failure_fixture["invalid"]:
        assert case["expected"] == "reject"
        assert signing_case_error(
            case, policy, failure_fixture["validation_time"]
        ) == case["error_code"], case["name"]
    invalid_names = [case["name"] for case in failure_fixture["invalid"]]
    assert len(invalid_names) == len(set(invalid_names))
    error_codes = {case["error_code"] for case in failure_fixture["invalid"]}
    for required in (
        "signing_environment_mismatch",
        "signing_threshold_not_met",
        "signing_wrong_role",
        "signing_metadata_expired",
        "signing_metadata_rollback",
        "signing_target_hash_mismatch",
        "signing_cross_environment_key_reuse",
        "signing_cross_environment_kms_reuse",
    ):
        assert required in error_codes


def apply_json_mutations(value: Any, mutations: list[dict[str, Any]]) -> Any:
    result = deepcopy(value)
    for mutation in mutations:
        pieces = [
            piece.replace("~1", "/").replace("~0", "~")
            for piece in mutation["path"].split("/")[1:]
        ]
        parent = result
        for piece in pieces[:-1]:
            parent = parent[int(piece)] if isinstance(parent, list) else parent[piece]
        key = pieces[-1]
        operation = mutation.get("op", "replace")
        if operation in {"replace", "add"}:
            if isinstance(parent, list):
                parent[int(key)] = mutation["value"]
            else:
                parent[key] = mutation["value"]
        elif operation == "remove":
            if isinstance(parent, list):
                parent.pop(int(key))
            else:
                del parent[key]
        else:
            raise AssertionError(f"unsupported fixture mutation: {operation}")
    return result


def provenance_semantic_error(
    attestation: dict[str, Any], trusted: dict[str, Any]
) -> str | None:
    git_materials = [
        item
        for item in attestation["materials"]
        if re.match(r"^git\+https://(?:github\.com|gitlab\.com)/", item["uri"])
    ]
    proof_materials = [
        item
        for item in attestation["materials"]
        if item["uri"].startswith("seen:source-proof:")
    ]
    policy_materials = [
        item
        for item in attestation["materials"]
        if item["uri"].startswith("seen:archive-policy:")
    ]
    if len(git_materials) != 1 or len(proof_materials) != 1 or len(
        policy_materials
    ) != 1:
        return "provenance_material_cardinality"
    if not is_exact_semver(attestation["subject"]["version"]):
        return "provenance_subject_version_invalid"
    if proof_materials[0]["sha256"] != attestation["source_proof_sha256"]:
        return "provenance_source_proof_mismatch"
    if git_materials[0]["sha256"] != attestation["packing"]["source_tree_sha256"]:
        return "provenance_material_digest_mismatch"
    if (
        attestation["packing"]["source_tree_sha256"]
        != attestation["packing"]["packed_tree_sha256"]
    ):
        return "provenance_packed_tree_mismatch"
    if attestation["subject"]["archive_sha256"] != trusted["subject"]["archive_sha256"]:
        return "provenance_subject_digest_mismatch"
    if attestation["source_proof_sha256"] != trusted["source_proof_sha256"]:
        return "provenance_source_proof_mismatch"
    if (
        attestation["packing"]["manifest_sha256"]
        != trusted["packing"]["manifest_sha256"]
    ):
        return "provenance_manifest_digest_mismatch"
    if attestation["materials"] != trusted["materials"]:
        return "provenance_material_digest_mismatch"
    if attestation["packing"]["packed_tree_sha256"] != trusted["packing"]["packed_tree_sha256"]:
        return "provenance_packed_tree_mismatch"
    if attestation["packing"]["archive_policy_result"] != "passed":
        return "provenance_archive_policy_failed"
    if not attestation["builder"]["isolated"] or attestation["builder"]["network_access"] != "none":
        return "provenance_builder_not_isolated"
    if attestation["builder"]["secret_access"] != "signing-service-only":
        return "provenance_builder_secret_scope"
    if attestation["environment"] != trusted["environment"]:
        return "provenance_environment_mismatch"
    if attestation["repository_id"] != trusted["repository_id"]:
        return "provenance_repository_mismatch"
    environment_prefix = "prod" if attestation["environment"] == "production" else "dev"
    if not attestation["builder"]["workload_identity"].startswith(
        f"seen-{environment_prefix}-packer@"
    ):
        return "provenance_workload_environment_mismatch"
    if not attestation["signature"]["keyid"].startswith(
        f"{environment_prefix}-provenance-"
    ):
        return "provenance_signature_wrong_role"
    started = datetime.fromisoformat(attestation["invocation"]["started_at"].replace("Z", "+00:00"))
    finished = datetime.fromisoformat(
        attestation["invocation"]["finished_at"].replace("Z", "+00:00")
    )
    generated = datetime.fromisoformat(attestation["generated_at"].replace("Z", "+00:00"))
    if not started <= finished <= generated:
        return "provenance_invalid_time_order"
    if attestation["invocation"]["reproducible"] is not True:
        return "provenance_nonreproducible_pack"
    if "archive_sha256" in attestation["packing"] and (
        attestation["packing"]["archive_sha256"]
        != attestation["subject"]["archive_sha256"]
    ):
        return "provenance_subject_digest_mismatch"
    canonical_statement = deepcopy(attestation)
    del canonical_statement["statement_sha256"]
    del canonical_statement["signature"]
    statement_digest = hashlib.sha256(canonical_json_bytes(canonical_statement)).hexdigest()
    if attestation["statement_sha256"] != statement_digest:
        return "provenance_statement_digest_mismatch"
    return None


def check_provenance_contracts() -> None:
    schema = load_json("schemas/provenance-attestation-v1.schema.json")
    base = load_json("fixtures/provenance-attestation-v1.json")
    errors = schema_errors(base, schema, schema)
    assert not errors, errors
    assert provenance_semantic_error(base, base) is None
    fixture = load_json("fixtures/provenance-failure-cases.json")
    assert fixture["base_attestation"] == "provenance-attestation-v1.json"
    names = [case["name"] for case in fixture["invalid_cases"]]
    assert len(names) == len(set(names))
    for case in fixture["invalid_cases"]:
        mutated = apply_json_mutations(base, case["mutations"])
        observed = provenance_semantic_error(mutated, base)
        if observed is None:
            schema_failures = schema_errors(mutated, schema, schema)
            assert schema_failures, case["name"]
        else:
            assert observed == case["error_code"], case["name"]
        assert case["expected"] == "reject"


def transition_matches(
    transition: dict[str, Any], state: dict[str, str], destination: str
) -> bool:
    if transition["from"] != state[transition["dimension"]]:
        return False
    if transition["to"] != destination:
        return False
    return all(state.get(key) == value for key, value in transition.get("when", {}).items())


def state_satisfies_invariants(
    fixture: dict[str, Any], state: dict[str, str]
) -> bool:
    dimensions = fixture["dimensions"]
    if set(state) != set(dimensions):
        return False
    if any(state[key] not in dimensions[key] for key in dimensions):
        return False
    for invariant in fixture["state_invariants"]:
        applies = all(
            state.get(key) == value
            for key, value in invariant.get("when", {}).items()
        )
        if "when_not" in invariant:
            applies = applies and all(
                state.get(key) != value
                for key, value in invariant["when_not"].items()
            )
        if applies and not all(
            state.get(key) == value
            for key, value in invariant["requires"].items()
        ):
            return False
    return True


def apply_trace_step(
    fixture: dict[str, Any], state: dict[str, str], step: dict[str, Any]
) -> bool:
    if not state_satisfies_invariants(fixture, state):
        return False
    dimension = step["dimension"]
    forbidden = [
        item
        for item in fixture["forbidden_transitions"]
        if item["dimension"] == dimension
    ]
    if any(
        transition_matches(item, state, step["to"])
        for item in forbidden
    ):
        return False
    transitions = fixture[f"{dimension}_transitions"]
    normalized = [{**item, "dimension": dimension} for item in transitions]
    evidence = set(step["evidence"])
    for transition in normalized:
        if not transition_matches(transition, state, step["to"]):
            continue
        if not set(transition["requires"]) <= evidence:
            continue
        next_state = dict(state)
        next_state[dimension] = transition["to"]
        next_state.update(transition.get("sets", {}))
        if not state_satisfies_invariants(fixture, next_state):
            return False
        state.clear()
        state.update(next_state)
        return True
    return False


def check_release_transitions() -> None:
    fixture = load_json("fixtures/release-transitions.json")
    dimensions = fixture["dimensions"]
    assert set(dimensions) == {"lifecycle", "visibility", "availability", "retention"}
    assert set(fixture["initial_state"]) == set(dimensions)
    assert state_satisfies_invariants(fixture, fixture["initial_state"])

    for invariant in fixture["state_invariants"]:
        for condition_name in ("when", "when_not", "requires"):
            for key, value in invariant.get(condition_name, {}).items():
                assert key in dimensions
                assert value in dimensions[key]

    for dimension, states in dimensions.items():
        transitions = fixture[f"{dimension}_transitions"]
        for transition in transitions:
            assert transition["from"] in states
            assert transition["to"] in states
            assert transition["requires"]
            assert set(transition.get("sets", {})) <= set(dimensions) - {dimension}
            for key, value in transition.get("sets", {}).items():
                assert value in dimensions[key]
            for key, value in transition.get("when", {}).items():
                assert key in dimensions
                assert value in dimensions[key]

    for transition in fixture["forbidden_transitions"]:
        dimension = transition["dimension"]
        assert dimension in dimensions
        assert transition["from"] in dimensions[dimension]
        assert transition["to"] in dimensions[dimension]
        for key, value in transition.get("when", {}).items():
            assert key in dimensions
            assert value in dimensions[key]

    public_skip = {
        (item["from"], item["to"], item.get("when", {}).get("visibility"))
        for item in fixture["forbidden_transitions"]
        if item["dimension"] == "lifecycle"
    }
    assert ("first-scanning", "ready", "public") in public_skip
    assert ("delayed", "ready", None) in public_skip
    assert "manifest" in fixture["immutable_with_archive_digest"]
    assert "source_proof_checks" in fixture["append_only_attestations"]

    release_schema = load_json("schemas/release-record.schema.json")
    release_base = load_json("fixtures/release-record-v1.json")
    state_fixture = load_json("fixtures/release-record-state-cases-v1.json")
    assert state_fixture["base_release_record"] == "release-record-v1.json"
    state_names = [case["name"] for case in state_fixture["cases"]]
    assert len(state_names) == len(set(state_names))
    for case in state_fixture["cases"]:
        release = deepcopy(release_base)
        release["state"] = case["state"]
        errors = schema_errors(release, release_schema, release_schema)
        valid_state = state_satisfies_invariants(fixture, case["state"])
        if case["expected"] == "accept":
            assert not errors, f"{case['name']}: {errors}"
            assert valid_state, case["name"]
        else:
            assert errors, case["name"]
            assert not valid_state, case["name"]
            assert case["error_code"].startswith("release_state_")

    for case in fixture["trace_cases"]:
        assert case["initial"] == fixture["initial_state"], case["name"]
        state = dict(case["initial"])
        assert state_satisfies_invariants(fixture, state), case["name"]
        failure_step: int | None = None
        for index, step in enumerate(case["steps"]):
            if not apply_trace_step(fixture, state, step):
                failure_step = index
                break
        if case["expected"] == "allow":
            assert failure_step is None, case["name"]
            if "final" in case:
                assert state == case["final"], case["name"]
        else:
            assert failure_step == case["failure_step"], case["name"]


def check_compiler_separation() -> None:
    source = (ROOT / "compiler_seen" / "src" / "main_compiler.seen").read_text(
        encoding="utf-8"
    )
    package_class = source.split("class PackageDependency {", 1)[1].split(
        "}\n\n", 1
    )[0]
    for field in (
        "var alias: String",
        "var packageIdentity: String",
        "var registryAlias: String",
        "var resolvedRegistryOrigin: String",
        "var resolvedArchiveSha256: String",
    ):
        assert field in package_class
    assert "var name: String" not in package_class
    assert "var registry: String" not in package_class
    assert "extractTomlPackageIdentity" in source
    assert 'section == "package"' in source
    assert 'key == "identity"' in source
    assert 'var lockContent = "version = 2\\n"' in source
    assert '"registry_origin = "' in source
    assert '"archive_sha256 = "' in source
    assert "packageLockTomlString(dep.resolvedRegistryOrigin)" in source
    assert '"name = \\"" + dep.alias' not in source
    assert 'return "https://seen.dev.yousef.codes/packages"' in source
    assert "hosted registry resolution is disabled" in source
    assert "curl -fsSL" not in source
    assert 'dep.source = "invalid"' in source
    assert "packageDependencyInlineFieldsAreKnownAndUnique" in source
    resolver = source.split("fun resolvePackageImportPath(", 1)[1].split(
        "\nfun resolveProjectSelfImportPath", 1
    )[0]
    assert 'rootModPath = packageRoot + "/src/mod.seen"' in resolver
    assert "return directPath\n}" not in resolver
    assert 'return versionRoot + "/" + archiveSha256' in source
    assert source.count("package [project].name must be a portable ASCII module root") == 3
    assert source.count("isExactSemanticVersion(version)") == 3
    assert "isExactSemanticVersion(dep.version)" in source

    atomic_writer = source.split("fun writeTextAtomically(", 1)[1].split(
        "\nfun isHttpRegistryLocation", 1
    )[0]
    assert 'let tempPath = path + ".tmp." + getPid().toString()' in atomic_writer
    assert "if not writeText(tempPath, content)" in atomic_writer
    assert 'runCommand("mv -f " +' in atomic_writer
    assert "if not commandWasSuccessful(renameResult)" in atomic_writer
    assert atomic_writer.count("removePathIfExists(tempPath)") >= 3

    lock_writer = source.split("fun writePackageLockfile(", 1)[1].split(
        "\nfun ensureProjectPackageDependenciesForManifest", 1
    )[0]
    assert ") r: Bool {" in lock_writer
    assert 'return writeTextAtomically(projectRoot + "/Seen.lock", lockContent)' in (
        lock_writer
    )
    dependency_gate = source.split(
        "fun ensureProjectPackageDependenciesForManifest(", 1
    )[1].split("\nfun ensureProjectPackageDependenciesForInput", 1)[0]
    assert "if not writePackageLockfile(projectRoot, deps)" in dependency_gate
    assert "if exists(lockPath)" in dependency_gate

    publish_flow = source.split('if subcommand == "publish" {', 1)[1].split(
        "\n    printPkgUsage()", 1
    )[0]
    assert "if not writeTextAtomically(indexPath, newIndex)" in publish_flow
    failed_index = publish_flow.split(
        "if not writeTextAtomically(indexPath, newIndex)", 1
    )[1].split("return 1", 1)[0]
    assert "removePathIfExists(archivePath)" in failed_index

    inline_validator = source.split(
        "fun packageDependencyInlineFieldsAreKnownAndUnique(", 1
    )[1].split("\nfun parsePackageDependencyEntry", 1)[0]
    for parser in (
        "scanTomlInlineQuotedString",
        "scanTomlInlineStringArray",
        "skipTomlInlineWhitespace",
    ):
        assert parser in inline_validator
    for counter in (
        "packageCount",
        "versionCount",
        "registryCount",
        "pathCount",
        "artifactCount",
        "allowCount",
        "systemCount",
    ):
        assert f"{counter} > 1" in inline_validator
    assert "if pos >= value.length() or value.byteAt(pos) == 125" in inline_validator

    boolean_parser = source.split("fun extractTomlInlineBooleanField(", 1)[1].split(
        "\nfun packageDependencyInlineFieldsAreKnownAndUnique", 1
    )[0]
    assert "var inQuote = false" in boolean_parser
    assert "var escaped = false" in boolean_parser
    dependency_parser = source.split("fun parsePackageDependencyEntry(", 1)[1].split(
        "\nfun extractTomlPackageDependencies", 1
    )[0]
    assert (
        "if not packageDependencyInlineFieldsAreKnownAndUnique(trimmedValue)"
        in dependency_parser
    )
    assert (
        'extractTomlInlineBooleanField(trimmedValue, "system") == 1'
        in dependency_parser
    )
    assert 'contains("system = true")' not in dependency_parser
    dependency_extractor = source.split(
        "fun extractTomlPackageDependencies(", 1
    )[1].split("\nfun countRegistryPackageDependencies", 1)[0]
    assert 'dep.error = "duplicate dependency alias"' in dependency_extractor

    assert "packageDependencyInlineFieldsAreKnownAndUnique(tomlVal)" in source
    assert 'extractTomlInlineBooleanField(tomlVal, "system") == 1' in source

    file_source = (ROOT / "seen_std" / "src" / "io" / "file.seen").read_text(
        encoding="utf-8"
    )
    write_text = file_source.split("fun writeText(", 1)[1].split(
        "\nfun appendText", 1
    )[0]
    assert "return written == content.length() and closeResult >= 0" in write_text


def check_draft_security_boundary() -> None:
    readme = (CONTRACT / "README.md").read_text(encoding="utf-8")
    threat_model = (CONTRACT / "threat-model.md").read_text(encoding="utf-8")
    packaging = (ROOT / "docs" / "packaging.md").read_text(encoding="utf-8")
    assert "not yet the normative v1 contract" in readme
    assert "Public upload" in readme and "must remain disabled" in readme
    assert "Signing-key compromise" in threat_model
    table_rows = [
        line
        for line in threat_model.splitlines()
        if line.startswith("|") and not line.startswith("| ---")
    ]
    assert table_rows[0] == (
        "| Threat | Required mitigation | Residual risk | Detection | Owner | Response |"
    )
    threats = {row.split("|")[1].strip() for row in table_rows[1:]}
    for required in (
        "Dependency confusion or registry substitution",
        "Typosquatting and confusables",
        "Account takeover",
        "Archive traversal, links, devices, or bombs",
        "Metadata rollback or freeze",
        "Signing-key compromise",
        "Forged source provenance",
        "Malicious source that passes scans",
        "Scanner escape or outage",
        "Cross-tenant private access or cache leakage",
        "Abuse or denial of service",
    ):
        assert required in threats
    for row in table_rows[1:]:
        columns = [column.strip() for column in row.split("|")[1:-1]]
        assert len(columns) == 6 and all(columns), row
    assert "legacy local-static" in packaging
    assert "must not be exposed as a public write path" in packaging


def main() -> None:
    check_identities()
    check_manifest()
    check_resolver_lock()
    check_semantic_versions()
    check_semver_requirements()
    check_registry_origins()
    check_api_contracts()
    check_archive_contracts()
    check_signing_contracts()
    check_provenance_contracts()
    check_release_transitions()
    check_compiler_separation()
    check_draft_security_boundary()
    print("PASS: Seen package registry v1 draft contracts and shared fixtures")


if __name__ == "__main__":
    main()
