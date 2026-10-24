#!/usr/bin/env python3
"""Execute emitted LLVM against the shared fixed Seen packed-number corpus."""
import argparse
import json
from pathlib import Path
import re

from check_gpu_amdgpu_artifacts import prepare_output
from check_gpu_amdgpu_scalar import normalize
from check_gpu_amdgpu_toolchain import invoke, load_profile, verify_installation
from gpu_certification_io import read_bounded, invoke_with_snapshot
from gpu_launch_contract import require, digest
from gpu_numeric_cases import read_numeric_cases


def cases_from_log(path):
    records = read_numeric_cases(path, 'seen-amdgpu-numeric-case-v1')
    for record in records:
        text = ''.join(record[key] for key in ('allocations', 'initializers', 'body'))
        require(len(text) <= 1048576 and not re.search(r'\b(getelementptr|addrspacecast|define|declare|ret|br|switch|unreachable)\b', text),
                'unexpected scalar-plan control or pointer instruction')
    return records


def program(records):
    declarations = 'target triple = "x86_64-unknown-linux-gnu"\ndeclare float @llvm.roundeven.f32(float)\n'
    functions, checks = [], []
    for index, record in enumerate(records):
        arguments = ', '.join('i32 %input' + str(i) for i in range(4))
        functions.append(f'define {{ i32, i32, i32 }} @numeric_case_{index}({arguments}) noinline {{\nentry:\n' +
                         record['allocations'] + record['initializers'] + record['body'] +
                         f"%numeric_result0 = insertvalue {{ i32, i32, i32 }} undef, i32 %v{record['value_id']}, 0\n" +
                         f"%numeric_result1 = insertvalue {{ i32, i32, i32 }} %numeric_result0, i32 %v{record['lane_id']}, 1\n" +
                         f"%numeric_result2 = insertvalue {{ i32, i32, i32 }} %numeric_result1, i32 %v{record['scale_id']}, 2\n" +
                         'ret { i32, i32, i32 } %numeric_result2\n}\n')
        actual = ', '.join('i32 ' + str(value) for value in record['inputs'])
        checks.append(f'%case_{index} = call {{ i32, i32, i32 }} @numeric_case_{index}({actual})')
        for field, expected in enumerate(record['expected']):
            checks.extend((f'%word_{index}_{field} = extractvalue {{ i32, i32, i32 }} %case_{index}, {field}',
                           f'%bad_{index}_{field} = icmp ne i32 %word_{index}_{field}, {expected}'))
        checks.extend((f'%bad_value_{index} = or i1 %bad_{index}_0, %bad_{index}_1',
                       f'%bad_case_{index} = or i1 %bad_value_{index}, %bad_{index}_2',
                       f'br i1 %bad_case_{index}, label %fail_{index}, label %next_{index}',
                       f'fail_{index}:', f'ret i32 {index + 1}', f'next_{index}:'))
    return normalize(declarations + '\n'.join(functions)) + '\ndefine i32 @main() {\nentry:\n' + '\n'.join(checks) + '\nret i32 0\n}\n'


def main():
    parser = argparse.ArgumentParser(description=__doc__); parser.add_argument('log', type=Path)
    args = parser.parse_args(); profile = load_profile(); identity = verify_installation(profile)
    records = cases_from_log(args.log); raw = program(records).encode('ascii')
    source = args.log.with_suffix('.numeric-host.ll'); optimized = args.log.with_suffix('.numeric-host.opt.ll')
    prepare_output(source); prepare_output(optimized); source.write_bytes(raw)
    status, output, diagnostic = invoke_with_snapshot(source, 33554432,
        lambda: invoke(profile, 'opt', ['-passes=verify,default<O2>,verify', '-S', str(source), '-o', str(optimized)]), raw)
    require(status == 0 and not output and not diagnostic, 'numeric LLVM compilation failed: ' + diagnostic)
    optimized_bytes = read_bounded(optimized, 33554432)
    status, output, diagnostic = invoke_with_snapshot(optimized, 33554432,
        lambda: invoke(profile, 'lli', ['--jit-kind=orc', str(optimized)]), optimized_bytes)
    failure = records[status - 1] if 1 <= status <= len(records) else None
    if failure is not None: failure = {key: failure[key] for key in ('inputs', 'expected')}
    require(status == 0 and not output and not diagnostic, 'emitted numeric case differs: ' + str(failure) + diagnostic)
    evidence = {'schema': 'seen-amdgpu-packed-numeric-v1', 'execution_backend': 'host-normalized-llvm',
                'hardware_verified': False, 'gpu_execution_verified': False, 'toolchain': identity,
                'cases': len(records), 'word_checks': len(records) * 3, 'program_sha256': digest(raw)}
    args.log.with_suffix('.numeric-evidence.json').write_text(json.dumps(evidence, sort_keys=True) + '\n')
    print('PASS: emitted AMDGPU LLVM matches all 140 shared IEEE packed numeric goldens and 420 result/validity words')


if __name__ == '__main__': main()
