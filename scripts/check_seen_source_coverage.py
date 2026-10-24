#!/usr/bin/env python3
"""Verify source counters against an independently specified execution fixture."""
import argparse
import json
from pathlib import Path


def verify(report: Path, source: Path) -> None:
    with report.open("rb") as stream:
        raw = stream.read(64 * 1024 * 1024 + 1)
    if not 0 < len(raw) <= 64 * 1024 * 1024:
        raise ValueError("coverage export exceeds its byte budget")
    data = json.loads(raw)
    if data.get("type") != "llvm.coverage.json.export":
        raise ValueError("expected an LLVM coverage export")
    units = data.get("data", [])
    files = [item for unit in units for item in unit.get("files", [])
             if item.get("filename", "").endswith(source.name)]
    if len(files) != 1:
        raise ValueError("missing or ambiguous Seen source coverage")
    summary = files[0]["summary"]
    if not (0 < summary["lines"]["covered"] < summary["lines"]["count"]):
        raise ValueError("covered and deliberately missed Seen lines must differ")
    functions = [item for unit in units for item in unit.get("functions", [])
                 if any(name.endswith(source.name) for name in item.get("filenames", []))]

    def function(name: str) -> dict:
        matches = [item for item in functions if f":{name}:" in item["name"] or item["name"].startswith(name + ":")]
        if len(matches) != 1:
            raise ValueError(f"missing or ambiguous source function: {name}")
        return matches[0]

    for name, count in (("main", 1), ("coverageChoice", 2), ("coverageNever", 0),
                        ("CoverageVisit_visit", 2), ("CoverageVisit_choose", 2), ("coverageMatch", 3)):
        if function(name)["count"] != count:
            raise ValueError(f"wrong execution count for {name}")
    choice = function("coverageChoice")["branches"]
    if len(choice) != 1 or choice[0][4:6] != [1, 1]:
        raise ValueError("both function branch outcomes must be observed once")
    matched = sorted(branch[4:6] for branch in function("coverageMatch")["branches"])
    if matched != [[1, 1], [1, 2]]:
        raise ValueError("match pattern outcomes must preserve both decisions")
    source_lines = source.read_text().splitlines()

    def line(prefix: str) -> int:
        matches = [index + 1 for index, text in enumerate(source_lines)
                   if text.strip().startswith(prefix)]
        if len(matches) != 1:
            raise ValueError(f"fixture location is ambiguous: {prefix}")
        return matches[0]

    branches = function("main")["branches"]
    expected = (("while index", [[3, 1]]), ("for item", [[3, 1]]),
                ("if no and", [[0, 1], [0, 1]]),
                ("if yes or", [[1, 0], [1, 0]]),
                ("if yes and", [[1, 0], [1, 0]]),
                ("if no or", [[0, 1], [1, 0]]))
    for prefix, counts in expected:
        position = line(prefix)
        actual = sorted(branch[4:6] for branch in branches if branch[0] == position)
        if actual != sorted(counts):
            raise ValueError(f"incorrect branch outcomes at {prefix}: {actual}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("report", type=Path)
    parser.add_argument("source", type=Path)
    args = parser.parse_args()
    try:
        verify(args.report, args.source)
    except (OSError, UnicodeError, ValueError, KeyError, TypeError, RecursionError) as error:
        print(f"test.002a.invalid: {error}")
        return 1
    print("PASS: actual Seen function, missed-line, loop and short-circuit counters")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
