# Bootstrap Stage-1 Blocker Analysis
**Date**: 2025-01-13  
**Status**: BLOCKED - Typechecker struct field resolution issue

## Summary

Stage-1 bootstrap compilation of `compiler_seen/src/main.seen` fails with ~100+ "Unknown field" type errors, despite struct definitions being present and correctly parsed. Investigation reveals a fundamental issue with how the Seen typechecker resolves struct types when multiple files define overlapping type names.

## Reproduction

```bash
cd /home/yousef/Projects/rust/SeenLang
SEEN_ENABLE_MANIFEST_MODULES=1 scripts/self_host_llvm.sh
```

**Error Pattern**:
```
Type error: Unknown field 'returnType' in struct 'FunctionNode' at 254:33
Type error: Unknown field 'params' in struct 'FunctionNode' at 261:48
Type error: Unknown field 'isInline' in struct 'FunctionNode' at 265:16
Type error: Unknown field 'body' in struct 'FunctionNode' at 276:29
...
```

## Investigation Results

### ✅ What Works

1. **Simple struct field access works in isolation**:
   ```seen
   struct Point { x: Int; y: Int }
   fun main() { let p = Point{x:1, y:2}; return p.x + p.y }
   ```
   Result: ✅ Compiles and runs successfully

2. **Multi-file structs work**:
   - Multiple files can define the same struct name
   - Field access works across file boundaries
   - Both single-line and multi-line struct syntax parse correctly

3. **Parser correctly reads struct definitions**:
   ```bash
   seen_cli parse compiler_seen/src/codegen/complete_codegen.seen
   ```
   Output shows StructDefinition with all fields properly parsed

### 🔴 Root Cause

**Conflicting Type Definitions**:

`compiler_seen/src/parser/interfaces.seen` defines:
```seen
struct Function {
    name: String
    parameters: Array<Parameter>  // ← Note: "parameters" not "params"
    returnType: Type               // ← Note: "Type" not "TypeNode?"
    body: Block                    // ← Note: "Block" not "BlockNode"
    isAsync: Bool                  // ← Missing "isInline"
}
```

`compiler_seen/src/parser/ast.seen` and inline definitions use:
```seen
struct FunctionNode {
    name: String
    params: Array<ParamNode>       // ← "params"
    returnType: TypeNode?          // ← "TypeNode?"
    body: BlockNode                // ← "BlockNode"
    isAsync: Bool
    isInline: Bool                 // ← Has "isInline"
}
```

### Hypothesis

When the Rust-based typechecker (`seen_typechecker` crate) compiles multiple .seen files together:
1. It encounters both `Function` and `FunctionNode` definitions
2. It may confuse them or prioritize the wrong one
3. Field lookups fail because it's checking against the wrong struct definition
4. The error message says "Unknown field 'params' in struct 'FunctionNode'" but the typechecker may actually be looking at `Function` which has `parameters`

## Attempted Solutions

1. ❌ **Option A: Unified AST types** - Tried creating `ast_types.seen` with single source of truth
   - Result: Import resolution doesn't work during bootstrap
   
2. ❌ **Inline definitions** - Added complete struct definitions directly in codegen files
   - Result: Still get "Unknown field" errors
   
3. ❌ **Multi-line format** - Expanded compact struct syntax to multi-line
   - Result: No improvement

## Recommended Fixes

### Fix 1: Remove Conflicting Definitions (QUICK)

Delete or rename incompatible types in `compiler_seen/src/parser/interfaces.seen`:
- Rename `Function` → `FunctionInterface` or similar
- Rename `Type` → `TypeInterface`
- Rename `Block` → `BlockInterface`
- Or comment out the entire interfaces.seen file during bootstrap

### Fix 2: Fix Typechecker Struct Resolution (PROPER)

Investigate `seen_typechecker` Rust crate:
```rust
// File: seen_typechecker/src/lib.rs or similar
// Look for struct type registration/lookup logic
// Ensure it properly namespaces types by module
// Fix field lookup to use the correct struct definition
```

### Fix 3: Implement Module Namespacing (COMPREHENSIVE)

Make the type system distinguish between:
- `parser.ast.FunctionNode`
- `parser.interfaces.Function`

So they don't conflict even with similar names.

## Work Completed

- ✅ Normalized all field names to camelCase across ~15 files
- ✅ Commented out problematic `import interfaces` statements
- ✅ Added comprehensive inline struct definitions
- ✅ Created minimal test cases to isolate the issue
- ✅ Documented the root cause

## Next Steps

1. **Immediate**: Try Fix 1 - comment out or rename types in interfaces.seen
2. **Short-term**: Implement Fix 2 - investigate and fix typechecker
3. **Long-term**: Implement Fix 3 - proper module namespacing

## Files Modified

- `compiler_seen/src/parser/ast.seen` - Field names normalized to camelCase
- `compiler_seen/src/codegen/complete_codegen.seen` - Added inline AST definitions
- `compiler_seen/src/codegen/real_codegen.seen` - Added inline AST definitions
- `compiler_seen/src/parser/real_parser.seen` - Updated import paths
- `compiler_seen/src/*/mod.seen` - Commented out unresolved imports
- Multiple test files created for validation

## Related Commits

- `ee88bb7` - refactor: expand inline struct definitions to multi-line format
- `8563875` - feat(bootstrap): attempt Option A unified AST with compact inline definitions
- `0efd1e6` - feat: normalize AST field names and add inline type definitions for bootstrap
