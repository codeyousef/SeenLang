#!/usr/bin/env python3
import argparse,importlib.util,json,statistics,sys,time
from pathlib import Path
def main():
 p=argparse.ArgumentParser(); p.add_argument("corpus",type=Path); p.add_argument("baseline",type=Path); a=p.parse_args()
 try:
  root=Path(__file__).resolve().parents[1]; spec=importlib.util.spec_from_file_location("checker",root/"scripts/check_fuzz_corpus.py"); c=importlib.util.module_from_spec(spec); spec.loader.exec_module(c)
  raw=a.corpus.read_bytes(); expected=c.validate(raw,a.corpus.parent); control=json.loads(raw); b=json.loads(a.baseline.read_bytes())
  if set(b)!={"baseline_ratio_ppm","iterations_per_sample","max_regression_percent","samples","version","warmups"} or b["version"]!=1 or b["warmups"]!=5 or b["samples"]!=30 or b["max_regression_percent"]!=5: raise ValueError("invalid 5/30/5 baseline")
  n=b["iterations_per_sample"]
  def candidate():
   start=time.perf_counter_ns()
   for _ in range(n):
    if c.validate(raw,a.corpus.parent)!=expected: raise ValueError("validation changed")
   return time.perf_counter_ns()-start
  def reference():
   start=time.perf_counter_ns()
   for _ in range(n):
    if json.loads(raw)!=control: raise ValueError("control changed")
   return time.perf_counter_ns()-start
  for _ in range(5): candidate(); reference()
  cs=[]; rs=[]
  for i in range(30):
   if i%2: cs.append(candidate()); rs.append(reference())
   else: rs.append(reference()); cs.append(candidate())
  ratio=statistics.median(cs)*1000000//statistics.median(rs); ceiling=b["baseline_ratio_ppm"]*105//100
  if ratio>ceiling: raise ValueError(f"ratio {ratio} exceeds 5% ceiling {ceiling}")
  print(f"fuzz-corpus benchmark: ratio_ppm={ratio} ceiling_ratio_ppm={ceiling} warmups=5 samples=30 status=pass")
 except (OSError,ValueError,json.JSONDecodeError) as error: print(f"fuzz-corpus benchmark: {error}",file=sys.stderr); return 1
 return 0
if __name__=="__main__": sys.exit(main())
