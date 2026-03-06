# Contributing to Seen VSCode Extension

Thank you for your interest in contributing to the Seen VSCode extension! This document provides guidelines and information for contributors.

## Development Setup

### Prerequisites
- **Node.js** 18.x or higher
- **VSCode** 1.75.0 or higher
- **Seen Compiler** (latest version)
- **Git**

### Setup Steps

1. **Clone the repository**
   ```bash
   git clone https://github.com/seen-lang/vscode-seen.git
   cd vscode-seen
   ```

2. **Install dependencies**
   ```bash
   npm install
   ```

3. **Build the extension**
   ```bash
   npm run compile
   ```

4. **Run tests**
   ```bash
   npm test
   ```

## Development Workflow

### Running the Extension
1. Open the project in VSCode
2. Press `F5` to launch Extension Development Host
3. The extension will be loaded in the new VSCode window
4. Open a `.seen` file to test functionality

### Making Changes
1. **Create a feature branch**
   ```bash
   git checkout -b feature/your-feature-name
   ```

2. **Make your changes**
   - Follow TypeScript coding standards
   - Add tests for new features
   - Update documentation as needed

3. **Run tests and linting**
   ```bash
   npm run lint
   npm test
   ```

4. **Commit changes**
   ```bash
   git add .
   git commit -m "feat: add your feature description"
   ```

5. **Create pull request**

### Project Structure

```
vscode-seen/
├── src/                          # TypeScript source files
│   ├── extension.ts              # Main extension entry point
│   ├── commands.ts               # Command implementations
│   ├── reactive.ts               # Reactive programming features
│   ├── benchmark.ts              # Benchmark integration
│   ├── debugger.ts               # Debug adapter
│   ├── repl.ts                   # REPL integration
│   └── test/                     # Test files
├── syntaxes/                     # TextMate grammars
├── snippets/                     # Code snippets
├── test-fixtures/                # Test files
├── .github/workflows/            # CI/CD workflows
└── package.json                  # Extension manifest
```

## Coding Standards

### TypeScript Guidelines
- Use TypeScript strict mode
- Prefer `const` over `let`
- Use meaningful variable names
- Add JSDoc comments for public APIs
- Follow async/await patterns

### Code Style
- Use ESLint configuration provided
- 4-space indentation
- Single quotes for strings
- Trailing commas in objects/arrays
- Line length limit of 120 characters

### Example:
```typescript
/**
 * Executes a Seen command with error handling
 */
async function executeSeenCommand(command: string, args: string[]): Promise<string> {
    try {
        const result = await runCommand(command, args);
        return result.stdout;
    } catch (error) {
        vscode.window.showErrorMessage(`Command failed: ${error.message}`);
        throw error;
    }
}
```

## Testing Guidelines

### Unit Tests
- Write tests for all new functionality
- Use Mocha test framework
- Place tests in `src/test/suite/`
- Follow naming convention: `*.test.ts`

### Integration Tests
- Test VSCode extension APIs
- Test LSP integration
- Test command execution

### Test Example:
```typescript
suite('Command Tests', () => {
    test('Build command should execute without errors', async () => {
        const result = await vscode.commands.executeCommand('seen.build');
        assert.ok(result !== undefined);
    });
});
```

## Feature Development

### Adding New Commands
1. Add command to `package.json` contributions
2. Implement in `src/commands.ts`
3. Register in `src/extension.ts`
4. Add tests
5. Update README

### Adding Language Features
1. Update TextMate grammar in `syntaxes/`
2. Add code snippets if applicable
3. Test syntax highlighting
4. Update language configuration

### Adding Reactive Features
1. Extend `ReactiveVisualizer` class
2. Add webview HTML/CSS/JS
3. Integrate with LSP notifications
4. Add marble diagram support

## Release Process

### Version Bumping
1. Update version in `package.json`
2. Update CHANGELOG.md
3. Create git tag: `git tag v1.0.1`
4. Push tag: `git push origin v1.0.1`

### Automated Release
- GitHub Actions automatically:
  - Runs tests
  - Packages extension
  - Creates GitHub release
  - Publishes to VSCode Marketplace
  - Publishes to Open VSX Registry

### Manual Release
```bash
# Package extension
npm run package

# Publish to marketplace
npx vsce publish

# Publish to Open VSX
npx ovsx publish
```

## Debugging

### Extension Debugging
1. Set breakpoints in TypeScript files
2. Press `F5` to launch debug session
3. Debug in Extension Development Host

### LSP Debugging
1. Enable LSP tracing in settings
2. Check "Seen Language Server" output channel
3. Use `seen lsp --help` for CLI debugging

### Common Issues

**Extension not activating:**
- Check activation events in package.json
- Verify Seen compiler installation
- Check VSCode Developer Console

**LSP not working:**
- Verify `seen lsp` command works
- Check LSP server logs
- Ensure Seen.toml exists in workspace

**Tests failing:**
- Run tests with `--verbose` flag
- Check test fixtures are valid
- Verify VSCode test environment

## Documentation

### Code Documentation
- Add JSDoc comments for all public functions
- Document complex algorithms
- Include usage examples

### User Documentation
- Update README.md for user-facing changes
- Add configuration examples
- Update feature descriptions

## Community Guidelines

### Code of Conduct
- Be respectful and inclusive
- Help others learn and contribute
- Focus on constructive feedback
- Report inappropriate behavior

### Issue Reporting
- Use issue templates
- Provide minimal reproduction steps
- Include system information
- Check for existing issues first

### Pull Request Guidelines
- Reference related issues
- Provide clear description
- Include tests for changes
- Keep changes focused and small
- Respond to review feedback promptly

## Getting Help

- **Discord**: [discord.gg/seen-lang](https://discord.gg/seen-lang)
- **Issues**: [GitHub Issues](https://github.com/seen-lang/vscode-seen/issues)
- **Documentation**: [docs.seen-lang.org](https://docs.seen-lang.org)

Thank you for contributing to the Seen language ecosystem! 🚀