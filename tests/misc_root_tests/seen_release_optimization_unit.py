#!/usr/bin/env python3
from __future__ import annotations
import importlib.util, json, unittest
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
SPEC=importlib.util.spec_from_file_location("checker",ROOT/"scripts/check_release_optimization.py")
assert SPEC is not None and SPEC.loader is not None
CHECKER=importlib.util.module_from_spec(SPEC); SPEC.loader.exec_module(CHECKER)
def plan() -> dict[str,object]:
    return {"lto_mode":"full","pgo_mode":"use","pgo_path":"profiles/app.profdata","release":True,"schema":"seen-release-optimization-plan-v1","target":"linux-x86_64"}
class Tests(unittest.TestCase):
    def raw(self,value:object|None=None)->bytes: return json.dumps(plan() if value is None else value).encode()
    def code(self,code:str,raw:bytes,**kwargs:object)->None:
        with self.assertRaises(CHECKER.ReleaseOptimizationError) as raised: CHECKER.validate(raw,**kwargs)
        self.assertEqual(code,raised.exception.code)
    def test_happy(self)->None: self.assertEqual("full",CHECKER.validate(self.raw())["lto_mode"])
    def test_invalid_limit_cancel_platform(self)->None:
        v=plan(); v["lto_mode"]="auto"; self.code("invalid",self.raw(v))
        v=plan(); v["target"]="macos-arm64"; self.code("platform",self.raw(v))
        v=plan(); v["pgo_path"]="../raw.profraw"; self.code("invalid",self.raw(v))
        self.code("limit",self.raw(),max_bytes=1); self.code("cancelled",self.raw(),cancelled=True)
    def test_duplicate_and_fuzz(self)->None:
        self.code("invalid",b'{"schema":1,"schema":2}'); CHECKER.fuzz(self.raw(),0.02,1101)
if __name__=="__main__": unittest.main()
