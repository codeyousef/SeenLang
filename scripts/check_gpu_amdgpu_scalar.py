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

from gpu_scalar_corpus import SENTINEL, portable_scalar_cases, splat_cases, geometry_cases
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
    portable_scalar_cases(harness)
    for override in ((0, 32, 0), (12, 16, 2), (18, 16, 1), (44, 32, 1), (64, 16, 0), (64, 16, 4)):
        harness.reset_array('short_output'); harness.geometry(override=override)
        harness.call('short_circuit', [('integers', 1), ('short_output', 2)])
        harness.check_array('short_output', [SENTINEL] * 2); harness.fault(16)







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
