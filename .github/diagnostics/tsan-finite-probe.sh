#!/usr/bin/env bash
# SDK-only experiment. No production source/IR changes or certification output.
set -euo pipefail
ROOT_DIR=$(git rev-parse --show-toplevel)
INPUTS=${1:?immutable diagnostic inputs directory required}
SCOPE=m0-tsan-finite-sdk-experiment
export SEEN_MEMORY_GUARD_METRICS_FILE="$ROOT_DIR/.seen/hardware-checks/tsan-finite-guard-metrics.txt"
if [ "${SEEN_CAPPED_PLATFORM_REGRESSION_ACTIVE:-0}" != 1 ]; then
    exec bash "$ROOT_DIR/scripts/run_capped_regression.sh" --platform "$SCOPE" -- bash "$0" "$INPUTS"
fi
bash "$ROOT_DIR/scripts/run_capped_regression.sh" --verify-platform-active "$SCOPE"
ulimit -c 0
mkdir -p -- "$ROOT_DIR/.seen/toolchains"
BASE=$(mktemp -d "$ROOT_DIR/.seen/toolchains/tsan-finite-probe.XXXXXX")
PROOF=$(mktemp -d "$ROOT_DIR/.seen/hardware-checks/tsan-finite-proof.XXXXXX")
WORK=$(mktemp -d "${SEEN_ARTIFACT_ROOT:?}/tsan-finite-control.XXXXXX")
python3 - "$BASE" "$PROOF" <<'PY'
import hashlib,json,pathlib,subprocess,sys,tarfile,urllib.request
base,proof=map(pathlib.Path,sys.argv[1:])
home=base/'gpg';home.mkdir(mode=0o700)
with urllib.request.urlopen('https://releases.llvm.org/release-keys.asc',timeout=60) as response:
 keys=response.read(65537)
if len(keys)>65536:raise SystemExit('Official key set exceeds bounded input ceiling')
(base/'release-keys.asc').write_bytes(keys)
subprocess.run(['gpg','--no-options','--homedir',str(home),'--batch','--import',str(base/'release-keys.asc')],check=True,timeout=30)
assets=[('compiler-rt-20.1.2.src.tar.xz',2603340,'eb4bec5583e28f69ae35715bce25f60a67a146286cefefe1f46e74e1aee6aba4'),('cmake-20.1.2.src.tar.xz',8648,'8a48d5ff59a078b7a94395b34f7d5a769f435a3211886e2c8bf83aa2981631bc'),('llvm-20.1.2.src.tar.xz',72707784,'6286c526db3b84ce79292f80118e7e6d3fbd5b5ce3e4a0ebb32b2d205233bd86')]
rows=[]
for filename,size,digest in assets:
 url='https://github.com/llvm/llvm-project/releases/download/llvmorg-20.1.2/'+filename
 with urllib.request.urlopen(url,timeout=90) as response:data=response.read(size+1)
 if len(data)!=size or hashlib.sha256(data).hexdigest()!=digest:raise SystemExit('Official source digest/size mismatch')
 archive_path=base/filename;archive_path.write_bytes(data)
 with urllib.request.urlopen(url+'.sig',timeout=60) as response:signature=response.read(65537)
 if len(signature)>65536:raise SystemExit('Signature exceeds bounded input ceiling')
 signature_path=base/(filename+'.sig');signature_path.write_bytes(signature)
 checked=subprocess.run(['gpg','--no-options','--homedir',str(home),'--batch','--status-fd','1','--verify',str(signature_path),str(archive_path)],stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=30)
 if checked.returncode or b'[GNUPG:] VALIDSIG 474E22316ABF4785A88C6E8EA2C794A986419D8A ' not in checked.stdout:raise SystemExit('Official historical release signature failed verification')
 rows.append({'file':filename,'bytes':size,'sha256':digest,'signature_status':checked.stdout.decode()})
 with tarfile.open(archive_path) as archive:
  members=archive.getmembers()
  if len(members)>250000 or sum(member.size for member in members)>3*1024**3:raise SystemExit('Source archive exceeds checked finite extraction ceiling')
  if filename.startswith('llvm-'):
   members=[member for member in members if member.name.startswith('llvm-20.1.2.src/cmake/')]
  archive.extractall(base,members=members,filter='data')
