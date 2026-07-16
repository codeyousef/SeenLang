# Seen Packaging

Seen 0.10.0 ships a version-coupled package client for declaring dependencies in
`Seen.toml`, resolving complete dependency graphs, enforcing `Seen.lock`, and
installing verified source packages. The compiler prepares those dependencies
before `compile`, `check`, and `run` and rejects a package client from a
different Seen release.

The planned development registry origin is:

```toml
[registries]
default = "https://seen.dev.yousef.codes/packages"
```

That URL is configuration, not a claim that the hosted service is live. Seen
0.10.0 deliberately ships without an official root envelope or digest for the
development or production origin, so both origins fail closed. An HTTPS origin
becomes usable only with a complete root of trust distributed out of band.

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
canonical `owner/name` registry identity. They are intentionally independent.
Consumer dependency keys are a third value: local aliases/import roots that do
not change registry identity.

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
calc = { package = "alice/mathx", version = "^1.2.0", allow = ["file"] }
web = { package = "seen/web", version = "~2.4", allow = ["network"] }

[package-grants]
"alice/mathx" = ["file"]
"seen/web" = ["network"]
```

Source imports use the alias:

```seen
import calc.value.{answer}
```

The supported registry requirement forms are exact SemVer, caret (`^2.1.0`),
tilde (`~1.4`), and a bounded comparator conjunction
(`>=1.2.3 <2.0.0`). The resolver intersects every incoming requirement for one
canonical `(registry origin, package identity)` key and chooses the highest
eligible version using deterministic graph and backtracking order. Stable,
prerelease, yanked, quarantined, cycle, diamond, and build-metadata behavior is
defined by the executable v1 contract.

## Lockfiles and Resolution Modes

`seen pkg fetch` resolves the normal graph, reuses a valid locked candidate when
possible, verifies all required metadata and archives, and atomically writes
`Seen.lock` version 2. `seen pkg update` ignores lock preference and selects the
newest eligible graph.

The lock records:

- the exact `Seen.toml` digest and root dependency edges
- every reachable transitive package node
- canonical origins, identities, requirements, and exact resolved versions
- exact archive digests, signed target paths, and metadata versions
- transitive dependency edges, package capability requests, and root grants

Equivalent graphs serialize identically. An incomplete, stale, non-canonical,
or manually inconsistent lock is rejected. Commit `Seen.lock` with the project.
Do not confuse it with `seen_std/Seen.modules.lock`, which records standard
library module/ABI hashes.

Resolution modes apply to `seen pkg fetch` and to the automatic fetch performed
by `seen compile`, `seen check`, and `seen run`:

| Mode | Behavior |
|------|----------|
| normal | Prefer a valid lock, resolve when needed, fetch verified state, and update the lock atomically. |
| `--locked` | Require the existing lock to match `Seen.toml`; permit only its exact signed metadata/blobs; never rewrite it. |
| `--offline` | Make no network requests; use only unexpired, previously verified metadata and verified blobs; a complete lock may be written. |
| `--frozen` | Apply both `--locked` and `--offline`. |

`update` cannot be combined with `--locked` or `--frozen`.

## Capability Consent

Packages declare requested capabilities in `[package].capabilities`. Every
dependency edge declares `allow`, and the root project grants capabilities by
canonical identity under `[package-grants]`. Grants must cover direct and
transitive packages. If a package adds a capability, resolution stops with
`capability_consent_required` before download, build, or lock write until the
root project explicitly grants it.

The canonical vocabulary is `file`, `network`, `process`, `environment`,
`dynamic-load`, `ffi`, `unsafe`, `native-link`, and `macro`. Capability consent
is an auditable policy signal, not an operating-system sandbox. FFI, unsafe
operations, native linking, and similar facilities can escape language-level
enforcement.

## Trust, Download, and Installation

Registry metadata uses threshold-verified TUF roles with canonical JSON and
Ed25519 signatures. The client binds an archive to its origin, canonical
identity, exact version, signed target path, byte length, and SHA-256 digest;
rejects rollback, expired or inconsistent metadata; and never follows an
origin to another registry.

Hosted identities are exact lowercase ASCII `owner/name` values. Aliases use a
portable ASCII import-root grammar. The client rejects lookalike Unicode,
case-folded, decoded, normalized, redirected, or otherwise repaired identities
instead of guessing which package the project meant.

Downloads are HTTPS-only, bounded, streamed into staging files, verified, and
atomically promoted. Source archives are preflighted and extracted in two
passes with limits on compressed and expanded size, entry count, individual
files, path depth/length, compression ratio, and validation time. Absolute or
ambiguous paths, links, device entries, native binaries, lifecycle scripts,
duplicate entries, manifest mismatches, and archives that change between passes
are rejected.

The initial archive limits are 25 MiB compressed, 100 MiB expanded, 4,096
entries, 10 MiB per file, 240 bytes per path, 16 path segments, a 100:1
expansion ratio, and 30 seconds of validation time. Limit and validation failures
reject closed.

Verified package content is stored immutably by digest. Projects receive local,
read-only copies under `.seen/views/` and an authoritative
`.seen/package-map.tsv` only after the complete graph succeeds. Views are not
hard links into the shared cache. A failed resolution does not expose a partial
package graph or replace the prior lock. Commit `Seen.lock`, but do not commit
the generated `.seen/` directory.

## Package Commands

The package command group is:

```bash
seen pkg add|remove|fetch|update|tree|audit [options]
seen pkg pack [project-dir-or-manifest] [output]
seen pkg prebuild [project-dir-or-manifest] [output-dir]
```

`add` and `remove` edit project dependencies, `fetch` installs the resolved
graph, `update` performs fresh selection, `tree` prints the locked graph,
`audit` validates lock graph/capability bindings and lists package digests, and
`pack` creates a source archive. `prebuild` remains the separate local
native-artifact workflow described below.

The CLI also reserves `login`, `logout`, `whoami`, `publish`, `yank`, and
`report` for the hosted workflow. They fail closed in 0.10.0 because the service
and Aether authentication integration are not active. They do not silently
write an unsigned registry or treat the planned development URL as live.

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

Development is planned to deploy first at
`https://seen.dev.yousef.codes/packages`. Production later promotes the same
host-neutral contract to `https://seen.yousef.codes/packages` by changing
environment configuration, delegated signing identities, and routing—not by
rewriting package or schema identities.

