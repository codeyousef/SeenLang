# LLVM Backend Architecture - Generic Type System

**Author:** yousef  
**Date:** 2025-11-17  
**Version:** 1.0  
**Epic:** Epic 1 - Complete LLVM Backend (Phase 1)  
**Status:** Design Document

---

## Executive Summary

This architecture document defines the technical solution for **Epic 1: Complete LLVM Backend** - the P0 critical blocker preventing benchmark execution. The current LLVM backend only supports hardcoded types (`StrArray`, `CommandResult`). This design enables **generic arrays and structs** for all types, unblocking all 10 production benchmarks.

**Core Problem:** Array operations like `Float[]`, `Int[]` generate incorrect GEP instructions because element types aren't preserved through the IR → LLVM pipeline.

**Solution:** Extend `IRType` tracking, implement generic type mapping, and build stdlib linking infrastructure.

**Timeline:** 12-16 hours implementation (Stories 1.1-1.5)

---

## Problem Statement

### Current State (Broken)

**File:** `seen_ir/src/llvm_backend.rs:1190`
```rust
IRType::Array(_) => {
    // BUG: Always returns i8* regardless of element type
    self.i8_ptr_t.into()
}
```

**Impact:**
- 7/10 benchmarks blocked (Matrix, Sieve, Binary Trees, FASTA, etc.)
- Array indexing generates wrong GEP offsets
- Type safety violations at LLVM level
- Cannot compile Float[], Int[], custom struct arrays

### Current Working Examples (Hardcoded)

**StrArray:** `struct { i64 len; i8** data }`
```rust
fn ty_str_array(&self) -> inkwell::types::StructType<'ctx> {
    self.ctx.struct_type(
        &[
            self.i64_t.into(),
            self.i8_ptr_t.ptr_type(inkwell::AddressSpace::from(0u16)).into(),
        ],
        false,
    )
}
```

**CommandResult:** `struct { bool success; i8* output }`
```rust
fn ty_cmd_result(&self) -> inkwell::types::StructType<'ctx> {
    self.ctx.struct_type(
        &[self.bool_t.into(), self.i8_ptr_t.into()],
        false
    )
}
```

**Problem:** These are special-cased. Generic types fall through to broken default behavior.

---

## Architecture Design

### 1. Type Metadata Preservation (Story 1.1)

**Objective:** Track element types from typechecker → IR → LLVM codegen

#### Current IRType Definition

**File:** `seen_ir/src/value.rs:8-35`
```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IRType {
    Void,
    Integer,
    Float,
    Boolean,
    Char,
    String,
    Vector { lanes: u32, lane_type: Box<IRType> },
    Array(Box<IRType>),  // ✅ ALREADY TRACKS ELEMENT TYPE
    Function { parameters: Vec<IRType>, return_type: Box<IRType> },
    Struct { name: String, fields: Vec<(String, IRType)> },  // ✅ TRACKS FIELDS
    Enum { name: String, variants: Vec<(String, Option<Vec<IRType>>)> },
    Pointer(Box<IRType>),
    Reference(Box<IRType>),
    Optional(Box<IRType>),
    Generic(String),
}
```

**Good News:** `IRType::Array(Box<IRType>)` already preserves element type! The bug is in LLVM mapping, not IR design.

#### Solution: Fix ir_type_to_llvm Mapping

**File:** `seen_ir/src/llvm_backend.rs:1176-1206`

**Current (Broken):**
```rust
fn ir_type_to_llvm(&self, t: &IRType) -> BasicTypeEnum<'ctx> {
    match t {
        IRType::Array(_) => {
            // BUG: Ignores element type
            self.i8_ptr_t.into()
        }
        // ... other cases
    }
}
```

**Fixed (Story 1.1):**
```rust
fn ir_type_to_llvm(&self, t: &IRType) -> BasicTypeEnum<'ctx> {
    match t {
        IRType::Array(element_type) => {
            // Generate runtime array struct: { i64 len; i64 cap; T* data }
            let element_llvm_type = self.ir_type_to_llvm(element_type);
            let data_ptr = element_llvm_type.ptr_type(AddressSpace::from(0u16));
            
            let array_struct = self.ctx.struct_type(
                &[
                    self.i64_t.into(),      // len field
                    self.i64_t.into(),      // capacity field
                    data_ptr.into(),        // data pointer (typed!)
                ],
                false
            );
            
            // Return pointer to struct (arrays are heap-allocated)
            array_struct.ptr_type(AddressSpace::from(0u16)).into()
        }
        // ... other cases
    }
}
```

