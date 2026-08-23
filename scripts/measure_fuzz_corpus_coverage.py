#!/usr/bin/env python3
"""Measure TEST-002B parser line and conditional-branch coverage."""
import ast,dis,importlib.util,sys,unittest
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]; CHECKER=ROOT/"scripts/check_fuzz_corpus.py"; TESTS=ROOT/"tests/runner/test_fuzz_corpus_unit.py"
TARGETS={"pairs","identity","digest","failure_code","regular_payload","validate"}
def load(name,path):
 spec=importlib.util.spec_from_file_location(name,path); module=importlib.util.module_from_spec(spec); spec.loader.exec_module(module); return module
def source_branches(path):
 tree=ast.parse(path.read_text()); arcs=set()
 def block(statements,after=None):
  for index,node in enumerate(statements):
   following=statements[index+1].lineno if index+1<len(statements) else after
   if isinstance(node,ast.If):
    arcs.add((node.lineno,node.body[0].lineno)); arcs.add((node.lineno,node.orelse[0].lineno if node.orelse else following)); block(node.body,following); block(node.orelse,following)
   elif isinstance(node,(ast.For,ast.While)):
    arcs.add((node.lineno,node.body[0].lineno)); arcs.add((node.lineno,following)); block(node.body,node.lineno); block(node.orelse,following)
  return arcs
 for node in tree.body:
  if isinstance(node,(ast.FunctionDef,ast.AsyncFunctionDef)) and node.name in TARGETS: block(node.body)
 return {arc for arc in arcs if None not in arc and arc[0]!=arc[1]}
def main():
 tests=load("test_fuzz_corpus_coverage_suite",TESTS); checker=tests.checker
 codes={getattr(checker,name).__code__ for name in TARGETS}; lines=set(); branches=source_branches(CHECKER); observed_lines=set(); observed_branches=set(); previous={}
 for code in codes:
  instructions=list(dis.get_instructions(code))
  for item in instructions:
   position=getattr(item,"positions",None); line=position.lineno if position is not None else item.starts_line
   if line: lines.add((code,line))
 def trace(frame,event,arg):
  code=frame.f_code
  if code not in codes: return trace
  if event=="line":
   observed_lines.add((code,frame.f_lineno)); prior=previous.get(frame)
   if prior is not None: observed_branches.add((prior,frame.f_lineno))
   previous[frame]=frame.f_lineno
  elif event=="return": previous.pop(frame,None)
  return trace
 suite=unittest.defaultTestLoader.loadTestsFromModule(tests); sys.settrace(trace)
 try: result=unittest.TextTestRunner(stream=sys.stderr,verbosity=0).run(suite)
 finally: sys.settrace(None)
 if not result.wasSuccessful(): return 1
 total_branches=len(branches); covered_branches=len(observed_branches&branches); line_percent=100*len(observed_lines&lines)/len(lines); branch_percent=100*covered_branches/total_branches
 print(f"fuzz-corpus parser coverage: line={line_percent:.2f}% branch={branch_percent:.2f}% lines={len(observed_lines&lines)}/{len(lines)} branches={covered_branches}/{total_branches}")
 if line_percent<90 or branch_percent<80:
  missing_lines=sorted(line for code,line in lines-observed_lines)
  missing_branches=sorted(f"{source}->{destination}" for source,destination in branches-observed_branches)
  print("missing lines: "+",".join(map(str,missing_lines)),file=sys.stderr); print("missing branches: "+",".join(missing_branches),file=sys.stderr)
 return 0 if line_percent>=90 and branch_percent>=80 else 1
if __name__=="__main__": sys.exit(main())
