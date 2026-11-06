#!/usr/bin/env python3
"""Execute actual emitted word operations against the shared 140 fixed cases."""
import argparse
import json
from pathlib import Path
import re

from gpu_numeric_cases import read_numeric_cases
from gpu_certification_io import prepare_output
from gpu_launch_contract import require, digest
from gpu_metal_host import load_profile, compile_source, run_executable


def program(records):
    functions, checks = [], []
    allowed = {'uint', 'int', 'float', 'as_type', 'min', 'rint', 'if', 'else', 'const',
               'seen_offset', 'seen_width', 'seen_mask', 'seen_shift'}
    for index, record in enumerate(records):
        body = ''.join(record[key] for key in ('allocations', 'initializers', 'body'))
        require(body.isascii() and not any(char in body for char in '#"\x00'), 'invalid numeric C++ fragment')
        for word in re.findall(r'\b[A-Za-z_]\w*', body):
            require(word in allowed or re.fullmatch(r'seen_w[1-9][0-9]*|input[0-3]', word), 'unexpected numeric identifier: ' + word)
        outputs = ','.join('seen_w' + str(record[key]) for key in ('value_id', 'lane_id', 'scale_id'))
        functions.append(f'__attribute__((noinline)) uint3 numeric_case_{index}(uint input0,uint input1,uint input2,uint input3) {{\n' +
                         body + f'\nreturn uint3({outputs});\n}}\n')
        inputs = ','.join(str(value) + 'u' for value in record['inputs'])
        checks.append(f'auto case_{index} = numeric_case_{index}({inputs});')
        bad = ' || '.join(f'case_{index}[{field}] != {value}u' for field, value in enumerate(record['expected']))
        checks.append(f'if ({bad}) return {index + 1};')
    return '#include <metal_stdlib>\nusing namespace metal;\n#pragma clang fp contract(off)\n' + \
        '\n'.join(functions) + '\nint main() {\n' + '\n'.join(checks) + '\nreturn 0; }\n'


def run(log):
    records = read_numeric_cases(log, 'seen-metal-numeric-case-v1'); raw = program(records).encode('ascii')
    source = log.with_suffix('.numeric-host.cpp'); executable = log.with_suffix('.numeric-host')
    prepare_output(source); prepare_output(executable); source.write_bytes(raw)
    headers = Path(__file__).resolve().parents[1] / 'tests/metal_msl_metallib_lowering/host'
    profile = load_profile(); status, output, diagnostic = compile_source(profile, source, executable, headers, raw)
    require(status == 0 and not output and not diagnostic, 'Metal numeric CPU compilation failed: ' + diagnostic)
    status, output, diagnostic = run_executable(executable)
    failure = records[status - 1] if not diagnostic and 1 <= status <= len(records) else None
    if failure is not None: failure = {key: failure[key] for key in ('inputs', 'expected')}
    require(status == 0 and not output and not diagnostic, 'Metal packed numeric words differ: ' + str(failure) + diagnostic)
    evidence = {'schema': 'seen-metal-packed-numeric-v1', 'cases': len(records), 'word_checks': len(records) * 3,
                'program_sha256': digest(raw), 'execution_backend': 'diagnostic-cpu',
                'official_validation': 'not-run', 'hardware_verified': False, 'gpu_execution_verified': False}
    log.with_suffix('.numeric-evidence.json').write_text(json.dumps(evidence, sort_keys=True) + '\n')
    print('PASS: emitted Metal operations match all 140 shared IEEE packed goldens and 420 result/validity words')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__); parser.add_argument('log', type=Path)
    run(parser.parse_args().log)
