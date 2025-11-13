# CRITICAL: Typechecker Must Be Fixed Before Self-Hosting Can Proceed

**Priority**: P0 - BLOCKS ALL MVP PROGRESS  
**Impact**: Self-hosting, examples, and all non-trivial Seen programs fail  
**Location**: `seen_typechecker` Rust crate  

## Problem Statement

The Seen typechecker (`seen_typechecker` crate) fails to properly register and resolve struct field definitions, causing ALL non-trivial Seen programs to fail with "Unknown field" errors.

## Evidence

### Simple Programs Work ✅
```seen
struct Point { x: Int; y: Int }
fun main() -> Int {
    let p = Point{ x: 1, y: 2 }
    return p.x + p.y  // ✅ This works!
}
```

### Complex Programs Fail ❌
```seen
struct World {
    entityCount: Int
}

fun createEntity(world: World) -> Int {
    return world.entityCount  // ❌ Type error: Unknown field 'entityCount'
}
```

### Bootstrap Fails ❌
```bash
SEEN_ENABLE_MANIFEST_MODULES=1 scripts/self_host_llvm.sh
# Result: ~100+ "Unknown field" errors
```

## Root Cause (Hypothesis)

When the typechecker processes **multiple .seen files together** or files with **method-like syntax** or **complex structures**, it fails to properly:

1. Register struct field definitions in the type environment
2. Look up fields during type checking
3. Handle method syntax (`World.createEntity(self: World)`)
4. Resolve types across file boundaries when manifest modules are enabled

## Files Affected

- Bootstrap: ALL `compiler_seen/src/**/*.seen` files
- Examples: `examples/seen-ecs-min/main.seen`, `examples/seen-vulkan-min/main.seen`
- Tests: Any test using structs with multiple fields or methods

## Required Fix

Someone with Rust expertise needs to:

1. **Debug `seen_typechecker/src/lib.rs`** (or relevant files):
   ```rust
   // Find where struct types are registered
   // Find where field lookups happen
   // Add logging/debugging to trace the issue
   ```

2. **Check these specific areas**:
   - Struct definition registration in type environment
   - Field lookup logic during type checking
   - How `SEEN_ENABLE_MANIFEST_MODULES=1` affects type resolution
   - Method syntax handling (`.method(self: Type)`)
   - Multi-file compilation type propagation

3. **Minimal Reproduction**:
   ```bash
   # Create test_struct_method.seen:
   struct World { count: Int }
   fun test(w: World) -> Int { return w.count }
   fun main() -> Int { let w = World{count: 5}; return test(w) }
   
   # Run:
   seen_cli run test_struct_method.seen
   # Expected: return 5
   # Actual: "Unknown field 'count'"
   ```

## Temporary Workarounds (All Failed)

- ❌ Inline struct definitions in each file
- ❌ Comment out conflicting type definitions
- ❌ Use multi-line vs single-line struct syntax
- ❌ Avoid imports and use local types
- ❌ Simplify examples to avoid methods

## Impact on MVP

**BLOCKS**:
- ✗ PSH-4: Parser/Self-host (Stage-1 bootstrap)
- ✗ PROD-5: Platform examples (ECS/Vulkan smoke tests)
- ✗ Any example more complex than `hello world`
- ✗ Progress toward Alpha plan

**CAN PROCEED**:
- ✓ Rust-based tooling improvements
- ✓ CLI flag additions
- ✓ Documentation updates
- ✓ Test infrastructure (once typechecker is fixed)

## Next Steps

1. **IMMEDIATE**: Assign Rust developer to debug `seen_typechecker`
2. **SHORT-TERM**: Add comprehensive typechecker unit tests
3. **LONG-TERM**: Consider rewriting typechecker with better architecture

## References

- `BOOTSTRAP_BLOCKER_ANALYSIS.md` - Detailed investigation
- `seen_typechecker/` - Rust crate to fix
- Recent commits: `df569d5`, `ee88bb7`, `8563875`, `0efd1e6`

---

**STATUS**: Waiting for Rust developer with typechecker expertise.  
**ALTERNATIVE**: Consider using a different approach to bootstrap (write Stage-1 compiler in C/Go/etc).
