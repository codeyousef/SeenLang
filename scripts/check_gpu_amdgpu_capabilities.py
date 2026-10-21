#!/usr/bin/env python3
"""Independent CPU witnesses for fixed emitted AMDGPU capability fixtures."""
import argparse
import json
from pathlib import Path

from check_gpu_amdgpu_scalar import run, SENTINEL
from check_gpu_amdgpu_toolchain import load_profile, verify_installation
from gpu_launch_contract import require


def atomic_cases(harness):
    require(set(harness.entries) == {'signedAtomics', 'unsignedAtomics', 'sharedAtomics'}, 'unexpected atomic fixture')
    harness.array('signed_data', [SENTINEL] * 4); harness.array('signed_output', [SENTINEL] * 16)
    harness.geometry(); harness.call('signedAtomics', [('signed_data', 1), ('signed_output', 14)])
    harness.check_array('signed_output', [-7, -7, 5, 8, 6, -11, -3, 13, 29, 30, 42, 42, 0, 0, SENTINEL, SENTINEL])
    harness.check_array('signed_data', [42, SENTINEL, SENTINEL, SENTINEL]); harness.fault(1)
    harness.array('unsigned_data', [SENTINEL] * 2); harness.array('unsigned_output', [SENTINEL] * 4)
    harness.geometry(); harness.call('unsignedAtomics', [('unsigned_data', 1), ('unsigned_output', 3)])
    harness.check_array('unsigned_output', [0xffffffff, 0x80000000, 0x80000000, SENTINEL])
    harness.check_array('unsigned_data', [0x80000000, SENTINEL]); harness.fault(1)
    harness.array('shared_output', [SENTINEL] * 6); harness.geometry()
    harness.call('sharedAtomics', [('shared_output', 4)])
    harness.check_array('shared_output', [9, 13, -5, -5, SENTINEL, SENTINEL]); harness.fault(1)


def main():
    parser = argparse.ArgumentParser(description=__doc__); parser.add_argument('atomic', type=Path)
    args = parser.parse_args(); profile = load_profile(); identity = verify_installation(profile)
    records = [run(args.atomic, atomic_cases, profile)]
    evidence = {'schema': 'seen-amdgpu-capability-host-v1', 'execution_backend': 'host-normalized-llvm',
                'hardware_verified': False, 'gpu_execution_verified': False, 'toolchain': identity, 'fixtures': records}
    args.atomic.with_suffix('.capability-host.json').write_text(json.dumps(evidence, sort_keys=True) + '\n')
    print('PASS: host-normalized AMDGPU scoped atomic old values signedness compare exchange and checked bounds match independent goldens')


if __name__ == '__main__': main()
