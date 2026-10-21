#!/usr/bin/env python3
"""Execute fixed AMDGPU scalar fixtures as explicit host-normalized LLVM tests.

This checks emitted control, values and bounds on the CPU. GPU target compilation
and HSA metadata validation are separate; this never claims device execution.
"""
import argparse
import json
from pathlib import Path
import re

from check_gpu_amdgpu_artifacts import check_artifact, prepare_output
from check_gpu_amdgpu_toolchain import invoke, load_profile, verify_installation
from gpu_certification_io import read_bounded, invoke_with_snapshot
from gpu_launch_contract import require, digest

SENTINEL = 0x6d6d6d6d
INTRINSICS = {'llvm.amdgcn.workitem.id.' + axis for axis in 'xyz'} | {'llvm.amdgcn.workgroup.id.' + axis for axis in 'xyz'}
INTRINSICS |= {'llvm.amdgcn.implicitarg.ptr', 'llvm.amdgcn.s.barrier', 'llvm.sqrt.f32', 'llvm.roundeven.f32'}


def normalize(text, extra_intrinsics=()):
    require(not re.search(r'\b(asm|inttoptr|ptrtoint|invoke|callbr|indirectbr|blockaddress|personality)\b', text), 'unexpected test instruction')
    for line in text.splitlines():
        if re.search(r'\bcall\b', line):
            call = re.search(r'@([A-Za-z0-9_.]+)\(', line)
            require(call is not None and call[1] in INTRINSICS | set(extra_intrinsics), 'unexpected test call')
        if line.startswith('@'):
            require(re.fullmatch(r'@seen\.shared\.\d+\.\d+ = internal addrspace\(3\) global \[\d+ x i8\] undef, align 16', line), 'unexpected test global')
    text = text.replace('target triple = "amdgcn-amd-amdhsa"', 'target triple = "x86_64-unknown-linux-gnu"')
    text = text.replace('define amdgpu_kernel void', 'define void').replace(', addrspace(5)', '')
    text = re.sub(r' addrspace\([1345]\)', '', text)
    text = re.sub(r' syncscope\("[a-z]+"\)', '', text)
    text = re.sub(r' #\d+ !reqd_work_group_size !\d+', '', text)
    text = re.sub(r'^attributes #\d+ = .*\n', '', text, flags=re.M)
    stubs = []
    for kind, source in (('local', 'workitem'), ('group', 'workgroup')):
        for axis in 'xyz':
            original = f'llvm.amdgcn.{source}.id.{axis}'; name = f'seen_test_{kind}_{axis}'
            text = text.replace('declare i32 @' + original + '()\n', '').replace('@' + original + '(', '@' + name + '(')
            stubs.append(f'@seen_test_{kind}_value_{axis} = internal global i32 0\n'
                         f'define i32 @{name}() {{\n %value = load i32, ptr @seen_test_{kind}_value_{axis}\n ret i32 %value\n}}\n')
    text = text.replace('declare ptr @llvm.amdgcn.implicitarg.ptr()\n', '').replace('@llvm.amdgcn.implicitarg.ptr(', '@seen_test_implicit_ptr(')
    text = text.replace('declare void @llvm.amdgcn.s.barrier()\n', '').replace('@llvm.amdgcn.s.barrier(', '@seen_test_barrier(')
    stubs.append('@seen_test_implicit = internal global [256 x i8] zeroinitializer, align 16\n'
                 '@seen_test_fault = internal global i32 0, align 4\n'
                 'define ptr @seen_test_implicit_ptr() { ret ptr @seen_test_implicit }\n'
                 'define void @seen_test_barrier() { ret void }\n'
                 'declare void @llvm.memset.p0.i64(ptr, i8, i64, i1 immarg)\n')
    return text + '\n' + '\n'.join(stubs)


