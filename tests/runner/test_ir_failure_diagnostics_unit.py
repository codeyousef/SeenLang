import tempfile
import unittest
from pathlib import Path
from scripts.report_ir_failure_context import failure_context, MAX_IR_BYTES


class FailureContextTests(unittest.TestCase):
    def test_context_includes_function_and_offending_instruction(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            folder = root / '.seen' / 'run' / 'seen_compile_123_abcd'
            folder.mkdir(parents=True)
            ir = folder / 'seen_module_0.ll'
            ir.write_text('define void @fixture() {\n  %value = fadd float 1.0, 2.0\n  store ptr %value, ptr null\n}\n')
            log = root / 'fixture.log'
            log.write_text(f'opt: {ir}:3:9: error: wrong type\n' * 2)
            result = failure_context(log, root)
            self.assertIn('define void @fixture()', result)
            self.assertIn('3:   store ptr %value', result)
            self.assertEqual(result.count('IR failure context:'), 1)

    def test_external_symlink_and_oversize_paths_are_ignored(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            folder = root / '.seen' / 'seen_compile_123_abcd'
            folder.mkdir(parents=True)
            outside = root / 'seen_compile_123_abcd'
            outside.mkdir()
            external = outside / 'seen_module_0.ll'
            external.write_text('sensitive\n')
            link = folder / 'seen_module_0.ll'
            link.symlink_to(external)
            large = folder / 'seen_module_1.ll'
            with large.open('wb') as stream:
                stream.truncate(MAX_IR_BYTES + 1)
            log = root / 'fixture.log'
            log.write_text(''.join(f'{path}:1:1: error: wrong\n' for path in [external, link, large]))
            self.assertEqual(failure_context(log, root), '')


if __name__ == '__main__':
    unittest.main()
