# Compilation Targets

Seen's shipped LLVM compiler can build native binaries for the platforms below.
Use the canonical target names with `seen compile --target=<name>`.

| Target | LLVM Triple | Notes |
|--------|-------------|-------|
| `linux-x86_64` | `x86_64-unknown-linux-gnu` | Default Linux desktop/server target |
| `linux-arm64` | `aarch64-unknown-linux-gnu` | GNU/Linux AArch64 cross target |
| `linux-riscv64` | `riscv64-unknown-linux-gnu` | RV64GC Linux GNU, LP64D ABI |
| `windows-x86_64` | `x86_64-w64-windows-gnu` | Windows cross-build/package target |
| `macos-x86_64` | `x86_64-apple-darwin` | Intel macOS target |
| `macos-arm64` | `arm64-apple-darwin` | Apple Silicon macOS target |
| `ios-arm64` | `arm64-apple-ios` | iOS device target |
| `ios-sim-arm64` | `arm64-apple-ios-simulator` | iOS simulator target |
| `android-arm64` | `aarch64-linux-android` | Android NDK ARM64 target |

Aliases accepted by the compiler include `riscv64`, `riscv64-linux-gnu`, and
`riscv64-unknown-linux-gnu` for `linux-riscv64`.

## Linux RISC-V

`linux-riscv64` is a first-class cross target for 64-bit Linux RISC-V userspace.
The baseline is RV64GC with the LP64D ABI. Runtime SIMD reports scalar fallback
features on RISC-V until RVV lowering is implemented.

On Arch-compatible hosts, the toolchain used by the QEMU smoke scripts is:

```bash
sudo pacman -Syu --needed clang llvm lld file qemu-user qemu-user-static qemu-system-riscv qemu-system-riscv-firmware riscv64-linux-gnu-binutils riscv64-linux-gnu-gcc riscv64-linux-gnu-glibc
```

Fast user-mode verification:

```bash
bash scripts/test_riscv64_qemu.sh --compiler compiler_seen/target/seen --require
```

Optional full-system verification requires a RISC-V kernel/rootfs and runs:

```bash
SEEN_RISCV64_QEMU_KERNEL=/path/to/Image \
SEEN_RISCV64_QEMU_ROOTFS=/path/to/rootfs.qcow2 \
SEEN_RISCV64_QEMU_IDENTITY=/path/to/key \
bash scripts/test_riscv64_system_qemu.sh --compiler compiler_seen/target/seen --require
```

## Cross-Build Artifacts

Packaging and cross-build helpers can request per-module LLVM IR without
scraping global temporary directories:

```bash
seen compile app.seen app-rv64 --target=linux-riscv64 \
  --emit-module-ir-dir target/riscv64-ir --stop-after-ir
```

The build and packaging scripts cache IR, transformed objects, runtime objects,
and package payload manifests under ignored `target/` directories when the full
target, toolchain, compiler, runtime, profile, and LTO signatures match.
