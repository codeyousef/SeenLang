#!/usr/bin/env python3
"""Enforce aggregate coverage for explicitly owned Seen source files."""
import argparse
import hashlib
import json
from pathlib import Path
from check_test_instrumentation import ContractError, require_source_coverage

ROOT = Path(__file__).resolve().parents[1]


def evaluate(report, sources, line_threshold, branch_threshold):
    if not sources or len(sources)>4096 or len(sources) != len(set(sources)):
        raise ValueError("coverage scope must contain unique source paths")
    if not 0 <= line_threshold <= 100 or not 0 <= branch_threshold <= 100:
        raise ValueError("coverage thresholds must be percentages")
    for source in sources:
        path = Path(source)
        if path.is_absolute() or ".." in path.parts or not source.endswith(".seen"):
            raise ValueError("coverage scope must use relative Seen source paths")
    value = require_source_coverage(report, tuple(sources))
    files = [item for unit in value["data"] for item in unit["files"]]
    totals = {kind: {"count": 0, "covered": 0} for kind in ("lines", "branches")}
    selected = []
    for source in sources:
        matches = [item for item in files if item["filename"] == source or
                   item["filename"].endswith("/" + source)]
        if len(matches) != 1:
            raise ValueError(f"coverage scope is missing or ambiguous: {source}")
        item = {"path": source}
        with (ROOT / source).open("rb") as stream:
            item["sha256"] = hashlib.file_digest(stream, "sha256").hexdigest()
        for kind in totals:
            counts = matches[0]["summary"].get(kind)
            if not isinstance(counts, dict):
                raise ValueError(f"missing {kind} counts for {source}")
            count, covered = counts.get("count"), counts.get("covered")
            if type(count) is not int or type(covered) is not int or not 0 <= covered <= count:
                raise ValueError(f"invalid {kind} counts for {source}")
            item[kind] = {"count": count, "covered": covered}
            totals[kind]["count"] += count
            totals[kind]["covered"] += covered
        selected.append(item)
    passed = True
    for kind, threshold in (("lines", line_threshold), ("branches", branch_threshold)):
        counts = totals[kind]
        counts["minimum_percent"] = threshold
        if not counts["count"] or counts["covered"] * 100 < counts["count"] * threshold:
            passed = False
    return {"schema": "seen-source-coverage-summary-v1", "sources": selected,
            "totals": totals, "passed": passed}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("report", type=Path)
    parser.add_argument("--source", action="append", required=True)
    parser.add_argument("--min-lines", type=int, default=90)
    parser.add_argument("--min-branches", type=int, default=80)
    args = parser.parse_args()
    try:
        result = evaluate(args.report, args.source, args.min_lines, args.min_branches)
    except (ContractError, OSError, UnicodeError, ValueError, KeyError, TypeError) as error:
        print(f"test.002a.invalid: {error}")
        return 1
    print(json.dumps(result, sort_keys=True, separators=(",", ":")))
    return 0 if result["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
