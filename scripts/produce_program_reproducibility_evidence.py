#!/usr/bin/env python3
"""Produce measured CORE-004F/004G evidence from bounded Seen builds."""

from __future__ import annotations

import argparse
import gzip
import hashlib
import io
import json
import os
import resource
import shutil
import stat
import subprocess
import sys
import tarfile
from pathlib import Path


EPOCH = 1_700_000_000
SEED = "1101"
MAX_CAPTURE = 16_777_216
ED25519_SPKI_PREFIX = bytes.fromhex("302a300506032b6570032100")
ED25519_SPKI_BYTES = len(ED25519_SPKI_PREFIX) + 32
CACHE_MODES = ("cold", "warm", "disabled")
PROGRAMS = (
    ("single-file", "default", "single-file/main.seen"),
    ("multi-module", "default", "multi-module/src/main.seen"),
    ("locked-package", "default", "locked-package/src/main.seen"),
    ("deterministic-runtime-context", "default", "runtime-context/main.seen"),
    ("default", "default", "single-file/main.seen"),
    ("release-full-lto", "release-full-lto", "single-file/main.seen"),
    ("release-thin-lto", "release-thin-lto", "single-file/main.seen"),
)
EXPECTED_STDOUT = {
    "single-file": b"core-004f-single\n",
    "multi-module": b"42\n",
    "locked-package": b"73\n",
    "deterministic-runtime-context": (
        b"PASS: CORE-004E_epoch_happy\n"
        b"PASS: CORE-004E_seed_happy\n"
        b"PASS: CORE-004E_missing_epoch\n"
        b"PASS: CORE-004E_invalid_seed\n"
        b"PASS: CORE-004E_env_denied\n"
        b"PASS: CORE-004E_locale_pinned\n"
        b"PASS: CORE-004E_external_input_denied\n"
        b"PASS: CORE-004E_run_parity\n"
        b"PASS: CORE-004E_cancel\n"
    ),
    "default": b"core-004f-single\n",
    "release-full-lto": b"core-004f-single\n",
    "release-thin-lto": b"core-004f-single\n",
}
EXPECTED_STDERR = b""


def die(message: str) -> None:
    raise SystemExit(f"CORE-004FG producer: {message}")


def canonical(value: object) -> bytes:
    return (json.dumps(value, separators=(",", ":"), sort_keys=True) + "\n").encode()