**Key Insight:** Arrays are runtime-managed heap objects. LLVM sees them as pointers to structs, not LLVM arrays.

#### Type Cache for Performance

```rust
pub struct LlvmBackend<'ctx> {
    // Existing fields...
    type_cache: HashMap<IRType, BasicTypeEnum<'ctx>>,  // NEW
}

fn ir_type_to_llvm_cached(&mut self, t: &IRType) -> BasicTypeEnum<'ctx> {
    if let Some(cached) = self.type_cache.get(t) {
        return *cached;
    }
    let llvm_type = self.ir_type_to_llvm(t);
    self.type_cache.insert(t.clone(), llvm_type);
    llvm_type
}
```

**Benefit:** Recursive struct types only computed once.

---

### 2. Generic Array GEP Instructions (Story 1.2)

**Objective:** Array indexing and mutation with correct type offsets

#### Current ArrayAccess (Broken for Generic Types)

**File:** `seen_ir/src/llvm_backend.rs:1926-1959`

**Current Implementation:**
```rust
Instruction::ArrayAccess { array, index, result } => {
    let arr_v = self.eval_value(array, fn_map)?;
    
    if let IRValue::Array(vs) = array {
        // Compile-time constant array - works
        let idx_val = // ... extract index ...
        let elem = self.eval_value(&vs[idx_val], fn_map)?;
        self.assign_value(result, elem)?;
    } else if arr_v.is_pointer_value() {
        // Runtime array - HARDCODED FOR StrArray
        let ty = self.ty_str_array();  // ❌ WRONG
        let arr_ptr = arr_v.into_pointer_value();
        let data_ptr_ptr = self.builder.build_struct_gep(ty, arr_ptr, 1, "data_ptr")?;
        // ... rest hardcoded for i8** ...
    }
}
```

**Problem:** `self.ty_str_array()` returns `{ i64, i8** }`, but we need `{ i64, i64, T* }` for any `T`.

#### Fixed Implementation (Story 1.2)

```rust
Instruction::ArrayAccess { array, index, result } => {
    let arr_v = self.eval_value(array, fn_map)?;
    let idx_v = self.eval_value(index, fn_map)?;
    let idx_i64 = self.as_i64(idx_v)?;
    
    // Get array element type from IR metadata
    let element_type = match array {
        IRValue::Register(reg_id) => {
            // Look up register type from instruction metadata
            self.get_register_type(*reg_id)?
        }
        _ => return Err(anyhow!("Cannot determine array element type")),
    };
    
    // Generate array struct type for this element type
    let array_struct_ty = self.ir_array_struct_type(&element_type);
    let element_llvm_ty = self.ir_type_to_llvm(&element_type);
    
    // Cast array value to correct struct pointer
    let arr_ptr = if arr_v.is_pointer_value() {
        arr_v.into_pointer_value()
    } else {
        self.builder.build_int_to_ptr(
            arr_v.into_int_value(),
            array_struct_ty.ptr_type(AddressSpace::from(0u16)),
            "arr_ptr"
        )?
    };
    
    // GEP to data field (field index 2)
    let data_ptr_ptr = self.builder.build_struct_gep(
        array_struct_ty, 
        arr_ptr, 
        2,  // data field
        "data_ptr"
    )?;
    
    // Load data pointer (T*)
    let data_ptr = self.builder.build_load(
        element_llvm_ty.ptr_type(AddressSpace::from(0u16)),
        data_ptr_ptr,
        "data"
    )?.into_pointer_value();
    
    // GEP into data array with index
    let elem_ptr = unsafe {
        self.builder.build_gep(
            element_llvm_ty,
            data_ptr,
            &[idx_i64],
            "elem_ptr"
        )?
    };
    
    // Load element value
    let elem_val = self.builder.build_load(
        element_llvm_ty,
        elem_ptr,
        "elem"
    )?;
    
    self.assign_value(result, elem_val)?;
}
```

