#!/usr/bin/env python3
"""Validate, canonicalize, minimize, and replay TEST-002B fuzz corpora."""
import argparse, hashlib, json, os, random, stat, sys, tempfile, time
from pathlib import Path

SCHEMA="seen-test-fuzz-corpus-v1"; MAX_INPUT=65536; MAX_ENTRIES=1024
MAX_REPLAYS=100; MAX_MANIFEST=1048576
class ContractError(ValueError):
    def __init__(self,code,message): super().__init__(message); self.code=code
def fail(code,message): raise ContractError(code,message)
def pairs(items):
    out={}
    for key,value in items:
        if key in out:
            fail("invalid",f"duplicate field: {key}")
        out[key]=value
    return out
def identity(value):
    if not isinstance(value,str) or not 0<len(value)<=128 or value!=value.strip("-"):
        return False
    for character in value:
        if not character.isascii() or not (character.islower() or character.isdigit() or character=="-"):
            return False
    return True
def digest(value):
    if not isinstance(value,str) or len(value)!=64:
        return False
    return all(character in "0123456789abcdef" for character in value)
def failure_code(value):
    if not isinstance(value,str) or not 10<=len(value)<=128 or not value.startswith("test."):
        return False
    return all(character in "abcdefghijklmnopqrstuvwxyz0123456789._-" for character in value)
def regular_payload(root,sha):
    cases_path=root/"cases"
    try:
        cases_mode=cases_path.lstat().st_mode
    except OSError as error:
        fail("io",f"missing corpus cases directory: {type(error).__name__}")
    if not stat.S_ISDIR(cases_mode) or stat.S_ISLNK(cases_mode):
        fail("invalid","corpus cases directory is not a contained directory")
    cases=cases_path.resolve(); path=cases/(sha+".bin")
    try:
        resolved=path.resolve(strict=True); payload_stat=path.lstat(); mode=payload_stat.st_mode
    except OSError as error:
        fail("io",f"missing corpus payload: {type(error).__name__}")
    if resolved.parent!=cases or not stat.S_ISREG(mode) or stat.S_ISLNK(mode):
        fail("invalid","corpus payload is not a contained regular file")
    if not 1<=payload_stat.st_size<=MAX_INPUT:
        fail("limit","corpus payload byte limit exceeded")
    try:
        return path.read_bytes()
    except OSError as error:
        fail("io",f"unreadable corpus payload: {type(error).__name__}")
def validate(raw,root,max_bytes=MAX_MANIFEST,cancelled=False):
    if not 1<=max_bytes<=MAX_MANIFEST or len(raw)>max_bytes:
        fail("limit","corpus manifest byte limit exceeded")
    try:
        value=json.loads(raw.decode(),object_pairs_hook=pairs)
    except (UnicodeDecodeError,json.JSONDecodeError) as error:
        fail("invalid",f"invalid JSON: {type(error).__name__}")
    if cancelled:
        fail("cancelled","fuzz corpus replay was cancelled")
    fields={"entries","max_input_bytes","required_replay_runs","schema","seed","target"}
    if not isinstance(value,dict) or set(value)!=fields:
        fail("invalid","invalid corpus manifest fields")
    if value["schema"]!=SCHEMA:
        fail("invalid","unsupported corpus schema")
    if value["target"]!="linux-x86_64":
        fail("platform","unsupported corpus target")
    if not isinstance(value["seed"],int) or isinstance(value["seed"],bool) or value["seed"]<1:
        fail("invalid","invalid corpus seed")
    maximum=value["max_input_bytes"]; runs=value["required_replay_runs"]; entries=value["entries"]
    if not isinstance(maximum,int) or isinstance(maximum,bool) or not 1<=maximum<=MAX_INPUT:
        fail("limit","invalid maximum input bytes")
    if not isinstance(runs,int) or isinstance(runs,bool) or not 1<=runs<=MAX_REPLAYS:
        fail("limit","invalid replay count")
    if not isinstance(entries,list) or not 1<=len(entries)<=MAX_ENTRIES:
        fail("limit","invalid corpus entry count")
    previous=""; digests=set(); canonical_entries=[]; total_bytes=0
    entry_fields={"failure_code","id","minimized_bytes","original_bytes","replay_codes","sha256"}
    for entry in entries:
        if not isinstance(entry,dict) or set(entry)!=entry_fields:
            fail("invalid","invalid corpus entry fields")
        name=entry["id"]; sha=entry["sha256"]; code=entry["failure_code"]
        if not identity(name) or (previous and name.encode()<=previous.encode()):
            fail("invalid","corpus identities are not canonical")
        if not digest(sha) or sha in digests:
            fail("invalid","corpus digest is invalid or duplicated")
        original=entry["original_bytes"]; minimized=entry["minimized_bytes"]
        if any(not isinstance(n,int) or isinstance(n,bool) for n in (original,minimized)) or not 1<=minimized<=original<=maximum:
            fail("invalid","corpus size lifecycle is invalid")
        if not failure_code(code):
            fail("invalid","failure code is invalid")
        replay=entry["replay_codes"]
        if not isinstance(replay,list) or len(replay)!=runs or any(observed!=code for observed in replay):
            fail("invalid","replay did not reproduce the stable failure")
        payload=regular_payload(Path(root),sha)
        if len(payload)!=minimized or hashlib.sha256(payload).hexdigest()!=sha:
            fail("invalid","payload bytes do not match content address")
        total_bytes+=len(payload)
        if total_bytes>maximum:
            fail("limit","corpus payload budget exceeded")
        canonical_entries.append(dict(entry)); previous=name; digests.add(sha)
    return {**value,"entries":canonical_entries}
