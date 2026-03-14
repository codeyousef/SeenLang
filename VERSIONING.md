# Versioning Policy

This project follows [Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html) with compiler-specific interpretations.

Format: **MAJOR.MINOR.PATCH**

## Pre-1.0 (current)

While below 1.0, the language spec is unstable. Minor versions may include breaking changes to syntax or semantics. The rules below describe intent, not hard guarantees.

## MAJOR (X.0.0)

Bump when the language makes a backward-incompatible change to stabilized features. Programs that compiled under the previous major version may require source changes.

Examples:
- Removing or renaming a keyword
- Changing the semantics of an existing operator
- Removing a stdlib function that was marked stable
- Changing the default calling convention or ABI

The bump from 0.x to 1.0 means: the core language spec is frozen. Existing programs will not break without a deprecation cycle.

## MINOR (0.X.0)

Bump when new functionality is added in a backward-compatible way. Existing programs continue to compile and behave identically.

Examples:
- New keyword or syntax (e.g., `comptime`, `where` clauses)
- New stdlib module or function
- New CLI subcommand (e.g., `seen fmt`, `seen lsp`)
- New compiler flag (e.g., `--null-safety`, `--emit-glsl`)
- New target platform or cross-compilation support
- New language keyword translations (adding a 7th language)

## PATCH (0.0.X)

Bump when existing functionality is fixed or improved without changing the public interface. Existing programs compile and behave the same or better.

Examples:
- Codegen bug fix (wrong IR, type mismatch, linker error)
- Optimizer fix (loop miscompilation, incorrect flag propagation)
- Runtime bug fix (HashMap, Array, StringBuilder)
- Performance improvement (faster compilation, smaller binaries)
- Build system fix (path resolution, caching, stale artifacts)
- Compiler crash fix
- Error message improvement

## What doesn't require a version bump

- Internal refactoring (splitting codegen modules, renaming internal variables)
- Documentation changes
- Test additions
- CI/CD changes
- Development tooling (VSCode extension versioned separately)

## Release process

1. Update `CHANGELOG.md` with the new version entry
2. Tag the commit: `git tag v0.X.Y`
3. Run `./scripts/safe_rebuild.sh` to verify bootstrap
4. Run `bash tests/e2e_multilang/run_all_e2e.sh` to verify 66/66 tests pass
