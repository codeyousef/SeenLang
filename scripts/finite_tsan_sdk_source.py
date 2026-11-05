"""Pinned LLVM source authentication; no binary SDK or diagnostic artifact trust."""
import hashlib
import os
from pathlib import Path, PurePosixPath
import subprocess
import tarfile
import tempfile
import urllib.request

SIGNER = '474E22316ABF4785A88C6E8EA2C794A986419D8A'
KEY_URL = 'https://releases.llvm.org/release-keys.asc'
RELEASE_URL = 'https://github.com/llvm/llvm-project/releases/download/llvmorg-20.1.2/'
ASSETS = {
    'compiler-rt-20.1.2.src.tar.xz': (2603340, 'eb4bec5583e28f69ae35715bce25f60a67a146286cefefe1f46e74e1aee6aba4'),
    'cmake-20.1.2.src.tar.xz': (8648, '8a48d5ff59a078b7a94395b34f7d5a769f435a3211886e2c8bf83aa2981631bc'),
    'llvm-20.1.2.src.tar.xz': (72707784, '6286c526db3b84ce79292f80118e7e6d3fbd5b5ce3e4a0ebb32b2d205233bd86'),
}
SIGNATURES = {
    'compiler-rt-20.1.2.src.tar.xz': '7651578054c96f4a7fdbfe81f53ca6fad66595b143be62a0dbdd12ff11a59e15',
    'cmake-20.1.2.src.tar.xz': '52d6f706028f1a1529f6e9c4050d84f639f4b7c6c93a39d1fd26ce45abe31cdc',
}
PATCH_SHA256 = '56e2a9e03d2e2cbb7902af0504b33fdee3b40c4256c1be7777c62eba439bc23f'
ORIGINAL_SHA256 = '4f7622030b7db53e7287b8d98ca6d880a2d1980ef24a9432176f762eb163143f'
MODIFIED_SHA256 = '4664c2730341417c0e8105e1e97a92876ab77794ca22553179fa9eef9977361f'


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(path, maximum=1024**3):
    """Read regular, non-symlink, stable bytes with a finite ceiling."""
    import stat
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(fd, 'rb') as stream:
        before = os.fstat(stream.fileno())
        require(stat.S_ISREG(before.st_mode) and 0 <= before.st_size <= maximum,
                f'unsafe or overlarge file: {path}')
        sha = hashlib.sha256()
        consumed = 0
        while chunk := stream.read(min(1024**2, before.st_size - consumed + 1)):
            consumed += len(chunk)
            require(consumed <= before.st_size, f'growing file: {path}')
            sha.update(chunk)
        after = os.fstat(stream.fileno())
        require((before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns, before.st_ctime_ns)
                == (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns, after.st_ctime_ns)
                and consumed == before.st_size, f'file changed while hashing: {path}')
        return {'bytes': consumed, 'sha256': sha.hexdigest()}


def fetch(url, path, maximum, exact=None, sha256=None):
    with urllib.request.urlopen(url, timeout=90) as response:
        require(response.url.startswith('https://'), 'source redirect lost HTTPS')
        data = response.read(maximum + 1)
    require(0 < len(data) <= maximum and (exact is None or len(data) == exact),
            f'official source size differs: {path.name}')
    require(sha256 is None or hashlib.sha256(data).hexdigest() == sha256,
            f'official source SHA-256 differs: {path.name}')
    path.write_bytes(data)


