# LLVM Integration Fix Instructions

## Files to Modify

### 1. seen_ir/src/codegen.rs

**Fix 1: Add Struct Declaration Handler (around line 108)**
After the `Declaration::Variable` match arm, add:
```rust
Declaration::Struct(_struct_decl) => {
    // TODO: Implement struct declaration code generation
    return Err(CodeGenError::UnsupportedFeature(
        "Struct declarations not yet implemented in IR generation".to_string()
    ));
}
```

**Fix 2: Add For Statement Handler (around line 388)**
Before the `Statement::DeclarationStatement` match arm, add:
```rust
Statement::For(_for_stmt) => {
    // TODO: Implement for loop code generation
    return Err(CodeGenError::UnsupportedFeature(
        "For loops not yet implemented in IR generation".to_string()
    ));
}
```

**Fix 3: Add Missing Expression Handlers (around line 511)**
After the `Expression::Assign` match arm, add:
```rust
Expression::StructLiteral(_struct_lit) => {
    Err(CodeGenError::UnsupportedFeature(
        "Struct literals not yet implemented in IR generation".to_string()
    ))
}
Expression::FieldAccess(_field_access) => {
    Err(CodeGenError::UnsupportedFeature(
        "Field access not yet implemented in IR generation".to_string()
    ))
}
Expression::ArrayLiteral(_array_lit) => {
    Err(CodeGenError::UnsupportedFeature(
        "Array literals not yet implemented in IR generation".to_string()
    ))
}
Expression::Index(_index_expr) => {
    Err(CodeGenError::UnsupportedFeature(
        "Array indexing not yet implemented in IR generation".to_string()
    ))
}
Expression::Range(_range_expr) => {
    Err(CodeGenError::UnsupportedFeature(
        "Range expressions not yet implemented in IR generation".to_string()
    ))
}
```

### 2. seen_ir/src/types.rs

**Fix 4: Add Struct Type Handler (around line 40)**
After the `Type::Array` match arm, add:
```rust
Type::Struct(_struct_name) => {
    return Err(CodeGenError::UnsupportedFeature(
        "Struct types not yet implemented in IR generation".to_string()
    ))
}
```

### 3. seen_cli/Cargo.toml

**Re-enable LLVM dependencies (lines 13-14 and 41)**
Uncomment:
```toml
seen_ir = { path = "../seen_ir" }
inkwell = { version = "0.6.0", features = ["llvm18-1"] }
```

### 4. seen_compiler/Cargo.toml

**Re-enable LLVM dependency (line 11)**
Uncomment:
```toml
inkwell = { version = "0.6.0", features = ["llvm18-1"] }
```

## Test Command

After making these changes, test with:
```bash
LLVM_SYS_181_PREFIX=/usr/lib/llvm-18 cargo build --package seen_ir
LLVM_SYS_181_PREFIX=/usr/lib/llvm-18 cargo build --package seen_cli
LLVM_SYS_181_PREFIX=/usr/lib/llvm-18 cargo build --package seen_compiler
```