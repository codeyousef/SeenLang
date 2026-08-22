#!/usr/bin/env python3
import argparse,importlib.util,io,json,sys,tempfile,unittest
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]; spec=importlib.util.spec_from_file_location("checker",ROOT/"scripts/check_test_instrumentation.py"); checker=importlib.util.module_from_spec(spec); spec.loader.exec_module(checker)
def component(name,software=True,hardware=False): return {"compiled":True,"hardware_executed":hardware,"name":name,"software_executed":software}
def value(): return {"components":[component(n) for n in checker.NAMES],"coverage":True,"profile":"coverage","sanitizer":"","schema":checker.SCHEMA,"target":"linux-x86_64"}
class Tests(unittest.TestCase):
 def test_happy(self): self.assertEqual(len(checker.validate(json.dumps(value()).encode())["components"]),5)
 def test_modes(self):
  for profile,sanitizer in checker.PROFILES.items():
   item=value(); item["profile"]=profile; item["sanitizer"]=sanitizer; item["coverage"]=profile=="coverage"; checker.validate(json.dumps(item).encode())
 def test_hardware_rejected(self):
  item=value(); item["components"][0]["hardware_executed"]=True
  with self.assertRaises(checker.ContractError): checker.validate(json.dumps(item).encode())
 def test_missing_rejected(self):
  item=value(); item["components"].pop()
  with self.assertRaises(checker.ContractError) as raised: checker.validate(json.dumps(item).encode())
  self.assertEqual(raised.exception.code,"limit")
 def test_cancel(self):
  with self.assertRaises(checker.ContractError) as raised: checker.validate(json.dumps(value()).encode(),cancelled=True)
  self.assertEqual(raised.exception.code,"cancelled")
 def test_matrix(self):
  cases=[b"{",b"\xff",b'{"schema":1,"schema":2}',json.dumps({}).encode(),json.dumps({**value(),"target":"macos"}).encode(),json.dumps({**value(),"profile":"bad"}).encode(),json.dumps({**value(),"components":[{}]*5}).encode(),json.dumps({**value(),"components":[component("compiler-host")]*5}).encode(),json.dumps({**value(),"components":[component("unknown")]+value()["components"][1:]}).encode()]
  for raw in cases:
   with self.assertRaises(checker.ContractError): checker.validate(raw)
  with self.assertRaises(checker.ContractError): checker.validate(json.dumps(value()).encode(),max_bytes=1)
 def test_fuzz(self):
  cases,rejected=checker.fuzz(json.dumps(value()).encode(),.002,1101); self.assertGreater(cases,0); self.assertGreaterEqual(rejected,0)
 def test_execution_derivation(self):
  with tempfile.TemporaryDirectory() as directory:
   root=Path(directory); components={"abi_shims":"compile-only","compiler_host":"compile-only","native_runtime":"compile-only","seen_modules":"compile-only"}; build={"components":components,"modes":{"coverage":True,"debug":False,"sanitizer":"undefined"},"schema":"seen-build-instrumentation-evidence-v1","target":"linux-x86_64"}; gpu={**build,"components":{**components,"compiler_host":"source-only"}}
   paths={name:root/name for name in ("compiler.json","gpu.json","compiler.profdata","gpu.profdata","compiler.txt","gpu.txt","shader.glsl","compiler.log","gpu.log")}
   paths["compiler.json"].write_text(json.dumps(build)); paths["gpu.json"].write_text(json.dumps(gpu))
   for name in ("compiler.profdata","gpu.profdata"): paths[name].write_bytes(b"profile")
   for name in ("compiler.txt","gpu.txt"): paths[name].write_bytes(b"TOTAL")
   paths["shader.glsl"].write_bytes(b"gl_GlobalInvocationID")
   paths["compiler.log"].write_bytes(b"1 compute shader(s) emitted")
   paths["gpu.log"].write_bytes(b"PASS: TEST-002A_gpu_emitters_software")
   args=argparse.Namespace(compiler_build_report=paths["compiler.json"],gpu_build_report=paths["gpu.json"],compiler_profdata=paths["compiler.profdata"],test_profdata=paths["gpu.profdata"],compiler_coverage=paths["compiler.txt"],test_coverage=paths["gpu.txt"],gpu_glsl=paths["shader.glsl"],compiler_log=paths["compiler.log"],test_log=paths["gpu.log"])
   self.assertEqual(checker.derive_execution(args)["schema"],checker.SCHEMA)
   paths["compiler.txt"].write_bytes(b"missing total")
   with self.assertRaises(checker.ContractError): checker.derive_execution(args)
   with self.assertRaises(checker.ContractError): checker.require_file(root/"missing")
   paths["compiler.json"].write_text("{")
   with self.assertRaises(checker.ContractError): checker.derive_execution(args)
 def test_main_paths(self):
  with tempfile.TemporaryDirectory() as directory:
   evidence=Path(directory)/"evidence.json"; evidence.write_text(json.dumps(value()))
   class Output:
    def __init__(self): self.buffer=io.BytesIO()
   original_stdout,original_stderr=sys.stdout,sys.stderr
   try:
    sys.stdout=Output(); sys.stderr=io.StringIO()
    self.assertEqual(checker.main(["--evidence",str(evidence)]),0)
    self.assertEqual(checker.main(["--evidence",str(evidence),"--fuzz-seconds",".001"]),0)
    self.assertEqual(checker.main(["--evidence",str(evidence),"--test-cancel-after-read"]),130)
    self.assertEqual(checker.main([]),1)
    self.assertEqual(checker.main(["--evidence",str(Path(directory)/"missing")]),1)
    self.assertEqual(checker.main(["--evidence",str(evidence),"--fuzz-seconds","301"]),1)
    self.assertEqual(checker.main(["--derive-execution"]),1)
   finally: sys.stdout,sys.stderr=original_stdout,original_stderr
if __name__=="__main__": unittest.main()
