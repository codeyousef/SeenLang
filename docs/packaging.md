# Seen Packaging

Seen packages are source archives served from a static registry. A registry can
live on a normal website or object storage bucket as long as it serves files
over HTTPS.

The current default public registry URL is:

```toml
[registries]
default = "https://seen.yousef.codes/packages"
```

## Package Manifest

Packages use the same `Seen.toml` format as applications:

```toml
[project]
name = "mathx"
version = "0.1.0"
language = "en"

[dependencies]

[native.dependencies]
```

The package name is currently also the dependency key and import root.

```seen
import mathx.value.{answer}
```

## Consumer Setup

```toml
[project]
name = "demo"
version = "0.1.0"

[registries]
default = "https://seen.yousef.codes/packages"

[dependencies]
mathx = "0.1.0"

[native.dependencies]
```

Registry dependencies are fetched into `.seen/packages/`, and registry-backed
projects get a `Seen.lock`.

## Static Registry Layout

The compiler expects this layout under the registry base URL:

```text
packages/
├── index/
│   └── mathx.toml
└── archives/
    └── mathx/
        └── mathx-0.1.0.seenpkg.tgz
```

Each package index is a TOML file:

```toml
version = 1

[[releases]]
version = "0.1.0"
archive = "archives/mathx/mathx-0.1.0.seenpkg.tgz"
sha256 = "..."
```

## Publishing

`seen pkg publish` writes into a local static registry directory:

```bash
seen pkg publish ./dist/registry ./path/to/package
```

For convenience, this repo also includes:

```bash
scripts/publish_registry.sh --manifest ./path/to/package
```

That script publishes into `dist/registry` by default. If you also pass
`--sync-dir`, it mirrors the registry tree into your website checkout or deploy
directory:

```bash
scripts/publish_registry.sh \
  --manifest ./examples/mathx \
  --sync-dir /path/to/site/public/packages
```

If your hosting setup serves `/path/to/site/public/packages` at
`https://seen.yousef.codes/packages`, the package is ready for consumers after
you deploy the site.

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

## Deploying To `seen.yousef.codes`

One simple flow is:

1. Build a compiler with `pkg` support.
2. Run `scripts/publish_registry.sh --manifest <package> --sync-dir <site-packages-dir>`.
3. Deploy your site as usual.
4. Verify that `https://seen.yousef.codes/packages/index/<package>.toml` is reachable.
5. Verify that a separate Seen project can `seen pkg fetch` from the hosted URL.

## Current Limits

- Registry versions are exact-only for now. `^`, `~`, and `*` are rejected.
- `seen pkg publish` writes to local directories; it does not upload over HTTP.
- Package name, dependency key, and import root are the same in this MVP.
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