Neither deployment is activated by this release. The official origins remain
untrusted until an official complete root envelope and digest are distributed
out of band. Hosted account operations, private-package access, publishing,
yanking, and reporting remain inactive until the service and Aether integration
are ready.

## Current Limits

- No official registry trust root is embedded in 0.10.0, so the planned
  development and production origins fail closed.
- Hosted authentication, private packages, publishing, yanking, and reporting
  are inactive until the service and Aether authentication integration ship.
- Hosted URLs must use canonical HTTPS origins; plain HTTP, origin changes, and
  unsigned fallback metadata are rejected.
- Prebuilt package artifacts are local path dependencies; registry publication
  is source-only.
- Artifact interface modules are declaration-only from the consumer's point of
  view; implementation objects come from `objects.tsv`.

## Release Artifact Builds

Release package scripts are designed to use an already-built compiler binary and
should be run under an explicit memory guard during release verification.

- `scripts/build_release.sh` stages the Linux tarball and, on hosts with the
  required tools, builds DEB, RPM, and AppImage artifacts.
- Release staging builds and installs the matching `seen-pkg` binary beside
  `seen`; its hash participates in package-artifact cache identities. Linux
  archives/packages and Windows ZIP/MSI/NSIS installers require the helper
  instead of producing a partial compiler-only installation.
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
`scripts/safe_rebuild.sh --tier full`. The stamp binds the exact clean Git tree,
compiler binary, and package-client binary; changing or committing release
inputs after the rebuild invalidates it. The upload script keeps reusable
artifacts for other versions in `dist/`, but removes every output for the
requested version before rebuilding so a failed optional builder cannot upload
stale bytes. Cross-host macOS archives must be supplied from a separate
directory through `SEEN_RELEASE_MACOS_INPUT_DIR`; implicit macOS files already
in `dist/` are discarded. Set `SEEN_RELEASE_CLEAN_DIST=1` for an explicit clean
release staging directory.

## Related

- [Project Configuration](project-config.md)
- [CLI Reference](cli-reference.md)
