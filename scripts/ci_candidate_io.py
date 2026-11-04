"""Linux descriptor-relative, bounded I/O for the bootstrap candidate journal."""

from __future__ import annotations

import ctypes
import hashlib
import json
import os
import secrets
import stat
from contextlib import contextmanager
from pathlib import Path

from release_toolchain_artifact import ArtifactError, no_duplicates

JSON_MAX = 64 * 1024


class InstallationCleanupError(ArtifactError):
    """The replacements committed, but cleanup failed; certification is absent."""


def fail(message: str) -> None:
    raise ArtifactError(f"bootstrap-candidate: {message}")


def absolute(path: Path) -> Path:
    if not path.is_absolute() or ".." in path.parts:
        fail(f"path must be absolute without traversal: {path}")
    return path


def snapshot(info: os.stat_result) -> tuple[int, ...]:
    return (info.st_dev, info.st_ino, info.st_mode, info.st_nlink,
            info.st_uid, info.st_gid, info.st_size, info.st_mtime_ns, info.st_ctime_ns)


@contextmanager
def directory(path: Path, create: bool = False):
    """Pin every parent without following links; never resolve an unsafe path away."""
    path = absolute(path)
    fd = os.open("/", os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC)
    try:
        for part in path.parts[1:]:
            if create:
                try:
                    os.mkdir(part, mode=0o755, dir_fd=fd)
                except FileExistsError:
                    pass
            child = os.open(part, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC,
                            dir_fd=fd)
            os.close(fd)
            fd = child
        yield fd
    finally:
        os.close(fd)


def unchanged_directory(path: Path, fd: int) -> None:
    with directory(path) as current:
        before, after = os.fstat(fd), os.fstat(current)
        if (before.st_dev, before.st_ino) != (after.st_dev, after.st_ino):
            fail(f"directory identity changed: {path}")


def entries(fd: int, expected: set[str]) -> None:
    # Stop after the first unexpected entry: do not materialize arbitrary directories.
    found = set()
    with os.scandir(fd) as source:
        for entry in source:
            if entry.name not in expected or entry.name in found:
                fail(f"unexpected directory entry: {entry.name}")
            found.add(entry.name)
    if found != expected:
        fail("required directory entries are missing")


def read_at(fd: int, name: str, maximum: int, executable: bool = False,
            virtual: bool = False) -> bytes:
    if not name or name in {".", ".."} or "/" in name:
        fail("invalid file name")
    file_fd = os.open(name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC,
                      dir_fd=fd)
    try:
        before = os.fstat(file_fd)
        if not stat.S_ISREG(before.st_mode) or before.st_nlink != 1:
            fail(f"file is not a single regular non-symlink file: {name}")
        mode = stat.S_IMODE(before.st_mode)
        if mode & 0o7000 or (executable and not mode & 0o111):
            fail(f"unsafe or non-executable mode: {name}")
        if (not virtual and before.st_size < 1) or before.st_size > maximum:
            fail(f"file size is invalid: {name}")
        chunks = bytearray()
        while len(chunks) <= maximum:
            chunk = os.read(file_fd, min(1024 * 1024, maximum + 1 - len(chunks)))
            if not chunk:
                break
            chunks.extend(chunk)
        if not 1 <= len(chunks) <= maximum:
            fail(f"file size is invalid: {name}")
        if snapshot(before) != snapshot(os.fstat(file_fd)):
            fail(f"file changed while reading: {name}")
        if snapshot(before) != snapshot(os.stat(name, dir_fd=fd, follow_symlinks=False)):
            fail(f"file identity changed while reading: {name}")
        return bytes(chunks)
    finally:
        os.close(file_fd)


def read_file(path: Path, maximum: int, executable: bool = False,
              virtual: bool = False) -> bytes:
    with directory(path.parent) as fd:
        data = read_at(fd, path.name, maximum, executable, virtual)
        unchanged_directory(path.parent, fd)
        return data


def metadata(data: bytes, mode: int = 0o755) -> dict:
    return {"bytes": len(data), "mode": mode, "sha256": hashlib.sha256(data).hexdigest()}