**Key Changes:**
1. **Type lookup:** `get_register_type()` retrieves element type from IR metadata
2. **Typed GEP:** Uses correct element type for pointer arithmetic
3. **Generic:** Works for `Float[]`, `Int[]`, `MyStruct[]`, etc.

#### ArraySet (Mutation) Implementation

```rust
Instruction::ArraySet { array, index, value } => {
    // Similar to ArrayAccess but use build_store instead of build_load
    let element_type = self.get_register_type(array_reg_id)?;
    // ... same GEP logic ...
    let elem_ptr = // ... computed as above ...
    let value_llvm = self.eval_value(value, fn_map)?;
    self.builder.build_store(elem_ptr, value_llvm)?;
}
```

#### Bounds Checking Strategy

**Debug Mode (-O0):**
```rust
fn emit_array_bounds_check(
    &mut self,
    arr_ptr: PointerValue<'ctx>,
    index: IntValue<'ctx>,
    array_struct_ty: StructType<'ctx>,
) -> Result<()> {
    // Load array length
    let len_ptr = self.builder.build_struct_gep(array_struct_ty, arr_ptr, 0, "len_ptr")?;
    let len = self.builder.build_load(self.i64_t, len_ptr, "len")?.into_int_value();
    
    // Compare: index < len
    let in_bounds = self.builder.build_int_compare(
        IntPredicate::ULT,
        index,
        len,
        "bounds_check"
    )?;
    
    // Conditional branch
    let check_pass = self.ctx.append_basic_block(self.current_function.unwrap(), "bounds_ok");
    let check_fail = self.ctx.append_basic_block(self.current_function.unwrap(), "bounds_fail");
    self.builder.build_conditional_branch(in_bounds, check_pass, check_fail)?;
    
    // Fail block: call __ArrayIndexOutOfBounds(index, len)
    self.builder.position_at_end(check_fail);
    let panic_fn = self.get_or_declare_panic_function();
    self.builder.build_call(panic_fn, &[index.into(), len.into()], "panic")?;
    self.builder.build_unreachable()?;
    
    // Continue in pass block
    self.builder.position_at_end(check_pass);
    Ok(())
}
```

**Release Mode (-O3):**
- Skip bounds checks entirely
- LLVM optimizes away unreachable panic code
- Maximum performance (matches Rust `unsafe` indexing)

---

### 3. Generic Struct Field Access (Story 1.3)

**Objective:** Field access for any struct type, not just CommandResult

#### Current Struct GEP (Limited)

**File:** `seen_ir/src/llvm_backend.rs:2483` (CommandResult example)
```rust
let ty = self.ty_cmd_result();  // Hardcoded
let cast = self.builder.build_pointer_cast(p, ty.ptr_type(...), "cmdres")?;
let sp = self.builder.build_struct_gep(ty, cast, 0, "succp")?;  // Field 0
let op = self.builder.build_struct_gep(ty, cast, 1, "outp")?;   // Field 1
```

**Problem:** Field indices hardcoded. No generic mechanism.

#### Solution: Struct Layout Tracker

```rust
pub struct LlvmBackend<'ctx> {
    // Existing fields...
    struct_layouts: HashMap<String, Vec<(String, IRType)>>,  // NEW
}

impl<'ctx> LlvmBackend<'ctx> {
    /// Register struct layout from IR
    fn register_struct_layout(&mut self, name: String, fields: Vec<(String, IRType)>) {
        self.struct_layouts.insert(name, fields);
    }
    
    /// Get field index by name
    fn get_field_index(&self, struct_name: &str, field_name: &str) -> Result<u32> {
        let fields = self.struct_layouts.get(struct_name)
            .ok_or_else(|| anyhow!("Unknown struct: {}", struct_name))?;
        
        fields.iter()
            .position(|(name, _)| name == field_name)
            .map(|pos| pos as u32)
            .ok_or_else(|| anyhow!("Unknown field {} in struct {}", field_name, struct_name))
    }
    
    /// Generate LLVM struct type from IR metadata
    fn ir_struct_to_llvm(&self, name: &str) -> Result<StructType<'ctx>> {
        let fields = self.struct_layouts.get(name)
            .ok_or_else(|| anyhow!("Unknown struct: {}", name))?;
        
        let field_types: Vec<BasicTypeEnum<'ctx>> = fields.iter()
            .map(|(_, ty)| self.ir_type_to_llvm(ty))
            .collect();
        
        Ok(self.ctx.struct_type(&field_types, false))
    }
}
```

