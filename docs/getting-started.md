# Getting Started

## Install a Release

The Linux and Windows release packages are built to include the compiler and
the toolchain pieces needed by normal users, including LLVM tools where the
package format supports bundling them. After installation, verify the compiler
is on your PATH:

```bash
seen --version
```

The shipped binary also prints command usage when no command is supplied:

```bash
seen
```

## Build from Source

Source builds still need local build tools because they rebuild the compiler and
runtime:

- LLVM 18+ with `clang`, `opt`, `llc`, `llvm-as`, and `lld`
- GCC or a compatible C compiler for runtime objects
- Git

On Ubuntu/Debian:

```bash
sudo apt install llvm-18 clang-18 lld-18 gcc git
```

On Arch Linux:

```bash
sudo pacman -S llvm clang lld gcc git
```

On macOS:

```bash
brew install llvm gcc git
```

Build the self-hosted compiler:

```bash
git clone https://github.com/codeyousef/SeenLang.git
cd SeenLang
./scripts/safe_rebuild.sh
```

The production compiler lands at `compiler_seen/target/seen`. Follow the
repository rebuild rules when running this script: derive and set explicit
memory limits rather than running an uncapped rebuild.

## Hello World

Create `hello.seen`:

```seen
fun main() {
    println("Hello, Seen!")
}
```

Compile and run:

```bash
seen compile hello.seen hello
./hello
```

Or compile and execute in one step:

```bash
seen run hello.seen
```

## Your First Project

A Seen project uses `Seen.toml` for configuration:

```text
my_project/
├── Seen.toml
├── src/
│   └── main.seen
└── tests/
    └── test_main.seen
```

Minimal `Seen.toml`:

```toml
[project]
name = "my_project"
version = "0.1.0"
language = "en"

[registries]
default = "https://seen.dev.yousef.codes/packages"

[dependencies]

[native.dependencies]
```

The `language` field sets the keyword language. Supported languages are `en`,
`ar`, `es`, `ru`, `zh`, and `ja`.

Example program:

```seen
class Counter {
    var count: Int

    static fun new() r: Counter {
        return Counter { count: 0 }
    }

    fun increment() {
        this.count = this.count + 1
    }

    fun value() r: Int {
        return this.count
    }
}

fun main() {
    let counter = Counter.new()
    var i = 0
    while i < 10 {
        counter.increment()
        i = i + 1
    }
    println("Count: {counter.value()}")
}
```

Compile:

```bash
seen compile src/main.seen my_project
./my_project
```

## Editor Setup

### VS Code

```bash
cd vscode-seen
npm install
npm run package
code --install-extension seen-*.vsix
```

The extension provides syntax highlighting, snippets, tasks, and LSP-backed
diagnostics/completions through the shipped `seen lsp` server.

### Any Editor With LSP

```bash
seen lsp
```

Neovim:

```lua
require'lspconfig'.seen.setup{
  cmd = {"seen", "lsp"},
  filetypes = {"seen"},
  root_dir = require'lspconfig.util'.root_pattern("Seen.toml", ".git"),
}
```

Emacs:

```elisp
(lsp-register-client
 (make-lsp-client :new-connection (lsp-stdio-connection '("seen" "lsp"))
                  :major-modes '(seen-mode)
                  :server-id 'seen-lsp))
```

## Writing in Other Languages

Arabic hello world:

```seen
دالة main() {
    println("!مرحبا، سين")
}
```

Compile with the language flag:

```bash
seen compile hello_ar.seen hello --language ar
```

See [Multi-Language Support](multilingual.md) for translation tables.

## Next Steps

- [Language Guide](language-guide.md) -- syntax and semantics
- [CLI Reference](cli-reference.md) -- shipped compiler commands and flags
- [API Reference](api-reference/index.md) -- standard library documentation
