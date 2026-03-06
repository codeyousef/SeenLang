# Bootstrap Compiler

This directory contains the frozen bootstrap compiler for the Seen language.

## Files

- `stage1_frozen` - Verified working self-hosted Seen compiler (Linux x86_64)
- `stage1_frozen.sha256` - SHA256 hash for verification
- `source_commit.txt` - Git commit from which this binary was built

## Usage

### Compiling Programs

```bash
./bootstrap/stage1_frozen compile source.seen output
```

### Rebuilding the Compiler

To rebuild the self-hosted compiler from scratch:

```bash
# Use the safe rebuild script (recommended)
./scripts/safe_rebuild.sh

# Or manually:
# Build stage2 from frozen stage1
./bootstrap/stage1_frozen compile compiler_seen/src/main_compiler.seen stage2_new

# Build stage3 from stage2 (should match stage2 if bootstrap is stable)
./stage2_new compile compiler_seen/src/main_compiler.seen stage3_new

# Verify bootstrap
diff stage2_new stage3_new
# If files differ, bootstrap verification failed
```

### Verifying the Frozen Compiler

```bash
sha256sum -c bootstrap/stage1_frozen.sha256
```

## Bootstrap Verification

This compiler achieves bootstrap at stage2 == stage3:
- Built from the current HEAD sources
- Produces identical binaries when recompiling itself
- Verified using `./scripts/safe_rebuild.sh`

## Current Features

This version includes:
- Full two-pass memory-optimized compilation
- Determinism checking with `--profile deterministic`
- `setPanicOnOverflow` method for overflow checking
- LLVM IR generation with forward declarations for all known methods

## Updating the Frozen Compiler

Only update when:
1. You have a verified working compiler that passes bootstrap
2. The new compiler is tested thoroughly
3. You have verified the new compiler can rebuild from scratch

To update:

```bash
# 1. Build and verify new compiler achieves fixed-point
./scripts/safe_rebuild.sh

# 2. If successful, update frozen compiler
cp compiler_seen/target/seen bootstrap/stage1_frozen
sha256sum bootstrap/stage1_frozen > bootstrap/stage1_frozen.sha256
git rev-parse --short HEAD > bootstrap/source_commit.txt

# 3. Commit
git add bootstrap/
git commit -m "Update frozen bootstrap compiler"
```

## Adding New Methods (Bootstrap-Safe Pattern)

When adding new methods to core classes like `LLVMIRGenerator`:

1. **Step 1**: Add the method definition and a forward declaration in `llvm_ir_gen.seen`
   - Add the method to the class
   - Add `declare void @ClassName_methodName(...)` to the hardcoded declarations list
   - Do NOT call the method yet

2. **Step 2**: Run `./scripts/safe_rebuild.sh` and update frozen compiler
   ```bash
   ./scripts/safe_rebuild.sh
   cp compiler_seen/target/seen bootstrap/stage1_frozen
   sha256sum bootstrap/stage1_frozen > bootstrap/stage1_frozen.sha256
   ```

3. **Step 3**: Now you can add calls to the method
   - The frozen compiler now knows about the method
   - Add calls in other files

4. **Step 4**: Run `./scripts/safe_rebuild.sh` again to verify

## Architecture Notes

The frozen compiler is built for Linux x86_64. For other platforms, use QEMU/Docker to run the Linux binary.

## Emergency Recovery

If bootstrap is broken:

```bash
# Use the frozen compiler directly
./bootstrap/stage1_frozen compile compiler_seen/src/main_compiler.seen recovery_compiler
```
