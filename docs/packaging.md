# Seen Packaging

Seen 0.10 is designing a hosted, signed package registry. The existing compiler
also has a legacy local-static package flow for development; that flow is not a
hosted-v1 publisher and must not be exposed as a public write path.

The planned development registry base URL is:

```toml
[registries]
default = "https://seen.dev.yousef.codes/packages"
```

## Package Manifest

Packages use the same `Seen.toml` format as applications:

```toml
manifest-version = 1

[project]
name = "math_core"
version = "0.1.0"
language = "en"

[package]
identity = "alice/mathx"
visibility = "public"
include = ["src/**/*.seen", "README.md", "LICENSE"]
assets = []
capabilities = []

[dependencies]

[native.dependencies]
```

`project.name` is the package's local module root. `package.identity` is its
canonical registry identity. They are intentionally independent. Consumer
dependency keys are a third value: local aliases/import roots that do not
change registry identity.

```seen
import math_core.value.{answer}
```

## Consumer Setup

```toml
[project]
name = "demo"
version = "0.1.0"

[registries]
default = "https://seen.dev.yousef.codes/packages"

[dependencies]
calc = { package = "alice/mathx", version = "0.1.0" }

[native.dependencies]
```

Source imports use the alias:

```seen
import calc.value.{answer}
```

Registry dependencies are fetched into
`.seen/packages/<owner>/<name>/<version>/<archive-sha256>/`, and registry-backed projects get a
dependency `Seen.lock` version 2 containing the alias, canonical identity,
exact version, resolved registry origin, and exact archive digest. The separate
stdlib ABI snapshot is named `Seen.modules.lock`. The current compiler writes
the dependency file as a provisional resolution report; lock enforcement is
not implemented yet, and hosted resolution remains disabled until it is.

## Legacy Local-Static Registry Layout

The current development-only compatibility flow writes this layout:

```text
packages/
├── index/
│   └── alice/
│       └── mathx.toml
└── archives/
    └── alice/
        └── mathx/
            └── mathx-0.1.0.seenpkg.tgz
```

Each package index is a TOML file:

```toml
version = 1

[[releases]]
version = "0.1.0"
archive = "archives/alice/mathx/mathx-0.1.0.seenpkg.tgz"
sha256 = "..."
```

## Local Development Publishing

`seen pkg publish` writes into a local static registry directory. It does not
perform hosted ingestion, provenance verification, sandboxed scans, the public
72-hour delay, a second scan, or signed metadata publication:

```bash
seen pkg publish ./dist/registry ./path/to/package
```

For convenience, this repo also includes:

```bash
scripts/publish_registry.sh --manifest ./path/to/package
```

That script publishes into `dist/registry` for filesystem-based development
only; it intentionally has no website-sync or public-URL option. Do not deploy
this output as the hosted Seen registry. Hosted v1 publication
must use `/packages/api/v1` after its writer is enabled and must remain
unavailable until ingestion, scanning, delay, and signing complete.

## Prebuilt Package Artifacts

Local builds can also consume a prebuilt package artifact instead of compiling
the package implementation modules again. Build the package once:

```bash
seen pkg prebuild ./path/to/package ./dist/mathx-0.1.0.seenpkg
```

The artifact directory contains:

```text
mathx-0.1.0.seenpkg/
├── Seen.pkg.toml
├── interface.index.tsv
├── objects.tsv
├── objects/
│   └── module_0.o
└── src/
    └── ...
```

Downstream projects reference it from `Seen.toml`:

```toml
[dependencies]
mathx = { artifact = "../dist/mathx-0.1.0.seenpkg" }
```

The compiler loads declarations from `interface.index.tsv` or the artifact
interface sources, links the objects from `objects.tsv`, and skips code
generation for modules provided by that artifact. `seen pkg prebuild` emits PIC
objects through the same `--object-manifest` path used by external link
workflows, so the artifact can be linked into executables or shared-library
builds.

`interface.index.tsv` is a declaration index. It records the package version,
module paths, imports, and public/package-visible declarations so downstream
projects can resolve package-root imports without compiling implementation
modules.

For artifact dependencies, declaration discovery and code generation are
separate on purpose. During the declaration pass, the compiler queues the
artifact's interface modules so local code can resolve package functions,
methods, fields, and aggregate return types. During object emission, those same
artifact modules are treated as already provided and are not compiled again; the
objects listed in `objects.tsv` are linked instead. This split is important for
helpers that return `String`, classes, arrays, or other aggregate values,
because the consumer must see the real signature even though the implementation
came from the artifact.

