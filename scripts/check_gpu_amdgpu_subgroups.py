#!/usr/bin/env python3
"""Execute emitted subgroup logic using an explicit fixed-lane permute oracle.

The oracle models one native instruction with fixed input words. The emitted
LLVM still computes membership, validity, votes, ordering and reductions. This
is CPU-normalized execution, never a device or synchronization certificate.
"""
import argparse
import json
from pathlib import Path
import re

from check_gpu_amdgpu_scalar import Harness, SENTINEL, normalize, run
from check_gpu_amdgpu_toolchain import load_profile, verify_installation
from gpu_certification_io import read_bounded
from gpu_launch_contract import require

ORACLE = '''
@seen_test_subgroup_source = internal global [64 x i32] zeroinitializer, align 16
@seen_test_subgroup_lane = internal global i32 0
declare void @llvm.trap()
define i32 @seen_test_shuffle(i32 %offset, i32 %source) {
entry:
 %lane = load i32, ptr @seen_test_subgroup_lane
 %index = lshr i32 %offset, 2
 %low = and i32 %offset, 3
 %aligned = icmp eq i32 %low, 0
 %bounded = icmp ult i32 %index, 64
 %lane_bounded = icmp ult i32 %lane, 64
 %bounds = and i1 %bounded, %lane_bounded
 %valid = and i1 %bounds, %aligned
 br i1 %valid, label %check_source, label %failed
check_source:
 %current = getelementptr [64 x i32], ptr @seen_test_subgroup_source, i32 0, i32 %lane
 %expected = load i32, ptr %current, align 4
 %matches = icmp eq i32 %source, %expected
 br i1 %matches, label %fetch, label %failed
fetch:
 %pointer = getelementptr [64 x i32], ptr @seen_test_subgroup_source, i32 0, i32 %index
 %value = load i32, ptr %pointer, align 4
 ret i32 %value
failed:
 call void @llvm.trap()
 unreachable
}
'''


def normalize_subgroups(text):
    text = normalize(text, {'llvm.amdgcn.ds.bpermute', 'llvm.amdgcn.wave.barrier'})
    text = text.replace('declare i32 @llvm.amdgcn.ds.bpermute(i32, i32)\n', '')
    text = text.replace('declare void @llvm.amdgcn.wave.barrier()\n', '')
    text = text.replace('@llvm.amdgcn.ds.bpermute(', '@seen_test_shuffle(')
    text = text.replace('@llvm.amdgcn.wave.barrier(', '@seen_test_barrier(')
    # Keep runtime inputs dynamic across cases; constant propagation may still
    # optimize each actual emitted function internally.
    text = re.sub(r'^(define void @[^\n]+) \{$', r'\1 noinline {', text, flags=re.M)
    return text + ORACLE


class SubgroupHarness(Harness):
    def __init__(self, reflection):
        super().__init__(reflection)
        self.width = 64 if reflection['target'] in ('gfx90a', 'gfx942') else 32
        self.case = 0

    def oracle(self, lane, words):
        require(0 <= lane < self.width and len(words) == 64, 'invalid fixed lane oracle')
        self.code.append(f'store i32 {lane}, ptr @seen_test_subgroup_lane')
        for index, value in enumerate(words):
            self.code.append(f'store i32 {value & 0xffffffff}, ptr getelementptr ([64 x i32], ptr @seen_test_subgroup_source, i32 0, i32 {index}), align 4')

    def output(self, count):
        self.case += 1; name = 'subgroup_output_' + str(self.case)
        self.array(name, [SENTINEL] * count); return name