#### FieldAccess Instruction Implementation

```rust
Instruction::FieldAccess { object, struct_name, field_name, result } => {
    let obj_v = self.eval_value(object, fn_map)?;
    
    // Get struct layout
    let struct_ty = self.ir_struct_to_llvm(struct_name)?;
    let field_index = self.get_field_index(struct_name, field_name)?;
    let field_type = &self.struct_layouts[struct_name][field_index as usize].1;
    let field_llvm_ty = self.ir_type_to_llvm(field_type);
    
    // Cast object to struct pointer
    let obj_ptr = if obj_v.is_pointer_value() {
        obj_v.into_pointer_value()
    } else {
        self.builder.build_int_to_ptr(
            obj_v.into_int_value(),
            struct_ty.ptr_type(AddressSpace::from(0u16)),
            "obj_ptr"
        )?
    };
    
    // GEP to field
    let field_ptr = self.builder.build_struct_gep(
        struct_ty,
        obj_ptr,
        field_index,
        &format!("{}_ptr", field_name)
    )?;
    
    // Load field value
    let field_val = self.builder.build_load(
        field_llvm_ty,
        field_ptr,
        field_name
    )?;
    
    self.assign_value(result, field_val)?;
}
```

#### FieldSet (Mutation) Implementation

```rust
Instruction::FieldSet { object, struct_name, field_name, value } => {
    // Same as FieldAccess but use build_store
    let field_ptr = // ... computed as above ...
    let value_llvm = self.eval_value(value, fn_map)?;
    self.builder.build_store(field_ptr, value_llvm)?;
}
```

#### Nested Field Access

**Example:** `obj.inner.field`

**IR Representation:**
```rust
// Instruction 1: %inner = FieldAccess %obj, "Matrix", "inner"
// Instruction 2: %field = FieldAccess %inner, "InnerStruct", "field"
```

**Works automatically** because each `FieldAccess` returns a value that can be the object for the next access.

---

### 4. Stdlib Linking Infrastructure (Story 1.4)

**Objective:** Compiled programs can call `Array.push()`, `HashMap.get()`, etc.

#### Current Problem

Stdlib methods exist only in interpreter (`seen_runtime`). LLVM compiled code has no access.

#### Solution: Pre-compiled Stdlib Strategy (Option B)

**Benefits:**
- ✅ Clean separation (stdlib compiled once)
- ✅ Reusable across programs
- ✅ Faster compilation (no stdlib rebuild)
- ✅ ABI stability enforced

**Architecture:**

```
┌──────────────────────┐
│   seen_std source    │  Seen stdlib code (Array.seen, HashMap.seen, etc.)
│   (Seen language)    │
└──────────┬───────────┘
           │ Compile once with Stage1 Rust compiler
           ↓
┌──────────────────────┐
│  libseen_std.a       │  Static library (LLVM bitcode or native .a)
│  (LLVM IR / native)  │
└──────────┬───────────┘
           │ Link at compile time
           ↓
┌──────────────────────┐
│  User Program        │  
│  (Compiled with      │  Links against libseen_std.a
│   seen build -O3)    │
└──────────────────────┘
```

#### Stdlib Build Process

**File:** `seen_std/build.sh` (NEW)
```bash
#!/bin/bash
# Build seen_std as static library

set -e

echo "Building seen_std static library..."

# Compile all stdlib modules to LLVM IR
seen build --backend llvm -O3 --emit-llvm-ir \
    seen_std/src/array.seen \
    seen_std/src/hashmap.seen \
    seen_std/src/string.seen \
    seen_std/src/io.seen \
    -o seen_std.bc

# Link all .bc files into single static library
llvm-link seen_std.bc -o libseen_std.bc
llvm-ar rcs libseen_std.a libseen_std.bc

echo "✅ libseen_std.a created"
```