class Harness:
    def __init__(self, reflection):
        self.entries = {entry['name']: entry for entry in reflection['entries']}
        self.globals, self.code, self.checks, self.lengths = [], [], [], {}
        self.next_value = 0

    def array(self, name, words):
        require(name not in self.lengths and 1 <= len(words) <= 4096, 'invalid test array')
        self.lengths[name] = len(words)
        data = ', '.join('i32 ' + str(word & 0xffffffff) for word in words)
        self.globals.append(f'@test_{name} = internal global [{len(words)} x i32] [{data}], align 16')

    def pointer(self, name, index=0):
        return f'getelementptr ([{self.lengths[name]} x i32], ptr @test_{name}, i64 0, i64 {index})'

    def geometry(self, shape=(1, 1, 1), counts=(1, 1, 1), local=(0, 0, 0), group=(0, 0, 0), override=None):
        self.code.append('call void @llvm.memset.p0.i64(ptr @seen_test_implicit, i8 0, i64 256, i1 false)')
        values = [(i * 4, 32, value) for i, value in enumerate(counts)]
        values += [(12 + i * 2, 16, value) for i, value in enumerate(shape)] + [(64, 16, 3)]
        for offset, width, value in values + ([] if override is None else [override]):
            self.code.append(f'store i{width} {value}, ptr getelementptr (i8, ptr @seen_test_implicit, i64 {offset}), align {width // 8}')
        for kind, coordinates in (('local', local), ('group', group)):
            for axis, value in zip('xyz', coordinates):
                self.code.append(f'store i32 {value}, ptr @seen_test_{kind}_value_{axis}')

    def call(self, name, arguments):
        entry = self.entries[name]
        require(len(arguments) == len(entry['bindings']), 'test binding count differs')
        actual = []
        for binding, argument in zip(entry['bindings'], arguments):
            if binding['kind'] == 'uniform':
                require(len(argument) == 4, 'test uniform requires four words')
                actual.append('<4 x i32> <' + ', '.join('i32 ' + str(word & 0xffffffff) for word in argument) + '>')
            else:
                buffer, count = argument
                require(0 <= count <= self.lengths[buffer], 'test storage count exceeds its array')
                actual.extend(('ptr @test_' + buffer, 'i32 ' + str(count)))
        actual.append('ptr @seen_test_fault')
        self.code.extend(('store i32 0, ptr @seen_test_fault', f"call void @{name}({', '.join(actual)})"))

    def check_word(self, pointer, expected, label):
        self.next_value += 1; value = self.next_value
        self.checks.append({'label': label, 'expected': expected & 0xffffffff}); check = len(self.checks)
        require(check < 250, 'test check exit-code ceiling')
        self.code.extend((f'%test_value_{value} = load i32, ptr {pointer}, align 4',
                          f'%test_bad_{value} = icmp ne i32 %test_value_{value}, {expected & 0xffffffff}',
                          f'br i1 %test_bad_{value}, label %failed_{value}, label %continue_{value}',
                          f'failed_{value}:', f'ret i32 {check}', f'continue_{value}:'))

    def check_array(self, name, expected):
        for index, value in enumerate(expected): self.check_word(self.pointer(name, index), value, f'{name}[{index}]')

    def fault(self, expected):
        self.check_word('@seen_test_fault', expected, 'fault mask')

    def reset_array(self, name, value=SENTINEL):
        for index in range(self.lengths[name]): self.code.append(f'store i32 {value}, ptr {self.pointer(name, index)}, align 4')

    def finish(self):
        return '\n'.join(self.globals) + '\ndefine i32 @main() {\nentry:\n' + '\n'.join(self.code) + '\nret i32 0\n}\n'


