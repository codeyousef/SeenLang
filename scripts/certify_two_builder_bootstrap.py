#!/usr/bin/env python3
"""Create canonical CORE-004A evidence from two independent builder outputs."""
import argparse, hashlib, importlib.util, json, os, stat, struct, sys, tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("checker", ROOT / "scripts/check_bootstrap_reproducibility.py")
checker = importlib.util.module_from_spec(SPEC); SPEC.loader.exec_module(checker)


def open_regular(path, maximum):
    try:
        descriptor = os.open(path, os.O_RDONLY | os.O_CLOEXEC | os.O_NOFOLLOW)
    except OSError:
        checker.fail("invalid", "builder or artifact cannot be opened safely")
    metadata = os.fstat(descriptor)
    if not stat.S_ISREG(metadata.st_mode) or not 1 <= metadata.st_size <= maximum:
        os.close(descriptor); checker.fail("limit", "builder or artifact size is invalid")
    return descriptor, metadata.st_size


def sha256_fd(descriptor):
    digest = hashlib.sha256(); total = 0
    with os.fdopen(descriptor, "rb") as stream:
        while True:
            chunk = stream.read(1_048_576)
            if not chunk: break
            digest.update(chunk); total += len(chunk)
    return digest.hexdigest(), total


def elf64_x86_64_v1(payload):
    """Normalize only GNU build-id bytes and .rela.dyn entry ordering."""
    if len(payload) < 64 or payload[:6] != b"\x7fELF\x02\x01" or payload[18:20] != b"\x3e\x00":
        checker.fail("invalid", "ELF normalization requires little-endian ELF64 x86-64")
    section_offset = struct.unpack_from("<Q", payload, 40)[0]
    section_size, section_count, names_index = struct.unpack_from("<HHH", payload, 58)
    if section_size != 64 or not 1 <= section_count <= 4096 or names_index >= section_count:
        checker.fail("invalid", "ELF section table is invalid")
    table_end = section_offset + section_size * section_count
    if section_offset < 64 or table_end > len(payload): checker.fail("invalid", "ELF section table is out of bounds")
    def section(index):
        start = section_offset + index * section_size
        return struct.unpack_from("<IIQQQQIIQQ", payload, start)
    names = section(names_index)
    names_offset, names_size = names[4], names[5]
    if names_offset + names_size > len(payload): checker.fail("invalid", "ELF section names are out of bounds")
    names_data = payload[names_offset:names_offset + names_size]
    normalized = bytearray(payload); found_note = found_relocations = False
    for index in range(section_count):
        header = section(index); name_offset, section_type = header[0], header[1]
        data_offset, data_size, entry_size = header[4], header[5], header[9]
        if name_offset >= len(names_data): checker.fail("invalid", "ELF section name is invalid")
        name_end = names_data.find(b"\0", name_offset)
        if name_end < 0: checker.fail("invalid", "ELF section name is unterminated")
        name = names_data[name_offset:name_end]
        if data_offset + data_size > len(payload): checker.fail("invalid", "ELF section data is out of bounds")
        if name == b".note.gnu.build-id":
            if found_note or section_type != 7 or data_size < 16: checker.fail("invalid", "GNU build-id section is invalid")
            namesz, descsz, note_type = struct.unpack_from("<III", payload, data_offset)
            descriptor_offset = data_offset + 12 + (namesz + 3) // 4 * 4
            if namesz != 4 or payload[data_offset + 12:data_offset + 16] != b"GNU\0" or note_type != 3 or descriptor_offset + descsz > data_offset + data_size:
                checker.fail("invalid", "GNU build-id note is invalid")
            normalized[descriptor_offset:descriptor_offset + descsz] = b"\0" * descsz; found_note = True
        elif name == b".rela.dyn":
            if found_relocations or section_type != 4 or entry_size != 24 or data_size % entry_size:
                checker.fail("invalid", "dynamic relocation section is invalid")
            entries = [payload[position:position + entry_size]
                       for position in range(data_offset, data_offset + data_size, entry_size)]
            offsets = [struct.unpack_from("<Q", entry)[0] for entry in entries]
            if len(offsets) != len(set(offsets)): checker.fail("invalid", "dynamic relocation offsets are not unique")
            normalized[data_offset:data_offset + data_size] = b"".join(sorted(entries)); found_relocations = True
    if not found_note or not found_relocations: checker.fail("invalid", "required ELF normalization sections are absent")
    return bytes(normalized)


