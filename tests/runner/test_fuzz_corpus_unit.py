#!/usr/bin/env python3
import copy,hashlib,importlib.util,io,json,sys,tempfile,unittest
from unittest import mock
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]; spec=importlib.util.spec_from_file_location("checker",ROOT/"scripts/check_fuzz_corpus.py"); checker=importlib.util.module_from_spec(spec); spec.loader.exec_module(checker)
def corpus(root,payload=b"A\n"):
 sha=hashlib.sha256(payload).hexdigest(); cases=root/"cases"; cases.mkdir(); (cases/(sha+".bin")).write_bytes(payload)
 return {"entries":[{"failure_code":"test.002b.reproducer","id":"alpha","minimized_bytes":len(payload),"original_bytes":8,"replay_codes":["test.002b.reproducer"]*3,"sha256":sha}],"max_input_bytes":65536,"required_replay_runs":3,"schema":checker.SCHEMA,"seed":1101,"target":"linux-x86_64"}
class Tests(unittest.TestCase):
 def reject(self,value,root):
  with self.assertRaises(checker.ContractError): checker.validate(json.dumps(value).encode(),root)
 def test_happy_and_canonical(self):
  with tempfile.TemporaryDirectory() as directory:
   root=Path(directory); value=corpus(root); self.assertEqual(checker.validate(json.dumps(value).encode(),root)["seed"],1101); self.assertEqual(checker.canonical(value,root),checker.canonical(value,root))
 def test_minimization(self): self.assertEqual(checker.minimize_bytes(b"zzAzzA",lambda item:b"A" in item),b"A")
 def test_non_reproducer(self):
  with self.assertRaises(checker.ContractError): checker.minimize_bytes(b"z",lambda item:False)
 def test_cancel_and_limit(self):
  with tempfile.TemporaryDirectory() as directory:
   root=Path(directory); raw=json.dumps(corpus(root)).encode()
   with self.assertRaises(checker.ContractError) as raised: checker.validate(raw,root,cancelled=True)
   self.assertEqual(raised.exception.code,"cancelled")
   with self.assertRaises(checker.ContractError) as raised: checker.validate(raw,root,max_bytes=1)
   self.assertEqual(raised.exception.code,"limit")
 def test_invalid_matrix(self):
  with tempfile.TemporaryDirectory() as directory:
   root=Path(directory); value=corpus(root)
   cases=[b"{",b"\xff",b'{"schema":1,"schema":2}',json.dumps({}).encode()]
   for raw in cases:
    with self.assertRaises(checker.ContractError): checker.validate(raw,root)
   changes=[("target","macos"),("seed",0),("max_input_bytes",65537),("required_replay_runs",0)]
   for key,bad in changes:
    item={**value,key:bad}
    with self.assertRaises(checker.ContractError): checker.validate(json.dumps(item).encode(),root)
 def test_parser_guard_matrix(self):
  self.assertEqual(checker.pairs([("a",1)]),{"a":1})
  with self.assertRaises(checker.ContractError): checker.pairs([("a",1),("a",2)])
  for value in (None,"","a"*129,"-a","a-","A","a_","é"):
   self.assertFalse(checker.identity(value))
  for value in ("a","1","a-b"): self.assertTrue(checker.identity(value))
  for value in (None,"0"*63,"G"*64): self.assertFalse(checker.digest(value))
  self.assertTrue(checker.digest("0"*64))
  for value in (None,"short","test.BAD","test.bad/segment","test.é"):
   self.assertFalse(checker.failure_code(value))
  self.assertTrue(checker.failure_code("test.valid-code_1"))
  with tempfile.TemporaryDirectory() as directory:
   root=Path(directory); good=corpus(root)
   for raw in (json.dumps([]).encode(),json.dumps({}).encode()):
    with self.assertRaises(checker.ContractError): checker.validate(raw,root)
   for key,bads in (("schema",["bad"]),("target",["macos"]),("seed",["1",True,0]),("max_input_bytes",["1",True,0,65537]),("required_replay_runs",["3",True,0,101]),("entries",["bad",[],[{}]*1025])):
    for bad in bads:
     item=copy.deepcopy(good); item[key]=bad; self.reject(item,root)
   for bad in (None,{},{"id":"alpha"}):
    item=copy.deepcopy(good); item["entries"]=[bad]; self.reject(item,root)
   entry_cases={"id":[None,"","a"*129,"-a","A"],"sha256":[None,"0"*63,"G"*64],"original_bytes":["8",True,0,65537],"minimized_bytes":["2",True,0,9],"failure_code":[None,"short","x"*129,"other.failure"],"replay_codes":[None,[],["test.002b.reproducer"],["test.changed"]*3]}
   for key,bads in entry_cases.items():
    for bad in bads:
     item=copy.deepcopy(good); item["entries"][0][key]=bad; self.reject(item,root)
   duplicate=copy.deepcopy(good); duplicate["entries"].append(copy.deepcopy(duplicate["entries"][0])); duplicate["entries"][1]["id"]="beta"; self.reject(duplicate,root)
   unordered=copy.deepcopy(good); unordered["entries"].append(copy.deepcopy(unordered["entries"][0])); unordered["entries"][0]["id"]="beta"; self.reject(unordered,root)
   first=b"A"*40000; second=b"B"*40000; first_sha=hashlib.sha256(first).hexdigest(); second_sha=hashlib.sha256(second).hexdigest(); (root/"cases"/(first_sha+".bin")).write_bytes(first); (root/"cases"/(second_sha+".bin")).write_bytes(second)
   budget=copy.deepcopy(good); budget["entries"]=[{"failure_code":"test.002b.reproducer","id":"alpha","minimized_bytes":40000,"original_bytes":40000,"replay_codes":["test.002b.reproducer"]*3,"sha256":first_sha},{"failure_code":"test.002b.reproducer","id":"beta","minimized_bytes":40000,"original_bytes":40000,"replay_codes":["test.002b.reproducer"]*3,"sha256":second_sha}]; self.reject(budget,root)
 def test_payload_guard_matrix(self):
  with tempfile.TemporaryDirectory() as directory:
   root=Path(directory)
   with self.assertRaises(checker.ContractError): checker.regular_payload(root,"0"*64)
   (root/"cases").write_text("not-directory")
   with self.assertRaises(checker.ContractError): checker.regular_payload(root,"0"*64)
   (root/"cases").unlink(); outside=root/"outside"; outside.mkdir(); (root/"cases").symlink_to(outside,target_is_directory=True)
   with self.assertRaises(checker.ContractError): checker.regular_payload(root,"0"*64)
   (root/"cases").unlink(); (root/"cases").mkdir()
   with self.assertRaises(checker.ContractError): checker.regular_payload(root,"0"*64)
   payload=root/"cases"/(("0"*64)+".bin"); payload.mkdir()
   with self.assertRaises(checker.ContractError): checker.regular_payload(root,"0"*64)
   payload.rmdir(); outside_file=root/"outside.bin"; outside_file.write_bytes(b"x"); payload.symlink_to(outside_file)
   with self.assertRaises(checker.ContractError): checker.regular_payload(root,"0"*64)
   payload.unlink(); payload.write_bytes(b"x")
   with mock.patch.object(Path,"read_bytes",side_effect=OSError("denied")):
    with self.assertRaises(checker.ContractError): checker.regular_payload(root,"0"*64)
   payload.write_bytes(b"x"*(checker.MAX_INPUT+1))
   with self.assertRaises(checker.ContractError): checker.regular_payload(root,"0"*64)
 def test_missing_symlink_and_hash_rejected(self):
  with tempfile.TemporaryDirectory() as directory:
   root=Path(directory); value=corpus(root); sha=value["entries"][0]["sha256"]; path=root/"cases"/(sha+".bin")
   path.write_bytes(b"changed")
   with self.assertRaises(checker.ContractError): checker.validate(json.dumps(value).encode(),root)
   path.unlink(); outside=root/"outside"; outside.write_bytes(b"A\n"); path.symlink_to(outside)
   with self.assertRaises(checker.ContractError): checker.validate(json.dumps(value).encode(),root)
   path.unlink()
   with self.assertRaises(checker.ContractError): checker.validate(json.dumps(value).encode(),root)
 def test_fuzz(self):
  with tempfile.TemporaryDirectory() as directory:
   root=Path(directory); raw=json.dumps(corpus(root)).encode(); cases,rejected=checker.fuzz(raw,root,.002,1101); self.assertGreater(cases,0); self.assertGreaterEqual(rejected,0)
 def test_atomic_cleanup_and_main(self):
  with tempfile.TemporaryDirectory() as directory:
   root=Path(directory); value=corpus(root); source=root/"corpus.json"; source.write_text(json.dumps(value)); output=root/"canonical.json"
   class Output:
    def __init__(self): self.buffer=io.BytesIO()
   original_stdout,original_stderr=sys.stdout,sys.stderr
   try:
    sys.stdout=Output(); sys.stderr=io.StringIO()
    self.assertEqual(checker.main(["--corpus",str(source),"--output",str(output),"--exercise-minimizer"]),0); self.assertTrue(output.is_file()); self.assertEqual(list(root.glob(".fuzz-corpus-*")),[])
    self.assertEqual(checker.main(["--corpus",str(source),"--test-cancel-after-read"]),130)
    self.assertEqual(checker.main(["--corpus",str(source),"--fuzz-seconds","301"]),1)
    self.assertEqual(checker.main(["--corpus",str(source),"--max-bytes","1"]),1)
    self.assertEqual(checker.main(["--corpus",str(root/"missing")]),1)
    link=root/"linked.json"; link.symlink_to(source)
    self.assertEqual(checker.main(["--corpus",str(link)]),1)
   finally: sys.stdout,sys.stderr=original_stdout,original_stderr
if __name__=="__main__": unittest.main()
