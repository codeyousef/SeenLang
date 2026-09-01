# Seen installer tooling

This directory contains release-packaging scripts and templates for Seen
0.19.3. It is maintainer tooling, not proof that every package is already
published to a public package-manager repository.

## Layout

- `scripts/` — Unix and PowerShell binary-release installers
- `linux/` — DEB, RPM, and AppImage builders
- `windows/` — MSI/WiX builder and validation scripts
- `homebrew/` — formula template and generator
- `scoop/` — manifest template and generator
- `test/` — installer/package smoke tests
- `assets/` — icons and release assets

The checked-in Homebrew and Scoop files are templates. Replace placeholder
checksums with hashes from the exact 0.19.3 release assets before publishing.

## Installed command surface

An installation should provide a matched `seen` and `seen-pkg`. Optional helper
binaries may also be present. The shipped compiler uses explicit source paths:

```bash
seen --version
seen check src/main.seen
seen run src/main.seen
seen compile src/main.seen my-program
./my-program
```

Create `Seen.toml` and `src/main.seen` yourself; 0.19.3 does not ship `init`,
`build`, `test`, `fmt`, or `clean` commands. New manifests begin with:

```toml
manifest-version = 1

[project]
name = "my_program"
version = "0.1.0"
language = "en"
```

The installed compiler is LLVM-only. Native compilation requires the
version-matched runtime/stdlib payload plus LLVM 19 or newer tools (`clang`,
`opt`, `llc`, `llvm-as`, and `lld`), with LLVM 20 preferred. Source rebuilds
also require the Go version documented in
[`docs/bootstrap.md`](../docs/bootstrap.md) to build the matching package
helper; binary releases include that helper.

## Building package artifacts

Examples for the current release version:

```bash
installer/linux/build-deb.sh 0.19.3 amd64
installer/linux/build-rpm.sh 0.19.3 x86_64
installer/linux/build-appimage.sh 0.19.3 x86_64
installer/homebrew/generate-formula.sh --version 0.19.3
```

```powershell
installer\windows\build.bat 0.19.3 x64
installer\scoop\generate-manifest.ps1 -Version 0.19.3
```

Each builder has its own host-tool prerequisites. Build release artifacts from
a verified compiler payload and run the format-specific validation before
publishing. Release archives should include the toolchain manifest/helper so an
installation can diagnose or provision its LLVM dependency consistently.

## Verification

At minimum, validate:

1. archive/package checksums and signatures;
2. installation and PATH behavior on a clean supported host;
3. `seen --version` and matching `seen-pkg` version;
4. `seen check`, `seen run`, and `seen compile` on a minimal `.seen` file;
5. uninstall/upgrade behavior and absence of undeclared generated files.

Use `installer/test/integration-test.sh` for the repository's mock packaging
workflow. It does not replace a clean-host test of the actual release artifact.

See [`docs/getting-started.md`](../docs/getting-started.md),
[`docs/cli-reference.md`](../docs/cli-reference.md), and
[`docs/targets.md`](../docs/targets.md) for the current language/toolchain
instructions.