#### ABI Stability Guard

**File:** `seen_std/src/abi_guard.seen`
```seen
// ABI version marker
pub const ABI_VERSION: Int = 1;

// Array ABI layout (must match LLVM backend)
pub struct ArrayABI<T> {
    len: Int,      // Field 0: i64
    capacity: Int, // Field 1: i64
    data: *T,      // Field 2: T*
}

// Compile-time assertion
static_assert(sizeof(ArrayABI<Int>) == 24, "Array ABI changed");
```

**Enforcement:** If stdlib recompiled with different layout, link fails with ABI mismatch error.

#### Extern Function Declarations

**In User Code:**
```seen
// Array methods are "extern" - defined in libseen_std.a
extern fn Array_push<T>(arr: *Array<T>, elem: T) -> Void;
extern fn Array_len<T>(arr: *Array<T>) -> Int;
extern fn Array_withCapacity<T>(cap: Int) -> Array<T>;
```

**LLVM Backend Handling:**
```rust
Instruction::Call { function, arguments, result } if function.starts_with("Array_") => {
    // Extern stdlib function - just emit call, linker will resolve
    let fn_name = function;
    let llvm_fn = self.module.get_function(fn_name)
        .unwrap_or_else(|| {
            // Declare as extern if not yet declared
            let fn_ty = self.infer_function_type(fn_name, arguments);
            self.module.add_function(fn_name, fn_ty, Some(Linkage::External))
        });
    
    let args: Vec<BasicMetadataValueEnum> = arguments.iter()
        .map(|arg| self.eval_value(arg, fn_map).unwrap().into())
        .collect();
    
    let call_site = self.builder.build_call(llvm_fn, &args, "stdlib_call")?;
    
    if let Some(res_reg) = result {
        let ret_val = call_site.try_as_basic_value().left().unwrap();
        self.assign_value(res_reg, ret_val)?;
    }
}
```

#### Linker Integration

**File:** `seen_cli/src/commands/build.rs`
```rust
fn link_program(object_files: &[PathBuf], output: &Path) -> Result<()> {
    let stdlib_path = std::env::var("SEEN_STDLIB_PATH")
        .unwrap_or_else(|_| "/usr/local/lib/libseen_std.a".to_string());
    
    let mut cmd = Command::new("clang");
    cmd.arg("-o").arg(output);
    
    for obj in object_files {
        cmd.arg(obj);
    }
    
    cmd.arg(&stdlib_path);  // Link stdlib
    cmd.arg("-lm");         // Link libm (math functions)
    cmd.arg("-lpthread");   // Link pthread (for future concurrency)
    
    let status = cmd.status()?;
    if !status.success() {
        return Err(anyhow!("Linking failed"));
    }
    
    Ok(())
}
```

---

### 5. Register Type Tracking (Supporting Infrastructure)

**Problem:** Array/struct instructions need to know element/field types at codegen time.

**Solution:** Extend IR to track register types.

```rust
pub struct IRFunction {
    pub name: String,
    pub return_type: IRType,
    pub parameters: Vec<Parameter>,
    pub instructions: Vec<Instruction>,
    pub register_types: HashMap<String, IRType>,  // NEW
}

impl IRFunction {
    /// Record type of a register
    pub fn set_register_type(&mut self, reg: &str, ty: IRType) {
        self.register_types.insert(reg.to_string(), ty);
    }
    
    /// Get type of a register
    pub fn get_register_type(&self, reg: &str) -> Option<&IRType> {
        self.register_types.get(reg)
    }
}
```

**IR Generation:** When emitting instructions, record result register types:

```rust
// In IRGenerator
fn generate_array_literal(&mut self, elements: &[Expression]) -> IRResult<(IRValue, Vec<Instruction>)> {
    // ... generate instructions ...
    
    let result_reg = self.next_register();
    let element_type = self.infer_element_type(elements)?;
    let array_type = IRType::Array(Box::new(element_type));
    
    // Record register type
    self.current_function.set_register_type(&result_reg, array_type);
    
    Ok((IRValue::Register(result_reg), instructions))
}
```