def artifact_hashes(path, normalization):
    descriptor, size = open_regular(path, checker.MAX_ARTIFACT_BYTES)
    with os.fdopen(descriptor, "rb") as stream: payload = stream.read()
    if len(payload) != size: checker.fail("io", "artifact changed while reading")
    raw = hashlib.sha256(payload).hexdigest()
    if normalization == "none": return raw, raw, size
    if normalization == "elf64-x86_64-v1":
        return raw, hashlib.sha256(elf64_x86_64_v1(payload)).hexdigest(), size
    checker.fail("invalid", "unsupported artifact normalization")


def atomic_write(path, payload, cancelled=False):
    parent = path.parent
    if parent.is_symlink() or not parent.is_dir(): checker.fail("invalid", "output parent is unsafe")
    if path.is_symlink(): checker.fail("invalid", "output path is a symlink")
    descriptor, temporary = tempfile.mkstemp(prefix=".two-builder-", dir=parent)
    try:
        with os.fdopen(descriptor, "wb") as stream:
            stream.write(payload); stream.flush(); os.fsync(stream.fileno())
        if cancelled: checker.fail("cancelled", "bootstrap certification was cancelled")
        os.replace(temporary, path)
        directory = os.open(parent, os.O_RDONLY | os.O_DIRECTORY)
        try: os.fsync(directory)
        finally: os.close(directory)
    finally:
        try: os.unlink(temporary)
        except FileNotFoundError: pass


def build(args):
    artifact_a_sha, normalized_a_sha, read_a = artifact_hashes(args.artifact_a, args.normalization)
    artifact_b_sha, normalized_b_sha, read_b = artifact_hashes(args.artifact_b, args.normalization)
    builder_a_fd, _ = open_regular(args.builder_a, checker.MAX_ARTIFACT_BYTES)
    builder_b_fd, _ = open_regular(args.builder_b, checker.MAX_ARTIFACT_BYTES)
    builder_a_sha, _ = sha256_fd(builder_a_fd); builder_b_sha, _ = sha256_fd(builder_b_fd)
    value = {
        "builders": [
            {"artifact_bytes": read_a, "build_root_digest": args.build_root_digest_a,
             "builder_sha256": builder_a_sha, "id": "builder-a",
             "normalized_artifact_sha256": normalized_a_sha,
             "raw_artifact_sha256": artifact_a_sha, "toolchain": args.toolchain_a},
            {"artifact_bytes": read_b, "build_root_digest": args.build_root_digest_b,
             "builder_sha256": builder_b_sha, "id": "builder-b",
             "normalized_artifact_sha256": normalized_b_sha,
             "raw_artifact_sha256": artifact_b_sha, "toolchain": args.toolchain_b},
        ],
        "command": args.command,
        "limits": {"jobs": args.jobs, "memory_max_bytes": args.memory_max_bytes,
                   "memory_swap_max_bytes": args.memory_swap_max_bytes,
                   "opt_jobs": args.opt_jobs, "pids_max": args.pids_max},
        "normalization": args.normalization, "schema": checker.SCHEMA,
        "source_commit": args.source_commit, "source_date_epoch": args.source_date_epoch,
        "source_digest": args.source_digest, "target": "linux-x86_64",
    }
    payload = checker.canonical(value)
    atomic_write(args.output, payload, args.test_cancel_before_commit)
    return value


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--artifact-a", type=Path, required=True); parser.add_argument("--artifact-b", type=Path, required=True)
    parser.add_argument("--builder-a", type=Path, required=True); parser.add_argument("--builder-b", type=Path, required=True)
    parser.add_argument("--build-root-digest-a", required=True); parser.add_argument("--build-root-digest-b", required=True)
    parser.add_argument("--source-commit", required=True); parser.add_argument("--source-digest", required=True)
    parser.add_argument("--source-date-epoch", type=int, required=True)
    parser.add_argument("--toolchain-a", required=True); parser.add_argument("--toolchain-b", required=True)
    parser.add_argument("--command", required=True); parser.add_argument("--memory-max-bytes", type=int, required=True)
    parser.add_argument("--normalization", choices=sorted(checker.NORMALIZATIONS), default="none")
    parser.add_argument("--memory-swap-max-bytes", type=int, required=True); parser.add_argument("--pids-max", type=int, required=True)
    parser.add_argument("--jobs", type=int, required=True); parser.add_argument("--opt-jobs", type=int, required=True)
    parser.add_argument("--output", type=Path, required=True); parser.add_argument("--test-cancel-before-commit", action="store_true")
    args = parser.parse_args(argv)
    try:
        build(args); print(f"PASS: two-builder compiler artifact is byte-identical: {args.output}")
    except checker.ContractError as error:
        print(f"core.004a.{error.code}: {error}", file=sys.stderr); return 130 if error.code == "cancelled" else 1
    except OSError as error: print(f"core.004a.io: {type(error).__name__}", file=sys.stderr); return 1
    return 0


if __name__ == "__main__": sys.exit(main())
