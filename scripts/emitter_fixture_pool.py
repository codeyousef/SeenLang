#!/usr/bin/env python3
"""Gate B's private, same-run native fixture deliveries (not a build cache).

Only successful uncached generic-helper builds can be sealed. Publication is
atomic visibility, not power-loss durability. Consumer metadata is independent;
only read-only executable bytes are hardlinked inside the attested artifact root.
"""
import argparse
import hashlib
from itertools import islice
import json
import os
from pathlib import Path
import re
import resource
import shlex
import shutil
import stat
import sys
import subprocess
import tempfile

import accelerator_gate_evidence as evidence
from gpu_certification_io import read_bounded, require

PROFILES = ("fast", "release", "undefined", "address")
PROGRAMS = {"text": "tests/gpu_artifacts/text_layout_contract.seen",
            "integrity": "tests/gpu_artifacts/integrity_contract.seen"}
FLAGS = {"fast": ["--fast"], "release": ["--release", "--lto=thin"],
         "undefined": ["--fast", "--sanitize=undefined"],
         "address": ["--fast", "--sanitize=address"]}
MAX_DOCUMENT = 256 * 1024
MAX_BLOB = 128 * 1024**2
MAX_BLOBS = 512 * 1024**2
MAX_LOG = 8 * 1024**2
TOOLS = ("opt", "llvm-as", "llvm-link", "llc", "clang", "clang++", "cc", "c++",
         "gcc", "g++", "ld", "ld.lld", "lld", "wasm-ld", "glslc")
# These fields describe shell nesting/leaf scope, never compilation semantics.
SCOPE_ENV = {"SEEN_CAPPED_REGRESSION_SCOPE", "SHLVL", "_", "SEEN_EMITTER_FIXTURE_POOL"}
# runtimeObjectBuildSignature/runtimeObjectCachePath in main_compiler.seen
# own these derived objects. Their source/header/toolchain/profile inputs are
# bound here; later independent builds may replace the outputs after linking.
DERIVED_RUNTIME_FILES = {name + suffix for name in
                         ("seen_runtime", "seen_region", "seen_gpu", "seen_hotreload",
                          "seen_tee_common", "seen_tee_sgx", "seen_tee_sev")
                         for suffix in (".o", ".o.sig")}


def canonical(path, directory=False):
    path = Path(os.path.abspath(path))
    require(path.resolve(strict=True) == path, "noncanonical or symbolic-link path")
    entry = path.lstat()
    require(stat.S_ISDIR(entry.st_mode) if directory else stat.S_ISREG(entry.st_mode),
            "unexpected path type")
    return path


def digest(path, ceiling):
    data = read_bounded(path, ceiling)
    return {"sha256": hashlib.sha256(data).hexdigest(), "bytes": len(data)}


def encoded(value):
    return (json.dumps(value, sort_keys=True, separators=(",", ":"), allow_nan=False) + "\n").encode()


def document(path):
    return json.loads(read_bounded(path, MAX_DOCUMENT),
                      object_pairs_hook=evidence.unique_object,
                      parse_constant=evidence.invalid_constant)


def publish(path, data):
    require(path.suffix != ".json" or len(data) <= MAX_DOCUMENT, "document ceiling exceeded")
    require(not path.exists() and not path.is_symlink(), "output already exists")
    fd, name = tempfile.mkstemp(prefix=".emitter-", dir=path.parent)
    try:
        with os.fdopen(fd, "wb") as stream:
            stream.write(data)
        os.replace(name, path)
    finally:
        if os.path.exists(name):
            os.unlink(name)


def proc_bytes(name, ceiling):
    # procfs reports a zero stat size, so use a bounded streaming read.
    with open("/proc/self/" + name, "rb") as stream:
        data = stream.read(ceiling + 1)
    require(len(data) <= ceiling, "process context ceiling exceeded")
    return data


