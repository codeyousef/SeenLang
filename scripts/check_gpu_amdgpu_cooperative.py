#!/usr/bin/env python3
"""Check emitted MFMA operand routing, fragment layouts and bounded memory.

CPU execution uses fixed native-instruction operand/result oracles. It checks
the emitted shader's data movement and guards, not matrix multiplication on a
GPU. Layout facts reference AMD's calculator at commit
2ef91896bcdc4d26624f952e5c905c787cd9bc9e; no calculator code is embedded.
"""
import argparse
import json
from pathlib import Path
import re

from check_gpu_amdgpu_scalar import Harness, SENTINEL, normalize, run
from check_gpu_amdgpu_toolchain import load_profile, verify_installation
from gpu_certification_io import read_bounded
from gpu_launch_contract import require

NATIVE = {'llvm.amdgcn.mfma.f32.16x16x16f16': ('half', 'seen_test_mfma_half'),
          'llvm.amdgcn.mfma.f32.16x16x16bf16.1k': ('i16', 'seen_test_mfma_bfloat')}
# lane, two packed A/B word indices, four row-major and column-major C indices.
LAYOUT = [(0, [0, 1], [0, 16, 32, 48], [0, 1, 2, 3]),
          (1, [8, 9], [1, 17, 33, 49], [16, 17, 18, 19]),
          (15, [120, 121], [15, 31, 47, 63], [240, 241, 242, 243]),
          (16, [2, 3], [64, 80, 96, 112], [4, 5, 6, 7]),
          (31, [122, 123], [79, 95, 111, 127], [244, 245, 246, 247]),
          (32, [4, 5], [128, 144, 160, 176], [8, 9, 10, 11]),
          (47, [124, 125], [143, 159, 175, 191], [248, 249, 250, 251]),
          (48, [6, 7], [192, 208, 224, 240], [12, 13, 14, 15]),
          (63, [126, 127], [207, 223, 239, 255], [252, 253, 254, 255])]


def oracle_function(kind, name):
    code = [f'define <4 x float> @{name}(<4 x {kind}> %a, <4 x {kind}> %b, <4 x float> %c, i32 %cbsz, i32 %abid, i32 %blgp) {{',
            f'%a_words = bitcast <4 x {kind}> %a to <2 x i32>',
            f'%b_words = bitcast <4 x {kind}> %b to <2 x i32>', '%c_words = bitcast <4 x float> %c to <4 x i32>']
    for vector, count, base in (('a', 2, 0), ('b', 2, 2), ('c', 4, 4)):
        for index in range(count):
            code.extend((f'%{vector}_{index} = extractelement <{count} x i32> %{vector}_words, i32 {index}',
                         f'call void @seen_test_mfma_check(i32 %{vector}_{index}, i32 {base + index})'))
    for index, name in enumerate(('cbsz', 'abid', 'blgp'), 8):
        code.append(f'call void @seen_test_mfma_check(i32 %{name}, i32 {index})')
    code.extend(('%bits = load <4 x i32>, ptr @seen_test_mfma_result, align 16',
                 '%result = bitcast <4 x i32> %bits to <4 x float>', 'ret <4 x float> %result', '}'))
    return '\n'.join(code) + '\n'


def normalize_matrices(text):
    text = normalize(text, NATIVE)
    for intrinsic, (kind, name) in NATIVE.items():
        text = re.sub(r'^declare <4 x float> @' + re.escape(intrinsic) + r'\([^\n]*\)\n', '', text, flags=re.M)
        text = text.replace('@' + intrinsic + '(', '@' + name + '(')
    text = re.sub(r'^(define void @[^\n]+) \{$', r'\1 noinline {', text, flags=re.M)
    support = '''
@seen_test_mfma_expected = internal global [11 x i32] zeroinitializer, align 16
@seen_test_mfma_result = internal global [4 x i32] zeroinitializer, align 16
declare void @llvm.trap()
define void @seen_test_mfma_check(i32 %actual, i32 %index) {
 %bounded = icmp ult i32 %index, 11
 br i1 %bounded, label %check, label %failed
check:
 %pointer = getelementptr [11 x i32], ptr @seen_test_mfma_expected, i32 0, i32 %index
 %expected = load i32, ptr %pointer, align 4
 %matches = icmp eq i32 %actual, %expected
 br i1 %matches, label %done, label %failed
failed:
 call void @llvm.trap()
 unreachable
done:
 ret void
}
'''
    return text + support + ''.join(oracle_function(kind, name) for kind, name in NATIVE.values())