## Hosted Registry Environments

Development will first deploy at
`https://seen.dev.yousef.codes/packages`. Production later promotes the same
host-neutral contract to `https://seen.yousef.codes/packages` by changing
environment configuration, delegated signing identities, and routing—not by
rewriting package or schema identities.

The hosted writer is currently disabled. The local-static script above is only
for local/private compatibility testing. The compiler also fails closed on
HTTPS registry resolution until signed resolver metadata and hardened archive
extraction are implemented; the configured development URL is the intended
future default, not an unsigned-static fallback.

## Current Limits

- Registry versions are exact-only for now. `^`, `~`, and `*` are rejected.
- `seen pkg publish` is a legacy local-development operation; it does not
  upload over HTTP and does not satisfy hosted-v1 security gates.
- Hosted URLs must use HTTPS. Plain HTTP registries are rejected; filesystem
  paths remain available for explicit local development.
- The current extractor still uses the platform archive tool and is not the
  hosted-v1 hardened extractor. Hosted resolution remains disabled until
  bounded preflight, link/device/path rejection, staging, and atomic promotion
  land under FEL-631/FEL-629.
- Legacy unscoped dependencies remain available for local static-registry
  compatibility; the hosted v1 registry requires canonical `owner/name`
  identities.
- Prebuilt package artifacts are local path dependencies; registry publication
  still serves source archives.
- Artifact interface modules are declaration-only from the consumer's point of
  view; implementation objects come from `objects.tsv`.

## Release Artifact Builds

Release package scripts are designed to use an already-built compiler binary and
should be run under an explicit memory guard during release verification.

- `scripts/build_release.sh` stages the Linux tarball and, on hosts with the
  required tools, builds DEB, RPM, and AppImage artifacts.
- Linux release staging reuses hashed stdlib/runtime/language/docs/toolchain
  payloads from `target/seen-build/package-payloads/` when the payload hash
  matches, then builds independent package formats with bounded
  `SEEN_PACKAGE_JOBS`.
- Linux release artifacts are also cached under
  `target/seen-build/package-artifacts/linux/` by version, compiler hash,
  payload hash, package scripts, package-tool availability, CPU baseline, and
  verification mode. A manifest-verified cache hit restores the tarball and
  optional package outputs without rebuilding them.
- `scripts/build_windows_installer.sh <version> --force-compile --zip-only`
  rebuilds `target-windows/seen.exe` from Linux cross-compilation tools, writes
  its reuse manifest, and stages the Windows ZIP.
- `scripts/build_windows_installer.sh <version> --skip-compile --nsis` builds
  the Windows ZIP and the NSIS setup executable from an existing, manifest-valid
  `target-windows/seen.exe`.
- Windows `target-windows/seen.exe` reuse is manifest-gated. The companion
  `target-windows/seen.exe.manifest.env` must match the package version, binary
  hash, payload hash, and toolchain hash before ZIP/installer staging proceeds.
- Windows cross-builds use compiler-owned `--emit-module-ir-dir` output plus
  `target-windows/ir-cache/`, `target-windows/object-cache/`, and
  `target-windows/runtime-cache/` instead of clearing `.seen_cache` or
  `/tmp/seen_ir_cache`. Windows ZIP outputs are cached under
  `target-windows/package-artifacts/` with the same manifest-gated reuse model.
- `installer/linux/build-appimage.sh` prefers an installed `appimagetool`, can
  use `SEEN_APPIMAGE_RUNTIME_FILE` for offline builds, defaults to `xz`
  compression for broad runtime compatibility, and validates with
  extract-and-run when FUSE is not available.
- `installer/linux/build-rpm.sh` keeps RPM temporary and database files inside
  the build tree so package creation does not depend on writable system RPM
  state.
- `installer/linux/build-deb.sh` writes package control metadata after staging
  files so `Installed-Size` reflects the actual payload.

`scripts/build_and_upload_release.sh` refuses to upload artifacts unless a
recent full verification stamp exists under `target/seen-build/`, written by
`scripts/safe_rebuild.sh --tier full`. It keeps reusable `dist/` artifacts by
default; set `SEEN_RELEASE_CLEAN_DIST=1` for an explicit clean release staging
directory.

## Related

- [Project Configuration](project-config.md)
- [CLI Reference](cli-reference.md)
