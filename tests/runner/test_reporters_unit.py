#!/usr/bin/env python3
import contextlib, importlib.util, io, sys, tempfile, unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("reporters", ROOT / "scripts/check_test_reporters.py")
R = importlib.util.module_from_spec(SPEC); assert SPEC.loader; SPEC.loader.exec_module(R)
HAPPY = ROOT / "tests/fixtures/test-001e/happy"

class ReporterTests(unittest.TestCase):
    def raw(self, name): return R.read(HAPPY / name)
    def test_happy_formats(self):
        for fmt, name in (("human","report.txt"),("json","report.json"),("junit","report.xml")):
            with self.subTest(fmt=fmt): self.assertEqual(R.validate(self.raw(name), fmt), (1,1,1))
    def test_invalid_formats(self):
        for fmt in ("bad", "human", "json", "junit"):
            raw = b"bad\n" if fmt != "junit" else R.read(ROOT / "tests/fixtures/test-001e/invalid/report.xml")
            with self.subTest(fmt=fmt), self.assertRaises(R.ContractError): R.validate(raw, fmt)
    def test_json_matrix(self):
        import json
        base=json.loads(self.raw("report.json"))
        for value in (None,{**base,"schema":"bad"},{**base,"results":"bad"},{**base,"passed":2},{**base,"exit_code":0}):
            with self.assertRaises(R.ContractError): R.validate_json(json.dumps(value).encode())
        with self.assertRaises(R.ContractError): R.validate_json(b'{"schema":1,"schema":2}')
    def test_human_matrix(self):
        for raw in (b"",b"PASS a",b"FAIL a (exit x)\ntest result: 0 passed; 1 failed; 0 skipped\n",b"BAD a\ntest result: 0 passed; 0 failed; 0 skipped\n",b"PASS b\nPASS a\ntest result: 2 passed; 0 failed; 0 skipped\n"):
            with self.assertRaises(R.ContractError): R.validate_human(raw)
    def test_junit_matrix(self):
        for raw in (b"<!DOCTYPE x><testsuite/>",b"<bad/>",b'<testsuite name="" tests="0" failures="0" skipped="0"/>',b'<testsuite name="x" tests="x" failures="0" skipped="0"/>'):
            with self.assertRaises(R.ContractError): R.validate_junit(raw)
    def test_fuzz_benchmark(self):
        for fmt,name in (("human","report.txt"),("json","report.json"),("junit","report.xml")):
            self.assertGreater(R.fuzz(self.raw(name),fmt,0,1101),0); R.benchmark(self.raw(name),fmt,1000)
        with mock.patch.object(R.time,"perf_counter_ns",side_effect=range(0,100_000_000,1_000_000)):
            with self.assertRaises(R.ContractError): R.benchmark(self.raw("report.json"),"json",0.01)
    def test_read_limits(self):
        with self.assertRaises(R.ContractError): R.read(HAPPY / "missing")
        with tempfile.TemporaryDirectory() as d:
            p=Path(d)/"empty"; p.write_bytes(b"")
            with self.assertRaises(R.ContractError): R.read(p)
    def invoke(self,*args):
        with mock.patch.object(sys,"argv",["check_test_reporters.py",*args]), contextlib.redirect_stdout(io.StringIO()): return R.main()
    def test_main_modes(self):
        path=str(HAPPY/"report.json")
        self.assertEqual(self.invoke("--format","json","--validate",path),0)
        self.assertEqual(self.invoke("--format","json","--validate",path,"--fuzz-seconds","0.0001"),0)
        self.assertEqual(self.invoke("--format","json","--validate",path,"--benchmark-limit-ms","10"),0)
        with self.assertRaises(R.ContractError): self.invoke("--format","json","--validate",path,"--fuzz-seconds","-1")
        with self.assertRaises(R.ContractError): self.invoke("--format","json","--validate",path,"--benchmark-limit-ms","0")

if __name__ == "__main__": unittest.main()
