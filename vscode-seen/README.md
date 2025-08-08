# Seen Language Support for Visual Studio Code

Official VSCode extension for the Seen programming language, providing comprehensive language support with advanced reactive programming features.

![Seen Language](https://via.placeholder.com/800x400/2E86C1/FFFFFF?text=Seen+Language+VSCode+Extension)

## Features

### 🚀 Core Language Support
- **Syntax Highlighting** - Rich syntax highlighting with TextMate grammar
- **IntelliSense** - Smart code completion with type information
- **Error Detection** - Real-time diagnostics and error checking
- **Go to Definition** - Navigate to symbol definitions
- **Find References** - Find all references to symbols
- **Symbol Outline** - Navigate code structure with symbol tree
- **Code Formatting** - Automatic code formatting with `seen format`

### ⚡ Reactive Programming
- **Stream Visualization** - Interactive marble diagrams for reactive streams
- **Real-time Debugging** - Inspect reactive stream values during debugging
- **Operator Documentation** - Hover help for reactive operators
- **Stream State Inspection** - View current stream states inline

### 📊 Performance & Benchmarking
- **Inline Benchmarks** - Run benchmarks directly from editor with CodeLens
- **Performance Results** - View benchmark results in integrated terminal
- **Benchmark History** - Track performance changes over time
- **Comparison Tools** - Compare performance across different implementations

### 🛠️ Build & Development
- **Build Integration** - Build projects with `Ctrl+Shift+B`
- **Run & Test** - Execute programs and tests with `F5`
- **REPL Integration** - Interactive development with integrated REPL
- **Multi-target Support** - Build for native, WASM, RISC-V, and mobile targets

### 🌍 Multi-language Support
- **Arabic Support** - Full support for Arabic language projects
- **Language Switching** - Dynamic language switching for international development
- **Code Translation** - Translate code between English and Arabic
- **Localized Errors** - Error messages in multiple languages

### 🐛 Debugging
- **Breakpoints** - Set breakpoints and step through code
- **Variable Inspection** - Inspect variable values during debugging
- **Call Stack** - Navigate the call stack
- **Debug Console** - Interactive debugging console

## Quick Start

1. **Install the Extension**: Search for "Seen Language" in the VSCode marketplace
2. **Install Seen Compiler**: Download from [seen-lang.org](https://seen-lang.org)
3. **Create New Project**: Use `Ctrl+Shift+P` → "Seen: Initialize New Project"
4. **Start Coding**: Open a `.seen` file and start developing!

## Commands

| Command | Shortcut | Description |
|---------|----------|-------------|
| `Seen: Build Project` | `Ctrl+Shift+B` | Build the current project |
| `Seen: Run Project` | `F5` | Run the current project |
| `Seen: Run Tests` | - | Execute all tests |
| `Seen: Run Benchmarks` | - | Run performance benchmarks |
| `Seen: Format Document` | `Shift+Alt+F` | Format current document |
| `Seen: Initialize New Project` | - | Create a new Seen project |
| `Seen: Open REPL` | - | Start interactive REPL |
| `Seen: Visualize Reactive Stream` | - | Show reactive stream marble diagram |

## Configuration

Configure the extension in your VSCode settings:

```json
{
  "seen.compiler.path": "seen",
  "seen.lsp.enabled": true,
  "seen.formatting.enable": true,
  "seen.reactive.marbleDiagrams": true,
  "seen.benchmark.showInline": true,
  "seen.target.default": "native",
  "seen.language.default": "en"
}
```

### Available Settings

- **seen.compiler.path** - Path to the Seen compiler executable
- **seen.lsp.enabled** - Enable/disable the Language Server Protocol
- **seen.lsp.trace.server** - LSP communication tracing level
- **seen.formatting.enable** - Enable automatic code formatting
- **seen.reactive.marbleDiagrams** - Show reactive stream visualizations
- **seen.benchmark.showInline** - Show benchmark CodeLens in editor
- **seen.target.default** - Default compilation target
- **seen.language.default** - Default project language (en/ar)

## Language Features

### Syntax Highlighting

The extension provides rich syntax highlighting for:
- Keywords and control structures
- Data types and primitives
- String interpolation and templates
- Comments and documentation
- Reactive programming constructs
- Annotations and attributes

### Code Snippets

Built-in code snippets for common patterns:
- `main` - Main function template
- `fun` - Function declaration
- `data` - Data class
- `sealed` - Sealed class
- `struct` - Struct declaration
- `enum` - Enum declaration
- `test` - Test function
- `bench` - Benchmark function
- `observable` - Observable creation
- `flow` - Flow creation

### Reactive Programming

Advanced support for reactive programming:
- Marble diagrams for stream visualization
- Operator hover documentation
- Stream state inspection during debugging
- Real-time value tracking

## Requirements

- **Visual Studio Code** 1.75.0 or higher
- **Seen Compiler** latest version
- **Node.js** 16.0 or higher (for extension development)

## Installation

### From Marketplace
1. Open VSCode
2. Go to Extensions (Ctrl+Shift+X)
3. Search for "Seen Language"
4. Click Install

### From VSIX
1. Download the `.vsix` file
2. Open VSCode
3. Run `Extensions: Install from VSIX...`
4. Select the downloaded file

## Project Structure

```
my-seen-project/
├── Seen.toml           # Project configuration
├── src/                # Source files
│   └── main.seen       # Main entry point
├── tests/              # Test files
├── benchmarks/         # Benchmark files
└── languages/          # Language configurations
    ├── en.toml         # English configuration
    └── ar.toml         # Arabic configuration
```

## Troubleshooting

### Extension Not Working
- Ensure Seen compiler is installed and in PATH
- Check `seen.compiler.path` setting
- Restart VSCode after configuration changes

### Language Server Issues
- Enable LSP tracing: set `seen.lsp.trace.server` to "verbose"
- Check Output panel → "Seen Language Server"
- Ensure project has valid `Seen.toml` file

### Build Errors
- Verify project structure matches requirements
- Check for syntax errors in `.seen` files
- Ensure all dependencies are available

## Contributing

We welcome contributions! Please see our [Contributing Guide](https://github.com/seen-lang/vscode-seen/blob/main/CONTRIBUTING.md) for details.

### Development Setup

1. Clone the repository
2. Run `npm install`
3. Open in VSCode
4. Press `F5` to launch extension development host

## License

This extension is licensed under the MIT License. See [LICENSE](LICENSE) for details.

## Links

- **Website**: [seen-lang.org](https://seen-lang.org)
- **GitHub**: [github.com/seen-lang](https://github.com/seen-lang)
- **Documentation**: [docs.seen-lang.org](https://docs.seen-lang.org)
- **Community**: [discord.gg/seen-lang](https://discord.gg/seen-lang)

---

**Enjoy coding with Seen! 🚀**