subprocess.run(['gpgconf','--homedir',str(home),'--kill','all'],check=True,timeout=15)
(proof/'source-provenance.json').write_text(json.dumps({'prototype_only':True,'production_certified':False,'assets':rows,'llvm_source_extraction':'genuine CMake source subtree; installed LLVM20 development package supplies supported configuration and headers'},indent=2)+'\n')
PY
printf '%s  %s\n' 4f7622030b7db53e7287b8d98ca6d880a2d1980ef24a9432176f762eb163143f "$BASE/compiler-rt-20.1.2.src/lib/tsan/rtl/tsan_platform_linux.cpp" | sha256sum --check
patch --batch --forward --fuzz=0 -p1 -d "$BASE/compiler-rt-20.1.2.src" < "$INPUTS/tsan-finite-as.patch"
printf '%s  %s\n' 4664c2730341417c0e8105e1e97a92876ab77794ca22553179fa9eef9977361f "$BASE/compiler-rt-20.1.2.src/lib/tsan/rtl/tsan_platform_linux.cpp" | sha256sum --check
triple=$(clang -dumpmachine)
cmake -S "$BASE/compiler-rt-20.1.2.src" -B "$BASE/build" -G Ninja \
    -DCMAKE_BUILD_TYPE=Release -DCMAKE_EXPORT_COMPILE_COMMANDS=ON \
    -DCMAKE_C_COMPILER="$(command -v clang)" -DCMAKE_CXX_COMPILER="$(command -v clang++)" \
    -DCMAKE_ASM_COMPILER="$(command -v clang)" -DCMAKE_C_COMPILER_TARGET="$triple" -DCMAKE_CXX_COMPILER_TARGET="$triple" \
    -DLLVM_COMMON_CMAKE_UTILS="$BASE/cmake-20.1.2.src" \
    -DLLVM_CMAKE_DIR=/usr/lib/llvm-20/lib/cmake/llvm -DLLVM_MAIN_SRC_DIR="$BASE/llvm-20.1.2.src" \
    -DCOMPILER_RT_DEFAULT_TARGET_ONLY=ON -DCOMPILER_RT_BUILD_BUILTINS=OFF \
    -DCOMPILER_RT_BUILD_SANITIZERS=ON -DCOMPILER_RT_SANITIZERS_TO_BUILD=tsan \
    -DCOMPILER_RT_BUILD_XRAY=OFF -DCOMPILER_RT_BUILD_LIBFUZZER=OFF \
    -DCOMPILER_RT_BUILD_PROFILE=OFF -DCOMPILER_RT_BUILD_CTX_PROFILE=OFF \
    -DCOMPILER_RT_BUILD_MEMPROF=OFF -DCOMPILER_RT_BUILD_ORC=OFF \
    -DCOMPILER_RT_BUILD_GWP_ASAN=OFF -DCOMPILER_RT_INCLUDE_TESTS=OFF > "$PROOF/configure.log" 2>&1 || { cat "$PROOF/configure.log"; exit 1; }
timeout --foreground --kill-after=5s 540s cmake --build "$BASE/build" --parallel 1 --target tsan > "$PROOF/build.log" 2>&1 || { cat "$PROOF/build.log"; exit 1; }
resource="$BASE/resource"
mkdir -p "$resource"
cp --archive --reflink=auto "$(clang -print-resource-dir)/." "$resource/"
for filename in libclang_rt.tsan-x86_64.a libclang_rt.tsan-x86_64.a.syms libclang_rt.tsan_cxx-x86_64.a libclang_rt.tsan_cxx-x86_64.a.syms; do
    cp -- "$BASE/build/lib/linux/$filename" "$resource/lib/linux/$filename"
