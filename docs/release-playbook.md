# Seen Release Playbook

This playbook captures the steps for producing a self-hosted release artifact on Linux,
including deterministic bootstrap runs, manifest/signature generation, and verification.
It assumes `seen_cli` is already built with the LLVM backend (`cargo build -p seen_cli --release --features llvm`).

## 1. Prepare Keys

1. Generate or retrieve the Ed25519 key pair used for release signing.
2. Store the private key in a workspace-local file (raw 32/64-byte seed or hex).
3. Store the public key (32-byte raw or hex) separately; this will be published for verification.

> Example layout:
> ```
> secrets/
>   release_signing_key.hex   # private key
>   release_public_key.hex    # public key
> ```

## 2. Build & Sign the Bootstrap Matrix

Run the matrix script with the signing key (and optionally a public key to self-verify):

```bash
scripts/release_bootstrap_matrix.sh \
  --matrix releases/bootstrap_matrix.toml \
  --output-dir artifacts/bootstrap \
  --signing-key secrets/release_signing_key.hex \
  --public-key secrets/release_public_key.hex \
  --abi-manifest seen_std/Seen.toml \
  --abi-lock seen_std/Seen.lock \
  --abi-snapshot artifacts/abi/seen_std.json
```

This performs, for each matrix entry:

1. Stage-1 build via `seen_cli`.
2. Stage-2/Stage-3 rebuilds via the freshly produced stages.
3. Manifest emission using `tools/sign_bootstrap_artifact sign ...`.
4. Optional verification (`sign_bootstrap_artifact verify ...`) if `--public-key` is supplied.

Outputs land under `artifacts/bootstrap/<entry>/`:

```
artifacts/bootstrap/linux-x86_64-llvm-deterministic/
  stage1_seen
  stage2_seen
  stage3_seen
  manifest.json
  manifest.json.sig   # when signing enabled
```

An `index.json` summarises all entries (hashes + metadata) at `artifacts/bootstrap/index.json`.

## 3. Verify Signatures (Consumers / CI)

Distribute the public key and run:

```bash
tools/sign_bootstrap_artifact/target/release/sign_bootstrap_artifact verify \
  --manifest artifacts/bootstrap/<entry>/manifest.json \
  --signature artifacts/bootstrap/<entry>/manifest.json.sig \
  --public-key release_public_key.hex
```

This command accepts raw or hex-encoded keys/signatures. CI should run it for every manifest
before publishing release artifacts.

Need to bypass the ABI guard for investigation? Pass `--skip-abi-verify` when invoking
`scripts/release_bootstrap_matrix.sh` (not recommended for real releases).

## 4. Publish Artifacts

1. Upload `stage3_seen`, `manifest.json`, and `manifest.json.sig` per matrix entry to the release bucket.
2. Publish `index.json` as the authoritative manifest list.
3. Include the public key (or link to it) in the release notes so consumers can verify downloads.
4. Package and upload the `seen_std` archive:

```bash
scripts/package_seen_std.sh --version $RELEASE_VERSION --output-dir artifacts/packages
```

This produces a deterministic `libseen_std-$RELEASE_VERSION.seenpkg` plus a `.sha256` checksum so consumers
can download the bundled stdlib that matches the published ABI lock.

## 5. Future Extensions

* Wire the signing key to a hardware-backed KMS instead of local files.
* Automate public-key distribution and provide a CLI shortcut (e.g., `seen release verify ...`).
* Integrate `sign_bootstrap_artifact verify` into CI gates for release tags.

---

## ABI Snapshot & Verification

The `tools/abi_guard` utility records the hashes for every module listed in `Seen.toml`
so we can prove that the self-hosted stdlib/ABI surface is stable between releases.

### Snapshot

```bash
cargo run -p abi_guard -- snapshot \
  --manifest seen_std/Seen.toml \
  --output artifacts/abi/seen_std-v0.1.0.json \
  --lock seen_std/Seen.lock \
  --update-lock
```

This command:

1. Hashes every module from the manifest relative to the project directory.
2. Writes a JSON snapshot (`schema_version`, `package`, `version`, per-module SHA256).
3. Updates `Seen.lock` so the repo records the canonical hash for each module.

### Verify

CI (and developers) can confirm nothing drifted by running:

```bash
cargo run -p abi_guard -- verify \
  --manifest seen_std/Seen.toml \
  --lock seen_std/Seen.lock
```

This recomputes the hashes and fails if any entry differs from the committed lock file.
Integrate this check before publishing releases so regressions trigger early.
