#!/usr/bin/env python3
import contextlib, copy, importlib.util, io, json, sys, unittest
from pathlib import Path
from unittest import mock

ROOT=Path(__file__).resolve().parents[2]
SPEC=importlib.util.spec_from_file_location("error_contract",ROOT/"scripts/check_error_contract.py")
E=importlib.util.module_from_spec(SPEC); assert SPEC.loader; SPEC.loader.exec_module(E)
HAPPY=ROOT/"tests/fixtures/err-001a/happy/contract.json"
class OutputCapture:
    def __init__(self): self.buffer=io.BytesIO(); self.text=io.StringIO()
    def write(self,value): return self.text.write(value)
    def flush(self): return None
    def value(self): return self.buffer.getvalue().decode()+self.text.getvalue()
class ErrorContractTests(unittest.TestCase):
    def value(self): return json.loads(HAPPY.read_bytes())
    def raw(self,value): return json.dumps(value,separators=(",",":"),sort_keys=True).encode()+b"\n"
    def test_happy_canonical(self):
        raw=HAPPY.read_bytes(); self.assertEqual(E.canonical_bytes(E.validate(raw)),raw)
        self.assertTrue(E.identity("io.read-1")); self.assertFalse(E.identity("IO"))
    def test_invalid_envelope_matrix(self):
        base=self.value()
        for value in (None,{**base,"schema":"bad"},{**base,"extra":1}):
            with self.assertRaises(E.ContractError): E.validate(self.raw(value))
        for raw in (b"",b'{"schema":1,"schema":2}',b"\xff"):
            with self.assertRaises(E.ContractError): E.validate(raw)
    def test_error_matrix(self):
        base=self.value()
        changes=(("code","BAD"),("message",1),("retry","later"),("redaction","secret"),("native_code",True),("causes",{}))
        for key,value in changes:
            changed=copy.deepcopy(base); changed["error"][key]=value
            with self.subTest(key=key),self.assertRaises(E.ContractError): E.validate(self.raw(changed))
        changed=copy.deepcopy(base); changed["error"]["message"]="x"*4097
        with self.assertRaises(E.ContractError): E.validate(self.raw(changed))
        changed=copy.deepcopy(base); changed["error"]["causes"]=[copy.deepcopy(base["error"])]*9
        with self.assertRaises(E.ContractError): E.validate(self.raw(changed))
    def test_context_matrix(self):
        base=self.value()
        for key,value in (("cancellation",1),("monotonic_deadline",-1),("trace_id","BAD"),("limits",{})):
            changed=copy.deepcopy(base); changed["context"][key]=value
            with self.subTest(key=key),self.assertRaises(E.ContractError): E.validate(self.raw(changed))
        for key in E.LIMIT_FIELDS:
            changed=copy.deepcopy(base); changed["context"]["limits"][key]=0
            with self.subTest(key=key),self.assertRaises(E.ContractError): E.validate(self.raw(changed))
        with self.assertRaises(E.ContractError): E.validate(HAPPY.read_bytes(),cancelled=True)
        with self.assertRaises(E.ContractError): E.validate(HAPPY.read_bytes(),max_bytes=1)
    def test_redaction_recursion(self):
        base=self.value(); nested=copy.deepcopy(base["error"]); nested["redaction"]="sensitive"; nested["message"]="secret"
        base["error"]["causes"]=[nested]; rendered=E.canonical_bytes(E.validate(self.raw(base)))
        self.assertNotIn(b"secret",rendered); self.assertIn(b"[redacted]",rendered)
    def test_fuzz_benchmark(self):
        raw=HAPPY.read_bytes(); cases,rejected=E.fuzz(raw,.001,1101); self.assertGreater(cases,0); self.assertGreater(rejected,0); E.benchmark(raw,1000)
        with mock.patch.object(E.time,"perf_counter_ns",side_effect=range(0,100_000_000,1_000_000)):
            with self.assertRaises(E.ContractError): E.benchmark(raw,.01)
    def invoke(self,*args):
        out=OutputCapture(); err=io.StringIO()
        with mock.patch.object(sys,"stdout",out),contextlib.redirect_stderr(err): return E.main(list(args)),out.value(),err.getvalue()
    def test_main_modes(self):
        status,out,_=self.invoke("--validate",str(HAPPY)); self.assertEqual(status,0); self.assertIn(E.SCHEMA,out)
        status,_,_=self.invoke("--validate",str(HAPPY),"--benchmark-limit-ms","1000"); self.assertEqual(status,0)
        status,_,err=self.invoke("--validate",str(HAPPY),"--test-cancel-after-read"); self.assertEqual(status,130); self.assertIn("cancelled",err)
        status,_,_=self.invoke("--validate",str(HAPPY),"--fuzz-seconds","-1"); self.assertEqual(status,1)
if __name__=="__main__": unittest.main()
