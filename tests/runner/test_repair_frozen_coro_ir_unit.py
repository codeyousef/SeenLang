"""Frozen-bootstrap coroutine ABI compatibility must remain fail closed."""

import importlib.util
from pathlib import Path
import unittest


SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "repair_frozen_coro_ir.py"
SPEC = importlib.util.spec_from_file_location("repair_frozen_coro_ir", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class FrozenCoroutineIrTest(unittest.TestCase):
    def test_new_llvm_rewrites_only_unused_declaration(self):
        source = (
            "declare i1 @llvm.coro.end(ptr, i1, token)\n"
            '@.text = constant [31 x i8] c" = call i1 @llvm.coro.end(ptr "\n'
        )
        actual = MODULE.repair(source, 23)
        self.assertIn("declare void @llvm.coro.end", actual)
        self.assertIn('c" = call i1 @llvm.coro.end', actual)
        self.assertEqual(actual, MODULE.repair(actual, 23))

    def test_old_llvm_keeps_declaration(self):
        source = "declare i1 @llvm.coro.end(ptr, i1, token)\n"
        self.assertEqual(source, MODULE.repair(source, 21))

    def test_live_old_abi_call_is_rejected(self):
        source = (
            "declare i1 @llvm.coro.end(ptr, i1, token)\n"
            "  %7 = call i1 @llvm.coro.end(ptr %1, i1 false, token none)\n"
        )
        with self.assertRaisesRegex(ValueError, "live old-ABI"):
            MODULE.repair(source, 23)


if __name__ == "__main__":
    unittest.main()
