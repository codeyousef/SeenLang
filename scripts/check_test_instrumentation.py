#!/usr/bin/env python3
"""Validate TEST-002A software-execution evidence without hardware promotion."""
import argparse, json, random, sys, time
from pathlib import Path

SCHEMA="seen-test-instrumentation-v1"; MAX_BYTES=65536
NAMES={"compiler-host","gpu-emitters","seen-modules","native-runtime","abi-shims"}
PROFILES={"coverage":"", "sanitizer-undefined":"undefined", "sanitizer-thread":"thread"}
class ContractError(ValueError):
    def __init__(self,code,message): super().__init__(message); self.code=code
def fail(code,message):
    raise ContractError(code,message)
def pairs(items):
    out={}
    for key,value in items:
        if key in out: fail("invalid",f"duplicate field: {key}")
        out[key]=value
    return out
def validate(raw,max_bytes=MAX_BYTES,cancelled=False):
    if not 1<=max_bytes<=MAX_BYTES or len(raw)>max_bytes: fail("limit","evidence byte limit exceeded")
    try: value=json.loads(raw.decode(),object_pairs_hook=pairs)
    except (UnicodeDecodeError,json.JSONDecodeError) as error: fail("invalid",f"invalid JSON: {type(error).__name__}")
    if cancelled: fail("cancelled","instrumentation execution cancelled")
    if not isinstance(value,dict) or set(value)!={"components","coverage","profile","sanitizer","schema","target"}: fail("invalid","invalid evidence fields")
    if value["schema"]!=SCHEMA or value["target"]!="linux-x86_64": fail("platform","unsupported evidence target")
    profile=value["profile"]
    if profile not in PROFILES or value["sanitizer"]!=PROFILES[profile] or value["coverage"]!=(profile=="coverage"): fail("invalid","profile and mode disagree")
    components=value["components"]
    if not isinstance(components,list) or len(components)!=5: fail("limit","component set is incomplete")
    seen=set()
    for component in components:
        if not isinstance(component,dict) or set(component)!={"compiled","hardware_executed","name","software_executed"}: fail("invalid","invalid component evidence")
        name=component["name"]
        if name not in NAMES or name in seen: fail("invalid","unknown or duplicate component")
        seen.add(name)
        if component["compiled"] is not True or component["software_executed"] is not True or component["hardware_executed"] is not False: fail("invalid","compile-only or hardware evidence cannot close software execution")
    return {**value,"components":sorted(components,key=lambda item:item["name"].encode())}
def canonical(value):
    return (json.dumps(validate(json.dumps(value).encode()),separators=(",",":"),sort_keys=True)+"\n").encode()
def read_json(path):
    try: return json.loads(path.read_text(),object_pairs_hook=pairs)
    except (OSError,UnicodeDecodeError,json.JSONDecodeError) as error: fail("io",f"invalid execution artifact: {type(error).__name__}")
def require_file(path,needle=None):
    try: raw=path.read_bytes()
    except OSError as error: fail("io",f"missing execution artifact: {type(error).__name__}")
    if not raw or (needle is not None and needle not in raw): fail("invalid",f"execution artifact is incomplete: {path.name}")
def require_source_coverage(path, required):
    try:
        with path.open("rb") as stream:
            raw=stream.read(64 * 1024 * 1024 + 1)
    except OSError as error:
        fail("io", f"missing coverage export: {type(error).__name__}")
    return validate_source_coverage(raw,required)