def scalar_cases(harness):
    require(set(harness.entries) == {'bounded', 'budgeted', 'short_circuit', 'casts', 'vectors'}, 'unexpected scalar fixture')
    inputs = [-7, -2147483648, -1, 1, 31, 32, 0, 7, 2147483647]
    harness.array('integers', inputs); harness.array('bounded_output', [SENTINEL] * 16)
    harness.geometry(); harness.call('bounded', [('integers', 9), ('bounded_output', 13)])
    harness.check_array('bounded_output', [7, 0, 0, 0, 0, -1, -2147483648, 19, 7, 18, 0, 0, 9] + [SENTINEL] * 3); harness.fault(3)
    harness.check_array('integers', inputs)
    harness.array('budget_output', [SENTINEL]); harness.geometry(); harness.call('budgeted', [('budget_output', 1)])
    harness.check_array('budget_output', [1000000]); harness.fault(4)
    harness.array('short_output', [SENTINEL] * 2); harness.geometry(); harness.call('short_circuit', [('integers', 1), ('short_output', 2)])
    harness.check_array('short_output', [0, 1]); harness.fault(0)
    floating = [0x80000000, 0x3fc00000, 0xbfc00000, 0x4f000000, 0xcf000000, 0x4f800000, 0x7fc12345, 0x80000001, 1, 0x7f800000]
    harness.array('floating', floating); harness.array('signed', [SENTINEL] * 10); harness.array('unsigned', [SENTINEL] * 10)
    harness.array('rounded', [SENTINEL] * 14); harness.geometry()
    harness.call('casts', [('floating', 10), ('signed', 10), ('unsigned', 10), ('rounded', 14)])
    harness.check_array('signed', [0, 1, -1, 0, -2147483648, 0, 0, 0, 0, 0])
    harness.check_array('unsigned', [0, 1, 0, 2147483648, 0, 0, 0, 0, 0, 0])
    harness.check_array('rounded', [0x4f000000, 0x4f800000, 0xcf000000, 0x80000000, 0x80000000, 0, 0x7fc12345, 0x3f800000,
                                    0x40000000, 0x3f800000, 0, 0, 0, 2]); harness.fault(10)
    harness.array('vector_input', [0x3f800000, 0xc0000000, 0, 0xdeadbeef]); harness.array('vector_output', [SENTINEL] * 12)
    harness.geometry(); harness.call('vectors', [('vector_input', 1), ('vector_output', 3), [0x40000000, 0x40800000, 0x80000000, 0]])
    harness.check_array('vector_output', [0x40400000, 0x40000000, 0, SENTINEL, 0x3f800000, 0xc0000000, 0, SENTINEL,
                                         0x40000000, 0x40800000, 0, SENTINEL]); harness.fault(0)
    for override in ((0, 32, 0), (12, 16, 2), (18, 16, 1), (44, 32, 1), (64, 16, 0), (64, 16, 4)):
        harness.reset_array('short_output'); harness.geometry(override=override)
        harness.call('short_circuit', [('integers', 1), ('short_output', 2)])
        harness.check_array('short_output', [SENTINEL] * 2); harness.fault(16)


def geometry_cases(harness):
    require(set(harness.entries) == {'coordinates', 'seen_shared_0_4', 'empty_entry'}, 'unexpected geometry fixture')
    harness.array('coordinates', [SENTINEL] * 16)
    harness.geometry((3, 2, 2), (2, 3, 4), (1, 1, 1), (1, 2, 3)); harness.call('coordinates', [('coordinates', 16)])
    harness.check_array('coordinates', [4, 5, 7, 1, 1, 1, 1, 2, 3, 2, 3, 4, 3, 2, 2, 10]); harness.fault(0)
    harness.reset_array('coordinates'); harness.geometry((3, 2, 2), (1431655766, 1, 1)); harness.call('coordinates', [('coordinates', 16)])
    harness.check_array('coordinates', [SENTINEL] * 16); harness.fault(16)
    harness.array('shared_input', [19, 23]); harness.array('shared_output', [SENTINEL]); harness.geometry()
    harness.call('seen_shared_0_4', [('shared_input', 2), ('shared_output', 1)]); harness.check_array('shared_output', [42]); harness.fault(0)
    harness.geometry(); harness.call('empty_entry', []); harness.fault(0)


