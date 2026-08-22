#!/usr/bin/env python3
"""Validate the canonical ERR-001A structured error and operation context."""

from __future__ import annotations

import argparse, copy, json, random, sys, time
from pathlib import Path

SCHEMA="seen-error-v1"; MAX_BYTES=1024*1024; MAX_MESSAGE=4096
MAX_CAUSES=8; MAX_DEPTH=8; MAX_IDENTITY=128
FIELDS={"context","error","schema"}
ERROR_FIELDS={"causes","code","message","native_code","operation","redaction","retry","subsystem"}
CONTEXT_FIELDS={"cancellation","limits","monotonic_deadline","trace_id"}
LIMIT_FIELDS={"max_graph_depth","max_graph_edges","max_graph_modules","max_identity_bytes","max_manifest_bytes","max_path_bytes","max_targets","max_version_bytes"}
LIMIT_MAX={"max_graph_depth":1024,"max_graph_edges":1_000_000,"max_graph_modules":100_000,"max_identity_bytes":4096,"max_manifest_bytes":16_777_216,"max_path_bytes":4096,"max_targets":100_000,"max_version_bytes":4096}

class ContractError(ValueError):
    def __init__(self,code,message): super().__init__(message); self.code=code
def fail(code,message): raise ContractError(code,message)
def no_duplicates(pairs):
    result={}
    for key,value in pairs:
        if key in result: fail("invalid",f"duplicate field: {key}")
        result[key]=value
    return result
def identity(value):
    return isinstance(value,str) and 0<len(value.encode())<=MAX_IDENTITY and value.isascii() and all(ch.islower() or ch.isdigit() or ch in "-._" for ch in value)
def validate_error(value,depth=0):
    if not isinstance(value,dict) or set(value)!=ERROR_FIELDS: fail("invalid","invalid error fields")
    if not all(identity(value[key]) for key in ("code","subsystem","operation")): fail("invalid","invalid error identity")
    if not isinstance(value["message"],str): fail("invalid","error message must be text")
    if len(value["message"].encode())>MAX_MESSAGE: fail("limit","error message exceeds limit")
    if value["retry"] not in ("never","transient") or value["redaction"] not in ("public","sensitive"): fail("invalid","invalid error policy")
    native=value["native_code"]
    if native is not None and (not isinstance(native,int) or isinstance(native,bool) or native<-(1<<63) or native>(1<<63)-1): fail("invalid","invalid native code")
    causes=value["causes"]
    if not isinstance(causes,list): fail("invalid","causes must be an array")
    if depth>MAX_DEPTH or len(causes)>MAX_CAUSES: fail("limit","error cause bound exceeded")
    for cause in causes: validate_error(cause,depth+1)
def validate_context(value,cancelled):
    if not isinstance(value,dict) or set(value)!=CONTEXT_FIELDS: fail("invalid","invalid context fields")
    if not isinstance(value["cancellation"],bool): fail("invalid","invalid cancellation flag")
    if cancelled or value["cancellation"]: fail("cancelled","operation context is cancelled")
    deadline=value["monotonic_deadline"]
    if not isinstance(deadline,int) or isinstance(deadline,bool) or deadline<0: fail("cancelled","operation deadline is invalid")
    if not identity(value["trace_id"]): fail("invalid","invalid trace identity")
    limits=value["limits"]
    if not isinstance(limits,dict) or set(limits)!=LIMIT_FIELDS: fail("invalid","invalid operation limits")
    for key,maximum in LIMIT_MAX.items():
        item=limits[key]
        if not isinstance(item,int) or isinstance(item,bool) or not 1<=item<=maximum: fail("limit",f"invalid operation limit: {key}")
def validate(raw,max_bytes=MAX_BYTES,cancelled=False):
    if not 1<=max_bytes<=MAX_BYTES or len(raw)>max_bytes: fail("limit","error contract byte limit exceeded")
    try: value=json.loads(raw.decode(),object_pairs_hook=no_duplicates)
    except (UnicodeDecodeError,json.JSONDecodeError) as error: fail("invalid",f"invalid JSON: {error}")
    if not isinstance(value,dict) or set(value)!=FIELDS or value["schema"]!=SCHEMA: fail("invalid","invalid error envelope")
    validate_context(value["context"],cancelled); validate_error(value["error"]); return value
def redact_error(value):
    result=copy.deepcopy(value)
    if result["redaction"]=="sensitive": result["message"]="[redacted]"
    result["causes"]=[redact_error(cause) for cause in result["causes"]]
    return result
def canonical_bytes(value):
    safe=copy.deepcopy(value); safe["error"]=redact_error(safe["error"])
    return (json.dumps(safe,separators=(",",":"),sort_keys=True)+"\n").encode()
def fuzz(raw,seconds,seed):
    rng=random.Random(seed); deadline=time.monotonic()+seconds; cases=rejected=0
    while time.monotonic()<deadline:
        changed=bytearray(raw)
        if changed: changed[rng.randrange(len(changed))]=rng.randrange(256)
        cases+=1
        try: validate(bytes(changed))
        except ContractError: rejected+=1
    return cases,rejected
def benchmark(raw,limit_ms):
    for _ in range(5): validate(raw)
    samples=[]
    for _ in range(30):
        started=time.perf_counter_ns(); validate(raw); samples.append((time.perf_counter_ns()-started)/1_000_000)
    samples.sort(); median=samples[len(samples)//2]
    if median>limit_ms: fail("limit",f"benchmark median {median:.6f}ms exceeds {limit_ms:.6f}ms")
    print(f"warmups=5 samples=30 median_ms={median:.6f} status=pass")
def main(argv=None):
    parser=argparse.ArgumentParser(description=__doc__); parser.add_argument("--validate",type=Path,required=True)
    parser.add_argument("--max-bytes",type=int,default=MAX_BYTES); parser.add_argument("--fuzz-seconds",type=float,default=0)
    parser.add_argument("--seed",type=int,default=1101); parser.add_argument("--benchmark-limit-ms",type=float,default=0)
    parser.add_argument("--test-cancel-after-read",action="store_true"); args=parser.parse_args(argv)
    try:
        if not 0<=args.fuzz_seconds<=300 or args.benchmark_limit_ms<0: fail("limit","invalid verification limit")
        raw=args.validate.read_bytes(); value=validate(raw,max_bytes=args.max_bytes,cancelled=args.test_cancel_after_read)
        if args.fuzz_seconds:
            cases,rejected=fuzz(raw,args.fuzz_seconds,args.seed); print(f"seed={args.seed} cases={cases} rejected={rejected}",file=sys.stderr)
        if args.benchmark_limit_ms: benchmark(raw,args.benchmark_limit_ms)
        else: sys.stdout.buffer.write(canonical_bytes(value))
    except ContractError as error:
        print(f"err.001a.{error.code}: {error}",file=sys.stderr); return 130 if error.code=="cancelled" else 1
    except OSError as error: print(f"err.001a.io: {error}",file=sys.stderr); return 1
    return 0
if __name__=="__main__": sys.exit(main())