done
(set -x; clang -std=c11 -D_GNU_SOURCE -O1 -g -march=x86-64 -fno-omit-frame-pointer \
    -fsanitize=thread -resource-dir "$resource" -fPIE -pie -pthread \
    "$INPUTS/tsan-finite-control.c" -o "$WORK/control") > "$WORK/compile.log" 2>&1
cp -- "$WORK/compile.log" "$PROOF/control-compile.log"
sha256sum -- "$WORK/control" "$resource/lib/linux/libclang_rt.tsan-x86_64.a" > "$PROOF/native-identities.sha256"
mkdir -p "$PROOF/sdk"
for filename in libclang_rt.tsan-x86_64.a libclang_rt.tsan-x86_64.a.syms libclang_rt.tsan_cxx-x86_64.a libclang_rt.tsan_cxx-x86_64.a.syms; do
    cp -- "$BASE/build/lib/linux/$filename" "$PROOF/sdk/$filename"
done
for mode in clean race; do
    status=0
    timeout --foreground --kill-after=5s 30s bash "$ROOT_DIR/scripts/run_asan_in_hard_memory_scope.sh" \
        --sanitizer thread --target-root "$WORK" --compile-log "$WORK/compile.log" \
        -- "$WORK/control" "$mode" > "$PROOF/$mode.out" 2> "$PROOF/$mode.err" || status=$?
    printf '%s\n' "$status" > "$PROOF/$mode.status"
    printf 'SDK finite-address control mode=%s actual_exit=%s\n' "$mode" "$status"
    cat -- "$PROOF/$mode.out" "$PROOF/$mode.err"
done
python3 - "$WORK/control" "$PROOF" <<'PY'
import json,os,pathlib,resource,subprocess,sys
binary,proof=map(pathlib.Path,sys.argv[1:])
def limited():
 limit=64*1024**4
 resource.setrlimit(resource.RLIMIT_AS,(limit,limit))
 actual=resource.getrlimit(resource.RLIMIT_AS)
 if actual!=(limit,limit):os._exit(125)
 os.write(2,f'sdk-boundary actual-soft={actual[0]} actual-hard={actual[1]}\n'.encode())
result=subprocess.run([str(binary),'clean'],env=dict(os.environ,TSAN_OPTIONS='halt_on_error=1:exitcode=66:report_signal_unsafe=1'),preexec_fn=limited,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=30)
(proof/'insufficient-address-space.out').write_bytes(result.stdout)
(proof/'insufficient-address-space.err').write_bytes(result.stderr)
clean=int((proof/'clean.status').read_text());race=int((proof/'race.status').read_text())
positive_output=(proof/'clean.out').read_text();race_error=(proof/'race.err').read_text()
passed=clean==0 and race!=0 and 'ThreadSanitizer: data race' in race_error and 'soft=140737488355328 hard=140737488355328 aslr_disabled=0' in positive_output and result.returncode!=0 and b'finite address-space limit must cover' in result.stderr
group_path=next(line[3:] for line in pathlib.Path('/proc/self/cgroup').read_text().splitlines() if line.startswith('0::'))
group=pathlib.Path('/sys/fs/cgroup'+group_path)
kernel={name:(group/name).read_text().strip() for name in ['memory.max','memory.peak','memory.events','pids.max','pids.peak','pids.events','cgroup.procs']}
processes=[]
for pid in kernel['cgroup.procs'].splitlines():
 try:processes.append({'pid':int(pid),'comm':pathlib.Path('/proc',pid,'comm').read_text().strip()})
 except FileNotFoundError:pass
packet={'prototype_only':True,'production_certified':False,'sdk_controls_passed':passed,'clean_exit':clean,'race_exit':race,'insufficient_limit_exit':result.returncode,'insufficient_limit_stdout':result.stdout.decode(errors='replace'),'insufficient_limit_stderr':result.stderr.decode(errors='replace'),'kernel':kernel,'remaining_scope_processes':processes}
(proof/'observations.json').write_text(json.dumps(packet,indent=2)+'\n')
print(json.dumps(packet,indent=2),flush=True)
PY
printf 'SDK-only observations retained: %s; individual controls, not this script status, determine proof.\n' "$PROOF"
