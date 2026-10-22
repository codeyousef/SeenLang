"""Cross-profile integrity and software-classification checks for GPU artifacts."""
import json

from gpu_certification_io import read_bounded, require


def profile_manifests(root, targets, backend):
    """Require complete and identical software-only manifests in four profiles."""
    baseline = None
    for profile in ('fast', 'release', 'undefined', 'address'):
        lines = read_bounded(root / f'artifact-{profile}.log', 1048576).decode().splitlines()
        manifests = [json.loads(line) for line in lines
                     if line.startswith('{"schema":"seen-gpu-artifact-manifest-v1"')]
        require([item['target'] for item in manifests] == targets,
                'profile omitted target manifests')
        require(all(item['backend'] == backend and item['classification'] == 'compile-only' and
                    item['official_validation'] == 'not-run' and item['hardware_verified'] is False and
                    item['gpu_execution_verified'] is False for item in manifests),
                'profile manifest backend or software classification differs')
        if baseline is None:
            baseline = manifests
        require(manifests == baseline, 'artifact integrity manifests differ between build profiles')
