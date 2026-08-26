#!/usr/bin/env python3
"""Fail-closed validator for bounded Qwen schemas, locks, and evidence."""

from __future__ import annotations

import argparse
import json
import re
import sys
from datetime import datetime
from pathlib import Path
from typing import Any

MAX_BYTES = 8 * 1024 * 1024


class ContractError(ValueError):
    pass


def fail(path: str, message: str) -> None:
    raise ContractError(f"{path}: {message}")


def object_pairs(path: Path):
    def reject_duplicates(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
        result: dict[str, Any] = {}
        for key, value in pairs:
            if key in result:
                fail(str(path), f"duplicate JSON key {key!r}")
            result[key] = value
        return result
    return reject_duplicates


def load(path: Path) -> Any:
    if path.is_symlink() or not path.is_file():
        fail(str(path), "input must be a regular non-symbolic file")
    raw = path.read_bytes()
    if len(raw) > MAX_BYTES:
        fail(str(path), "input exceeds the 8 MiB validation bound")
    try:
        return json.loads(raw.decode("utf-8"), object_pairs_hook=object_pairs(path))
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        fail(str(path), f"invalid UTF-8 JSON ({type(error).__name__})")


def resolve_ref(root: dict[str, Any], reference: str, path: str) -> dict[str, Any]:
    if not reference.startswith("#/"):
        fail(path, "only document-local schema references are permitted")
    value: Any = root
    for component in reference[2:].split("/"):
        key = component.replace("~1", "/").replace("~0", "~")
        if not isinstance(value, dict) or key not in value:
            fail(path, f"unresolved schema reference {reference}")
        value = value[key]
    if not isinstance(value, dict):
        fail(path, f"schema reference {reference} is not an object")
    return value


def is_type(value: Any, expected: str) -> bool:
    return {
        "object": isinstance(value, dict),
        "array": isinstance(value, list),
        "string": isinstance(value, str),
        "integer": isinstance(value, int) and not isinstance(value, bool),
        "number": isinstance(value, (int, float)) and not isinstance(value, bool),
        "boolean": isinstance(value, bool),
        "null": value is None,
    }.get(expected, False)


def validate(value: Any, schema: dict[str, Any], root: dict[str, Any], path: str) -> None:
    if "$ref" in schema:
        validate(value, resolve_ref(root, schema["$ref"], path), root, path)
        return
    if "const" in schema and value != schema["const"]:
        fail(path, f"value does not equal required constant {schema['const']!r}")
    if "enum" in schema and value not in schema["enum"]:
        fail(path, "value is outside the allowed enumeration")
    expected = schema.get("type")
    if expected is not None:
        choices = expected if isinstance(expected, list) else [expected]
        if not any(is_type(value, choice) for choice in choices):
            fail(path, f"expected schema type {expected!r}")
    if isinstance(value, dict):
        required = schema.get("required", [])
        missing = [key for key in required if key not in value]
        if missing:
            fail(path, f"missing required field {missing[0]!r}")
        properties = schema.get("properties", {})
        additional = schema.get("additionalProperties", True)
        for key, child in value.items():
            child_schema = properties.get(key)
            if child_schema is None:
                if additional is False:
                    fail(path, f"unknown field {key!r}")
                if isinstance(additional, dict):
                    child_schema = additional
            if child_schema is not None:
                validate(child, child_schema, root, f"{path}.{key}")
    if isinstance(value, list):
        if len(value) < schema.get("minItems", 0):
            fail(path, "array is shorter than minItems")
        if "maxItems" in schema and len(value) > schema["maxItems"]:
            fail(path, "array is longer than maxItems")
        if schema.get("uniqueItems"):
            encoded = [json.dumps(item, sort_keys=True, separators=(",", ":")) for item in value]
            if len(encoded) != len(set(encoded)):
                fail(path, "array items are not unique")
        item_schema = schema.get("items")
        if isinstance(item_schema, dict):
            for index, child in enumerate(value):
                validate(child, item_schema, root, f"{path}[{index}]")
    if isinstance(value, str):
        if len(value) < schema.get("minLength", 0):
            fail(path, "string is shorter than minLength")
        if "maxLength" in schema and len(value) > schema["maxLength"]:
            fail(path, "string is longer than maxLength")
        if "pattern" in schema and re.fullmatch(schema["pattern"], value) is None:
            fail(path, "string does not match the required pattern")
        if schema.get("format") == "date-time":
            try:
                datetime.fromisoformat(value.replace("Z", "+00:00"))
            except ValueError:
                fail(path, "invalid RFC 3339 date-time")
    if isinstance(value, (int, float)) and not isinstance(value, bool):
        if "minimum" in schema and value < schema["minimum"]:
            fail(path, "number is below minimum")
        if "maximum" in schema and value > schema["maximum"]:
            fail(path, "number is above maximum")
        if "exclusiveMinimum" in schema and value <= schema["exclusiveMinimum"]:
            fail(path, "number is not above exclusiveMinimum")


def check_schema(schema: Any, path: Path) -> dict[str, Any]:
    if not isinstance(schema, dict):
        fail(str(path), "schema root must be an object")
    if schema.get("$schema") != "https://json-schema.org/draft/2020-12/schema":
        fail(str(path), "schema must declare JSON Schema draft 2020-12")
    if schema.get("type") != "object" or schema.get("additionalProperties") is not False:
        fail(str(path), "schema root must be a closed object")
    return schema


def check_model_invariants(value: dict[str, Any], path: str) -> None:
    files = value["files"]
    paths = [entry["path"] for entry in files]
    if paths != sorted(paths) or len(paths) != len(set(paths)):
        fail(path, "model files must be uniquely sorted by path")
    shards = [entry for entry in files if entry["purpose"] == "safetensors-shard"]
    if len(shards) != value["tensor_index"]["shards"]:
        fail(path, "Safetensors shard count does not match tensor index")
    if sum(entry["bytes"] for entry in shards) < value["tensor_index"]["total_size"]:
        fail(path, "shard storage is smaller than declared tensor bytes")
    if value["config_sha256"] != next(
        entry["sha256"] for entry in files if entry["path"] == "config.json"
    ):
        fail(path, "config digest disagrees with the file registry")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--schema-dir", type=Path, default=Path("schemas/qwen"))
    parser.add_argument("--document", nargs=2, action="append", metavar=("SCHEMA", "JSON"))
    args = parser.parse_args()
    try:
        schemas: dict[str, dict[str, Any]] = {}
        paths = sorted(args.schema_dir.glob("*.schema.json"))
        if not paths:
            fail(str(args.schema_dir), "no Qwen schemas found")
        for path in paths:
            schema = check_schema(load(path), path)
            identifier = schema.get("$id")
            if not isinstance(identifier, str) or identifier in schemas:
                fail(str(path), "schema $id is missing or duplicated")
            schemas[identifier] = schema
        for schema_path_text, document_path_text in args.document or []:
            schema_path = Path(schema_path_text)
            document_path = Path(document_path_text)
            schema = check_schema(load(schema_path), schema_path)
            document = load(document_path)
            validate(document, schema, schema, str(document_path))
            if document.get("schema") == "seen-qwen-model-lock-v1":
                check_model_invariants(document, str(document_path))
        print(f"qwen-contracts: valid schemas={len(paths)} documents={len(args.document or [])}")
        return 0
    except (OSError, ContractError, StopIteration) as error:
        print(f"qwen-contracts: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