class MatrixHarness(Harness):
    def __init__(self, reflection):
        super().__init__(reflection); self.case = 0; self.word_comparisons = 0
        require(set(self.entries) == {'mfmaHalf', 'mfmaBfloat', 'mfmaShared', 'fillMatrix'}, 'unexpected cooperative fixture')

    def buffer(self, words):
        self.case += 1; name = 'matrix_buffer_' + str(self.case); self.array(name, words); return name

    def check_word(self, pointer, expected, label):
        self.word_comparisons += 1; super().check_word(pointer, expected, label)

    def full_buffer(self, name, expected):
        require(len(expected) == self.lengths[name] and len(expected) <= 4096, 'invalid expected matrix buffer')
        self.next_value += 1; identity = self.next_value; condition = None
        for index, value in enumerate(expected):
            prefix = f'matrix_{identity}_{index}'
            self.code.extend((f'%{prefix} = load i32, ptr {self.pointer(name, index)}, align 4',
                              f'%{prefix}_bad = icmp ne i32 %{prefix}, {value & 0xffffffff}'))
            current = '%' + prefix + '_bad'
            if condition is not None:
                self.code.append(f'%{prefix}_any = or i1 {condition}, {current}'); current = '%' + prefix + '_any'
            condition = current
        self.word_comparisons += len(expected)
        self.checks.append({'label': name, 'expected_words': len(expected)}); check = len(self.checks)
        require(check < 250, 'matrix fixture assertion ceiling')
        self.code.extend((f'br i1 {condition}, label %matrix_failed_{identity}, label %matrix_next_{identity}',
                          f'matrix_failed_{identity}:', f'ret i32 {check}', f'matrix_next_{identity}:'))

    def oracle(self, left, right, accumulator, result):
        require(len(left) == len(right) == 2 and len(accumulator) == len(result) == 4, 'invalid MFMA operand oracle')
        for name, size, values in (('expected', 11, left + right + accumulator + [0, 0, 0]), ('result', 4, result)):
            for index, value in enumerate(values):
                self.code.append(f'store i32 {value & 0xffffffff}, ptr getelementptr ([{size} x i32], ptr @seen_test_mfma_{name}, i32 0, i32 {index}), align 4')


def filled(size, positions, words):
    result = [SENTINEL] * size
    for position, word in zip(positions, words): result[position] = word
    return result


def fill_cases(harness):
    for lane, _, positions, _ in LAYOUT:
        output = harness.buffer([SENTINEL] * 256); harness.geometry((64, 1, 1), local=(lane, 0, 0))
        harness.call('fillMatrix', [(output, 256), [0x3f800000, 0, 0, 0], [0] * 4, [16, 0, 0, 0]])
        harness.full_buffer(output, filled(256, positions, [0x3f800000] * 4)); harness.fault(0)
    for lane, positions in ((0, [5, 29, 53, 77]), (63, [308, 332, 356, 380])):
        output = harness.buffer([SENTINEL] * 400); harness.geometry((64, 1, 1), local=(lane, 0, 0))
        harness.call('fillMatrix', [(output, 400), [0x80000000, 0, 0, 0], [5, 0, 0, 0], [24, 0, 0, 0]])
        harness.full_buffer(output, filled(400, positions, [0x80000000] * 4)); harness.fault(0)
    for count, offset, stride in ((255, 0, 16), (256, 1, 16), (512, 0, 15), (512, 0, 0),
                                  (512, 0xffffffff, 16), (512, 0, 0xffffffff), (0, 0, 16)):
        output = harness.buffer([SENTINEL] * 512); harness.geometry((64, 1, 1), local=(63, 0, 0))
        harness.call('fillMatrix', [(output, count), [0x3f800000, 0, 0, 0], [offset, 0, 0, 0], [stride, 0, 0, 0]])
        harness.full_buffer(output, [SENTINEL] * 512); harness.fault(1)


