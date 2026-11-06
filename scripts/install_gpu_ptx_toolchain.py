#!/usr/bin/env python3
"""Install only the pinned offline assembler from its verified wheel."""
import argparse
import hashlib
import io
import os
from pathlib import Path
import tempfile
import urllib.request
import zipfile
from check_gpu_ptx_toolchain import pinned_profile, require, verify_installation


def install(destination):
    profile = pinned_profile()
    require(destination.is_absolute() and destination.parent.is_dir(), 'installation needs an absolute existing parent')
    require(not destination.is_symlink(), 'assembler destination must not be a symlink')
    with urllib.request.urlopen(profile['package_url'], timeout=120) as response:
        data = response.read(48 * 1024**2 + 1)
    require(len(data) <= 48 * 1024**2 and hashlib.sha256(data).hexdigest() == profile['package_sha256'],
            'assembler package digest or size differs')
    with zipfile.ZipFile(io.BytesIO(data)) as archive:
        info = archive.getinfo(profile['package_member'])
        require(not info.is_dir() and info.file_size <= 64 * 1024**2, 'invalid assembler member')
        binary = archive.read(info)
    require(hashlib.sha256(binary).hexdigest() == profile['tools']['ptxas']['sha256'], 'assembler member digest differs')
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(dir=destination.parent, prefix='.ptxas-', delete=False) as output:
            temporary = Path(output.name); output.write(binary)
        del binary, data
        temporary.chmod(0o755)
        checked = {**profile, 'tools': {'ptxas': {**profile['tools']['ptxas'], 'path': str(temporary)}}}
        verify_installation(checked)
        os.replace(temporary, destination); temporary = None
    finally:
        if temporary is not None: temporary.unlink(missing_ok=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('destination', type=Path)
    try:
        install(parser.parse_args().destination)
    except (OSError, ValueError, KeyError, zipfile.BadZipFile) as error:
        raise SystemExit(str(error))
