# Tooling

## VS Code Extension

The `vscode-seen/` directory contains a full-featured VS Code extension.

### Installation

```bash
cd vscode-seen
npm install
npm run package
code --install-extension seen-*.vsix
```

### Features

- **Syntax highlighting** -- TextMate grammar covering keywords, types, operators, strings, comments
- **IntelliSense** -- via built-in LSP (completions, hover, go-to-definition)
- **Real-time diagnostics** -- error and warning reporting
- **Code formatting** -- `Shift+Alt+F` formats the current file
- **Debugging** -- debug adapter for Seen programs
- **34 code snippets** -- common patterns (see below)

### Commands

| Command | Shortcut | Description |
|---------|----------|-------------|
| Seen: Build | `Ctrl+Shift+B` | Build project |
| Seen: Run | `F5` | Run project |
| Seen: Format | `Shift+Alt+F` | Format document |
| Seen: Check | -- | Type check project |
| Seen: Test | -- | Run tests |
| Seen: Clean | -- | Clean build artifacts |
| Seen: Init | -- | Initialize new project |
| Seen: REPL | -- | Open interactive REPL |
| Seen: Switch Language | -- | Change keyword language |

### Configuration

```json
{
    "seen.compiler.path": "seen",
    "seen.lsp.enabled": true,
    "seen.lsp.trace.server": "off",
    "seen.formatting.enable": true,
    "seen.target.default": "native",
    "seen.language.default": "en"
}
```

### Code Snippets

| Prefix | Description |
|--------|-------------|
| `main` | Main function entry point |
| `fun` | Function declaration |
| `class` | Class with constructor |
| `struct` | Struct declaration |
| `enum` | Enum declaration |
| `trait` | Trait declaration |
| `impl` | Trait implementation |
| `if` / `ife` | If / if-else |
| `for` | For-in loop |
| `while` | While loop |
| `when` / `match` | Pattern matching |
| `let` / `var` | Variable declarations |
| `async` | Async function |
| `compute` | GPU compute shader |
| `parallel_for` | Parallel for loop |
| `simd` | SIMD vector construction |
| `trycatch` | Try-catch block |
| `defer` / `errdefer` | Deferred cleanup |
| `extern` | External/FFI function |
| `derive` | Derive macro |
| `test` / `bench` | Test/benchmark function |
| `unsafe` | Unsafe block |
| `region` / `arena` | Memory region/arena |
| `println` | Print with newline |

## Language Server Protocol (LSP)

### Built-in LSP

```bash
seen lsp
```

Features:
- Diagnostics (errors and warnings)
- Completions
- Hover information
- Go-to-definition
- Find references
- Semantic tokens

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

## Code Formatting

```bash
seen fmt <source.seen>
```

Formatting rules:
- 4-space indentation
- Trim trailing whitespace
- Ensure final newline
- Configurable via `[format]` in `Seen.toml`:

```toml
[format]
line-width = 100
indent = 4
trailing-comma = true
```

## Testing

```bash
seen test
```

Write tests with the `@test` decorator:

```seen
@test
fun test_addition() {
    assert(1 + 1 == 2, "basic addition")
}

@test
fun test_string_concat() {
    let result = "hello" + " " + "world"
    assert(result == "hello world", "string concat")
}
```

Run end-to-end tests across all languages:

```bash
bash tests/e2e_multilang/run_all_e2e.sh
```

## Debugging

### Environment variable tracing

```bash
# Type checker
SEEN_DEBUG_TYPES=1 seen build program.seen

# LLVM IR generation
SEEN_TRACE_LLVM=all seen build program.seen

# Struct layout
SEEN_TRACE_LLVM=gep seen build program.seen
```

### LLVM IR inspection

```bash
seen build program.seen --emit-llvm
cat program.ll
```

### Debug symbols

```bash
seen build program.seen -g -o program
gdb ./program
```

### Compile database

```bash
seen build program.seen --emit-compile-db
# produces compile_commands.json
```

## Linux ARM64 Cross Sysroot

On pacman-compatible Linux hosts, you can create a local AArch64 cross sysroot for Seen without installing system packages globally:

```bash
./scripts/setup_linux_arm64_sysroot.sh
source artifacts/toolchains/linux-arm64/env.sh
```

The helper resolves the Arch/CachyOS cross-package URLs with `pacman -Sp`, downloads them locally, and extracts them under `artifacts/toolchains/linux-arm64/`. The generated `env.sh` exports both `SEEN_LINUX_ARM64_SYSROOT` and `SEEN_LINUX_ARM64_GCC_TOOLCHAIN`, which is enough for `compiler_seen/target/seen` and the native smoke harness to produce Linux ARM64 binaries on an x86_64 Linux host.

Validate the local setup with either command:

```bash
bash scripts/native_target_smoke.sh --compiler compiler_seen/target/seen --target linux-arm64
bash scripts/platform_matrix.sh --stage3 compiler_seen/target/seen --platform linux-arm64
```

If you want the helper to replace an existing extracted toolchain directory, rerun it with `--force`.

## Import from C

Generate Seen bindings from a C header:

```bash
seen import-c mylib.h
```

Outputs `extern fun` declarations that can be pasted into Seen source.

## Related

- [Getting Started](getting-started.md) -- editor setup
- [CLI Reference](cli-reference.md) -- all flags
- [Project Configuration](project-config.md) -- Seen.toml format
