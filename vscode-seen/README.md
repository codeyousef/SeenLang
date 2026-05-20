<p align="center">
  <img src="images/icon.png" alt="Seen Language" width="100">
</p>

<h1 align="center">Seen Language for Visual Studio Code</h1>

<p align="center">
  Official extension for the <a href="https://github.com/codeyousef/seenlang">Seen programming language</a> -- syntax highlighting, LSP, debugging, and multi-language keyword support.
</p>

---

## Features

- **Syntax Highlighting** -- TextMate grammar for `.seen` files with support for `///` block comments, `@export`, SIMD types, GPU annotations, async/await, performance collection types, and all 6 keyword languages
- **IntelliSense** -- Code completion and type information via the built-in LSP
- **Error Diagnostics** -- Real-time error checking as you type
- **Go to Definition / Find References** -- Navigate your codebase
- **Code Formatting** -- Format with `Shift+Alt+F` through LSP formatting
- **Debugging** -- Breakpoints, stepping, variable inspection
- **REPL** -- Interactive Seen session in the terminal
- **Compile Integration** -- Compile, run, check, and package from the editor
- **Shared Module Builds** -- Compile PIC objects and object manifests for hot-reload/shared-library workflows
- **Package Workflows** -- Run `seen pkg fetch`, `pack`, `prebuild`, and `publish` from VS Code
- **Target Awareness** -- Configuration choices track the shipped native and cross targets, including `linux-riscv64`
- **Multi-Language Keywords** -- Switch between English, Arabic, Spanish, Russian, Chinese, and Japanese keywords

## Quick Start

1. Install the extension (search "Seen Language" in the marketplace, or install from `.vsix`)
2. Install the [Seen compiler](https://github.com/codeyousef/seenlang) (syntax highlighting works without it)
3. Open a `.seen` file and start coding

## Commands

| Command | Shortcut | Description |
|---------|----------|-------------|
| Seen: Compile Current File | `Ctrl+Shift+B` | Compile the active Seen source file |
| Seen: Run Project | `F5` | Execute the program |
| Seen: Format Document | `Shift+Alt+F` | Format the current file |
| Seen: Check Project | -- | Type-check without compiling |
| Seen: Compile Shared Module Objects | -- | Emit PIC objects plus an object manifest |
| Seen: Package Fetch | -- | Fetch package dependencies |
| Seen: Package Pack | -- | Create a package archive |
| Seen: Package Prebuild | -- | Emit prebuilt package artifacts |
| Seen: Package Publish | -- | Publish to a local static registry |
| Seen: Initialize New Project | -- | Scaffold a new Seen project |
| Seen: Open REPL | -- | Launch interactive REPL |
| Seen: Switch Project Language | -- | Change keyword language |
| Seen: Translate to Another Language | -- | Translate code keywords |

## Snippets

| Prefix | Description |
|--------|-------------|
| `main` | Main function entry point |
| `fun` / `funv` | Function with/without return type |
| `class` | Class with constructor |
| `struct` / `data` | Struct / data record |
| `enum` | Enum declaration |
| `trait` / `impl` | Trait declaration / implementation |
| `if` / `ife` | If / if-else |
| `for` / `while` | Loops |
| `match` / `when` | Pattern matching |
| `let` / `var` | Variable bindings |
| `test` | Test function with `@test` |
| `extern` | External function (FFI) |
| `export` | Exported function with a stable native symbol |
| `effect` / `using` | Capability effect and `@using(...)` annotation |
| `pkgimport` / `pkgfrom` | Package-root and package module imports |
| `in` | Membership test |
| `sealed` | Sealed class declaration |
| `comment` | `///` multi-line block comment |
| `hotreload` / `hotmodule` / `hotcall` | Hot reload import, load, and Int-call helpers |
| `stringbuilder` / `bytebuffer` | StringBuilder and byte-backed buffer setup |
| `hashmap` / `priorityqueue` | HashMap lookup and priority queue setup |
| `closure` | Closure expression `\|x\| expr` |
| `parallel_for` | Parallel for loop |
| `compute` | GPU compute kernel with `@compute` |
| `derive` | `@derive(...)` annotation |
| `trycatch` | Try-catch block |
| `defer` | Defer block |
| `println` | Print with newline |
| `lambda` | Lambda expression |
| `static` / `method` / `ext` | Static / instance / extension methods |
| `import` | Import statement |

## Configuration

```json
{
  "seen.compiler.path": "seen",
  "seen.lsp.enabled": true,
  "seen.formatting.enable": true,
  "seen.target.default": "native",
  "seen.compile.pic": false,
  "seen.compile.objectManifest": "",
  "seen.language.default": "en"
}
```

| Setting | Default | Description |
|---------|---------|-------------|
| `seen.compiler.path` | `"seen"` | Path to the Seen compiler |
| `seen.lsp.enabled` | `true` | Enable language server |
| `seen.lsp.trace.server` | `"off"` | LSP tracing (`off`, `messages`, `verbose`) |
| `seen.formatting.enable` | `true` | Enable code formatting |
| `seen.target.default` | `"native"` | Compilation target: `native`, `linux-x86_64`, `linux-arm64`, `linux-riscv64`, `windows-x86_64`, `macos-x86_64`, `macos-arm64`, `ios-arm64`, `ios-sim-arm64`, or `android-arm64` |
| `seen.compile.pic` | `false` | Emit PIC objects for shared-library builds |
| `seen.compile.objectManifest` | `""` | Optional manifest path for shared-module object builds |
| `seen.language.default` | `"en"` | Keyword language (`en`, `ar`, `es`, `ru`, `zh`, `ja`) |

## Troubleshooting

**Extension not detecting compiler** -- Set `seen.compiler.path` to the full path of your `seen` binary, or ensure it's in your `PATH`.

**LSP not starting** -- Check the Output panel ("Seen Language Server"). Make sure your project has a `Seen.toml` file. Try setting `seen.lsp.trace.server` to `"verbose"`.

**Syntax highlighting looks wrong** -- Reload the window (`Ctrl+Shift+P` > "Reload Window"). If using non-English keywords, set `seen.language.default` to match.

## Development

```bash
cd vscode-seen
npm install
# Press F5 in VS Code to launch extension development host
```

To package:

```bash
npm run package
code --install-extension seen-*.vsix
```

## Requirements

- VS Code 1.75.0+
- Seen compiler (optional -- syntax highlighting works without it)

## License

MIT -- see [LICENSE](LICENSE).

**Source**: [github.com/codeyousef/seenlang](https://github.com/codeyousef/seenlang)