def validate_source_coverage(raw,required):
    if not 0 < len(raw) <= 64 * 1024 * 1024:
        fail("limit", "coverage export byte limit exceeded")
    try:
        value=json.loads(raw,object_pairs_hook=pairs)
    except (UnicodeError,ValueError,RecursionError) as error:
        fail("invalid", f"invalid coverage export: {type(error).__name__}")
    if not isinstance(value,dict) or value.get("type")!="llvm.coverage.json.export":
        fail("invalid", "coverage export type is invalid")
    data=value.get("data")
    if not isinstance(data,list) or not 1<=len(data)<=64:
        fail("limit", "coverage export data is incomplete")
    covered=[]; count=0
    for unit in data:
        if not isinstance(unit,dict) or not isinstance(unit.get("files"),list):
            fail("invalid", "coverage export files are invalid")
        for source in unit["files"]:
            count+=1
            if count>4096: fail("limit", "coverage file limit exceeded")
            if not isinstance(source,dict): fail("invalid", "coverage source is invalid")
            name=source.get("filename")
            if not isinstance(name,str): fail("invalid", "coverage source name is invalid")
            try: name_bytes=len(name.encode())
            except UnicodeError: fail("invalid", "coverage source name is not UTF-8")
            if not 1<=name_bytes<=4096:
                fail("invalid", "coverage source name is invalid")
            summary=source.get("summary")
            if not isinstance(summary,dict): fail("invalid", "coverage summary is invalid")
            lines=summary.get("lines")
            if not isinstance(lines,dict): fail("invalid", "coverage lines are invalid")
            total=lines.get("count"); hit=lines.get("covered")
            if type(total) is not int or type(hit) is not int or not 0<=hit<=total:
                fail("invalid", "coverage line counts are invalid")
            if hit: covered.append(name.replace("\\", "/"))
    for group in required:
        def matches(name):
            if group.endswith((".seen", ".c")): return name.endswith(group)
            return group in name and name.endswith(".seen")
        if not any(matches(name) for name in covered):
            fail("invalid", f"coverage export has no executed source for {group}")
    return value

def validate_gpu_probe(shader):
    if not 20<=len(shader)<=4*1024*1024 or len(shader)%4 or shader[:4]!=b"\x03\x02\x23\x07":
        fail("invalid", "GPU probe is not a bounded SPIR-V artifact")
    version=int.from_bytes(shader[4:8],"little")
    bound=int.from_bytes(shader[12:16],"little")
    if version not in (0x00010500,0x00010600) or not 0<bound<=1048576 or shader[16:20]!=b"\x00"*4:
        fail("invalid", "GPU probe SPIR-V header is invalid")

def derive_execution(args):
    compiler=read_json(args.compiler_build_report); gpu=read_json(args.gpu_build_report)
    expected={"abi_shims":"compile-only","compiler_host":"compile-only","native_runtime":"compile-only","seen_modules":"compile-only"}
    gpu_expected={**expected,"compiler_host":"source-only"}
    for value,components in ((compiler,expected),(gpu,gpu_expected)):
        if value.get("schema")!="seen-build-instrumentation-evidence-v1" or value.get("target")!="linux-x86_64" or value.get("components")!=components: fail("invalid","build evidence does not cover required software components")
        modes=value.get("modes",{})
        if modes.get("coverage") is not True or modes.get("sanitizer")!="undefined": fail("invalid","build evidence modes are incomplete")
    require_file(args.compiler_profdata); require_file(args.test_profdata)
    require_file(args.compiler_coverage,b"TOTAL")
    require_file(args.test_coverage,b"TOTAL")
    require_source_coverage(args.compiler_coverage_json,
        ("main_compiler.seen", "/codegen/gpu/", "seen_runtime.c"))
    require_source_coverage(args.test_coverage_json, ("test_002a_gpu_emitters.seen",))
    try:
        with args.gpu_artifact.open("rb") as stream: shader=stream.read(4 * 1024 * 1024 + 1)
    except OSError as error: fail("io", f"missing GPU artifact: {type(error).__name__}")
    validate_gpu_probe(shader)
    require_file(args.compiler_log,b"[OK] GPU artifact written")
    require_file(args.test_log,b"PASS: TEST-002A_gpu_emitters_software")
    return {"components":[{"compiled":True,"hardware_executed":False,"name":name,"software_executed":True} for name in sorted(NAMES)],"coverage":True,"profile":"coverage","sanitizer":"","schema":SCHEMA,"target":"linux-x86_64"}