def directory_identity(path):
    path = canonical(path, directory=True)
    entry = path.stat()
    require(entry.st_uid == os.getuid(), "foreign directory owner")
    return {"path": str(path), "device": entry.st_dev, "inode": entry.st_ino,
            "uid": entry.st_uid}


def dependency_identity(root):
    """Include ignored native dependencies as well as tracked SDK source bytes.

    Cache/target directories are not inputs to --no-cache --frozen compilation.
    All standard-library source, native runtime files and language packs are.
    """
    catalog = {}
    remaining = 128 * 1024**2
    entries = 0
    for relative in ("compiler_seen/src", "seen_std/src", "seen_runtime", "languages"):
        base = canonical(root / relative, directory=True)
        for directory, directories, files in os.walk(base, followlinks=False):
            require(len(files) + len(directories) <= 8192, "dependency directory ceiling exceeded")
            entries += len(files) + len(directories)
            require(entries <= 16384, "dependency catalog entry ceiling exceeded")
            directories[:] = sorted(name for name in directories
                                    if name not in {".seen", ".seen_cache", "target", "__pycache__"})
            for name in directories:
                canonical(Path(directory) / name, directory=True)
            for name in sorted(files):
                path = canonical(Path(directory) / name)
                if path.parent == root / "seen_runtime" and path.name in DERIVED_RUNTIME_FILES:
                    continue
                require(len(catalog) < 8192, "dependency file-count ceiling exceeded")
                item = digest(path, min(32 * 1024**2, remaining))
                remaining -= item["bytes"]
                catalog[str(path.relative_to(root))] = item
    for relative in ("Seen.toml", "Seen.lock", "seen_std/Seen.toml", "seen_std/Seen.lock",
                     "seen_std/Seen.modules.lock", "compiler_seen/Seen.toml", "compiler_seen/Seen.lock"):
        catalog[relative] = digest(canonical(root / relative), 1024 * 1024)
    return digest_value(catalog)


def digest_value(value):
    return hashlib.sha256(encoded(value)).hexdigest()


def tool_identity():
    result = {}
    bounded = canonical(os.environ["SEEN_BOUNDED_TOOLCHAIN_DIR"], directory=True)
    for name in TOOLS:
        selected = shutil.which(name)
        if selected is None:
            result[name] = None
            continue
        path = Path(os.path.abspath(selected))
        # PATH tools commonly use vendor symlinks. Bind both selected spelling
        # and canonical real executable; generated bounded wrappers may not link.
        real = canonical(path.resolve(strict=True))
        item = {"selected": str(path), "real": str(real), **digest(real, 256 * 1024**2)}
        if path.parent == bounded:
            canonical(path)
            wrapper = read_bounded(path, 64 * 1024).decode()
            targets = set()
            for line in wrapper.splitlines():
                words = shlex.split(line)
                if words[:3] == ["exec", "-a", "$driver_argv0"]:
                    require(len(words) >= 4 and words[3].startswith("/"), "invalid bounded tool target")
                    targets.add(words[3])
            require(len(targets) == 1, "bounded wrapper must select one actual tool")
            target = canonical(Path(next(iter(targets))).resolve(strict=True))
            item["underlying"] = {"path": str(target), **digest(target, 256 * 1024**2)}
        result[name] = item
    require(result["clang"] is not None and result["opt"] is not None, "incomplete native toolchain")
    return result


def discovery(arguments):
    """Run only bound native tools, with finite time and output-file limits."""
    def output_limit():
        soft, hard = resource.getrlimit(resource.RLIMIT_FSIZE)
        limit = 256 * 1024 if soft == resource.RLIM_INFINITY else min(soft, 256 * 1024)
        resource.setrlimit(resource.RLIMIT_FSIZE, (limit, hard))

    with tempfile.TemporaryFile() as output, tempfile.TemporaryFile() as diagnostic:
        result = subprocess.run(arguments, input=b"", stdout=output, stderr=diagnostic,
                                timeout=30, check=False, preexec_fn=output_limit)
        require(output.tell() + diagnostic.tell() <= 256 * 1024, "SDK discovery output ceiling exceeded")
        output.seek(0)
        diagnostic.seek(0)
        stdout = output.read(256 * 1024 + 1).decode()
        stderr = diagnostic.read(256 * 1024 + 1).decode()
    require(len(stdout.encode()) + len(stderr.encode()) <= 256 * 1024,
            "SDK discovery output ceiling exceeded")
    return result.returncode, stdout, stderr