**LLVM Backend Usage:**
```rust
impl<'ctx> LlvmBackend<'ctx> {
    fn get_register_type(&self, reg_id: &str) -> Result<IRType> {
        self.current_function
            .get_register_type(reg_id)
            .cloned()
            .ok_or_else(|| anyhow!("Unknown register type: {}", reg_id))
    }
}
```

---

## Implementation Plan

### Story 1.1: Generic Array Type System (3-4 hours)

**Tasks:**
1. Fix `ir_type_to_llvm()` for `IRType::Array`
2. Add `type_cache` HashMap for performance
3. Implement `ir_array_struct_type()` helper
4. Add unit tests for Int[], Float[], String[], Bool[]

**Test Case:**
```seen
let arr: Float[] = [1.0, 2.0, 3.0];
let x: Float = arr[0];  // Should compile to correct GEP
```

**Acceptance Criteria:**
- [ ] All primitive array types compile
- [ ] Generated LLVM IR uses correct element types
- [ ] No hardcoded type assumptions

---

### Story 1.2: Array Indexing & Mutation (2-3 hours)

**Tasks:**
1. Implement `get_register_type()` lookup
2. Rewrite `Instruction::ArrayAccess` with generic GEP
3. Implement `Instruction::ArraySet` for mutation
4. Add bounds checking (debug mode)
5. Add tests for read/write operations

**Test Case:**
```seen
let arr: Int[] = Array.withCapacity(10);
arr[5] = 42;
let val: Int = arr[5];  // val == 42
```

**Acceptance Criteria:**
- [ ] Array reads generate correct loads
- [ ] Array writes generate correct stores
- [ ] Bounds checks work in debug mode
- [ ] No bounds checks in -O3 mode

---

### Story 1.3: Generic Struct Field Access (3-4 hours)

**Tasks:**
1. Add `struct_layouts` HashMap
2. Implement `register_struct_layout()` from IR
3. Implement `get_field_index()` lookup
4. Rewrite `Instruction::FieldAccess` generically
5. Implement `Instruction::FieldSet` for mutation
6. Add tests for nested field access

**Test Case:**
```seen
struct Matrix {
    rows: Int,
    cols: Int,
    data: Float[],
}

let m: Matrix = Matrix { rows: 3, cols: 3, data: [1.0, 2.0, 3.0] };
let r: Int = m.rows;  // Should compile to GEP field 0
m.rows = 5;           // Should compile to GEP field 0 + store
```

**Acceptance Criteria:**
- [ ] Field reads work for any struct
- [ ] Field writes work for any struct
- [ ] Nested access (`obj.inner.field`) works
- [ ] No hardcoded struct types remain

---

### Story 1.4: Stdlib Linking Infrastructure (4-5 hours)

**Tasks:**
1. Create `seen_std/build.sh` script
2. Compile stdlib to `libseen_std.a`
3. Add ABI version guard
4. Implement extern function resolution in LLVM backend
5. Update `seen build` to link stdlib
6. Add tests for stdlib method calls

**Test Case:**
```seen
let arr: Int[] = Array.withCapacity(10);
arr.push(42);
arr.push(100);
let len: Int = arr.len();  // len == 2
```

**Acceptance Criteria:**
- [ ] Stdlib builds to static library
- [ ] User programs link successfully
- [ ] Array methods work in compiled code
- [ ] HashMap, String, etc. methods work

---

### Story 1.5: Float Arithmetic Validation (1 hour)

**Tasks:**
1. Verify all float operations tested
2. Add mixed Int/Float operation tests
3. Document float semantics

**Test Case:**
```seen
let x: Float = 3.14;
let y: Int = 2;
let z: Float = x * y.toFloat();  // Mixed operations
```

**Acceptance Criteria:**
- [ ] All float ops (fadd, fsub, fmul, fdiv, frem) tested
- [ ] Mixed operations work correctly
- [ ] Float comparisons work

---

## Validation & Testing

### Unit Tests

