#!/usr/bin/env python3
"""Finite SDK trust-boundary tests; never impersonate a compiler or build an SDK."""
import os
from pathlib import Path
import sys
import tempfile
import unittest
import tarfile
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts'))
import finite_tsan_sdk as sdk
import finite_tsan_sdk_source as source

# Actual pinned release status fields, not a synthetic compiler success fixture.
STATUS = '''[GNUPG:] NEWSIG
[GNUPG:] KEYEXPIRED 1746208199
[GNUPG:] EXPKEYSIG A2C794A986419D8A Tom Stellard <tstellar@redhat.com>
[GNUPG:] VALIDSIG 474E22316ABF4785A88C6E8EA2C794A986419D8A 2025-04-02 1743619635 0 4 0 1 8 00 474E22316ABF4785A88C6E8EA2C794A986419D8A
'''
CREATED = 1700000000
EXPIRES = 1746208199


class HistoricalSignatureTests(unittest.TestCase):
    def test_valid_signature_from_before_key_expiry_is_not_signature_expiry(self):
        source.accept_signature(STATUS, 0, CREATED, EXPIRES)

    def test_rejects_bad_revoked_expired_and_error_statuses(self):
        for status in ('BADSIG', 'ERRSIG', 'EXPSIG', 'REVKEYSIG', 'KEYREVOKED',
                       'NO_PUBKEY', 'NODATA', 'FAILURE', 'ERROR', 'BADARMOR'):
            with self.subTest(status=status), self.assertRaisesRegex(ValueError, 'bad, revoked, expired'):
                source.accept_signature(STATUS + '[GNUPG:] ' + status + '\n', 0, CREATED, EXPIRES)

    def test_rejects_unsuccessful_verifier_even_with_validsig(self):
        with self.assertRaisesRegex(ValueError, 'unverifiable'):
            source.accept_signature(STATUS, 1, CREATED, EXPIRES)

    def test_rejects_signature_outside_key_validity(self):
        for created, expires in ((1743619636, EXPIRES), (CREATED, 1743619635)):
            with self.subTest(created=created, expires=expires), self.assertRaisesRegex(ValueError, 'during pinned key validity'):
                source.accept_signature(STATUS, 0, created, expires)

    def test_rejects_expiring_signature(self):
        with self.assertRaisesRegex(ValueError, 'during pinned key validity'):
            source.accept_signature(STATUS.replace('1743619635 0 4', '1743619635 1743619636 4'),
                                    0, CREATED, EXPIRES)

    def test_rejects_different_signer(self):
        with self.assertRaisesRegex(ValueError, 'signer differs'):
            source.accept_signature(STATUS.replace(source.SIGNER, '0' * 40), 0, CREATED, EXPIRES)

    def test_rejects_multiple_signatures(self):
        with self.assertRaisesRegex(ValueError, 'signer differs'):
            source.accept_signature(STATUS + STATUS, 0, CREATED, EXPIRES)

    def test_rejects_mismatched_expired_key(self):
        with self.assertRaisesRegex(ValueError, 'historical expired-key status'):
            source.accept_signature(STATUS.replace('KEYEXPIRED 1746208199', 'KEYEXPIRED 1746208198'),
                                    0, CREATED, EXPIRES)


class FileBoundaryTests(unittest.TestCase):
    def setUp(self):
        cache = ROOT / '.seen/finite-tsan-unit-tests'
        cache.mkdir(parents=True, exist_ok=True)
        self.temporary = tempfile.TemporaryDirectory(dir=cache)
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()

    def test_rejects_empty_and_malformed_manifests_before_tool_discovery(self):
        manifest = self.root / 'manifest.json'
        manifest.write_bytes(b'')
        with self.assertRaisesRegex(ValueError, 'empty manifest'):
            sdk.read_manifest(manifest)
        manifest.write_text('{}')
        with self.assertRaisesRegex(ValueError, 'malformed finite TSan manifest'):
            sdk.read_manifest(manifest)
        manifest.write_text('{')
        with self.assertRaises(ValueError):
            sdk.read_manifest(manifest)

    def test_rejects_manifest_replacement_after_wrapper_binding(self):
        manifest = self.root / 'manifest.json'
        manifest.write_text('{}')
        pinned = source.digest(manifest)['sha256']
        manifest.write_text('[]')
        with self.assertRaisesRegex(ValueError, 'after wrapper binding'):
            sdk.read_manifest(manifest, expected_sha256=pinned)

    def test_rejects_symlink_manifest_and_parent(self):
        manifest = self.root / 'manifest.json'
        manifest.write_text('{}')
        link = self.root / 'link'
        link.symlink_to(manifest)
        with self.assertRaisesRegex(ValueError, 'non-symlink'):
            sdk.read_manifest(link)
        directory = self.root / 'directory'
        directory.mkdir()
        parent = self.root / 'parent'
        parent.symlink_to(directory, target_is_directory=True)
        (directory / 'manifest.json').write_text('{}')
        with self.assertRaisesRegex(ValueError, 'non-symlink'):
            sdk.read_manifest(parent / 'manifest.json')

    def test_rejects_resource_symlinks_and_special_files(self):
        tree = self.root / 'resource'
        tree.mkdir()
        target = self.root / 'archive'
        target.write_bytes(b'actual bytes')
        link = tree / 'link'
        link.symlink_to(target)
        with self.assertRaisesRegex(ValueError, 'resource file is a symlink'):
            sdk.inventory(tree)
        link.unlink()
        os.mkfifo(tree / 'fifo')
        with self.assertRaisesRegex(ValueError, 'unsafe or overlarge'):
            sdk.inventory(tree)

    def test_detects_same_size_byte_tampering(self):
        tree = self.root / 'resource'
        tree.mkdir()
        file = tree / 'archive'
        file.write_bytes(b'abcdefgh')
        original = sdk.inventory(tree)
        file.write_bytes(b'abcdEfgh')
        self.assertNotEqual(original, sdk.inventory(tree))

    def test_rejects_overlarge_file_without_reading_it(self):
        file = self.root / 'oversize'
        with file.open('wb') as stream:
            stream.truncate(1025)
        with self.assertRaisesRegex(ValueError, 'unsafe or overlarge'):
            source.digest(file, 1024)

    def test_missing_explicit_selection_does_not_fall_back(self):
        with self.assertRaises(FileNotFoundError):
            sdk.validate(self.root / 'missing-manifest.json')

    def test_rejects_archive_traversal_absolute_paths_and_links(self):
        name = 'compiler-rt-20.1.2.src.tar.xz'
        destination = self.root / 'extracted'
        destination.mkdir()
        for member_name, member_type, message in (
            ('../escape', tarfile.REGTYPE, 'escapes pinned root'),
            ('/escape', tarfile.REGTYPE, 'escapes pinned root'),
            ('compiler-rt-20.1.2.src/link', tarfile.SYMTYPE, 'link or special file'),
        ):
            with self.subTest(member_name=member_name):
                with tarfile.open(self.root / name, 'w:xz') as archive:
                    member = tarfile.TarInfo(member_name)
                    member.type = member_type
                    if member_type == tarfile.SYMTYPE:
                        member.linkname = '../../escape'
                    archive.addfile(member)
                with mock.patch.dict(source.ASSETS, {name: (0, '')}, clear=True):
                    with self.assertRaisesRegex(ValueError, message):
                        source.extract_sources(self.root, destination)
                self.assertEqual(list(destination.iterdir()), [])


if __name__ == '__main__':
    unittest.main()
