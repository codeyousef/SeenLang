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
  imports, effects, hot reload helpers, and multilingual keywords.
- LSP-backed diagnostics, completions, hover, definitions, references, rename,
  and document symbols through `seen lsp`.
- Build/check/run/package tasks using the shipped `seen compile`, `seen check`,
  `seen run`, and `seen pkg` commands.
- Snippets for functions, classes, structs, enums, traits/interfaces, effects,
  packages, shared modules, hot reload, GPU/SIMD, defer/errdefer, and FFI.

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
| Seen: Package Publish | `seen pkg publish` |
| Seen: Translate to Another Language | `seen translate <file> --from <lang> --to <lang>` |

Extension-only commands such as update checks or visual helpers are editor UI
features; compiler behavior remains controlled by the `seen` CLI.

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
capabilities, hot reload, imports, sealed classes, exports, and stdlib modules.

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
- [Project Configuration](project-config.md)
