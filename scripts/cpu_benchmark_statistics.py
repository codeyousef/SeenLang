#!/usr/bin/env python3
"""Shared deterministic statistics for host-normalized CPU benchmarks."""

from __future__ import annotations

import statistics


def paired_median_ratio_ppm(candidates: list[int], controls: list[int]) -> int:
    """Reduce adjacent candidate/control pairs without combining unrelated medians."""
    if len(candidates) != len(controls) or not candidates:
        raise ValueError("candidate/control sample pairing is invalid")
    ratios: list[int] = []
    for candidate, control in zip(candidates, controls):
        if (
            isinstance(candidate, bool)
            or isinstance(control, bool)
            or not isinstance(candidate, int)
            or not isinstance(control, int)
            or candidate < 1
            or control < 1
        ):
            raise ValueError("candidate/control samples must be positive integers")
        ratios.append(candidate * 1_000_000 // control)
    return int(statistics.median(ratios))
