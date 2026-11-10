#!/usr/bin/env python3
"""Reject ABI corruptions in metadata produced by the actual pinned HSA tools."""
import copy
import json
from pathlib import Path
import re
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'scripts'))
from check_gpu_amdgpu_artifacts import check_metadata
from check_gpu_amdgpu_toolchain import load_profile
from gpu_certification_io import read_bounded


def metadata_note(document):
    return next(note for section in document[0]['NoteSections']
                for note in section['NoteSection']['Notes'] if 'AMDGPU Metadata' in note)


def main(root):
    profile = load_profile(); rejected = 0
    # Include both wave sizes, empty entries, a three-dimensional workgroup and
    # multiple padded shared declarations. These baselines come from LLVM itself.
    mutations = [
        (r'^(amdhsa\.target:\s+\S+)$', r'\1invalid'),
        (r'^(    \.name:\s+)coordinates$', r'\1unexpected'),
        (r'^(        \.offset:\s+)0$', r'\g<1>4'),
        (r'^(        \.size:\s+)8$', r'\g<1>4'),
        (r'^(        \.value_kind:\s+)global_buffer$', r'\1by_value'),
        (r'^(      - \.address_space:\s+)global$', r'\1private'),
        (r'^(    \.kernarg_segment_size:\s+)\d+$', r'\g<1>0'),
        (r'^(    \.kernarg_segment_align:\s+)\d+$', r'\g<1>32'),
        (r'^(    \.wavefront_size:\s+)\d+$', r'\g<1>16'),
        (r'^(    \.max_flat_workgroup_size:\s+)12$', r'\g<1>1'),
        (r'^(    \.group_segment_fixed_size:\s+)\d+$', r'\g<1>65537'),
        (r'^(    \.private_segment_fixed_size:\s+)\d+$', r'\g<1>65537'),
        (r'^(      - )3$', r'\g<1>2'),
        (r'^(        \.value_kind:\s+)hidden_block_count_x$', r'\1hidden_unrecognized'),
        (r'^(    \.wavefront_size:\s+\d+)$', r'\1\n\1'),
        (r'^(        \.offset:\s+\d+)$', r'\1\n\1'),
    ]
    for target in profile['targets']:
        prefix = root / ('edges-fast-' + target)
        document = json.loads(read_bounded(prefix.with_suffix('.metadata.json'), 8 * 1024**2))
        reflection = json.loads(read_bounded(prefix.with_suffix('.json'), 4 * 1024**2))
        original = copy.deepcopy(document); check_metadata(document, reflection, profile)
        for pattern, replacement in mutations:
            corrupted = copy.deepcopy(document); note = metadata_note(corrupted)
            text, count = re.subn(pattern, replacement, note['AMDGPU Metadata'], count=1, flags=re.M)
            assert count == 1 and text != note['AMDGPU Metadata'], (target, pattern)
            note['AMDGPU Metadata'] = text
            try: check_metadata(corrupted, reflection, profile)
            except ValueError: rejected += 1
            else: raise AssertionError('accepted corrupted metadata: ' + pattern)
        assert document == original, 'validator mutated borrowed metadata'
    assert rejected == 80
    (root / 'metadata-adverse.json').write_text(json.dumps({
        'schema': 'seen-amdgpu-metadata-adverse-v1', 'official_baselines': 5,
        'rejected': rejected, 'hardware_verified': False, 'gpu_execution_verified': False,
    }, sort_keys=True) + '\n')
    print('PASS: five official HSA metadata baselines and 80 corrupt ABI geometry identity and duplicate-field rejections')


if __name__ == '__main__': main(Path(sys.argv[1]))