def accept_signature(status, returncode, created, expires):
    """Historical key expiry is allowed, never signature expiry or bad/revoked keys."""
    rows = [line[len('[GNUPG:] '):].split() for line in status.splitlines()
            if line.startswith('[GNUPG:] ')]
    forbidden = {'BADSIG', 'ERRSIG', 'EXPSIG', 'REVKEYSIG', 'KEYREVOKED', 'NO_PUBKEY',
                 'NODATA', 'FAILURE', 'ERROR', 'BADARMOR'}
    require(returncode == 0 and not any(row[0] in forbidden for row in rows),
            'official detached signature is bad, revoked, expired or unverifiable')
    valid = [row for row in rows if row[0] == 'VALIDSIG']
    require(len(valid) == 1 and valid[0][1] == SIGNER and valid[0][-1] == SIGNER,
            'official detached signature signer differs')
    signature = valid[0]
    require(len(signature) == 11 and signature[5:9] == ['4', '0', '1', '8'],
            'unexpected official signature algorithm')
    timestamp, signature_expiry = int(signature[3]), int(signature[4])
    require(created <= timestamp < expires and signature_expiry == 0,
            'official signature was not made during pinned key validity')
    expired = [row for row in rows if row[0] == 'EXPKEYSIG']
    require(not expired or (len(expired) == 1 and expired[0][1] == SIGNER[-16:]
                            and any(row == ['KEYEXPIRED', str(expires)] for row in rows)),
            'historical expired-key status does not match pinned key')


def authenticate(inputs, download=False):
    if download:
        fetch(KEY_URL, inputs / 'release-keys.asc', 65536)
        for name, (size, sha) in ASSETS.items():
            fetch(RELEASE_URL + name, inputs / name, size, size, sha)
            fetch(RELEASE_URL + name + '.sig', inputs / (name + '.sig'), 566, 566,
                  SIGNATURES.get(name))
    for name, (size, sha) in ASSETS.items():
        require(digest(inputs / name, size) == {'bytes': size, 'sha256': sha},
                'pinned official source differs')
        signature = digest(inputs / (name + '.sig'), 566)
        require(signature['bytes'] == 566 and
                (name not in SIGNATURES or signature['sha256'] == SIGNATURES[name]),
                'pinned official signature differs')
    digest(inputs / 'release-keys.asc', 65536)
    # gpgconf's homedir confines cleanup to this private verification agent.
    with tempfile.TemporaryDirectory(prefix='.gpg-', dir=inputs.parent) as temporary:
        home = Path(temporary)
        os.chmod(home, 0o700)
        command = ['gpg', '--no-options', '--homedir', str(home), '--batch',
                   '--no-auto-key-retrieve', '--auto-key-locate', 'clear']
        try:
            subprocess.run(command + ['--import', str(inputs / 'release-keys.asc')],
                           check=True, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, timeout=30)
            listed = subprocess.run(command + ['--with-colons', '--list-keys', SIGNER],
                                    check=True, capture_output=True, text=True, timeout=30).stdout
            rows = [row.split(':') for row in listed.splitlines()]
            public = next(row for row in rows if row[0] == 'pub')
            require(public[1] != 'r' and public[4] == SIGNER[-16:] and public[5] and public[6],
                    'pinned release key is revoked or has no finite validity')
            require(any(row[0] == 'fpr' and row[9] == SIGNER for row in rows),
                    'pinned release key fingerprint differs')
            statuses = {}
            for name in ASSETS:
                checked = subprocess.run(command + ['--status-fd', '1', '--verify',
                                          str(inputs / (name + '.sig')), str(inputs / name)],
                                         capture_output=True, text=True, timeout=30)
                accept_signature(checked.stdout, checked.returncode, int(public[5]), int(public[6]))
                statuses[name] = checked.stdout
            return {'key_anchor': KEY_URL, 'signer': SIGNER, 'status': statuses}
        finally:
            subprocess.run(['gpgconf', '--homedir', str(home), '--kill', 'gpg-agent'],
                           check=True, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, timeout=15)


def extract_sources(inputs, destination):
    for name in ASSETS:
        prefix = name.removesuffix('.tar.xz')
        with tarfile.open(inputs / name, 'r:xz') as archive:
            members = archive.getmembers()
            require(len(members) <= 250000 and sum(m.size for m in members) <= 3 * 1024**3,
                    'official source extraction exceeds finite ceiling')
            selected = []
            for member in members:
                path = PurePosixPath(member.name)
                require(not path.is_absolute() and '..' not in path.parts and path.parts and path.parts[0] == prefix,
                        'source archive path escapes pinned root')
                if not name.startswith('llvm-') or member.name.startswith(prefix + '/cmake/'):
                    require(member.isfile() or member.isdir(),
                            'source archive contains link or special file')
                    selected.append(member)
            archive.extractall(destination, members=selected, filter='data')