def subgroup_cases(harness):
    require(set(harness.entries) == {'groupWords', 'groupVotes', 'groupFloat', 'soleFloat'}, 'unexpected subgroup fixture')
    width = harness.width; indices = [0, 1, 47] if width == 64 else [0, 31, 32, 47]
    for index in indices:
        lane = index % width; count = min(width, 48 - index // width * width)
        words = [i + 1 if i < count else 0xbad10000 + i for i in range(64)]
        for requested in dict.fromkeys((0, count - 1, count, width, 0xffffffff)):
            output = harness.output(242); harness.geometry((3, 2, 8), local=(index % 3, index // 3 % 2, index // 6))
            harness.oracle(lane, words); harness.call('groupWords', [(output, 240), [requested, 0, 0, 0]])
            selected = requested + 1 if requested < count else 0
            expected = [selected, selected, count * (count + 1) // 2, width, index // width]
            for field, value in enumerate(expected): harness.check_word(harness.pointer(output, index * 5 + field), value, 'word subgroup result')
            harness.check_word(harness.pointer(output, index * 5 + 5), SENTINEL, 'following word untouched')
            if index > 0: harness.check_word(harness.pointer(output, index * 5 - 1), SENTINEL, 'preceding word untouched')
            else: harness.check_word(harness.pointer(output, 241), SENTINEL, 'tail word untouched')
            harness.fault(0 if requested < count else 1)
    for index in ([0, 47] if width == 64 else [0, 32, 47]):
        lane = index % width; count = min(width, 48 - index // width * width)
        for threshold in dict.fromkeys((0, 1, count, width + 1)):
            words = [int(i < threshold) if i < count else int(i >= threshold) for i in range(64)]
            output = harness.output(144); harness.geometry((48, 1, 1), local=(index, 0, 0)); harness.oracle(lane, words)
            harness.call('groupVotes', [(output, 144), [threshold, 0, 0, 0]])
            for field, value in enumerate((int(threshold >= count), int(threshold > 0), int(lane == 0))):
                harness.check_word(harness.pointer(output, index * 3 + field), value, 'subgroup vote/election')
            harness.fault(0)
    quarter_sums = {1: 0x3e800000, 16: 0x40800000, 32: 0x41000000, 48: 0x41400000}
    for index in indices:
        lane = index % width; count = min(width, 48 - index // width * width)
        for value in (0x80000000, 0x3e800000):
            output = harness.output(48); harness.geometry((48, 1, 1), local=(index, 0, 0))
            harness.oracle(lane, [value if i < count else 0x7fc54321 for i in range(64)])
            harness.call('groupFloat', [(output, 48), [value, 0, 0, 0]])
            harness.check_word(harness.pointer(output, index), value if value == 0x80000000 else quarter_sums[count], 'ordered floating subgroup reduction')
            harness.fault(0)
    for value in (0x80000000, 0x3e800000):
        output = harness.output(1); harness.geometry(); harness.oracle(0, [value] + [0x7fc54321] * 63)
        harness.call('soleFloat', [(output, 1), [value, 0, 0, 0]])
        harness.check_array(output, [value]); harness.fault(0)


def main():
    parser = argparse.ArgumentParser(description=__doc__); parser.add_argument('wave64', type=Path); parser.add_argument('wave32', type=Path)
    args = parser.parse_args(); profile = load_profile(); identity = verify_installation(profile); records = []
    for source, target in ((args.wave64, 'gfx90a'), (args.wave32, 'gfx1100')):
        reflection = json.loads(read_bounded(source.with_suffix('.json'), 4194304))
        require(reflection['target'] == target, 'subgroup witness target differs')
        records.append(run(source, subgroup_cases, profile, normalize_subgroups, SubgroupHarness))
    evidence = {'schema': 'seen-amdgpu-subgroup-host-v1', 'execution_backend': 'host-normalized-llvm',
                'native_permute': 'fixed-lane-word-oracle', 'hardware_verified': False, 'gpu_execution_verified': False,
                'toolchain': identity, 'fixtures': records}
    args.wave64.with_suffix('.subgroup-host.json').write_text(json.dumps(evidence, sort_keys=True) + '\n')
    print('PASS: emitted wave64/wave32 subgroup membership votes shuffle bounds and ordered reductions match fixed-lane word oracle')


if __name__ == '__main__': main()
