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

- LLVM 19+ with `clang`, `opt`, `llc`, `llvm-as`, and `lld` (LLVM 20 is
  preferred and used by hosted CI)
- GCC or a compatible C compiler for runtime objects
- Go 1.26+ for the version-matched `seen-pkg` helper
- Git

On Ubuntu/Debian:

```bash
sudo apt install llvm-20 clang-20 lld-20 libclang-rt-20-dev gcc git
```

On Arch Linux:

```bash
sudo pacman -S llvm clang lld gcc git
```

On macOS:

```bash
brew install llvm gcc git
```

After cloning the repository, follow the complete capped command in
[Bootstrap System](bootstrap.md). Use an explicit tier: `--tier quick` produces
the development compiler, while `--tier verify` produces the verified
`compiler_seen/target/seen`. Never invoke the rebuild script uncapped.

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
manifest-version = 1

[project]
name = "my_project"
version = "0.1.0"
language = "en"

[dependencies]

[native.dependencies]
```

`manifest-version = 1` is the current manifest schema. Keep it at the top of
new manifests. Publishing also requires the strict package/dependency fields in
the packaging guide.

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

## Add Package Dependencies

Local source packages can be declared directly:

```toml
[dependencies]
gamekit = { path = "../gamekit" }
```

For a signed registry, configure its canonical HTTPS origin and use a local
alias for each canonical `owner/name` package identity:

```toml
manifest-version = 1

[registries]
default = "https://seen.dev.yousef.codes/packages"

[dependencies]
calc = { package = "alice/mathx", version = "^1.2.0", allow = ["file"] }

[package-grants]
"alice/mathx" = ["file"]
```

Fetch and lock the dependency graph, then verify reproducible use:

```bash
seen pkg fetch
seen check src/main.seen --locked
seen compile src/main.seen my_project --frozen
```

`compile`, `check`, and `run` prepare declared dependencies automatically. The
development origin in this example serves public signed metadata and catalog
reads, and the client embeds its official trust root. No submitted release is
currently resolver-visible because promotion remains disabled. See
[Packaging](packaging.md) for custom signed registries, lock modes, and
capability consent.

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