class SDKCatalog:
    """Bound aliases and unique regular-file bytes without copying SDK trees."""
    def __init__(self):
        self.catalog = hashlib.sha256()
        self.files = {}
        self.directories = set()
        self.bytes = 0
        self.entries = 0

    def add(self, requested, optional=False):
        requested = Path(os.path.abspath(requested))
        if optional and not requested.exists() and not requested.is_symlink():
            self.catalog.update(encoded({"missing": str(requested)}))
            return
        pending = [requested]
        while pending:
            path = pending.pop()
            self.entries += 1
            require(self.entries <= 200000, "SDK entry ceiling exceeded")
            real = path.resolve(strict=True)
            entry = real.lstat()
            self.catalog.update(encoded({"path": str(path), "real": str(real)}))
            if stat.S_ISDIR(entry.st_mode):
                if real in self.directories:
                    continue
                self.directories.add(real)
                with os.scandir(real) as scanner:
                    children = list(islice(scanner, 100001))
                require(len(children) <= 100000, "SDK directory ceiling exceeded")
                pending.extend(real / child.name for child in
                               sorted(children, key=lambda child: child.name, reverse=True))
                continue
            require(stat.S_ISREG(entry.st_mode), "SDK input is not a regular file")
            if real not in self.files:
                require(len(self.files) < 100000 and 0 <= entry.st_size <= 512 * 1024**2,
                        "SDK file-count or per-file ceiling exceeded")
                require(self.bytes + entry.st_size <= 4 * 1024**3, "SDK byte ceiling exceeded")
                checksum = hashlib.sha256()
                fd = os.open(real, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
                with os.fdopen(fd, "rb") as stream:
                    before = os.fstat(stream.fileno())
                    require((entry.st_dev, entry.st_ino) == (before.st_dev, before.st_ino),
                            "SDK input replaced during open")
                    size = 0
                    while True:
                        block = stream.read(1024 * 1024)
                        if not block:
                            break
                        size += len(block)
                        require(size <= entry.st_size, "SDK input grew during hashing")
                        checksum.update(block)
                    after = os.fstat(stream.fileno())
                identity = lambda value: (value.st_dev, value.st_ino, value.st_size,
                                          value.st_mtime_ns, value.st_ctime_ns)
                require(identity(entry) == identity(before) == identity(after) and size == entry.st_size,
                        "SDK input changed during hashing")
                self.files[real] = {"sha256": checksum.hexdigest(), "bytes": size}
                self.bytes += size
            self.catalog.update(encoded(self.files[real]))

    def result(self):
        return {"sha256": self.catalog.hexdigest(), "files": len(self.files),
                "directories": len(self.directories), "bytes": self.bytes}


def sdk_identity(tools):
    catalog = SDKCatalog()
    probes = hashlib.sha256()
    for tool, language in (("clang", "c"), ("clang++", "c++")):
        binding = tools[tool]
        require(binding is not None and "underlying" in binding, "SDK discovery needs bounded Clang wrappers")
        status, output, text = discovery([binding["selected"], "-E", "-x", language, "-", "-v"])
        require(status == 0, f"native include discovery failed: {text[:4096]}")
        probes.update(encoded({"stdout": output, "stderr": text}))
        inside = False
        roots = []
        for line in text.splitlines():
            if line in ('#include "..." search starts here:', "#include <...> search starts here:"):
                inside = True
            elif line == "End of search list.":
                inside = False
            elif inside:
                value = line.strip()
                require(value and not value.endswith("(framework directory)"), "unsupported native include context")
                roots.append(Path(value))
            elif line.startswith("Configuration file: "):
                catalog.add(Path(line.split(": ", 1)[1]))
        require(roots and not inside, "missing native include search catalog")
        for path in roots:
            catalog.add(path)
    clang = tools["clang"]["selected"]
    status, text, diagnostic = discovery([clang, "-print-resource-dir"])
    require(status == 0 and len(text.splitlines()) == 1,
            f"Clang resource discovery failed: {diagnostic[:4096]}")
    probes.update(encoded({"stdout": text, "stderr": diagnostic}))
    catalog.add(Path(text.strip()))
    required = ("crt1.o", "Scrt1.o", "crti.o", "crtn.o", "crtbegin.o", "crtbeginS.o",
                "crtend.o", "crtendS.o", "libgcc.a", "libgcc_s.so", "libc.so", "libc.so.6",
                "libc_nonshared.a", "libm.so", "libm.so.6", "libstdc++.so")
    optional = ("libgcc_eh.a", "libgcc_s.so.1", "libmvec.so.1", "libpthread.so.0",
                "libdl.so.2", "librt.so.1", "libstdc++.so.6", "libsupc++.a",
                "libunwind.so", "libunwind.a")
    support = []
    for name in (*required, *optional):
        status, text, diagnostic = discovery([clang, "-print-file-name=" + name])
        require(status == 0 and len(text.splitlines()) == 1,
                f"native support discovery failed: {name}: {diagnostic[:4096]}")
        probes.update(encoded({"stdout": text, "stderr": diagnostic}))
        value = text.strip()
        if value == name:
            require(name not in required, "missing required native startup/support artifact")
            continue
        path = Path(value)
        require(path.is_absolute(), "noncanonical native support selection")
        catalog.add(path)
        support.append(path.resolve(strict=True))
    # Empty include/library search components mean the actual caller cwd.
    for variable in ("CPATH", "C_INCLUDE_PATH", "CPLUS_INCLUDE_PATH", "OBJC_INCLUDE_PATH",
                     "LIBRARY_PATH", "LD_LIBRARY_PATH", "COMPILER_PATH", "GCC_EXEC_PREFIX"):
        if variable in os.environ:
            for value in os.environ[variable].split(":"):
                catalog.add(Path(value) if value else Path.cwd(), optional=True)
    catalog.add(Path("/etc/clang"), optional=True)
    ldd = canonical(Path("/usr/bin/ldd").resolve(strict=True))
    # User namespaces expose unmapped host-root ownership as overflowuid. Check
    # protection from this execution identity rather than invent a UID mapping.
    for protected in (ldd, *ldd.parents):
        require(protected.stat().st_mode & 0o022 == 0 and
                not os.access(protected, os.W_OK, effective_ids=True),
                "writable SDK dependency inspector or ancestor")
    catalog.add(ldd)
    selected = {Path(item.get("underlying", {"path": item["real"]})["path"])
                for item in tools.values() if item is not None}
    selected.update(support)
    dependency_diagnostics = {}
    for path in sorted(selected):
        with path.open("rb") as stream:
            header = stream.read(20)
        if header[:4] != b"\x7fELF":
            continue
        require(len(header) == 20 and header[4] in (1, 2) and header[5] in (1, 2),
                "unsupported native ELF header")
        elf_type = int.from_bytes(header[16:18], "little" if header[5] == 1 else "big")
        require(elf_type in (1, 2, 3), "unsupported native ELF object type")
        # ET_REL startup objects are hashed above but have no loader dependency
        # graph. Invoking ldd on them produces an expected non-executable warning.
        if elf_type == 1:
            continue
        status, text, diagnostic = discovery([str(ldd), str(path)])
        if diagnostic:
            require(len(diagnostic.encode()) <= 4096, "native dependency diagnostic ceiling exceeded")
            dependency_diagnostics[str(path)] = diagnostic
            require(len(encoded(dependency_diagnostics)) <= 32 * 1024,
                    "native dependency diagnostic catalog ceiling exceeded")
        combined = text + diagnostic
        if status != 0:
            require(status == 1 and combined.strip() in {"not a dynamic executable", "statically linked"},
                    f"native dependency discovery failed: {path} (exit {status}): {combined[:4096]}")
            continue
        require("not found" not in combined,
                f"missing native dependency: {path}: {combined[:4096]}")
        for line in text.splitlines():
            value = line.strip()
            if not value:
                continue
            if re.fullmatch(r"linux-vdso\.so\.[0-9]+ \(0x[0-9a-fA-F]+\)", value):
                continue
            match = re.fullmatch(r"(?:\S+ => )?(/[^\s]+) \(0x[0-9a-fA-F]+\)", value)
            require(match is not None, f"unsupported native dependency record: {path}: {value[:1024]}")
            catalog.add(Path(match.group(1)))
    return {**catalog.result(), "discovery_sha256": probes.hexdigest(),
            "dependency_diagnostics": dependency_diagnostics}


def context(args, pool):
    root = canonical(args.root, directory=True)
    require(root == evidence.ROOT, "pool root differs from evidence source root")
    compiler = canonical(args.compiler)
    artifact = canonical(os.environ["SEEN_ARTIFACT_ROOT"], directory=True)
    require(pool.is_relative_to(artifact) and pool != artifact, "pool escaped current artifact scope")
    require(pool.parent.name.startswith("p0-accel-b."), "pool is not B-owned")
    require(pool.stat().st_mode & 0o077 == 0, "pool must remain private to its owner")
    require(os.environ.get("SEEN_CAPPED_REGRESSION_ACTIVE") == "1", "inactive capped binding")
    require(os.environ.get("SEEN_CAPPED_REGRESSION_COMPILER") == str(compiler), "wrong compiler binding")
    environment = {key: value for key, value in os.environ.items() if key not in SCOPE_ENV}
    environment["PATH"] = ":".join(dict.fromkeys(environment.get("PATH", "").split(":")))
    require(len(encoded(environment)) <= MAX_DOCUMENT, "environment ceiling exceeded")
    sources = {name: {"path": str(canonical(root / relative)),
                      **digest(root / relative, 1024 * 1024)}
               for name, relative in PROGRAMS.items()}
    tools = tool_identity()
    sdk = sdk_identity(tools)
    return {"schema": "seen-emitter-fixture-context-v1", "pool": directory_identity(pool),
            "artifact_root": directory_identity(artifact), "root": str(root),
            "cwd": str(canonical(Path.cwd(), directory=True)),
            "compiler": {"path": str(compiler), **digest(compiler, 256 * 1024**2)},
            "source": evidence.source_identity(), "dependencies": dependency_identity(root),
            "environment_sha256": digest_value(environment), "tools": tools, "sdk": sdk,
            "sources": sources, "profiles": FLAGS, "target_cpu": "x86-64",
            "common_flags": ["--no-cache", "--frozen"],
            "cgroup": proc_bytes("cgroup", 64 * 1024).decode(),
            "mount_namespace": os.readlink("/proc/self/ns/mnt")}


def pool_files(pool):
    entries = list(islice(pool.iterdir(), 49))
    require(len(entries) <= 48, "pool file-count ceiling exceeded")
    return {entry.name for entry in entries}


def artifacts(pool, sealed=False):
    result = {}
    total = 0
    allowed = {"context.json", "manifest.json", "00-active.log"}
    for program in PROGRAMS:
        for profile in PROFILES:
            slot = program + "-" + profile
            allowed.update(slot + suffix for suffix in ("", ".compile.log", ".log", ".compile.status"))
            native = canonical(pool / slot)
            entry = native.stat()
            require(entry.st_uid == os.getuid() and entry.st_mode & 0o111,
                    "native blob ownership or executable mode mismatch")
            if sealed:
                require(entry.st_mode & 0o222 == 0, "sealed native blob is writable")
            status = read_bounded(pool / (slot + ".compile.status"), 16)
            require(status == b"0\n", "producer did not compile successfully")
            item = {suffix: digest(canonical(pool / (slot + suffix)), ceiling)
                    for suffix, ceiling in (("", MAX_BLOB), (".compile.log", MAX_LOG),
                                            (".log", MAX_LOG), (".compile.status", 16))}
            require(all((pool / (slot + suffix)).stat().st_uid == os.getuid() for suffix in item),
                    "foreign producer metadata owner")
            require(item[".compile.log"] == item[".log"], "producer logs disagree")
            total += item[""]["bytes"]
            require(total <= MAX_BLOBS, "native pool byte ceiling exceeded")
            result[slot] = item
    require(pool_files(pool) <= allowed, "unexpected pool artifacts")
    return result


def remove_ir(pool):
    # Only the eight fixed producer-owned directories, after actual status zero.
    # Refuse links/special files and bounded traversal before removing anything.
    directories = []
    count = 0
    for program in PROGRAMS:
        for profile in PROFILES:
            directory = pool / (program + "-" + profile + "-ir")
            if not directory.exists() and not directory.is_symlink():
                continue
            canonical(directory, directory=True)
            for current, children, files in os.walk(directory, followlinks=False):
                count += len(children) + len(files)
                require(count <= 16384, "producer IR file-count ceiling exceeded")
                for child in children:
                    canonical(Path(current) / child, directory=True)
                for name in files:
                    canonical(Path(current) / name)
            directories.append(directory)
    for directory in directories:
        shutil.rmtree(directory)


def clone(args, pool):
    destination = Path(os.path.abspath(args.destination))
    parent = canonical(destination.parent, directory=True)
    artifact = canonical(os.environ["SEEN_ARTIFACT_ROOT"], directory=True)
    require(parent.is_relative_to(artifact) and not parent.is_relative_to(pool), "unsafe consumer parent")
    require(destination.name not in {"", ".", ".."} and len(destination.name) <= 128,
            "unsafe consumer prefix")
    outputs = [destination.with_name(destination.name + suffix) for suffix in
               ("", ".compile.log", ".log", ".compile.status", ".compile.origin.json")]
    rejected = destination.with_name(destination.name + ".rejected.context.json")
    # Never leave an earlier successful consumer visible after rejection. Refuse
    # unsafe entries rather than following them, and clear status before bytes.
    for path in [outputs[3], *outputs[:3], outputs[4], rejected]:
        if path.exists() or path.is_symlink():
            require(stat.S_ISREG(path.lstat().st_mode), "unsafe existing consumer output")
            path.unlink()
    try:
        pool = canonical(pool, directory=True)
        manifest = document(pool / "manifest.json")
        require(type(manifest) is dict and set(manifest) == {"schema", "context", "artifacts"}
                and manifest["schema"] == "seen-emitter-fixture-pool-v1", "invalid sealed manifest")
        current = context(args, pool)
        expected = manifest["context"]
        baseline = document(pool / "context.json")
        require(type(expected) is dict and type(baseline) is dict, "invalid context document")
        if encoded(expected) != encoded(current) or encoded(baseline) != encoded(current):
            changed = [key for key in sorted(set(expected) | set(baseline) | set(current))
                       if encoded(expected.get(key)) != encoded(current.get(key)) or
                       encoded(baseline.get(key)) != encoded(current.get(key))]
            publish(rejected, encoded(current))
            raise ValueError("stale fixture context: " + ", ".join(changed)[:512])
        observed = artifacts(pool, sealed=True)
        require(encoded(manifest["artifacts"]) == encoded(observed), "damaged sealed producer artifacts")
        require(args.profile in PROFILES, "unsupported fixture profile")
        source = canonical(args.source)
        programs = [name for name, value in current["sources"].items() if value["path"] == str(source)]
        require(len(programs) == 1, "unsupported fixture source")
        slot = programs[0] + "-" + args.profile
        origin = {"schema": "seen-emitter-fixture-origin-v1", "origin": "same-run-reuse",
                  "pool": str(pool), "producer_prefix": slot,
                  "manifest_sha256": digest(pool / "manifest.json", MAX_DOCUMENT)["sha256"],
                  "source": current["sources"][programs[0]], "profile": args.profile,
                  "compiler": current["compiler"], "native": observed[slot][""],
                  "compilation_executed_here": False}
        os.link(pool / slot, destination, follow_symlinks=False)
        require(digest(destination, MAX_BLOB) == observed[slot][""] and
                destination.stat().st_mode & 0o222 == 0, "native blob changed during delivery")
        for suffix in (".compile.log", ".log"):
            data = read_bounded(pool / (slot + suffix), MAX_LOG)
            require({"sha256": hashlib.sha256(data).hexdigest(), "bytes": len(data)} ==
                    observed[slot][suffix], "producer metadata changed during delivery")
            publish(destination.with_name(destination.name + suffix), data)
        publish(outputs[4], encoded(origin))
        # A successful status is the last publication, never manufactured before
        # complete validation of the actual producer's zero status and bytes.
        publish(outputs[3], read_bounded(pool / (slot + ".compile.status"), 16))
    except BaseException:
        for path in outputs:
            if path.exists() and stat.S_ISREG(path.lstat().st_mode):
                path.unlink()
        raise


class PoolParser(argparse.ArgumentParser):
    def error(self, message):
        self.exit(126, "emitter fixture pool rejected: " + message + "\n")


def main():
    parser = PoolParser(description=__doc__)
    parser.add_argument("operation", choices=("fingerprint", "seal", "clone"))
    for name in ("root", "compiler", "pool"):
        parser.add_argument("--" + name, required=True, type=Path)
    parser.add_argument("--source", type=Path)
    parser.add_argument("--profile")
    parser.add_argument("--destination", type=Path)
    args = parser.parse_args()
    try:
        pool = Path(os.path.abspath(args.pool))
        if args.operation == "fingerprint":
            pool = canonical(pool, directory=True)
            require(not pool_files(pool), "initial pool must be empty")
            require(os.environ.get("SEEN_CAPPED_REGRESSION_SCOPE") == "seen-p0-accel-gate-b",
                    "only Gate B may provision fixtures")
            publish(pool / "context.json", encoded(context(args, pool)))
        elif args.operation == "seal":
            pool = canonical(pool, directory=True)
            initial = document(pool / "context.json")
            require(encoded(initial) == encoded(context(args, pool)), "producer context changed during compilation")
            # Check successes before discarding only their own module IR.
            for program in PROGRAMS:
                for profile in PROFILES:
                    require(read_bounded(pool / (program + "-" + profile + ".compile.status"), 16) == b"0\n",
                            "incomplete producer build")
            remove_ir(pool)
            observed = artifacts(pool)
            for slot in observed:
                path = pool / slot
                path.chmod(path.stat().st_mode & ~0o222)
            require(encoded(initial) == encoded(context(args, pool)), "producer context changed before publication")
            publish(pool / "manifest.json", encoded({"schema": "seen-emitter-fixture-pool-v1",
                    "context": initial, "artifacts": observed}))
        else:
            require(args.source is not None and args.profile is not None and args.destination is not None,
                    "clone requires source, profile and destination")
            clone(args, pool)
        return 0
    except (OSError, ValueError, KeyError, TypeError, UnicodeError,
            RecursionError, RuntimeError, subprocess.SubprocessError) as error:
        print("emitter fixture pool rejected: " + str(error), file=sys.stderr)
        return 126


if __name__ == "__main__":
    sys.exit(main())
