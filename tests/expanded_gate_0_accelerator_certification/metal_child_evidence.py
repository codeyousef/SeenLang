#!/usr/bin/env python3
"""Collect real Metal-C CPU proof separately from its optional Apple SDK result."""
import json
from pathlib import Path
import re
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'scripts'))
from gpu_certification_io import read_bounded, prepare_output, require

CHILD = 'seen_gpu_metal_c'
PROFILES = {'fast', 'release', 'undefined', 'address'}
TARGETS = {'macos13', 'macos14', 'macos15'}


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, 'duplicate Metal receipt key')
        result[key] = value
    return result


def reject_constant(value):
    raise ValueError('nonfinite Metal receipt number: ' + value)


def collect(root):
    raw = read_bounded(root / (CHILD + '.log'), 64 * 1024**2)
    status_bytes = read_bounded(root / (CHILD + '.status'), 16)
    require(re.fullmatch(rb'(0|[1-9][0-9]{0,2})\n', status_bytes), 'invalid Metal child status')
    status = int(status_bytes)
    require(status <= 255, 'Metal child status exceeds process exit range')
    receipts = []
    for line in raw.splitlines():
        if not line.startswith(b'{'):
            continue
        require(len(line) <= 2 * 1024**2, 'Metal receipt exceeds byte ceiling')
        value = json.loads(line, object_pairs_hook=unique_object, parse_constant=reject_constant)
        if isinstance(value, dict) and value.get('schema') == 'seen-metal-certification-v1':
            receipts.append(value)
    require(len(receipts) == 1, 'missing or duplicate Metal certification receipt')
    receipt = receipts[0]
    require(receipt['classification'] == 'compile-only' and
            receipt['hardware_verified'] is False and receipt['gpu_execution_verified'] is False,
            'Metal artifact evidence cannot claim hardware execution')
    cpu = receipt['cpu']
    require(cpu['status'] == 'passed' and cpu['classification'] == 'native-cpu-integrity-only',
            'Metal child did not finish its native CPU integrity checks')
    profiles = cpu['profiles']
    require(len(profiles) == len(PROFILES) and {item['profile'] for item in profiles} == PROFILES and
            all(type(item['compile_status']) is int and item['compile_status'] == 0 and
                type(item['run_status']) is int and item['run_status'] == 0 and
                item['ownership_baseline_verified'] is True and
                re.fullmatch('[0-9a-f]{64}', item['log_sha256']) for item in profiles),
            'incomplete native Metal integrity profiles')
    artifacts = cpu['artifacts']
    require(len(artifacts) == len(TARGETS) and {item['target'] for item in artifacts} == TARGETS and
            all(item['source_relocation_verified'] is True and
                item['four_profile_hash_identity_verified'] is True and
                re.fullmatch('[0-9a-f]{64}', item['sha256']) and
                re.fullmatch('[0-9a-f]{64}', item['reflection_sha256']) and
                item['reflection_file_sha256'] == item['reflection_sha256'] for item in artifacts),
            'incomplete Metal byte/reflection/relocation evidence')
    if status == 126:
        require(receipt['status'] == 'unsupported' and receipt['official_validation'] == 'not-run',
                'unsupported exit did not carry an unexecuted SDK receipt')
    elif status == 0:
        require(receipt['status'] == 'passed' and receipt['official_validation'] == 'passed',
                'successful Metal child omitted official validation')
    else:
        require(receipt['status'] == 'failed', 'failed Metal child carried a success receipt')
    sdk_executed = receipt['official_validation'] in ('passed', 'failed')
    evidence_name = 'metal-official-sdk.json'
    destination = root / evidence_name
    prepare_output(destination)
    destination.write_text(json.dumps({'child_exit_status': status, 'observed': receipt}, sort_keys=True) + '\n')
    checks = root / 'checks.json'
    rows = json.loads(read_bounded(checks, 4 * 1024**2))
    child_name = receipt['artifact_directory']
    require(type(child_name) is str and re.fullmatch(r'gpu-metal-c\.[A-Za-z0-9]{6}', child_name),
            'invalid current-run Metal artifact directory')
    child_root = root.parent / child_name
    require(child_root.is_dir() and not child_root.is_symlink(),
            'Metal receipt does not identify an owned artifact directory')
    require(json.loads(read_bounded(child_root / 'certification.json', 2 * 1024**2),
                       object_pairs_hook=unique_object, parse_constant=reject_constant) == receipt,
            'printed Metal receipt differs from its retained source')
    native_root = root / 'metal-native'
    native_root.mkdir()
    for item in profiles:
        profile = item['profile']
        directory = native_root / profile
        directory.mkdir()
        witnesses = [evidence_name]
        for suffix, limit in (('.log', 1048576), ('.compile.log', 16 * 1024**2),
                              ('.compile.status', 16), ('.run.status', 16)):
            name = 'artifact-' + profile + suffix
            data = read_bounded(child_root / name, limit)
            if suffix == '.log':
                import hashlib
                require(hashlib.sha256(data).hexdigest() == item['log_sha256'],
                        'native Metal log differs from the CPU receipt')
            if suffix.endswith('.status'):
                require(data == b'0\n', 'native Metal status changed after certification')
            destination = directory / name
            destination.write_bytes(data)
            witnesses.append(destination.relative_to(root).as_posix())
        rows.append({'name': 'metal-cpu-integrity-' + profile, 'exit_status': 0, 'required': True,
                     'execution': 'native-cpu', 'classification': 'compile-only', 'logs': witnesses,
                     'details': {'official_validation': 'not-inferred-from-cpu-proof'}})
    logs = [evidence_name, CHILD + '.log', CHILD + '.status']
    rows.append({'name': 'metal-official-sdk', 'exit_status': status, 'required': False,
                 'execution': 'official-validation' if sdk_executed else 'none',
                 'classification': 'compile-only' if sdk_executed else 'unsupported', 'logs': logs,
                 'details': {'state': receipt['status'], 'official_validation': receipt['official_validation'],
                             'production_certified': False}})
    checks.write_text(json.dumps(rows, indent=2) + '\n')


if __name__ == '__main__':
    try:
        collect(Path(sys.argv[1]))
    except (OSError, ValueError, KeyError, TypeError, IndexError, UnicodeError) as error:
        print('metal-child-evidence: ' + str(error), file=sys.stderr)
        raise SystemExit(1)
