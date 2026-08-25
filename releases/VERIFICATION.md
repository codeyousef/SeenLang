# Verifying Seen Release Artifacts

All Seen release binaries are signed using [Sigstore](https://www.sigstore.dev/) for supply chain security. Each release includes `.bundle` files containing the signature and certificate chain.

## Quick Verification

### 1. Install cosign

```bash
# Via Go
go install github.com/sigstore/cosign/v2/cmd/cosign@latest

# Via Homebrew
brew install cosign

# Via package manager (Arch)
pacman -S cosign
```

### 2. Download the component set and manifest

```bash
# Download the `seen-<version>-release-artifacts.json` manifest, its `.sha256`
# and `.bundle`, plus all files named by its four artifact records.
```

### 3. Verify the checksum

```bash
echo "$(cat seen-<version>-release-artifacts.json.sha256)  seen-<version>-release-artifacts.json" | sha256sum --check
```

### 4. Verify the Sigstore signature

```bash
./scripts/verify_release.sh \
  --manifest seen-<version>-release-artifacts.json \
  --artifact-dir .
```

A successful verification prints `Verified OK`.

For keyless releases, the script derives one exact certificate identity from
the manifest version: the SeenLang `release.yml` workflow at that exact
`refs/tags/v<version>` ref. The regular expression is anchored and all literal
dots are escaped—for example,
`^https://github\.com/codeyousef/SeenLang/\.github/workflows/release\.yml@refs/tags/v0\.12\.0$`.
A broader `--certificate-identity` override, a different workflow or tag, or a
non-GitHub-Actions issuer is rejected before signature verification.

## Using the verification script

The repository includes a convenience script:

```bash
./scripts/verify_release.sh \
  --manifest seen-<version>-release-artifacts.json \
  --artifact-dir .
```

## What gets verified

- **Shape**: the canonical manifest contains exactly the ordered compiler,
  runtime, standard-library, and package-client roles
- **Pins**: source commit/archive, target, version, sizes, artifact hashes, and
  signature-bundle hashes all match
- **Checksums**: every required `.sha256` sidecar is present and exact
- **Signatures**: the manifest and all four components verify against the
  declared signing policy; a missing or invalid signature fails closed

## Signing modes

Release artifacts can be signed using:

| Mode | Flag | Use case |
|------|------|----------|
| Keyless (OIDC) | `--keyless` | CI/CD pipelines with ambient identity |
| Local key | `--key <path>` | Manual releases with cosign key pair |
| KMS/HSM | `--kms <uri>` | Hardware-backed signing (AWS KMS, GCP KMS, PKCS#11) |

## Manifest

Each release includes a canonical `seen-<version>-release-artifacts.json` with
the exact component pins. The manifest itself is checksummed, signed, and
verified before upload. Installer archives may be additional release assets;
they never replace the four component trust anchors.
