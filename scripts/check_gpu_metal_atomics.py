#!/usr/bin/env python3
"""Execute emitted atomic control/bounds with explicit CPU pointer normalization.

This establishes neither Apple SDK compatibility nor GPU ordering or rendezvous.
"""
import argparse
import json
from pathlib import Path

from check_gpu_metal_artifacts import ATOMIC_POINTER_CAST
from check_gpu_metal_scalar import run
from gpu_metal_host import load_profile
from gpu_launch_contract import require


def normalize_atomic_pointers(raw):
    text = raw.decode('ascii')
    def pointer(match):
        original = match.group(0)
        return original.replace('atomic_' + match['kind'], match['kind'], 1)
    return ATOMIC_POINTER_CAST.sub(pointer, text).encode('ascii')


def atomic_cases(harness, ordered=False):
    require(set(harness.entries) == {'signedAtomics', 'unsignedAtomics', 'sharedAtomics'}, 'unexpected Metal atomic fixture')
    sentinel = 0x7f51a993
    harness.array('signed_data', [19]); harness.array('signed_output', [sentinel] * 14)
    harness.call('signedAtomics', [('signed_data', 1), ('signed_output', 14)])
    harness.check_array('signed_output', [-7, -7, 5, 8, 6, -11, -3, 13, 29, 30, 42, 42, 0, 0])
    harness.check_array('signed_data', [42]); harness.fault(1)
    harness.array('unsigned_data', [19]); harness.array('unsigned_output', [sentinel] * 3)
    harness.call('unsignedAtomics', [('unsigned_data', 1), ('unsigned_output', 3)])
    harness.check_array('unsigned_output', [0xffffffff, 0x80000000, 0x80000000])
    harness.check_array('unsigned_data', [0x80000000]); harness.fault(1)
    harness.array('shared_output', [sentinel] * 4)
    harness.call('sharedAtomics', [('shared_output', 4)])
    harness.check_array('shared_output', [9, 13, -5, -5]); harness.fault(1)
    harness.code.append('seen_cas_spurious_remaining = 1u; seen_cas_calls = 0u;')
    harness.reset_array('signed_output', sentinel)
    harness.call('signedAtomics', [('signed_data', 1), ('signed_output', 14)])
    harness.check_array('signed_output', [-7, -7, 5, 8, 6, -11, -3, 13, 29, 30, 42, 42, 0, 0])
    harness.check('seen_cas_spurious_remaining', 0, 'injected spurious failure consumed')
    harness.check('seen_cas_calls', 3, 'strong CAS retries spurious failure before true mismatch')
    harness.fault(1)
    events = []
    if ordered:
        signed = [(1, order, 4) for order in (3, 2, 3, 2, 2, 3, 2, 3, 2, 3, 2, 2, 2)]
        events = signed + [(1, 3, 4), (1, 2, 4)] + [(2, order, 3) for order in (3, 3, 2, 3, 2, 2)] + signed
    harness.check('seen_fence_count', len(events), 'exact native fence count')
    for index, (spaces, order, scope) in enumerate(events):
        for field, expected in (('spaces', spaces), ('order', order), ('scope', scope)):
            harness.check(f'seen_fence_events[{index}].{field}', expected, f'fence {index} {field}')


def edge_cases(harness):
    require(set(harness.entries) == {'wrappingAtomics', 'compareOnly', 'negativeOnly', 'readonlyAtomic'}, 'unexpected Metal atomic edge fixture')
    sentinel = 0x7f51a993
    harness.array('data', [19]); harness.array('output', [sentinel] * 4)
    harness.call('wrappingAtomics', [('data', 1), ('output', 4)])
    harness.check_array('output', [0x7fffffff, 0x80000000, 0x80000000, 0x7fffffff])
    harness.check_array('data', [0x7fffffff]); harness.fault(0)
    for initial, expected in ((13, [13, 42]), (7, [7, 7])):
        harness.code.append(f'test_data[0] = {initial}u;')
        harness.call('compareOnly', [('data', 1), ('output', 2)])
        for index, word in enumerate(expected): harness.check(f'test_output[{index}]', word, f'CAS old/observed {index}')
        harness.check_array('data', [expected[1]]); harness.fault(0)
    harness.code.append('test_data[0] = 13u; seen_cas_spurious_remaining = 1000001u; seen_cas_calls = 0u;')
    harness.call('compareOnly', [('data', 1), ('output', 2)])
    harness.check('test_output[0]', 0, 'CAS budget result'); harness.check('test_output[1]', 13, 'CAS budget observed storage')
    harness.check_array('data', [13]); harness.fault(4)
    harness.check('seen_cas_calls', 1000000, 'CAS retry ceiling')
    harness.check('seen_cas_spurious_remaining', 1, 'CAS budget stopped native calls')
    harness.code.append('seen_cas_spurious_remaining = 0u;')
    harness.call('negativeOnly', [('data', 1), ('output', 2)], count_overrides={0: 0xffffffff})
    harness.check('test_output[0]', 0, 'forged-count negative atomic result')
    harness.check('test_output[1]', 13, 'forged-count negative atomic preserved storage')
    harness.check_array('data', [13]); harness.fault(1)
    harness.call('readonlyAtomic', [('data', 1), ('output', 2)])
    harness.check('test_output[0]', 13, 'read-only atomic word'); harness.check('test_output[1]', 0, 'read-only invalid atomic word')
    harness.check_array('data', [13]); harness.fault(1)
    harness.check('seen_fence_count', 0, 'relaxed edges add no fences')


def fence_cases(harness):
    require(set(harness.entries) == {'synchronization'}, 'unexpected Metal fence fixture')
    harness.array('output', [0]); harness.call('synchronization', [('output', 1)])
    harness.check_array('output', [23]); harness.fault(0)
    events = [(1, 2, 1), (2, 3, 2), (3, 4, 3), (1, 5, 4), (3, 5, 3), (3, 5, 3)]
    harness.check('seen_fence_count', len(events), 'standalone and execution fence count')
    for index, (spaces, order, scope) in enumerate(events):
        for field, expected in (('spaces', spaces), ('order', order), ('scope', scope)):
            harness.check(f'seen_fence_events[{index}].{field}', expected, f'fence {index} {field}')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__); parser.add_argument('source', type=Path)
    modes = parser.add_mutually_exclusive_group()
    modes.add_argument('--ordered', action='store_true'); modes.add_argument('--edges', action='store_true')
    modes.add_argument('--fences', action='store_true')
    args = parser.parse_args()
    cases = fence_cases if args.fences else edge_cases if args.edges else lambda harness: atomic_cases(harness, args.ordered)
    record = run(args.source, cases, load_profile(), normalize=normalize_atomic_pointers,
                 preamble=b'#include "metal_atomic_test.hpp"\n')
    print(json.dumps({'schema': 'seen-metal-atomic-cpu-witness-v1', 'record': record,
                      'normalization': 'atomic-pointers-to-corresponding-cpu-word-pointers',
                      'official_validation': 'not-run', 'hardware_verified': False,
                      'gpu_execution_verified': False}, sort_keys=True))
