# Getting Started

## Prerequisites

- **LLVM 18+** with `clang`, `opt`, and `lld`
- **GCC** (for runtime compilation)
- **Git**

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

## Build from Source

```bash
git clone https://github.com/codeyousef/SeenLang.git
cd SeenLang
./scripts/safe_rebuild.sh
```

The production compiler lands at `compiler_seen/target/seen`.

## Install

Copy the binary to your PATH:

```bash
sudo cp compiler_seen/target/seen /usr/local/bin/seen
```

Or add the project directory:

```bash
export PATH="$PATH:/path/to/SeenLang/compiler_seen/target"
```

## Hello World

Create `hello.seen`:

```seen
fun main() {
    println("Hello, Seen!")
}
```

Compile and run:

```bash
seen build hello.seen -o hello
./hello
```

Or use JIT execution (no binary produced):

```bash
seen run hello.seen
```

## Your First Project

### Project Structure

A Seen project uses `Seen.toml` for configuration:

```
my_project/
├── Seen.toml
├── src/
│   └── main.seen
└── tests/
    └── test_main.seen
```

### Seen.toml

```toml
[project]
name = "my_project"
version = "0.1.0"
language = "en"

[dependencies]
seen_std = "../seen_std"
```

The `language` field sets the keyword language. Options: `en`, `ar`, `es`, `ru`, `zh`, `fr`.

### A Bigger Example

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
seen build src/main.seen -o my_project
./my_project
```

## Editor Setup

### VS Code (Recommended)

```bash
cd vscode-seen
npm install
npm run package
code --install-extension seen-*.vsix
```

The extension provides:
- Syntax highlighting
- IntelliSense via built-in LSP
- Real-time error diagnostics
- Code formatting
- 34 code snippets

### Any Editor (LSP)

Seen includes a built-in language server:

```bash
seen lsp
```

**Neovim:**

```lua
require'lspconfig'.seen.setup{
  cmd = {"seen", "lsp"},
  filetypes = {"seen"},
  root_dir = require'lspconfig.util'.root_pattern("Seen.toml", ".git"),
}
```

**Emacs:**

```elisp
(lsp-register-client
 (make-lsp-client :new-connection (lsp-stdio-connection '("seen" "lsp"))
                  :major-modes '(seen-mode)
                  :server-id 'seen-lsp))
```

## Writing in Other Languages

The same hello world in Arabic:

```seen
دالة main() {
    println("!مرحبا، سين")
}
```

Compile with language flag:

```bash
seen build hello_ar.seen -o hello --language ar
```

See [Multi-Language Support](multilingual.md) for full translation tables.

## Next Steps

- [Language Guide](language-guide.md) -- complete syntax reference
- [CLI Reference](cli-reference.md) -- all compiler commands and flags
- [API Reference](api-reference/index.md) -- standard library documentation
