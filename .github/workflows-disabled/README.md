# Disabled GitHub Workflows

These workflows are temporarily disabled until the Seen Language compiler achieves self-hosting.

## Why Disabled?

Currently using the **Bootstrap Rust Compiler** (`seen_cli`) for development:
- Located at: `target/debug/seen_cli` or `target/release/seen_cli`
- Commands: `cargo run -p seen_cli -- build file.seen`

The disabled workflows expect a **Self-Hosted Seen Compiler**:
- Expected location: `./compiler_seen/target/native/release/seen_compiler` 
- Expected commands: `seen build file.seen`, `seen test`, `seen benchmark`

## Disabled Workflows:

### `test.yml`
- **Issue**: Looks for `./compiler_seen/target/native/release/seen_compiler` 
- **Commands**: `seen_compiler test --unit`, `seen_compiler benchmark`
- **Status**: Will be re-enabled when self-hosting is achieved

### `benchmark.yml`
- **Issue**: Expects `seen` binary and `seen build` commands
- **Commands**: `seen build benchmarks/harness/runner.seen --release`
- **Status**: Will be re-enabled when self-hosting is achieved

### `performance_monitoring.yml`
- **Issue**: Likely depends on self-hosted compiler
- **Status**: Will be re-enabled when self-hosting is achieved

### `release.yml`
- **Issue**: Expects production-ready self-hosted compiler
- **Status**: Will be re-enabled when self-hosting is achieved

## Still Active Workflows:

### `ci.yml` ✅ **ACTIVE**
- **Purpose**: Tests the Rust bootstrap compiler 
- **Commands**: `cargo test`, `cargo build`, `cargo clippy`
- **Status**: ✅ **Working** - Uses Rust/Cargo commands

## Re-enabling Process:

1. **Achieve Self-Hosting** (~6-8 weeks remaining)
   - Bootstrap compiler compiles Seen compiler written in Seen
   - Generate `seen` binary at expected paths
   - Verify all `seen` commands work

2. **Update Workflow Paths** 
   - Change paths from `./compiler_seen/target/native/release/seen_compiler`
   - To actual self-hosted binary location
   - Test all workflow commands

3. **Move Back to Active**
   ```bash
   mv .github/workflows-disabled/*.yml .github/workflows/
   ```

## Current Development Status:
- **Bootstrap Compiler**: 82% complete ✅
- **Core Language**: 90% complete (structs, enums, patterns) ✅  
- **Self-Hosting**: 0% complete ❌ **Next major milestone**

*Last Updated: August 14, 2025*