**File:** `seen_ir/tests/llvm_generic_types.rs`
```rust
#[test]
fn test_float_array_indexing() {
    let code = r#"
        fn test() -> Float {
            let arr: Float[] = [1.0, 2.0, 3.0];
            return arr[1];  // Should return 2.0
        }
    "#;
    
    let result = compile_and_run(code);
    assert_eq!(result, 2.0);
}

#[test]
fn test_struct_field_access() {
    let code = r#"
        struct Point {
            x: Float,
            y: Float,
        }
        
        fn test() -> Float {
            let p: Point = Point { x: 3.0, y: 4.0 };
            return p.x + p.y;  // Should return 7.0
        }
    "#;
    
    let result = compile_and_run(code);
    assert_eq!(result, 7.0);
}

#[test]
fn test_stdlib_array_methods() {
    let code = r#"
        fn test() -> Int {
            let arr: Int[] = Array.withCapacity(5);
            arr.push(10);
            arr.push(20);
            arr.push(30);
            return arr.len();  // Should return 3
        }
    "#;
    
    let result = compile_and_run(code);
    assert_eq!(result, 3);
}
```

### Integration Tests

**Matrix Multiply Stub (Epic 1 Validation):**
```seen
struct Matrix {
    rows: Int,
    cols: Int,
    data: Float[],
}

fn matrix_new(rows: Int, cols: Int) -> Matrix {
    let size = rows * cols;
    return Matrix {
        rows: rows,
        cols: cols,
        data: Array.withCapacity(size),
    };
}

fn matrix_get(m: Matrix, row: Int, col: Int) -> Float {
    let idx = row * m.cols + col;
    return m.data[idx];
}

fn matrix_set(m: Matrix, row: Int, col: Int, val: Float) -> Void {
    let idx = row * m.cols + col;
    m.data[idx] = val;
}

fn main() -> Int {
    let m = matrix_new(2, 2);
    matrix_set(m, 0, 0, 1.0);
    matrix_set(m, 1, 1, 2.0);
    let sum = matrix_get(m, 0, 0) + matrix_get(m, 1, 1);
    __PrintFloat(sum);  // Should print 3.0
    return 0;
}
```

**Run:** `seen build -O3 --backend llvm matrix_test.seen && ./matrix_test`

**Expected Output:** `3.0`

---

## Performance Considerations

### Type Cache Impact

**Before (no cache):**
- Every array access recomputes struct type
- O(n) for nested structs
- Visible overhead in large functions

**After (with cache):**
- Type computed once per unique IRType
- O(1) lookups
- Negligible overhead

**Measurement:** Profile Epic 3 benchmarks, ensure <1% overhead from type system.

### GEP Optimization

LLVM's GEP instruction is **zero-cost abstraction** when optimized:
- `-O0`: Generates actual pointer arithmetic
- `-O3`: GEP chains collapse to single offset calculation
- **No runtime overhead** vs handwritten C pointer math

### Stdlib Linking Strategy Comparison

| Strategy | Compile Time | Binary Size | Maintainability |
|----------|--------------|-------------|-----------------|
| **Option A: Inline stdlib** | Slow (rebuild every time) | Small (dead code eliminated) | Hard (ABI drift) |
| **Option B: Pre-compiled stdlib** | Fast (link only) | Medium (+stdlib overhead) | Easy (enforced ABI) |
| **Option C: Dynamic linking** | Fast | Smallest (shared .so) | Complex (version hell) |

**Selected: Option B** for MVP. Option C for post-MVP optimization.

---

## Risk Mitigation

### Risk 1: Type Inference Failures

**Scenario:** Register type unknown at codegen time

**Mitigation:**
- IR generator must **always** record types when creating registers
- Add `#[cfg(debug_assertions)]` validation in LLVM backend
- Fail fast with clear error: "Register %r5 has unknown type"

### Risk 2: ABI Incompatibility

**Scenario:** Stdlib recompiled with different layout, linker doesn't catch it

**Mitigation:**
- `abi_guard.seen` with compile-time assertions
- Version number in static library metadata
- `seen build` checks ABI version before linking

### Risk 3: Performance Regression

**Scenario:** Generic type system slower than hardcoded types

**Mitigation:**
- **Unlikely:** GEP is zero-cost, type cache is O(1)
- If measured >1% overhead: add fast-path for common types (Int[], Float[])
- Validate with Epic 5 profiling

