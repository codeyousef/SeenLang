#!/usr/bin/env python3
"""Fail-closed validation for the versioned native-boundary ledger."""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

PLATFORMS = {"linux-x86_64", "linux-aarch64", "macos", "windows"}
ID = re.compile(r"^[a-z][a-z0-9-]{1,63}$")
SUBSYSTEM = re.compile(r"^[a-z][a-z0-9_.-]{1,127}$")
SYMBOL = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*$")
FIELDS = {"id", "subsystem", "purpose", "abi", "platforms", "symbols", "production"}


def fail(message: str) -> None:
    raise ValueError(message)


def string(value: object, name: str, pattern: re.Pattern[str], maximum: int) -> str:
    if not isinstance(value, str) or not value or len(value) > maximum or not pattern.fullmatch(value):
        fail(f"{name} is invalid")
    return value


def validate(document: object) -> None:
    expected_fields = {"version", "inventory", "boundaries"}
    if not isinstance(document, dict) or set(document) != expected_fields:
        fail("ledger must contain exactly version, inventory, and boundaries")
    if document["version"] != 1:
        fail("unsupported ledger version")
    if document["inventory"] != "native-inventory.json":
        fail("inventory must name native-inventory.json")
    boundaries = document["boundaries"]
    if not isinstance(boundaries, list) or not boundaries:
        fail("boundaries must be a non-empty array")
    ids: set[str] = set()
    for index, boundary in enumerate(boundaries):
        prefix = f"boundaries[{index}]"
        if not isinstance(boundary, dict) or set(boundary) != FIELDS:
            fail(f"{prefix} has missing or unknown fields")
        identifier = string(boundary["id"], f"{prefix}.id", ID, 64)
        if identifier in ids:
            fail(f"duplicate boundary id: {identifier}")
        ids.add(identifier)
        string(boundary["subsystem"], f"{prefix}.subsystem", SUBSYSTEM, 128)
        purpose = boundary["purpose"]
        abi = boundary["abi"]
        if not isinstance(purpose, str) or not purpose or len(purpose) > 512:
            fail(f"{prefix}.purpose is invalid")
        if not isinstance(abi, str) or not abi or len(abi) > 64:
            fail(f"{prefix}.abi is invalid")
        for name, allowed, pattern in (("platforms", PLATFORMS, None), ("symbols", None, SYMBOL)):
            values = boundary[name]
            if not isinstance(values, list) or not values or len(values) != len(set(values)):
                fail(f"{prefix}.{name} must be a non-empty unique array")
            for value in values:
                if not isinstance(value, str) or (allowed is not None and value not in allowed) or (pattern is not None and not pattern.fullmatch(value)):
                    fail(f"{prefix}.{name} contains an invalid value")
        if not isinstance(boundary["production"], bool):
            fail(f"{prefix}.production must be boolean")
    if [item["id"] for item in boundaries] != sorted(ids):
        fail("boundaries must be ordered by id")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("ledger", type=Path)
    args = parser.parse_args()
    try:
        raw = args.ledger.read_bytes()
        if raw.startswith(b"\xef\xbb\xbf"):
            fail("ledger must not contain a UTF-8 BOM")
        validate(json.loads(raw))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError, ValueError) as error:
        print(f"native-boundaries: {error}", file=sys.stderr)
        return 1
    print(f"native-boundaries: valid {args.ledger}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