def splat_cases(harness):
    require(set(harness.entries) == {'constantSplat', 'variableSplat'}, 'unexpected splat fixture')
    harness.array('constant_output', [SENTINEL] * 4); harness.geometry()
    harness.call('constantSplat', [('constant_output', 1)])
    harness.check_array('constant_output', [0x40000000] * 3 + [SENTINEL]); harness.fault(0)
    # Compare raw lane bits, including signed zero, subnormal and NaN payloads.
    for index, value in enumerate((0xc0200000, 0x80000000, 1, 0x7fc12345)):
        name = 'splat_input_' + str(index); output = 'splat_output_' + str(index)
        harness.array(name, [value]); harness.array(output, [SENTINEL] * 4); harness.geometry()
        harness.call('variableSplat', [(name, 1), (output, 1)])
        harness.check_array(output, [value] * 3 + [SENTINEL]); harness.fault(0)
        harness.check_array(name, [value])
    harness.array('empty_input', [SENTINEL]); harness.array('empty_output', [SENTINEL] * 4); harness.geometry()
    harness.call('variableSplat', [('empty_input', 0), ('empty_output', 1)])
    harness.check_array('empty_output', [0] * 3 + [SENTINEL]); harness.fault(1)


def run(source, cases, profile, normalizer=normalize, harness_type=Harness, label='host'):
    require(re.fullmatch('[a-z0-9-]{1,64}', label) is not None, 'invalid host evidence label')
    raw = read_bounded(source, 33554432)
    reflection = json.loads(read_bounded(source.with_suffix('.json'), 4194304))
    manifest = json.loads(read_bounded(source.with_suffix('.manifest.json'), 1048576))
    check_artifact({'text': raw.decode('ascii'), 'target': reflection['target']}, reflection, manifest)
    harness = harness_type(reflection); cases(harness)
    program = (normalizer(raw.decode('ascii')) + harness.finish()).encode('ascii')
    host = source.with_suffix('.' + label + '.ll'); optimized = source.with_suffix('.' + label + '.opt.ll')
    prepare_output(host); prepare_output(optimized); host.write_bytes(program)
    status, output, diagnostic = invoke_with_snapshot(host, 33554432,
        lambda: invoke(profile, 'opt', ['-passes=verify,default<O2>,verify', '-S', str(host), '-o', str(optimized)]), program)
    require(status == 0 and not output and not diagnostic, 'host differential compilation failed: ' + diagnostic)
    optimized_bytes = read_bounded(optimized, 33554432)
    status, output, diagnostic = invoke_with_snapshot(optimized, 33554432,
        lambda: invoke(profile, 'lli', ['--jit-kind=orc', str(optimized)]), optimized_bytes)
    failure = harness.checks[status - 1] if 1 <= status <= len(harness.checks) else None
    require(status == 0 and not output and not diagnostic, 'host scalar/geometry witness failed: ' + str(failure) + diagnostic)
    return {'input_sha256': digest(raw), 'normalized_sha256': digest(program), 'checks': len(harness.checks),
            'word_comparisons': getattr(harness, 'word_comparisons', len(harness.checks))}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__); parser.add_argument('scalar', type=Path); parser.add_argument('geometry', type=Path)
    parser.add_argument('splat', type=Path, nargs='?')
    args = parser.parse_args(); profile = load_profile(); identity = verify_installation(profile)
    records = [run(args.scalar, scalar_cases, profile), run(args.geometry, geometry_cases, profile)]
    if args.splat is not None: records.append(run(args.splat, splat_cases, profile))
    evidence = {'schema': 'seen-amdgpu-host-differential-v1', 'execution_backend': 'host-normalized-llvm',
                'hardware_verified': False, 'gpu_execution_verified': False, 'toolchain': identity, 'fixtures': records}
    args.scalar.with_suffix('.host-evidence.json').write_text(json.dumps(evidence, sort_keys=True) + '\n')
    print('PASS: host-normalized AMDGPU LLVM scalar bits bounds control padded vectors and HSA launch guards match independent goldens')