---

## Success Metrics

### Epic 1 Complete When:

1. ✅ **All 5 stories pass acceptance criteria**
2. ✅ **Matrix multiply stub compiles and runs**
3. ✅ **No hardcoded types in `llvm_backend.rs`**
4. ✅ **CI tests pass (unit + integration)**
5. ✅ **Performance baseline: <1% overhead vs hardcoded version**

### Unblocks:

- **Epic 2:** Self-hosting (compiler can compile itself)
- **Epic 3:** All 10 benchmarks (Matrix, Sieve, Binary Trees, etc.)
- **Epic 4:** Performance measurement infrastructure
- **Epic 5:** Optimization (can't optimize what doesn't compile!)

---

## Appendix A: LLVM IR Examples

### Float Array Indexing

**Seen Code:**
```seen
let arr: Float[] = [1.0, 2.0, 3.0];
let x: Float = arr[1];
```

**Generated LLVM IR:**
```llvm
; Array struct: { i64 len, i64 cap, double* data }
%array_ty = type { i64, i64, double* }

; Allocate array struct
%arr = call %array_ty* @Array_allocate(i64 3)

; Initialize elements
; ... (omitted for brevity) ...

; Index access: arr[1]
%arr_ptr = ... ; Get pointer to array struct
%data_ptr_ptr = getelementptr %array_ty, %array_ty* %arr_ptr, i32 0, i32 2
%data_ptr = load double*, double** %data_ptr_ptr
%elem_ptr = getelementptr double, double* %data_ptr, i64 1
%x = load double, double* %elem_ptr
```

**Key Points:**
- GEP with field index 2 gets `data` pointer
- GEP with element index 1 gets `arr[1]`
- All offsets computed at compile time (zero-cost)

### Struct Field Access

**Seen Code:**
```seen
struct Point { x: Float, y: Float }
let p: Point = Point { x: 3.0, y: 4.0 };
let a: Float = p.x;
```

**Generated LLVM IR:**
```llvm
%Point = type { double, double }

%p = alloca %Point
%x_ptr = getelementptr %Point, %Point* %p, i32 0, i32 0  ; Field 0 (x)
store double 3.0, double* %x_ptr
%y_ptr = getelementptr %Point, %Point* %p, i32 0, i32 1  ; Field 1 (y)
store double 4.0, double* %y_ptr

%a_ptr = getelementptr %Point, %Point* %p, i32 0, i32 0
%a = load double, double* %a_ptr
```

---

## Appendix B: Alternative Designs Considered

### Alternative 1: LLVM Arrays Instead of Runtime Structs

**Approach:** Use LLVM's native array type `[n x T]`

**Pros:**
- Simpler GEP (no struct indirection)
- Better LLVM optimization

**Cons:**
- ❌ Fixed size at compile time (can't resize)
- ❌ No capacity tracking
- ❌ Incompatible with stdlib runtime (expects structs)

**Verdict:** Rejected. SeenLang arrays are dynamic, need runtime management.

### Alternative 2: Fat Pointers

**Approach:** Array = `{ T*, i64 len }` passed by value

**Pros:**
- Smaller struct (16 bytes vs 24 bytes)
- Cache-friendly

**Cons:**
- ❌ No capacity field (push() requires reallocation check)
- ❌ Incompatible with stdlib ABI

**Verdict:** Rejected. Capacity is needed for efficient push().

### Alternative 3: Type Erasure (i8* for all arrays)

**Approach:** Keep current design (all arrays are `i8*`), track size externally

**Pros:**
- Zero implementation cost (already exists)

**Cons:**
- ❌ No type safety
- ❌ Requires manual size calculations (error-prone)
- ❌ Cannot optimize based on type

**Verdict:** Rejected. This is the current broken state we're fixing.

---

_This architecture enables Epic 1 completion, unblocking all 10 production benchmarks and enabling the path to ≥1.0x Rust performance._

_Created by: Winston (Architect Agent)_  
_Source: PRD (docs/prd.md), Epics (docs/epics-and-stories.md)_  
_Implementation Guide: Stories 1.1-1.5 in Epic 1_
