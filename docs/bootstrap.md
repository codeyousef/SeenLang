# Bootstrap System

The Seen compiler is self-hosted: it compiles itself. The bootstrap system ensures this property is maintained safely across changes.

## What is Bootstrap?

A self-hosted compiler can compile its own source code. To verify this:

1. **Stage 1** (frozen): A known-good compiler binary
2. **Stage 2**: Compile the source with Stage 1
3. **Stage 3**: Compile the source with Stage 2
4. If **Stage 2 == Stage 3**, the compiler is at a fixed-point -- it reproduces itself

This proves the compiler is correct and self-consistent.

## Key Files

| File | Purpose |
|------|---------|
| `bootstrap/stage1_frozen` | Frozen, verified bootstrap compiler |
| `bootstrap/stage1_frozen.sha256` | SHA-256 hash for integrity |
| `compiler_seen/target/seen` | Production compiler |
| `scripts/safe_rebuild.sh` | Safe rebuild with verification |
| `.githooks/pre-commit` | Automatic bootstrap check on commit |

## Safe Rebuild

The recommended way to rebuild after compiler changes:

```bash
./scripts/safe_rebuild.sh
```

This script:
1. Builds Stage 2 from `bootstrap/stage1_frozen`
2. Builds Stage 3 from Stage 2
3. Compares Stage 2 and Stage 3
4. If identical, updates `compiler_seen/target/seen`
5. If different, reports failure and does not update

### Expected Results

- **S2 == S3**: Bootstrap verified. The compiler is self-consistent.
- **S2 != S3**: The frozen compiler is stale or changes broke self-hosting.

## Pre-Commit Hook

Enable automatic bootstrap verification on commit:

```bash
git config core.hooksPath .githooks
```

The hook:
1. Detects changes to `compiler_seen/src/`
2. Runs bootstrap verification
3. Blocks commits that break self-hosting

## Development Workflow

1. Make changes to compiler source (`compiler_seen/src/`)
2. Run `./scripts/safe_rebuild.sh`
3. If verification passes, commit
4. If it fails, fix the issue before committing

## Updating the Frozen Compiler

Only update when you have a verified working compiler:

```bash
# 1. Verify current compiler passes bootstrap
./scripts/safe_rebuild.sh

# 2. If successful, update frozen compiler
cp stage2_head bootstrap/stage1_frozen
sha256sum bootstrap/stage1_frozen > bootstrap/stage1_frozen.sha256

# 3. Commit
git add bootstrap/
git commit -m "Update frozen bootstrap compiler"
```

## Manual Bootstrap

Build stages manually:

```bash
# Stage 2: frozen compiler builds the source
./bootstrap/stage1_frozen compile compiler_seen/src/main_compiler.seen stage2_new

# Stage 3: stage2 builds the source
./stage2_new compile compiler_seen/src/main_compiler.seen stage3_new

# Verify
diff stage2_new stage3_new  # should be identical
```

## Emergency Recovery

If the production compiler is broken:

```bash
# Option 1: Use the frozen bootstrap compiler
./bootstrap/stage1_frozen compile compiler_seen/src/main_compiler.seen recovery_compiler

# Option 2: Check out a known-good commit
git checkout ead1940 -- compiler_seen/src/
./bootstrap/stage1_frozen compile compiler_seen/src/main_compiler.seen recovery_compiler
```

## Adding New Features to the Compiler

Some features require a two-phase bootstrap:

1. Add a stub implementation
2. Rebuild and update frozen compiler
3. Add the real implementation
4. Rebuild again

This is necessary when the new feature uses syntax or constructs that the current frozen compiler doesn't understand.

## Common Bootstrap Issues

| Problem | Cause | Solution |
|---------|-------|----------|
| "undefined value" error | New method not in old compiler | Two-phase bootstrap (stub first) |
| GEP index out of bounds | Struct field count mismatch | Update `struct_layouts.seen`, verify field order |
| S2 != S3 != S4 | Non-deterministic codegen | Check for HashMap/time usage in compiler |
| Cache invalidation | Changed declarations | Clear `.seen_cache/` and `/tmp/seen_ir_cache` |

## Related

- [Compiler Architecture](compiler-architecture.md) -- pipeline internals
- [Known Limitations](known-limitations.md) -- current bugs
