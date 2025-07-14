# Manual LLVM Integration Fixes

Since there are permission issues, here are the exact changes to make manually:

## 1. Fix seen_ir/src/codegen.rs

### Fix 1: Around line 108, after the Variable match arm
Replace:
```rust
        match declaration {
            Declaration::Function(func_decl) => {
                self.generate_function(func_decl)?;
            }
            Declaration::Variable(var_decl) => {
                self.generate_global_variable(var_decl)?;
            }
        }
```

With:
```rust
        match declaration {
            Declaration::Function(func_decl) => {
                self.generate_function(func_decl)?;
            }
            Declaration::Variable(var_decl) => {
                self.generate_global_variable(var_decl)?;
            }
            Declaration::Struct(_struct_decl) => {
                // TODO: Implement struct declaration code generation
                return Err(CodeGenError::UnsupportedFeature(
                    "Struct declarations not yet implemented in IR generation".to_string()
                ));
            }
        }
```

### Fix 2: Around line 388, before DeclarationStatement
Find the match statement in `generate_statement` and add the For case before `Statement::DeclarationStatement`:

```rust
            Statement::For(_for_stmt) => {
                // TODO: Implement for loop code generation
                return Err(CodeGenError::UnsupportedFeature(
                    "For loops not yet implemented in IR generation".to_string()
                ));
            }
            Statement::DeclarationStatement(decl) => self.generate_declaration(decl),
```

### Fix 3: Around line 511, after Expression::Assign
After the `Expression::Assign(assign) => self.generate_assignment(assign),` line, add:

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

## 2. Fix seen_ir/src/types.rs

Around line 40, after the Array type handling, add:

```rust
            Type::Struct(_struct_name) => {
                return Err(CodeGenError::UnsupportedFeature(
                    "Struct types not yet implemented in IR generation".to_string()
                ))
            }
```

## 3. Enable LLVM in seen_cli/Cargo.toml

Uncomment these lines (remove the # at the beginning):
- Line 13: `seen_ir = { path = "../seen_ir" }`
- Line 14: `# seen_ir = { path = "../seen_ir" }  # Disabled due to LLVM dependency`
- Line 41: `inkwell = { version = "0.6.0", features = ["llvm18-1"] }`

## 4. Enable LLVM in seen_compiler/Cargo.toml

Uncomment line 11:
- `inkwell = { version = "0.6.0", features = ["llvm18-1"] }`

## Test Command

After making all changes:
```bash
export LLVM_SYS_181_PREFIX=/usr/lib/llvm-18
cargo build --package seen_ir
cargo build --package seen_cli
cargo build --package seen_compiler
```

If successful, run the full build script:
```bash
./build_with_llvm.sh
```