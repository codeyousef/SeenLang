# Story 1.6: Class Method Support - IN PROGRESS 🚧

**Status:** Partial implementation  
**Blocker:** Implicit `this` parameter not registered in `var_ir_types`

## Problem Discovered

When compiling `benchmarks/production/01_matrix_mult.seen`, we hit:
```
Error: Cannot infer type for struct variable 'this'
```

## Root Cause Analysis

### What We Fixed

1. **Type Registration System** ✅
   - Added `type_definitions: HashMap<String, IRType>` to `GenerationContext`
   - Modified `convert_ast_type_to_ir()` to look up registered class/struct/enum types
   - Classes now properly convert from `ast::Type{name: "Matrix"}` to `IRType::Struct{...}`

2. **Struct Field Access** ✅
   - Fixed FieldAccess/FieldSet to reconstruct LLVM struct type from IRType::Struct fields
   - Handles opaque pointers in inkwell 0.6 correctly
   - No longer fails on pointer-to-struct conversion

### What Still Needs Fixing

**Implicit `this` Parameter:**

In SeenLang, class methods have implicit `this`:
```seen
class Matrix {
    data: Array<Float>
    
    fun get(row: Int, col: Int) -> Float {
        this.data[row * this.cols + col]  // `this` is implicit
    }
}
```

But in IR generation:
- We inject explicit receiver parameter in `register_class_type` (line 2010-2024)
- The receiver might be named "self" not "this" (line 2018)
- When generating method body expressions, `this` references don't map to parameters

**The Fix Needed:**

Option A: **Make `this` a special variable**
- In `generate_method_function`, add `this` to `var_ir_types` explicitly
- Map it to the first parameter's type

Option B: **Rewrite `this` to parameter name**
- During expression generation, replace `this` with actual parameter name ("self")
- Requires AST transformation before IR generation

Option C: **Context-aware generation**
- Track "current receiver type" in `GenerationContext`
- When encountering `this`, inject it with current receiver type

**Recommended:** Option A - cleanest integration with current architecture

## Files Modified

### `seen_ir/src/generator.rs`

**Added type registry:**
```rust
pub struct GenerationContext {
    // ... existing fields ...
    pub type_definitions: HashMap<String, IRType>,  // NEW
}
```

**Updated type lookup:**
```rust
fn convert_ast_type_to_ir(&self, ast_type: &seen_parser::ast::Type) -> IRType {
    match ast_type.name.as_str() {
        "Int" => IRType::Integer,
        // ... primitives ...
        _ => {
            // Look up registered types (classes/structs/enums)
            if let Some(ir_type) = self.context.type_definitions.get(&ast_type.name) {
                ir_type.clone()
            } else {
                IRType::Integer  // Fallback
            }
        }
    }
}
```

**Register types on definition:**
- `register_struct_type`: Added `context.type_definitions.insert(...)`
- `register_class_type`: Added `context.type_definitions.insert(...)`  
- `register_enum_type`: Added `context.type_definitions.insert(...)`

### `seen_ir/src/llvm_backend.rs`

**Fixed struct type generation for GEP:**
```rust
// FieldAccess (line ~2862)
let llvm_struct_type = if let IRType::Struct { fields, .. } = struct_ir_type {
    let field_types: Vec<BasicTypeEnum<'ctx>> = fields.iter()
        .map(|(_, field_type)| self.ir_type_to_llvm(field_type))
        .collect();
    self.ctx.struct_type(&field_types, false)
} else {
    return Err(anyhow!("Expected struct type"));
};
```

Same pattern applied to FieldSet (line ~3170).

**Why:** `ir_type_to_llvm(IRType::Struct{...})` returns `ptr` (opaque pointer), but `build_struct_gep` needs the actual struct type. We reconstruct it from fields.

## Next Steps (Story 1.6 Completion)

1. **Implement `this` parameter mapping** (1-2 hours)
   - Add method to `GenerationContext`: `register_method_receiver(type: IRType)`
   - In `generate_method_function`, call this before generating body
   - Modify expression generator to check for `this` identifier

2. **Test with Matrix class** (30 min)
   - Compile `01_matrix_mult.seen`
   - Should succeed with class type information preserved

3. **Validate all benchmarks** (1 hour)
   - Check which benchmarks use classes
   - Ensure they compile

## Technical Debt

- **Vtable generation:** Currently adds dummy vtable field (line 1982-1988) but doesn't implement virtual dispatch
- **Type fallback:** Unknown types default to `IRType::Integer` - should error instead
- **Array construction:** `Matrix { data: arr, ... }` requires struct literal support (not yet implemented)

## Performance Note

With proper class types, LLVM can optimize:
- Inline field accesses (no function calls)
- Dead code elimination (unused methods)
- Struct layout optimization

This should help reach ≥1.0x Rust performance target.

## Related Stories

- **Story 1.1:** Generic array types ✅
- **Story 1.2:** Array operations ✅  
- **Story 1.3:** Struct operations ✅
- **Story 1.4:** Stdlib linking ✅
- **Story 1.5:** Validation ✅
- **Story 1.6:** Class method support 🚧 (current)
- **Epic 3:** Production benchmarks (blocked until 1.6 complete)

---

**Estimated remaining effort:** 2-3 hours to complete Story 1.6