def fuzz(raw,seconds,seed,validate_bytes=validate):
    rng=random.Random(seed); deadline=time.monotonic()+seconds; cases=rejected=0
    while time.monotonic()<deadline:
        changed=bytearray(raw)
        if changed: changed[rng.randrange(len(changed))]=rng.randrange(256)
        cases+=1
        try: validate_bytes(bytes(changed))
        except ContractError: rejected+=1
    return cases,rejected
def main(argv=None):
    parser=argparse.ArgumentParser(description=__doc__); parser.add_argument("--evidence",type=Path); parser.add_argument("--derive-execution",action="store_true"); parser.add_argument("--compiler-build-report",type=Path); parser.add_argument("--gpu-build-report",type=Path); parser.add_argument("--compiler-profdata",type=Path); parser.add_argument("--test-profdata",type=Path); parser.add_argument("--compiler-coverage",type=Path); parser.add_argument("--test-coverage",type=Path); parser.add_argument("--compiler-coverage-json",type=Path); parser.add_argument("--test-coverage-json",type=Path); parser.add_argument("--gpu-artifact",type=Path); parser.add_argument("--compiler-log",type=Path); parser.add_argument("--test-log",type=Path); parser.add_argument("--max-bytes",type=int,default=MAX_BYTES); parser.add_argument("--fuzz-seconds",type=float,default=0); parser.add_argument("--seed",type=int,default=1101); parser.add_argument("--test-cancel-after-read",action="store_true"); args=parser.parse_args(argv)
    try:
        if not 0<=args.fuzz_seconds<=300: fail("limit","invalid fuzz duration")
        if args.derive_execution:
            required=(args.compiler_build_report,args.gpu_build_report,args.compiler_profdata,args.test_profdata,args.compiler_coverage,args.test_coverage,args.compiler_coverage_json,args.test_coverage_json,args.gpu_artifact,args.compiler_log,args.test_log)
            if any(path is None for path in required) or args.evidence is not None: fail("invalid","execution derivation arguments are incomplete")
            raw=canonical(derive_execution(args)); value=validate(raw,args.max_bytes,args.test_cancel_after_read)
        else:
            if args.evidence is None: fail("invalid","--evidence is required")
            raw=args.evidence.read_bytes(); value=validate(raw,args.max_bytes,args.test_cancel_after_read)
        if args.fuzz_seconds:
            cases,rejected=fuzz(raw,args.fuzz_seconds,args.seed); print(f"seed={args.seed} cases={cases} rejected={rejected}",file=sys.stderr)
            source=json.dumps({"type":"llvm.coverage.json.export","data":[{"files":[{"filename":"probe.seen","summary":{"lines":{"count":2,"covered":1}}}]}]}).encode()
            cases,rejected=fuzz(source,args.fuzz_seconds,args.seed,
                lambda changed:validate_source_coverage(changed,("probe.seen",)))
            print(f"source-coverage seed={args.seed} cases={cases} rejected={rejected}",file=sys.stderr)
            shader=b"\x03\x02\x23\x07\x00\x06\x01\x00"+b"\x00"*4+b"\x01\x00\x00\x00"+b"\x00"*4
            cases,rejected=fuzz(shader,args.fuzz_seconds,args.seed,validate_gpu_probe)
            print(f"gpu-probe seed={args.seed} cases={cases} rejected={rejected}",file=sys.stderr)
        sys.stdout.buffer.write((json.dumps(value,separators=(",",":"),sort_keys=True)+"\n").encode())
    except ContractError as error: print(f"test.002a.{error.code}: {error}",file=sys.stderr); return 130 if error.code=="cancelled" else 1
    except OSError as error: print(f"test.002a.io: {type(error).__name__}",file=sys.stderr); return 1
    return 0
if __name__=="__main__": sys.exit(main())
