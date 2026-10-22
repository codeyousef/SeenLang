#!/usr/bin/env python3
"""Execute emitted MSL's fixed scalar bodies with explicitly labelled CPU stubs.

This is an arithmetic/control/bounds witness, never Apple SDK validation,
threadgroup race verification, native buffer binding or device execution.
"""
import argparse
import json
from pathlib import Path

from gpu_scalar_corpus import SENTINEL, portable_scalar_cases, splat_cases, geometry_cases
from check_gpu_metal_artifacts import check_artifact
from gpu_metal_host import load_profile, compile_source, run_executable
from gpu_certification_io import read_bounded, prepare_output
from gpu_launch_contract import require, digest


class Harness:
    def __init__(self, reflection):
        self.entries = {entry['name']: entry for entry in reflection['entries']}
        self.code, self.checks, self.lengths = [], [], {}
        self.shape = self.counts = (1, 1, 1)
        self.local = self.group = (0, 0, 0)

    def array(self, name, words):
        require(name not in self.lengths and 1 <= len(words) <= 4096, 'invalid test array')
        self.lengths[name] = len(words)
        self.code.append(f'alignas(16) uint test_{name}[] = {{' + ','.join(str(w & 0xffffffff) + 'u' for w in words) + '};')

    def geometry(self, shape=(1, 1, 1), counts=(1, 1, 1), local=(0, 0, 0), group=(0, 0, 0), override=None):
        require(override is None, 'HSA-only implicit arguments cannot enter a Metal test')
        self.shape, self.counts, self.local, self.group = shape, counts, local, group

    def call(self, name, arguments, *, count_overrides=None):
        entry = self.entries[name]
        require(len(arguments) == len(entry['bindings']), 'Metal test binding count differs')
        actual, counts = [], []
        for binding, argument in zip(entry['bindings'], arguments):
            if binding['kind'] == 'uniform':
                require(len(argument) == 4, 'Metal test uniform requires four words')
                actual.append('uint4(' + ','.join(str(v & 0xffffffff) + 'u' for v in argument) + ')'); counts.append(0)
            else:
                buffer, count = argument
                require(0 <= count * binding['element_stride'] <= self.lengths[buffer] * 4, 'test binding exceeds storage')
                actual.append('test_' + buffer); counts.append(count)
        # Adverse witnesses may deliberately forge count metadata to establish
        # that a negative signed index cannot form a pointer under any count.
        for index, count in (count_overrides or {}).items():
            require(0 <= index < len(counts) and 0 <= count <= 0xffffffff, 'invalid adverse count override')
            counts[index] = count
        self.code.append('{ test_fault.store(0u); const uint4 test_counts[] = {' +
                         ','.join(f'uint4({count}u,0u,0u,0u)' for count in (counts or [0])) + '};')
        actual.extend(('&test_fault', 'test_counts'))
        global_id = tuple((group * size + local) & 0xffffffff for group, size, local in zip(self.group, self.shape, self.local))
        for coordinates in (global_id, self.local, self.group, self.counts, self.shape):
            actual.append('uint3(' + ','.join(str(value) + 'u' for value in coordinates) + ')')
        self.code.append('seen_metal_entry_' + name + '(' + ','.join(actual) + '); }')

    def check(self, expression, expected, label):
        self.checks.append({'label': label, 'expected': expected & 0xffffffff}); number = len(self.checks)
        require(number < 250, 'Metal test exit-code ceiling')
        self.code.append(f'if ({expression} != {expected & 0xffffffff}u) return {number};')

    def check_array(self, name, expected):
        for index, value in enumerate(expected): self.check(f'test_{name}[{index}]', value, f'{name}[{index}]')

    def fault(self, expected): self.check('test_fault.load()', expected, 'fault mask')

    def reset_array(self, name, value=SENTINEL):
        for index in range(self.lengths[name]): self.code.append(f'test_{name}[{index}] = {value}u;')

    def finish(self):
        return '\nint main() { atomic_uint test_fault{0u};\n' + '\n'.join(self.code) + '\nreturn 0; }\n'


