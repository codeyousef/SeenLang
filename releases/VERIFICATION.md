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

### 2. Download the artifact and its bundle

```bash
# Example for Linux x64
curl -LO https://github.com/seenlang/seen/releases/download/v1.0.0/seen-linux-x64
curl -LO https://github.com/seenlang/seen/releases/download/v1.0.0/seen-linux-x64.bundle
curl -LO https://github.com/seenlang/seen/releases/download/v1.0.0/seen-linux-x64.sha256
```

### 3. Verify the checksum

```bash
echo "$(cat seen-linux-x64.sha256)  seen-linux-x64" | sha256sum --check
```

### 4. Verify the Sigstore signature

```bash
cosign verify-blob \
  --bundle seen-linux-x64.bundle \
  --certificate-identity-regexp "github.com/.*seenlang" \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com \
  seen-linux-x64
```

A successful verification prints `Verified OK`.

## Using the verification script

The repository includes a convenience script:

```bash
./scripts/verify_release.sh seen-linux-x64
```

## What gets verified

- **Checksum**: SHA-256 hash matches the published `.sha256` file
- **Signature**: The binary was signed by the Seen project's CI pipeline (GitHub Actions OIDC)
- **Transparency**: The signature is recorded in the Sigstore public transparency log (Rekor)

## Signing modes

Release artifacts can be signed using:

| Mode | Flag | Use case |
|------|------|----------|
| Keyless (OIDC) | `--keyless` | CI/CD pipelines with ambient identity |
| Local key | `--key <path>` | Manual releases with cosign key pair |
| KMS/HSM | `--kms <uri>` | Hardware-backed signing (AWS KMS, GCP KMS, PKCS#11) |

## Manifest

Each release includes a `manifest.json` with checksums and sizes for all artifacts. The manifest itself is also signed.