def canonical(value,root): return (json.dumps(validate(json.dumps(value).encode(),root),separators=(",",":"),sort_keys=True)+"\n").encode()
def minimize_bytes(payload,reproduces):
    if not payload or not reproduces(payload): fail("invalid","candidate does not reproduce")
    current=bytes(payload); granularity=2
    while len(current)>1:
        chunk=max(1,(len(current)+granularity-1)//granularity); changed=False
        for start in range(0,len(current),chunk):
            candidate=current[:start]+current[start+chunk:]
            if candidate and reproduces(candidate): current=candidate; granularity=max(2,granularity-1); changed=True; break
        if not changed:
            if granularity>=len(current): break
            granularity=min(len(current),granularity*2)
    return current
def fuzz(raw,root,seconds,seed):
    rng=random.Random(seed); deadline=time.monotonic()+seconds; cases=rejected=0
    while time.monotonic()<deadline:
        changed=bytearray(raw)
        if changed: changed[rng.randrange(len(changed))]=rng.randrange(256)
        cases+=1
        try: validate(bytes(changed),root)
        except ContractError: rejected+=1
    return cases,rejected
def write_atomic(path,raw):
    path=Path(path); descriptor=None; temporary=None
    try:
        descriptor,temporary=tempfile.mkstemp(prefix=".fuzz-corpus-",dir=path.parent)
        with os.fdopen(descriptor,"wb") as output: descriptor=None; output.write(raw); output.flush(); os.fsync(output.fileno())
        os.replace(temporary,path); temporary=None
    except OSError as error: fail("io",f"canonical corpus write failed: {type(error).__name__}")
    finally:
        if descriptor is not None: os.close(descriptor)
        if temporary is not None:
            try: os.unlink(temporary)
            except FileNotFoundError: pass
def read_manifest(path,max_bytes):
    if not 1<=max_bytes<=MAX_MANIFEST: fail("limit","corpus manifest byte limit exceeded")
    try: metadata=path.lstat()
    except OSError as error: fail("io",f"missing corpus manifest: {type(error).__name__}")
    if not stat.S_ISREG(metadata.st_mode) or stat.S_ISLNK(metadata.st_mode): fail("invalid","corpus manifest is not a regular non-symlink file")
    if metadata.st_size>max_bytes: fail("limit","corpus manifest byte limit exceeded")
    try: return path.read_bytes()
    except OSError as error: fail("io",f"unreadable corpus manifest: {type(error).__name__}")
def main(argv=None):
    parser=argparse.ArgumentParser(description=__doc__); parser.add_argument("--corpus",type=Path,required=True); parser.add_argument("--output",type=Path); parser.add_argument("--max-bytes",type=int,default=MAX_MANIFEST); parser.add_argument("--fuzz-seconds",type=float,default=0); parser.add_argument("--seed",type=int,default=1101); parser.add_argument("--exercise-minimizer",action="store_true"); parser.add_argument("--test-cancel-after-read",action="store_true"); args=parser.parse_args(argv)
    try:
        if not 0<=args.fuzz_seconds<=300: fail("limit","invalid fuzz duration")
        raw=read_manifest(args.corpus,args.max_bytes); root=args.corpus.parent; value=validate(raw,root,args.max_bytes,args.test_cancel_after_read); rendered=(json.dumps(value,separators=(",",":"),sort_keys=True)+"\n").encode()
        if args.exercise_minimizer and minimize_bytes(b"zzAzzA",lambda item:b"A" in item)!=b"A": fail("invalid","deterministic minimization failed")
        if args.fuzz_seconds:
            cases,rejected=fuzz(raw,root,args.fuzz_seconds,args.seed); print(f"seed={args.seed} cases={cases} rejected={rejected}",file=sys.stderr)
        if args.output is not None: write_atomic(args.output,rendered)
        sys.stdout.buffer.write(rendered)
    except ContractError as error: print(f"test.002b.{error.code}: {error}",file=sys.stderr); return 130 if error.code=="cancelled" else 1
    except OSError as error: print(f"test.002b.io: {type(error).__name__}",file=sys.stderr); return 1
    return 0
if __name__=="__main__": sys.exit(main())
