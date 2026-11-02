#!/usr/bin/env python3
"""Reject CPU-validated Metal input replacement before any SDK invocation."""
import hashlib
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts'))
import certify_gpu_metal as certification


class CpuArtifactBinding(unittest.TestCase):
    def setUp(self):
        artifact_root = ROOT / '.seen/tmp'
        artifact_root.mkdir(parents=True, exist_ok=True)
        temporary = tempfile.TemporaryDirectory(prefix='seen-metal-binding-', dir=artifact_root)
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.source = self.root / 'original-macos13.metal'
        self.reflection = self.source.with_suffix('.json')
        source = b'kernel void seen_metal_entry_probe() {}\n'
        reflection = json.dumps({'entries': [{'name': 'probe'}]}).encode('ascii')
        self.source.write_bytes(source)
        self.reflection.write_bytes(reflection)
        self.evidence = {'cpu': {'artifacts': [{
            'target': 'macos13',
            'sha256': hashlib.sha256(source).hexdigest(),
            'reflection_file_sha256': hashlib.sha256(reflection).hexdigest(),
        }]}}
        # No SDK is provisioned or simulated. The low-level boundary receives
        # its caller's admitted profile; reaching a producer is the failure.
        self.profile = {'sdk': {'path': str(self.root)}}

    def reject_before_sdk(self):
        with patch.object(certification, 'native_call',
                          side_effect=AssertionError('replaced CPU input reached an SDK producer')) as native:
            with self.assertRaises(ValueError):
                certification.certify_official(self.root, self.profile, self.evidence)
            native.assert_not_called()

    def test_source_replacement_after_cpu_validation_is_rejected(self):
        self.source.write_bytes(b'kernel void seen_metal_entry_replacement() {}\n')
        self.reject_before_sdk()

    def test_reflection_replacement_cannot_erase_kernel_checks(self):
        self.reflection.write_text('{"entries": []}', encoding='ascii')
        self.reject_before_sdk()


if __name__ == '__main__':
    unittest.main()
