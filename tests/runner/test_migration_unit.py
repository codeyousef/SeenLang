#!/usr/bin/env python3
import contextlib, importlib.util, io, json, sys, tempfile, unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "test_migration", ROOT / "scripts/check_test_migration.py")
M = importlib.util.module_from_spec(SPEC); assert SPEC.loader
SPEC.loader.exec_module(M)
HAPPY = ROOT / "tests/fixtures/test-001f/happy/plan.json"

class OutputCapture:
    def __init__(self): self.buffer=io.BytesIO(); self.text=io.StringIO()
    def write(self,value): return self.text.write(value)
    def flush(self): return None
    def value(self): return self.buffer.getvalue().decode()+self.text.getvalue()

class MigrationTests(unittest.TestCase):
    def test_happy_and_canonical(self):
        raw = HAPPY.read_bytes(); value = M.validate(raw)
        self.assertEqual(M.canonical_bytes(value), raw)
        self.assertEqual(value["total_tests"], 3)
        self.assertTrue(M.canonical_path("tests/fixtures/external_package"))
    def test_invalid_matrix(self):
        base = json.loads(HAPPY.read_bytes())
        values = (None, {**base, "schema":"bad"}, {**base, "entries":[]},
                  {**base, "total_tests":4})
        for value in values:
            with self.subTest(value=value), self.assertRaises(M.MigrationError):
                M.validate(json.dumps(value).encode())
        for raw in (b"", b'{"schema":1,"schema":2}', b"\xff"):
            with self.assertRaises(M.MigrationError): M.validate(raw)
    def test_entry_path_and_limits(self):
        base=json.loads(HAPPY.read_bytes())
        for field,value in (("source","bad"),("package_root","../bad"),
                            ("test_count",True),("test_count",0)):
            changed=json.loads(json.dumps(base)); changed["entries"][0][field]=value
            with self.assertRaises(M.MigrationError): M.validate(json.dumps(changed).encode())
        with self.assertRaises(M.MigrationError): M.validate(HAPPY.read_bytes(),max_tests=2)
        with self.assertRaises(M.MigrationError): M.validate(HAPPY.read_bytes(),cancelled=True)
        with self.assertRaises(M.MigrationError): M.validate(HAPPY.read_bytes(),max_bytes=1)
    def test_discover_topology(self):
        value=M.discover(ROOT); self.assertEqual(len(value["entries"]),3)
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaises(M.MigrationError): M.discover(Path(directory))
    def test_fuzz_and_benchmark(self):
        raw=HAPPY.read_bytes(); cases,rejected=M.fuzz(raw,0.001,1101)
        self.assertGreater(cases,0); self.assertGreater(rejected,0)
        M.benchmark(raw,1000)
        with mock.patch.object(M.time,"perf_counter_ns",side_effect=range(0,100_000_000,1_000_000)):
            with self.assertRaises(M.MigrationError): M.benchmark(raw,0.01)
    def invoke(self,*args):
        stdout=OutputCapture(); stderr=io.StringIO()
        with mock.patch.object(sys,"stdout",stdout), contextlib.redirect_stderr(stderr):
            return M.main(list(args)),stdout.value(),stderr.getvalue()
    def test_main_modes(self):
        status,out,_=self.invoke("--validate",str(HAPPY)); self.assertEqual(status,0); self.assertIn(M.SCHEMA,out)
        status,_,_=self.invoke("--discover",str(ROOT),"--benchmark-limit-ms","1000"); self.assertEqual(status,0)
        status,_,err=self.invoke("--validate",str(HAPPY),"--test-cancel-after-read"); self.assertEqual(status,130); self.assertIn("cancelled",err)
        status,_,_=self.invoke("--validate",str(HAPPY),"--fuzz-seconds","-1"); self.assertEqual(status,1)

if __name__ == "__main__": unittest.main()