def encode(value: dict) -> bytes:
    data = (json.dumps(value, separators=(",", ":"), sort_keys=True) + "\n").encode()
    if len(data) > JSON_MAX:
        fail("JSON exceeds size limit")
    return data


def decode(data: bytes) -> dict:
    try:
        value = json.loads(data.decode("utf-8"), object_pairs_hook=no_duplicates,
                           parse_constant=lambda _: fail("nonfinite JSON number"))
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        fail(f"invalid JSON: {error}")
    if not isinstance(value, dict):
        fail("JSON must be an object")
    return value


def write_at(fd: int, name: str, data: bytes, mode: int) -> None:
    file_fd = os.open(name, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC,
                      mode, dir_fd=fd)
    try:
        with os.fdopen(file_fd, "wb", closefd=False) as sink:
            sink.write(data)
            sink.flush()
            os.fchmod(file_fd, mode)
            os.fsync(file_fd)
    finally:
        os.close(file_fd)


def rename_new(fd: int, source: str, destination: str) -> None:
    """Atomic non-overwrite publication (Linux, like the required CI containment)."""
    libc = ctypes.CDLL(None, use_errno=True)
    rename = libc.renameat2
    rename.argtypes = [ctypes.c_int, ctypes.c_char_p, ctypes.c_int, ctypes.c_char_p, ctypes.c_uint]
    rename.restype = ctypes.c_int
    if rename(fd, os.fsencode(source), fd, os.fsencode(destination), 1) != 0:
        code = ctypes.get_errno()
        raise OSError(code, os.strerror(code), destination)


def publish_directory(path: Path, files: dict[str, tuple[bytes, int]]) -> None:
    with directory(path.parent, create=True) as parent:
        temporary = f".bootstrap-candidate.{secrets.token_hex(12)}"
        os.mkdir(temporary, mode=0o700, dir_fd=parent)
        try:
            child = os.open(temporary, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=parent)
            try:
                for name, (data, mode) in files.items():
                    write_at(child, name, data, mode)
                os.fchmod(child, 0o755)
                os.fsync(child)
            finally:
                os.close(child)
            unchanged_directory(path.parent, parent)
            rename_new(parent, temporary, path.name)
            os.fsync(parent)
        finally:
            # Only our exact, small temporary directory; never recursively remove inputs.
            try:
                child = os.open(temporary, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW,
                                dir_fd=parent)
            except FileNotFoundError:
                child = None
            if child is not None:
                try:
                    for name in files:
                        try:
                            os.unlink(name, dir_fd=child)
                        except FileNotFoundError:
                            pass
                finally:
                    os.close(child)
                os.rmdir(temporary, dir_fd=parent)


def publish_json(path: Path, value: dict) -> None:
    with directory(path.parent, create=True) as parent:
        temporary = f".receipt.{secrets.token_hex(12)}"
        try:
            write_at(parent, temporary, encode(value), 0o644)
            unchanged_directory(path.parent, parent)
            rename_new(parent, temporary, path.name)
            os.fsync(parent)
        finally:
            try:
                os.unlink(temporary, dir_fd=parent)
            except FileNotFoundError:
                pass

def invalidate_stamp(root: Path) -> None:
    path = root / "target/seen-build/full-release.stamp"
    with directory(path.parent, create=True) as parent:
        try:
            original = os.stat(path.name, dir_fd=parent, follow_symlinks=False)
        except FileNotFoundError:
            return
        if not stat.S_ISREG(original.st_mode) or original.st_nlink != 1:
            fail("obsolete full release stamp is unsafe")
        # Move the name before inspecting/removing it; a racing substitution cannot
        # make us unlink a different stamp after a check on the original inode.
        temporary = f".obsolete-full-stamp.{secrets.token_hex(12)}"
        unchanged_directory(path.parent, parent)
        rename_new(parent, path.name, temporary)
        moved = os.stat(temporary, dir_fd=parent, follow_symlinks=False)
        if (original.st_dev, original.st_ino) != (moved.st_dev, moved.st_ino):
            rename_new(parent, temporary, path.name)
            fail("full release stamp changed during invalidation")
        os.unlink(temporary, dir_fd=parent)
        os.fsync(parent)