def fragment_cases(harness):
    for bfloat in (False, True):
        a_words = [(0x3f803f00 if bfloat else 0x38003c00) + i for i in range(256)]
        b_words = [(0x3f003f80 if bfloat else 0x3c003800) + i for i in range(256)]
        a = harness.buffer(a_words); b = harness.buffer(b_words); kernel = 'mfmaBfloat' if bfloat else 'mfmaHalf'
        for lane, inputs, row, column in LAYOUT:
            result = [(0x40000000 if bfloat else 0x3f800000) + lane * 16 + i for i in range(4)]
            output = harness.buffer([SENTINEL] * 256); harness.geometry((64, 1, 1), local=(lane, 0, 0))
            harness.oracle([a_words[i] for i in inputs], [b_words[i] for i in inputs], [0] * 4, result)
            harness.call(kernel, [(a, 128), (b, 128), (output, 256)] + [[v, 0, 0, 0] for v in (0, 0, 8, 8, 0, 16)])
            harness.full_buffer(output, filled(256, column if bfloat else row, result)); harness.fault(0)
        for lane, a_indices, b_indices, row, column in ((0, [5, 6], [7, 8], [3, 27, 51, 75], [3, 4, 5, 6]),
                                                       (63, [251, 252], [253, 254], [306, 330, 354, 378], [375, 376, 377, 378])):
            result = [0x40800000 + i for i in range(4)]; output = harness.buffer([SENTINEL] * 400)
            harness.geometry((64, 1, 1), local=(lane, 0, 0))
            harness.oracle([a_words[i] for i in a_indices], [b_words[i] for i in b_indices], [0] * 4, result)
            harness.call(kernel, [(a, 256), (b, 256), (output, 400)] + [[v, 0, 0, 0] for v in (5, 7, 16, 16, 3, 24)])
            harness.full_buffer(output, filled(400, column if bfloat else row, result)); harness.fault(0)
        # Invalid complete footprints must zero the entire native input bundle.
        for count, offset, stride in ((127, 0, 8), (128, 1, 8), (128, 0xffffffff, 8), (128, 0, 0xffffffff), (0, 0, 8)):
            output = harness.buffer([SENTINEL] * 256); harness.geometry((64, 1, 1), local=(63, 0, 0))
            harness.oracle([0, 0], b_words[126:128], [0] * 4, [0] * 4)
            harness.call(kernel, [(a, count), (b, 128), (output, 256)] + [[v, 0, 0, 0] for v in (offset, 0, stride, 8, 0, 16)])
            positions = LAYOUT[-1][3 if bfloat else 2]
            harness.full_buffer(output, filled(256, positions, [0] * 4)); harness.fault(1)
        harness.full_buffer(a, a_words); harness.full_buffer(b, b_words)


def main():
    parser = argparse.ArgumentParser(description=__doc__); parser.add_argument('gfx90a', type=Path); parser.add_argument('gfx942', type=Path)
    args = parser.parse_args(); profile = load_profile(); identity = verify_installation(profile); records = []
    for source, target in ((args.gfx90a, 'gfx90a'), (args.gfx942, 'gfx942')):
        require(json.loads(read_bounded(source.with_suffix('.json'), 4194304))['target'] == target, 'matrix witness target differs')
        for label, cases in (('matrix-fill-host', fill_cases), ('matrix-fragment-host', fragment_cases)):
            records.append(run(source, cases, profile, normalize_matrices, MatrixHarness, label))
    args.gfx90a.with_suffix('.matrix-host.json').write_text(json.dumps({
        'schema': 'seen-amdgpu-matrix-host-v1', 'execution_backend': 'host-normalized-llvm',
        'native_mfma': 'fixed-operand-and-result-oracle', 'hardware_verified': False, 'gpu_execution_verified': False,
        'toolchain': identity, 'fixtures': records,
    }, sort_keys=True) + '\n')
    print('PASS: emitted CDNA MFMA operands native fragment layouts full matrix footprints and nonaliasing stores match fixed oracles')


if __name__ == '__main__': main()