def scalar_cases(harness):
    portable_scalar_cases(harness)
    for shape, counts in (((2, 1, 1), (1, 1, 1)), ((1, 1, 1), (0, 1, 1))):
        harness.reset_array('short_output'); harness.geometry(shape, counts)
        harness.call('short_circuit', [('integers', 1), ('short_output', 2)])
        harness.check_array('short_output', [SENTINEL] * 2); harness.fault(16)


def semantic_edges(harness):
    require(set(harness.entries) == {'repeated_helper', 'signed_index', 'signed_store', 'threadgroup', 'boolean_uniform'}, 'unexpected Metal edge fixture')
    harness.array('repeat_output', [SENTINEL] * 2); harness.geometry()
    harness.call('repeated_helper', [('repeat_output', 2)])
    harness.check_array('repeat_output', [77, 77]); harness.fault(0)
    harness.array('boolean_output', [SENTINEL] * 2)
    for value in (0, 1, 2, 0xffffffff):
        harness.reset_array('boolean_output')
        harness.call('boolean_uniform', [('boolean_output', 2), [value, 0, 0, 0]])
        harness.check_array('boolean_output', [1, SENTINEL] if value else [SENTINEL, 1]); harness.fault(0)
    harness.array('negative_input', [19]); harness.array('negative_output', [SENTINEL] * 2)
    harness.call('signed_index', [('negative_input', 1), ('negative_output', 2)], count_overrides={0: 0xffffffff})
    harness.check_array('negative_output', [0, 0]); harness.check_array('negative_input', [19]); harness.fault(1)
    harness.array('negative_store', [SENTINEL])
    harness.call('signed_store', [('negative_store', 1)], count_overrides={0: 0xffffffff})
    harness.check_array('negative_store', [23]); harness.fault(1)
    harness.call('threadgroup', [('negative_store', 1)]); harness.check_array('negative_store', [37]); harness.fault(0)


def run(source, cases, profile, *, normalize=None, preamble=b''):
    raw = read_bounded(source, 33554432)
    reflection = json.loads(read_bounded(source.with_suffix('.json'), 4194304))
    manifest = json.loads(read_bounded(source.with_suffix('.manifest.json'), 1048576))
    check_artifact({'text': raw.decode('ascii'), 'target': reflection['target']}, reflection, manifest)
    harness = Harness(reflection); cases(harness)
    body = raw if normalize is None else normalize(raw)
    program = preamble + body + harness.finish().encode('ascii')
    host = source.with_suffix('.host.cpp'); executable = source.with_suffix('.host')
    prepare_output(host); prepare_output(executable); host.write_bytes(program)
    headers = Path(__file__).resolve().parent.parent / 'tests/metal_msl_metallib_lowering/host'
    status, output, diagnostic = compile_source(profile, host, executable, headers, program)
    require(status == 0 and not output and not diagnostic, 'Metal CPU witness compilation failed: ' + diagnostic)
    status, output, diagnostic = run_executable(executable)
    failure = harness.checks[status - 1] if not diagnostic and 1 <= status <= len(harness.checks) else None
    require(status == 0 and not output and not diagnostic, 'Metal CPU semantic witness failed: ' + str(failure) + diagnostic)
    return {'input_sha256': digest(raw), 'witness_sha256': digest(program), 'checks': len(harness.checks)}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('scalar', type=Path); parser.add_argument('geometry', type=Path); parser.add_argument('splat', type=Path)
    parser.add_argument('--edges', type=Path)
    args = parser.parse_args(); profile = load_profile()
    records = [run(source, cases, profile) for source, cases in
               ((args.scalar, scalar_cases), (args.geometry, geometry_cases), (args.splat, splat_cases))]
    if args.edges is not None: records.append(run(args.edges, semantic_edges, profile))
    print(json.dumps({'schema': 'seen-metal-cpu-witness-v1', 'records': records, 'official_validation': 'not-run',
                      'gpu_execution_verified': False}, sort_keys=True))