def install_files(root: Path, binaries: dict[str, bytes]) -> None:
    """Atomically replace each file; roll back failures before the commit boundary.

    There is no cross-directory atomic transaction or crash certification.
    After every replacement is synced, cleanup errors report a committed but
    uncertified installation instead of pretending the old files were restored.
    Original hard-link backups retain ownership and permissions until commit.
    """
    from contextlib import ExitStack

    token = secrets.token_hex(12)
    staged = []
    replaced = []
    preserve_backups = False
    committed = False
    pending_error = None
    with ExitStack() as stack:
        try:
            for relative, data in binaries.items():
                path = root / relative
                parent = stack.enter_context(directory(path.parent, create=True))
                temporary = f".candidate-install.{token}.{path.name}"
                backup = f".candidate-backup.{token}.{path.name}"
                old = None
                try:
                    old = os.stat(path.name, dir_fd=parent, follow_symlinks=False)
                except FileNotFoundError:
                    pass
                staged.append((path, parent, temporary, backup, old))
                if old is not None:
                    if not stat.S_ISREG(old.st_mode) or old.st_nlink != 1:
                        fail(f"unsafe installation destination: {relative}")
                    os.link(path.name, backup, src_dir_fd=parent, dst_dir_fd=parent,
                            follow_symlinks=False)
                    linked = os.stat(backup, dir_fd=parent, follow_symlinks=False)
                    if (old.st_dev, old.st_ino) != (linked.st_dev, linked.st_ino):
                        fail("installation destination changed during backup")
                write_at(parent, temporary, data, 0o755)
            for item in staged:
                path, parent, temporary, backup, old = item
                unchanged_directory(path.parent, parent)
                try:
                    current = os.stat(path.name, dir_fd=parent, follow_symlinks=False)
                except FileNotFoundError:
                    current = None
                installed = os.stat(temporary, dir_fd=parent, follow_symlinks=False)
                if old is None:
                    if current is not None:
                        fail("installation destination appeared")
                    rename_new(parent, temporary, path.name)
                else:
                    if current is None or (current.st_dev, current.st_ino) != (old.st_dev, old.st_ino):
                        fail("installation destination changed")
                    os.replace(temporary, path.name, src_dir_fd=parent, dst_dir_fd=parent)
                replaced.append((item, installed))
                os.fsync(parent)
            # Backups remain available until all replacement syncs succeed.
            committed = True
        except BaseException as error:
            pending_error = error
            preserve_backups = True
            try:
                for item, installed in reversed(replaced):
                    path, parent, temporary, backup, old = item
                    current = os.stat(path.name, dir_fd=parent, follow_symlinks=False)
                    if (current.st_dev, current.st_ino) != (installed.st_dev, installed.st_ino):
                        fail("cannot restore installation: published destination changed")
                    if old is None:
                        os.unlink(path.name, dir_fd=parent)
                    else:
                        os.replace(backup, path.name, src_dir_fd=parent, dst_dir_fd=parent)
                    os.fsync(parent)
            except BaseException as rollback_error:
                pending_error = rollback_error
                rollback_error.add_note(f"Original installation error: {error}")
                raise
            preserve_backups = False
            raise
        finally:
            cleanup_error = None
            for path, parent, temporary, backup, old in staged:
                for name in ((temporary,) if preserve_backups else (temporary, backup)):
                    try:
                        os.unlink(name, dir_fd=parent)
                    except FileNotFoundError:
                        pass
                    except OSError as error:
                        cleanup_error = cleanup_error or error
                try:
                    os.fsync(parent)
                except OSError as error:
                    cleanup_error = cleanup_error or error
            if cleanup_error is not None:
                if pending_error is not None:
                    pending_error.add_note(f"Installation cleanup also failed: {cleanup_error}")
                elif committed:
                    raise InstallationCleanupError(
                        f"bootstrap-candidate: installation committed but cleanup failed; "
                        f"no certification written: {cleanup_error}") from cleanup_error
                else:
                    raise cleanup_error