def sha_bytes(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def sha_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        while chunk := stream.read(1_048_576):
            digest.update(chunk)
    return digest.hexdigest()


def path_identity(path: Path) -> str:
    return sha_bytes(os.fsencode(str(path.resolve(strict=True))))


def safe_tree_digest(root: Path) -> str:
    digest = hashlib.sha256()
    for path in sorted(root.rglob("*"), key=lambda item: item.as_posix()):
        if path.is_symlink():
            continue
        if not path.is_file() or ".seen" in path.relative_to(root).parts:
            continue
        relative = path.relative_to(root).as_posix().encode()
        digest.update(len(relative).to_bytes(8, "big"))
        digest.update(relative)
        size = path.stat().st_size
        digest.update(size.to_bytes(8, "big"))
        with path.open("rb") as stream:
            while chunk := stream.read(1_048_576):
                digest.update(chunk)
    return digest.hexdigest()


def require_under(path: Path, root: Path, *, exists: bool = True) -> Path:
    root = root.resolve(strict=True)
    candidate = path.resolve(strict=exists)
    if candidate == root or root not in candidate.parents:
        die(f"path escaped its root: {candidate}")
    if exists and candidate.is_symlink():
        die(f"symbolic links are forbidden: {candidate}")
    return candidate


def write_json(path: Path, value: object) -> None:
    temporary = path.with_name(f".{path.name}.tmp")
    temporary.write_bytes(canonical(value))
    os.replace(temporary, path)


def annotate_main(path: Path) -> None:
    source = path.read_text(encoding="utf-8")
    marker = "fun main() r: Int {"
    if marker not in source:
        die(f"fixture has no canonical main function: {path}")
    if "@nondeterministic\n" + marker not in source:
        source = source.replace(marker, "@nondeterministic\n" + marker, 1)
    path.write_text(source, encoding="utf-8")


def ignore_generated(directory: str, names: list[str]) -> set[str]:
    ignored = {name for name in names if name in (".seen", ".seen_cache") or
               name.endswith((".o", ".sig", ".a"))}
    ignored.update(name for name in names if (Path(directory) / name).is_symlink())
    return ignored


def copy_corpus(repo: Path, destination: Path) -> None:
    source = repo / "tests/fixtures/core-004f/corpus"
    shutil.copytree(source, destination, ignore=ignore_generated)
    context_source = repo / "tests/fixtures/core-004e/context_contract.seen"
    shutil.copy2(context_source, destination / "runtime-context/main.seen")
    for relative in (
        "single-file/main.seen",
        "multi-module/src/main.seen",
        "locked-package/src/main.seen",
        "runtime-context/main.seen",
    ):
        annotate_main(destination / relative)


def archive_members(repo: Path, compiler: Path) -> list[tuple[str, Path]]:
    members = [("bin/seen", compiler)]
    for base, patterns in (
        (repo / "seen_runtime", ("*.c", "*.h")),
        (repo / "seen_std/src", ("*.seen",)),
    ):
        for pattern in patterns:
            for path in sorted(base.rglob(pattern)):
                if path.is_file() and not path.is_symlink():
                    members.append((path.relative_to(repo).as_posix(), path))
    members.append(("seen_std/Seen.toml", repo / "seen_std/Seen.toml"))
    return sorted(members)


def make_installed_archive(repo: Path, compiler: Path, output: Path) -> None:
    manifest = {
        "compiler_sha256": sha_file(compiler),
        "runtime_sha256": safe_tree_digest(repo / "seen_runtime"),
        "schema": "seen-program-installed-input-v1",
        "source_date_epoch": EPOCH,
        "stdlib_sha256": safe_tree_digest(repo / "seen_std/src"),
    }
    raw = io.BytesIO()
    with tarfile.open(fileobj=raw, mode="w", format=tarfile.GNU_FORMAT) as archive:
        for name, source in archive_members(repo, compiler):
            info = archive.gettarinfo(str(source), arcname=name)
            info.uid = info.gid = 0
            info.uname = info.gname = ""
            info.mtime = EPOCH
            info.mode = 0o755 if name == "bin/seen" else 0o644
            with source.open("rb") as stream:
                archive.addfile(info, stream)
        payload = canonical(manifest)
        info = tarfile.TarInfo("manifest.json")
        info.size = len(payload)
        info.mode = 0o644
        info.mtime = EPOCH
        archive.addfile(info, io.BytesIO(payload))
    with output.open("wb") as stream:
        with gzip.GzipFile(filename="", mode="wb", fileobj=stream, mtime=EPOCH, compresslevel=9) as zipped:
            zipped.write(raw.getvalue())


def prepare(args: argparse.Namespace) -> None:
    repo = Path(args.repo).resolve(strict=True)
    session = require_under(Path(args.session), repo / ".seen", exists=False)
    if session.exists():
        if session.is_symlink() or not session.is_dir() or any(session.iterdir()):
            die("existing session is not an empty safe directory")
    else:
        session.mkdir(parents=True)
    compiler = Path(args.compiler).resolve(strict=True)
    if not compiler.is_file() or compiler.is_symlink() or compiler.stat().st_mode & 0o111 == 0:
        die("compiler must be a non-symlink executable")
    for suffix in ("a", "b"):
        copy_corpus(repo, session / f"source-{suffix}")
        builder = session / f"builder-{suffix}"
        builder.mkdir()
        shutil.copy2(compiler, builder / "seen")
        (session / f"source-{suffix}/.seen_cache").mkdir()
    installed = session / "installed-b"
    (installed / "bin").mkdir(parents=True)
    (installed / "lib/seen").mkdir(parents=True)
    shutil.copy2(compiler, installed / "bin/seen")
    package_client = repo / "tools/seen-pkg/bin/seen-pkg"
    if package_client.is_file():
        shutil.copy2(package_client, installed / "bin/seen-pkg")
    compatibility = repo / "releases/compatibility-manifest.json"
    if compatibility.is_file():
        shutil.copy2(compatibility, installed / "bin/compatibility-manifest.json")
    shutil.copytree(repo / "seen_std/src", installed / "lib/seen/std",
                    ignore=ignore_generated)
    shutil.copytree(repo / "seen_runtime", installed / "lib/seen/runtime",
                    ignore=ignore_generated)
    for path in sorted(installed.rglob("*"), reverse=True):
        path.chmod(path.stat().st_mode & ~0o222)
    installed.chmod(installed.stat().st_mode & ~0o222)
    archive = session / "seen-runtime.tar.gz"
    make_installed_archive(repo, compiler, archive)
    for suffix in ("a", "b"):
        shutil.copy2(archive, session / f"builder-{suffix}/seen-runtime.tar.gz")
    pins = {}
    for builder_id in ("builder-a", "builder-b"):
        private_key = session / f"{builder_id}.private.pem"
        public_key = session / f"{builder_id}.public.pem"
        openssl(["genpkey", "-algorithm", "ED25519", "-out", str(private_key)])
        private_key.chmod(0o600)
        openssl(["pkey", "-in", str(private_key), "-pubout", "-out", str(public_key)])
        public_key.chmod(0o644)
        pins[builder_id] = {
            "identity": ed25519_public_identity(public_key),
            "issuer": "seenlang-local-release-gate",
            "public_key": public_key.name,
        }
    if pins["builder-a"]["identity"] == pins["builder-b"]["identity"]:
        die("independent builders received the same signing identity")
    write_json(session / "trust-pins.json", {
        **pins,
        "schema": "seen-program-trust-pins-v1",
    })
    print(f"prepared={session}")


def read_numeric(path: Path, name: str) -> int:
    raw = path.read_text(encoding="ascii").strip()
    if not raw.isdecimal():
        die(f"{name} readback is not numeric")
    return int(raw)


def containment() -> dict[str, int]:
    if os.environ.get("SEEN_CAPPED_REGRESSION_ACTIVE") != "1":
        die("worker is outside the capped regression boundary")
    if os.environ.get("SEEN_JOBS") != "1" or os.environ.get("SEEN_OPT_JOBS") != "1":
        die("worker settings are not serial")
    cgroup_line = next((line for line in Path("/proc/self/cgroup").read_text().splitlines()
                        if line.startswith("0::")), "")
    if not cgroup_line:
        die("unified cgroup identity is unavailable")
    cgroup = Path("/sys/fs/cgroup") / cgroup_line[3:].lstrip("/")
    memory = read_numeric(cgroup / "memory.max", "memory.max")
    swap = read_numeric(cgroup / "memory.swap.max", "memory.swap.max")
    pids = read_numeric(cgroup / "pids.max", "pids.max")
    tasks = read_numeric(cgroup / "pids.current", "pids.current")
    requested_memory = int(os.environ.get("SEEN_MEMORY_GUARD_RSS_KB", "0")) * 1024
    requested_pids = int(os.environ.get("SEEN_MEMORY_GUARD_TASKS_MAX", "0"))
    if (memory != requested_memory or memory > 68_719_476_736 or swap != 0 or
            pids != requested_pids or not 1 <= pids <= 24 or not 1 <= tasks <= pids):
        die("aggregate cgroup limits do not match their requested values")
    available_kib = 0
    for line in Path("/proc/meminfo").read_text().splitlines():
        if line.startswith("MemAvailable:"):
            available_kib = int(line.split()[1])
            break
    available_bytes = available_kib * 1024
    if available_bytes < memory:
        die("aggregate memory cap now exceeds live available memory")
    return {
        "jobs": 1,
        "memory_available_bytes": available_bytes,
        "memory_max_bytes": memory,
        "memory_max_readback_bytes": memory,
        "memory_swap_max_bytes": 0,
        "memory_swap_max_readback_bytes": swap,
        "opt_jobs": 1,
        "pids_max": pids,
        "pids_max_readback": pids,
        "tasks_current": tasks,
    }


def deterministic_environment() -> dict[str, str]:
    environment = dict(os.environ)
    environment.pop("SEEN_COMPILER_SOURCE_ROOT", None)
    environment.pop("SEEN_PACKAGE_CLIENT", None)
    environment.update({
        "CORE_004E_VISIBLE": "granted-value",
        "LANG": "C.UTF-8",
        "LC_ALL": "C.UTF-8",
        "SEEN_DETERMINISTIC": "1",
        "SEEN_DETERMINISTIC_SEED": SEED,
        "SEEN_HASH_SEED": SEED,
        "SOURCE_DATE_EPOCH": str(EPOCH),
        "TZ": "UTC",
    })
    return environment


def command_output(command: list[str]) -> str:
    result = subprocess.run(command, check=True, stdout=subprocess.PIPE,
                            stderr=subprocess.STDOUT, text=True, timeout=30)
    return " ".join(result.stdout.split())[:256]


def program_metadata(source_root: Path, compiler: Path, toolchain: str,
                     program_id: str, mode: str, relative: str) -> dict[str, object]:
    program_root = (source_root / relative).parent
    manifest_root = program_root
    while manifest_root != source_root and not (manifest_root / "Seen.toml").exists():
        manifest_root = manifest_root.parent
    if not (manifest_root / "Seen.toml").exists():
        manifest_root = program_root
    source_digest = safe_tree_digest(manifest_root)
    lock = manifest_root / "Seen.lock"
    lock_digest = sha_file(lock) if lock.exists() else sha_bytes(b"absent\n")
    build_input = {
        "compiler_sha256": sha_file(compiler),
        "environment": {
            "LANG": "C.UTF-8", "LC_ALL": "C.UTF-8",
            "SEEN_DETERMINISTIC_SEED": SEED, "SEEN_HASH_SEED": SEED,
            "SOURCE_DATE_EPOCH": str(EPOCH), "TZ": "UTC",
        },
        "flags": ["--deterministic", "--target=linux-x86_64",
                  "--target-cpu=x86-64", "--simd=none", "--no-fork"],
        "lock_digest": lock_digest,
        "mode": mode,
        "object_cache_abi": "seen-object-cache-abi-v3",
        "program": program_id,
        "source_digest": source_digest,
        "toolchain": toolchain,
    }
    return {
        "build_input_digest": sha_bytes(canonical(build_input)),
        "command": f"seen compile /seen/src/{program_id}/main.seen {program_id} --deterministic --target-cpu=x86-64 --simd=none",
        "lock_digest": lock_digest,
        "source_digest": source_digest,
    }


def bounded_run(executable: Path, environment: dict[str, str]) -> subprocess.CompletedProcess[bytes]:
    def limit_stack() -> None:
        resource.setrlimit(resource.RLIMIT_STACK, (8 * 1024 * 1024, 8 * 1024 * 1024))

    return subprocess.run([str(executable)], env=environment, stdout=subprocess.PIPE,
                          stderr=subprocess.PIPE, timeout=120, preexec_fn=limit_stack)


def compile_program(attested: Path, compiler: Path, source_root: Path,
                    source: Path, output: Path,
                    mode: str, cache_mode: str, environment: dict[str, str],
                    log: Path) -> None:
    command = ["bash", str(attested), str(compiler), "compile", str(source), str(output),
               "--deterministic", "--target=linux-x86_64", "--target-cpu=x86-64",
               "--simd=none", "--no-fork"]
    if mode == "release-full-lto":
        command.extend(("--release", "--lto=full"))
    elif mode == "release-thin-lto":
        command.extend(("--release", "--lto=thin"))
    if cache_mode == "disabled":
        command.append("--no-cache")
    if "locked-package" in source.parts or "multi-module" in source.parts:
        command.append("--frozen")
    with log.open("wb") as stream:
        result = subprocess.run(command, cwd=source_root, env=environment,
                                stdout=stream, stderr=subprocess.STDOUT, timeout=1200)
    if log.stat().st_size > MAX_CAPTURE:
        die(f"compiler log exceeded {MAX_CAPTURE} bytes: {log}")
    if result.returncode != 0 or not output.is_file() or output.stat().st_mode & 0o111 == 0:
        tail = log.read_text(errors="replace")[-8000:]
        die(f"compile failed for {source} ({cache_mode}):\n{tail}")
    if list(output.parent.glob(f"{output.name}.tmp.*")):
        die(f"atomic output scratch survived: {output}")


def worker(args: argparse.Namespace) -> None:
    repo = Path(args.repo).resolve(strict=True)
    session = require_under(Path(args.session), repo / ".seen")
    suffix = args.builder.removeprefix("builder-")
    if suffix not in ("a", "b") or args.builder != f"builder-{suffix}":
        die("builder identity must be builder-a or builder-b")
    source_root = require_under(session / f"source-{suffix}", session)
    builder_root = require_under(session / args.builder, session)
    cache_root = require_under(source_root / ".seen_cache", source_root)
    compiler = Path(args.compiler).resolve(strict=True)
    attested = Path(os.environ.get("SEEN_ATTESTED_COMPILER_RUNNER", "")).resolve(strict=True)
    limits = containment()
    environment = deterministic_environment()
    compiler_identity = command_output([str(compiler), "--version"])
    toolchain = command_output(["clang", "--version"])
    compiler_pin = builder_root / "seen"
    if sha_file(compiler_pin) != sha_file(compiler):
        die("staged compiler does not match the selected compiler")
    program_results = []
    for program_id, mode, relative in PROGRAMS:
        source = source_root / relative
        metadata = program_metadata(source_root, compiler, toolchain, program_id, mode, relative)
        runs = []
        program_cache = cache_root
        if program_cache.exists():
            shutil.rmtree(program_cache)
        program_cache.mkdir()
        for cache_mode in CACHE_MODES:
            if cache_mode == "cold":
                if program_cache.exists():
                    shutil.rmtree(program_cache)
            output = builder_root / f"{program_id}-{cache_mode}.bin"
            output.unlink(missing_ok=True)
            log = session / f"{args.builder}-{program_id}-{cache_mode}.compile.log"
            compile_program(attested, compiler, source_root, source, output,
                            mode, cache_mode, environment, log)
            execution = bounded_run(output, environment)
            if execution.returncode != 0:
                die(f"runtime did not succeed: {args.builder}/{program_id}/{cache_mode}")
            if len(execution.stdout) > MAX_CAPTURE or len(execution.stderr) > MAX_CAPTURE:
                die(f"runtime capture exceeded its bound: {program_id}")
            if execution.stdout != EXPECTED_STDOUT[program_id] or \
                    execution.stderr != EXPECTED_STDERR:
                die(f"runtime oracle mismatch: {args.builder}/{program_id}/{cache_mode}")
            stdout = builder_root / f"{program_id}-{cache_mode}.out"
            stderr = builder_root / f"{program_id}-{cache_mode}.err"
            stdout.write_bytes(execution.stdout)
            stderr.write_bytes(execution.stderr)
            runs.append({
                "artifact": output.name,
                "artifact_bytes": output.stat().st_size,
                "artifact_sha256": sha_file(output),
                "cache_mode": cache_mode,
                "exit_code": execution.returncode,
                "manifest": f"{program_id}-{cache_mode}.json",
                "stderr": stderr.name,
                "stderr_bytes": stderr.stat().st_size,
                "stderr_sha256": sha_file(stderr),
                "stdout": stdout.name,
                "stdout_bytes": stdout.stat().st_size,
                "stdout_sha256": sha_file(stdout),
            })
        comparison_fields = ("artifact_bytes", "artifact_sha256", "exit_code",
                             "stderr_bytes", "stderr_sha256", "stdout_bytes", "stdout_sha256")
        for run in runs[1:]:
            if any(run[field] != runs[0][field] for field in comparison_fields):
                die(f"cold/warm/disabled raw mismatch: {args.builder}/{program_id}")
        manifest = canonical({
            "artifact_bytes": runs[0]["artifact_bytes"],
            "artifact_sha256": runs[0]["artifact_sha256"],
            "build_input_digest": metadata["build_input_digest"],
            "mode": mode,
            "program": program_id,
            "schema": "seen-program-run-manifest-v1",
        })
        for run in runs:
            manifest_path = builder_root / str(run["manifest"])
            manifest_path.write_bytes(manifest)
            run["manifest_bytes"] = len(manifest)
            run["manifest_sha256"] = sha_bytes(manifest)
        program_results.append({
            **metadata,
            "id": program_id,
            "mode": mode,
            "runs": runs,
        })
    partial = {
        "build_root_digest": path_identity(builder_root),
        "builder_identity": sha_bytes(f"seen-program-{args.builder}\n".encode()),
        "cache_root_digest": path_identity(cache_root),
        "compiler": "seen",
        "compiler_identity": compiler_identity,
        "compiler_sha256": sha_file(compiler_pin),
        "containment": limits,
        "id": args.builder,
        "installed_archive": "seen-runtime.tar.gz",
        "installed_archive_sha256": sha_file(builder_root / "seen-runtime.tar.gz"),
        "programs": program_results,
        "toolchain": toolchain,
    }
    write_json(session / f"{args.builder}.partial.json", partial)
    signed = sign_builder(session, partial)
    write_json(session / f"{args.builder}.attested.json", signed)
    print(f"PASS: {args.builder} produced {len(PROGRAMS) * len(CACHE_MODES)} measured builds")


def builder_inventory(builder: dict[str, object]) -> bytes:
    unsigned = dict(builder)
    unsigned.pop("attestation", None)
    return canonical(unsigned)


def openssl(arguments: list[str]) -> bytes:
    try:
        result = subprocess.run(["openssl", *arguments], stdout=subprocess.PIPE,
                                stderr=subprocess.PIPE, timeout=30, check=False)
    except (FileNotFoundError, subprocess.TimeoutExpired) as error:
        die(f"OpenSSL Ed25519 operation is unavailable: {type(error).__name__}")
    if result.returncode != 0:
        detail = result.stderr.decode(errors="replace")[-2000:]
        die(f"OpenSSL Ed25519 operation failed: {detail}")
    return result.stdout


def ed25519_public_identity(public_key: Path) -> str:
    der = openssl(["pkey", "-pubin", "-in", str(public_key), "-outform", "DER"])
    if len(der) != ED25519_SPKI_BYTES or not der.startswith(ED25519_SPKI_PREFIX):
        die("generated builder public key is not Ed25519")
    return "ed25519-sha256:" + sha_bytes(der)


def sign_inventory(private_key: Path, inventory: bytes, inventory_path: Path,
                   signature_path: Path) -> bytes:
    inventory_path.write_bytes(inventory)
    openssl(["pkeyutl", "-sign", "-inkey", str(private_key), "-rawin",
             "-in", str(inventory_path), "-out", str(signature_path)])
    signature = signature_path.read_bytes()
    if len(signature) != 64:
        die("Ed25519 builder signature has the wrong size")
    return signature


def sign_builder(session: Path, partial: dict[str, object]) -> dict[str, object]:
    programs = []
    for program in partial["programs"]:
        programs.append({
            "build_input_digest": program["build_input_digest"],
            "id": program["id"],
            "runs": program["runs"],
        })
    builder = {key: value for key, value in partial.items() if key != "programs"}
    builder["programs"] = programs
    inventory_payload = builder_inventory(builder)
    inventory = sha_bytes(inventory_payload)
    private_key = session / f"{builder['id']}.private.pem"
    public_key = session / f"{builder['id']}.public.pem"
    signature_path = session / f"{builder['id']}.sig"
    trust = json.loads((session / "trust-pins.json").read_text())
    pin = trust.get(str(builder["id"]), {})
    if not private_key.is_file() or not public_key.is_file():
        die(f"pre-provisioned builder identity is unavailable: {builder['id']}")
    identity = ed25519_public_identity(public_key)
    if pin.get("identity") != identity or pin.get("public_key") != public_key.name:
        die(f"pre-provisioned builder trust pin changed: {builder['id']}")
    try:
        signature = sign_inventory(
            private_key, inventory_payload,
            session / f"{builder['id']}.inventory.json", signature_path,
        )
    finally:
        private_key.unlink(missing_ok=True)
    builder["attestation"] = {
        "identity": identity,
        "inventory_sha256": inventory,
        "issuer": pin.get("issuer", ""),
        "mode": "key",
        "signature_sha256": sha_bytes(signature),
    }
    return builder


def verify_builder_attestation(session: Path, partial: dict[str, object],
                               builder: dict[str, object]) -> None:
    expected = {key: value for key, value in partial.items() if key != "programs"}
    expected["programs"] = [{
        "build_input_digest": program["build_input_digest"],
        "id": program["id"],
        "runs": program["runs"],
    } for program in partial["programs"]]
    unsigned = dict(builder)
    attestation = unsigned.pop("attestation", None)
    if unsigned != expected or not isinstance(attestation, dict):
        die("builder attestation does not cover its measured partial")
    builder_id = str(builder.get("id", ""))
    trust = json.loads((session / "trust-pins.json").read_text())
    pin = trust.get(builder_id, {})
    public_key = session / str(pin.get("public_key", ""))
    signature = session / f"{builder_id}.sig"
    inventory = session / f"{builder_id}.inventory.json"
    payload = builder_inventory(builder)
    if inventory.read_bytes() != payload:
        die(f"builder inventory changed after signing: {builder_id}")
    if attestation.get("identity") != pin.get("identity") or \
            attestation.get("issuer") != pin.get("issuer") or \
            attestation.get("inventory_sha256") != sha_bytes(payload) or \
            attestation.get("signature_sha256") != sha_file(signature):
        die(f"builder attestation is not pinned: {builder_id}")
    openssl(["pkeyutl", "-verify", "-pubin", "-inkey", str(public_key),
             "-rawin", "-in", str(inventory), "-sigfile", str(signature)])


def finalize(args: argparse.Namespace) -> None:
    repo = Path(args.repo).resolve(strict=True)
    session = require_under(Path(args.session), repo / ".seen")
    partials = [json.loads((session / f"builder-{suffix}.partial.json").read_text())
                for suffix in ("a", "b")]
    builders = [json.loads((session / f"builder-{suffix}.attested.json").read_text())
                for suffix in ("a", "b")]
    if list(session.glob("*.private.pem")):
        die("finalizer refuses access to builder private keys")
    for partial, builder in zip(partials, builders):
        verify_builder_attestation(session, partial, builder)
    first, second = partials
    if first["compiler_sha256"] != second["compiler_sha256"] or \
            first["installed_archive_sha256"] != second["installed_archive_sha256"] or \
            first["toolchain"] != second["toolchain"]:
        die("builder tool inputs differ")
    descriptors = []
    for index, (program_id, mode, _) in enumerate(PROGRAMS):
        a = first["programs"][index]
        b = second["programs"][index]
        fields = ("artifact_bytes", "artifact_sha256", "exit_code", "manifest_bytes",
                  "manifest_sha256", "stderr_bytes", "stderr_sha256", "stdout_bytes",
                  "stdout_sha256")
        baseline = a["runs"][0]
        if a["build_input_digest"] != b["build_input_digest"]:
            die(f"cross-builder input identity mismatch: {program_id}")
        for program in (a, b):
            for run in program["runs"]:
                if any(run[field] != baseline[field] for field in fields):
                    die(f"cross-builder raw mismatch: {program_id}")
        descriptors.append({
            "build_input_digest": a["build_input_digest"],
            "command": a["command"],
            "expected_exit_code": baseline["exit_code"],
            "expected_stderr_sha256": baseline["stderr_sha256"],
            "expected_stdout_sha256": baseline["stdout_sha256"],
            "id": program_id,
            "lock_digest": a["lock_digest"],
            "mode": mode,
            "source_digest": a["source_digest"],
        })
    source_commit = subprocess.run(["git", "-C", str(repo), "rev-parse", "HEAD"],
                                   check=True, text=True, stdout=subprocess.PIPE).stdout.strip()
    evidence = {
        "build_input_digest": sha_bytes(canonical([item["build_input_digest"] for item in descriptors])),
        "builders": builders,
        "cancelled": False,
        "lock_digest": sha_bytes(canonical([item["lock_digest"] for item in descriptors])),
        "programs": descriptors,
        "schema": "seen-program-reproducibility-v1",
        "source_commit": source_commit,
        "source_date_epoch": EPOCH,
        "source_digest": sha_bytes(canonical([item["source_digest"] for item in descriptors])),
        "target": "linux-x86_64",
    }
    write_json(session / "program-reproducibility.json", evidence)

    source_program = first["programs"][0]
    env_pins = []
    for name, value in sorted({"LANG": "C.UTF-8", "LC_ALL": "C.UTF-8",
                               "SEEN_DETERMINISTIC_SEED": SEED,
                               "SEEN_HASH_SEED": SEED,
                               "SOURCE_DATE_EPOCH": str(EPOCH),
                               "TZ": "UTC"}.items()):
        env_pins.append({"name": name, "value_sha256": sha_bytes(value.encode())})
    runtime_digest = safe_tree_digest(repo / "seen_runtime")
    stdlib_digest = safe_tree_digest(repo / "seen_std/src")
    package_client = repo / "tools/seen-pkg/bin/seen-pkg"
    package_digest = sha_file(package_client) if package_client.is_file() else sha_bytes(b"absent\n")
    build_input = {
        "compiler_sha256": first["compiler_sha256"],
        "coverage": False,
        "cpu": "x86-64",
        "debug": False,
        "environment": env_pins,
        "features": [],
        "lock_sha256": source_program["lock_digest"],
        "lto": "none",
        "object_cache_abi": "seen-object-cache-abi-v3",
        "package_client_sha256": package_digest,
        "path_map": "/seen/src",
        "pic": False,
        "profile": "deterministic",
        "runtime_sha256": runtime_digest,
        "sanitizer": "none",
        "simd": "none",
        "source_date_epoch": EPOCH,
        "source_tree_sha256": source_program["source_digest"],
        "stdlib_sha256": stdlib_digest,
        "target": "linux-x86_64",
        "toolchain": first["toolchain"],
        "triple": "x86_64-unknown-linux-gnu",
    }
    build_input["input_digest"] = sha_bytes(canonical(build_input))
    result_ids = ("root-a-cold", "root-a-warm", "root-a-disabled",
                  "root-b-cold", "root-b-warm", "root-b-disabled")
    results = []
    for result_index, result_id in enumerate(result_ids):
        builder_index = 0 if result_index < 3 else 1
        run_index = result_index % 3
        run = partials[builder_index]["programs"][0]["runs"][run_index]
        result_root = session / "core-004f" / result_id
        result_root.mkdir(parents=True)
        source_artifact = session / f"builder-{'a' if builder_index == 0 else 'b'}" / run["artifact"]
        shutil.copy2(source_artifact, result_root / "program")
        artifacts = [{"bytes": run["artifact_bytes"], "executable": True,
                      "name": "program", "role": "executable",
                      "sha256": run["artifact_sha256"]}]
        artifact_set = sha_bytes(canonical(artifacts))
        cache_key = sha_bytes(b"seen-program-cache-v1\0two-root-default\0" +
                              str(build_input["input_digest"]).encode())
        results.append({
            "artifacts": artifacts,
            "build_input_digest": build_input["input_digest"],
            "cache": {"artifact_set_sha256": artifact_set,
                      "input_digest": build_input["input_digest"],
                      "key": cache_key,
                      "state": ("miss", "hit", "disabled")[run_index]},
            "cache_identity": partials[builder_index]["cache_root_digest"],
            "cache_mode": CACHE_MODES[run_index],
            "compiler_origin": "checkout" if builder_index == 0 else "installed",
            "exit_code": run["exit_code"],
            "id": result_id,
            "path_map": "/seen/src",
            "published_atomically": True,
            "root_identity": partials[builder_index]["build_root_digest"],
            "stderr_sha256": run["stderr_sha256"],
            "stdout_sha256": run["stdout_sha256"],
        })
    artifact_evidence = {"build_input": build_input, "cancelled": False,
                         "case": "two-root-default", "results": results,
                         "schema": "seen-program-artifacts-v1"}
    write_json(session / "program-artifacts.json", artifact_evidence)
    print(f"evidence={session / 'program-reproducibility.json'}")


def mutate_file(args: argparse.Namespace) -> None:
    path = Path(args.path).resolve(strict=True)
    payload = bytearray(path.read_bytes())
    if payload:
        payload[-1] ^= 1
    else:
        payload.append(1)
    path.write_bytes(payload)


def mutate_evidence(args: argparse.Namespace) -> None:
    source = Path(args.source).resolve(strict=True)
    output = Path(args.output).resolve(strict=False)
    value = json.loads(source.read_text())
    if args.kind == "core004f-input":
        value["build_input"]["source_tree_sha256"] = "0" * 64
    elif args.kind == "core004g-signature-pin":
        if not args.signature:
            die("signature-pin mutation requires --signature")
        value["builders"][1]["attestation"]["signature_sha256"] = sha_file(
            Path(args.signature).resolve(strict=True))
    elif args.kind == "core004g-root":
        builder = value["builders"][0]
        builder["build_root_digest"] = "0" * 64
        inventory_payload = builder_inventory(builder)
        inventory = sha_bytes(inventory_payload)
        signature_path = output.with_suffix(".sig")
        signature = sign_inventory(
            source.parent / "builder-a.private.pem", inventory_payload,
            output.with_suffix(".inventory.json"), signature_path,
        )
        builder["attestation"]["inventory_sha256"] = inventory
        builder["attestation"]["signature_sha256"] = sha_bytes(signature)
        print(f"signature={signature_path}")
    else:
        die("unknown mutation kind")
    write_json(output, value)


def print_trust(args: argparse.Namespace) -> None:
    session = Path(args.session).resolve(strict=True)
    trust = json.loads((session / "trust-pins.json").read_text())
    pin = trust.get(args.builder)
    if trust.get("schema") != "seen-program-trust-pins-v1" or not isinstance(pin, dict):
        die("trust pin document is invalid")
    for field in ("identity", "issuer", "public_key"):
        value = pin.get(field)
        if not isinstance(value, str) or not value or "\n" in value or "\r" in value:
            die(f"trust pin field is invalid: {field}")
        print(value)


def self_test() -> None:
    payload = {"b": 2, "a": 1}
    if canonical(payload) != b'{"a":1,"b":2}\n':
        die("canonical JSON self-test failed")
    if sha_bytes(b"") != "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855":
        die("SHA-256 self-test failed")
    print("PASS: CORE-004FG producer self-test")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    for name in ("prepare", "worker", "finalize"):
        child = subparsers.add_parser(name)
        child.add_argument("--repo", required=True)
        child.add_argument("--session", required=True)
        if name in ("prepare", "worker"):
            child.add_argument("--compiler", required=True)
        if name == "worker":
            child.add_argument("--builder", required=True)
    mutation = subparsers.add_parser("mutate-file")
    mutation.add_argument("--path", required=True)
    evidence = subparsers.add_parser("mutate-evidence")
    evidence.add_argument("--source", required=True)
    evidence.add_argument("--output", required=True)
    evidence.add_argument("--kind", required=True,
                          choices=("core004f-input", "core004g-root",
                                   "core004g-signature-pin"))
    evidence.add_argument("--signature")
    trust = subparsers.add_parser("print-trust")
    trust.add_argument("--session", required=True)
    trust.add_argument("--builder", required=True, choices=("builder-a", "builder-b"))
    subparsers.add_parser("self-test")
    args = parser.parse_args(argv)
    if args.command == "prepare":
        prepare(args)
    elif args.command == "worker":
        worker(args)
    elif args.command == "finalize":
        finalize(args)
    elif args.command == "mutate-file":
        mutate_file(args)
    elif args.command == "mutate-evidence":
        mutate_evidence(args)
    elif args.command == "print-trust":
        print_trust(args)
    else:
        self_test()
    return 0


if __name__ == "__main__":
    sys.exit(main())
