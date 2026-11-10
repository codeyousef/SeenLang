#!/usr/bin/env python3
"""Exercise refusal semantics against B's real, already-built native fixture pool.

This contract never invokes the Seen compiler or builds/runs a native fixture.
Clone may probe the native SDK's identity. The two accepted clone controls
establish that the same CLI/context works before and after the adverse cases;
every rejection uses a separate, initially empty consumer.
"""

import argparse
from contextlib import contextmanager
import errno
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys
import tempfile


PROFILES = ("fast", "release", "undefined", "address")
PROGRAMS = ("text", "integrity")
REJECTED = 126


class ContractFailure(RuntimeError):
    pass


def require(condition, message):
    if not condition:
        raise ContractFailure(message)


def regular(path):
    return stat.S_ISREG(path.lstat().st_mode)


def link_or_copy(source, destination):
    """Preserve real bytes without duplicating the eight native blobs on disk."""
    try:
        os.link(source, destination, follow_symlinks=False)
    except OSError as error:
        if error.errno != errno.EXDEV:
            raise
        shutil.copy2(source, destination, follow_symlinks=False)
    return destination


class NativePoolContract:
    def __init__(self, root, compiler, pool, scratch):
        self.root = root
        self.compiler = compiler
        self.pool = pool
        self.scratch = scratch
        self.source = root / "tests/gpu_artifacts/text_layout_contract.seen"
        self.cli = root / "scripts/emitter_fixture_pool.py"
        self.environment = os.environ.copy()
        self.cases = 0

    def clone(self, name, *, pool=None, compiler=None, source=None,
              profile="fast", environment=None, accepted=False):
        consumer = self.scratch / name
        consumer.mkdir()
        destination = consumer / "fixture"
        command = [
            sys.executable, str(self.cli), "clone",
            "--root", str(self.root),
            "--compiler", str(self.compiler if compiler is None else compiler),
            "--pool", str(self.pool if pool is None else pool),
            "--source", str(self.source if source is None else source),
            "--profile", profile,
            "--destination", str(destination),
        ]
        log = consumer / "clone.output"
        with log.open("wb") as output:
            completed = subprocess.run(
                command, env=self.environment if environment is None else environment,
                stdout=output, stderr=subprocess.STDOUT, timeout=120, check=False,
            )
        with log.open("rb") as output:
            diagnostic = output.read(4096).decode("utf-8", errors="replace")
        expected = 0 if accepted else REJECTED
        require(completed.returncode == expected,
                f"{name}: expected exit {expected}, got {completed.returncode}: {diagnostic}")
        if accepted:
            require(regular(destination) and os.access(destination, os.X_OK),
                    f"{name}: accepted clone did not deliver a native executable")
            for suffix in (".compile.log", ".log", ".compile.status", ".compile.origin.json"):
                require(regular(Path(str(destination) + suffix)),
                        f"{name}: accepted clone lacks regular {suffix}")
            require(Path(str(destination) + ".compile.status").read_bytes().strip() == b"0",
                    f"{name}: accepted clone did not retain the actual successful status")
        else:
            require(not os.path.lexists(destination),
                    f"{name}: refusal left a consumer executable")
            require(not os.path.lexists(str(destination) + ".compile.origin.json"),
                    f"{name}: refusal published successful reuse provenance")
            for artifact in consumer.iterdir():
                if artifact.name.endswith(".status") and artifact.is_file():
                    require(artifact.read_bytes().strip() != b"0",
                            f"{name}: refusal published successful status {artifact.name}")
                if artifact != log and artifact.is_file():
                    require(not (artifact.stat().st_mode & 0o111),
                            f"{name}: refusal left executable artifact {artifact.name}")
            self.cases += 1
        print(f"PASS {name}: clone exit {completed.returncode}", flush=True)
        # Accepted deliveries can be hardlinks to producer blobs. Remove them
        # before any producer mutation, not merely at whole-contract shutdown.
        shutil.rmtree(consumer)

    @contextmanager
    def displaced(self, name, path):
        """Restore the original inode, bytes and permissions even on failure."""
        require(regular(path), f"{name}: producer artifact is not a regular file: {path}")
        saved = self.scratch / ("original-" + name)
        os.replace(path, saved)
        try:
            yield saved
        finally:
            os.replace(saved, path)

    def damage(self, name, path):
        with self.displaced(name, path) as saved:
            shutil.copy2(saved, path)
            # Never write through a pool/consumer hardlink to the original inode.
            path.chmod(stat.S_IMODE(path.stat().st_mode) | stat.S_IWUSR)
            with path.open("r+b") as artifact:
                size = artifact.seek(0, os.SEEK_END)
                artifact.seek(size // 2)
                original = artifact.read(1)
                artifact.seek(size // 2)
                artifact.write(bytes([original[0] ^ 1]) if original else b"\x01")
            self.clone(name)

    def run(self):
        # Fail loudly on an incomplete input rather than allowing every negative
        # case to pass merely because the producer never made a usable pool.
        for program in PROGRAMS:
            for profile in PROFILES:
                prefix = self.pool / f"{program}-{profile}"
                require(regular(prefix) and os.access(prefix, os.X_OK),
                        f"missing actual native producer artifact: {prefix}")
                status = Path(str(prefix) + ".compile.status")
                require(regular(status) and status.read_bytes().strip() == b"0",
                        f"producer did not actually succeed: {prefix}")
        self.clone("accepted-before", accepted=True)

        stale_environment = self.environment.copy()
        stale_environment["FIXTURE_TARGET_CPU"] = (
            "x86-64-v3" if stale_environment.get("FIXTURE_TARGET_CPU") != "x86-64-v3"
            else "x86-64"
        )
        self.clone("stale-target-cpu", environment=stale_environment)

        # Select a different canonical path to the same genuine compiler bytes;
        # no mock compiler or Seen compiler invocation is involved.
        other_compiler = self.scratch / "other-selected-compiler"
        shutil.copy2(self.compiler, other_compiler)
        self.clone("changed-selected-compiler", compiler=other_compiler)
        other_compiler.unlink()

        self.clone("unsupported-profile", profile="debug")
        other_source = self.scratch / "other-selected-source.seen"
        shutil.copy2(self.source, other_source)
        self.clone("unsupported-source", source=other_source)
        other_source.unlink()

        empty_pool = self.scratch / "unsealed-pool"
        empty_pool.mkdir()
        self.clone("unsealed-pool-rejected", pool=empty_pool)
        empty_pool.rmdir()

        # Copy the actual pool, including its real manifest and statuses. All
        # bytes are genuine, but the new directory has a different owner binding.
        foreign_pool = self.scratch / "cross-owned-pool"
        shutil.copytree(self.pool, foreign_pool, copy_function=link_or_copy, symlinks=True)
        try:
            self.clone("cross-owned-pool-rejected", pool=foreign_pool)
        finally:
            shutil.rmtree(foreign_pool)

        # Request text-fast while an unrelated slot is absent. Validation must
        # establish completeness of the sealed eight-program pool, not just the
        # requested program.
        unrelated = self.pool / "integrity-address"
        with self.displaced("missing-unrequested-native", unrelated):
            self.clone("missing-unrequested-native")
        with self.displaced("missing-unrequested-status",
                            Path(str(unrelated) + ".compile.status")):
            self.clone("missing-unrequested-status")

        selected = self.pool / "text-fast"
        self.damage("damaged-native", selected)
        self.damage("damaged-compile-log", Path(str(selected) + ".compile.log"))

        # Even genuine bytes cannot authorize a linked metadata/native path.
        for name, path in (("symlink-native", selected),
                           ("symlink-manifest", self.pool / "manifest.json")):
            with self.displaced(name, path) as saved:
                path.symlink_to(saved)
                self.clone(name)

        status = Path(str(selected) + ".compile.status")
        for name, contents in (("failed-native-status", b"1\n"),
                               ("malformed-native-status", b"not-a-status\n")):
            with self.displaced(name, status):
                status.write_bytes(contents)
                self.clone(name)

        manifest = self.pool / "manifest.json"
        with self.displaced("malformed-manifest", manifest):
            manifest.write_bytes(b"{\n")
            self.clone("malformed-manifest")

        # This also detects failed restoration or accidental mutation of shared
        # native/log/status inodes. No malformed native program is ever executed.
        self.clone("accepted-after", accepted=True)
        print(f"PASS native fixture pool adverse contract: {self.cases} observed refusals",
              flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", required=True, type=Path)
    parser.add_argument("--compiler", required=True, type=Path)
    parser.add_argument("--pool", required=True, type=Path)
    args = parser.parse_args()
    try:
        artifact_value = os.environ.get("SEEN_ARTIFACT_ROOT")
        require(bool(artifact_value), "SEEN_ARTIFACT_ROOT must identify the current B artifact scope")
        artifact_root = Path(artifact_value).resolve(strict=True)
        root = args.root.resolve(strict=True)
        compiler = args.compiler.resolve(strict=True)
        pool = args.pool.resolve(strict=True)
        require(artifact_root.is_dir(), "SEEN_ARTIFACT_ROOT is not a directory")
        require(pool != artifact_root and artifact_root in pool.parents,
                "producer pool must be below the current SEEN_ARTIFACT_ROOT")
        require(regular(compiler), "selected compiler is not a regular file")
        with tempfile.TemporaryDirectory(prefix="emitter-pool-contract.",
                                         dir=artifact_root) as temporary:
            NativePoolContract(root, compiler, pool, Path(temporary)).run()
    except (ContractFailure, OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"FAIL native fixture pool adverse contract: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
