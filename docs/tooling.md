# Tooling

## VS Code Extension

The `vscode-seen/` directory contains the official VS Code extension. It tracks
the shipped six-language set: `en`, `ar`, `es`, `ru`, `zh`, and `ja`.

### Installation From Source

```bash
cd vscode-seen
npm install
npm run package
code --install-extension seen-*.vsix
```

### Features

- TextMate syntax highlighting for Seen syntax, comments, annotations, package
  imports, module aliases, facade component syntax, effects, hot reload helpers,
  and multilingual keywords.
- LSP-backed diagnostics, completions, hover, definitions, references, rename,
  and document symbols through `seen lsp`.
- Import-block folding for contiguous top-level `import`, `use`, and
  `pub import` declarations.
- Build/check/run/package tasks using the shipped `seen compile`, `seen check`,
  `seen run`, and `seen pkg` commands.
- Snippets for functions, classes, structs, enums, traits/interfaces, effects,
  packages, facade components, UI state/effects, shared modules, hot reload,
  GPU/SIMD, defer/errdefer, and FFI.

### Commands

| Command | Typical CLI |
|---------|-------------|
| Seen: Build Project | `seen compile <file> [output]` |
| Seen: Run Project | `seen run <file>` |
| Seen: Check Project | `seen check <file>` |
| Seen: Compile Shared Module Objects | `seen compile ... --pic --object-manifest <path>` |
| Seen: Package Fetch | `seen pkg fetch` |
| Seen: Package Pack | `seen pkg pack` |
| Seen: Package Prebuild | `seen pkg prebuild` |
| Seen: Package Publish | `seen pkg publish` (hosted operation inactive) |
| Seen: Translate to Another Language | `seen translate <file> --from <lang> --to <lang>` |

Extension-only commands such as update checks or visual helpers are editor UI
features; compiler behavior remains controlled by the `seen` CLI.

Hosted package authentication, private-package access, publishing, yanking,
and reporting are inactive in Seen 0.10.0 pending service and Aether
integration. Package fetches also fail closed unless their registry has an
out-of-band trust root; the planned official origins do not yet have one
embedded in the release.

### Configuration

```json
{
  "seen.compiler.path": "seen",
  "seen.lsp.enabled": true,
  "seen.lsp.trace.server": "off",
  "seen.formatting.enable": true,
  "seen.target.default": "native",
  "seen.compile.pic": false,
  "seen.compile.objectManifest": "",
  "seen.language.default": "en"
}
```

## Language Server Protocol

Start the built-in language server:

```bash
seen lsp
```

The server resolves project language from nearby `Seen.toml` when possible and
falls back to English. It masks `/// ... ///` block comments for source-symbol
operations and supports completions/hover for packages, annotations, effects,
capabilities, hot reload, imports, sealed classes, exports, new collection and
memory types, and stdlib modules.

### Neovim

```lua
require'lspconfig'.seen.setup{
  cmd = {"seen", "lsp"},
  filetypes = {"seen"},
  root_dir = require'lspconfig.util'.root_pattern("Seen.toml", ".git"),
}
```

### Emacs

```elisp
(lsp-register-client
 (make-lsp-client :new-connection (lsp-stdio-connection '("seen" "lsp"))
                  :major-modes '(seen-mode)
                  :server-id 'seen-lsp))
```

## Diagnostics and Checks

Use the shipped checker for frontend/type diagnostics:

```bash
seen check src/main.seen
```

Use deterministic mode when auditing reproducibility-sensitive code:

```bash
seen check src/main.seen --profile deterministic
```

The checker and LSP surface warning diagnostics for conservative dead-code
cases, including unreachable statements, unused locals or parameters, unused
private top-level functions, and unused imports. Warning codes are reported as
warnings in editor clients rather than promoted to errors.

## Debugging Compiler Output

Useful environment-variable tracing:

```bash
SEEN_DEBUG_TYPES=1 seen compile program.seen program
SEEN_TRACE_LLVM=all seen compile program.seen program
SEEN_TRACE_LLVM=gep seen compile program.seen program
```

LLVM and compile-database emission are compile flags:

```bash
seen compile program.seen program --emit-llvm
seen compile program.seen program --emit-compile-db
```

## Linux ARM64 Cross Sysroot

On pacman-compatible Linux hosts, create a local AArch64 cross sysroot without
installing system packages globally:

```bash
./scripts/setup_linux_arm64_sysroot.sh
source artifacts/toolchains/linux-arm64/env.sh
```

The helper downloads and extracts cross-packages under
`artifacts/toolchains/linux-arm64/`. The generated `env.sh` sets
`SEEN_LINUX_ARM64_SYSROOT` and `SEEN_LINUX_ARM64_GCC_TOOLCHAIN`.

Validate the setup:

```bash
bash scripts/native_target_smoke.sh --compiler compiler_seen/target/seen --target linux-arm64
bash scripts/platform_matrix.sh --stage3 compiler_seen/target/seen --platform linux-arm64
```

## Linux RISC-V Cross Sysroot and QEMU

Seen supports `linux-riscv64` as an RV64GC Linux GNU userspace target. The
canonical target list and triples are documented in
[Compilation Targets](targets.md). On pacman-compatible Linux hosts, install
system packages directly or create a local sysroot:

```bash
sudo pacman -Syu --needed clang llvm lld file qemu-user qemu-user-static qemu-system-riscv qemu-system-riscv-firmware riscv64-linux-gnu-binutils riscv64-linux-gnu-gcc riscv64-linux-gnu-glibc

# Optional local sysroot instead of relying on /usr/riscv64-linux-gnu:
./scripts/setup_linux_riscv64_sysroot.sh
source artifacts/toolchains/linux-riscv64/env.sh
```

Validate the fast emulator tier with QEMU user-mode:

```bash
bash scripts/test_riscv64_qemu.sh --compiler compiler_seen/target/seen --require
bash scripts/native_target_smoke.sh --compiler compiler_seen/target/seen --target linux-riscv64
```

For full guest validation, provide a RISC-V Linux kernel/rootfs with SSH and run:

```bash
SEEN_RISCV64_QEMU_KERNEL=/path/to/Image \
SEEN_RISCV64_QEMU_ROOTFS=/path/to/rootfs.qcow2 \
SEEN_RISCV64_QEMU_IDENTITY=/path/to/key \
bash scripts/test_riscv64_system_qemu.sh --compiler compiler_seen/target/seen --require
```

## Import from C

Generate Seen bindings from a C header:

```bash
seen import-c mylib.h
```

The command outputs `extern fun` declarations that can be copied into Seen
source or package interface modules.

## Related

- [Getting Started](getting-started.md)
- [CLI Reference](cli-reference.md)
- [Compilation Targets](targets.md)
- [Project Configuration](project-config.md)
